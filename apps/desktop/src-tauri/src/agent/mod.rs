//! Autonomous work agent: the layers above Knov's collector and threads.
//!
//! ```text
//! activity → normalize → workflows / goals / state → skills → policy gate
//!          → runtime (plan, approve, execute, verify) → journal → learning
//! ```
//!
//! Everything here is local and deterministic. The agent never calls a model
//! provider; the only side effects are the bounded adapters in `actions`.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use chrono::{Datelike, Local, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

mod actions;
mod goals;
mod models;
mod normalize;
mod policy;
mod runtime;
mod skills;
mod store;
mod workflows;

pub use actions::{ActionHost, SystemHost};
#[cfg(test)]
pub use models::WorkflowStep;
pub use models::{
    ActionDecision, AgentOverview, AgentRun, AutonomyOverview, GrantRequest, ProposalResponse,
    Skill, SkillUpdate, Workflow, WorkflowProgress,
};

use actions::{catalog, validate_workspace_path};
use goals::{infer_goals, match_progress, GOAL_LOOKBACK_DAYS};
use models::{
    AutonomyGrant, AutonomyProposal, DetectedWorkspace, PolicyPreference, StateResource, StateStep,
    UnresolvedItem, WorkState,
};
use normalize::{normalize_events, split_sessions, step_title, SemanticStep};
use policy::{proposal_eligible, tendency};
use runtime::PlanContext;
use workflows::{mine_workflows, score_opportunity};

use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::Settings,
    platform::detected_editor_workspaces,
    prediction::sanitize_text,
};

const MINING_LOOKBACK_DAYS: i64 = 30;
const MINING_INTERVAL_SECONDS: i64 = 30 * 60;
const SNAPSHOT_INTERVAL_SECONDS: i64 = 10 * 60;
const PROGRESS_RECENCY_SECONDS: i64 = 30 * 60;
const CONTEXT_TRIGGER_COOLDOWN_SECONDS: i64 = 2 * 3_600;
const SCHEDULE_TRIGGER_COOLDOWN_SECONDS: i64 = 20 * 3_600;
const ATTENTION_WINDOW_SECONDS: i64 = 24 * 3_600;
const MINING_STATE_KEY: &str = "agent_mining_state";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct MiningState {
    last_mined_at: i64,
    mined_activity_at: i64,
    last_snapshot_at: i64,
    snapshot_activity_at: i64,
}

/// Re-mines workflows from the last 30 days of local activity.
pub fn refresh_workflows(db: &Database, settings: &Settings, now: i64) -> AppResult<Vec<Workflow>> {
    let events = db.agent_events(now - MINING_LOOKBACK_DAYS * 86_400, now)?;
    let sessions = split_sessions(normalize_events(&events, settings));
    db.store_mined_workflows(&mine_workflows(&sessions), now)?;
    let mut mining = mining_state(db)?;
    mining.last_mined_at = now;
    mining.mined_activity_at = db.latest_activity_at()?.unwrap_or(0);
    db.set_setting(MINING_STATE_KEY, &mining)?;
    workflows(db)
}

pub fn workflows(db: &Database) -> AppResult<Vec<Workflow>> {
    let skills = db.workflow_skill_ids()?;
    let outcomes = db.skill_outcomes()?;
    let workspaces = db.approved_workspaces()?;
    let mut workflows = db
        .stored_workflows()?
        .into_iter()
        .map(|stored| {
            let skill_id = skills.get(&stored.id).cloned();
            let outcome = skill_id.as_ref().and_then(|id| outcomes.get(id)).copied();
            let has_workspace =
                skills::workspace_for_thread(stored.stats.thread.as_deref(), &workspaces).is_some();
            let mut opportunity =
                score_opportunity(&stored.steps, &stored.stats, outcome, has_workspace);
            if stored.status == "dismissed" || !stored.active {
                opportunity.surfaced = false;
            }
            Workflow {
                title: stored
                    .user_title
                    .clone()
                    .unwrap_or_else(|| stored.generated_title.clone()),
                id: stored.id,
                generated_title: stored.generated_title,
                status: stored.status,
                active: stored.active,
                steps: stored.steps,
                stats: stored.stats,
                evidence: stored.evidence,
                opportunity,
                skill_id,
                first_seen_at: stored.first_seen_at,
                last_seen_at: stored.last_seen_at,
                updated_at: stored.updated_at,
            }
        })
        .collect::<Vec<_>>();
    workflows.sort_by(|left, right| {
        (right.status != "dismissed")
            .cmp(&(left.status != "dismissed"))
            .then_with(|| right.opportunity.score.total_cmp(&left.opportunity.score))
            .then_with(|| right.stats.occurrences.cmp(&left.stats.occurrences))
    });
    Ok(workflows)
}

