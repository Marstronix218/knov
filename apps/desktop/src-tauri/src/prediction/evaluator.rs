use crate::{models::ActivityEvent, threading::semantic_topics};

use super::{models::WorkPrediction, safe_domain, stable_thread_id};

pub(super) fn evaluate_prediction(
    prediction: &WorkPrediction,
    events: &[ActivityEvent],
) -> (f64, String) {
    if prediction.user_feedback.as_deref() == Some("correct") {
        return (
            1.0,
            "User explicitly marked this prediction correct.".into(),
        );
    }
    if prediction.user_feedback.as_deref() == Some("incorrect") {
        return (
            0.0,
            "User explicitly marked this prediction incorrect.".into(),
        );
    }
    let topics = semantic_topics(events);
    let mut signals = Vec::new();
    if let Some(resource) = &prediction.next_resource {
        let expected = resource.label.to_ascii_lowercase();
        let resource_match = events
            .iter()
            .any(|event| match resource.resource_type.as_str() {
                "application" => event.app_name.eq_ignore_ascii_case(&resource.label),
                "domain" | "url" => event
                    .url
                    .as_deref()
                    .and_then(safe_domain)
                    .is_some_and(|domain| domain.contains(&expected)),
                _ => event_search_text(event).contains(&expected),
            });
        if resource_match {
            signals.push((
                0.55,
                format!("Observed predicted resource: {}", resource.label),
            ));
        }
    }
    if let Some(thread_id) = &prediction.thread_id {
        if topics
            .values()
            .any(|title| stable_thread_id(title) == *thread_id)
        {
            signals.push((0.45, "Observed predicted work thread.".into()));
        }
    }
    let action_terms = meaningful_terms(&prediction.next_action);
    let overlap = events
        .iter()
        .map(|event| {
            let text = event_search_text(event);
            action_terms
                .iter()
                .filter(|term| text.contains(*term))
                .count()
        })
        .max()
        .unwrap_or(0);
    if !action_terms.is_empty() && overlap > 0 {
        signals.push((
            (overlap as f64 / action_terms.len() as f64).min(1.0) * 0.35,
            "Observed metadata overlapped with the predicted action.".into(),
        ));
    }
    let score = signals
        .iter()
        .map(|(weight, _)| *weight)
        .sum::<f64>()
        .min(1.0);
    let outcome = if signals.is_empty() {
        "No deterministic matching signal was observed during the prediction horizon.".into()
    } else {
        signals
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join(" ")
    };
    (score, outcome)
}

fn meaningful_terms(value: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "and", "with", "this", "that", "continue", "resume", "work", "recent",
    ];
    value
        .split(|character: char| !character.is_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|term| term.len() >= 4 && !STOP.contains(&term.as_str()))
        .collect()
}

fn event_search_text(event: &ActivityEvent) -> String {
    format!(
        "{} {} {} {} {}",
        event.app_name,
        event.window_title.as_deref().unwrap_or_default(),
        event.page_title.as_deref().unwrap_or_default(),
        event.url.as_deref().unwrap_or_default(),
        event.search_query.as_deref().unwrap_or_default()
    )
    .to_ascii_lowercase()
}
