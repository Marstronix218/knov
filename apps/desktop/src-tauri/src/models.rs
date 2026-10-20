use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: Option<i64>,
    pub occurred_at: i64,
    pub ended_at: Option<i64>,
    pub duration_seconds: i64,
    pub app_name: String,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub page_title: Option<String>,
    pub search_query: Option<String>,
    pub browser_profile_id: Option<String>,
    pub source: ActivitySource,
    pub is_bootstrap: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivitySource {
    AppFocus,
    ChromeHistory,
    ChromeExtension,
    EditorHistory,
}

impl ActivitySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppFocus => "app_focus",
            Self::ChromeHistory => "chrome_history",
            Self::ChromeExtension => "chrome_extension",
            Self::EditorHistory => "editor_history",
        }
    }
}

impl TryFrom<&str> for ActivitySource {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "app_focus" => Ok(Self::AppFocus),
            "chrome_history" => Ok(Self::ChromeHistory),
            "chrome_extension" => Ok(Self::ChromeExtension),
            "editor_history" => Ok(Self::EditorHistory),
            _ => Err(format!("unknown activity source: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeProfile {
    pub id: String,
    /// Chromium-family browser key, e.g. `chrome`, `arc`, `brave`.
    pub browser: String,
    pub name: String,
    pub path: String,
    pub selected: bool,
    pub support_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDocument {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub interests: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub active_projects: Vec<String>,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCorrection {
    pub id: String,
    pub subject: String,
    pub value: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub title: String,
    pub text: String,
    pub evidence: String,
    pub dismissed: bool,
    pub feedback: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardRequest {
    pub start_at: i64,
    pub end_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageItem {
    pub key: String,
    pub seconds: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub total_seconds: i64,
    pub focused_seconds: i64,
    pub applications: Vec<UsageItem>,
    pub websites: Vec<UsageItem>,
    pub recommendations: Vec<Recommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRequest {
    pub start_at: i64,
    pub end_at: i64,
    pub search: Option<String>,
    pub source: Option<ActivitySource>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct Settings {
    pub collection_enabled: bool,
    pub sampling_interval_seconds: u64,
    pub selected_provider: Option<String>,
    pub excluded_apps: Vec<String>,
    pub excluded_domains: Vec<String>,
    pub selected_chrome_profiles: Vec<String>,
    pub behavioral_guidance_enabled: bool,
    pub launch_at_login: bool,
    pub suppressed_profile_items: Vec<String>,
    pub last_profile_refresh_day: Option<String>,
    pub initial_profile_completed: bool,
    pub prediction_experiment_enabled: bool,
    pub prediction_display_threshold: f64,
    pub prediction_cooldown_minutes: i64,
    /// Kill switch for agent execution; independent of activity collection.
    pub agent_paused: bool,
    /// Budget for actions that run under an automatic grant.
    pub agent_max_actions_per_hour: i64,
    /// Address of a local Ollama or OpenAI-compatible server on this Mac.
    pub local_base_url: String,
    /// Local model name; the first installed model is used when unset.
    pub local_model: Option<String>,
    /// Shows experimental surfaces: work agent, workflows, interviews, predictions.
    pub labs_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            collection_enabled: false,
            sampling_interval_seconds: 5,
            selected_provider: None,
            excluded_apps: vec![],
            excluded_domains: vec![],
            selected_chrome_profiles: vec![],
            behavioral_guidance_enabled: true,
            launch_at_login: false,
            suppressed_profile_items: vec![],
            last_profile_refresh_day: None,
            initial_profile_completed: false,
            prediction_experiment_enabled: false,
            prediction_display_threshold: 0.65,
            prediction_cooldown_minutes: 15,
            agent_paused: false,
            agent_max_actions_per_hour: 30,
            local_base_url: crate::providers::DEFAULT_LOCAL_BASE_URL.into(),
            local_model: None,
            labs_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionStatus {
    pub enabled: bool,
    pub accessibility_available: bool,
    pub accessibility_message: Option<String>,
    pub extension_connected: bool,
    pub extension_last_seen_at: Option<i64>,
    pub data_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadContextEvent {
    pub observed_at: String,
    pub app_name: String,
    pub source: String,
    pub title: Option<String>,
    pub resource: Option<String>,
    pub search_query: Option<String>,
    pub observed_active_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadContext {
    pub version: u8,
    pub subject: String,
    pub signal_count: usize,
    pub apps: Vec<String>,
    #[serde(default)]
    pub modified_files: Vec<String>,
    pub observed_from: Option<String>,
    pub observed_through: Option<String>,
    pub events: Vec<ThreadContextEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueryActivityFacts {
    pub subject: String,
    pub match_basis: String,
    pub matched_events: i64,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub observed_span_seconds: i64,
    pub observed_active_seconds: i64,
    pub app_focus_seconds: i64,
    pub live_browser_seconds: i64,
    pub historical_visits: i64,
    pub historical_reported_seconds: i64,
    pub editor_changes: i64,
    #[serde(default)]
    pub modified_files: Vec<String>,
    pub coverage_start_at: i64,
    pub coverage_end_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResult {
    pub profile: ProfileDocument,
    pub recommendations: Vec<Recommendation>,
    pub completed_at: i64,
}
