//! State engine helpers: durable goal inference and in-progress workflow
//! matching. Goals are provisional claims the user can confirm, rename,
//! complete, or dismiss.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Local, TimeZone};

use super::{
    models::{Goal, Workflow, WorkflowProgress},
    normalize::SemanticStep,
};
use crate::prediction::{sanitize_text, stable_thread_id};

pub(crate) const GOAL_LOOKBACK_DAYS: i64 = 14;
const MIN_GOAL_DAYS: usize = 2;
const MIN_GOAL_SECONDS: i64 = 20 * 60;
const MIN_GOAL_SIGNALS: usize = 8;
const MAX_GOALS: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GoalReview {
    pub status: String,
    pub user_title: Option<String>,
}

#[derive(Default)]
struct TopicStats<'a> {
    days: HashSet<String>,
    seconds: i64,
    signals: usize,
    last_active: i64,
    categories: BTreeMap<&'a str, i64>,
    labels: BTreeMap<&'a str, i64>,
}

pub(crate) fn goal_id(topic: &str) -> String {
    format!("goal-{}", stable_thread_id(topic))
}

pub(crate) fn infer_goals(
    steps: &[SemanticStep],
    reviews: &HashMap<String, GoalReview>,
) -> Vec<Goal> {
    let mut topics = BTreeMap::<&str, TopicStats>::new();
    for step in steps {
        let Some(topic) = step.thread.as_deref() else {
            continue;
        };
        let stats = topics.entry(topic).or_default();
        if let Some(day) = Local.timestamp_opt(step.started_at, 0).single() {
            stats.days.insert(day.format("%Y-%m-%d").to_string());
        }
        stats.seconds += step.seconds;
        stats.signals += 1;
        stats.last_active = stats.last_active.max(step.ended_at);
        *stats.categories.entry(step.category).or_default() += step.seconds.max(30);
        *stats.labels.entry(step.label.as_str()).or_default() += step.seconds.max(30);
    }

    let mut goals = topics
        .into_iter()
        .filter_map(|(topic, stats)| {
            let id = goal_id(topic);
            let review = reviews.get(&id);
            if review
                .is_some_and(|review| matches!(review.status.as_str(), "dismissed" | "completed"))
            {
                return None;
            }
            let confirmed = review.is_some_and(|review| review.status == "confirmed");
            let persistent = stats.days.len() >= MIN_GOAL_DAYS
                && (stats.seconds >= MIN_GOAL_SECONDS || stats.signals >= MIN_GOAL_SIGNALS);
            if !persistent && !confirmed {
                return None;
            }
            let dominant_category = stats
                .categories
                .iter()
                .max_by_key(|(_, weight)| **weight)
                .map(|(category, _)| *category)
                .unwrap_or("other");
            let topic_label = sanitize_text(topic, 80);
            let title = review
                .and_then(|review| review.user_title.clone())
                .unwrap_or_else(|| format!("{} {topic_label}", goal_verb(dominant_category)));
            let confidence = if confirmed {
                1.0
            } else {
                (0.35
                    + 0.08 * stats.days.len() as f64
                    + if stats.categories.len() >= 2 {
                        0.1
                    } else {
                        0.0
                    }
                    + if stats.seconds >= 3_600 { 0.1 } else { 0.0 })
                .min(0.95)
            };
            let mut labels = stats.labels.iter().collect::<Vec<_>>();
            labels.sort_by(|left, right| right.1.cmp(left.1).then_with(|| left.0.cmp(right.0)));
            let top_labels = labels
                .iter()
                .take(3)
                .map(|(label, _)| sanitize_text(label, 40))
                .collect::<Vec<_>>()
                .join(", ");
            Some(Goal {
                id,
                title,
                topic: topic_label,
                status: if confirmed { "confirmed" } else { "inferred" }.into(),
                confidence: (confidence * 100.0).round() / 100.0,
                evidence: vec![
                    format!(
                        "Active on {} of the last {GOAL_LOOKBACK_DAYS} days",
                        stats.days.len()
                    ),
                    format!(
                        "{} signals, {} observed, mostly in {top_labels}",
                        stats.signals,
                        human_duration(stats.seconds)
                    ),
                    format!("Work looks like {}", category_phrase(dominant_category)),
                ],
                active_days: stats.days.len() as i64,
                observed_seconds: stats.seconds,
                last_active_at: stats.last_active,
            })
        })
        .collect::<Vec<_>>();
    goals.sort_by(|left, right| {
        (right.status == "confirmed")
            .cmp(&(left.status == "confirmed"))
            .then_with(|| right.last_active_at.cmp(&left.last_active_at))
            .then_with(|| right.active_days.cmp(&left.active_days))
    });
    goals.truncate(MAX_GOALS);
    goals
}

fn goal_verb(category: &str) -> &'static str {
    match category {
        "code" | "terminal" | "code-hosting" => "Build",
        "docs" | "search" | "video" | "ai" => "Research",
        "notes" => "Write about",
        "spreadsheet" => "Analyze",
        "communication" | "email" => "Coordinate",
        "design" => "Design",
        "project" => "Plan",
        _ => "Work on",
    }
}

fn category_phrase(category: &str) -> &'static str {
    match category {
        "code" | "terminal" | "code-hosting" => "building software",
        "docs" | "search" | "video" | "ai" => "research and learning",
        "notes" => "writing",
        "spreadsheet" => "analysis",
        "communication" | "email" => "coordination",
        "design" => "design",
        "project" => "planning",
        _ => "general work",
    }
}

