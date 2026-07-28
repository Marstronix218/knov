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
}

impl ActivitySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppFocus => "app_focus",
            Self::ChromeHistory => "chrome_history",
            Self::ChromeExtension => "chrome_extension",
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
            _ => Err(format!("unknown activity source: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeProfile {
    pub id: String,
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
pub struct RefreshResult {
    pub profile: ProfileDocument,
    pub recommendations: Vec<Recommendation>,
    pub completed_at: i64,
}
