//! Typed Tauri IPC boundary.
//!
//! Command names and argument casing intentionally mirror `apps/desktop/src/lib/api.ts`.
//! Secrets are accepted only by `save_provider_key`; no command ever returns a key.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock, RwLock,
    },
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::State;
use tauri_plugin_autostart::ManagerExt;
use uuid::Uuid;

use crate::{
    analytics::{estimated_cost_usd, estimated_tokens, provider_scaled_measurement, InferenceRun},
    context::{
        build_baseline_context, build_optimized_context_package, compose_measured_prompt,
        configured_context_token_budget, configured_request_token_budget,
        CHAT_OUTPUT_TOKEN_RESERVE,
    },
    db::Database,
    error::{AppError, AppResult},
    memory::safe_local_search,
    models::{
        ChatMessage, DashboardRequest, HistoryRequest, Settings, ThreadContext, UserCorrection,
    },
    platform::{
        collection_status, discover_chrome_profiles, ensure_pairing_token,
        import_selected_chrome_history, recent_editor_workspace_changes, RuntimeStatus,
    },
    providers::ProviderClient,
    threading::semantic_topics,
};

pub struct AppState {
    pub db: Arc<Database>,
    pub providers: ProviderClient,
    pub runtime: Arc<RwLock<RuntimeStatus>>,
    pub refresh_lock: Arc<AtomicBool>,
}

#[derive(Debug)]
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

fn wait_for_refresh(lock: &Arc<AtomicBool>, timeout: Duration) -> AppResult<RefreshPermit> {
    let deadline = Instant::now() + timeout;
    loop {
        match acquire_refresh(lock) {
            Ok(permit) => return Ok(permit),
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(_) => {
                return Err(AppError::InvalidInput(
                    "Another profile refresh is still running. Try again in a moment.".into(),
                ));
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UiUsage {
    name: String,
    seconds: i64,
    percentage: f64,
    color: String,
}

const RECENT_ACTIVITY_LIMIT: usize = 200;
const TOPIC_EVIDENCE_LIMIT: usize = 3;
const MAX_CHAT_QUESTION_CHARS: usize = 4_000;
const MAX_CHAT_HISTORY_MESSAGE_CHARS: usize = 4_000;
const MAX_CHAT_HISTORY_TOKENS: i64 = 2_000;

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
    let semantic_topics = semantic_topics(&history);
    let editor_workspace_changes = editor_workspace_changes_by_app(&history, start_at);
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
            "description":format!("{} accounted for {:.1}% of recorded foreground time.",top.key,top.percentage),
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
            "evidence":"Observed distinct URLs only; Knov does not claim they were read, watched, or completed."
        }));
    }
    let topics = ranked_topics(&history, &semantic_topics);
    let recent = select_dashboard_activity(&history, &semantic_topics, &topics);
    for (topic, count) in topics.iter().take(3) {
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
        "focusedSeconds":dashboard.focused_seconds,
        "activeTopics":topics.into_iter().map(|(name,count)| json!({
            "name":name,
            "count":count
        })).collect::<Vec<_>>(),
        "appUsage":usage(dashboard.applications),
        "siteUsage":usage(dashboard.websites),
        "recentActivity":recent.into_iter().map(|event| {
            let semantic_topic = event.id.and_then(|id| semantic_topics.get(&id));
            let modified_files = editor_workspace_changes
                .get(&event.app_name.to_ascii_lowercase())
                .map(Vec::as_slice)
                .unwrap_or_default();
            activity_to_ui_with_topic(event, semantic_topic.map(String::as_str), modified_files)
        }).collect::<Vec<_>>(),
        "insights":insights,
        "recommendations":recommendations,
        "generatedAt":Utc::now().to_rfc3339()
    }))
}

fn event_topic(
    event: &crate::models::ActivityEvent,
    semantic_topics: &HashMap<i64, String>,
) -> Option<String> {
    event
        .id
        .and_then(|id| semantic_topics.get(&id).cloned())
        .or_else(|| inferred_topic(event).map(str::to_string))
}

fn ranked_topics(
    history: &[crate::models::ActivityEvent],
    semantic_topics: &HashMap<i64, String>,
) -> Vec<(String, usize)> {
    let mut topics = BTreeMap::<String, usize>::new();
    for event in history {
        if let Some(topic) = event_topic(event, semantic_topics) {
            *topics.entry(topic).or_default() += 1;
        }
    }
    let mut topics = topics.into_iter().collect::<Vec<_>>();
    topics.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    topics.truncate(5);
    topics
}

fn select_dashboard_activity(
    history: &[crate::models::ActivityEvent],
    semantic_topics: &HashMap<i64, String>,
    active_topics: &[(String, usize)],
) -> Vec<crate::models::ActivityEvent> {
    let mut selected = history
        .iter()
        .take(RECENT_ACTIVITY_LIMIT)
        .cloned()
        .collect::<Vec<_>>();
    let mut selected_ids = selected
        .iter()
        .filter_map(|event| event.id)
        .collect::<HashSet<_>>();

    for (topic, _) in active_topics {
        let already_selected = selected
            .iter()
            .filter(|event| event_topic(event, semantic_topics).as_deref() == Some(topic.as_str()))
            .count();
        let needed = TOPIC_EVIDENCE_LIMIT.saturating_sub(already_selected);
        if needed == 0 {
            continue;
        }
        let additions = history
            .iter()
            .filter(|event| {
                event_topic(event, semantic_topics).as_deref() == Some(topic.as_str())
                    && event.id.is_none_or(|id| !selected_ids.contains(&id))
            })
            .take(needed)
            .cloned()
            .collect::<Vec<_>>();
        for event in additions {
            if let Some(id) = event.id {
                selected_ids.insert(id);
            }
            selected.push(event);
        }
    }

    selected.sort_by_key(|event| std::cmp::Reverse(event.occurred_at));
    selected
}

