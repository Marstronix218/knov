use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use tiny_http::{Header, Method, Response, Server, StatusCode};
use url::Url;
use uuid::Uuid;

use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{ActivityEvent, ActivitySource, ChromeProfile, CollectionStatus},
};

const MAX_HISTORICAL_VISIT_SECONDS: i64 = 12 * 60 * 60;

#[derive(Default)]
pub struct RuntimeStatus {
    pub accessibility_available: bool,
    pub accessibility_message: Option<String>,
}

pub fn discover_chrome_profiles(db: &Database) -> AppResult<Vec<ChromeProfile>> {
    let root = chrome_root()
        .ok_or_else(|| AppError::InvalidInput("Chrome data directory not found".into()))?;
    let local_state = fs::read_to_string(root.join("Local State")).unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&local_state).unwrap_or_default();
    let info = parsed
        .pointer("/profile/info_cache")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let settings = db.settings()?;
    let mut persisted = Vec::new();
    let mut result = Vec::new();
    for (directory, metadata) in info {
        let path = root.join(&directory);
        if !path.join("History").exists() {
            continue;
        }
        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&directory)
            .to_string();
        persisted.push((
            directory.clone(),
            name.clone(),
            path.to_string_lossy().into_owned(),
        ));
        result.push(ChromeProfile {
            selected: settings.selected_chrome_profiles.contains(&directory),
            id: directory,
            name,
            path: path.to_string_lossy().into_owned(),
            support_level: "history_and_live_tab".into(),
        });
    }
    // Chrome occasionally omits info_cache; Default still exists.
    if result.is_empty() {
        let path = root.join("Default");
        if path.join("History").exists() {
            persisted.push((
                "Default".into(),
                "Default".into(),
                path.to_string_lossy().into_owned(),
            ));
            result.push(ChromeProfile {
                id: "Default".into(),
                name: "Default".into(),
                path: path.to_string_lossy().into_owned(),
                selected: settings
                    .selected_chrome_profiles
                    .iter()
                    .any(|v| v == "Default"),
                support_level: "history_and_live_tab".into(),
            });
        }
    }
    db.save_chrome_profiles(&persisted)?;
    Ok(result)
}

pub fn import_selected_chrome_history(db: &Database, lookback_days: i64) -> AppResult<usize> {
    let cutoff = Utc::now().timestamp() - lookback_days.max(0) * 86_400;
    let mut imported = 0;
    for (profile_id, profile_path) in db.selected_profile_paths()? {
        imported += import_chrome_history(db, &profile_id, &profile_path.join("History"), cutoff)?;
    }
    Ok(imported)
}

fn import_chrome_history(
    db: &Database,
    profile_id: &str,
    source: &Path,
    cutoff: i64,
) -> AppResult<usize> {
    if !source.exists() {
        return Ok(0);
    }
    let temporary = std::env::temp_dir().join(format!("knov-history-{}.sqlite", Uuid::new_v4()));
    fs::copy(source, &temporary)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    }
    let result = (|| -> AppResult<usize> {
        let connection = Connection::open(&temporary)?;
        let chrome_cutoff = (cutoff + 11_644_473_600) * 1_000_000;
        let has_visit_duration = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM pragma_table_info('visits') WHERE name='visit_duration'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?;
        let visit_duration = if has_visit_duration {
            "v.visit_duration"
        } else {
            "0"
        };
        let mut statement = connection.prepare(&format!(
            "SELECT u.url,u.title,v.visit_time,{visit_duration}
             FROM visits v JOIN urls u ON u.id=v.url
             WHERE v.visit_time>=?1 ORDER BY v.visit_time"
        ))?;
        let rows = statement.query_map([chrome_cutoff], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let now = Utc::now().timestamp();
        let mut count = 0;
        for row in rows {
            let (url, title, chrome_time, chrome_duration) = row?;
            let occurred_at = chrome_time / 1_000_000 - 11_644_473_600;
            let reported_duration = chrome_duration.max(0) / 1_000_000;
            let duration_seconds = if reported_duration <= MAX_HISTORICAL_VISIT_SECONDS
                && reported_duration <= now.saturating_sub(occurred_at)
            {
                reported_duration
            } else {
                0
            };
            let search_query = extract_search_query(&url);
            let event = ActivityEvent {
                id: None,
                occurred_at,
                ended_at: (duration_seconds > 0)
                    .then(|| occurred_at.saturating_add(duration_seconds)),
                duration_seconds,
                app_name: "Google Chrome".into(),
                window_title: None,
                url: Some(url.clone()),
                page_title: nonempty(title),
                search_query,
                browser_profile_id: Some(profile_id.into()),
                source: ActivitySource::ChromeHistory,
                is_bootstrap: occurred_at < now - 30 * 86_400,
            };
            if db.insert_event(&event, &fingerprint(&event))? {
                count += 1;
            }
        }
        Ok(count)
    })();
    let cleanup = fs::remove_file(temporary);
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(AppError::Io(error)),
        (Ok(count), Ok(())) => Ok(count),
    }
}

