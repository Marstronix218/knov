use std::collections::{HashMap, HashSet};

use url::Url;

use crate::models::ActivityEvent;

const BROWSER_CONTEXT_WINDOW_SECONDS: i64 = 5 * 60;
const WORKOUT_ANCHOR: &str = "workout";

#[derive(Default)]
struct AnchorStats {
    event_count: usize,
    apps: HashSet<String>,
}

/// Assigns a stable, human-readable subject to events that share a meaningful
/// anchor or a high-confidence canonical intent. Low-information navigation
/// pages can inherit an unambiguous nearby subject from the same known browser
/// profile. This stays local: it uses metadata already stored by Knov and
/// never opens page contents.
pub fn semantic_topics(events: &[ActivityEvent]) -> HashMap<i64, String> {
    let event_tokens = events
        .iter()
        .map(|event| (event.id, subject_tokens(event)))
        .collect::<Vec<_>>();
    let mut stats = HashMap::<String, AnchorStats>::new();

    for (event, (_, tokens)) in events.iter().zip(&event_tokens) {
        for token in tokens {
            let entry = stats.entry(token.clone()).or_default();
            entry.event_count += 1;
            entry.apps.insert(event.app_name.to_ascii_lowercase());
        }
    }

    let mut assignments = HashMap::new();
    for (event_id, tokens) in &event_tokens {
        let Some(event_id) = *event_id else {
            continue;
        };
        let anchor = tokens
            .iter()
            .filter(|token| {
                is_canonical_anchor(token)
                    || stats
                        .get(token.as_str())
                        .is_some_and(|value| value.event_count >= 2)
            })
            .max_by(|left, right| {
                anchor_score(left, &stats)
                    .cmp(&anchor_score(right, &stats))
                    .then_with(|| right.cmp(left))
            });
        if let Some(anchor) = anchor {
            assignments.insert(event_id, display_label(anchor));
        }
    }

    inherit_browser_context(events, &event_tokens, &mut assignments);
    assignments
}

fn is_canonical_anchor(token: &str) -> bool {
    token == WORKOUT_ANCHOR
}

fn anchor_score(token: &str, stats: &HashMap<String, AnchorStats>) -> (usize, usize, usize, usize) {
    let value = &stats[token];
    (
        is_canonical_anchor(token) as usize,
        value.event_count,
        value.apps.len(),
        token.len(),
    )
}

fn subject_tokens(event: &ActivityEvent) -> HashSet<String> {
    let mut tokens = HashSet::new();
    let mut descriptive_text = String::new();
    for value in [
        event.search_query.as_deref(),
        event.page_title.as_deref(),
        event.window_title.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        extend_tokens(&mut tokens, value);
        descriptive_text.push(' ');
        descriptive_text.push_str(&value.to_ascii_lowercase());
    }
    if let Some(url) = event
        .url
        .as_deref()
        .and_then(|value| Url::parse(value).ok())
    {
        if let Some(host) = url.host_str() {
            extend_tokens(&mut tokens, host);
        }
        extend_tokens(&mut tokens, url.path());
    }
    if is_workout_context(&descriptive_text) {
        tokens.insert(WORKOUT_ANCHOR.into());
        tokens.remove("exercise");
        tokens.remove("exercises");
        tokens.remove("exercising");
        tokens.remove("training");
    } else {
        // The word "exercise" is too ambiguous to stand alone: it also appears
        // in finance, legal, and instruction contexts.
        tokens.remove("exercise");
        tokens.remove("exercises");
        tokens.remove("exercising");
    }
    if tokens.contains("snowflake") && is_natural_snowflake_context(&descriptive_text) {
        tokens.remove("snowflake");
    }
    if is_low_information_navigation(event, &tokens) {
        tokens.clear();
    }
    tokens
}