fn inferred_topic(event: &crate::models::ActivityEvent) -> Option<&'static str> {
    if matches!(
        event.app_name.as_str(),
        "Code" | "Visual Studio Code" | "Cursor" | "Cortex Code" | "Xcode"
    ) {
        return Some("Software development");
    }
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
            "Software development",
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
            "Video research",
            ["youtube.com", "vimeo.com", "video"].as_slice(),
        ),
        (
            "Planning and notes",
            ["notion", "obsidian", "notes", "document"].as_slice(),
        ),
        (
            "Communication",
            ["slack", "discord", "mail", "messages"].as_slice(),
        ),
        (
            "Web research",
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
    let history = state.db.history(&HistoryRequest {
        start_at,
        end_at,
        search: query,
        source: None,
        limit: Some(1000),
        offset: None,
    })?;
    let semantic_topics = semantic_topics(&history);
    let editor_workspace_changes = editor_workspace_changes_by_app(&history, start_at);
    Ok(history
        .into_iter()
        .map(|event| {
            let semantic_topic = event.id.and_then(|id| semantic_topics.get(&id));
            let modified_files = editor_workspace_changes
                .get(&event.app_name.to_ascii_lowercase())
                .map(Vec::as_slice)
                .unwrap_or_default();
            activity_to_ui_with_topic(event, semantic_topic.map(String::as_str), modified_files)
        })
        .collect())
}

static ACTIVITY_ICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
static ACTIVITY_PREVIEW_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPreview {
    kind: String,
    url: String,
    thumbnail_data_url: Option<String>,
    embed_url: Option<String>,
}

#[tauri::command]
pub async fn get_activity_preview(url: String) -> AppResult<ActivityPreview> {
    let preview = activity_preview(&url)?;
    let Some(video_id) = youtube_video_id(&preview.url) else {
        return Ok(preview);
    };

    tauri::async_runtime::spawn_blocking(move || {
        let cache = ACTIVITY_PREVIEW_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let cached = cache
            .lock()
            .expect("activity preview cache poisoned")
            .get(&video_id)
            .cloned();
        let thumbnail_data_url = cached.or_else(|| {
            let value = fetch_youtube_thumbnail(&video_id);
            if let Some(data_url) = value.as_ref() {
                cache
                    .lock()
                    .expect("activity preview cache poisoned")
                    .insert(video_id, data_url.clone());
            }
            value
        });
        Ok(ActivityPreview {
            thumbnail_data_url,
            ..preview
        })
    })
    .await
    .map_err(|error| {
        AppError::InvalidInput(format!("Could not resolve activity preview: {error}"))
    })?
}

fn activity_preview(value: &str) -> AppResult<ActivityPreview> {
    let mut parsed = url::Url::parse(value.trim())
        .map_err(|_| AppError::InvalidInput("Activity preview URL is invalid.".into()))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(AppError::InvalidInput(
            "Activity preview URL must be an HTTP or HTTPS URL without credentials.".into(),
        ));
    }
    parsed.set_fragment(None);

    let Some(video_id) = youtube_video_id(parsed.as_str()) else {
        return Ok(ActivityPreview {
            kind: "link".into(),
            url: parsed.into(),
            thumbnail_data_url: None,
            embed_url: None,
        });
    };

    Ok(ActivityPreview {
        kind: "youtube".into(),
        url: format!("https://www.youtube.com/watch?v={video_id}"),
        thumbnail_data_url: None,
        embed_url: Some(format!("https://www.youtube-nocookie.com/embed/{video_id}")),
    })
}

fn reopenable_web_url(value: &str) -> AppResult<url::Url> {
    let parsed = url::Url::parse(value.trim())
        .map_err(|_| AppError::InvalidInput("The resource URL is invalid.".into()))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(AppError::InvalidInput(
            "Only HTTP and HTTPS resources without credentials can be reopened.".into(),
        ));
    }
    Ok(parsed)
}

#[tauri::command]
pub fn open_resource(url: String) -> AppResult<()> {
    let url = reopenable_web_url(&url)?;
    let status = open_external_url(url.as_str())?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::InvalidInput(
            "The system could not open the resource.".into(),
        ))
    }
}

fn normalized_application_name(value: &str) -> AppResult<String> {
    let app_name = value.trim();
    if app_name.is_empty()
        || app_name.chars().count() > 128
        || value.chars().any(char::is_control)
        || app_name.contains(['/', '\\'])
        || app_name.starts_with('-')
        || matches!(app_name, "." | "..")
    {
        return Err(AppError::InvalidInput(
            "The application name is invalid.".into(),
        ));
    }
    Ok(match app_name {
        "Code" => "Visual Studio Code".into(),
        _ => app_name.into(),
    })
}

#[tauri::command]
pub fn open_application(app_name: String) -> AppResult<()> {
    let app_name = normalized_application_name(&app_name)?;
    let status = open_native_application(&app_name)?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::InvalidInput(
            "The system could not open the application.".into(),
        ))
    }
}

#[cfg(target_os = "macos")]
fn open_native_application(app_name: &str) -> AppResult<std::process::ExitStatus> {
    Ok(Command::new("/usr/bin/open")
        .args(["-a", app_name])
        .status()?)
}

#[cfg(not(target_os = "macos"))]
fn open_native_application(_app_name: &str) -> AppResult<std::process::ExitStatus> {
    Err(AppError::InvalidInput(
        "Opening native applications is currently supported only on macOS.".into(),
    ))
}

#[cfg(target_os = "macos")]
fn open_external_url(url: &str) -> std::io::Result<std::process::ExitStatus> {
    Command::new("/usr/bin/open").arg(url).status()
}

