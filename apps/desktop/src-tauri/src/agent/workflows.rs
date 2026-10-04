//! Workflow miner: finds recurring contiguous step sequences across sessions
//! and days, then ranks them as automation opportunities. Deterministic and
//! local; no provider is involved.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Datelike, Local, TimeZone, Timelike};
use sha2::{Digest, Sha256};

use super::{
    models::{OpportunityScore, WorkflowOccurrence, WorkflowStats, WorkflowStep},
    normalize::{step_title, SemanticStep},
};
use crate::prediction::sanitize_text;

pub(crate) const MIN_STEPS: usize = 3;
pub(crate) const MAX_STEPS: usize = 6;
pub(crate) const MIN_OCCURRENCES: usize = 3;
const MIN_DISTINCT_DAYS: usize = 2;
/// A workflow involves at least three different steps; two-step alternation
/// (A → B → A → B) is a habit, not an automatable process.
const MIN_DISTINCT_STEPS: usize = 3;
/// Patterns finished this rarely once started are coincidences of a common
/// opening, not stable workflows.
const MIN_COMPLETION_RATE: f64 = 0.15;
const MAX_WORKFLOWS: usize = 30;
const EVIDENCE_LIMIT: usize = 5;
/// Opportunities below this score stay listed as workflows but are not
/// surfaced on the Automation Opportunity Radar.
pub(crate) const OPPORTUNITY_THRESHOLD: f64 = 0.40;
/// Stronger evidence than mining requires before Knov pitches automation.
const OPPORTUNITY_MIN_OCCURRENCES: i64 = 4;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MinedWorkflow {
    pub id: String,
    pub generated_title: String,
    pub steps: Vec<WorkflowStep>,
    pub stats: WorkflowStats,
    pub evidence: Vec<WorkflowOccurrence>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

/// Outcome history of a skill created from a workflow, used to close the loop
/// between execution results and opportunity ranking.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct SkillOutcome {
    pub runs: i64,
    pub completed: i64,
    pub rolled_back: i64,
}

pub(crate) fn workflow_id(keys: &[&str]) -> String {
    let digest = Sha256::digest(keys.join("\u{1f}").as_bytes());
    format!("wf-{}", &format!("{digest:x}")[..12])
}