pub fn review_workflow(
    db: &Database,
    id: &str,
    status: &str,
    title: Option<&str>,
    now: i64,
) -> AppResult<Workflow> {
    if !matches!(status, "discovered" | "confirmed" | "dismissed") {
        return Err(AppError::InvalidInput(
            "Unsupported workflow review.".into(),
        ));
    }
    let title = title.map(|value| sanitize_text(value, 80));
    if !db.set_workflow_review(id, status, title.as_deref(), now)? {
        return Err(AppError::InvalidInput(
            "That workflow no longer exists.".into(),
        ));
    }
    find_workflow(db, id)
}

fn find_workflow(db: &Database, id: &str) -> AppResult<Workflow> {
    workflows(db)?
        .into_iter()
        .find(|workflow| workflow.id == id)
        .ok_or_else(|| AppError::InvalidInput("That workflow no longer exists.".into()))
}

fn mining_state(db: &Database) -> AppResult<MiningState> {
    Ok(db.get_setting(MINING_STATE_KEY)?.unwrap_or_default())
}

fn observe(
    db: &Database,
    settings: &Settings,
    now: i64,
    seconds: i64,
) -> AppResult<Vec<SemanticStep>> {
    let events = db.agent_events(now - seconds, now)?;
    Ok(normalize_events(&events, settings))
}

pub fn work_state(db: &Database, settings: &Settings, now: i64) -> AppResult<WorkState> {
    let steps = observe(db, settings, now, GOAL_LOOKBACK_DAYS * 86_400)?;
    build_state(db, steps, now)
}

fn build_state(db: &Database, steps: Vec<SemanticStep>, now: i64) -> AppResult<WorkState> {
    let goals = infer_goals(&steps, &db.goal_reviews()?);
    let session = split_sessions(steps).pop().unwrap_or_default();
    let active_thread = session.iter().rev().find_map(|step| step.thread.clone());
    let goal = active_thread
        .as_deref()
        .and_then(|thread| goals.iter().find(|goal| goal.topic == thread))
        .or_else(|| goals.first())
        .cloned();
    let recent_steps = session
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|step| StateStep {
            label: step.label.clone(),
            title: step_title(step.category, &step.label),
            category: step.category.into(),
            at: step.started_at,
            seconds: step.seconds,
        })
        .collect::<Vec<_>>();
    let mut seen = std::collections::HashSet::new();
    let open_resources = session
        .iter()
        .rev()
        .filter(|step| step.kind != "search" && step.category != "sensitive")
        .filter(|step| seen.insert(step.locator.clone().unwrap_or_else(|| step.key.clone())))
        .take(6)
        .map(|step| StateResource {
            label: step
                .locator
                .as_deref()
                .map(actions::display_url)
                .unwrap_or_else(|| step.label.clone()),
            kind: step.kind.into(),
            locator: step.locator.clone(),
            at: step.started_at,
        })
        .collect::<Vec<_>>();
    let in_progress = session
        .last()
        .is_some_and(|step| now - step.ended_at.max(step.started_at) <= PROGRESS_RECENCY_SECONDS);
    let workflow_progress = if in_progress {
        match_progress(&session, &workflows(db)?)
    } else {
        None
    };

    let mut unresolved = Vec::new();
    for run in db.attention_runs(now - ATTENTION_WINDOW_SECONDS)? {
        let (kind, detail) = match run.status.as_str() {
            "awaiting_approval" => ("awaiting_approval", run.summary.clone()),
            "completed_with_exceptions" | "failed" | "blocked" => {
                ("needs_attention", run.summary.clone())
            }
            _ => continue,
        };
        unresolved.push(UnresolvedItem {
            kind: kind.into(),
            title: run.title.clone(),
            detail,
            run_id: Some(run.id.clone()),
            at: run.created_at,
        });
    }
    if let Some(progress) = &workflow_progress {
        unresolved.push(UnresolvedItem {
            kind: "workflow_in_progress".into(),
            title: format!("Unfinished: {}", progress.title),
            detail: format!(
                "Step {} of {} done; next is “{}”.",
                progress.matched_steps, progress.total_steps, progress.next_step.title
            ),
            run_id: None,
            at: now,
        });
    }
    Ok(WorkState {
        generated_at: now,
        active_thread,
        goal,
        goals,
        recent_steps,
        open_resources,
        workflow_progress,
        unresolved,
    })
}

