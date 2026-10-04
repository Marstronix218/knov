use std::collections::{BTreeMap, HashSet};

use chrono::{Datelike, Local, Timelike, Utc};
use url::Url;
use uuid::Uuid;

mod evaluator;
mod models;
mod store;
use evaluator::evaluate_prediction;
use models::{
    CurrentWorkState, HistoricalExample, PredictionResource, ProviderPrediction,
    ProviderPredictions, StateEvent,
};
pub use models::{PredictionDashboard, PredictionHistoryItem, WorkPrediction};

use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{ActivityEvent, HistoryRequest, Settings},
    providers::ProviderClient,
    threading::semantic_topics,
};

pub const DEFAULT_HORIZON_MINUTES: i64 = 20;
pub const MATCHED_THRESHOLD: f64 = 0.75;
pub const PARTIAL_THRESHOLD: f64 = 0.40;
const CURRENT_WINDOW_SECONDS: i64 = 60 * 60;
const MIN_CONTEXT_EVENTS: usize = 3;
const HISTORY_LOOKBACK_SECONDS: i64 = 30 * 86_400;
const HISTORY_EXAMPLE_LIMIT: usize = 5;

pub async fn generate_prediction_set(
    db: &Database,
    provider_client: &ProviderClient,
    settings: &Settings,
) -> AppResult<Vec<WorkPrediction>> {
    if !settings.prediction_experiment_enabled {
        return Err(AppError::InvalidInput(
            "The prediction experiment is disabled.".into(),
        ));
    }
    if !settings.collection_enabled {
        return Err(AppError::InvalidInput(
            "Activity collection must be enabled for predictions.".into(),
        ));
    }
    let now = Utc::now().timestamp();
    let state = build_current_state(db, settings, now)?;
    if state.recent_events.len() < MIN_CONTEXT_EVENTS {
        return Err(AppError::InvalidInput(
            "More recent work activity is needed before predicting.".into(),
        ));
    }
    let history = historical_examples(db, settings, &state, now)?;
    let state_summary = serde_json::to_string(&state)?;
    let batch_id = Uuid::new_v4().to_string();
    // Agent context is optional: a failure here must not block predictions.
    let agent = crate::agent::prediction_context(db, settings, now).ok();
    let goal = agent.as_ref().and_then(|context| context.goal.clone());
    let mut baseline = baseline_prediction(&state, &batch_id);
    baseline.goal = goal.clone();
    db.insert_prediction(&baseline, &batch_id, 1, &state_summary)?;
    let mut local = vec![baseline];
    if let Some(candidate) = agent
        .as_ref()
        .and_then(|context| workflow_candidate(context, goal.clone(), now))
    {
        db.insert_prediction(&candidate, &batch_id, 1, &state_summary)?;
        local.push(candidate);
    }

    let Some(provider) = settings.selected_provider.as_deref() else {
        return Ok(local);
    };
    if !provider_client.has_key(provider) {
        return Ok(local);
    }

    let raw = provider_client
        .predict_work(provider, &state, &history)
        .await;
    let Ok(raw) = raw else {
        return Ok(local);
    };
    let Ok(decoded) = serde_json::from_value::<ProviderPredictions>(raw) else {
        return Ok(local);
    };
    let allowed_threads = state
        .active_thread_id
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let mut generated = Vec::new();
    for (rank, candidate) in decoded.predictions.into_iter().take(3).enumerate() {
        if let Some(mut prediction) =
            normalize_provider_prediction(candidate, now, &allowed_threads, &state_summary)
        {
            prediction.goal = goal.clone();
            db.insert_prediction(&prediction, &batch_id, rank as i64 + 1, &state_summary)?;
            generated.push(prediction);
        }
    }
    if generated.is_empty() {
        Ok(local)
    } else {
        generated.extend(
            local
                .into_iter()
                .filter(|prediction| prediction.source == "workflow"),
        );
        Ok(generated)
    }
}