pub(crate) fn mine_workflows(sessions: &[Vec<SemanticStep>]) -> Vec<MinedWorkflow> {
    let mut candidates = HashMap::<Vec<&str>, Vec<(usize, usize)>>::new();
    for (session_index, session) in sessions.iter().enumerate() {
        let keys = session
            .iter()
            .map(|step| step.key.as_str())
            .collect::<Vec<_>>();
        for length in MIN_STEPS..=MAX_STEPS {
            if keys.len() < length {
                break;
            }
            for start in 0..=keys.len() - length {
                let window = &keys[start..start + length];
                if window.iter().collect::<HashSet<_>>().len() < MIN_DISTINCT_STEPS {
                    continue;
                }
                candidates
                    .entry(window.to_vec())
                    .or_default()
                    .push((session_index, start));
            }
        }
    }

    let mut frequent =
        candidates
            .into_iter()
            .filter_map(|(pattern, positions)| {
                let occurrences = non_overlapping(&positions, pattern.len());
                let days = occurrences
                    .iter()
                    .map(|(session, start)| local_day(sessions[*session][*start].started_at))
                    .collect::<HashSet<_>>();
                (occurrences.len() >= MIN_OCCURRENCES && days.len() >= MIN_DISTINCT_DAYS)
                    .then_some((pattern, occurrences, days.len()))
            })
            .collect::<Vec<_>>();

    // Prefer longer patterns, then more frequent ones; keep only maximal ones.
    frequent.sort_by(|left, right| {
        right
            .0
            .len()
            .cmp(&left.0.len())
            .then_with(|| right.1.len().cmp(&left.1.len()))
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut kept = Vec::<(Vec<&str>, Vec<(usize, usize)>, usize)>::new();
    for candidate in frequent {
        let subsumed = kept.iter().any(|(longer, occurrences, _)| {
            longer.len() > candidate.0.len()
                && contains_window(longer, &candidate.0)
                && occurrences.len() * 4 >= candidate.1.len() * 3
        });
        if !subsumed {
            kept.push(candidate);
        }
    }
    kept.sort_by(|left, right| {
        (right.1.len() * right.0.len())
            .cmp(&(left.1.len() * left.0.len()))
            .then_with(|| left.0.cmp(&right.0))
    });
    kept.truncate(MAX_WORKFLOWS);

    let observation_days = observation_days(sessions);
    kept.into_iter()
        .map(|(pattern, occurrences, distinct_days)| {
            build_workflow(
                sessions,
                &pattern,
                &occurrences,
                distinct_days,
                observation_days,
            )
        })
        .filter(|workflow| workflow.stats.completion_rate >= MIN_COMPLETION_RATE)
        .collect()
}

fn non_overlapping(positions: &[(usize, usize)], length: usize) -> Vec<(usize, usize)> {
    let mut sorted = positions.to_vec();
    sorted.sort_unstable();
    let mut selected = Vec::new();
    let mut last: Option<(usize, usize)> = None;
    for (session, start) in sorted {
        if last.is_some_and(|(last_session, last_end)| last_session == session && start < last_end)
        {
            continue;
        }
        selected.push((session, start));
        last = Some((session, start + length));
    }
    selected
}

fn contains_window(haystack: &[&str], needle: &[&str]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn local_day(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

fn observation_days(sessions: &[Vec<SemanticStep>]) -> f64 {
    let first = sessions
        .iter()
        .filter_map(|session| session.first())
        .map(|step| step.started_at)
        .min();
    let last = sessions
        .iter()
        .filter_map(|session| session.last())
        .map(|step| step.ended_at)
        .max();
    match (first, last) {
        (Some(first), Some(last)) => ((last - first) as f64 / 86_400.0).max(7.0),
        _ => 7.0,
    }
}

fn build_workflow(
    sessions: &[Vec<SemanticStep>],
    pattern: &[&str],
    occurrences: &[(usize, usize)],
    distinct_days: usize,
    observation_days: f64,
) -> MinedWorkflow {
    let windows = occurrences
        .iter()
        .map(|(session, start)| &sessions[*session][*start..*start + pattern.len()])
        .collect::<Vec<_>>();
    let steps = (0..pattern.len())
        .map(|index| {
            let sample = &windows[0][index];
            let average_seconds = windows
                .iter()
                .map(|window| window[index].seconds)
                .sum::<i64>()
                / windows.len() as i64;
            WorkflowStep {
                key: sample.key.clone(),
                kind: sample.kind.into(),
                category: sample.category.into(),
                label: sample.label.clone(),
                title: step_title(sample.category, &sample.label),
                resource: representative_resource(&windows, index),
                average_seconds,
            }
        })
        .collect::<Vec<_>>();

    let thread_per_occurrence = windows
        .iter()
        .map(|window| dominant(window.iter().filter_map(|step| step.thread.as_deref())))
        .collect::<Vec<_>>();
    let thread =
        dominant(thread_per_occurrence.iter().filter_map(Option::as_deref)).filter(|candidate| {
            thread_per_occurrence
                .iter()
                .filter(|value| value.as_deref() == Some(candidate.as_str()))
                .count()
                * 2
                >= windows.len()
        });

    let durations = windows
        .iter()
        .map(|window| {
            let last = window.last().expect("non-empty window");
            (last.ended_at.max(last.started_at + last.seconds) - window[0].started_at).max(0)
        })
        .collect::<Vec<_>>();
    let starts = windows
        .iter()
        .map(|window| window[0].started_at)
        .collect::<Vec<_>>();
    let prefix_starts = sessions
        .iter()
        .map(|session| {
            session
                .windows(2)
                .filter(|pair| pair[0].key == pattern[0] && pair[1].key == pattern[1])
                .count()
        })
        .sum::<usize>()
        .max(occurrences.len());

    let mut evidence = windows
        .iter()
        .zip(&thread_per_occurrence)
        .map(|(window, thread)| WorkflowOccurrence {
            started_at: window[0].started_at,
            ended_at: window
                .last()
                .map_or(window[0].started_at, |step| step.ended_at),
            thread: thread.clone(),
            details: window.iter().map(step_detail).collect(),
        })
        .collect::<Vec<_>>();
    evidence.sort_by_key(|occurrence| std::cmp::Reverse(occurrence.started_at));
    evidence.truncate(EVIDENCE_LIMIT);

    let stats = WorkflowStats {
        occurrences: occurrences.len() as i64,
        distinct_days: distinct_days as i64,
        per_week: round1(occurrences.len() as f64 * 7.0 / observation_days),
        average_duration_seconds: durations.iter().sum::<i64>() / durations.len() as i64,
        completion_rate: round2(occurrences.len() as f64 / prefix_starts as f64),
        typical_weekday: typical_weekday(&starts),
        typical_hour: typical_hour(&starts),
        thread: thread.clone(),
    };
    MinedWorkflow {
        id: workflow_id(pattern),
        generated_title: workflow_title(&steps, thread.as_deref()),
        steps,
        stats,
        evidence,
        first_seen_at: *starts.iter().min().unwrap_or(&0),
        last_seen_at: *starts.iter().max().unwrap_or(&0),
    }
}

/// The most common safe locator for a web step when it is stable across runs,
/// otherwise the site root. The runtime later prefers the latest locator from
/// the active thread.
fn representative_resource(windows: &[&[SemanticStep]], index: usize) -> Option<String> {
    let sample = &windows[0][index];
    match sample.kind {
        "app" => Some(sample.label.clone()),
        "web" => {
            let locator = dominant(
                windows
                    .iter()
                    .filter_map(|window| window[index].locator.as_deref()),
            );
            let stable = locator.as_deref().is_some_and(|value| {
                windows
                    .iter()
                    .filter(|window| window[index].locator.as_deref() == Some(value))
                    .count()
                    * 2
                    >= windows.len()
            });
            if stable {
                locator
            } else {
                let host = sample.key.trim_start_matches("web:");
                Some(format!("https://{host}"))
            }
        }
        _ => None,
    }
}

fn step_detail(step: &SemanticStep) -> String {
    let detail = step
        .locator
        .as_deref()
        .map(|locator| {
            locator
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .to_string()
        })
        .unwrap_or_else(|| step.label.clone());
    sanitize_text(&detail, 120)
}

fn dominant<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(left.0)))
        .map(|(value, _)| value.to_string())
}

fn typical_weekday(starts: &[i64]) -> Option<String> {
    let mut counts = BTreeMap::<u32, usize>::new();
    for start in starts {
        if let Some(value) = Local.timestamp_opt(*start, 0).single() {
            *counts
                .entry(value.weekday().num_days_from_monday())
                .or_default() += 1;
        }
    }
    let (day, count) = counts.into_iter().max_by_key(|(_, count)| *count)?;
    (count >= 3 && count * 10 >= starts.len() * 6).then(|| weekday_name(day).into())
}

fn typical_hour(starts: &[i64]) -> Option<u32> {
    let hours = starts
        .iter()
        .filter_map(|start| Local.timestamp_opt(*start, 0).single())
        .map(|value| value.hour())
        .collect::<Vec<_>>();
    let best = (0..24)
        .map(|hour: u32| {
            let near = hours
                .iter()
                .filter(|value| (**value as i32 - hour as i32).abs() <= 1)
                .count();
            (near, hour)
        })
        .max()?;
    (best.0 >= 3 && best.0 * 10 >= hours.len() * 6).then_some(best.1)
}

pub(crate) fn weekday_name(day: u32) -> &'static str {
    [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ][day.min(6) as usize]
}