pub fn overview(db: &Database, settings: &Settings, now: i64) -> AppResult<AgentOverview> {
    let state = work_state(db, settings, now)?;
    let workflows = workflows(db)?;
    Ok(AgentOverview {
        paused: settings.agent_paused,
        attention: db.attention_runs(now - ATTENTION_WINDOW_SECONDS)?,
        awaiting_count: db.count_runs("awaiting_approval")?,
        proposals: proposals(db, now)?,
        opportunity_count: workflows
            .iter()
            .filter(|workflow| workflow.opportunity.surfaced && workflow.skill_id.is_none())
            .count() as i64,
        workflow_count: workflows
            .iter()
            .filter(|workflow| workflow.status != "dismissed" && workflow.active)
            .count() as i64,
        state,
    })
}

pub fn review_goal(
    db: &Database,
    settings: &Settings,
    id: &str,
    status: &str,
    title: Option<&str>,
    now: i64,
) -> AppResult<AgentOverview> {
    if !id.starts_with("goal-")
        || !matches!(status, "confirmed" | "dismissed" | "completed" | "inferred")
    {
        return Err(AppError::InvalidInput("Unsupported goal review.".into()));
    }
    let title = title.map(|value| sanitize_text(value, 100));
    db.set_goal_review(id, status, title.as_deref(), now)?;
    overview(db, settings, now)
}

fn proposals(db: &Database, now: i64) -> AppResult<Vec<AutonomyProposal>> {
    let grants = db.active_grants(now)?;
    let dismissals = db.proposal_dismissals()?;
    Ok(db
        .action_histories()?
        .into_iter()
        .filter(|row| {
            let has_grant = grants.iter().any(|grant| {
                grant.action_type == row.action_type
                    && grant.scope_kind == row.scope_kind
                    && grant.scope_value == row.scope_value
            });
            let dismissed = dismissals
                .get(&(
                    row.action_type.clone(),
                    row.scope_kind.clone(),
                    row.scope_value.clone().unwrap_or_default(),
                ))
                .copied();
            actions::action_kind(&row.action_type).is_some_and(|kind| kind.available)
                && proposal_eligible(&row.history, has_grant, dismissed)
        })
        .map(|row| {
            let title = actions::action_kind(&row.action_type).map_or("this action", |kind| kind.title);
            AutonomyProposal {
                message: format!(
                    "You approved “{title}” for {} {} times and declined it 0 times. Allow it automatically there?",
                    row.scope_label, row.history.approvals
                ),
                action_type: row.action_type,
                action_title: title.into(),
                scope_kind: row.scope_kind,
                scope_value: row.scope_value,
                scope_label: row.scope_label,
                approvals: row.history.approvals,
            }
        })
        .collect())
}

pub fn autonomy(db: &Database, settings: &Settings, now: i64) -> AppResult<AutonomyOverview> {
    let workspaces = db.approved_workspaces()?;
    let detected_workspaces = detected_editor_workspaces(16)
        .into_iter()
        .filter(|(_, path)| {
            let path = path.to_string_lossy();
            !workspaces.iter().any(|workspace| workspace.path == path)
                && validate_workspace_path(&path).is_ok()
        })
        .take(8)
        .map(|(label, path)| DetectedWorkspace {
            label: sanitize_text(&label, 80),
            path: path.to_string_lossy().into_owned(),
        })
        .collect();
    let policy = db
        .action_histories()?
        .into_iter()
        .take(10)
        .map(|row| PolicyPreference {
            action_title: actions::action_kind(&row.action_type)
                .map_or(row.action_type.as_str(), |kind| kind.title)
                .into(),
            tendency: tendency(row.history.approvals, row.history.rejections, row.automatic).into(),
            action_type: row.action_type,
            scope_label: row.scope_label,
            approvals: row.history.approvals,
            rejections: row.history.rejections,
            automatic: row.automatic,
            rolled_back: row.history.rolled_back,
        })
        .collect();
    Ok(AutonomyOverview {
        paused: settings.agent_paused,
        max_actions_per_hour: settings.agent_max_actions_per_hour,
        auto_actions_last_hour: db.auto_actions_since(now - 3_600)?,
        catalog: catalog(),
        grants: db.active_grants(now)?,
        proposals: proposals(db, now)?,
        workspaces,
        detected_workspaces,
        policy,
        metrics: db.agent_metrics(now)?,
    })
}

pub fn skills(db: &Database) -> AppResult<Vec<Skill>> {
    db.skills()
}

