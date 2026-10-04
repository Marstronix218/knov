use std::collections::{HashMap, HashSet};

use rusqlite::{params, OptionalExtension, Row};

use super::{
    actions::{check_preset, detect_presets, display_url, Execution, ResolvedAction, RollbackPlan},
    goals::GoalReview,
    models::{
        AgentAction, AgentMetrics, AgentRun, ApprovedWorkspace, AutonomyGrant, RunState, Skill,
        SkillStats, Verification, WorkflowOccurrence, WorkflowStats, WorkflowStep,
    },
    policy::ApprovalHistory,
    workflows::{MinedWorkflow, SkillOutcome},
};
use crate::{
    db::{map_activity, Database},
    error::{AppError, AppResult},
    models::ActivityEvent,
};

const AGENT_EVENT_LIMIT: i64 = 80_000;
const EXECUTED: &str = "('succeeded','needs_attention','failed','rolled_back')";

#[derive(Debug, Clone)]
pub(crate) struct StoredWorkflow {
    pub id: String,
    pub generated_title: String,
    pub user_title: Option<String>,
    pub status: String,
    pub active: bool,
    pub steps: Vec<WorkflowStep>,
    pub stats: WorkflowStats,
    pub evidence: Vec<WorkflowOccurrence>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct NewRun {
    pub id: String,
    pub skill_id: Option<String>,
    pub title: String,
    pub origin: String,
    pub status: String,
    pub on_exception: String,
    pub state: RunState,
    pub manual_steps: Vec<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct NewAction {
    pub id: String,
    pub step_id: String,
    pub position: i64,
    pub action_type: String,
    pub title: String,
    pub risk_class: String,
    pub resolved: Option<ResolvedAction>,
    pub target_label: String,
    pub rationale: String,
    pub scope_kind: String,
    pub scope_value: Option<String>,
    pub scope_label: String,
    pub decision: String,
    pub decision_reason: String,
    pub grant_id: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredAction {
    pub id: String,
    pub run_id: String,
    pub position: i64,
    pub action_type: String,
    pub decision: String,
    pub status: String,
    pub resolved: Option<ResolvedAction>,
    pub rollback: Option<RollbackPlan>,
    pub on_exception: String,
    pub scope_kind: String,
    pub scope_value: Option<String>,
    pub scope_label: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ActionHistoryRow {
    pub action_type: String,
    pub scope_kind: String,
    pub scope_value: Option<String>,
    pub scope_label: String,
    pub history: ApprovalHistory,
    pub automatic: i64,
}

const ACTION_COLUMNS: &str =
    "a.id,a.run_id,a.step_id,a.position,a.action_type,a.title,a.risk_class,
    a.spec_json,a.target_label,a.rationale,a.scope_kind,a.scope_value,a.scope_label,a.decision,
    a.decision_reason,a.grant_id,a.status,a.result_summary,a.output_excerpt,a.verification_json,
    a.rollback_json,a.created_at,a.started_at,a.finished_at";

const RUN_COLUMNS: &str =
    "id,skill_id,title,origin,status,on_exception,state_json,manual_steps_json,
    summary,created_at,finished_at";

impl Database {
    pub(crate) fn agent_events(&self, start_at: i64, end_at: i64) -> AppResult<Vec<ActivityEvent>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id,occurred_at,ended_at,duration_seconds,app_name,window_title,url,page_title,
                    search_query,browser_profile_id,source,is_bootstrap
             FROM activity_events WHERE occurred_at BETWEEN ?1 AND ?2
             ORDER BY occurred_at ASC LIMIT ?3",
        )?;
        let rows =
            statement.query_map(params![start_at, end_at, AGENT_EVENT_LIMIT], map_activity)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub(crate) fn store_mined_workflows(&self, mined: &[MinedWorkflow], now: i64) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        for workflow in mined {
            tx.execute(
                "INSERT INTO workflows (id,generated_title,status,active,steps_json,stats_json,evidence_json,
                   first_seen_at,last_seen_at,updated_at)
                 VALUES (?1,?2,'discovered',1,?3,?4,?5,?6,?7,?8)
                 ON CONFLICT(id) DO UPDATE SET generated_title=excluded.generated_title,active=1,
                   steps_json=excluded.steps_json,stats_json=excluded.stats_json,
                   evidence_json=excluded.evidence_json,
                   first_seen_at=MIN(workflows.first_seen_at,excluded.first_seen_at),
                   last_seen_at=MAX(workflows.last_seen_at,excluded.last_seen_at),
                   updated_at=excluded.updated_at",
                params![
                    workflow.id,
                    workflow.generated_title,
                    serde_json::to_string(&workflow.steps)?,
                    serde_json::to_string(&workflow.stats)?,
                    serde_json::to_string(&workflow.evidence)?,
                    workflow.first_seen_at,
                    workflow.last_seen_at,
                    now
                ],
            )?;
        }
        let current = mined
            .iter()
            .map(|workflow| workflow.id.as_str())
            .collect::<HashSet<_>>();
        let existing = {
            let mut statement = tx.prepare("SELECT id,status FROM workflows")?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for (id, status) in existing {
            if current.contains(id.as_str()) {
                continue;
            }
            // Unreviewed patterns that no longer recur disappear; reviewed ones
            // keep the user's decision but drop expired evidence.
            if status == "discovered" {
                tx.execute("DELETE FROM workflows WHERE id=?1", [&id])?;
            } else {
                tx.execute(
                    "UPDATE workflows SET active=0,evidence_json='[]',updated_at=?2 WHERE id=?1",
                    params![id, now],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn stored_workflows(&self) -> AppResult<Vec<StoredWorkflow>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id,generated_title,user_title,status,active,steps_json,stats_json,evidence_json,
                    first_seen_at,last_seen_at,updated_at FROM workflows",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(StoredWorkflow {
                id: row.get(0)?,
                generated_title: row.get(1)?,
                user_title: row.get(2)?,
                status: row.get(3)?,
                active: row.get(4)?,
                steps: json_column(row, 5)?,
                stats: json_column(row, 6)?,
                evidence: json_column(row, 7)?,
                first_seen_at: row.get(8)?,
                last_seen_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub(crate) fn set_workflow_review(
        &self,
        id: &str,
        status: &str,
        user_title: Option<&str>,
        now: i64,
    ) -> AppResult<bool> {
        let changed = self.conn().execute(
            "UPDATE workflows SET status=?2,
               user_title=CASE WHEN ?3 IS NULL THEN user_title ELSE NULLIF(TRIM(?3),'') END,
               updated_at=?4 WHERE id=?1",
            params![id, status, user_title, now],
        )?;
        Ok(changed == 1)
    }

    pub(crate) fn save_skill(&self, skill: &Skill) -> AppResult<()> {
        let mut definition = skill.clone();
        definition.stats = SkillStats::default();
        self.conn().execute(
            "INSERT INTO skills (id,workflow_id,definition_json,enabled,created_at,updated_at,last_triggered_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(id) DO UPDATE SET definition_json=excluded.definition_json,
               enabled=excluded.enabled,updated_at=excluded.updated_at",
            params![
                skill.id,
                skill.workflow_id,
                serde_json::to_string(&definition)?,
                skill.enabled,
                skill.created_at,
                skill.updated_at,
                skill.last_triggered_at
            ],
        )?;
        Ok(())
    }

    pub(crate) fn skills(&self) -> AppResult<Vec<Skill>> {
        let stats = self.skill_stats()?;
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT definition_json,enabled,last_triggered_at FROM skills ORDER BY created_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            let mut skill: Skill = json_column(row, 0)?;
            skill.enabled = row.get(1)?;
            skill.last_triggered_at = row.get(2)?;
            Ok(skill)
        })?;
        let mut skills = rows.collect::<Result<Vec<_>, _>>()?;
        for skill in &mut skills {
            skill.stats = stats.get(&skill.id).cloned().unwrap_or_default();
        }
        Ok(skills)
    }

    pub(crate) fn skill(&self, id: &str) -> AppResult<Option<Skill>> {
        Ok(self.skills()?.into_iter().find(|skill| skill.id == id))
    }

    pub(crate) fn delete_skill(&self, id: &str, now: i64) -> AppResult<bool> {
        let conn = self.conn();
        conn.execute(
            "UPDATE autonomy_grants SET revoked_at=?2
             WHERE scope_kind='skill' AND scope_value=?1 AND revoked_at IS NULL",
            params![id, now],
        )?;
        Ok(conn.execute("DELETE FROM skills WHERE id=?1", [id])? == 1)
    }

    pub(crate) fn set_skill_triggered(&self, id: &str, at: i64) -> AppResult<()> {
        self.conn().execute(
            "UPDATE skills SET last_triggered_at=?2 WHERE id=?1",
            params![id, at],
        )?;
        Ok(())
    }

    pub(crate) fn workflow_skill_ids(&self) -> AppResult<HashMap<String, String>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT workflow_id,id FROM skills WHERE workflow_id IS NOT NULL ORDER BY created_at",
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(AppError::from)
    }

    fn skill_stats(&self) -> AppResult<HashMap<String, SkillStats>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT r.skill_id,
                SUM(r.status IN ('completed','completed_with_exceptions','failed')),
                SUM(r.status='completed'),
                SUM(r.status='completed_with_exceptions'),
                SUM(r.status='failed'),
                SUM(EXISTS(SELECT 1 FROM agent_actions a WHERE a.run_id=r.id AND a.status='rolled_back')),
                MAX(r.created_at)
             FROM agent_runs r WHERE r.skill_id IS NOT NULL GROUP BY r.skill_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                SkillStats {
                    runs: row.get(1)?,
                    completed: row.get(2)?,
                    needs_attention: row.get(3)?,
                    failed: row.get(4)?,
                    rolled_back: row.get(5)?,
                    last_run_at: row.get(6)?,
                },
            ))
        })?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(AppError::from)
    }

    pub(crate) fn skill_outcomes(&self) -> AppResult<HashMap<String, SkillOutcome>> {
        Ok(self
            .skill_stats()?
            .into_iter()
            .map(|(id, stats)| {
                (
                    id,
                    SkillOutcome {
                        runs: stats.runs,
                        completed: stats.completed + stats.needs_attention,
                        rolled_back: stats.rolled_back,
                    },
                )
            })
            .collect())
    }

    pub(crate) fn insert_run(&self, run: &NewRun, actions: &[NewAction]) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO agent_runs (id,skill_id,title,origin,status,on_exception,state_json,
               manual_steps_json,summary,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'',?9)",
            params![
                run.id,
                run.skill_id,
                run.title,
                run.origin,
                run.status,
                run.on_exception,
                serde_json::to_string(&run.state)?,
                serde_json::to_string(&run.manual_steps)?,
                run.created_at
            ],
        )?;
        for action in actions {
            tx.execute(
                "INSERT INTO agent_actions (id,run_id,step_id,position,action_type,title,risk_class,spec_json,
                   target_label,rationale,scope_kind,scope_value,scope_label,decision,decision_reason,
                   grant_id,status,created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
                params![
                    action.id,
                    run.id,
                    action.step_id,
                    action.position,
                    action.action_type,
                    action.title,
                    action.risk_class,
                    action
                        .resolved
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()?,
                    action.target_label,
                    action.rationale,
                    action.scope_kind,
                    action.scope_value,
                    action.scope_label,
                    action.decision,
                    action.decision_reason,
                    action.grant_id,
                    action.status,
                    run.created_at
                ],
            )?;
        }
        tx.commit()?;
        drop(conn);
        self.refresh_run_status(&run.id, run.created_at)?;
        Ok(())
    }

    pub(crate) fn run(&self, id: &str) -> AppResult<Option<AgentRun>> {
        let runs = self.query_runs(
            &format!("SELECT {RUN_COLUMNS} FROM agent_runs WHERE id=?1"),
            params![id],
        )?;
        Ok(runs.into_iter().next())
    }

    pub(crate) fn runs(&self, limit: i64) -> AppResult<Vec<AgentRun>> {
        self.query_runs(
            &format!("SELECT {RUN_COLUMNS} FROM agent_runs ORDER BY created_at DESC LIMIT ?1"),
            params![limit.clamp(1, 200)],
        )
    }

    /// Runs waiting on the user, plus recent runs whose results need a look.
    pub(crate) fn attention_runs(&self, since: i64) -> AppResult<Vec<AgentRun>> {
        self.query_runs(
            &format!(
                "SELECT {RUN_COLUMNS} FROM agent_runs WHERE acknowledged_at IS NULL AND (
                   status='awaiting_approval'
                   OR (created_at>=?1 AND (status IN ('completed_with_exceptions','failed','blocked')
                       OR (status='completed' AND origin!='manual')))
                 ) ORDER BY (status='awaiting_approval') DESC, created_at DESC LIMIT 6"
            ),
            params![since],
        )
    }

    pub(crate) fn count_runs(&self, status: &str) -> AppResult<i64> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*) FROM agent_runs WHERE status=?1",
            [status],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn open_run_for_skill(&self, skill_id: &str) -> AppResult<bool> {
        Ok(self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE skill_id=?1
               AND status IN ('awaiting_approval','ready','running'))",
            [skill_id],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn acknowledge_run(&self, id: &str, now: i64) -> AppResult<bool> {
        Ok(self.conn().execute(
            "UPDATE agent_runs SET acknowledged_at=?2 WHERE id=?1",
            params![id, now],
        )? == 1)
    }

    fn query_runs(&self, sql: &str, parameters: impl rusqlite::Params) -> AppResult<Vec<AgentRun>> {
        let mut runs = {
            let conn = self.conn();
            let mut statement = conn.prepare(sql)?;
            let rows = statement.query_map(parameters, map_run)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        if runs.is_empty() {
            return Ok(runs);
        }
        let ids = runs.iter().map(|run| run.id.clone()).collect::<Vec<_>>();
        let placeholders = vec!["?"; ids.len()].join(",");
        let conn = self.conn();
        let mut statement = conn.prepare(&format!(
            "SELECT {ACTION_COLUMNS} FROM agent_actions a WHERE a.run_id IN ({placeholders})
             ORDER BY a.position"
        ))?;
        let rows = statement.query_map(rusqlite::params_from_iter(ids.iter()), map_action_view)?;
        let mut by_run = HashMap::<String, Vec<AgentAction>>::new();
        for action in rows {
            let action = action?;
            by_run
                .entry(action.run_id.clone())
                .or_default()
                .push(action);
        }
        for run in &mut runs {
            run.actions = by_run.remove(&run.id).unwrap_or_default();
        }
        Ok(runs)
    }

    pub(crate) fn stored_action(&self, id: &str) -> AppResult<Option<StoredAction>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT a.id,a.run_id,a.position,a.action_type,a.decision,a.status,a.spec_json,
                        a.rollback_json,r.on_exception,a.scope_kind,a.scope_value,a.scope_label
                 FROM agent_actions a JOIN agent_runs r ON r.id=a.run_id WHERE a.id=?1",
                [id],
                map_stored_action,
            )
            .optional()?)
    }

    pub(crate) fn next_approved_action(&self) -> AppResult<Option<StoredAction>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT a.id,a.run_id,a.position,a.action_type,a.decision,a.status,a.spec_json,
                        a.rollback_json,r.on_exception,a.scope_kind,a.scope_value,a.scope_label
                 FROM agent_actions a JOIN agent_runs r ON r.id=a.run_id
                 WHERE a.status='approved' ORDER BY r.created_at,a.position LIMIT 1",
                [],
                map_stored_action,
            )
            .optional()?)
    }

    pub(crate) fn has_approved_actions(&self) -> AppResult<bool> {
        Ok(self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_actions WHERE status='approved')",
            [],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn run_has_exception_before(&self, run_id: &str, position: i64) -> AppResult<bool> {
        Ok(self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_actions WHERE run_id=?1 AND position<?2
               AND status IN ('failed','needs_attention'))",
            params![run_id, position],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn awaiting_actions(&self, run_id: &str) -> AppResult<Vec<StoredAction>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT a.id,a.run_id,a.position,a.action_type,a.decision,a.status,a.spec_json,
                    a.rollback_json,r.on_exception,a.scope_kind,a.scope_value,a.scope_label
             FROM agent_actions a JOIN agent_runs r ON r.id=a.run_id
             WHERE a.run_id=?1 AND a.status='awaiting_approval' ORDER BY a.position",
        )?;
        let rows = statement.query_map([run_id], map_stored_action)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub(crate) fn set_action_decision(
        &self,
        id: &str,
        status: &str,
        decision: &str,
        reason: &str,
        grant_id: Option<&str>,
    ) -> AppResult<()> {
        self.conn().execute(
            "UPDATE agent_actions SET status=?2,decision=?3,decision_reason=?4,
               grant_id=COALESCE(?5,grant_id) WHERE id=?1",
            params![id, status, decision, reason, grant_id],
        )?;
        Ok(())
    }

    pub(crate) fn skip_action(&self, id: &str, reason: &str, now: i64) -> AppResult<()> {
        self.conn().execute(
            "UPDATE agent_actions SET status='skipped',result_summary=?2,finished_at=?3 WHERE id=?1",
            params![id, reason, now],
        )?;
        Ok(())
    }

    pub(crate) fn block_action(&self, id: &str, reason: &str, now: i64) -> AppResult<()> {
        self.conn().execute(
            "UPDATE agent_actions SET status='blocked',decision_reason=?2,result_summary=?2,finished_at=?3
             WHERE id=?1",
            params![id, reason, now],
        )?;
        Ok(())
    }

    /// Claims an approved action for execution; returns false if another
    /// executor already took it.
    pub(crate) fn start_action(&self, id: &str, now: i64) -> AppResult<bool> {
        Ok(self.conn().execute(
            "UPDATE agent_actions SET status='running',started_at=?2 WHERE id=?1 AND status='approved'",
            params![id, now],
        )? == 1)
    }

    pub(crate) fn finish_action(&self, id: &str, execution: &Execution, now: i64) -> AppResult<()> {
        self.conn().execute(
            "UPDATE agent_actions SET status=?2,result_summary=?3,output_excerpt=?4,verification_json=?5,
               rollback_json=?6,finished_at=?7 WHERE id=?1",
            params![
                id,
                execution.status,
                execution.summary,
                execution.output_excerpt,
                serde_json::to_string(&execution.verification)?,
                execution.rollback.as_ref().map(serde_json::to_string).transpose()?,
                now
            ],
        )?;
        Ok(())
    }

    pub(crate) fn mark_rolled_back(&self, id: &str, summary: &str) -> AppResult<()> {
        self.conn().execute(
            "UPDATE agent_actions SET status='rolled_back',result_summary=?2 WHERE id=?1",
            params![id, summary],
        )?;
        Ok(())
    }

    pub(crate) fn fail_interrupted_actions(&self, now: i64) -> AppResult<usize> {
        let run_ids = {
            let conn = self.conn();
            let mut statement =
                conn.prepare("SELECT DISTINCT run_id FROM agent_actions WHERE status='running'")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let changed = self.conn().execute(
            "UPDATE agent_actions SET status='failed',finished_at=?1,
               result_summary='Interrupted: Knov stopped before this action was verified.',
               verification_json='{\"passed\":false,\"checks\":[\"Interrupted before verification\"]}'
             WHERE status='running'",
            [now],
        )?;
        for run_id in run_ids {
            self.refresh_run_status(&run_id, now)?;
        }
        Ok(changed)
    }

    pub(crate) fn auto_actions_since(&self, since: i64) -> AppResult<i64> {
        Ok(self.conn().query_row(
            &format!(
                "SELECT COUNT(*) FROM agent_actions WHERE decision='auto' AND status IN {EXECUTED}
                   AND started_at>=?1"
            ),
            [since],
            |row| row.get(0),
        )?)
    }

    /// Derives a run's status and summary from its actions.
    pub(crate) fn refresh_run_status(&self, run_id: &str, now: i64) -> AppResult<String> {
        let statuses = {
            let conn = self.conn();
            let mut statement = conn.prepare("SELECT status FROM agent_actions WHERE run_id=?1")?;
            let rows = statement.query_map([run_id], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let count = |status: &str| statuses.iter().filter(|value| *value == status).count();
        let status = if count("running") > 0 {
            "running"
        } else if count("approved") > 0 {
            "ready"
        } else if count("awaiting_approval") > 0 {
            "awaiting_approval"
        } else if count("failed") > 0 {
            "failed"
        } else if count("needs_attention") > 0 {
            "completed_with_exceptions"
        } else if count("succeeded") + count("rolled_back") > 0 {
            "completed"
        } else if count("blocked") > 0 {
            "blocked"
        } else {
            "cancelled"
        };
        let mut parts = Vec::new();
        for (label, value) in [
            ("done", count("succeeded")),
            ("need attention", count("needs_attention")),
            ("failed", count("failed")),
            ("waiting for you", count("awaiting_approval")),
            ("declined", count("rejected")),
            ("blocked", count("blocked")),
            ("skipped", count("skipped")),
            ("undone", count("rolled_back")),
        ] {
            if value > 0 {
                parts.push(format!("{value} {label}"));
            }
        }
        let summary = if parts.is_empty() {
            "No actions to run.".to_string()
        } else {
            parts.join(" · ")
        };
        let terminal = !matches!(status, "running" | "ready" | "awaiting_approval");
        self.conn().execute(
            "UPDATE agent_runs SET status=?2,summary=?3,
               finished_at=CASE WHEN ?4 THEN COALESCE(finished_at,?5) ELSE NULL END WHERE id=?1",
            params![run_id, status, summary, terminal, now],
        )?;
        Ok(status.into())
    }

    pub(crate) fn active_grants(&self, now: i64) -> AppResult<Vec<AutonomyGrant>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id,action_type,scope_kind,scope_value,scope_label,mode,source,created_at,expires_at
             FROM autonomy_grants WHERE revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?1)
             ORDER BY created_at DESC",
        )?;
        let rows = statement.query_map([now], |row| {
            Ok(AutonomyGrant {
                id: row.get(0)?,
                action_type: row.get(1)?,
                scope_kind: row.get(2)?,
                scope_value: row.get(3)?,
                scope_label: row.get(4)?,
                mode: row.get(5)?,
                source: row.get(6)?,
                created_at: row.get(7)?,
                expires_at: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    /// Replaces any active grant for the same action and scope.
    pub(crate) fn put_grant(&self, grant: &AutonomyGrant, now: i64) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE autonomy_grants SET revoked_at=?4 WHERE revoked_at IS NULL AND action_type=?1
               AND scope_kind=?2 AND COALESCE(scope_value,'')=COALESCE(?3,'')",
            params![grant.action_type, grant.scope_kind, grant.scope_value, now],
        )?;
        tx.execute(
            "INSERT INTO autonomy_grants (id,action_type,scope_kind,scope_value,scope_label,mode,source,
               created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                grant.id,
                grant.action_type,
                grant.scope_kind,
                grant.scope_value,
                grant.scope_label,
                grant.mode,
                grant.source,
                grant.created_at,
                grant.expires_at
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn revoke_grant(&self, id: &str, now: i64) -> AppResult<bool> {
        Ok(self.conn().execute(
            "UPDATE autonomy_grants SET revoked_at=?2 WHERE id=?1 AND revoked_at IS NULL",
            params![id, now],
        )? == 1)
    }

    pub(crate) fn action_histories(&self) -> AppResult<Vec<ActionHistoryRow>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT action_type,scope_kind,scope_value,MAX(scope_label),
                SUM(decision='approved' AND status IN ('succeeded','needs_attention')),
                SUM(decision='rejected'),
                SUM(status='failed' AND decision IN ('approved','auto')),
                SUM(status='rolled_back'),
                SUM(decision='auto' AND status IN ('succeeded','needs_attention'))
             FROM agent_actions GROUP BY action_type,scope_kind,scope_value
             ORDER BY COUNT(*) DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ActionHistoryRow {
                action_type: row.get(0)?,
                scope_kind: row.get(1)?,
                scope_value: row.get(2)?,
                scope_label: row.get(3)?,
                history: ApprovalHistory {
                    approvals: row.get(4)?,
                    rejections: row.get(5)?,
                    failures: row.get(6)?,
                    rolled_back: row.get(7)?,
                },
                automatic: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub(crate) fn proposal_dismissals(&self) -> AppResult<HashMap<(String, String, String), i64>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT action_type,scope_kind,scope_value,approvals_at_dismissal
             FROM autonomy_proposal_dismissals",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(((row.get(0)?, row.get(1)?, row.get(2)?), row.get(3)?))
        })?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(AppError::from)
    }

    pub(crate) fn dismiss_proposal(
        &self,
        action_type: &str,
        scope_kind: &str,
        scope_value: Option<&str>,
        approvals: i64,
        now: i64,
    ) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO autonomy_proposal_dismissals (action_type,scope_kind,scope_value,
               approvals_at_dismissal,dismissed_at) VALUES (?1,?2,COALESCE(?3,''),?4,?5)
             ON CONFLICT(action_type,scope_kind,scope_value) DO UPDATE SET
               approvals_at_dismissal=excluded.approvals_at_dismissal,dismissed_at=excluded.dismissed_at",
            params![action_type, scope_kind, scope_value, approvals, now],
        )?;
        Ok(())
    }

    pub(crate) fn approved_workspaces(&self) -> AppResult<Vec<ApprovedWorkspace>> {
        let conn = self.conn();
        let mut statement = conn
            .prepare("SELECT id,label,path,created_at FROM approved_workspaces ORDER BY label")?;
        let rows = statement.query_map([], |row| {
            let path: String = row.get(2)?;
            Ok(ApprovedWorkspace {
                id: row.get(0)?,
                label: row.get(1)?,
                check_presets: detect_presets(std::path::Path::new(&path)),
                path,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub(crate) fn insert_workspace(
        &self,
        id: &str,
        label: &str,
        path: &str,
        now: i64,
    ) -> AppResult<()> {
        let exists: bool = self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM approved_workspaces WHERE path=?1)",
            [path],
            |row| row.get(0),
        )?;
        if exists {
            return Err(AppError::InvalidInput(
                "That folder is already approved.".into(),
            ));
        }
        self.conn().execute(
            "INSERT INTO approved_workspaces (id,label,path,created_at) VALUES (?1,?2,?3,?4)",
            params![id, label, path, now],
        )?;
        Ok(())
    }

    pub(crate) fn remove_workspace(&self, id: &str, now: i64) -> AppResult<bool> {
        let conn = self.conn();
        conn.execute(
            "UPDATE autonomy_grants SET revoked_at=?2
             WHERE scope_kind='workspace' AND scope_value=?1 AND revoked_at IS NULL",
            params![id, now],
        )?;
        Ok(conn.execute("DELETE FROM approved_workspaces WHERE id=?1", [id])? == 1)
    }

    pub(crate) fn goal_reviews(&self) -> AppResult<HashMap<String, GoalReview>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT id,status,user_title FROM goal_reviews")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                GoalReview {
                    status: row.get(1)?,
                    user_title: row.get(2)?,
                },
            ))
        })?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(AppError::from)
    }

    pub(crate) fn set_goal_review(
        &self,
        id: &str,
        status: &str,
        user_title: Option<&str>,
        now: i64,
    ) -> AppResult<()> {
        if status == "inferred" {
            self.conn()
                .execute("DELETE FROM goal_reviews WHERE id=?1", [id])?;
            return Ok(());
        }
        self.conn().execute(
            "INSERT INTO goal_reviews (id,status,user_title,updated_at) VALUES (?1,?2,NULLIF(TRIM(?3),''),?4)
             ON CONFLICT(id) DO UPDATE SET status=excluded.status,
               user_title=COALESCE(excluded.user_title,goal_reviews.user_title),updated_at=excluded.updated_at",
            params![id, status, user_title, now],
        )?;
        Ok(())
    }

    pub(crate) fn insert_state_snapshot(&self, now: i64, snapshot: &str) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO state_snapshots (created_at,snapshot_json) VALUES (?1,?2)",
            params![now, snapshot],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn state_snapshot_count(&self) -> AppResult<i64> {
        Ok(self
            .conn()
            .query_row("SELECT COUNT(*) FROM state_snapshots", [], |row| row.get(0))?)
    }

    pub(crate) fn agent_metrics(&self, now: i64) -> AppResult<AgentMetrics> {
        let conn = self.conn();
        let scalar = |sql: &str| -> AppResult<i64> {
            Ok(conn
                .query_row(sql, [], |row| row.get::<_, Option<i64>>(0))?
                .unwrap_or(0))
        };
        let runs = scalar("SELECT COUNT(*) FROM agent_runs")?;
        let finished_runs = scalar(
            "SELECT COUNT(*) FROM agent_runs WHERE status IN ('completed','completed_with_exceptions','failed','blocked')",
        )?;
        let completed_runs = scalar(
            "SELECT COUNT(*) FROM agent_runs WHERE status IN ('completed','completed_with_exceptions')",
        )?;
        let actions_executed = scalar(&format!(
            "SELECT COUNT(*) FROM agent_actions WHERE status IN {EXECUTED}"
        ))?;
        let approved = scalar("SELECT COUNT(*) FROM agent_actions WHERE decision='approved'")?;
        let rejected = scalar("SELECT COUNT(*) FROM agent_actions WHERE decision='rejected'")?;
        let rolled_back = scalar("SELECT COUNT(*) FROM agent_actions WHERE status='rolled_back'")?;
        let high_risk = scalar(&format!(
            "SELECT COUNT(*) FROM agent_actions WHERE status IN {EXECUTED}
               AND risk_class IN ('external_communication','destructive')"
        ))?;
        let active_auto_grants = conn.query_row(
            "SELECT COUNT(*) FROM autonomy_grants WHERE revoked_at IS NULL AND mode='auto'
               AND (expires_at IS NULL OR expires_at>?1)",
            [now],
            |row| row.get(0),
        )?;
        let actions_last_7_days = conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM agent_actions WHERE status IN {EXECUTED} AND started_at>=?1"
            ),
            [now - 7 * 86_400],
            |row| row.get(0),
        )?;
        let minutes = {
            let mut statement = conn.prepare(
                "SELECT action_type,COUNT(*) FROM agent_actions
                 WHERE status IN ('succeeded','needs_attention') GROUP BY action_type",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|(action_type, count)| estimated_minutes(&action_type) * count as f64)
                .sum::<f64>()
        };
        let verified_actions = {
            let mut statement = conn.prepare(
                "SELECT verification_json FROM agent_actions WHERE verification_json IS NOT NULL",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|value| {
                    serde_json::from_str::<Verification>(value)
                        .is_ok_and(|verification| verification.passed)
                })
                .count() as i64
        };
        let ratio = |numerator: i64, denominator: i64| {
            (denominator > 0)
                .then(|| ((numerator as f64 / denominator as f64) * 1000.0).round() / 1000.0)
        };
        Ok(AgentMetrics {
            runs,
            finished_runs,
            completed_runs,
            task_completion_rate: ratio(completed_runs, finished_runs),
            actions_executed,
            verified_actions,
            verification_rate: ratio(verified_actions, actions_executed),
            approval_acceptance_rate: ratio(approved, approved + rejected),
            rollback_rate: ratio(rolled_back, actions_executed),
            estimated_minutes_saved: (minutes * 10.0).round() / 10.0,
            active_auto_grants,
            high_risk_actions: high_risk,
            actions_last_7_days,
        })
    }
}

