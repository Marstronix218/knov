//! Run lifecycle: plan → authorize → (approve) → execute → verify → journal.
//! Every planned action is persisted before anything happens, so what the
//! user approves is exactly what runs.

use std::collections::HashSet;

use chrono::{Local, TimeZone};
use uuid::Uuid;

use super::{
    actions::{
        action_kind, check_preset, display_url, execute, ActionHost, ActionSpec, ResolvedAction,
    },
    models::{ActionDecision, ApprovedWorkspace, AutonomyGrant, RunState, Skill, WorkState},
    normalize::SemanticStep,
    policy::{authorize, ActionScope, Decision},
    skills::render_draft,
    store::{target_label, NewAction, NewRun},
};
use crate::{
    db::Database,
    error::{AppError, AppResult},
    platform::{normalized_application_name, reopenable_web_url},
    prediction::sanitize_text,
};

pub(crate) const MAX_ACTIONS_PER_RUN: usize = 10;

pub(crate) struct PlanContext<'a> {
    pub state: &'a WorkState,
    pub recent: &'a [SemanticStep],
    pub grants: &'a [AutonomyGrant],
    pub workspaces: &'a [ApprovedWorkspace],
    pub paused: bool,
    pub now: i64,
}

pub(crate) fn plan_run(
    skill: &Skill,
    origin: &str,
    context: &PlanContext<'_>,
) -> AppResult<(NewRun, Vec<NewAction>)> {
    let unattended = origin != "manual";
    let mut actions = Vec::<NewAction>::new();
    let mut manual_steps = Vec::new();
    let mut seen_targets = HashSet::new();
    for step in skill.steps.iter().filter(|step| step.enabled) {
        let Some(spec) = &step.action else {
            manual_steps.push(step.title.clone());
            continue;
        };
        let kind = action_kind(spec.action_type()).expect("catalogued action");
        let scope = ActionScope {
            skill_id: Some(skill.id.clone()),
            workspace_id: match spec {
                ActionSpec::RunChecks { workspace_id, .. } => Some(workspace_id.clone()),
                _ => None,
            },
        };
        let (scope_kind, scope_value) = scope.primary(kind.id);
        let scope_label = scope_label(
            scope_kind,
            scope_value.as_deref(),
            skill,
            context.workspaces,
        );
        if actions.len() >= MAX_ACTIONS_PER_RUN {
            manual_steps.push(format!(
                "{} (over the {MAX_ACTIONS_PER_RUN}-action limit)",
                step.title
            ));
            continue;
        }
        let mut action = NewAction {
            id: format!("act-{}", Uuid::new_v4()),
            step_id: step.id.clone(),
            position: actions.len() as i64 + 1,
            action_type: kind.id.into(),
            title: step.title.clone(),
            risk_class: kind.risk.as_str().into(),
            resolved: None,
            target_label: step.title.clone(),
            rationale: String::new(),
            scope_kind: scope_kind.into(),
            scope_value,
            scope_label,
            decision: "blocked".into(),
            decision_reason: String::new(),
            grant_id: None,
            status: "blocked".into(),
        };
        match resolve(spec, skill, context) {
            Err(reason) => {
                action.rationale = format!("Step “{}” could not be prepared safely.", step.title);
                action.decision_reason = reason;
            }
            Ok((resolved, rationale)) => {
                let label = target_label(&resolved);
                if !seen_targets.insert(format!("{}:{label}", kind.id)) {
                    continue;
                }
                action.target_label = label;
                action.rationale = rationale;
                action.resolved = Some(resolved);
                let (status, decision, reason, grant_id) =
                    match authorize(kind, &scope, context.grants, context.paused, unattended) {
                        // Manual runs always pause for the user's "Run", even when
                        // everything is pre-approved by a grant.
                        Decision::Allow { grant_id, reason } => (
                            if unattended {
                                "approved"
                            } else {
                                "awaiting_approval"
                            },
                            "auto",
                            reason,
                            grant_id,
                        ),
                        Decision::Ask { reason } => ("awaiting_approval", "pending", reason, None),
                        Decision::Deny { reason } => ("blocked", "blocked", reason, None),
                    };
                action.status = status.into();
                action.decision = decision.into();
                action.decision_reason = reason;
                action.grant_id = grant_id;
            }
        }
        actions.push(action);
    }
    if actions.is_empty() {
        return Err(AppError::InvalidInput(
            "This skill has no enabled steps Knov can do. Enable an action step first.".into(),
        ));
    }
    let run = NewRun {
        id: format!("run-{}", Uuid::new_v4()),
        skill_id: Some(skill.id.clone()),
        title: skill.name.clone(),
        origin: origin.into(),
        status: "awaiting_approval".into(),
        on_exception: skill.on_exception.clone(),
        state: run_state(context.state),
        manual_steps,
        created_at: context.now,
    };
    Ok((run, actions))
}