fn is_workout_context(value: &str) -> bool {
    let normalized = normalized_words(value);
    let words = normalized.split_whitespace().collect::<HashSet<_>>();
    let direct_signals = [
        "calisthenics",
        "cardio",
        "fitness",
        "situps",
        "weightlifting",
        "workout",
        "workouts",
    ];
    if direct_signals.iter().any(|signal| words.contains(signal)) {
        return true;
    }

    let padded = format!(" {normalized} ");
    let has_exercise_word = ["exercise", "exercises", "exercising"]
        .iter()
        .any(|signal| words.contains(signal));
    let has_fitness_companion = [
        "cardio", "fitness", "gym", "muscle", "routine", "routines", "strength", "workout",
    ]
    .iter()
    .any(|signal| words.contains(signal));
    let has_physical_exercise_phrase = [
        " physical activity ",
        " physical activities ",
        " physical exercise ",
        " physical exercises ",
    ]
    .iter()
    .any(|phrase| padded.contains(phrase));
    if has_exercise_word && (has_fitness_companion || has_physical_exercise_phrase) {
        return true;
    }

    if [
        " sit ups ",
        " push ups ",
        " strength training ",
        " weight training ",
    ]
    .iter()
    .any(|phrase| padded.contains(phrase))
    {
        return true;
    }

    words.contains("plank")
        && ["abs", "challenge", "core", "exercise", "hold", "workout"]
            .iter()
            .any(|signal| words.contains(signal))
}