pub(crate) fn workflow_title(steps: &[WorkflowStep], thread: Option<&str>) -> String {
    let categories = steps
        .iter()
        .map(|step| step.category.as_str())
        .collect::<HashSet<_>>();
    let has = |category: &str| categories.contains(category);
    let named = if has("code") && has("terminal") {
        Some("Build and test")
    } else if has("code-hosting") && has("code") {
        Some("Issue to code")
    } else if has("ai") && has("code") {
        Some("AI-assisted coding")
    } else if has("spreadsheet") && (has("communication") || has("email")) {
        Some("Reporting routine")
    } else if (has("communication") || has("email")) && has("notes") {
        Some("Follow-up routine")
    } else if has("search") && (has("docs") || has("video") || has("ai")) {
        Some("Research loop")
    } else if has("design") {
        Some("Design review")
    } else if has("project") {
        Some("Planning routine")
    } else {
        None
    };
    let base = named.map(str::to_string).unwrap_or_else(|| {
        let mut seen = HashSet::new();
        steps
            .iter()
            .map(|step| step.label.as_str())
            .filter(|label| seen.insert(*label))
            .take(3)
            .collect::<Vec<_>>()
            .join(" → ")
    });
    match thread {
        Some(thread) => format!("{base} · {}", sanitize_text(thread, 60)),
        None => base,
    }
}