/// Grounded next-step candidate from a repeated workflow the user is part-way
/// through. Its confidence is the workflow's observed completion rate,
/// adjusted for how far along the user is.
fn workflow_candidate(
    context: &crate::agent::PredictionContext,
    goal: Option<String>,
    now: i64,
) -> Option<WorkPrediction> {
    let progress = context.progress.as_ref()?;
    let step = &progress.next_step;
    let next_resource = match step.kind.as_str() {
        "app" => Some(PredictionResource {
            resource_type: "application".into(),
            label: sanitize_text(&step.label, 160),
            safe_locator: None,
        }),
        "web" => step
            .key
            .strip_prefix("web:")
            .map(|rest| PredictionResource {
                resource_type: "domain".into(),
                label: rest.split('/').next().unwrap_or(rest).to_string(),
                safe_locator: step.resource.as_deref().and_then(safe_locator),
            }),
        _ => None,
    };
    Some(WorkPrediction {
        id: Uuid::new_v4().to_string(),
        created_at: now,
        source: "workflow".into(),
        intent: sanitize_text(&format!("Continue “{}”", progress.title), 240),
        next_action: sanitize_text(&step.title, 300),
        next_resource,
        thread_id: context.workflow_thread.as_deref().map(stable_thread_id),
        confidence: progress.confidence.clamp(0.0, 1.0),
        horizon_minutes: DEFAULT_HORIZON_MINUTES,
        reasoning_summary: format!(
            "You have done {} of {} steps of a workflow you repeat, and it usually continues this way.",
            progress.matched_steps, progress.total_steps
        ),
        evidence: vec![
            sanitize_text(
                &format!(
                    "Matched the first {} steps of “{}”",
                    progress.matched_steps, progress.title
                ),
                180,
            ),
            format!(
                "This workflow is usually finished once started ({:.0}% confidence)",
                progress.confidence * 100.0
            ),
        ],
        evaluation_status: "pending".into(),
        expires_at: now + DEFAULT_HORIZON_MINUTES * 60,
        match_score: None,
        user_feedback: None,
        goal,
        workflow_id: Some(progress.workflow_id.clone()),
    })
}

pub fn evaluate_due_predictions(db: &Database, settings: &Settings, now: i64) -> AppResult<usize> {
    let pending = db.pending_predictions(now)?;
    let mut count = 0;
    for item in pending {
        let events = filtered_history(
            db,
            settings,
            item.prediction.created_at,
            item.prediction.expires_at,
            1000,
        )?;
        let (score, outcome) = evaluate_prediction(&item.prediction, &events);
        let explicit = matches!(
            item.prediction.user_feedback.as_deref(),
            Some("correct" | "incorrect")
        );
        let status = if events.is_empty() && !explicit {
            "expired"
        } else if score >= MATCHED_THRESHOLD {
            "matched"
        } else if score >= PARTIAL_THRESHOLD {
            "partial"
        } else {
            "missed"
        };
        db.finish_prediction_evaluation(&item.prediction.id, status, score, &outcome, now)?;
        count += 1;
    }
    Ok(count)
}

pub fn dashboard(db: &Database, settings: &Settings, now: i64) -> AppResult<PredictionDashboard> {
    let predictions = if settings.prediction_experiment_enabled {
        db.visible_predictions(now, settings.prediction_display_threshold)?
    } else {
        Vec::new()
    };
    Ok(PredictionDashboard {
        enabled: settings.prediction_experiment_enabled,
        predictions,
        stats: db.prediction_stats()?,
    })
}