pub fn start_collector(db: Arc<Database>, runtime: Arc<RwLock<RuntimeStatus>>) {
    thread::spawn(move || {
        let mut current: Option<ActivityEvent> = None;
        loop {
            let settings = db.settings().unwrap_or_default();
            let _ = db.purge_expired(Utc::now().timestamp(), false);
            if !settings.collection_enabled {
                current = None;
                thread::sleep(Duration::from_secs(2));
                continue;
            }
            if let Some(idle_seconds) = inactive_user_seconds() {
                if idle_seconds >= 300 {
                    if let Some(mut completed) = current.take() {
                        let ended_at = Utc::now().timestamp().saturating_sub(idle_seconds as i64);
                        completed.ended_at = Some(ended_at);
                        completed.duration_seconds = ended_at.saturating_sub(completed.occurred_at);
                        let _ = db.insert_event(&completed, &fingerprint(&completed));
                    }
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
            }
            match active_window() {
                Ok((app_name, title)) => {
                    if let Ok(mut state) = runtime.write() {
                        state.accessibility_available = title.is_some();
                        state.accessibility_message = if title.is_none() {
                            Some("Foreground apps are available, but window titles require Accessibility permission.".into())
                        } else {
                            None
                        };
                    }
                    let excluded = settings
                        .excluded_apps
                        .iter()
                        .any(|v| v.eq_ignore_ascii_case(&app_name));
                    let extension_recent = db
                        .extension_state()
                        .ok()
                        .flatten()
                        .and_then(|(_, seen)| seen)
                        .map(|seen| Utc::now().timestamp() - seen < 120)
                        .unwrap_or(false);
                    let chrome_is_covered =
                        app_name.eq_ignore_ascii_case("Google Chrome") && extension_recent;
                    if excluded || chrome_is_covered {
                        current = None;
                    } else {
                        let now = Utc::now().timestamp();
                        let changed = current
                            .as_ref()
                            .map(|event| event.app_name != app_name || event.window_title != title)
                            .unwrap_or(true);
                        if changed {
                            if let Some(mut completed) = current.take() {
                                completed.ended_at = Some(now);
                                completed.duration_seconds =
                                    now.saturating_sub(completed.occurred_at);
                                let _ = db.insert_event(&completed, &fingerprint(&completed));
                            }
                            current = Some(ActivityEvent {
                                id: None,
                                occurred_at: now,
                                ended_at: None,
                                duration_seconds: 0,
                                app_name,
                                window_title: title,
                                url: None,
                                page_title: None,
                                search_query: None,
                                browser_profile_id: None,
                                source: ActivitySource::AppFocus,
                                is_bootstrap: false,
                            });
                        }
                        if let Some(active) = current.as_mut() {
                            active.ended_at = Some(now);
                            active.duration_seconds = now.saturating_sub(active.occurred_at);
                            let _ = db.insert_event(active, &fingerprint(active));
                        }
                    }
                }
                Err(message) => {
                    if let Ok(mut state) = runtime.write() {
                        state.accessibility_available = false;
                        state.accessibility_message = Some(message);
                    }
                }
            }
            thread::sleep(Duration::from_secs(
                settings.sampling_interval_seconds.clamp(2, 300),
            ));
        }
    });
}

#[cfg(target_os = "macos")]
fn active_window() -> Result<(String, Option<String>), String> {
    let script = r#"tell application "System Events"
set p to first application process whose frontmost is true
set appName to name of p
try
set windowName to name of front window of p
on error
set windowName to ""
end try
return appName & linefeed & windowName
end tell"#;
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(if message.trim().is_empty() {
            "Accessibility permission is unavailable.".into()
        } else {
            format!(
                "Accessibility permission is unavailable: {}",
                message.trim()
            )
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let app = lines.next().unwrap_or("").trim().to_string();
    if app.is_empty() {
        return Err("No foreground application was reported.".into());
    }
    Ok((app, lines.next().and_then(|v| nonempty(v.to_string()))))
}

#[cfg(target_os = "macos")]
fn inactive_user_seconds() -> Option<u64> {
    let output = Command::new("/usr/sbin/ioreg")
        .args(["-c", "IOHIDSystem"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    if text.contains("CGSSessionScreenIsLocked") && text.contains("Yes") {
        return Some(300);
    }
    let line = text.lines().find(|line| line.contains("\"HIDIdleTime\""))?;
    let nanoseconds = line.split('=').nth(1)?.trim().parse::<u64>().ok()?;
    Some(nanoseconds / 1_000_000_000)
}

#[cfg(not(target_os = "macos"))]
fn active_window() -> Result<(String, Option<String>), String> {
    Err("Foreground collection is supported on macOS only.".into())
}

#[cfg(not(target_os = "macos"))]
fn inactive_user_seconds() -> Option<u64> {
    None
}

pub fn ensure_pairing_token(db: &Database) -> AppResult<String> {
    if let Some((token, _)) = db.extension_state()? {
        return Ok(token);
    }
    let token = URL_SAFE_NO_PAD.encode(Uuid::new_v4().as_bytes());
    db.set_pairing_token(&token)?;
    Ok(token)
}

pub fn start_ingestion_server(db: Arc<Database>) -> AppResult<()> {
    ensure_pairing_token(&db)?;
    start_native_socket(db.clone())?;
    let server = Server::http("127.0.0.1:48321")
        .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?;
    thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let cors =
                Header::from_bytes("Access-Control-Allow-Origin", "chrome-extension://*").ok();
            if request.method() == &Method::Options {
                let mut response = Response::empty(204);
                if let Some(header) = cors.clone() {
                    response.add_header(header);
                }
                let _ = request.respond(response);
                continue;
            }
            let response = handle_ingestion_request(&db, &mut request);
            let mut response = match response {
                Ok(value) => Response::from_string(value).with_status_code(StatusCode(200)),
                Err((code, message)) => {
                    Response::from_string(message).with_status_code(StatusCode(code))
                }
            };
            if let Some(header) = cors.clone() {
                response.add_header(header);
            }
            let _ = request.respond(response);
        }
    });
    Ok(())
}

#[cfg(unix)]
fn start_native_socket(db: Arc<Database>) -> AppResult<()> {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};

    let directory = dirs::data_dir()
        .ok_or_else(|| AppError::InvalidInput("Application data directory unavailable".into()))?
        .join("com.knov.desktop");
    fs::create_dir_all(&directory)?;
    let socket_path = directory.join("native-messaging.sock");
    if socket_path.exists() {
        fs::remove_file(&socket_path)?;
    }
    let listener = UnixListener::bind(&socket_path)?;
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut payload = Vec::new();
            let response = match Read::by_ref(&mut stream)
                .take(256 * 1024)
                .read_to_end(&mut payload)
            {
                Ok(_) => serde_json::from_slice::<NativeEnvelope>(&payload)
                    .map(|envelope| handle_native_envelope(&db, envelope))
                    .unwrap_or_else(|_| {
                        serde_json::json!({
                            "protocolVersion":1,"requestId":"","ok":false,
                            "errorCode":"protocol","message":"Invalid request."
                        })
                    }),
                Err(_) => serde_json::json!({
                    "protocolVersion":1,"requestId":"","ok":false,
                    "errorCode":"protocol","message":"Could not read request."
                }),
            };
            let _ = stream.write_all(&serde_json::to_vec(&response).unwrap_or_default());
        }
    });
    Ok(())
}

