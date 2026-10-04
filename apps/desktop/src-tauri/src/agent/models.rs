use serde::{Deserialize, Serialize};

use super::actions::ActionSpec;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStep {
    pub key: String,
    pub kind: String,
    pub category: String,
    pub label: String,
    pub title: String,
    #[serde(default)]
    pub resource: Option<String>,
    pub average_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowOccurrence {
    pub started_at: i64,
    pub ended_at: i64,
    #[serde(default)]
    pub thread: Option<String>,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStats {
    pub occurrences: i64,
    pub distinct_days: i64,
    pub per_week: f64,
    pub average_duration_seconds: i64,
    pub completion_rate: f64,
    #[serde(default)]
    pub typical_weekday: Option<String>,
    #[serde(default)]
    pub typical_hour: Option<u32>,
    #[serde(default)]
    pub thread: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpportunityScore {
    pub score: f64,
    pub frequency: f64,
    pub time_cost: f64,
    pub stability: f64,
    pub executability: f64,
    pub risk: f64,
    pub preparable_steps: i64,
    pub executable_steps: i64,
    pub estimated_minutes_saved_per_week: f64,
    pub surfaced: bool,
    pub rationale: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub id: String,
    pub title: String,
    pub generated_title: String,
    pub status: String,
    pub active: bool,
    pub steps: Vec<WorkflowStep>,
    pub stats: WorkflowStats,
    pub evidence: Vec<WorkflowOccurrence>,
    pub opportunity: OpportunityScore,
    #[serde(default)]
    pub skill_id: Option<String>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillTrigger {
    pub kind: String,
    #[serde(default)]
    pub weekday: Option<u32>,
    #[serde(default)]
    pub hour: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillStep {
    pub id: String,
    pub title: String,
    pub category: String,
    #[serde(default)]
    pub action: Option<ActionSpec>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkillStats {
    pub runs: i64,
    pub completed: i64,
    pub needs_attention: i64,
    pub failed: i64,
    pub rolled_back: i64,
    #[serde(default)]
    pub last_run_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub workflow_id: Option<String>,
    #[serde(default)]
    pub thread: Option<String>,
    pub trigger: SkillTrigger,
    pub on_exception: String,
    pub steps: Vec<SkillStep>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub last_triggered_at: Option<i64>,
    #[serde(default)]
    pub stats: SkillStats,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillStepUpdate {
    pub id: String,
    pub enabled: bool,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub check_preset: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillUpdate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub trigger: SkillTrigger,
    pub on_exception: String,
    pub enabled: bool,
    pub steps: Vec<SkillStepUpdate>,
    #[serde(default)]
    pub include_brief: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub passed: bool,
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunState {
    #[serde(default)]
    pub thread: Option<String>,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub workflow: Option<String>,
    #[serde(default)]
    pub recent_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentAction {
    pub id: String,
    pub run_id: String,
    pub step_id: String,
    pub position: i64,
    pub action_type: String,
    pub title: String,
    pub risk_class: String,
    pub target_label: String,
    pub rationale: String,
    pub scope_kind: String,
    #[serde(default)]
    pub scope_value: Option<String>,
    pub scope_label: String,
    pub decision: String,
    pub decision_reason: String,
    #[serde(default)]
    pub grant_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub result_summary: Option<String>,
    #[serde(default)]
    pub output_excerpt: Option<String>,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub verification: Option<Verification>,
    pub rollback_available: bool,
    pub can_open: bool,
    pub created_at: i64,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRun {
    pub id: String,
    #[serde(default)]
    pub skill_id: Option<String>,
    pub title: String,
    pub origin: String,
    pub status: String,
    pub summary: String,
    pub state: RunState,
    pub manual_steps: Vec<String>,
    pub on_exception: String,
    pub actions: Vec<AgentAction>,
    pub created_at: i64,
    #[serde(default)]
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionDecision {
    pub action_id: String,
    pub approved: bool,
    #[serde(default)]
    pub remember: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutonomyGrant {
    pub id: String,
    pub action_type: String,
    pub scope_kind: String,
    #[serde(default)]
    pub scope_value: Option<String>,
    pub scope_label: String,
    pub mode: String,
    pub source: String,
    pub created_at: i64,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantRequest {
    pub action_type: String,
    pub scope_kind: String,
    #[serde(default)]
    pub scope_value: Option<String>,
    pub mode: String,
    #[serde(default)]
    pub duration_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutonomyProposal {
    pub action_type: String,
    pub action_title: String,
    pub scope_kind: String,
    #[serde(default)]
    pub scope_value: Option<String>,
    pub scope_label: String,
    pub approvals: i64,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalResponse {
    pub action_type: String,
    pub scope_kind: String,
    #[serde(default)]
    pub scope_value: Option<String>,
    pub accept: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedWorkspace {
    pub id: String,
    pub label: String,
    pub path: String,
    pub check_presets: Vec<CheckPresetView>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CheckPresetView {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DetectedWorkspace {
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionKindView {
    pub action_type: String,
    pub title: String,
    pub description: String,
    pub risk_class: String,
    pub risk_label: String,
    pub default_policy: String,
    pub interrupts_user: bool,
    pub rollback: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolicyPreference {
    pub action_type: String,
    pub action_title: String,
    pub scope_label: String,
    pub approvals: i64,
    pub rejections: i64,
    pub automatic: i64,
    pub rolled_back: i64,
    pub tendency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentMetrics {
    pub runs: i64,
    pub finished_runs: i64,
    pub completed_runs: i64,
    #[serde(default)]
    pub task_completion_rate: Option<f64>,
    pub actions_executed: i64,
    pub verified_actions: i64,
    #[serde(default)]
    pub verification_rate: Option<f64>,
    #[serde(default)]
    pub approval_acceptance_rate: Option<f64>,
    #[serde(default)]
    pub rollback_rate: Option<f64>,
    pub estimated_minutes_saved: f64,
    pub active_auto_grants: i64,
    pub high_risk_actions: i64,
    pub actions_last_7_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutonomyOverview {
    pub paused: bool,
    pub max_actions_per_hour: i64,
    pub auto_actions_last_hour: i64,
    pub catalog: Vec<ActionKindView>,
    pub grants: Vec<AutonomyGrant>,
    pub proposals: Vec<AutonomyProposal>,
    pub workspaces: Vec<ApprovedWorkspace>,
    pub detected_workspaces: Vec<DetectedWorkspace>,
    pub policy: Vec<PolicyPreference>,
    pub metrics: AgentMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    pub id: String,
    pub title: String,
    pub topic: String,
    pub status: String,
    pub confidence: f64,
    pub evidence: Vec<String>,
    pub active_days: i64,
    pub observed_seconds: i64,
    pub last_active_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StateStep {
    pub label: String,
    pub title: String,
    pub category: String,
    pub at: i64,
    pub seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StateResource {
    pub label: String,
    pub kind: String,
    #[serde(default)]
    pub locator: Option<String>,
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowProgress {
    pub workflow_id: String,
    pub title: String,
    pub matched_steps: i64,
    pub total_steps: i64,
    pub next_step: WorkflowStep,
    pub confidence: f64,
    #[serde(default)]
    pub skill_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedItem {
    pub kind: String,
    pub title: String,
    pub detail: String,
    #[serde(default)]
    pub run_id: Option<String>,
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkState {
    pub generated_at: i64,
    #[serde(default)]
    pub active_thread: Option<String>,
    #[serde(default)]
    pub goal: Option<Goal>,
    pub goals: Vec<Goal>,
    pub recent_steps: Vec<StateStep>,
    pub open_resources: Vec<StateResource>,
    #[serde(default)]
    pub workflow_progress: Option<WorkflowProgress>,
    pub unresolved: Vec<UnresolvedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentOverview {
    pub paused: bool,
    pub state: WorkState,
    pub attention: Vec<AgentRun>,
    pub awaiting_count: i64,
    pub proposals: Vec<AutonomyProposal>,
    pub opportunity_count: i64,
    pub workflow_count: i64,
}