fn build_current_state(
    db: &Database,
    settings: &Settings,
    now: i64,
) -> AppResult<CurrentWorkState> {
    let events = filtered_history(db, settings, now - CURRENT_WINDOW_SECONDS, now, 500)?;
    let topics = semantic_topics(&events);
    let recent_events = events
        .iter()
        .take(30)
        .map(|event| StateEvent {
            occurred_at: event.occurred_at,
            app: sanitize_text(&event.app_name, 100),
            title: event
                .page_title
                .as_deref()
                .or(event.window_title.as_deref())
                .map(|value| sanitize_text(value, 160))
                .filter(|value| !value.is_empty()),
            domain: event.url.as_deref().and_then(safe_domain),
            thread: event
                .id
                .and_then(|id| topics.get(&id))
                .map(|value| sanitize_text(value, 120)),
            duration_seconds: event.duration_seconds.max(0),
        })
        .collect::<Vec<_>>();
    let active = recent_events.first();
    let active_thread_title = active.and_then(|event| event.thread.clone());
    let active_thread_id = active_thread_title.as_deref().map(stable_thread_id);
    let recent_apps = ranked_values(recent_events.iter().map(|event| event.app.as_str()), 8);
    let recent_domains = ranked_values(
        recent_events
            .iter()
            .filter_map(|event| event.domain.as_deref()),
        8,
    );
    let local = Local::now();
    Ok(CurrentWorkState {
        generated_at: now,
        active_app: active.map(|event| event.app.clone()).unwrap_or_default(),
        active_window_title: active.and_then(|event| event.title.clone()),
        active_domain: active.and_then(|event| event.domain.clone()),
        active_thread_id,
        active_thread_title,
        session_duration_seconds: active.map_or(0, |event| event.duration_seconds),
        recent_events,
        recent_apps,
        recent_domains,
        time_of_day: match local.hour() {
            5..=11 => "morning",
            12..=16 => "afternoon",
            17..=21 => "evening",
            _ => "night",
        }
        .into(),
        day_of_week: local.weekday().to_string(),
    })
}