#[cfg(not(unix))]
fn start_native_socket(_db: Arc<Database>) -> AppResult<()> {
    Ok(())
}

fn handle_ingestion_request(
    db: &Database,
    request: &mut tiny_http::Request,
) -> Result<String, (u16, String)> {
    if request.method() == &Method::Post && request.url() == "/v1/extension/native" {
        let mut body = String::new();
        request
            .as_reader()
            .take(256 * 1024)
            .read_to_string(&mut body)
            .map_err(|_| (400, "invalid body".into()))?;
        let envelope: NativeEnvelope =
            serde_json::from_str(&body).map_err(|_| (400, "invalid request".into()))?;
        let response = handle_native_envelope(db, envelope);
        return Ok(response.to_string());
    }
    let is_status = request.method() == &Method::Get && request.url() == "/v1/extension/status";
    let is_events = request.method() == &Method::Post && request.url() == "/v1/extension/events";
    if !is_status && !is_events {
        return Err((404, "not found".into()));
    }
    let authorization = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))
        .map(|h| h.value.as_str())
        .unwrap_or("");
    let token = authorization.strip_prefix("Bearer ").unwrap_or("");
    if !db
        .mark_extension_seen(token, Utc::now().timestamp())
        .map_err(|_| (500, "storage error".into()))?
    {
        return Err((401, "invalid pairing token".into()));
    }
    if is_status {
        let settings = db.settings().map_err(|_| (500, "storage error".into()))?;
        return Ok(serde_json::json!({
            "protocolVersion":1,
            "ok":true,
            "collectionEnabled":settings.collection_enabled
        })
        .to_string());
    }
    let mut body = String::new();
    request
        .as_reader()
        .take(256 * 1024)
        .read_to_string(&mut body)
        .map_err(|_| (400, "invalid body".into()))?;
    let batch: ExtensionEventBatch =
        serde_json::from_str(&body).map_err(|_| (400, "invalid event payload".into()))?;
    let settings = db.settings().map_err(|_| (500, "storage error".into()))?;
    if !settings.collection_enabled {
        return Err((409, "collection paused".into()));
    }
    let accepted = ingest_extension_batch(db, &settings, batch)?;
    Ok(serde_json::json!({
        "protocolVersion":1,
        "ok":true,
        "acceptedEventIds":accepted,
        "collectionEnabled":true
    })
    .to_string())
}