#[cfg(target_os = "windows")]
fn open_external_url(url: &str) -> std::io::Result<std::process::ExitStatus> {
    Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .status()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_external_url(url: &str) -> std::io::Result<std::process::ExitStatus> {
    Command::new("xdg-open").arg(url).status()
}

fn youtube_video_id(value: &str) -> Option<String> {
    let parsed = url::Url::parse(value).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed
        .host_str()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let candidate = if host == "youtu.be" || host == "www.youtu.be" {
        parsed.path_segments()?.next().map(str::to_string)
    } else if host == "youtube.com" || host.ends_with(".youtube.com") {
        let mut segments = parsed.path_segments()?;
        match segments.next() {
            Some("watch") => parsed
                .query_pairs()
                .find_map(|(key, value)| (key == "v").then(|| value.into_owned())),
            Some("shorts" | "embed" | "live") => segments.next().map(str::to_string),
            _ => None,
        }
    } else if host == "youtube-nocookie.com" || host.ends_with(".youtube-nocookie.com") {
        let mut segments = parsed.path_segments()?;
        (segments.next() == Some("embed"))
            .then(|| segments.next())
            .flatten()
            .map(str::to_string)
    } else {
        None
    }?;

    is_valid_youtube_video_id(&candidate).then_some(candidate)
}

fn is_valid_youtube_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn fetch_youtube_thumbnail(video_id: &str) -> Option<String> {
    if !is_valid_youtube_video_id(video_id) {
        return None;
    }
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;
    let url = url::Url::parse(&format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg")).ok()?;
    fetch_image(&client, url)
}

#[tauri::command]
pub async fn get_activity_icon(app_name: String, url: Option<String>) -> AppResult<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let key = url
            .as_deref()
            .and_then(|value| url::Url::parse(value).ok())
            .map(|value| value.origin().ascii_serialization())
            .unwrap_or_else(|| format!("app:{app_name}"));
        let cache = ACTIVITY_ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Some(icon) = cache
            .lock()
            .expect("activity icon cache poisoned")
            .get(&key)
        {
            return icon.clone();
        }

        let icon = url
            .as_deref()
            .and_then(fetch_website_icon)
            .or_else(|| native_application_icon(&app_name));
        cache
            .lock()
            .expect("activity icon cache poisoned")
            .insert(key, icon.clone());
        icon
    })
    .await
    .map_err(|error| AppError::InvalidInput(format!("Could not resolve activity icon: {error}")))
}

fn fetch_website_icon(page_url: &str) -> Option<String> {
    let page_url = url::Url::parse(page_url).ok()?;
    if !matches!(page_url.scheme(), "http" | "https") {
        return None;
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(4))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .ok()?;
    let mut favicon_url = page_url.clone();
    favicon_url.set_path("/favicon.ico");
    favicon_url.set_query(None);
    favicon_url.set_fragment(None);
    if let Some(icon) = fetch_image(&client, favicon_url) {
        return Some(icon);
    }

    let response = client
        .get(page_url.clone())
        .header(reqwest::header::USER_AGENT, "Knov/0.2 favicon")
        .send()
        .ok()?
        .error_for_status()
        .ok()?;
    let mut html = Vec::new();
    response.take(2_097_153).read_to_end(&mut html).ok()?;
    if html.len() > 2_097_152 {
        return None;
    }
    let html = String::from_utf8_lossy(&html);
    let href = declared_icon_href(&html)?;
    let declared_url = page_url.join(&href).ok()?;
    if !matches!(declared_url.scheme(), "http" | "https") {
        return None;
    }
    fetch_image(&client, declared_url)
}

fn fetch_image(client: &reqwest::blocking::Client, url: url::Url) -> Option<String> {
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "Knov/0.2 favicon")
        .send()
        .ok()?
        .error_for_status()
        .ok()?;
    let header_mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .filter(|value| value.starts_with("image/"))
        .map(str::to_owned);
    let mut bytes = Vec::new();
    response.take(1_048_577).read_to_end(&mut bytes).ok()?;
    if bytes.is_empty() || bytes.len() > 1_048_576 {
        return None;
    }
    let mime = header_mime.or_else(|| detect_image_mime(&bytes).map(str::to_owned))?;
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

fn declared_icon_href(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(start) = lower[offset..].find("<link") {
        let start = offset + start;
        let end = start + lower[start..].find('>')? + 1;
        let tag = &html[start..end];
        let rel = html_attribute(tag, "rel").unwrap_or_default();
        if rel
            .split_ascii_whitespace()
            .any(|value| value.eq_ignore_ascii_case("icon"))
        {
            if let Some(href) = html_attribute(tag, "href") {
                return Some(href);
            }
        }
        offset = end;
    }
    None
}

fn html_attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(found) = lower[offset..].find(name) {
        let start = offset + found;
        let before = lower.as_bytes().get(start.wrapping_sub(1)).copied();
        let after = lower.as_bytes().get(start + name.len()).copied();
        if before.is_some_and(|value| value.is_ascii_alphanumeric() || value == b'-')
            || after.is_some_and(|value| value.is_ascii_alphanumeric() || value == b'-')
        {
            offset = start + name.len();
            continue;
        }
        let remainder = &tag[start + name.len()..];
        let remainder = remainder.trim_start();
        let remainder = remainder.strip_prefix('=')?.trim_start();
        let quote = remainder.as_bytes().first().copied()?;
        if quote == b'\'' || quote == b'"' {
            let value = &remainder[1..];
            let end = value.find(quote as char)?;
            return Some(value[..end].to_owned());
        }
        let end = remainder
            .find(|value: char| value.is_ascii_whitespace() || value == '>')
            .unwrap_or(remainder.len());
        return Some(remainder[..end].to_owned());
    }
    None
}

fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"\x00\x00\x01\x00") {
        Some("image/x-icon")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") {
        Some("image/svg+xml")
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn native_application_icon(app_name: &str) -> Option<String> {
    let searchable_name = match app_name {
        "Code" => "Visual Studio Code",
        name => name,
    };
    let escaped_name = searchable_name.replace('\\', "\\\\").replace('\'', "\\'");
    let query = format!(
        "kMDItemContentType == 'com.apple.application-bundle' && (kMDItemDisplayName == '{escaped_name}'cd || kMDItemFSName == '{escaped_name}.app'cd)"
    );
    let output = Command::new("/usr/bin/mdfind").arg(query).output().ok()?;
    let app_path = String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .next()?
        .trim()
        .to_owned();
    let resources = Path::new(&app_path).join("Contents/Resources");
    let plist = Path::new(&app_path).join("Contents/Info.plist");
    let icon_name = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleIconFile", "raw", "-o", "-"])
        .arg(&plist)
        .output()
        .ok()
        .filter(|result| result.status.success())
        .and_then(|result| String::from_utf8(result.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "AppIcon".into());
    let icon_name = if Path::new(&icon_name).extension().is_some() {
        icon_name
    } else {
        format!("{icon_name}.icns")
    };
    let source = resources.join(icon_name);
    if !source.is_file() {
        return None;
    }

    let digest = format!("{:x}", Sha256::digest(app_path.as_bytes()));
    let cache_dir = std::env::temp_dir().join("knov-app-icons");
    fs::create_dir_all(&cache_dir).ok()?;
    let png = cache_dir.join(format!("{digest}.png"));
    if !png.is_file() && !convert_icon_to_png(&source, &png) {
        return None;
    }
    let bytes = fs::read(png).ok()?;
    Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

#[cfg(target_os = "macos")]
fn convert_icon_to_png(source: &Path, destination: &PathBuf) -> bool {
    Command::new("/usr/bin/sips")
        .args(["-s", "format", "png"])
        .arg(source)
        .arg("--out")
        .arg(destination)
        .output()
        .is_ok_and(|result| result.status.success())
}

#[cfg(not(target_os = "macos"))]
fn native_application_icon(_app_name: &str) -> Option<String> {
    None
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
    let imported = import_selected_chrome_history(&state.db, 90)?;
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
pub async fn reimport_chrome_history(state: State<'_, AppState>) -> AppResult<Value> {
    let refresh_lock = state.refresh_lock.clone();
    let _permit = tauri::async_runtime::spawn_blocking(move || {
        wait_for_refresh(&refresh_lock, Duration::from_secs(120))
    })
    .await
    .map_err(|error| {
        AppError::InvalidInput(format!(
            "Could not wait for the active profile refresh: {error}"
        ))
    })??;

    import_selected_chrome_history(&state.db, 30)?;
    let provider = selected_provider(&state.db)?;
    state
        .providers
        .refresh_profile(&state.db, &provider, "manual")
        .await?;
    profile_to_ui(&state.db)
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
pub async fn save_profile_correction(
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
    let correction = UserCorrection {
        id: id.unwrap_or_else(|| Uuid::new_v4().to_string()),
        subject: label.trim().into(),
        value: description.unwrap_or_default().trim().into(),
        created_at: existing.map(|item| item.created_at).unwrap_or(now),
        updated_at: now,
    };
    state.db.upsert_correction(&correction)?;
    let mut profile = profile_to_ui(&state.db)?;
    profile["memorySync"] = json!({
        "status":"local-only",
        "message":"Saved locally and available to Knov's on-device memory retrieval."
    });
    Ok(profile)
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
        if !matches!(provider, "openai" | "anthropic" | "bedrock") {
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
pub async fn chat(
    messages: Vec<UiChatMessage>,
    mode: Option<String>,
    thread_context: Option<ThreadContext>,
    state: State<'_, AppState>,
) -> AppResult<Value> {
    let last = messages
        .last()
        .ok_or_else(|| AppError::InvalidInput("A chat message is required.".into()))?;
    if last.role != "user" || last.content.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "The final chat message must be a non-empty user question.".into(),
        ));
    }
    if last.content.chars().count() > MAX_CHAT_QUESTION_CHARS {
        return Err(AppError::InvalidInput(format!(
            "The question may contain at most {MAX_CHAT_QUESTION_CHARS} characters."
        )));
    }
    let mode = mode.unwrap_or_else(|| "optimized".into());
    if mode != "optimized" {
        return Err(AppError::InvalidInput(
            "Knov answers with compact, query-complete context; Full Context is comparison-only."
                .into(),
        ));
    }
    let thread_context = validated_thread_context(thread_context)?;
    let request_budget = configured_request_token_budget();
    let question_tokens = estimated_tokens(&last.content);
    let history_token_budget = MAX_CHAT_HISTORY_TOKENS.min(
        request_budget
            .saturating_sub(CHAT_OUTPUT_TOKEN_RESERVE)
            .saturating_sub(question_tokens)
            .saturating_sub(900),
    );
    let history =
        bounded_chat_history(&messages[..messages.len() - 1], history_token_budget.max(0));
    let conversation_text = history
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n");
    let non_context_tokens = estimated_tokens(&compose_measured_prompt(
        "",
        &conversation_text,
        &last.content,
    ));
    let safe_input_limit = request_budget
        .saturating_sub(CHAT_OUTPUT_TOKEN_RESERVE)
        .saturating_mul(85)
        / 100;
    let available_context_tokens = safe_input_limit.saturating_sub(non_context_tokens);
    if available_context_tokens < 800 {
        return Err(AppError::InvalidInput(
            "The question and recent conversation leave no safe context budget; shorten the question or start a new chat."
                .into(),
        ));
    }
    let provider = selected_provider(&state.db)?;
    let now = Utc::now().timestamp();
    let (today_start, _) = range_bounds("today")?;
    let activity_summary = json!({
        "today":state.db.chat_activity_summary(today_start, now)?,
        "7d":state.db.chat_activity_summary(now - 7 * 86_400, now)?,
        "30d":state.db.chat_activity_summary(now - 30 * 86_400, now)?
    });
    let profile = state.db.profile()?;
    let corrections = state.db.corrections()?;
    let memory_query = memory_query_text(&last.content, thread_context.as_ref());
    let activity_query = activity_query_text(&last.content, &history, thread_context.as_ref());
    let retrieved_memories = safe_local_search(&profile, &corrections, &memory_query, 8);
    let memory_provider = "local-profile";
    let (activity_start, activity_end) = query_activity_range(&last.content, today_start, now);
    let mut activity_facts =
        state
            .db
            .query_activity_facts(&activity_query, activity_start, activity_end)?;
    if crate::db::recent_file_activity_intent(&last.content) {
        let modified_files = chat_modified_files(
            &state.db,
            thread_context.as_ref(),
            activity_start,
            activity_end,
        );
        for facts in &mut activity_facts {
            facts.modified_files.clone_from(&modified_files);
        }
    }
    let baseline_context = build_baseline_context(
        &profile,
        &corrections,
        &activity_summary,
        &retrieved_memories,
        &activity_facts,
        thread_context.as_ref(),
    );
    let optimized_package = build_optimized_context_package(
        &retrieved_memories,
        &activity_facts,
        thread_context.as_ref(),
        configured_context_token_budget().min(available_context_tokens),
    );
    let optimized_context = &optimized_package.text;
    let baseline_prompt =
        compose_measured_prompt(&baseline_context, &conversation_text, &last.content);
    let optimized_prompt =
        compose_measured_prompt(optimized_context, &conversation_text, &last.content);
    let selected_context = optimized_context;
    let provider_started = Instant::now();
    let completion = state
        .providers
        .chat(
            &provider,
            selected_context,
            &history,
            &last.content,
            request_budget.saturating_sub(CHAT_OUTPUT_TOKEN_RESERVE),
        )
        .await?;
    let latency_ms = provider_started.elapsed().as_millis() as i64;
    let full_prompt_input_tokens = completion
        .preflight_input_tokens
        .or(completion.input_tokens);
    let measurement = provider_scaled_measurement(
        &baseline_prompt,
        &optimized_prompt,
        full_prompt_input_tokens,
        &mode,
    );
    let query_id = Uuid::new_v4().to_string();
    let output_tokens = completion
        .output_tokens
        .or_else(|| Some(estimated_tokens(&completion.text)));
    let run = InferenceRun {
        id: query_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        model: completion.model.clone(),
        baseline_input_tokens: measurement.baseline_input_tokens,
        optimized_input_tokens: measurement.optimized_input_tokens,
        tokens_saved: measurement.tokens_saved(),
        reduction_percent: measurement.reduction_percent(),
        actual_input_tokens: completion.input_tokens,
        output_tokens,
        latency_ms,
        estimated_cost_usd: estimated_cost_usd(
            completion.input_tokens,
            output_tokens,
            completion.cache_read_input_tokens,
            completion.cache_write_input_tokens,
        ),
        memory_count: retrieved_memories.len() as i64,
        context_budget_tokens: optimized_package.manifest.budget_tokens,
        context_estimated_tokens: optimized_package.manifest.estimated_tokens,
        context_units_considered: optimized_package.manifest.units_considered as i64,
        context_units_sent: optimized_package.manifest.units_sent as i64,
        context_units_omitted: optimized_package.manifest.units_omitted as i64,
        context_detail_level: optimized_package.manifest.detail_level.clone(),
        provider_preflight_input_tokens: completion.preflight_input_tokens,
        cache_read_input_tokens: completion.cache_read_input_tokens,
        cache_write_input_tokens: completion.cache_write_input_tokens,
        mode: mode.clone(),
        memory_provider: memory_provider.into(),
        measurement_method: measurement.measurement_method.clone(),
    };
    state.db.record_inference_run(&run)?;
    let telemetry_status = "stored-locally";
    Ok(json!({
        "message":{
            "id":Uuid::new_v4().to_string(),
            "role":"assistant",
            "content":completion.text,
            "createdAt":Utc::now().to_rfc3339()
        },
        "retrievedMemories":retrieved_memories,
        "economics":{
            "queryId":query_id,
            "mode":mode,
            "model":completion.model,
            "baselineInputTokens":measurement.baseline_input_tokens,
            "optimizedInputTokens":measurement.optimized_input_tokens,
            "tokensSaved":measurement.tokens_saved(),
            "reductionPercent":measurement.reduction_percent(),
            "actualInputTokens":completion.input_tokens,
            "outputTokens":output_tokens,
            "latencyMs":latency_ms,
            "estimatedCostUsd":run.estimated_cost_usd,
            "memoryCount":retrieved_memories.len(),
            "contextBudgetTokens":optimized_package.manifest.budget_tokens,
            "contextEstimatedTokens":optimized_package.manifest.estimated_tokens,
            "contextUnitsConsidered":optimized_package.manifest.units_considered,
            "contextUnitsSent":optimized_package.manifest.units_sent,
            "contextUnitsOmitted":optimized_package.manifest.units_omitted,
            "contextDetailLevel":optimized_package.manifest.detail_level,
            "providerPreflightInputTokens":completion.preflight_input_tokens,
            "cacheReadInputTokens":completion.cache_read_input_tokens,
            "cacheWriteInputTokens":completion.cache_write_input_tokens,
            "measurementMethod":measurement.measurement_method,
            "telemetryStatus":telemetry_status,
            "baselineContextPreview":baseline_context,
            "optimizedContextPreview":optimized_context
        }
    }))
}

#[tauri::command]
pub fn get_pairing_info(state: State<'_, AppState>) -> AppResult<Value> {
    Ok(json!({
        "nativeHost":"com.knov.companion",
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
        directory.join("knov-native-host"),
        directory.join("../Resources/knov-native-host"),
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
    let manifest_path = manifest_dir.join("com.knov.companion.json");
    let manifest = json!({
        "name":"com.knov.companion",
        "description":"Knov local activity bridge",
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
    state.providers.delete_key("bedrock")?;
    state.db.delete_all_local_data()?;
    ensure_pairing_token(&state.db)?;
    if let Some(home) = dirs::home_dir() {
        let manifest = home.join(
            "Library/Application Support/Google/Chrome/NativeMessagingHosts/com.knov.companion.json",
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

fn editor_workspace_changes_by_app(
    history: &[crate::models::ActivityEvent],
    since: i64,
) -> HashMap<String, Vec<String>> {
    let mut changes = HashMap::new();
    for event in history {
        let key = event.app_name.to_ascii_lowercase();
        if changes.contains_key(&key)
            || !matches!(
                key.as_str(),
                "code" | "visual studio code" | "cursor" | "cortex code" | "xcode"
            )
        {
            continue;
        }
        let paths = recent_editor_workspace_changes(&event.app_name, since, 8);
        if !paths.is_empty() {
            changes.insert(key, paths);
        }
    }
    changes
}

fn activity_to_ui_with_topic(
    event: crate::models::ActivityEvent,
    semantic_topic: Option<&str>,
    modified_files: &[String],
) -> Value {
    let topic = semantic_topic.map(str::to_string).or_else(|| {
        if event.source == crate::models::ActivitySource::EditorHistory {
            event.window_title.as_deref().and_then(|title| {
                title
                    .split_once(" — ")
                    .map(|(workspace, _)| workspace.to_string())
            })
        } else {
            inferred_topic(&event).map(str::to_string)
        }
    });
    json!({
        "id":event.id.unwrap_or_default().to_string(),
        "appName":event.app_name,
        "windowTitle":event.window_title,
        "url":event.url,
        "pageTitle":event.page_title,
        "searchQuery":event.search_query,
        "browserProfile":event.browser_profile_id,
        "startedAt":timestamp(event.occurred_at),
        "durationSeconds":event.duration_seconds,
        "modifiedFiles":modified_files,
        "topic":topic,
        "source":match event.source {
            crate::models::ActivitySource::AppFocus=>"collector",
            crate::models::ActivitySource::ChromeHistory=>"history",
            crate::models::ActivitySource::ChromeExtension=>"chrome",
            crate::models::ActivitySource::EditorHistory=>"editor",
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

fn query_activity_range(query: &str, today_start: i64, now: i64) -> (i64, i64) {
    let query = query.to_ascii_lowercase();
    if query.contains("yesterday") {
        (today_start - 86_400, today_start.saturating_sub(1))
    } else if query.contains("today") {
        (today_start, now)
    } else if query.contains("this week")
        || query.contains("past week")
        || query.contains("last week")
        || query.contains("7 days")
    {
        (now - 7 * 86_400, now)
    } else {
        (now - 30 * 86_400, now)
    }
}

fn activity_query_text(
    question: &str,
    history: &[ChatMessage],
    thread_context: Option<&ThreadContext>,
) -> String {
    let references_prior_subject = question
        .split(|character: char| !character.is_alphanumeric())
        .map(str::to_ascii_lowercase)
        .any(|word| matches!(word.as_str(), "it" | "that" | "this" | "them" | "those"));
    let mut parts = vec![memory_query_text(question, thread_context)];
    if references_prior_subject && !crate::db::has_meaningful_activity_subject(question) {
        if let Some(previous_question) = history
            .iter()
            .rev()
            .find(|message| message.role == "user" && !message.content.trim().is_empty())
        {
            parts.push(previous_question.content.trim().into());
        }
    }
    parts.join(" ")
}

fn chat_modified_files(
    db: &Database,
    thread_context: Option<&ThreadContext>,
    start_at: i64,
    end_at: i64,
) -> Vec<String> {
    if let Some(files) = thread_context
        .map(|context| context.modified_files.clone())
        .filter(|files| !files.is_empty())
    {
        return files;
    }

    let mut apps = thread_context
        .into_iter()
        .flat_map(|context| context.apps.iter().cloned())
        .collect::<Vec<_>>();
    let recent_activity = db
        .history(&HistoryRequest {
            start_at,
            end_at,
            search: None,
            source: None,
            limit: Some(1_000),
            offset: None,
        })
        .unwrap_or_default();
    apps.extend(recent_activity.into_iter().map(|event| event.app_name));
    apps.extend(
        ["Code", "Cursor", "Cortex Code", "Xcode"]
            .into_iter()
            .map(str::to_string),
    );

    let mut seen = HashSet::new();
    apps.into_iter()
        .filter(|app| {
            matches!(
                app.trim().to_ascii_lowercase().as_str(),
                "code" | "visual studio code" | "cursor" | "cortex code" | "xcode"
            )
        })
        .filter(|app| seen.insert(app.trim().to_ascii_lowercase()))
        .find_map(|app| {
            let files = recent_editor_workspace_changes(&app, start_at, 16);
            (!files.is_empty()).then_some(files)
        })
        .unwrap_or_default()
}

fn memory_query_text(question: &str, thread_context: Option<&ThreadContext>) -> String {
    let has_explicit_known_subject = contains_known_activity_subject(question);
    let mut parts = vec![question.trim().to_string()];
    if let Some(subject) = thread_context
        .and_then(|context| safe_memory_subject(&context.subject))
        .filter(|subject| {
            !question
                .to_ascii_lowercase()
                .contains(&subject.to_ascii_lowercase())
        })
        .filter(|_| !has_explicit_known_subject)
    {
        parts.push(subject);
    }
    parts.join(" ")
}

fn safe_memory_subject(subject: &str) -> Option<String> {
    let subject = subject.split_whitespace().collect::<Vec<_>>().join(" ");
    let lowered = subject.to_ascii_lowercase();
    let looks_sensitive = subject.is_empty()
        || [
            "authorization:",
            "bearer ",
            "api_key",
            "apikey",
            "password=",
            "secret=",
            "token=",
            "file://",
            "/users/",
            "/home/",
        ]
        .iter()
        .any(|marker| lowered.contains(marker));
    (!looks_sensitive).then_some(subject)
}

fn contains_known_activity_subject(question: &str) -> bool {
    let question = question.to_ascii_lowercase();
    [
        "bigquery",
        "databricks",
        "openai",
        "postgresql",
        "snowflake",
    ]
    .iter()
    .any(|subject| question.contains(subject))
}

fn validated_thread_context(context: Option<ThreadContext>) -> AppResult<Option<ThreadContext>> {
    let Some(context) = context else {
        return Ok(None);
    };
    if context.version != 1 {
        return Err(AppError::InvalidInput(
            "Unsupported thread-context version.".into(),
        ));
    }
    if context.subject.trim().is_empty() || context.subject.chars().count() > 160 {
        return Err(AppError::InvalidInput(
            "Thread context requires a bounded subject.".into(),
        ));
    }
    if context.events.len() > 100 {
        return Err(AppError::InvalidInput(
            "Thread context may include at most 100 event candidates.".into(),
        ));
    }
    Ok(Some(context))
}

fn is_synthetic_welcome(message: &UiChatMessage) -> bool {
    message.role == "assistant"
        && (message
            .content
            .starts_with("Your selected thread is ready.")
            || message.content.starts_with("Ask anything. I’ll retrieve"))
}

fn bounded_chat_history(messages: &[UiChatMessage], token_budget: i64) -> Vec<ChatMessage> {
    if token_budget <= 0 {
        return Vec::new();
    }
    let mut remaining = token_budget;
    let mut selected = Vec::new();
    for message in messages
        .iter()
        .rev()
        .filter(|message| matches!(message.role.as_str(), "user" | "assistant"))
        .filter(|message| !is_synthetic_welcome(message))
        .take(8)
    {
        let content = message
            .content
            .chars()
            .take(MAX_CHAT_HISTORY_MESSAGE_CHARS)
            .collect::<String>();
        let tokens = estimated_tokens(&content);
        if tokens > remaining {
            continue;
        }
        remaining -= tokens;
        selected.push(ChatMessage {
            role: message.role.clone(),
            content,
        });
    }
    selected.reverse();
    while selected
        .first()
        .is_some_and(|message| message.role == "assistant")
    {
        selected.remove(0);
    }
    selected
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
    use crate::models::{ActivityEvent, ActivitySource};

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
    fn query_activity_range_honors_explicit_recent_scopes() {
        let now = 10 * 86_400;
        let today_start = 9 * 86_400;
        assert_eq!(
            query_activity_range("How long did I work today?", today_start, now),
            (today_start, now)
        );
        assert_eq!(
            query_activity_range("What did I do yesterday?", today_start, now),
            (today_start - 86_400, today_start - 1)
        );
        assert_eq!(
            query_activity_range("How much Snowflake time this week?", today_start, now),
            (now - 7 * 86_400, now)
        );
        assert_eq!(
            query_activity_range("Since when have I used Snowflake?", today_start, now),
            (now - 30 * 86_400, now)
        );
    }

    #[test]
    fn activity_query_text_resolves_follow_up_subjects_from_recent_context() {
        let thread_context = |subject: &str| ThreadContext {
            version: 1,
            subject: subject.into(),
            signal_count: 38,
            apps: vec!["Google Chrome".into()],
            modified_files: vec![],
            observed_from: None,
            observed_through: None,
            events: vec![],
        };
        let history = vec![ChatMessage {
            role: "user".into(),
            content: "Tell me about Snowflake.".into(),
        }];
        assert_eq!(
            activity_query_text("How long have I worked on it?", &history, None),
            "How long have I worked on it? Tell me about Snowflake."
        );
        assert_eq!(
            memory_query_text("How long have I worked on it?", None),
            "How long have I worked on it?"
        );
        assert_eq!(
            memory_query_text(
                "What next?",
                Some(&thread_context("Authorization: Bearer local-secret")),
            ),
            "What next?"
        );
        assert_eq!(
            activity_query_text("How long on this?", &[], Some(&thread_context("Snowflake")),),
            "How long on this? Snowflake"
        );
        assert_eq!(
            activity_query_text("What next?", &[], Some(&thread_context("Snowflake"))),
            "What next? Snowflake"
        );
        assert_eq!(
            activity_query_text(
                "How long on this Snowflake project?",
                &history,
                Some(&thread_context("BigQuery")),
            ),
            "How long on this Snowflake project?"
        );
    }

    #[test]
    fn bounded_chat_history_limits_messages_characters_and_tokens() {
        let messages = vec![
            UiChatMessage {
                role: "assistant".into(),
                content: "Ask anything. I’ll retrieve only approved memories.".into(),
            },
            UiChatMessage {
                role: "user".into(),
                content: "old ".repeat(2_000),
            },
            UiChatMessage {
                role: "assistant".into(),
                content: "old answer ".repeat(1_000),
            },
            UiChatMessage {
                role: "user".into(),
                content: "recent question".into(),
            },
            UiChatMessage {
                role: "assistant".into(),
                content: "recent answer".into(),
            },
        ];

        let history = bounded_chat_history(&messages, 100);
        let total_tokens = history
            .iter()
            .map(|message| estimated_tokens(&message.content))
            .sum::<i64>();

        assert!(history.len() <= 8);
        assert!(history
            .iter()
            .all(|message| message.content.chars().count() <= MAX_CHAT_HISTORY_MESSAGE_CHARS));
        assert!(total_tokens <= 100);
        assert_ne!(
            history.first().map(|message| message.role.as_str()),
            Some("assistant")
        );
        assert!(history
            .iter()
            .all(|message| !is_synthetic_welcome(&UiChatMessage {
                role: message.role.clone(),
                content: message.content.clone(),
            })));
    }

    #[test]
    fn refresh_permit_releases_lock_after_failure_scope() {
        let lock = Arc::new(AtomicBool::new(false));
        let permit = acquire_refresh(&lock).expect("first refresh should acquire the lock");
        assert!(acquire_refresh(&lock).is_err());

        drop(permit);
        assert!(acquire_refresh(&lock).is_ok());
    }

    #[test]
    fn user_refresh_waits_for_an_active_refresh_to_finish() {
        let lock = Arc::new(AtomicBool::new(false));
        let permit = acquire_refresh(&lock).expect("background refresh should acquire the lock");
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            drop(permit);
        });

        let next = wait_for_refresh(&lock, Duration::from_secs(1))
            .expect("user refresh should acquire the released lock");
        release.join().expect("release thread should finish");
        drop(next);

        assert!(acquire_refresh(&lock).is_ok());
    }

    #[test]
    fn user_refresh_times_out_before_import_when_refresh_stays_busy() {
        let lock = Arc::new(AtomicBool::new(false));
        let _permit = acquire_refresh(&lock).expect("background refresh should acquire the lock");

        let error = wait_for_refresh(&lock, Duration::from_millis(1))
            .expect_err("the wait should time out while the lock remains held");

        assert_eq!(
            error.to_string(),
            "invalid input: Another profile refresh is still running. Try again in a moment."
        );
    }

    #[test]
    fn activity_icon_detection_accepts_common_favicon_formats() {
        assert_eq!(
            detect_image_mime(b"\x89PNG\r\n\x1a\nrest"),
            Some("image/png")
        );
        assert_eq!(
            detect_image_mime(b"\x00\x00\x01\x00rest"),
            Some("image/x-icon")
        );
        assert_eq!(detect_image_mime(b"not an image"), None);
    }

    #[test]
    fn activity_icon_parser_finds_declared_shortcut_icons() {
        let html = r#"<html><head><link rel="shortcut icon" href="/favicon.svg" type="image/svg+xml"></head></html>"#;
        assert_eq!(declared_icon_href(html).as_deref(), Some("/favicon.svg"));
    }

    #[test]
    fn activity_preview_normalizes_supported_youtube_video_urls() {
        for value in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=20",
            "https://youtu.be/dQw4w9WgXcQ?si=example",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
        ] {
            let preview = activity_preview(value).expect("YouTube URL should be accepted");
            assert_eq!(preview.kind, "youtube");
            assert_eq!(preview.url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            assert_eq!(
                preview.embed_url.as_deref(),
                Some("https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ")
            );
            assert_eq!(preview.thumbnail_data_url, None);
        }
    }

    #[test]
    fn activity_preview_keeps_non_video_and_lookalike_hosts_link_only() {
        for value in [
            "https://www.youtube.com/playlist?list=PL123",
            "https://youtube.com.evil.example/watch?v=dQw4w9WgXcQ",
            "https://example.com/article#section",
        ] {
            let preview = activity_preview(value).expect("ordinary web URL should be accepted");
            assert_eq!(preview.kind, "link");
            assert_eq!(preview.thumbnail_data_url, None);
            assert_eq!(preview.embed_url, None);
            assert!(!preview.url.contains('#'));
        }
    }

    #[test]
    fn activity_preview_rejects_unsafe_urls_and_invalid_video_ids() {
        for value in [
            "javascript:alert(1)",
            "file:///tmp/private",
            "https://user:secret@example.com/",
        ] {
            assert!(activity_preview(value).is_err());
        }

        let preview = activity_preview("https://youtube.com/watch?v=../../secret")
            .expect("invalid video id should safely fall back to a link");
        assert_eq!(preview.kind, "link");
        assert!(youtube_video_id(&preview.url).is_none());
    }

    #[test]
    fn reopenable_resources_allow_only_hosted_web_urls() {
        assert!(reopenable_web_url("https://example.com/work").is_ok());
        assert!(reopenable_web_url("http://localhost:1420/dashboard").is_ok());
        assert!(reopenable_web_url("file:///tmp/private.txt").is_err());
        assert!(reopenable_web_url("javascript:alert(1)").is_err());
        assert!(reopenable_web_url("https://user:secret@example.com/").is_err());
        assert!(reopenable_web_url("not a url").is_err());
    }

    #[test]
    fn application_names_are_trimmed_for_native_launching() {
        assert_eq!(
            normalized_application_name("  Visual Studio Code  ")
                .expect("ordinary recorded app names should be accepted"),
            "Visual Studio Code"
        );
        assert_eq!(
            normalized_application_name("Safari")
                .expect("single-word application names should be accepted"),
            "Safari"
        );
        assert_eq!(
            normalized_application_name("Code")
                .expect("recorded application aliases should resolve to installed app names"),
            "Visual Studio Code"
        );
    }

    #[test]
    fn application_names_reject_empty_control_and_path_like_values() {
        for value in [
            "",
            "   ",
            "\nSafari",
            "Finder\nCalculator",
            "../../Calculator",
            r"Applications\Calculator",
            "-W",
            ".",
            "..",
        ] {
            assert!(
                normalized_application_name(value).is_err(),
                "{value:?} should be rejected"
            );
        }
        assert!(normalized_application_name(&"a".repeat(129)).is_err());
    }

    #[test]
    fn activity_ui_prefers_semantic_subject_and_includes_search_evidence() {
        let event = ActivityEvent {
            id: Some(42),
            occurred_at: 1,
            ended_at: None,
            duration_seconds: 0,
            app_name: "Google Chrome".into(),
            window_title: Some("snowflake - Google Search".into()),
            url: Some("https://google.com/search?q=snowflake".into()),
            page_title: Some("snowflake - Google Search".into()),
            search_query: Some("snowflake architecture".into()),
            browser_profile_id: Some("Default".into()),
            source: ActivitySource::ChromeHistory,
            is_bootstrap: false,
        };

        let modified_files = vec!["src/main.rs".to_string()];
        let value = activity_to_ui_with_topic(event, Some("Snowflake"), &modified_files);

        assert_eq!(value["topic"], "Snowflake");
        assert_eq!(value["searchQuery"], "snowflake architecture");
        assert_eq!(value["modifiedFiles"], json!(["src/main.rs"]));
    }

    #[test]
    fn dashboard_activity_preserves_video_evidence_beyond_global_recent_limit() {
        let mut history = (0..250)
            .map(|index| ActivityEvent {
                id: Some(1_000 + index),
                occurred_at: 10_000 - index,
                ended_at: None,
                duration_seconds: 5,
                app_name: "Finder".into(),
                window_title: Some(format!("Folder {index}")),
                url: None,
                page_title: None,
                search_query: None,
                browser_profile_id: None,
                source: ActivitySource::AppFocus,
                is_bootstrap: false,
            })
            .collect::<Vec<_>>();
        history.extend((0..3).map(|index| ActivityEvent {
            id: Some(10 + index),
            occurred_at: 1_000 - index,
            ended_at: None,
            duration_seconds: 0,
            app_name: "Google Chrome".into(),
            window_title: None,
            url: Some(format!("https://youtube.com/watch?v=research-{index}")),
            page_title: Some(format!("Research video {index}")),
            search_query: None,
            browser_profile_id: Some("Default".into()),
            source: ActivitySource::ChromeHistory,
            is_bootstrap: false,
        }));
        let semantic_topics = HashMap::new();
        let topics = ranked_topics(&history, &semantic_topics);

        let selected = select_dashboard_activity(&history, &semantic_topics, &topics);

        assert_eq!(
            selected
                .iter()
                .filter(|event| inferred_topic(event) == Some("Video research"))
                .count(),
            3
        );
        assert!(selected.len() > RECENT_ACTIVITY_LIMIT);
    }
}