fn human_duration(seconds: i64) -> String {
    let minutes = seconds / 60;
    if minutes >= 60 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

/// Matches the end of the current session against the beginning of a known
/// workflow. Confidence is grounded in how often the workflow is finished once
/// started and grows with the length of the match.
pub(crate) fn match_progress(
    recent: &[SemanticStep],
    workflows: &[Workflow],
) -> Option<WorkflowProgress> {
    let mut best: Option<(f64, usize, WorkflowProgress)> = None;
    for workflow in workflows
        .iter()
        .filter(|workflow| workflow.status != "dismissed")
    {
        let total = workflow.steps.len();
        for matched in (2..total).rev() {
            if recent.len() < matched {
                continue;
            }
            let suffix = &recent[recent.len() - matched..];
            let aligned = suffix
                .iter()
                .zip(&workflow.steps[..matched])
                .all(|(step, expected)| step.key == expected.key);
            if !aligned {
                continue;
            }
            let completion = workflow.stats.completion_rate.clamp(0.0, 1.0);
            let progress = if total > 2 {
                (matched - 2) as f64 / (total - 2) as f64
            } else {
                1.0
            };
            let confidence = (completion + (1.0 - completion) * 0.5 * progress).min(0.9);
            let candidate = WorkflowProgress {
                workflow_id: workflow.id.clone(),
                title: workflow.title.clone(),
                matched_steps: matched as i64,
                total_steps: total as i64,
                next_step: workflow.steps[matched].clone(),
                confidence: (confidence * 100.0).round() / 100.0,
                skill_id: workflow.skill_id.clone(),
            };
            let better = best.as_ref().is_none_or(|(score, length, _)| {
                confidence > *score || (confidence == *score && matched > *length)
            });
            if better {
                best = Some((confidence, matched, candidate));
            }
            break;
        }
    }
    best.map(|(_, _, progress)| progress)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{
        models::{OpportunityScore, WorkflowStats, WorkflowStep},
        workflows::tests::step,
    };

    fn workflow(keys: &[&str], completion_rate: f64) -> Workflow {
        Workflow {
            id: "wf-test".into(),
            title: "Build and test · Knov desktop".into(),
            generated_title: "Build and test · Knov desktop".into(),
            status: "confirmed".into(),
            active: true,
            steps: keys
                .iter()
                .map(|key| WorkflowStep {
                    key: (*key).into(),
                    kind: "app".into(),
                    category: "code".into(),
                    label: (*key).into(),
                    title: (*key).into(),
                    resource: None,
                    average_seconds: 60,
                })
                .collect(),
            stats: WorkflowStats {
                completion_rate,
                occurrences: 4,
                ..WorkflowStats::default()
            },
            evidence: vec![],
            opportunity: OpportunityScore::default(),
            skill_id: Some("skill-1".into()),
            first_seen_at: 0,
            last_seen_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn infers_multi_day_goals_and_honors_reviews() {
        let steps = (0..3)
            .flat_map(|day| {
                let base = 1_800_000_000 + day * 86_400;
                [
                    step("app:visual studio code", "code", base, 900),
                    step("app:terminal", "terminal", base + 1_000, 300),
                ]
            })
            .collect::<Vec<_>>();
        let goals = infer_goals(&steps, &HashMap::new());
        assert_eq!(goals.len(), 1);
        assert_eq!(goals[0].title, "Build Knov desktop");
        assert_eq!(goals[0].active_days, 3);
        assert!(goals[0].confidence > 0.5 && goals[0].confidence < 1.0);

        let mut reviews = HashMap::new();
        reviews.insert(
            goals[0].id.clone(),
            GoalReview {
                status: "confirmed".into(),
                user_title: Some("Ship the Knov alpha".into()),
            },
        );
        let confirmed = infer_goals(&steps, &reviews);
        assert_eq!(confirmed[0].title, "Ship the Knov alpha");
        assert_eq!(confirmed[0].confidence, 1.0);

        reviews.insert(
            goals[0].id.clone(),
            GoalReview {
                status: "dismissed".into(),
                user_title: None,
            },
        );
        assert!(infer_goals(&steps, &reviews).is_empty());
    }

    #[test]
    fn a_single_day_burst_is_not_a_goal() {
        let steps = vec![step("app:visual studio code", "code", 1_800_000_000, 7_200)];
        assert!(infer_goals(&steps, &HashMap::new()).is_empty());
    }

    #[test]
    fn progress_matches_a_workflow_prefix_and_predicts_the_next_step() {
        let workflows = vec![workflow(&["a", "b", "c", "d"], 0.6)];
        let recent = vec![
            step("app:x", "other", 0, 60),
            step("a", "code", 100, 60),
            step("b", "code", 200, 60),
        ];
        let progress = match_progress(&recent, &workflows).expect("progress");
        assert_eq!(progress.matched_steps, 2);
        assert_eq!(progress.next_step.key, "c");
        assert_eq!(progress.confidence, 0.6);

        let further = vec![
            step("a", "code", 100, 60),
            step("b", "code", 200, 60),
            step("c", "code", 300, 60),
        ];
        let progress = match_progress(&further, &workflows).expect("progress");
        assert_eq!(progress.next_step.key, "d");
        assert!(progress.confidence > 0.6);

        let finished = vec![
            step("a", "code", 100, 60),
            step("b", "code", 200, 60),
            step("c", "code", 300, 60),
            step("d", "code", 400, 60),
        ];
        assert!(match_progress(&finished, &workflows).is_none());
    }
}