/// What Knov could do for a step: open it (prepare), run it (execute), or
/// leave it to the user (manual).
pub(crate) fn step_capability(step: &WorkflowStep, has_workspace: bool) -> &'static str {
    match (step.kind.as_str(), step.category.as_str()) {
        (_, "sensitive") | ("search", _) => "manual",
        ("app", "terminal") if has_workspace => "execute",
        ("app", _) => "prepare",
        ("web", _) if step.resource.is_some() => "prepare",
        _ => "manual",
    }
}

pub(crate) fn score_opportunity(
    steps: &[WorkflowStep],
    stats: &WorkflowStats,
    outcome: Option<SkillOutcome>,
    has_workspace: bool,
) -> OpportunityScore {
    let mut seen = HashSet::new();
    let mut preparable = 0_i64;
    let mut executable = 0_i64;
    let mut weighted = 0.0;
    for step in steps {
        let capability = step_capability(step, has_workspace);
        let first_time = seen.insert(step.key.as_str());
        match capability {
            "execute" => {
                executable += first_time as i64;
                weighted += 1.0;
            }
            "prepare" => {
                preparable += first_time as i64;
                weighted += 0.5;
            }
            _ => {}
        }
    }
    let frequency = (stats.per_week / 5.0).clamp(0.0, 1.0);
    let time_cost = (stats.average_duration_seconds as f64 / 1_800.0).clamp(0.0, 1.0);
    let stability = stats.completion_rate.clamp(0.0, 1.0);
    let executability = (weighted / steps.len().max(1) as f64).clamp(0.0, 1.0);
    let risk = steps
        .iter()
        .map(|step| match step.category.as_str() {
            "sensitive" => 0.9,
            "communication" | "email" => 0.3,
            _ => 0.1,
        })
        .fold(0.0_f64, f64::max);
    let outcome_factor = outcome
        .filter(|value| value.runs > 0)
        .map(|value| {
            let completed = value.completed as f64 / value.runs as f64;
            let rolled_back = value.rolled_back as f64 / value.runs as f64;
            (0.5 + 0.5 * completed - 0.3 * rolled_back).clamp(0.2, 1.0)
        })
        .unwrap_or(1.0);
    let score = round2(
        ((0.45 * frequency + 0.15 * time_cost + 0.2 * stability + 0.2 * executability)
            * (1.0 - risk)
            * outcome_factor)
            .clamp(0.0, 1.0),
    );
    let minutes = stats.per_week * (preparable as f64 * 15.0 + executable as f64 * 60.0) / 60.0;

    let mut rationale = vec![
        format!(
            "Seen {} times on {} days (about {:.1} per week).",
            stats.occurrences, stats.distinct_days, stats.per_week
        ),
        format!(
            "Takes about {} each time.",
            human_minutes(stats.average_duration_seconds)
        ),
        format!(
            "Finished {:.0}% of the times it was started.",
            stability * 100.0
        ),
    ];
    rationale.push(match (preparable, executable) {
        (0, 0) => "Knov has no safe action for these steps yet.".into(),
        (prepare, 0) => format!("Knov can prepare {prepare} of these steps by opening them."),
        (prepare, execute) => {
            format!("Knov can prepare {prepare} step(s) and run checks for {execute} step(s).")
        }
    });
    if risk >= 0.9 {
        rationale.push("Includes a sensitive site, so Knov will not automate it.".into());
    } else if risk >= 0.3 {
        rationale.push("Includes communication; Knov only opens it and never sends.".into());
    }
    if let Some(outcome) = outcome.filter(|value| value.runs > 0) {
        rationale.push(format!(
            "Skill runs so far: {} of {} completed, {} rolled back.",
            outcome.completed, outcome.runs, outcome.rolled_back
        ));
    }

    OpportunityScore {
        score,
        frequency: round2(frequency),
        time_cost: round2(time_cost),
        stability: round2(stability),
        executability: round2(executability),
        risk: round2(risk),
        preparable_steps: preparable,
        executable_steps: executable,
        estimated_minutes_saved_per_week: round1(minutes),
        surfaced: score >= OPPORTUNITY_THRESHOLD
            && stats.occurrences >= OPPORTUNITY_MIN_OCCURRENCES
            && preparable + executable > 0
            && risk < 0.9,
        rationale,
    }
}