fn ingest_extension_batch(
    db: &Database,
    settings: &crate::models::Settings,
    batch: ExtensionEventBatch,
) -> Result<Vec<String>, (u16, String)> {
    if batch.protocol_version != 1 || batch.source != "chrome_extension" {
        return Err((400, "unsupported protocol".into()));
    }
    let mut accepted = Vec::new();
    for incoming in batch.events {
        let incoming_id = incoming.id.clone();
        if incoming.incognito {
            accepted.push(incoming_id);
            continue;
        }
        if !settings
            .selected_chrome_profiles
            .iter()
            .any(|profile| profile == &incoming.browser_profile_id)
        {
            accepted.push(incoming_id);
            continue;
        }
        let started_at = chrono::DateTime::parse_from_rfc3339(&incoming.started_at)
            .map_err(|_| (400, "invalid event timestamp".into()))?
            .timestamp();
        let ended_at = chrono::DateTime::parse_from_rfc3339(&incoming.ended_at)
            .map_err(|_| (400, "invalid event timestamp".into()))?
            .timestamp();
        let event = ActivityEvent {
            id: None,
            occurred_at: started_at,
            ended_at: Some(ended_at),
            duration_seconds: (incoming.duration_ms / 1000).max(0),
            app_name: "Google Chrome".into(),
            window_title: Some(incoming.title.clone()),
            url: Some(incoming.url.clone()),
            page_title: Some(incoming.title),
            search_query: extract_search_query(&incoming.url),
            browser_profile_id: Some(incoming.browser_profile_id),
            source: ActivitySource::ChromeExtension,
            is_bootstrap: false,
        };
        let excluded = event
            .url
            .as_deref()
            .and_then(|value| Url::parse(value).ok())
            .and_then(|url| url.host_str().map(ToOwned::to_owned))
            .map(|host| {
                settings
                    .excluded_domains
                    .iter()
                    .any(|v| host == *v || host.ends_with(&format!(".{v}")))
            })
            .unwrap_or(false);
        if !excluded {
            db.insert_event(&event, &fingerprint(&event))
                .map_err(|_| (500, "storage error".into()))?;
        }
        // Both stored/deduplicated events and policy-dropped events are terminal.
        accepted.push(incoming_id);
    }
    Ok(accepted)
}