fn run_state(state: &WorkState) -> RunState {
    RunState {
        thread: state.active_thread.clone(),
        goal: state.goal.as_ref().map(|goal| goal.title.clone()),
        workflow: state.workflow_progress.as_ref().map(|progress| {
            format!(
                "{} (step {} of {})",
                progress.title, progress.matched_steps, progress.total_steps
            )
        }),
        // Chronological, oldest first, so "A → B" reads in the order it happened.
        recent_steps: state.recent_steps[state.recent_steps.len().saturating_sub(4)..]
            .iter()
            .map(|step| step.title.clone())
            .collect(),
    }
}

fn scope_label(
    scope_kind: &str,
    scope_value: Option<&str>,
    skill: &Skill,
    workspaces: &[ApprovedWorkspace],
) -> String {
    match scope_kind {
        "workspace" => workspaces
            .iter()
            .find(|workspace| Some(workspace.id.as_str()) == scope_value)
            .map(|workspace| format!("the {} workspace", workspace.label))
            .unwrap_or_else(|| "this workspace".into()),
        "skill" => format!("“{}”", skill.name),
        _ => "everywhere".into(),
    }
}

fn resolve(
    spec: &ActionSpec,
    skill: &Skill,
    context: &PlanContext<'_>,
) -> Result<(ResolvedAction, String), String> {
    match spec {
        ActionSpec::OpenUrl {
            url,
            resolve_domain,
        } => {
            let latest = resolve_domain.as_deref().and_then(|domain| {
                context
                    .recent
                    .iter()
                    .rev()
                    .filter(|step| step.kind == "web" && step_domain(&step.key) == Some(domain))
                    .filter(|step| skill.thread.is_some() && step.thread == skill.thread)
                    .find_map(|step| step.locator.clone())
            });
            let (target, rationale) = match latest {
                Some(locator) => (
                    locator,
                    format!(
                        "Reopens the latest {} page from this thread, the next resource this workflow uses.",
                        resolve_domain.as_deref().unwrap_or("web")
                    ),
                ),
                None => (
                    url.clone(),
                    format!("Opens {}, where this workflow usually continues.", display_url(url)),
                ),
            };
            let parsed = reopenable_web_url(&target).map_err(|error| error.to_string())?;
            Ok((
                ResolvedAction::OpenUrl {
                    url: parsed.to_string(),
                },
                rationale,
            ))
        }
        ActionSpec::OpenApplication { app } => {
            let app = normalized_application_name(app).map_err(|error| error.to_string())?;
            let rationale =
                format!("Brings {app} forward, as this workflow usually does at this step.");
            Ok((ResolvedAction::OpenApplication { app }, rationale))
        }
        ActionSpec::WriteDraft { template } => {
            let (title, content) = render_draft(template, skill, context.state, context.now);
            let date = Local
                .timestamp_opt(context.now, 0)
                .single()
                .map(|value| value.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "draft".into());
            let file_stem = format!(
                "{date}-{}-{}",
                slug(&title),
                &Uuid::new_v4().simple().to_string()[..6]
            );
            Ok((
                ResolvedAction::WriteDraft {
                    title,
                    file_stem,
                    content,
                },
                "Saves a Markdown draft in Knov's Drafts folder for you to review. Nothing is sent anywhere."
                    .into(),
            ))
        }
        ActionSpec::RunChecks {
            workspace_id,
            preset,
        } => {
            let workspace = context
                .workspaces
                .iter()
                .find(|workspace| &workspace.id == workspace_id)
                .ok_or_else(|| "The workspace is no longer approved.".to_string())?;
            if !workspace
                .check_presets
                .iter()
                .any(|candidate| &candidate.id == preset)
            {
                return Err("That check is no longer available in the workspace.".into());
            }
            let label = check_preset(preset).map_or(preset.as_str(), |preset| preset.label);
            Ok((
                ResolvedAction::RunChecks {
                    workspace_id: workspace.id.clone(),
                    path: workspace.path.clone(),
                    preset: preset.clone(),
                },
                format!(
                    "Runs `{label}` in {} so results are ready when you get there. Output stays on this Mac.",
                    workspace.label
                ),
            ))
        }
    }
}

