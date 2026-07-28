//! Typed Tauri IPC boundary.
//!
//! Command names and argument casing intentionally mirror `apps/desktop/src/lib/api.ts`.
//! Secrets are accepted only by `save_provider_key`; no command ever returns a key.

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

use chrono::{Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::State;
use tauri_plugin_autostart::ManagerExt;
use uuid::Uuid;

use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{ChatMessage, DashboardRequest, HistoryRequest, Settings, UserCorrection},
    platform::{
        collection_status, discover_chrome_profiles, ensure_pairing_token,
        import_selected_chrome_history, RuntimeStatus,
    },
    providers::ProviderClient,
};

pub struct AppState {
    pub db: Arc<Database>,
    pub providers: ProviderClient,
    pub runtime: Arc<RwLock<RuntimeStatus>>,
    pub refresh_lock: Arc<AtomicBool>,
}

struct RefreshPermit(Arc<AtomicBool>);

impl Drop for RefreshPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn acquire_refresh(lock: &Arc<AtomicBool>) -> AppResult<RefreshPermit> {
    lock.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map(|_| RefreshPermit(lock.clone()))
        .map_err(|_| AppError::InvalidInput("A profile refresh is already running.".into()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UiUsage {
    name: String,
    seconds: i64,
    percentage: f64,
    color: String,
}

#[tauri::command]
pub fn get_dashboard(range: String, state: State<'_, AppState>) -> AppResult<Value> {
    let (start_at, end_at) = range_bounds(&range)?;
    let dashboard = state.db.dashboard(&DashboardRequest { start_at, end_at })?;
    let history = state.db.history(&HistoryRequest {
        start_at,
        end_at,
        search: None,
        source: None,
        limit: Some(1000),
        offset: None,
    })?;
    let recent = history.iter().take(8).cloned().collect::<Vec<_>>();
    let palette = ["#4968a6", "#7c5c9e", "#399279", "#c07a3e", "#7e8798"];
    let behavioral_guidance_enabled = state.db.settings()?.behavioral_guidance_enabled;
    let usage = |values: Vec<crate::models::UsageItem>| {
        values
            .into_iter()
            .enumerate()
            .map(|(index, value)| UiUsage {
                name: value.key,
                seconds: value.seconds,
                percentage: value.percentage,
                color: palette[index % palette.len()].into(),
            })
            .collect::<Vec<_>>()
    };
    let recommendations = dashboard
        .recommendations
        .into_iter()
        .filter(|item| behavioral_guidance_enabled || item.kind != "behavioral")
        .map(|item| {
            json!({
                "id":item.id,
                "kind":item.kind,
                "title": if item.kind=="behavioral" {"Activity suggestion"} else {"Suggested next step"},
                "body":item.text,
                "evidence":item.evidence,
                "createdAt":timestamp(item.created_at)
            })
        })
        .collect::<Vec<_>>();
    let mut insights = Vec::new();
    if let Some(top) = dashboard.applications.first() {
        insights.push(json!({
            "id":"top-application",
            "title":"Most active application",
            "description":format!("{} accounted for {:.0}% of recorded foreground time.",top.key,top.percentage),
            "metric":format_duration(top.seconds),
            "evidence":"Observed foreground-session duration in the selected range."
        }));
    }
    if let Some(longest) = history.iter().max_by_key(|event| event.duration_seconds) {
        insights.push(json!({
            "id":"longest-focus",
            "title":"Longest focused session",
            "description":format!("A {} foreground session was the longest observed session.",longest.app_name),
            "metric":format_duration(longest.duration_seconds),
            "evidence":"Observed active-window duration; this does not imply productivity or comprehension."
        }));
    }
    let distinct_pages = history
        .iter()
        .filter_map(|event| event.url.as_deref())
        .collect::<std::collections::HashSet<_>>()
        .len();
    if distinct_pages > 0 {
        insights.push(json!({
            "id":"active-pages",
            "title":"Active web resources",
            "description":"Distinct web pages were active in recorded browser sessions.",
            "metric":format!("{distinct_pages} pages"),
            "evidence":"Observed distinct URLs only; Knoveyla does not claim they were read, watched, or completed."
        }));
    }
    let mut topics = BTreeMap::<&str, usize>::new();
    for event in &history {
        if let Some(topic) = inferred_topic(event) {
            *topics.entry(topic).or_default() += 1;
        }
    }
    let mut topics = topics.into_iter().collect::<Vec<_>>();
    topics.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    for (topic, count) in topics.into_iter().take(3) {
        insights.push(json!({
            "id":format!("topic-{}",topic.replace(' ',"-")),
            "title":format!("Likely topic: {topic}"),
            "description":format!("{count} recorded sessions or pages matched local title/domain signals for this topic."),
            "metric":format!("{count} items"),
            "evidence":"Cautious local categorization from app names, page titles, and domains; it does not imply reading, watching, completion, or expertise."
        }));
    }
    Ok(json!({
        "range":range,
        "trackedSeconds":dashboard.total_seconds,
        "focusedSeconds":dashboard.total_seconds,
        "appUsage":usage(dashboard.applications),
        "siteUsage":usage(dashboard.websites),
        "recentActivity":recent.into_iter().map(activity_to_ui).collect::<Vec<_>>(),
        "insights":insights,
        "recommendations":recommendations,
        "generatedAt":Utc::now().to_rfc3339()
    }))
}

fn inferred_topic(event: &crate::models::ActivityEvent) -> Option<&'static str> {
    let text = format!(
        "{} {} {} {}",
        event.app_name,
        event.window_title.as_deref().unwrap_or_default(),
        event.page_title.as_deref().unwrap_or_default(),
        event.url.as_deref().unwrap_or_default()
    )
    .to_ascii_lowercase();
    [
        (
            "software development",
            [
                "github",
                "visual studio code",
                "xcode",
                "rust",
                "tauri",
                "typescript",
            ]
            .as_slice(),
        ),
        (
            "video research",
            ["youtube.com", "vimeo.com", "video"].as_slice(),
        ),
        (
            "planning and notes",
            ["notion", "obsidian", "notes", "document"].as_slice(),
        ),
        (
            "communication",
            ["slack", "discord", "mail", "messages"].as_slice(),
        ),
        (
            "web research",
            ["wikipedia", "docs.", "search", "research"].as_slice(),
        ),
    ]
    .into_iter()
    .find(|(_, signals)| signals.iter().any(|signal| text.contains(signal)))
    .map(|(topic, _)| topic)
}

#[tauri::command]
pub fn get_activity_history(
    range: String,
    query: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<Value>> {
    let (start_at, end_at) = range_bounds(&range)?;
    Ok(state
        .db
        .history(&HistoryRequest {
            start_at,
            end_at,
            search: query,
            source: None,
            limit: Some(1000),
            offset: None,
        })?
        .into_iter()
        .map(activity_to_ui)
        .collect())
}

#[tauri::command]
pub fn get_profile(state: State<'_, AppState>) -> AppResult<Value> {
    profile_to_ui(&state.db)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<Value> {
    settings_to_ui(&state)
}

#[tauri::command]
pub fn get_browser_profiles(state: State<'_, AppState>) -> AppResult<Vec<Value>> {
    Ok(discover_chrome_profiles(&state.db)?
        .into_iter()
        .map(|profile| {
            json!({
                "id":profile.id,
                "browser":"chrome",
                "name":profile.name,
                "path":profile.path,
                "selected":profile.selected,
                "support":"required"
            })
        })
        .collect())
}

#[tauri::command]
pub fn get_bootstrap_status(state: State<'_, AppState>) -> AppResult<Value> {
    let settings = state.db.settings()?;
    Ok(if settings.initial_profile_completed {
        json!({"phase":"complete","importedEvents":0,"progress":100,"message":"Initial profile is ready."})
    } else {
        state.db.get_setting::<Value>("bootstrap_status")?.unwrap_or_else(||
            json!({"phase":"not-started","importedEvents":0,"progress":0,"message":"Ready to import browser history."})
        )
    })
}

#[tauri::command]
pub fn set_collection_enabled(enabled: bool, state: State<'_, AppState>) -> AppResult<Value> {
    let mut settings = state.db.settings()?;
    settings.collection_enabled = enabled;
    state.db.save_settings(&settings)?;
    settings_to_ui(&state)
}

#[tauri::command]
pub fn request_accessibility_permission(state: State<'_, AppState>) -> bool {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
    collection_status(&state.db, &state.runtime)
        .map(|status| status.accessibility_available)
        .unwrap_or(false)
}

#[tauri::command]
pub fn set_browser_profiles(profile_ids: Vec<String>, state: State<'_, AppState>) -> AppResult<()> {
    let available = discover_chrome_profiles(&state.db)?;
    if profile_ids.is_empty() {
        return Err(AppError::InvalidInput(
            "Select at least one Chrome profile.".into(),
        ));
    }
    if profile_ids
        .iter()
        .any(|id| !available.iter().any(|profile| &profile.id == id))
    {
        return Err(AppError::InvalidInput(
            "A selected Chrome profile is unavailable.".into(),
        ));
    }
    let mut settings = state.db.settings()?;
    settings.selected_chrome_profiles = profile_ids;
    state.db.save_settings(&settings)
}

#[tauri::command]
pub async fn start_bootstrap(state: State<'_, AppState>) -> AppResult<Value> {
    let _permit = acquire_refresh(&state.refresh_lock)?;
    state.db.set_setting(
        "bootstrap_status",
        &json!({"phase":"importing","importedEvents":0,"progress":20,"message":"Importing selected Chrome history…"}),
    )?;
    let imported = import_selected_chrome_history(&state.db)?;
    state.db.set_setting(
        "bootstrap_status",
        &json!({"phase":"profiling","importedEvents":imported,"progress":70,"message":"Generating the first profile…"}),
    )?;
    let provider = selected_provider(&state.db)?;
    match state
        .providers
        .refresh_profile(&state.db, &provider, "bootstrap")
        .await
    {
        Ok(_) => {
            let value = json!({"phase":"complete","importedEvents":imported,"progress":100,"message":"Initial profile and recommendations are ready."});
            state.db.set_setting("bootstrap_status", &value)?;
            Ok(value)
        }
        Err(error) => {
            let value = json!({"phase":"error","importedEvents":imported,"progress":70,"message":error.to_string()});
            state.db.set_setting("bootstrap_status", &value)?;
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn refresh_profile(state: State<'_, AppState>) -> AppResult<Value> {
    let _permit = acquire_refresh(&state.refresh_lock)?;
    let provider = selected_provider(&state.db)?;
    state
        .providers
        .refresh_profile(&state.db, &provider, "manual")
        .await?;
    profile_to_ui(&state.db)
}

#[tauri::command]
pub fn save_profile_correction(
    id: Option<String>,
    label: String,
    description: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Value> {
    if label.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "Correction label is required.".into(),
        ));
    }
    let now = Utc::now().timestamp();
    let existing = id.as_deref().and_then(|candidate| {
        state
            .db
            .corrections()
            .ok()?
            .into_iter()
            .find(|item| item.id == candidate)
    });
    state.db.upsert_correction(&UserCorrection {
        id: id.unwrap_or_else(|| Uuid::new_v4().to_string()),
        subject: label.trim().into(),
        value: description.unwrap_or_default().trim().into(),
        created_at: existing.map(|item| item.created_at).unwrap_or(now),
        updated_at: now,
    })?;
    profile_to_ui(&state.db)
}

#[tauri::command]
pub fn remove_profile_correction(id: String, state: State<'_, AppState>) -> AppResult<Value> {
    state.db.remove_correction(&id)?;
    profile_to_ui(&state.db)
}

#[tauri::command]
pub fn dismiss_profile_inference(id: String, state: State<'_, AppState>) -> AppResult<Value> {
    let mut settings = state.db.settings()?;
    if !settings.suppressed_profile_items.contains(&id) {
        settings.suppressed_profile_items.push(id);
        state.db.save_settings(&settings)?;
    }
    profile_to_ui(&state.db)
}

#[tauri::command]
pub fn save_profile_summary(summary: String, state: State<'_, AppState>) -> AppResult<Value> {
    if summary.chars().count() > 600 {
        return Err(AppError::InvalidInput(
            "Profile summary must be 600 characters or fewer.".into(),
        ));
    }
    let mut profile = state.db.profile()?;
    profile.summary = summary.trim().into();
    state.db.update_latest_profile(&profile)?;
    profile_to_ui(&state.db)
}

#[tauri::command]
pub fn save_provider_key(
    provider: String,
    key: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.providers.save_key(&provider, &key)?;
    let mut settings = state.db.settings()?;
    settings.selected_provider = Some(provider);
    state.db.save_settings(&settings)
}

#[tauri::command]
pub fn remove_provider_key(provider: String, state: State<'_, AppState>) -> AppResult<()> {
    state.providers.delete_key(&provider)
}

#[tauri::command]
pub async fn test_provider(provider: String, state: State<'_, AppState>) -> AppResult<String> {
    state.providers.validate(&provider).await?;
    Ok("Connection successful.".into())
}

#[tauri::command]
pub fn save_settings(
    settings: Value,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Value> {
    let mut current = state.db.settings()?;
    if let Some(provider) = settings.get("provider").and_then(Value::as_str) {
        if !matches!(provider, "openai" | "anthropic") {
            return Err(AppError::InvalidInput("Unsupported provider.".into()));
        }
        current.selected_provider = Some(provider.into());
    }
    if let Some(enabled) = settings
        .get("behavioralGuidanceEnabled")
        .and_then(Value::as_bool)
    {
        current.behavioral_guidance_enabled = enabled;
    }
    if let Some(enabled) = settings.get("launchAtLogin").and_then(Value::as_bool) {
        current.launch_at_login = enabled;
        if enabled {
            app.autolaunch().enable().map_err(|error| {
                AppError::InvalidInput(format!("Could not enable launch at login: {error}"))
            })?;
        } else {
            app.autolaunch().disable().map_err(|error| {
                AppError::InvalidInput(format!("Could not disable launch at login: {error}"))
            })?;
        }
    }
    if let Some(values) = settings.get("excludedApps").and_then(Value::as_array) {
        current.excluded_apps = string_array(values);
    }
    if let Some(values) = settings.get("excludedDomains").and_then(Value::as_array) {
        current.excluded_domains = string_array(values)
            .into_iter()
            .filter_map(|value| normalize_domain(&value))
            .collect();
    }
    if let Some(values) = settings
        .get("selectedBrowserProfileIds")
        .and_then(Value::as_array)
    {
        current.selected_chrome_profiles = string_array(values);
    }
    state.db.save_settings(&current)?;
    settings_to_ui(&state)
}

#[tauri::command]
pub fn dismiss_recommendation(
    id: String,
    feedback: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.db.dismiss_recommendation(&id, feedback.as_deref())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiChatMessage {
    pub role: String,
    pub content: String,
}

#[tauri::command]
pub async fn chat(messages: Vec<UiChatMessage>, state: State<'_, AppState>) -> AppResult<Value> {
    let last = messages
        .last()
        .ok_or_else(|| AppError::InvalidInput("A chat message is required.".into()))?;
    let history = messages[..messages.len() - 1]
        .iter()
        .map(|message| ChatMessage {
            role: message.role.clone(),
            content: message.content.clone(),
        })
        .collect::<Vec<_>>();
    let provider = selected_provider(&state.db)?;
    let answer = state
        .providers
        .chat(
            &provider,
            &state.db.profile()?,
            &state.db.corrections()?,
            &history,
            &last.content,
        )
        .await?;
    Ok(json!({
        "id":Uuid::new_v4().to_string(),
        "role":"assistant",
        "content":answer,
        "createdAt":Utc::now().to_rfc3339()
    }))
}

#[tauri::command]
pub fn get_pairing_info(state: State<'_, AppState>) -> AppResult<Value> {
    Ok(json!({
        "nativeHost":"com.knoveyla.companion",
        "pairingToken":ensure_pairing_token(&state.db)?,
        "localhostEndpoint":"http://127.0.0.1:48321",
        "protocolVersion":1
    }))
}

#[tauri::command]
pub fn install_native_host(extension_id: String) -> AppResult<String> {
    if extension_id.len() != 32
        || !extension_id
            .chars()
            .all(|character| ('a'..='p').contains(&character))
    {
        return Err(AppError::InvalidInput(
            "The Chrome extension ID is invalid.".into(),
        ));
    }
    let app_executable = std::env::current_exe()?;
    let directory = app_executable
        .parent()
        .ok_or_else(|| AppError::InvalidInput("Application bundle path is unavailable.".into()))?;
    let candidates = [
        directory.join("knoveyla-native-host"),
        directory.join("../Resources/knoveyla-native-host"),
    ];
    let host = candidates
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| {
            AppError::InvalidInput(
                "The Native Messaging helper is missing from this build; use localhost transport for development."
                    .into(),
            )
        })?;
    let manifest_dir = dirs::home_dir()
        .ok_or_else(|| AppError::InvalidInput("Home directory is unavailable.".into()))?
        .join("Library/Application Support/Google/Chrome/NativeMessagingHosts");
    std::fs::create_dir_all(&manifest_dir)?;
    let manifest_path = manifest_dir.join("com.knoveyla.companion.json");
    let manifest = json!({
        "name":"com.knoveyla.companion",
        "description":"Knoveyla local activity bridge",
        "path":host.canonicalize()?.to_string_lossy(),
        "type":"stdio",
        "allowed_origins":[format!("chrome-extension://{extension_id}/")]
    });
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(manifest_path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn delete_all_data(state: State<'_, AppState>) -> AppResult<()> {
    // Credentials live outside SQLite and are explicitly included in single-action deletion.
    state.providers.delete_key("openai")?;
    state.providers.delete_key("anthropic")?;
    state.db.delete_all_local_data()?;
    ensure_pairing_token(&state.db)?;
    if let Some(home) = dirs::home_dir() {
        let manifest = home.join(
            "Library/Application Support/Google/Chrome/NativeMessagingHosts/com.knoveyla.companion.json",
        );
        if manifest.exists() {
            std::fs::remove_file(manifest)?;
        }
    }
    Ok(())
}

pub fn start_scheduler(state: Arc<AppState>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        let settings = match state.db.settings() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let today = Local::now().format("%Y-%m-%d").to_string();
        if !scheduled_refresh_due(&settings, &today) {
            continue;
        }
        let Some(provider) = settings.selected_provider else {
            continue;
        };
        if !state.providers.has_key(&provider) {
            continue;
        }
        if let Ok(_permit) = acquire_refresh(&state.refresh_lock) {
            let result = tauri::async_runtime::block_on(
                state
                    .providers
                    .refresh_profile(&state.db, &provider, "nightly"),
            );
            if let Err(error) = result {
                eprintln!("scheduled profile refresh did not complete: {error}");
            }
        }
    });
}

fn scheduled_refresh_due(settings: &Settings, today: &str) -> bool {
    settings.initial_profile_completed
        && settings.last_profile_refresh_day.as_deref() != Some(today)
}

fn settings_to_ui(state: &AppState) -> AppResult<Value> {
    let settings = state.db.settings()?;
    let status = collection_status(&state.db, &state.runtime)?;
    let provider = settings
        .selected_provider
        .clone()
        .unwrap_or_else(|| "openai".into());
    Ok(json!({
        "provider":provider,
        "hasProviderKey":state.providers.has_key(&provider),
        "behavioralGuidanceEnabled":settings.behavioral_guidance_enabled,
        "launchAtLogin":settings.launch_at_login,
        "selectedBrowserProfileIds":settings.selected_chrome_profiles,
        "excludedApps":settings.excluded_apps,
        "excludedDomains":settings.excluded_domains,
        "collectionStatus":{
            "enabled":status.enabled,
            "accessibilityGranted":status.accessibility_available,
            "browserConnected":status.extension_connected,
            "lastCapturedAt":status.extension_last_seen_at.map(timestamp),
            "dataPath":status.data_path,
            "degradedReasons":status.accessibility_message.into_iter().chain(
                (!status.extension_connected).then_some("Chrome extension is disconnected; history import remains available.".into())
            ).collect::<Vec<String>>()
        }
    }))
}

fn profile_to_ui(db: &Database) -> AppResult<Value> {
    let profile = db.profile()?;
    let suppressed = db.settings()?.suppressed_profile_items;
    let inferred = |section: &str, values: Vec<String>| {
        values
            .into_iter()
            .map(|value| (stable_profile_id(section, &value), value))
            .filter(|(id, _)| !suppressed.contains(id))
            .map(|(id, value)| {
                json!({"id":id,"label":value,"confidence":0.7,"provenance":"inferred"})
            })
            .collect::<Vec<_>>()
    };
    let corrections = db
        .corrections()?
        .into_iter()
        .map(|item| {
            json!({"id":item.id,"label":item.subject,"description":item.value,"provenance":"user"})
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "summary":profile.summary,
        "sections":[
            {"id":"interests","title":"Interests","items":inferred("interests",profile.interests)},
            {"id":"skills","title":"Skills","items":inferred("skills",profile.skills)},
            {"id":"projects","title":"Active projects","items":inferred("projects",profile.active_projects)},
            {"id":"patterns","title":"Patterns","items":inferred("patterns",profile.patterns)},
            {"id":"corrections","title":"Your authoritative corrections","items":corrections}
        ],
        "updatedAt":(profile.updated_at>0).then(|| timestamp(profile.updated_at))
    }))
}

fn stable_profile_id(section: &str, value: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(section.as_bytes());
    digest.update([0]);
    digest.update(value.as_bytes());
    format!("inferred-{:x}", digest.finalize())
}

fn activity_to_ui(event: crate::models::ActivityEvent) -> Value {
    json!({
        "id":event.id.unwrap_or_default().to_string(),
        "appName":event.app_name,
        "windowTitle":event.window_title,
        "url":event.url,
        "pageTitle":event.page_title,
        "browserProfile":event.browser_profile_id,
        "startedAt":timestamp(event.occurred_at),
        "durationSeconds":event.duration_seconds,
        "source":match event.source {
            crate::models::ActivitySource::AppFocus=>"collector",
            crate::models::ActivitySource::ChromeHistory=>"history",
            crate::models::ActivitySource::ChromeExtension=>"chrome",
        }
    })
}

fn selected_provider(db: &Database) -> AppResult<String> {
    db.settings()?
        .selected_provider
        .ok_or(AppError::ProviderNotConfigured)
}

fn range_bounds(range: &str) -> AppResult<(i64, i64)> {
    let now = Utc::now();
    let start = match range {
        "today" => Local::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|value| Local.from_local_datetime(&value).earliest())
            .map(|value| value.timestamp())
            .unwrap_or(now.timestamp() - 86_400),
        "7d" => now.timestamp() - 7 * 86_400,
        "30d" => now.timestamp() - 30 * 86_400,
        _ => return Err(AppError::InvalidInput("Unsupported date range.".into())),
    };
    Ok((start, now.timestamp()))
}

fn timestamp(value: i64) -> String {
    Utc.timestamp_opt(value, 0)
        .single()
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

fn string_array(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_domain(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return None;
    }
    let candidate = if value.contains("://") {
        value
    } else {
        format!("https://{value}")
    };
    url::Url::parse(&candidate)
        .ok()?
        .host_str()
        .map(|host| host.trim_matches('.').to_string())
}

fn format_duration(seconds: i64) -> String {
    if seconds >= 3600 {
        format!("{:.1} hours", seconds as f64 / 3600.0)
    } else {
        format!("{} minutes", (seconds / 60).max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_refresh_waits_for_bootstrap_and_runs_once_per_day() {
        let mut settings = Settings::default();
        assert!(!scheduled_refresh_due(&settings, "2026-07-27"));

        settings.initial_profile_completed = true;
        assert!(scheduled_refresh_due(&settings, "2026-07-27"));

        settings.last_profile_refresh_day = Some("2026-07-27".into());
        assert!(!scheduled_refresh_due(&settings, "2026-07-27"));
    }

    #[test]
    fn refresh_permit_releases_lock_after_failure_scope() {
        let lock = Arc::new(AtomicBool::new(false));
        let permit = acquire_refresh(&lock).expect("first refresh should acquire the lock");
        assert!(acquire_refresh(&lock).is_err());

        drop(permit);
        assert!(acquire_refresh(&lock).is_ok());
    }
}