/// Creating a skill confirms the workflow it was learned from.
pub fn create_skill(db: &Database, workflow_id: &str, now: i64) -> AppResult<Skill> {
    let workflow = find_workflow(db, workflow_id)?;
    if workflow.status == "dismissed" {
        return Err(AppError::InvalidInput(
            "Restore this workflow before turning it into a skill.".into(),
        ));
    }
    if let Some(existing) = workflow
        .skill_id
        .as_deref()
        .and_then(|id| db.skill(id).ok().flatten())
    {
        return Ok(existing);
    }
    db.set_workflow_review(&workflow.id, "confirmed", None, now)?;
    let skill = skills::skill_from_workflow(&workflow, &db.approved_workspaces()?, now);
    db.save_skill(&skill)?;
    Ok(skill)
}

pub fn update_skill(db: &Database, update: SkillUpdate, now: i64) -> AppResult<Skill> {
    let existing = db
        .skill(&update.id)?
        .ok_or_else(|| AppError::InvalidInput("That skill no longer exists.".into()))?;
    let updated = skills::apply_update(&existing, update, &db.approved_workspaces()?, now)?;
    db.save_skill(&updated)?;
    db.skill(&updated.id)?
        .ok_or_else(|| AppError::InvalidInput("That skill no longer exists.".into()))
}

pub fn delete_skill(db: &Database, id: &str, now: i64) -> AppResult<()> {
    if !db.delete_skill(id, now)? {
        return Err(AppError::InvalidInput(
            "That skill no longer exists.".into(),
        ));
    }
    Ok(())
}

pub fn start_run(
    db: &Database,
    settings: &Settings,
    skill_id: &str,
    origin: &str,
    now: i64,
) -> AppResult<AgentRun> {
    let skill = db
        .skill(skill_id)?
        .ok_or_else(|| AppError::InvalidInput("That skill no longer exists.".into()))?;
    if !skill.enabled && origin != "manual" {
        return Err(AppError::InvalidInput("That skill is turned off.".into()));
    }
    let steps = observe(db, settings, now, GOAL_LOOKBACK_DAYS * 86_400)?;
    let recent = steps
        .iter()
        .filter(|step| step.started_at >= now - 86_400)
        .cloned()
        .collect::<Vec<_>>();
    let state = build_state(db, steps, now)?;
    let grants = db.active_grants(now)?;
    let workspaces = db.approved_workspaces()?;
    let (run, actions) = runtime::plan_run(
        &skill,
        origin,
        &PlanContext {
            state: &state,
            recent: &recent,
            grants: &grants,
            workspaces: &workspaces,
            paused: settings.agent_paused,
            now,
        },
    )?;
    db.insert_run(&run, &actions)?;
    db.set_skill_triggered(&skill.id, now)?;
    db.run(&run.id)?
        .ok_or_else(|| AppError::InvalidInput("The run could not be created.".into()))
}

pub fn runs(db: &Database, limit: i64) -> AppResult<Vec<AgentRun>> {
    db.runs(limit)
}

pub fn run(db: &Database, id: &str) -> AppResult<AgentRun> {
    db.run(id)?
        .ok_or_else(|| AppError::InvalidInput("That run no longer exists.".into()))
}

pub fn decide_run(
    db: &Database,
    run_id: &str,
    decisions: &[ActionDecision],
    now: i64,
) -> AppResult<AgentRun> {
    runtime::decide_run(db, run_id, decisions, now)?;
    run(db, run_id)
}

pub fn cancel_run(db: &Database, run_id: &str, now: i64) -> AppResult<AgentRun> {
    runtime::cancel_run(db, run_id, now)?;
    run(db, run_id)
}

pub fn acknowledge_run(db: &Database, run_id: &str, now: i64) -> AppResult<()> {
    if !db.acknowledge_run(run_id, now)? {
        return Err(AppError::InvalidInput("That run no longer exists.".into()));
    }
    Ok(())
}

pub fn rollback_action(
    db: &Database,
    host: &dyn ActionHost,
    action_id: &str,
    now: i64,
) -> AppResult<AgentRun> {
    let run_id = runtime::rollback_action(db, host, action_id, now)?;
    run(db, &run_id)
}

pub fn open_draft(db: &Database, host: &dyn ActionHost, action_id: &str) -> AppResult<()> {
    let action = db
        .stored_action(action_id)?
        .ok_or_else(|| AppError::InvalidInput("That action no longer exists.".into()))?;
    let plan = action
        .rollback
        .filter(|_| action.action_type == "write_draft" && action.status == "succeeded")
        .ok_or_else(|| AppError::InvalidInput("There is no draft to open.".into()))?;
    let path = actions::draft_path_for_open(&plan, host)?;
    host.open_file(&path)
        .map_err(|error| AppError::InvalidInput(format!("Could not open the draft: {error}")))
}

