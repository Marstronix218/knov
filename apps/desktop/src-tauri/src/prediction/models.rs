use serde::{Deserialize, Serialize};

use super::DEFAULT_HORIZON_MINUTES;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PredictionResource {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub label: String,
    #[serde(default)]
    pub safe_locator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkPrediction {
    pub id: String,
    pub created_at: i64,
    pub source: String,
    pub intent: String,
    pub next_action: String,
    #[serde(default)]
    pub next_resource: Option<PredictionResource>,
    #[serde(default)]
    pub thread_id: Option<String>,
    pub confidence: f64,
    pub horizon_minutes: i64,
    pub reasoning_summary: String,
    pub evidence: Vec<String>,
    pub evaluation_status: String,
    pub expires_at: i64,
    #[serde(default)]
    pub match_score: Option<f64>,
    #[serde(default)]
    pub user_feedback: Option<String>,
    /// Inferred goal at prediction time: the top of the prediction hierarchy.
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub workflow_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationBin {
    pub label: String,
    pub min_confidence: f64,
    pub max_confidence: f64,
    pub count: i64,
    #[serde(default)]
    pub mean_confidence: Option<f64>,
    #[serde(default)]
    pub observed_accuracy: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PredictionStats {
    pub total_predictions: i64,
    pub evaluated_predictions: i64,
    pub matched: i64,
    pub partial: i64,
    pub missed: i64,
    pub provider_top1_accuracy: Option<f64>,
    pub baseline_top1_accuracy: Option<f64>,
    #[serde(default)]
    pub workflow_top1_accuracy: Option<f64>,
    pub high_confidence_accuracy: Option<f64>,
    pub user_positive_feedback_rate: Option<f64>,
    #[serde(default)]
    pub calibration: Vec<CalibrationBin>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PredictionDashboard {
    pub enabled: bool,
    pub predictions: Vec<WorkPrediction>,
    pub stats: PredictionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PredictionHistoryItem {
    #[serde(flatten)]
    pub prediction: WorkPrediction,
    #[serde(default)]
    pub observed_outcome: Option<String>,
    #[serde(default)]
    pub feedback_reason: Option<String>,
    #[serde(default)]
    pub evaluated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentWorkState {
    pub(super) generated_at: i64,
    pub(super) active_app: String,
    pub(super) active_window_title: Option<String>,
    pub(super) active_domain: Option<String>,
    pub(super) active_thread_id: Option<String>,
    pub(super) active_thread_title: Option<String>,
    pub(super) recent_events: Vec<StateEvent>,
    pub(super) recent_apps: Vec<String>,
    pub(super) recent_domains: Vec<String>,
    pub(super) session_duration_seconds: i64,
    pub(super) time_of_day: String,
    pub(super) day_of_week: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StateEvent {
    pub(super) occurred_at: i64,
    pub(super) app: String,
    pub(super) title: Option<String>,
    pub(super) domain: Option<String>,
    pub(super) thread: Option<String>,
    pub(super) duration_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistoricalExample {
    pub(super) observed_at: i64,
    pub(super) sequence: Vec<String>,
    pub(super) shared_signals: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProviderPrediction {
    pub(super) intent: String,
    pub(super) next_action: String,
    #[serde(default)]
    pub(super) next_resource: Option<PredictionResource>,
    #[serde(default)]
    pub(super) thread_id: Option<String>,
    pub(super) confidence: f64,
    #[serde(default = "default_horizon")]
    pub(super) horizon_minutes: i64,
    pub(super) reasoning_summary: String,
    #[serde(default)]
    pub(super) evidence: Vec<String>,
}

fn default_horizon() -> i64 {
    DEFAULT_HORIZON_MINUTES
}

#[derive(Debug, Deserialize)]
pub(super) struct ProviderPredictions {
    pub(super) predictions: Vec<ProviderPrediction>,
}