/// Conservative per-action estimate of manual effort avoided, in minutes.
pub(crate) fn estimated_minutes(action_type: &str) -> f64 {
    match action_type {
        "open_url" | "open_application" => 0.25,
        "write_draft" => 2.0,
        "run_checks" => 1.0,
        _ => 0.0,
    }
}

fn json_column<T: serde::de::DeserializeOwned>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    let raw: String = row.get(index)?;
    serde_json::from_str(&raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn map_run(row: &Row<'_>) -> rusqlite::Result<AgentRun> {
    Ok(AgentRun {
        id: row.get(0)?,
        skill_id: row.get(1)?,
        title: row.get(2)?,
        origin: row.get(3)?,
        status: row.get(4)?,
        on_exception: row.get(5)?,
        state: json_column(row, 6)?,
        manual_steps: json_column(row, 7)?,
        summary: row.get(8)?,
        created_at: row.get(9)?,
        finished_at: row.get(10)?,
        actions: vec![],
    })
}

fn map_stored_action(row: &Row<'_>) -> rusqlite::Result<StoredAction> {
    let spec: Option<String> = row.get(6)?;
    let rollback: Option<String> = row.get(7)?;
    Ok(StoredAction {
        id: row.get(0)?,
        run_id: row.get(1)?,
        position: row.get(2)?,
        action_type: row.get(3)?,
        decision: row.get(4)?,
        status: row.get(5)?,
        resolved: spec.and_then(|value| serde_json::from_str(&value).ok()),
        rollback: rollback.and_then(|value| serde_json::from_str(&value).ok()),
        on_exception: row.get(8)?,
        scope_kind: row.get(9)?,
        scope_value: row.get(10)?,
        scope_label: row.get(11)?,
    })
}

fn map_action_view(row: &Row<'_>) -> rusqlite::Result<AgentAction> {
    let action_type: String = row.get(4)?;
    let status: String = row.get(16)?;
    let spec: Option<String> = row.get(7)?;
    let resolved = spec.and_then(|value| serde_json::from_str::<ResolvedAction>(&value).ok());
    let verification: Option<String> = row.get(19)?;
    let rollback: Option<String> = row.get(20)?;
    Ok(AgentAction {
        id: row.get(0)?,
        run_id: row.get(1)?,
        step_id: row.get(2)?,
        position: row.get(3)?,
        title: row.get(5)?,
        risk_class: row.get(6)?,
        target_label: row.get(8)?,
        rationale: row.get(9)?,
        scope_kind: row.get(10)?,
        scope_value: row.get(11)?,
        scope_label: row.get(12)?,
        decision: row.get(13)?,
        decision_reason: row.get(14)?,
        grant_id: row.get(15)?,
        result_summary: row.get(17)?,
        output_excerpt: row.get(18)?,
        preview: resolved.as_ref().map(action_preview),
        verification: verification.and_then(|value| serde_json::from_str(&value).ok()),
        rollback_available: rollback.is_some() && status == "succeeded",
        can_open: action_type == "write_draft" && status == "succeeded",
        created_at: row.get(21)?,
        started_at: row.get(22)?,
        finished_at: row.get(23)?,
        action_type,
        status,
    })
}

fn action_preview(action: &ResolvedAction) -> String {
    match action {
        ResolvedAction::OpenUrl { url } => url.clone(),
        ResolvedAction::OpenApplication { app } => format!("Open {app}"),
        ResolvedAction::WriteDraft { content, .. } => content.clone(),
        ResolvedAction::RunChecks { path, preset, .. } => {
            let home = dirs::home_dir().map(|home| home.to_string_lossy().into_owned());
            let display = match home {
                Some(home) if path.starts_with(&home) => path.replacen(&home, "~", 1),
                _ => path.clone(),
            };
            format!(
                "`{}` in {display}",
                check_preset(preset).map_or(preset.as_str(), |preset| preset.label)
            )
        }
    }
}

pub(crate) fn target_label(action: &ResolvedAction) -> String {
    match action {
        ResolvedAction::OpenUrl { url } => display_url(url),
        ResolvedAction::OpenApplication { app } => app.clone(),
        ResolvedAction::WriteDraft { title, .. } => title.clone(),
        ResolvedAction::RunChecks { path, preset, .. } => format!(
            "{} in {}",
            check_preset(preset).map_or(preset.as_str(), |preset| preset.label),
            std::path::Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
    }
}