pub fn put_grant(
    db: &Database,
    settings: &Settings,
    request: GrantRequest,
    now: i64,
) -> AppResult<AutonomyOverview> {
    let kind = actions::action_kind(&request.action_type)
        .filter(|kind| kind.available)
        .ok_or_else(|| AppError::InvalidInput("That action cannot be granted.".into()))?;
    if !matches!(request.mode.as_str(), "auto" | "ask" | "never") {
        return Err(AppError::InvalidInput(
            "Unsupported permission mode.".into(),
        ));
    }
    let scope_label = match (request.scope_kind.as_str(), request.scope_value.as_deref()) {
        ("global", None) => "everywhere".to_string(),
        ("skill", Some(id)) => db
            .skill(id)?
            .map(|skill| format!("“{}”", skill.name))
            .ok_or_else(|| AppError::InvalidInput("That skill no longer exists.".into()))?,
        ("workspace", Some(id)) if kind.id == "run_checks" => db
            .approved_workspaces()?
            .into_iter()
            .find(|workspace| workspace.id == id)
            .map(|workspace| format!("the {} workspace", workspace.label))
            .ok_or_else(|| AppError::InvalidInput("That workspace is not approved.".into()))?,
        _ => {
            return Err(AppError::InvalidInput(
                "Unsupported permission scope.".into(),
            ))
        }
    };
    let expires_at = match request.duration_days {
        Some(days) if (1..=365).contains(&days) => Some(now + days * 86_400),
        Some(_) => {
            return Err(AppError::InvalidInput(
                "Choose a duration between 1 and 365 days.".into(),
            ))
        }
        None => None,
    };
    db.put_grant(
        &AutonomyGrant {
            id: format!("grant-{}", Uuid::new_v4()),
            action_type: kind.id.into(),
            scope_kind: request.scope_kind,
            scope_value: request.scope_value,
            scope_label,
            mode: request.mode,
            source: "user".into(),
            created_at: now,
            expires_at,
        },
        now,
    )?;
    autonomy(db, settings, now)
}

pub fn revoke_grant(
    db: &Database,
    settings: &Settings,
    id: &str,
    now: i64,
) -> AppResult<AutonomyOverview> {
    if !db.revoke_grant(id, now)? {
        return Err(AppError::InvalidInput(
            "That permission is no longer active.".into(),
        ));
    }
    autonomy(db, settings, now)
}

pub fn respond_proposal(
    db: &Database,
    settings: &Settings,
    response: ProposalResponse,
    now: i64,
) -> AppResult<AutonomyOverview> {
    let proposal = proposals(db, now)?
        .into_iter()
        .find(|proposal| {
            proposal.action_type == response.action_type
                && proposal.scope_kind == response.scope_kind
                && proposal.scope_value == response.scope_value
        })
        .ok_or_else(|| AppError::InvalidInput("That suggestion is no longer available.".into()))?;
    if response.accept {
        db.put_grant(
            &AutonomyGrant {
                id: format!("grant-{}", Uuid::new_v4()),
                action_type: proposal.action_type,
                scope_kind: proposal.scope_kind,
                scope_value: proposal.scope_value,
                scope_label: proposal.scope_label,
                mode: "auto".into(),
                source: "proposal".into(),
                created_at: now,
                expires_at: None,
            },
            now,
        )?;
    } else {
        db.dismiss_proposal(
            &proposal.action_type,
            &proposal.scope_kind,
            proposal.scope_value.as_deref(),
            proposal.approvals,
            now,
        )?;
    }
    autonomy(db, settings, now)
}

pub fn approve_workspace(
    db: &Database,
    settings: &Settings,
    path: &str,
    now: i64,
) -> AppResult<AutonomyOverview> {
    let (canonical, label) = validate_workspace_path(path)?;
    db.insert_workspace(
        &format!("ws-{}", Uuid::new_v4()),
        &label,
        &canonical.to_string_lossy(),
        now,
    )?;
    autonomy(db, settings, now)
}

pub fn remove_workspace(
    db: &Database,
    settings: &Settings,
    id: &str,
    now: i64,
) -> AppResult<AutonomyOverview> {
    if !db.remove_workspace(id, now)? {
        return Err(AppError::InvalidInput(
            "That workspace is no longer approved.".into(),
        ));
    }
    autonomy(db, settings, now)
}

/// Marks actions left running by a previous process as failed.
pub fn recover(db: &Database, now: i64) -> AppResult<usize> {
    db.fail_interrupted_actions(now)
}