fn handle_native_envelope(db: &Database, envelope: NativeEnvelope) -> serde_json::Value {
    let error = |code: &str, message: &str| {
        serde_json::json!({
            "protocolVersion":1,"requestId":envelope.request_id,
            "ok":false,"errorCode":code,"message":message
        })
    };
    if envelope.protocol_version != 1 {
        return error("protocol", "The extension protocol version is unsupported.");
    }
    if !db
        .authenticate_native_extension(
            &envelope.pairing_token,
            &envelope.extension_id,
            Utc::now().timestamp(),
        )
        .unwrap_or(false)
    {
        return error("authentication", "The pairing token was rejected.");
    }
    let settings = match db.settings() {
        Ok(value) => value,
        Err(_) => return error("internal", "Local settings are unavailable."),
    };
    match envelope.kind.as_str() {
        "status" => serde_json::json!({
            "protocolVersion":1,"requestId":envelope.request_id,"ok":true,
            "collectionEnabled":settings.collection_enabled
        }),
        "events" => {
            if !settings.collection_enabled {
                return error("unavailable", "Collection is paused in the Mac app.");
            }
            let Some(batch) = envelope.payload else {
                return error("protocol", "The event batch is missing.");
            };
            match ingest_extension_batch(db, &settings, batch) {
                Ok(ids) => serde_json::json!({
                    "protocolVersion":1,"requestId":envelope.request_id,
                    "ok":true,"acceptedEventIds":ids
                }),
                Err((_, message)) => error("protocol", &message),
            }
        }
        _ => error("protocol", "Unknown request type."),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeEnvelope {
    protocol_version: u8,
    request_id: String,
    extension_id: String,
    pairing_token: String,
    #[allow(dead_code)]
    sent_at: String,
    #[serde(rename = "type")]
    kind: String,
    payload: Option<ExtensionEventBatch>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionEventBatch {
    protocol_version: u8,
    source: String,
    #[allow(dead_code)]
    extension_id: String,
    #[allow(dead_code)]
    sent_at: String,
    events: Vec<ExtensionBrowserEvent>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionBrowserEvent {
    id: String,
    browser_profile_id: String,
    url: String,
    title: String,
    started_at: String,
    ended_at: String,
    duration_ms: i64,
    incognito: bool,
}

pub fn collection_status(
    db: &Database,
    runtime: &RwLock<RuntimeStatus>,
) -> AppResult<CollectionStatus> {
    let settings = db.settings()?;
    let (_, last_seen_at) = db.extension_state()?.unwrap_or_default();
    let runtime = runtime.read().unwrap_or_else(|e| e.into_inner());
    let now = Utc::now().timestamp();
    Ok(CollectionStatus {
        enabled: settings.collection_enabled,
        accessibility_available: runtime.accessibility_available,
        accessibility_message: runtime.accessibility_message.clone(),
        extension_connected: last_seen_at.map(|v| now - v < 120).unwrap_or(false),
        extension_last_seen_at: last_seen_at,
        data_path: db.path().to_string_lossy().into_owned(),
    })
}

pub fn fingerprint(event: &ActivityEvent) -> String {
    let mut digest = Sha256::new();
    for value in [
        event.occurred_at.to_string(),
        event.app_name.clone(),
        event.window_title.clone().unwrap_or_default(),
        event.url.clone().unwrap_or_default(),
        event.browser_profile_id.clone().unwrap_or_default(),
        event.source.as_str().to_string(),
    ] {
        digest.update(value.as_bytes());
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}

fn extract_search_query(value: &str) -> Option<String> {
    let parsed = Url::parse(value).ok()?;
    let known: HashMap<&str, &str> = [
        ("google.com", "q"),
        ("www.google.com", "q"),
        ("bing.com", "q"),
        ("www.bing.com", "q"),
        ("duckduckgo.com", "q"),
        ("www.youtube.com", "search_query"),
    ]
    .into_iter()
    .collect();
    let key = known.get(parsed.host_str()?)?;
    parsed
        .query_pairs()
        .find(|(name, _)| name == *key)
        .map(|(_, value)| value.into_owned())
}

fn chrome_root() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join("Library/Application Support/Google/Chrome"))
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::HistoryRequest;

    const CHROME_EPOCH_OFFSET_SECONDS: i64 = 11_644_473_600;

    fn create_chrome_history(
        include_duration: bool,
        duration_microseconds: i64,
    ) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("History");
        let connection = Connection::open(history_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE urls(
                    id INTEGER PRIMARY KEY,
                    url TEXT NOT NULL,
                    title TEXT NOT NULL
                 );",
            )
            .unwrap();
        if include_duration {
            connection
                .execute_batch(
                    "CREATE TABLE visits(
                        url INTEGER NOT NULL,
                        visit_time INTEGER NOT NULL,
                        visit_duration INTEGER DEFAULT 0 NOT NULL
                     );",
                )
                .unwrap();
        } else {
            connection
                .execute_batch(
                    "CREATE TABLE visits(
                        url INTEGER NOT NULL,
                        visit_time INTEGER NOT NULL
                     );",
                )
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO urls(id,url,title) VALUES(1,?1,?2)",
                ["https://example.com/work", "Example"],
            )
            .unwrap();
        let visit_time = (1_700_000_000 + CHROME_EPOCH_OFFSET_SECONDS) * 1_000_000;
        if include_duration {
            connection
                .execute(
                    "INSERT INTO visits(url,visit_time,visit_duration) VALUES(1,?1,?2)",
                    rusqlite::params![visit_time, duration_microseconds],
                )
                .unwrap();
        } else {
            connection
                .execute(
                    "INSERT INTO visits(url,visit_time) VALUES(1,?1)",
                    [visit_time],
                )
                .unwrap();
        }
        directory
    }

    fn imported_history(db: &Database) -> Vec<ActivityEvent> {
        db.history(&HistoryRequest {
            start_at: 0,
            end_at: i64::MAX,
            search: None,
            source: Some(ActivitySource::ChromeHistory),
            limit: None,
            offset: None,
        })
        .unwrap()
    }

    #[test]
    fn extracts_only_known_search_parameters() {
        assert_eq!(
            extract_search_query("https://www.google.com/search?q=local+ai"),
            Some("local ai".into())
        );
        assert_eq!(extract_search_query("https://example.com/?q=private"), None);
    }

    #[test]
    fn fingerprints_are_stable_and_field_sensitive() {
        let mut event = ActivityEvent {
            id: None,
            occurred_at: 1,
            ended_at: None,
            duration_seconds: 3,
            app_name: "Code".into(),
            window_title: None,
            url: None,
            page_title: None,
            search_query: None,
            browser_profile_id: None,
            source: ActivitySource::AppFocus,
            is_bootstrap: false,
        };
        let first = fingerprint(&event);
        assert_eq!(first, fingerprint(&event));
        event.window_title = Some("Different document".into());
        assert_ne!(first, fingerprint(&event));
    }

    #[test]
    fn imports_chrome_visit_duration_as_seconds() {
        let history = create_chrome_history(true, 125_900_000);
        let db = Database::in_memory().unwrap();

        let imported =
            import_chrome_history(&db, "Default", &history.path().join("History"), 0).unwrap();

        assert_eq!(imported, 1);
        let events = imported_history(&db);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].duration_seconds, 125);
        assert_eq!(events[0].ended_at, Some(events[0].occurred_at + 125));
    }

    #[test]
    fn discards_implausibly_long_chrome_visit_durations() {
        let history = create_chrome_history(true, (MAX_HISTORICAL_VISIT_SECONDS + 1) * 1_000_000);
        let db = Database::in_memory().unwrap();

        import_chrome_history(&db, "Default", &history.path().join("History"), 0).unwrap();

        let events = imported_history(&db);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].duration_seconds, 0);
        assert_eq!(events[0].ended_at, None);
    }

    #[test]
    fn imports_legacy_chrome_history_without_duration_as_zero() {
        let history = create_chrome_history(false, 0);
        let db = Database::in_memory().unwrap();

        let imported =
            import_chrome_history(&db, "Default", &history.path().join("History"), 0).unwrap();

        assert_eq!(imported, 1);
        let events = imported_history(&db);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].duration_seconds, 0);
        assert_eq!(events[0].ended_at, None);
    }

    #[test]
    fn native_envelope_authenticates_and_ingests_extension_event() {
        let db = Database::in_memory().unwrap();
        db.set_pairing_token("token").unwrap();
        let mut settings = db.settings().unwrap();
        settings.collection_enabled = true;
        settings.selected_chrome_profiles = vec!["Default".into()];
        db.save_settings(&settings).unwrap();
        let id = Uuid::new_v4().to_string();
        let response = handle_native_envelope(
            &db,
            NativeEnvelope {
                protocol_version: 1,
                request_id: "request".into(),
                extension_id: "abcdefghijklmnopabcdefghijklmnop".into(),
                pairing_token: "token".into(),
                sent_at: "2026-01-01T00:00:30Z".into(),
                kind: "events".into(),
                payload: Some(ExtensionEventBatch {
                    protocol_version: 1,
                    source: "chrome_extension".into(),
                    extension_id: "abcdefghijklmnopabcdefghijklmnop".into(),
                    sent_at: "2026-01-01T00:00:30Z".into(),
                    events: vec![ExtensionBrowserEvent {
                        id: id.clone(),
                        browser_profile_id: "Default".into(),
                        url: "https://example.com/page".into(),
                        title: "Example".into(),
                        started_at: "2026-01-01T00:00:00Z".into(),
                        ended_at: "2026-01-01T00:00:30Z".into(),
                        duration_ms: 30_000,
                        incognito: false,
                    }],
                }),
            },
        );
        assert_eq!(response["ok"], true);
        assert_eq!(response["acceptedEventIds"][0], id);
        let stored = db
            .history(&crate::models::HistoryRequest {
                start_at: 0,
                end_at: i64::MAX,
                search: None,
                source: Some(ActivitySource::ChromeExtension),
                limit: None,
                offset: None,
            })
            .unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].browser_profile_id.as_deref(), Some("Default"));
    }
}