fn human_minutes(seconds: i64) -> String {
    let minutes = (seconds as f64 / 60.0).round() as i64;
    if minutes < 1 {
        "under a minute".into()
    } else if minutes < 90 {
        format!("{minutes} min")
    } else {
        format!("{:.1} h", minutes as f64 / 60.0)
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn step(key: &str, category: &'static str, at: i64, seconds: i64) -> SemanticStep {
        let (kind, label) = match key.split_once(':') {
            Some(("web", domain)) => ("web", domain.to_string()),
            Some((_, app)) => ("app", app.to_string()),
            None => ("search", "Web search".to_string()),
        };
        let locator = (kind == "web").then(|| format!("https://{label}/issues/{at}"));
        SemanticStep {
            key: key.into(),
            kind,
            category,
            label,
            locator,
            thread: Some("Knov desktop".into()),
            started_at: at,
            ended_at: at + seconds,
            seconds,
        }
    }

    /// Issue → code → terminal on several days, with noise around it.
    pub(crate) fn developer_sessions(days: i64) -> Vec<Vec<SemanticStep>> {
        (0..days)
            .map(|day| {
                let base = 1_800_000_000 + day * 86_400;
                vec![
                    step("app:slack", "communication", base, 120),
                    step("web:github.com", "code-hosting", base + 200, 300),
                    step("app:visual studio code", "code", base + 600, 1_200),
                    step("app:terminal", "terminal", base + 1_900, 300),
                    step(
                        if day % 2 == 0 {
                            "web:docs.rs"
                        } else {
                            "app:notes"
                        },
                        if day % 2 == 0 { "docs" } else { "notes" },
                        base + 2_300,
                        200,
                    ),
                ]
            })
            .collect()
    }

    #[test]
    fn mines_a_repeated_cross_day_workflow_and_keeps_it_maximal() {
        let workflows = mine_workflows(&developer_sessions(4));
        let top = &workflows[0];
        let keys = top
            .steps
            .iter()
            .map(|step| step.key.as_str())
            .collect::<Vec<_>>();
        assert!(
            keys.windows(3).any(
                |window| window == ["web:github.com", "app:visual studio code", "app:terminal"]
            ),
            "{keys:?}"
        );
        assert_eq!(top.stats.occurrences, 4);
        assert_eq!(top.stats.distinct_days, 4);
        assert_eq!(top.stats.thread.as_deref(), Some("Knov desktop"));
        assert!(top.generated_title.starts_with("Build and test"));
        // The 3-step core is subsumed by the longer pattern with equal support.
        assert!(
            !workflows
                .iter()
                .any(|workflow| workflow.steps.len() == 3
                    && workflow.steps[0].key == "web:github.com")
        );
        // Evidence carries domains and labels only.
        assert!(top.evidence.iter().all(|occurrence| occurrence
            .details
            .iter()
            .all(|detail| !detail.contains("https://"))));
    }

    #[test]
    fn ignores_patterns_seen_on_a_single_day_or_too_rarely() {
        let mut sessions = developer_sessions(2);
        assert!(mine_workflows(&sessions).is_empty());
        // Three occurrences on one day are still not a cross-day workflow.
        let base = 1_900_000_000;
        sessions = (0..3)
            .map(|offset| {
                vec![
                    step("web:github.com", "code-hosting", base + offset * 300, 60),
                    step(
                        "app:visual studio code",
                        "code",
                        base + offset * 300 + 100,
                        60,
                    ),
                    step("app:terminal", "terminal", base + offset * 300 + 200, 60),
                ]
            })
            .collect();
        assert!(mine_workflows(&sessions).is_empty());
    }

    #[test]
    fn alternation_and_coincidental_openings_are_not_workflows() {
        // A → B → A → B across many days has only two distinct steps.
        let alternating = (0..5)
            .map(|day| {
                let base = 1_800_000_000 + day * 86_400;
                (0..4)
                    .map(|index| {
                        let (key, category) = if index % 2 == 0 {
                            ("web:youtube.com", "video")
                        } else {
                            ("search", "search")
                        };
                        step(key, category, base + index * 100, 60)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert!(mine_workflows(&alternating).is_empty());

        // The same opening is extremely common but rarely leads to the full pattern.
        let mut sessions = developer_sessions(3);
        for day in 20..40 {
            let base = 1_800_000_000 + day * 86_400;
            sessions.push(vec![
                step("app:slack", "communication", base, 60),
                step("web:github.com", "code-hosting", base + 100, 60),
            ]);
        }
        assert!(mine_workflows(&sessions)
            .iter()
            .all(|workflow| workflow.stats.completion_rate >= 0.15));
    }

    #[test]
    fn workflow_ids_are_stable_for_the_same_step_signature() {
        let first = mine_workflows(&developer_sessions(4));
        let second = mine_workflows(&developer_sessions(5));
        assert_eq!(first[0].id, second[0].id);
    }

    #[test]
    fn completion_rate_counts_abandoned_starts() {
        let mut sessions = developer_sessions(3);
        // Start the workflow twice more without finishing it.
        for day in 10..12 {
            let base = 1_800_000_000 + day * 86_400;
            sessions.push(vec![
                step("app:slack", "communication", base, 60),
                step("web:github.com", "code-hosting", base + 100, 60),
                step("app:mail", "email", base + 200, 60),
            ]);
        }
        let workflows = mine_workflows(&sessions);
        let core = workflows
            .iter()
            .find(|workflow| workflow.steps[0].key == "app:slack")
            .expect("core workflow");
        // Three finished runs out of five starts.
        assert_eq!(core.stats.completion_rate, 0.6);
    }

    #[test]
    fn opportunity_score_is_bounded_and_penalizes_sensitive_steps() {
        let workflow = &mine_workflows(&developer_sessions(5))[0];
        let score = score_opportunity(&workflow.steps, &workflow.stats, None, false);
        assert!((0.0..=1.0).contains(&score.score));
        assert!(score.preparable_steps >= 2);
        assert_eq!(score.executable_steps, 0);
        let with_workspace = score_opportunity(&workflow.steps, &workflow.stats, None, true);
        assert_eq!(with_workspace.executable_steps, 1);
        assert!(with_workspace.score >= score.score);

        let mut sensitive = workflow.steps.clone();
        sensitive[0].category = "sensitive".into();
        let risky = score_opportunity(&sensitive, &workflow.stats, None, false);
        assert!(risky.score < score.score);
        assert!(!risky.surfaced);

        let poor = score_opportunity(
            &workflow.steps,
            &workflow.stats,
            Some(SkillOutcome {
                runs: 4,
                completed: 1,
                rolled_back: 3,
            }),
            false,
        );
        assert!(poor.score < score.score);
    }
}