/// Drains approved actions on a background thread. Only one executor runs at
/// a time; a lost wake-up is recovered by the next scheduler tick.
pub fn spawn_executor(db: Arc<Database>, host: Arc<dyn ActionHost>, lock: Arc<AtomicBool>) {
    if lock
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || loop {
        for _ in 0..100 {
            match runtime::execute_next(&db, host.as_ref(), &|| Utc::now().timestamp()) {
                Ok(true) => continue,
                Ok(false) => break,
                Err(error) => {
                    eprintln!("agent action execution stopped: {error}");
                    break;
                }
            }
        }
        lock.store(false, Ordering::Release);
        let more = db.has_approved_actions().unwrap_or(false);
        if !more
            || lock
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            break;
        }
    });
}

/// Called once a minute: mines workflows when new activity arrived, records
/// state snapshots for later evaluation, fires due skill triggers, and drains
/// approved actions.
pub fn scheduler_tick(
    db: &Arc<Database>,
    host: &Arc<dyn ActionHost>,
    lock: &Arc<AtomicBool>,
    now: i64,
) -> AppResult<()> {
    let settings = db.settings()?;
    let latest_activity = db.latest_activity_at()?.unwrap_or(0);
    let mining = mining_state(db)?;
    if now - mining.last_mined_at >= MINING_INTERVAL_SECONDS
        && latest_activity > mining.mined_activity_at
    {
        refresh_workflows(db, &settings, now)?;
    }
    if settings.collection_enabled {
        let mut mining = mining_state(db)?;
        if now - mining.last_snapshot_at >= SNAPSHOT_INTERVAL_SECONDS
            && latest_activity > mining.snapshot_activity_at
        {
            let state = work_state(db, &settings, now)?;
            db.insert_state_snapshot(now, &snapshot_json(&state))?;
            mining.last_snapshot_at = now;
            mining.snapshot_activity_at = latest_activity;
            db.set_setting(MINING_STATE_KEY, &mining)?;
        }
    }
    if !settings.agent_paused {
        trigger_skills(db, &settings, now)?;
    }
    spawn_executor(db.clone(), host.clone(), lock.clone());
    Ok(())
}

fn snapshot_json(state: &WorkState) -> String {
    json!({
        "activeThread": state.active_thread,
        "goal": state.goal.as_ref().map(|goal| json!({"id": goal.id, "title": goal.title, "status": goal.status})),
        "recentSteps": state.recent_steps.iter().map(|step| json!({"label": step.label, "category": step.category, "at": step.at})).collect::<Vec<_>>(),
        "workflowProgress": state.workflow_progress.as_ref().map(|progress| json!({
            "workflowId": progress.workflow_id,
            "matched": progress.matched_steps,
            "total": progress.total_steps,
            "nextStep": progress.next_step.key,
            "confidence": progress.confidence
        })),
        "unresolved": state.unresolved.iter().map(|item| item.kind.clone()).collect::<Vec<_>>()
    })
    .to_string()
}

fn trigger_skills(db: &Database, settings: &Settings, now: i64) -> AppResult<()> {
    let candidates = db
        .skills()?
        .into_iter()
        .filter(|skill| skill.enabled && skill.trigger.kind != "manual")
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(());
    }
    let local = Local
        .timestamp_opt(now, 0)
        .single()
        .ok_or_else(|| AppError::InvalidInput("Local time is unavailable.".into()))?;
    let mut progress = None::<Option<WorkflowProgress>>;
    for skill in candidates {
        if db.open_run_for_skill(&skill.id)? {
            continue;
        }
        let since_last = skill.last_triggered_at.map_or(i64::MAX, |last| now - last);
        let due = match skill.trigger.kind.as_str() {
            "schedule" => {
                skill.trigger.hour == Some(local.hour())
                    && skill
                        .trigger
                        .weekday
                        .is_none_or(|day| day == local.weekday().num_days_from_monday())
                    && since_last >= SCHEDULE_TRIGGER_COOLDOWN_SECONDS
            }
            "context"
                if settings.collection_enabled
                    && since_last >= CONTEXT_TRIGGER_COOLDOWN_SECONDS =>
            {
                let current = match &progress {
                    Some(value) => value.clone(),
                    None => {
                        let value = current_progress(db, settings, now)?;
                        progress = Some(value.clone());
                        value
                    }
                };
                current.is_some_and(|progress| Some(progress.workflow_id) == skill.workflow_id)
            }
            _ => false,
        };
        if !due {
            continue;
        }
        let origin = if skill.trigger.kind == "schedule" {
            "schedule"
        } else {
            "context"
        };
        if let Err(error) = start_run(db, settings, &skill.id, origin, now) {
            eprintln!("skill trigger did not create a run: {error}");
            db.set_skill_triggered(&skill.id, now)?;
        }
    }
    Ok(())
}