fn step_domain(key: &str) -> Option<&str> {
    key.strip_prefix("web:")
        .map(|rest| rest.split('/').next().unwrap_or(rest))
}

fn slug(value: &str) -> String {
    let slug = value
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = slug.chars().take(40).collect::<String>();
    if slug.is_empty() {
        "draft".into()
    } else {
        slug.trim_end_matches('-').to_string()
    }
}

pub(crate) fn decide_run(
    db: &Database,
    run_id: &str,
    decisions: &[ActionDecision],
    now: i64,
) -> AppResult<()> {
    if db.run(run_id)?.is_none() {
        return Err(AppError::InvalidInput("That run no longer exists.".into()));
    }
    for action in db.awaiting_actions(run_id)? {
        let Some(choice) = decisions
            .iter()
            .find(|choice| choice.action_id == action.id)
        else {
            continue;
        };
        if !choice.approved {
            db.set_action_decision(
                &action.id,
                "rejected",
                "rejected",
                "You declined this step.",
                None,
            )?;
            continue;
        }
        let pre_approved = action.decision == "auto";
        let mut grant_id = None;
        if choice.remember && !pre_approved {
            let grant = AutonomyGrant {
                id: format!("grant-{}", Uuid::new_v4()),
                action_type: action.action_type.clone(),
                scope_kind: action.scope_kind.clone(),
                scope_value: action.scope_value.clone(),
                scope_label: action.scope_label.clone(),
                mode: "auto".into(),
                source: "approval".into(),
                created_at: now,
                expires_at: None,
            };
            db.put_grant(&grant, now)?;
            grant_id = Some(grant.id);
        }
        let (decision, reason) = match (pre_approved, grant_id.is_some()) {
            (true, _) => ("auto", "Allowed by your grant; you started this run."),
            (false, true) => ("approved", "You approved this and allowed it from now on."),
            (false, false) => ("approved", "You approved this step."),
        };
        db.set_action_decision(
            &action.id,
            "approved",
            decision,
            reason,
            grant_id.as_deref(),
        )?;
    }
    db.refresh_run_status(run_id, now)?;
    Ok(())
}

/// Closes a run the user did not act on. Cancelled steps do not count as
/// declined when Knov learns your preferences.
pub(crate) fn cancel_run(db: &Database, run_id: &str, now: i64) -> AppResult<()> {
    for action in db.awaiting_actions(run_id)? {
        db.set_action_decision(
            &action.id,
            "skipped",
            "cancelled",
            "Run closed without a decision.",
            None,
        )?;
    }
    db.refresh_run_status(run_id, now)?;
    Ok(())
}

/// Executes the next approved action, failing closed on pause, budget, and
/// earlier exceptions. Returns false when nothing is waiting.
pub(crate) fn execute_next(
    db: &Database,
    host: &dyn ActionHost,
    clock: &dyn Fn() -> i64,
) -> AppResult<bool> {
    let settings = db.settings()?;
    let Some(action) = db.next_approved_action()? else {
        return Ok(false);
    };
    let now = clock();
    let skip_reason = if settings.agent_paused {
        Some("Agent execution was paused before this ran.")
    } else if action.on_exception == "stop"
        && db.run_has_exception_before(&action.run_id, action.position)?
    {
        Some("Stopped because an earlier step needs attention.")
    } else {
        None
    };
    if let Some(reason) = skip_reason {
        db.skip_action(&action.id, reason, now)?;
        db.refresh_run_status(&action.run_id, now)?;
        return Ok(true);
    }
    if action.decision == "auto"
        && db.auto_actions_since(now - 3_600)? >= settings.agent_max_actions_per_hour.max(1)
    {
        db.block_action(
            &action.id,
            "The hourly budget for automatic actions was reached.",
            now,
        )?;
        db.refresh_run_status(&action.run_id, now)?;
        return Ok(true);
    }
    let Some(resolved) = action.resolved.clone() else {
        db.block_action(
            &action.id,
            "The planned action could not be read back.",
            now,
        )?;
        db.refresh_run_status(&action.run_id, now)?;
        return Ok(true);
    };
    if !db.start_action(&action.id, now)? {
        return Ok(true);
    }
    db.refresh_run_status(&action.run_id, now)?;
    let approved_paths = db
        .approved_workspaces()?
        .into_iter()
        .map(|workspace| (workspace.id, workspace.path))
        .collect::<Vec<_>>();
    let execution = execute(&resolved, host, &approved_paths);
    let finished = clock();
    db.finish_action(&action.id, &execution, finished)?;
    db.refresh_run_status(&action.run_id, finished)?;
    Ok(true)
}