fn historical_examples(
    db: &Database,
    settings: &Settings,
    state: &CurrentWorkState,
    now: i64,
) -> AppResult<Vec<HistoricalExample>> {
    let historical = filtered_history(
        db,
        settings,
        now - HISTORY_LOOKBACK_SECONDS,
        now - CURRENT_WINDOW_SECONDS,
        1000,
    )?;
    let topics = semantic_topics(&historical);
    let mut scored = historical
        .windows(3)
        .filter_map(|window| {
            let anchor = &window[0];
            let domain = anchor.url.as_deref().and_then(safe_domain);
            let thread = anchor.id.and_then(|id| topics.get(&id)).cloned();
            let mut shared = Vec::new();
            if anchor.app_name.eq_ignore_ascii_case(&state.active_app) {
                shared.push(format!(
                    "same application: {}",
                    sanitize_text(&anchor.app_name, 80)
                ));
            }
            if domain.as_deref() == state.active_domain.as_deref() && domain.is_some() {
                shared.push(format!(
                    "same domain: {}",
                    domain.as_deref().unwrap_or_default()
                ));
            }
            if thread.as_deref() == state.active_thread_title.as_deref() && thread.is_some() {
                shared.push(format!(
                    "same thread: {}",
                    sanitize_text(thread.as_deref().unwrap_or_default(), 100)
                ));
            }
            if shared.is_empty() {
                return None;
            }
            let sequence = window.iter().rev().map(event_label).collect::<Vec<_>>();
            Some((
                shared.len(),
                anchor.occurred_at,
                HistoricalExample {
                    observed_at: anchor.occurred_at,
                    sequence,
                    shared_signals: shared,
                },
            ))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    Ok(scored
        .into_iter()
        .take(HISTORY_EXAMPLE_LIMIT)
        .map(|(_, _, example)| example)
        .collect())
}

fn filtered_history(
    db: &Database,
    settings: &Settings,
    start_at: i64,
    end_at: i64,
    limit: u32,
) -> AppResult<Vec<ActivityEvent>> {
    let excluded_apps = settings
        .excluded_apps
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let excluded_domains = settings
        .excluded_domains
        .iter()
        .map(|value| value.trim().trim_start_matches("www.").to_ascii_lowercase())
        .collect::<HashSet<_>>();
    Ok(db
        .history(&HistoryRequest {
            start_at,
            end_at,
            search: None,
            source: None,
            limit: Some(limit),
            offset: None,
        })?
        .into_iter()
        .filter(|event| !excluded_apps.contains(&event.app_name.trim().to_ascii_lowercase()))
        .filter(|event| {
            event
                .url
                .as_deref()
                .and_then(safe_domain)
                .is_none_or(|domain| !excluded_domains.contains(&domain))
        })
        .collect())
}

fn baseline_prediction(state: &CurrentWorkState, batch_id: &str) -> WorkPrediction {
    let thread = state.active_thread_title.as_deref();
    let intent = thread
        .map(|value| format!("Continue {value}"))
        .unwrap_or_else(|| format!("Continue work in {}", state.active_app));
    let next_resource = state
        .active_domain
        .as_ref()
        .map(|domain| PredictionResource {
            resource_type: "domain".into(),
            label: domain.clone(),
            safe_locator: Some(format!("https://{domain}")),
        })
        .or_else(|| {
            (!state.active_app.is_empty()).then(|| PredictionResource {
                resource_type: "application".into(),
                label: state.active_app.clone(),
                safe_locator: None,
            })
        });
    WorkPrediction {
        id: format!("{batch_id}-baseline"),
        created_at: state.generated_at,
        source: "heuristic".into(),
        intent,
        next_action: thread
            .map(|value| format!("Resume the recent {value} work"))
            .unwrap_or_else(|| format!("Resume the recent {} session", state.active_app)),
        next_resource,
        thread_id: state.active_thread_id.clone(),
        confidence: if thread.is_some() { 0.58 } else { 0.45 },
        horizon_minutes: DEFAULT_HORIZON_MINUTES,
        reasoning_summary: "Recent work continuity is the strongest local signal.".into(),
        evidence: state
            .recent_events
            .iter()
            .take(3)
            .map(|event| format!("Recent activity in {}", event.app))
            .collect(),
        evaluation_status: "pending".into(),
        expires_at: state.generated_at + DEFAULT_HORIZON_MINUTES * 60,
        match_score: None,
        user_feedback: None,
        goal: None,
        workflow_id: None,
    }
}

fn normalize_provider_prediction(
    value: ProviderPrediction,
    now: i64,
    allowed_threads: &HashSet<String>,
    sanitized_state_summary: &str,
) -> Option<WorkPrediction> {
    let intent = sanitize_text(&value.intent, 240);
    let next_action = sanitize_text(&value.next_action, 300);
    if intent.is_empty() || next_action.is_empty() || !value.confidence.is_finite() {
        return None;
    }
    let horizon_minutes = value.horizon_minutes.clamp(5, 120);
    let next_resource = value.next_resource.and_then(|resource| {
        let resource_type = resource.resource_type.to_ascii_lowercase();
        let allowed = [
            "thread",
            "url",
            "domain",
            "application",
            "document",
            "repository",
            "unknown",
        ];
        if !allowed.contains(&resource_type.as_str()) {
            return None;
        }
        let label = sanitize_text(&resource.label, 160);
        if label.is_empty() {
            return None;
        }
        let supported = match resource_type.as_str() {
            "application" | "domain" | "url" | "document" | "repository" => sanitized_state_summary
                .to_ascii_lowercase()
                .contains(&label.to_ascii_lowercase()),
            "thread" => allowed_threads.contains(&stable_thread_id(&label)),
            "unknown" => true,
            _ => false,
        };
        if !supported {
            return None;
        }
        let safe_locator = resource.safe_locator.as_deref().and_then(safe_locator);
        Some(PredictionResource {
            resource_type,
            label,
            safe_locator,
        })
    });
    Some(WorkPrediction {
        id: Uuid::new_v4().to_string(),
        created_at: now,
        source: "provider".into(),
        intent,
        next_action,
        next_resource,
        thread_id: value.thread_id.filter(|id| allowed_threads.contains(id)),
        confidence: value.confidence.clamp(0.0, 1.0),
        horizon_minutes,
        reasoning_summary: sanitize_text(&value.reasoning_summary, 300),
        evidence: value
            .evidence
            .into_iter()
            .map(|item| sanitize_text(&item, 180))
            .filter(|item| !item.is_empty())
            .take(5)
            .collect(),
        evaluation_status: "pending".into(),
        expires_at: now + horizon_minutes * 60,
        match_score: None,
        user_feedback: None,
        goal: None,
        workflow_id: None,
    })
}

fn event_label(event: &ActivityEvent) -> String {
    let app = sanitize_text(&event.app_name, 80);
    event
        .url
        .as_deref()
        .and_then(safe_domain)
        .map_or(app.clone(), |domain| format!("{app} ({domain})"))
}

pub(crate) fn safe_domain(value: &str) -> Option<String> {
    let parsed = Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https")
        .then(|| {
            parsed
                .host_str()
                .map(|host| host.trim_start_matches("www.").to_ascii_lowercase())
        })
        .flatten()
}

pub(crate) fn safe_locator(value: &str) -> Option<String> {
    let parsed = Url::parse(value.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return None;
    }
    let host = parsed.host_str()?.trim_start_matches("www.");
    Some(format!("{}://{}{}", parsed.scheme(), host, parsed.path()))
}

pub(crate) fn sanitize_text(value: &str, max_chars: usize) -> String {
    let credential_markers = [
        "authorization:",
        "bearer",
        "api_key",
        "apikey",
        "password=",
        "token=",
        "secret=",
    ];
    let mut result = Vec::<String>::new();
    let mut redact_next = false;
    for token in value.split_whitespace() {
        let trimmed = token.trim_matches(|c: char| {
            matches!(
                c,
                '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
        });
        let lower = trimmed.to_ascii_lowercase();
        let windows_path = trimmed.as_bytes().get(1) == Some(&b':')
            && matches!(trimmed.as_bytes().get(2), Some(b'/') | Some(b'\\'));
        if redact_next {
            result.push("[redacted]".into());
            redact_next = matches!(lower.as_str(), "authorization:" | "bearer");
        } else if credential_markers
            .iter()
            .any(|marker| lower.contains(marker))
        {
            result.push("[redacted]".into());
            redact_next = matches!(
                lower.as_str(),
                "authorization:"
                    | "bearer"
                    | "api_key"
                    | "apikey"
                    | "password"
                    | "token"
                    | "secret"
            );
        } else if lower.starts_with("file://")
            || lower.starts_with("~/")
            || lower.starts_with("/users/")
            || lower.starts_with("/home/")
            || lower.starts_with("/private/")
            || lower.starts_with("/tmp/")
            || windows_path
        {
            result.push("[local-path-redacted]".into());
        } else if lower.starts_with("http://") || lower.starts_with("https://") {
            if let Some(domain) = safe_domain(trimmed.trim_end_matches(['.', ',', ')', ']'])) {
                result.push(domain);
            }
        } else {
            result.push(token.into());
        }
    }
    result
        .join(" ")
        .chars()
        .filter(|character| !character.is_control())
        .take(max_chars)
        .collect()
}

pub(crate) fn stable_thread_id(value: &str) -> String {
    let normalized = value
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if normalized.is_empty() {
        "untitled".into()
    } else {
        normalized
    }
}

fn ranked_values<'a>(values: impl Iterator<Item = &'a str>, limit: usize) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values {
        *counts.entry(value.to_string()).or_default() += 1;
    }
    let mut values = counts.into_iter().collect::<Vec<_>>();
    values.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    values
        .into_iter()
        .take(limit)
        .map(|(value, _)| value)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ActivitySource;

    fn event(at: i64, app: &str, title: &str, url: Option<&str>) -> ActivityEvent {
        ActivityEvent {
            id: None,
            occurred_at: at,
            ended_at: Some(at + 60),
            duration_seconds: 60,
            app_name: app.into(),
            window_title: Some(title.into()),
            url: url.map(str::to_string),
            page_title: None,
            search_query: None,
            browser_profile_id: None,
            source: ActivitySource::AppFocus,
            is_bootstrap: false,
        }
    }
    fn insert(db: &Database, event: ActivityEvent, index: usize) {
        db.insert_event(&event, &format!("prediction-{index}"))
            .unwrap();
    }

    #[test]
    fn state_excludes_configured_activity_and_sanitizes_paths_and_credentials() {
        let db = Database::in_memory().unwrap();
        let now = 1_800_000_000;
        insert(
            &db,
            event(
                now - 60,
                "Code",
                "Editing /Users/me/secret/main.rs token=hunter Authorization: Bearer topsecret",
                None,
            ),
            1,
        );
        insert(
            &db,
            event(
                now - 120,
                "Bank",
                "Account",
                Some("https://bank.example/private"),
            ),
            2,
        );
        insert(&db, event(now - 180, "Terminal", "cargo test", None), 3);
        let settings = Settings {
            excluded_domains: vec!["bank.example".into()],
            ..Settings::default()
        };
        let state = build_current_state(&db, &settings, now).unwrap();
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("bank.example"));
        assert!(!json.contains("/Users/me"));
        assert!(!json.contains("hunter"));
        assert!(!json.contains("topsecret"));
        assert!(json.contains("local-path-redacted"));
        assert!(json.contains("redacted"));
    }

    #[test]
    fn baseline_and_evaluator_are_deterministic() {
        let state = CurrentWorkState {
            generated_at: 100,
            active_app: "Code".into(),
            active_window_title: None,
            active_domain: Some("github.com".into()),
            active_thread_id: Some("knov".into()),
            active_thread_title: Some("Knov".into()),
            recent_events: vec![],
            recent_apps: vec![],
            recent_domains: vec![],
            session_duration_seconds: 60,
            time_of_day: "morning".into(),
            day_of_week: "Mon".into(),
        };
        let prediction = baseline_prediction(&state, "batch");
        assert_eq!(prediction.confidence, 0.58);
        let events = vec![event(
            101,
            "Chrome",
            "Knov",
            Some("https://github.com/project"),
        )];
        let (score, _) = evaluate_prediction(&prediction, &events);
        assert!(score >= MATCHED_THRESHOLD);
    }

    #[test]
    fn persistence_feedback_stats_and_deletion_include_predictions() {
        let db = Database::in_memory().unwrap();
        let state = CurrentWorkState {
            generated_at: 100,
            active_app: "Code".into(),
            active_window_title: None,
            active_domain: None,
            active_thread_id: None,
            active_thread_title: None,
            recent_events: vec![],
            recent_apps: vec![],
            recent_domains: vec![],
            session_duration_seconds: 0,
            time_of_day: "night".into(),
            day_of_week: "Fri".into(),
        };
        let prediction = baseline_prediction(&state, "batch");
        db.insert_prediction(&prediction, "batch", 1, "{}").unwrap();
        assert_eq!(db.prediction_history(10).unwrap().len(), 1);
        assert!(db
            .record_prediction_feedback(&prediction.id, "correct", None)
            .unwrap());
        assert_eq!(db.prediction_stats().unwrap().total_predictions, 1);
        db.delete_all_local_data().unwrap();
        assert!(db.prediction_history(10).unwrap().is_empty());
    }

    fn provider_candidate(id: &str, created_at: i64) -> WorkPrediction {
        WorkPrediction {
            id: id.into(),
            created_at,
            source: "provider".into(),
            intent: "Review pull request".into(),
            next_action: "Open the review".into(),
            next_resource: None,
            thread_id: None,
            confidence: 0.9,
            horizon_minutes: DEFAULT_HORIZON_MINUTES,
            reasoning_summary: "Recent review activity.".into(),
            evidence: vec![],
            evaluation_status: "pending".into(),
            expires_at: created_at + DEFAULT_HORIZON_MINUTES * 60,
            match_score: None,
            user_feedback: None,
            goal: None,
            workflow_id: None,
        }
    }

    #[test]
    fn dismissed_and_incorrect_predictions_stay_hidden_after_reload() {
        let db = Database::in_memory().unwrap();
        let now = 1_800_000_000;
        for (rank, id) in ["keep", "correct", "dismissed", "incorrect"]
            .iter()
            .enumerate()
        {
            db.insert_prediction(&provider_candidate(id, now), "batch", rank as i64 + 1, "{}")
                .unwrap();
        }
        for feedback in ["correct", "dismissed", "incorrect"] {
            assert!(db
                .record_prediction_feedback(feedback, feedback, None)
                .unwrap());
        }
        let mut visible = db
            .visible_predictions(now + 60, 0.65)
            .unwrap()
            .into_iter()
            .map(|prediction| prediction.id)
            .collect::<Vec<_>>();
        visible.sort();
        assert_eq!(visible, vec!["correct", "keep"]);
    }

    #[test]
    fn explicit_feedback_is_scored_even_without_later_activity() {
        let db = Database::in_memory().unwrap();
        let now = 1_800_000_000;
        for (rank, id) in ["correct", "incorrect", "silent"].iter().enumerate() {
            db.insert_prediction(&provider_candidate(id, now), "batch", rank as i64 + 1, "{}")
                .unwrap();
        }
        db.record_prediction_feedback("correct", "correct", None)
            .unwrap();
        db.record_prediction_feedback("incorrect", "incorrect", None)
            .unwrap();
        let later = now + DEFAULT_HORIZON_MINUTES * 60 + 1;
        assert_eq!(
            evaluate_due_predictions(&db, &Settings::default(), later).unwrap(),
            3
        );
        let statuses = db
            .prediction_history(10)
            .unwrap()
            .into_iter()
            .map(|item| (item.prediction.id, item.prediction.evaluation_status))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(statuses["correct"], "matched");
        assert_eq!(statuses["incorrect"], "missed");
        assert_eq!(statuses["silent"], "expired");
    }

    #[test]
    fn workflow_candidates_are_grounded_visible_and_calibrated() {
        let db = Database::in_memory().unwrap();
        let context = crate::agent::PredictionContext {
            goal: Some("Build Knov".into()),
            progress: Some(crate::agent::WorkflowProgress {
                workflow_id: "wf-1".into(),
                title: "Build and test · Knov".into(),
                matched_steps: 2,
                total_steps: 3,
                next_step: crate::agent::WorkflowStep {
                    key: "app:terminal".into(),
                    kind: "app".into(),
                    category: "terminal".into(),
                    label: "Terminal".into(),
                    title: "Run commands in Terminal".into(),
                    resource: Some("Terminal".into()),
                    average_seconds: 120,
                },
                confidence: 0.8,
                skill_id: None,
            }),
            workflow_thread: Some("Knov".into()),
        };
        let candidate = workflow_candidate(&context, context.goal.clone(), 1_000).unwrap();
        assert_eq!(candidate.source, "workflow");
        assert_eq!(candidate.goal.as_deref(), Some("Build Knov"));
        assert_eq!(candidate.thread_id.as_deref(), Some("knov"));
        let resource = candidate.next_resource.clone().unwrap();
        assert_eq!(
            (resource.resource_type.as_str(), resource.label.as_str()),
            ("application", "Terminal")
        );

        db.insert_prediction(&candidate, "batch", 1, "{}").unwrap();
        let visible = db.visible_predictions(1_001, 0.65).unwrap();
        assert_eq!(visible[0].workflow_id.as_deref(), Some("wf-1"));

        let (score, _) = evaluate_prediction(
            &candidate,
            &[event(1_100, "Terminal", "Run commands", None)],
        );
        assert!(score >= MATCHED_THRESHOLD);
        db.finish_prediction_evaluation(&candidate.id, "matched", score, "observed", 2_000)
            .unwrap();
        let stats = db.prediction_stats().unwrap();
        assert_eq!(stats.workflow_top1_accuracy, Some(1.0));
        let top_bin = stats
            .calibration
            .iter()
            .find(|bin| bin.label == "80% and above")
            .unwrap();
        assert_eq!((top_bin.count, top_bin.observed_accuracy), (1, Some(1.0)));
        assert_eq!(
            stats.calibration.iter().map(|bin| bin.count).sum::<i64>(),
            1
        );
    }

    #[test]
    fn provider_output_is_bounded_and_rejects_unsupported_thread_and_locator() {
        let value = ProviderPrediction {
            intent: "Inspect test".into(),
            next_action: "Fix it".into(),
            next_resource: Some(PredictionResource {
                resource_type: "url".into(),
                label: "repo".into(),
                safe_locator: Some("file:///Users/me/repo".into()),
            }),
            thread_id: Some("invented".into()),
            confidence: 4.0,
            horizon_minutes: 999,
            reasoning_summary: "evidence".into(),
            evidence: vec![],
        };
        let prediction = normalize_provider_prediction(
            value,
            100,
            &HashSet::new(),
            r#"{"recentDomains":["repo"]}"#,
        )
        .unwrap();
        assert_eq!(prediction.confidence, 1.0);
        assert_eq!(prediction.horizon_minutes, 120);
        assert_eq!(prediction.thread_id, None);
        assert_eq!(prediction.next_resource.unwrap().safe_locator, None);
    }
}