/// The workflow the user appears to be in the middle of, from the last two
/// hours of activity.
fn current_progress(
    db: &Database,
    settings: &Settings,
    now: i64,
) -> AppResult<Option<WorkflowProgress>> {
    let steps = observe(db, settings, now, 2 * 3_600)?;
    let session = split_sessions(steps).pop().unwrap_or_default();
    let recent = session
        .last()
        .is_some_and(|step| now - step.ended_at.max(step.started_at) <= PROGRESS_RECENCY_SECONDS);
    if !recent {
        return Ok(None);
    }
    Ok(match_progress(&session, &workflows(db)?))
}

/// Agent context attached to every prediction batch: the inferred goal (the
/// top of the prediction hierarchy) and, when the user is mid-workflow, the
/// grounded next step for the `workflow` prediction source.
pub(crate) struct PredictionContext {
    pub goal: Option<String>,
    pub progress: Option<WorkflowProgress>,
    pub workflow_thread: Option<String>,
}

pub(crate) fn prediction_context(
    db: &Database,
    settings: &Settings,
    now: i64,
) -> AppResult<PredictionContext> {
    let state = work_state(db, settings, now)?;
    let workflow_thread = match &state.workflow_progress {
        Some(progress) => workflows(db)?
            .into_iter()
            .find(|workflow| workflow.id == progress.workflow_id)
            .and_then(|workflow| workflow.stats.thread),
        None => None,
    };
    Ok(PredictionContext {
        goal: state.goal.map(|goal| goal.title),
        progress: state.workflow_progress,
        workflow_thread,
    })
}