pub(crate) fn rollback_action(
    db: &Database,
    host: &dyn ActionHost,
    action_id: &str,
    now: i64,
) -> AppResult<String> {
    let action = db
        .stored_action(action_id)?
        .ok_or_else(|| AppError::InvalidInput("That action no longer exists.".into()))?;
    if action.status != "succeeded" {
        return Err(AppError::InvalidInput(
            "Only completed actions can be undone.".into(),
        ));
    }
    let plan = action
        .rollback
        .ok_or_else(|| AppError::InvalidInput("This action has nothing to undo.".into()))?;
    let summary = super::actions::rollback(&plan, host)?;
    db.mark_rolled_back(&action.id, &sanitize_text(&summary, 200))?;
    db.refresh_run_status(&action.run_id, now)?;
    Ok(action.run_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{
        actions::tests::FakeHost,
        models::{CheckPresetView, SkillStats, SkillStep, SkillTrigger},
        workflows::tests::step,
    };

    fn skill(steps: Vec<SkillStep>) -> Skill {
        Skill {
            id: "skill-1".into(),
            name: "Build and test · Knov desktop".into(),
            description: String::new(),
            workflow_id: Some("wf-1".into()),
            thread: Some("Knov desktop".into()),
            trigger: SkillTrigger {
                kind: "manual".into(),
                weekday: None,
                hour: None,
            },
            on_exception: "stop".into(),
            steps,
            enabled: true,
            created_at: 0,
            updated_at: 0,
            last_triggered_at: None,
            stats: SkillStats::default(),
        }
    }

    fn action_step(id: &str, action: ActionSpec) -> SkillStep {
        SkillStep {
            id: id.into(),
            title: format!("Step {id}"),
            category: "code".into(),
            action: Some(action),
            enabled: true,
        }
    }

    fn state() -> WorkState {
        WorkState {
            generated_at: 0,
            active_thread: Some("Knov desktop".into()),
            goal: None,
            goals: vec![],
            recent_steps: vec![],
            open_resources: vec![],
            workflow_progress: None,
            unresolved: vec![],
        }
    }

    fn standard_skill(workspace: &ApprovedWorkspace) -> Skill {
        skill(vec![
            action_step(
                "1",
                ActionSpec::OpenUrl {
                    url: "https://github.com".into(),
                    resolve_domain: Some("github.com".into()),
                },
            ),
            action_step("2", ActionSpec::OpenApplication { app: "Code".into() }),
            action_step(
                "3",
                ActionSpec::RunChecks {
                    workspace_id: workspace.id.clone(),
                    preset: "cargo-test".into(),
                },
            ),
            action_step(
                "4",
                ActionSpec::WriteDraft {
                    template: "thread_brief".into(),
                },
            ),
            SkillStep {
                id: "5".into(),
                title: "Search the web".into(),
                category: "search".into(),
                action: None,
                enabled: true,
            },
        ])
    }

    fn workspace(directory: &std::path::Path) -> ApprovedWorkspace {
        let path = directory.join("knov");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("Cargo.toml"), "[package]\n").unwrap();
        ApprovedWorkspace {
            id: "ws-1".into(),
            label: "knov".into(),
            path: path.canonicalize().unwrap().to_string_lossy().into_owned(),
            check_presets: vec![CheckPresetView {
                id: "cargo-test".into(),
                label: "cargo test".into(),
            }],
            created_at: 0,
        }
    }

    fn plan(
        db: &Database,
        skill: &Skill,
        origin: &str,
        grants: &[AutonomyGrant],
        workspaces: &[ApprovedWorkspace],
        paused: bool,
    ) -> String {
        let recent = vec![
            SemanticStep {
                thread: Some("Other project".into()),
                ..step("web:github.com", "code-hosting", 50, 60)
            },
            step("web:github.com", "code-hosting", 100, 60),
        ];
        let state = state();
        let (run, actions) = plan_run(
            skill,
            origin,
            &PlanContext {
                state: &state,
                recent: &recent,
                grants,
                workspaces,
                paused,
                now: 1_800_000_000,
            },
        )
        .unwrap();
        db.insert_run(&run, &actions).unwrap();
        run.id
    }

    fn approve_all(db: &Database, run_id: &str, remember: bool) {
        let run = db.run(run_id).unwrap().unwrap();
        let decisions = run
            .actions
            .iter()
            .map(|action| ActionDecision {
                action_id: action.id.clone(),
                approved: true,
                remember,
            })
            .collect::<Vec<_>>();
        decide_run(db, run_id, &decisions, 1_800_000_010).unwrap();
    }

    fn drain(db: &Database, host: &FakeHost) {
        while execute_next(db, host, &|| 1_800_000_020).unwrap() {}
    }

    #[test]
    fn manual_runs_wait_for_approval_then_execute_in_order_with_verification() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::in_memory().unwrap();
        let workspace = workspace(directory.path());
        db.insert_workspace(&workspace.id, &workspace.label, &workspace.path, 0)
            .unwrap();
        let host = FakeHost::new(directory.path().join("drafts"));
        let run_id = plan(
            &db,
            &standard_skill(&workspace),
            "manual",
            &[],
            std::slice::from_ref(&workspace),
            false,
        );

        let run = db.run(&run_id).unwrap().unwrap();
        assert_eq!(run.status, "awaiting_approval");
        assert_eq!(run.manual_steps, vec!["Search the web"]);
        assert_eq!(run.actions.len(), 4);
        // The latest same-thread URL wins over another project's page.
        assert_eq!(run.actions[0].target_label, "github.com/issues/100");
        assert!(run.actions[3]
            .preview
            .as_deref()
            .unwrap()
            .contains("Resume brief"));
        // Nothing runs before approval.
        assert!(!execute_next(&db, &host, &|| 1_800_000_005).unwrap());
        assert!(host.calls().is_empty());

        approve_all(&db, &run_id, false);
        drain(&db, &host);
        let run = db.run(&run_id).unwrap().unwrap();
        assert_eq!(run.status, "completed");
        assert!(run
            .actions
            .iter()
            .all(|action| action.status == "succeeded"
                && action.verification.as_ref().unwrap().passed));
        // A clean manual run the user just watched does not resurface on Now.
        assert!(db.attention_runs(0).unwrap().is_empty());
        let calls = host.calls();
        assert_eq!(calls[0], "open_url https://github.com/issues/100");
        assert_eq!(calls[1], "open_application Visual Studio Code");
        assert!(calls[2].starts_with("run cargo test in "));
        assert!(run.actions[3].rollback_available && run.actions[3].can_open);
    }

    #[test]
    fn declining_and_stop_on_exception_are_journaled() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::in_memory().unwrap();
        let workspace = workspace(directory.path());
        db.insert_workspace(&workspace.id, &workspace.label, &workspace.path, 0)
            .unwrap();
        let host = FakeHost {
            exit_code: 1,
            ..FakeHost::new(directory.path().join("drafts"))
        };
        let run_id = plan(
            &db,
            &standard_skill(&workspace),
            "manual",
            &[],
            std::slice::from_ref(&workspace),
            false,
        );
        let run = db.run(&run_id).unwrap().unwrap();
        let decisions = run
            .actions
            .iter()
            .enumerate()
            .map(|(index, action)| ActionDecision {
                action_id: action.id.clone(),
                approved: index != 0,
                remember: false,
            })
            .collect::<Vec<_>>();
        decide_run(&db, &run_id, &decisions, 1_800_000_010).unwrap();
        drain(&db, &host);
        let run = db.run(&run_id).unwrap().unwrap();
        let statuses = run
            .actions
            .iter()
            .map(|action| action.status.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            statuses,
            ["rejected", "succeeded", "needs_attention", "skipped"]
        );
        assert_eq!(run.status, "completed_with_exceptions");
        assert!(run.summary.contains("1 declined"));
        assert_eq!(
            db.agent_metrics(1_800_000_100)
                .unwrap()
                .approval_acceptance_rate,
            Some(0.75)
        );
    }

    #[test]
    fn remembered_approvals_become_grants_and_unattended_runs_respect_them() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::in_memory().unwrap();
        let workspace = workspace(directory.path());
        db.insert_workspace(&workspace.id, &workspace.label, &workspace.path, 0)
            .unwrap();
        let host = FakeHost::new(directory.path().join("drafts"));
        let skill = standard_skill(&workspace);
        let run_id = plan(
            &db,
            &skill,
            "manual",
            &[],
            std::slice::from_ref(&workspace),
            false,
        );
        approve_all(&db, &run_id, true);
        drain(&db, &host);
        let grants = db.active_grants(1_800_000_100).unwrap();
        assert_eq!(grants.len(), 4);
        assert!(grants
            .iter()
            .any(|grant| grant.action_type == "run_checks" && grant.scope_kind == "workspace"));

        // A scheduled run executes non-interrupting actions, but still asks
        // before opening windows.
        let scheduled = plan(
            &db,
            &skill,
            "schedule",
            &grants,
            std::slice::from_ref(&workspace),
            false,
        );
        let run = db.run(&scheduled).unwrap().unwrap();
        let statuses = run
            .actions
            .iter()
            .map(|action| (action.action_type.as_str(), action.status.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            statuses,
            [
                ("open_url", "awaiting_approval"),
                ("open_application", "awaiting_approval"),
                ("run_checks", "approved"),
                ("write_draft", "approved"),
            ]
        );
        drain(&db, &host);
        let run = db.run(&scheduled).unwrap().unwrap();
        assert_eq!(run.status, "awaiting_approval");
        assert_eq!(run.actions[2].decision, "auto");
        assert_eq!(run.actions[2].status, "succeeded");

        // The kill switch blocks planning and skips anything already approved.
        let paused = plan(&db, &skill, "schedule", &grants, &[workspace], true);
        let run = db.run(&paused).unwrap().unwrap();
        assert!(run.actions.iter().all(|action| action.status == "blocked"));
        assert_eq!(run.status, "blocked");
    }

    #[test]
    fn hourly_budget_fails_closed_for_automatic_actions() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::in_memory().unwrap();
        let mut settings = db.settings().unwrap();
        settings.agent_max_actions_per_hour = 1;
        db.save_settings(&settings).unwrap();
        let host = FakeHost::new(directory.path().join("drafts"));
        let skill = skill(vec![
            action_step(
                "1",
                ActionSpec::WriteDraft {
                    template: "thread_brief".into(),
                },
            ),
            action_step(
                "2",
                ActionSpec::WriteDraft {
                    template: "workflow_checklist".into(),
                },
            ),
        ]);
        let grants = vec![AutonomyGrant {
            id: "g".into(),
            action_type: "write_draft".into(),
            scope_kind: "global".into(),
            scope_value: None,
            scope_label: "everywhere".into(),
            mode: "auto".into(),
            source: "user".into(),
            created_at: 0,
            expires_at: None,
        }];
        let run_id = plan(&db, &skill, "schedule", &grants, &[], false);
        drain(&db, &host);
        let run = db.run(&run_id).unwrap().unwrap();
        let statuses = run
            .actions
            .iter()
            .map(|action| action.status.as_str())
            .collect::<Vec<_>>();
        assert_eq!(statuses, ["succeeded", "blocked"]);
    }

    #[test]
    fn rollback_undoes_drafts_and_is_journaled() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::in_memory().unwrap();
        let host = FakeHost::new(directory.path().join("drafts"));
        let skill = skill(vec![action_step(
            "1",
            ActionSpec::WriteDraft {
                template: "thread_brief".into(),
            },
        )]);
        let run_id = plan(&db, &skill, "manual", &[], &[], false);
        approve_all(&db, &run_id, false);
        drain(&db, &host);
        let action_id = db.run(&run_id).unwrap().unwrap().actions[0].id.clone();
        rollback_action(&db, &host, &action_id, 1_800_000_030).unwrap();
        let run = db.run(&run_id).unwrap().unwrap();
        assert_eq!(run.actions[0].status, "rolled_back");
        assert!(!run.actions[0].rollback_available);
        assert!(rollback_action(&db, &host, &action_id, 1_800_000_040).is_err());
        assert_eq!(
            db.agent_metrics(1_800_000_100).unwrap().rollback_rate,
            Some(1.0)
        );
    }

    #[test]
    fn interrupted_actions_fail_closed_on_restart() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::in_memory().unwrap();
        let skill = skill(vec![action_step(
            "1",
            ActionSpec::OpenApplication {
                app: "Terminal".into(),
            },
        )]);
        let run_id = plan(&db, &skill, "manual", &[], &[], false);
        approve_all(&db, &run_id, false);
        let action = db.next_approved_action().unwrap().unwrap();
        assert!(db.start_action(&action.id, 1).unwrap());
        assert_eq!(db.fail_interrupted_actions(2).unwrap(), 1);
        let run = db.run(&run_id).unwrap().unwrap();
        assert_eq!(run.actions[0].status, "failed");
        assert_eq!(run.status, "failed");
        drop(directory);
    }
}