fn normalized_words(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_low_information_navigation(event: &ActivityEvent, tokens: &HashSet<String>) -> bool {
    let contains_only_generic_tokens =
        tokens.is_empty() || (tokens.len() == 1 && tokens.contains("training"));
    if !contains_only_generic_tokens {
        return false;
    }
    let Some(url) = event
        .url
        .as_deref()
        .and_then(|value| Url::parse(value).ok())
    else {
        return false;
    };
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let path = url.path().trim_end_matches('/');
    let youtube_shell =
        (host == "youtu.be" || host == "youtube.com" || host.ends_with(".youtube.com"))
            && (path.is_empty() || path == "/results");
    let search_shell = (host == "google.com"
        || host.ends_with(".google.com")
        || host == "bing.com"
        || host.ends_with(".bing.com")
        || host == "duckduckgo.com"
        || host.ends_with(".duckduckgo.com"))
        && (path.is_empty() || path == "/search");
    youtube_shell || search_shell
}

fn inherit_browser_context(
    events: &[ActivityEvent],
    event_tokens: &[(Option<i64>, HashSet<String>)],
    assignments: &mut HashMap<i64, String>,
) {
    let direct_assignments = assignments.clone();
    let mut direct_by_browser = HashMap::<(String, String), Vec<(i64, String)>>::new();
    for candidate in events {
        let Some(topic) = candidate
            .id
            .and_then(|candidate_id| direct_assignments.get(&candidate_id))
        else {
            continue;
        };
        let Some(profile_id) = candidate.browser_profile_id.as_ref() else {
            continue;
        };
        direct_by_browser
            .entry((candidate.app_name.to_ascii_lowercase(), profile_id.clone()))
            .or_default()
            .push((candidate.occurred_at, topic.clone()));
    }
    for candidates in direct_by_browser.values_mut() {
        candidates.sort_unstable_by_key(|(occurred_at, _)| *occurred_at);
    }

    for (event, (event_id, tokens)) in events.iter().zip(event_tokens) {
        let Some(event_id) = *event_id else {
            continue;
        };
        if assignments.contains_key(&event_id)
            || !tokens.is_empty()
            || !is_navigation_platform(event)
        {
            continue;
        }
        let Some(profile_id) = event.browser_profile_id.as_ref() else {
            continue;
        };
        let key = (event.app_name.to_ascii_lowercase(), profile_id.clone());
        let Some(candidates) = direct_by_browser.get(&key) else {
            continue;
        };
        let window_start = event
            .occurred_at
            .saturating_sub(BROWSER_CONTEXT_WINDOW_SECONDS);
        let window_end = event
            .occurred_at
            .saturating_add(BROWSER_CONTEXT_WINDOW_SECONDS);
        let first = candidates.partition_point(|(occurred_at, _)| *occurred_at < window_start);
        let nearby_topics = candidates[first..]
            .iter()
            .take_while(|(occurred_at, _)| *occurred_at <= window_end)
            .map(|(_, topic)| topic)
            .collect::<HashSet<_>>();
        if nearby_topics.len() == 1 {
            assignments.insert(
                event_id,
                nearby_topics
                    .into_iter()
                    .next()
                    .expect("one nearby topic")
                    .clone(),
            );
        }
    }
}

fn is_navigation_platform(event: &ActivityEvent) -> bool {
    let Some(host) = event
        .url
        .as_deref()
        .and_then(|value| Url::parse(value).ok())
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
    else {
        return false;
    };
    host == "youtu.be"
        || host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "google.com"
        || host.ends_with(".google.com")
        || host == "bing.com"
        || host.ends_with(".bing.com")
        || host == "duckduckgo.com"
        || host.ends_with(".duckduckgo.com")
}

pub fn is_natural_snowflake_context(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    ["crystal", "photography", "snow storm", "weather", "winter"]
        .iter()
        .any(|signal| value.contains(signal))
}

fn extend_tokens(tokens: &mut HashSet<String>, value: &str) {
    let mut current = String::new();
    for character in value.chars() {
        if character.is_alphanumeric() {
            current.extend(character.to_lowercase());
        } else {
            push_token(tokens, &mut current);
        }
    }
    push_token(tokens, &mut current);
}

fn push_token(tokens: &mut HashSet<String>, current: &mut String) {
    if current.len() >= 4
        && !is_stopword(current)
        && !current.chars().all(|value| value.is_numeric())
    {
        tokens.insert(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn is_stopword(value: &str) -> bool {
    matches!(
        value,
        "about"
            | "account"
            | "activity"
            | "application"
            | "architecture"
            | "beginner"
            | "browser"
            | "chrome"
            | "client"
            | "cloud"
            | "code"
            | "com"
            | "dashboard"
            | "data"
            | "desktop"
            | "developer"
            | "development"
            | "document"
            | "docs"
            | "file"
            | "getting"
            | "google"
            | "guide"
            | "home"
            | "html"
            | "http"
            | "https"
            | "implementation"
            | "index"
            | "introduction"
            | "learn"
            | "main"
            | "notes"
            | "official"
            | "overview"
            | "page"
            | "platform"
            | "pricing"
            | "project"
            | "query"
            | "research"
            | "results"
            | "schema"
            | "search"
            | "software"
            | "studio"
            | "tutorial"
            | "using"
            | "video"
            | "watch"
            | "window"
            | "with"
            | "work"
            | "workspace"
            | "youtube"
    )
}

fn display_label(anchor: &str) -> String {
    match anchor {
        "bigquery" => "BigQuery".into(),
        "databricks" => "Databricks".into(),
        "openai" => "OpenAI".into(),
        "postgresql" => "PostgreSQL".into(),
        "snowflake" => "Snowflake".into(),
        _ => {
            let mut characters = anchor.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ActivitySource;

    fn event(
        id: i64,
        app: &str,
        page_title: Option<&str>,
        window_title: Option<&str>,
        url: Option<&str>,
        search_query: Option<&str>,
    ) -> ActivityEvent {
        ActivityEvent {
            id: Some(id),
            occurred_at: id,
            ended_at: None,
            duration_seconds: 0,
            app_name: app.into(),
            window_title: window_title.map(str::to_string),
            url: url.map(str::to_string),
            page_title: page_title.map(str::to_string),
            search_query: search_query.map(str::to_string),
            browser_profile_id: None,
            source: ActivitySource::AppFocus,
            is_bootstrap: false,
        }
    }

    fn browser_event(
        id: i64,
        app: &str,
        page_title: Option<&str>,
        window_title: Option<&str>,
        url: Option<&str>,
        search_query: Option<&str>,
    ) -> ActivityEvent {
        let mut event = event(id, app, page_title, window_title, url, search_query);
        event.browser_profile_id = Some("Default".into());
        event.source = ActivitySource::ChromeHistory;
        event
    }

    #[test]
    fn groups_snowflake_across_search_video_dashboard_document_and_editor() {
        let events = vec![
            browser_event(
                1,
                "Google Chrome",
                Some("Snowflake architecture tutorial - YouTube"),
                None,
                Some("https://youtube.com/watch?v=demo"),
                None,
            ),
            browser_event(
                2,
                "Google Chrome",
                Some("snowflake architecture - Google Search"),
                None,
                Some("https://google.com/search?q=snowflake"),
                Some("snowflake architecture"),
            ),
            browser_event(
                3,
                "Google Chrome",
                Some("Snowsight"),
                None,
                Some("https://app.snowflake.com/example"),
                None,
            ),
            event(
                4,
                "Preview",
                Some("Snowflake migration notes.pdf"),
                None,
                None,
                None,
            ),
            event(
                5,
                "Cursor",
                Some("src/snowflake_client.rs"),
                Some("Knov — src/snowflake_client.rs"),
                None,
                None,
            ),
        ];

        let topics = semantic_topics(&events);

        assert_eq!(topics.len(), 5);
        assert!(topics.values().all(|value| value == "Snowflake"));
    }

    #[test]
    fn keeps_unrelated_warehouse_work_separate() {
        let events = vec![
            event(1, "Chrome", Some("Snowflake pricing"), None, None, None),
            event(
                2,
                "Preview",
                Some("Snowflake migration.md"),
                None,
                None,
                None,
            ),
            event(3, "Chrome", Some("BigQuery pricing"), None, None, None),
            event(4, "Cursor", Some("bigquery_client.ts"), None, None, None),
        ];

        let topics = semantic_topics(&events);

        assert_eq!(topics.get(&1).map(String::as_str), Some("Snowflake"));
        assert_eq!(topics.get(&2).map(String::as_str), Some("Snowflake"));
        assert_eq!(topics.get(&3).map(String::as_str), Some("BigQuery"));
        assert_eq!(topics.get(&4).map(String::as_str), Some("BigQuery"));
    }

    #[test]
    fn does_not_merge_weather_snowflakes_with_the_company() {
        let events = vec![
            event(
                1,
                "Chrome",
                Some("How a snowflake forms in winter"),
                None,
                None,
                None,
            ),
            event(
                2,
                "Chrome",
                Some("Snowflake data warehouse"),
                None,
                Some("https://app.snowflake.com"),
                None,
            ),
        ];

        assert!(semantic_topics(&events).is_empty());
    }

    #[test]
    fn groups_a_workout_browsing_journey_across_search_video_and_health_pages() {
        let events = vec![
            browser_event(
                1,
                "Google Chrome",
                Some("INTENSE Total Plank Workout - 8 minutes for toned abs and a strong core! - YouTube"),
                None,
                Some("https://youtube.com/watch?v=plank-workout"),
                None,
            ),
            browser_event(
                2,
                "Google Chrome",
                Some("YouTube"),
                None,
                Some("https://youtube.com/"),
                None,
            ),
            browser_event(
                3,
                "Google Chrome",
                Some("plank challenge - YouTube"),
                None,
                Some("https://youtube.com/results?search_query=plank+challenge"),
                None,
            ),
            browser_event(
                4,
                "Google Chrome",
                Some("how to do situps without hurting tailbone - Google Search"),
                None,
                Some("https://google.com/search?q=how+to+do+situps+without+hurting+tailbone"),
                Some("how to do situps without hurting tailbone"),
            ),
            browser_event(
                5,
                "Google Chrome",
                Some("Tailbone Pain From Sitting? Why Sit-Ups Hurt Your Coccyx + Pelvic Floor PT in Oakland"),
                None,
                Some("https://bodyfulphysicaltherapy.com/blog/tailbone-pain-sit-ups"),
                None,
            ),
            browser_event(
                6,
                "Google Chrome",
                Some("YouTube"),
                None,
                Some("https://youtube.com/"),
                None,
            ),
            browser_event(
                7,
                "Google Chrome",
                Some("12 Min Beginner Weight Training - Strength Training for Beginners - YouTube"),
                None,
                Some("https://youtube.com/watch?v=beginner-strength"),
                None,
            ),
            browser_event(
                8,
                "Google Chrome",
                Some("training - YouTube"),
                None,
                Some("https://youtube.com/results?search_query=training"),
                None,
            ),
        ];

        let topics = semantic_topics(&events);

        assert_eq!(topics.len(), events.len());
        assert!(topics.values().all(|value| value == "Workout"));
    }

    #[test]
    fn does_not_pull_unrelated_health_content_into_a_workout_thread() {
        let events = vec![
            browser_event(
                1,
                "Google Chrome",
                Some("20 Minute Full Body Workout for Beginners - YouTube"),
                None,
                Some("https://youtube.com/watch?v=full-body"),
                None,
            ),
            browser_event(
                2,
                "Google Chrome",
                Some("plank challenge - YouTube"),
                None,
                Some("https://youtube.com/results?search_query=plank+challenge"),
                None,
            ),
            browser_event(
                3,
                "Google Chrome",
                Some("Hydration and sleep quality"),
                None,
                Some("https://example.com/health/hydration-and-sleep"),
                None,
            ),
        ];

        let topics = semantic_topics(&events);

        assert_eq!(topics.get(&1).map(String::as_str), Some("Workout"));
        assert_eq!(topics.get(&2).map(String::as_str), Some("Workout"));
        assert!(!topics.contains_key(&3));
    }

    #[test]
    fn leaves_navigation_shell_unassigned_when_nearby_topics_conflict() {
        let events = vec![
            browser_event(
                1,
                "Google Chrome",
                Some("20 Minute Full Body Workout - YouTube"),
                None,
                Some("https://youtube.com/watch?v=full-body"),
                None,
            ),
            browser_event(
                2,
                "Google Chrome",
                Some("plank challenge - YouTube"),
                None,
                Some("https://youtube.com/results?search_query=plank+challenge"),
                None,
            ),
            browser_event(
                3,
                "Google Chrome",
                Some("Rust ownership guide"),
                None,
                Some("https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html"),
                None,
            ),
            browser_event(
                4,
                "Google Chrome",
                Some("Rust lifetimes explained"),
                None,
                Some("https://example.com/rust-lifetimes"),
                None,
            ),
            browser_event(
                5,
                "Google Chrome",
                Some("YouTube"),
                None,
                Some("https://youtube.com/"),
                None,
            ),
        ];

        let topics = semantic_topics(&events);

        assert_eq!(topics.get(&1).map(String::as_str), Some("Workout"));
        assert_eq!(topics.get(&2).map(String::as_str), Some("Workout"));
        assert_eq!(topics.get(&3).map(String::as_str), Some("Rust"));
        assert_eq!(topics.get(&4).map(String::as_str), Some("Rust"));
        assert!(!topics.contains_key(&5));
    }

    #[test]
    fn canonical_workout_wins_over_repeated_aliases() {
        for titles in [
            ["strength training plan", "strength training routine"],
            ["exercise workout guide", "exercise workout routine"],
            ["fitness workout guide", "fitness workout routine"],
            [
                "Exercise benefits of regular physical activity",
                "daily physical exercise routine",
            ],
        ] {
            let events = titles
                .into_iter()
                .enumerate()
                .map(|(index, title)| {
                    event(index as i64 + 1, "Chrome", Some(title), None, None, None)
                })
                .collect::<Vec<_>>();

            let topics = semantic_topics(&events);

            assert!(topics.values().all(|value| value == "Workout"));
        }
    }

    #[test]
    fn does_not_treat_ambiguous_exercise_or_posture_language_as_workouts() {
        let events = vec![
            event(
                1,
                "Chrome",
                Some("How to exercise stock options"),
                None,
                None,
                None,
            ),
            event(
                2,
                "Chrome",
                Some("When shareholders exercise voting rights"),
                None,
                None,
                None,
            ),
            event(
                3,
                "Chrome",
                Some("How to sit up straight at a desk"),
                None,
                None,
                None,
            ),
            event(
                4,
                "Chrome",
                Some("How to exercise your right to access health records"),
                None,
                None,
                None,
            ),
            event(
                5,
                "Chrome",
                Some("Exercise caution during this activity"),
                None,
                None,
                None,
            ),
        ];

        assert!(semantic_topics(&events).is_empty());
    }

    #[test]
    fn browser_context_inheritance_requires_the_same_known_profile() {
        let workout = browser_event(
            1,
            "Google Chrome",
            Some("20 Minute Full Body Workout - YouTube"),
            None,
            Some("https://youtube.com/watch?v=full-body"),
            None,
        );
        let mut different_profile = browser_event(
            2,
            "Google Chrome",
            Some("YouTube"),
            None,
            Some("https://youtube.com/"),
            None,
        );
        different_profile.browser_profile_id = Some("Profile 2".into());
        let mut unknown_profile = browser_event(
            3,
            "Google Chrome",
            Some("YouTube"),
            None,
            Some("https://youtube.com/"),
            None,
        );
        unknown_profile.browser_profile_id = None;
        let events = vec![workout, different_profile, unknown_profile];

        let topics = semantic_topics(&events);

        assert_eq!(topics.get(&1).map(String::as_str), Some("Workout"));
        assert!(!topics.contains_key(&2));
        assert!(!topics.contains_key(&3));
    }

    #[test]
    fn preserves_explicit_single_token_search_subjects() {
        let events = vec![
            browser_event(
                1,
                "Google Chrome",
                Some("20 Minute Full Body Workout - YouTube"),
                None,
                Some("https://youtube.com/watch?v=full-body"),
                None,
            ),
            browser_event(
                2,
                "Google Chrome",
                Some("rust - YouTube"),
                None,
                Some("https://youtube.com/results?search_query=rust"),
                Some("rust"),
            ),
            browser_event(
                3,
                "Google Chrome",
                Some("rust - Google Search"),
                None,
                Some("https://google.com/search?q=rust"),
                Some("rust"),
            ),
            browser_event(
                4,
                "Google Chrome",
                Some("snowflake - YouTube"),
                None,
                Some("https://youtube.com/results?search_query=snowflake"),
                Some("snowflake"),
            ),
        ];

        let topics = semantic_topics(&events);

        assert_eq!(topics.get(&1).map(String::as_str), Some("Workout"));
        assert_eq!(topics.get(&2).map(String::as_str), Some("Rust"));
        assert_eq!(topics.get(&3).map(String::as_str), Some("Rust"));
        assert!(!topics.contains_key(&4));
    }

    #[test]
    fn keeps_training_available_as_a_non_workout_subject() {
        let events = vec![
            event(1, "Chrome", Some("Training overview"), None, None, None),
            event(2, "Notes", Some("Training guide"), None, None, None),
        ];

        let topics = semantic_topics(&events);

        assert_eq!(topics.get(&1).map(String::as_str), Some("Training"));
        assert_eq!(topics.get(&2).map(String::as_str), Some("Training"));
    }

    #[test]
    fn breaks_equal_anchor_scores_deterministically() {
        let events = vec![
            event(1, "Chrome", Some("alpha bravo"), None, None, None),
            event(2, "Notes", Some("alpha bravo"), None, None, None),
        ];

        let topics = semantic_topics(&events);

        assert!(topics.values().all(|value| value == "Alpha"));
    }
}