/// Read-only summary used by the delete-everything and retention tests.
#[cfg(test)]
pub(crate) fn table_counts(db: &Database) -> std::collections::HashMap<&'static str, i64> {
    [
        "workflows",
        "skills",
        "agent_runs",
        "agent_actions",
        "autonomy_grants",
        "approved_workspaces",
        "goal_reviews",
        "state_snapshots",
    ]
    .into_iter()
    .map(|table| {
        let count = db
            .conn()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        (table, count)
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agent::normalize::tests::{focus, visit},
        models::ActivityEvent,
    };

    fn seed_developer_days(db: &Database, days: i64, now: i64) {
        let mut index = 0;
        for day in 0..days {
            let base = now - (days - day) * 86_400;
            let events: Vec<ActivityEvent> = vec![
                visit(0, base, "https://github.com/acme/knov/issues/12"),
                focus(0, base + 120, "Code", 900),
                focus(0, base + 1_100, "Terminal", 240),
            ];
            for mut event in events {
                index += 1;
                event.id = None;
                db.insert_event(&event, &format!("seed-{index}")).unwrap();
            }
        }
    }

    #[test]
    fn mining_persists_workflows_and_preserves_reviews_across_rescans() {
        let db = Database::in_memory().unwrap();
        let now = 1_800_000_000;
        seed_developer_days(&db, 4, now);
        let settings = Settings::default();
        let mined = refresh_workflows(&db, &settings, now).unwrap();
        assert_eq!(mined.len(), 1);
        let workflow = &mined[0];
        assert_eq!(workflow.steps.len(), 3);
        assert_eq!(workflow.status, "discovered");

        review_workflow(&db, &workflow.id, "confirmed", Some("Issue triage"), now).unwrap();
        let rescanned = refresh_workflows(&db, &settings, now + 60).unwrap();
        assert_eq!(rescanned[0].title, "Issue triage");
        assert_eq!(rescanned[0].status, "confirmed");

        // A confirmed workflow that stops recurring keeps its review but loses evidence.
        db.conn()
            .execute("DELETE FROM activity_events", [])
            .unwrap();
        let gone = refresh_workflows(&db, &settings, now + 120).unwrap();
        assert_eq!(gone.len(), 1);
        assert!(!gone[0].active);
        assert!(gone[0].evidence.is_empty());
        assert!(!gone[0].opportunity.surfaced);
    }

    #[test]
    fn skills_runs_and_proposals_flow_end_to_end() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::in_memory().unwrap();
        let now = 1_800_000_000;
        seed_developer_days(&db, 4, now);
        let settings = Settings::default();
        let workflow = refresh_workflows(&db, &settings, now).unwrap().remove(0);
        let skill = create_skill(&db, &workflow.id, now).unwrap();
        assert_eq!(
            find_workflow(&db, &workflow.id).unwrap().status,
            "confirmed"
        );
        assert_eq!(create_skill(&db, &workflow.id, now).unwrap().id, skill.id);

        let host = actions::tests::FakeHost::new(directory.path().join("drafts"));
        for round in 0..policy::PROPOSAL_MIN_APPROVALS {
            let run = start_run(&db, &settings, &skill.id, "manual", now + round * 10).unwrap();
            let decisions = run
                .actions
                .iter()
                .map(|action| ActionDecision {
                    action_id: action.id.clone(),
                    approved: true,
                    remember: false,
                })
                .collect::<Vec<_>>();
            decide_run(&db, &run.id, &decisions, now + round * 10 + 1).unwrap();
            while runtime::execute_next(&db, &host, &|| now + round * 10 + 2).unwrap() {}
        }
        let overview = autonomy(&db, &settings, now + 100).unwrap();
        assert!(!overview.proposals.is_empty());
        assert_eq!(overview.metrics.task_completion_rate, Some(1.0));
        let proposal = overview.proposals[0].clone();
        let accepted = respond_proposal(
            &db,
            &settings,
            ProposalResponse {
                action_type: proposal.action_type.clone(),
                scope_kind: proposal.scope_kind.clone(),
                scope_value: proposal.scope_value.clone(),
                accept: true,
            },
            now + 200,
        )
        .unwrap();
        assert!(accepted
            .grants
            .iter()
            .any(|grant| grant.action_type == proposal.action_type && grant.source == "proposal"));
        assert!(!accepted
            .proposals
            .iter()
            .any(|candidate| candidate.action_type == proposal.action_type
                && candidate.scope_value == proposal.scope_value));
        assert!(skills(&db).unwrap()[0].stats.completed >= 5);
        assert!(workflows(&db).unwrap()[0].skill_id.is_some());
    }

    #[test]
    fn state_reports_goals_progress_and_snapshots_stay_sanitized() {
        let db = Database::in_memory().unwrap();
        let now = Utc::now().timestamp();
        seed_developer_days(&db, 4, now);
        let settings = Settings {
            collection_enabled: true,
            ..Settings::default()
        };
        db.save_settings(&settings).unwrap();
        refresh_workflows(&db, &settings, now).unwrap();
        // Start the workflow again right now.
        db.insert_event(
            &visit(
                0,
                now - 300,
                "https://github.com/acme/knov/issues/13?token=abc",
            ),
            "live-1",
        )
        .unwrap();
        db.insert_event(&focus(0, now - 200, "Code", 120), "live-2")
            .unwrap();
        let state = work_state(&db, &settings, now).unwrap();
        let progress = state
            .workflow_progress
            .clone()
            .expect("workflow in progress");
        assert_eq!(progress.matched_steps, 2);
        assert_eq!(progress.next_step.label, "Terminal");
        assert!(state
            .unresolved
            .iter()
            .any(|item| item.kind == "workflow_in_progress"));
        assert!(state.open_resources.iter().all(|resource| resource
            .locator
            .as_deref()
            .is_none_or(|locator| !locator.contains("token"))));

        let context = prediction_context(&db, &settings, now).unwrap();
        assert_eq!(
            context.progress.expect("prediction").next_step.label,
            "Terminal"
        );

        let host: Arc<dyn ActionHost> = Arc::new(actions::tests::FakeHost::default());
        let db = Arc::new(db);
        scheduler_tick(&db, &host, &Arc::new(AtomicBool::new(false)), now).unwrap();
        assert_eq!(db.state_snapshot_count().unwrap(), 1);
        let snapshot: String = db
            .conn()
            .query_row("SELECT snapshot_json FROM state_snapshots", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(!snapshot.contains("token"));
        assert!(!snapshot.contains("Private page title"));
    }

    #[test]
    fn delete_everything_removes_agent_state() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::in_memory().unwrap();
        let now = 1_800_000_000;
        seed_developer_days(&db, 4, now);
        let settings = Settings::default();
        let workflow = refresh_workflows(&db, &settings, now).unwrap().remove(0);
        let skill = create_skill(&db, &workflow.id, now).unwrap();
        start_run(&db, &settings, &skill.id, "manual", now).unwrap();
        db.set_goal_review("goal-x", "confirmed", None, now)
            .unwrap();
        db.insert_state_snapshot(now, "{}").unwrap();
        let project = directory.path().join("p");
        std::fs::create_dir_all(&project).unwrap();
        db.insert_workspace("ws", "p", &project.to_string_lossy(), now)
            .unwrap();
        assert!(
            table_counts(&db)
                .values()
                .filter(|count| **count > 0)
                .count()
                >= 6
        );
        db.delete_all_local_data().unwrap();
        assert!(table_counts(&db).values().all(|count| *count == 0));
        assert!(!db.settings().unwrap().agent_paused);
    }
}
