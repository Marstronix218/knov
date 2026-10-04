//! Event normalizer: converts app, browser, and editor metadata into a stable
//! sequence of semantic work steps. Only app names, domains, safe locators,
//! and local thread subjects survive; window and page titles do not.

use std::collections::HashSet;

use url::Url;

use crate::{
    models::{ActivityEvent, ActivitySource, Settings},
    prediction::{safe_domain, safe_locator},
    threading::semantic_topics,
};

/// Idle gap that separates two working sessions.
pub(crate) const SESSION_GAP_SECONDS: i64 = 20 * 60;
/// Foreground blips shorter than this are app-switcher noise, not work.
const MIN_FOCUS_SECONDS: i64 = 8;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SemanticStep {
    pub key: String,
    pub kind: &'static str,
    pub category: &'static str,
    pub label: String,
    pub locator: Option<String>,
    pub thread: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    pub seconds: i64,
}

/// `events` may be in any order; the result is chronological with consecutive
/// identical steps merged.
pub(crate) fn normalize_events(events: &[ActivityEvent], settings: &Settings) -> Vec<SemanticStep> {
    let excluded_apps = settings
        .excluded_apps
        .iter()
        .map(|value| canonical_app_name(value.trim()).to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let excluded_domains = settings
        .excluded_domains
        .iter()
        .map(|value| value.trim().trim_start_matches("www.").to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let topics = semantic_topics(events);
    let mut web_times = events
        .iter()
        .filter(|event| is_web_source(event.source))
        .map(|event| event.occurred_at)
        .collect::<Vec<_>>();
    web_times.sort_unstable();

    let mut steps = events
        .iter()
        .filter(|event| {
            !excluded_apps.contains(&canonical_app_name(&event.app_name).to_ascii_lowercase())
        })
        .filter(|event| {
            event
                .url
                .as_deref()
                .and_then(safe_domain)
                .is_none_or(|domain| !domain_is_excluded(&domain, &excluded_domains))
        })
        .filter_map(|event| {
            let thread = event.id.and_then(|id| topics.get(&id)).cloned();
            step_for(event, thread, &web_times)
        })
        .collect::<Vec<_>>();
    steps.sort_by_key(|step| step.started_at);
    merge_consecutive(steps)
}

pub(crate) fn split_sessions(steps: Vec<SemanticStep>) -> Vec<Vec<SemanticStep>> {
    let mut sessions = Vec::<Vec<SemanticStep>>::new();
    for step in steps {
        let continues = sessions
            .last()
            .and_then(|session| session.last())
            .is_some_and(|previous| step.started_at - previous.ended_at <= SESSION_GAP_SECONDS);
        if continues {
            sessions.last_mut().expect("open session").push(step);
        } else {
            sessions.push(vec![step]);
        }
    }
    sessions
}

fn merge_consecutive(steps: Vec<SemanticStep>) -> Vec<SemanticStep> {
    let mut merged = Vec::<SemanticStep>::with_capacity(steps.len());
    for step in steps {
        if let Some(previous) = merged.last_mut() {
            if previous.key == step.key
                && step.started_at - previous.ended_at <= SESSION_GAP_SECONDS
            {
                previous.ended_at = previous.ended_at.max(step.ended_at);
                previous.seconds += step.seconds;
                if step.locator.is_some() {
                    previous.locator = step.locator;
                }
                if previous.thread.is_none() {
                    previous.thread = step.thread;
                }
                continue;
            }
        }
        merged.push(step);
    }
    merged
}

fn step_for(
    event: &ActivityEvent,
    thread: Option<String>,
    web_times: &[i64],
) -> Option<SemanticStep> {
    let app = canonical_app_name(&event.app_name);
    if is_noise_app(app) {
        return None;
    }
    let seconds = event.duration_seconds.max(0);
    let started_at = event.occurred_at;
    let ended_at = event
        .ended_at
        .unwrap_or(started_at + seconds)
        .max(started_at);
    match event.source {
        ActivitySource::ChromeHistory | ActivitySource::ChromeExtension => {
            let url = event.url.as_deref()?;
            let parsed = Url::parse(url).ok()?;
            let domain = safe_domain(url)?;
            // Only general web search counts as "search"; searching inside a
            // site (for example YouTube results) is part of using that site.
            if is_search_engine(&domain)
                && (event.search_query.is_some() || is_search_path(parsed.path()))
            {
                return Some(SemanticStep {
                    key: "search".into(),
                    kind: "search",
                    category: "search",
                    label: "Web search".into(),
                    locator: None,
                    thread,
                    started_at,
                    ended_at,
                    seconds,
                });
            }
            let (key, label) = web_identity(&domain, parsed.path());
            Some(SemanticStep {
                key,
                kind: "web",
                category: domain_category(&domain, parsed.path()),
                label,
                locator: safe_locator(url),
                thread,
                started_at,
                ended_at,
                seconds,
            })
        }
        ActivitySource::AppFocus => {
            if seconds < MIN_FOCUS_SECONDS {
                return None;
            }
            let category = app_category(app);
            // Browser focus is represented by its page-level web steps when those exist.
            if category == "browser"
                && event.url.is_none()
                && overlaps(web_times, started_at, ended_at)
            {
                return None;
            }
            Some(app_step(
                app, category, thread, started_at, ended_at, seconds,
            ))
        }
        ActivitySource::EditorHistory => {
            Some(app_step(app, "code", thread, started_at, started_at, 0))
        }
    }
}

fn app_step(
    app: &str,
    category: &'static str,
    thread: Option<String>,
    started_at: i64,
    ended_at: i64,
    seconds: i64,
) -> SemanticStep {
    SemanticStep {
        key: format!("app:{}", app.to_ascii_lowercase()),
        kind: "app",
        category,
        label: app.into(),
        locator: None,
        thread,
        started_at,
        ended_at,
        seconds,
    }
}

fn is_web_source(source: ActivitySource) -> bool {
    matches!(
        source,
        ActivitySource::ChromeHistory | ActivitySource::ChromeExtension
    )
}

fn overlaps(sorted_times: &[i64], start: i64, end: i64) -> bool {
    let first = sorted_times.partition_point(|time| *time < start);
    sorted_times.get(first).is_some_and(|time| *time <= end)
}

fn domain_is_excluded(domain: &str, excluded: &[String]) -> bool {
    excluded
        .iter()
        .any(|rule| domain == rule || domain.ends_with(&format!(".{rule}")))
}

pub(crate) fn canonical_app_name(value: &str) -> &str {
    match value.trim() {
        "Code" => "Visual Studio Code",
        other => other,
    }
}

fn is_noise_app(app: &str) -> bool {
    matches!(
        app.to_ascii_lowercase().as_str(),
        "" | "knov"
            | "appsdesktop"
            | "loginwindow"
            | "dock"
            | "notification center"
            | "notificationcenter"
            | "control center"
            | "controlcenter"
            | "spotlight"
            | "screensaverengine"
            | "usernotificationcenter"
            | "system settings"
            | "system preferences"
            | "unknown"
    )
}

fn is_search_engine(domain: &str) -> bool {
    domain == "google.com"
        || domain.starts_with("google.")
        || domain == "bing.com"
        || domain == "duckduckgo.com"
        || domain == "search.brave.com"
        || domain == "kagi.com"
}

fn is_search_path(path: &str) -> bool {
    matches!(path.trim_end_matches('/'), "" | "/search" | "/html")
}

fn web_identity(domain: &str, path: &str) -> (String, String) {
    if domain == "docs.google.com" {
        let product = path.split('/').find(|part| !part.is_empty()).unwrap_or("");
        let label = match product {
            "spreadsheets" => "Google Sheets",
            "presentation" => "Google Slides",
            "forms" => "Google Forms",
            _ => "Google Docs",
        };
        return (format!("web:docs.google.com/{product}"), label.into());
    }
    (format!("web:{domain}"), domain.into())
}

fn domain_matches(domain: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| domain == *candidate || domain.ends_with(&format!(".{candidate}")))
}

pub(crate) fn domain_category(domain: &str, path: &str) -> &'static str {
    if is_sensitive_domain(domain) {
        "sensitive"
    } else if domain_matches(domain, &["github.com", "gitlab.com", "bitbucket.org"]) {
        "code-hosting"
    } else if domain_matches(
        domain,
        &[
            "mail.google.com",
            "outlook.live.com",
            "outlook.office.com",
            "outlook.office365.com",
            "mail.yahoo.com",
            "app.fastmail.com",
        ],
    ) {
        "email"
    } else if domain_matches(
        domain,
        &[
            "slack.com",
            "discord.com",
            "teams.microsoft.com",
            "web.whatsapp.com",
            "messenger.com",
            "web.telegram.org",
        ],
    ) {
        "communication"
    } else if domain == "docs.google.com" {
        if path.starts_with("/spreadsheets") {
            "spreadsheet"
        } else {
            "notes"
        }
    } else if domain_matches(domain, &["airtable.com", "smartsheet.com"]) {
        "spreadsheet"
    } else if domain_matches(
        domain,
        &[
            "notion.so",
            "notion.site",
            "coda.io",
            "quip.com",
            "dropbox.com",
        ],
    ) {
        "notes"
    } else if domain_matches(domain, &["calendar.google.com", "cal.com", "calendly.com"]) {
        "calendar"
    } else if domain_matches(
        domain,
        &["youtube.com", "youtu.be", "vimeo.com", "loom.com"],
    ) {
        "video"
    } else if domain_matches(
        domain,
        &[
            "chatgpt.com",
            "chat.openai.com",
            "claude.ai",
            "gemini.google.com",
            "perplexity.ai",
            "copilot.microsoft.com",
        ],
    ) {
        "ai"
    } else if domain_matches(domain, &["figma.com", "miro.com", "canva.com"]) {
        "design"
    } else if domain_matches(
        domain,
        &[
            "linear.app",
            "atlassian.net",
            "trello.com",
            "asana.com",
            "clickup.com",
            "monday.com",
        ],
    ) {
        "project"
    } else if domain_matches(
        domain,
        &[
            "stackoverflow.com",
            "stackexchange.com",
            "developer.mozilla.org",
            "readthedocs.io",
            "docs.rs",
            "crates.io",
            "npmjs.com",
            "pypi.org",
            "wikipedia.org",
            "arxiv.org",
            "medium.com",
            "dev.to",
        ],
    ) || domain.starts_with("docs.")
        || domain.starts_with("developer.")
        || domain.contains(".docs.")
    {
        "docs"
    } else {
        "web"
    }
}

/// Financial, health, government-identity, and credential sites. Knov never
/// prepares or automates steps on them.
pub(crate) fn is_sensitive_domain(domain: &str) -> bool {
    [
        "bank",
        "paypal.",
        "venmo.",
        "wise.com",
        "coinbase.",
        "stripe.com",
        "mychart",
        "patient",
        "health",
        "irs.gov",
        "1password.",
        "bitwarden.",
        "lastpass.",
    ]
    .iter()
    .any(|marker| domain.contains(marker))
}

pub(crate) fn app_category(app: &str) -> &'static str {
    match app.to_ascii_lowercase().as_str() {
        "visual studio code" | "cursor" | "xcode" | "cortex code" | "zed" | "sublime text"
        | "intellij idea" | "pycharm" | "webstorm" | "android studio" | "nova" | "bbedit"
        | "rustrover" | "goland" | "windsurf" => "code",
        "terminal" | "iterm2" | "warp" | "ghostty" | "alacritty" | "kitty" | "wezterm"
        | "hyper" => "terminal",
        "google chrome" | "safari" | "arc" | "firefox" | "brave browser" | "microsoft edge"
        | "orion" | "vivaldi" | "opera" => "browser",
        "slack" | "discord" | "microsoft teams" | "messages" | "zoom.us" | "whatsapp"
        | "telegram" | "signal" => "communication",
        "mail" | "microsoft outlook" | "outlook" | "spark" | "superhuman" | "mimestream" => "email",
        "notion" | "obsidian" | "notes" | "bear" | "craft" | "pages" | "microsoft word"
        | "ulysses" | "logseq" => "notes",
        "numbers" | "microsoft excel" => "spreadsheet",
        "figma" | "sketch" | "pixelmator pro" | "adobe photoshop" | "affinity designer" => "design",
        "calendar" | "fantastical" | "notion calendar" => "calendar",
        "chatgpt" | "claude" => "ai",
        "preview" | "books" | "skim" | "pdf expert" => "docs",
        "linear" | "jira" | "things3" | "todoist" | "omnifocus" | "reminders" => "project",
        "1password" | "1password 7" | "bitwarden" | "keychain access" => "sensitive",
        _ => "other",
    }
}

pub(crate) fn step_title(category: &str, label: &str) -> String {
    match category {
        "code" => format!("Edit code in {label}"),
        "terminal" => format!("Run commands in {label}"),
        "code-hosting" => format!("Review on {label}"),
        "docs" => format!("Read {label}"),
        "search" => "Search the web".into(),
        "communication" => format!("Check {label}"),
        "email" => format!("Handle email in {label}"),
        "notes" => format!("Write in {label}"),
        "spreadsheet" => format!("Update {label}"),
        "calendar" => format!("Check {label}"),
        "video" => format!("Watch on {label}"),
        "ai" => format!("Ask {label}"),
        "design" => format!("Design in {label}"),
        "project" => format!("Track work in {label}"),
        "browser" => format!("Browse in {label}"),
        _ => format!("Use {label}"),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn focus(id: i64, at: i64, app: &str, seconds: i64) -> ActivityEvent {
        ActivityEvent {
            id: Some(id),
            occurred_at: at,
            ended_at: Some(at + seconds),
            duration_seconds: seconds,
            app_name: app.into(),
            window_title: Some(format!("{app} window")),
            url: None,
            page_title: None,
            search_query: None,
            browser_profile_id: None,
            source: ActivitySource::AppFocus,
            is_bootstrap: false,
        }
    }

    pub(crate) fn visit(id: i64, at: i64, url: &str) -> ActivityEvent {
        ActivityEvent {
            id: Some(id),
            occurred_at: at,
            ended_at: None,
            duration_seconds: 0,
            app_name: "Google Chrome".into(),
            window_title: None,
            url: Some(url.into()),
            page_title: Some("Private page title".into()),
            search_query: None,
            browser_profile_id: Some("Default".into()),
            source: ActivitySource::ChromeHistory,
            is_bootstrap: false,
        }
    }

    #[test]
    fn merges_repeated_focus_and_drops_noise_and_titles() {
        let events = vec![
            focus(1, 100, "Code", 120),
            focus(2, 220, "Code", 60),
            focus(3, 280, "Dock", 30),
            focus(4, 310, "Terminal", 4),
            focus(5, 320, "Terminal", 90),
        ];
        let steps = normalize_events(&events, &Settings::default());
        let keys = steps
            .iter()
            .map(|step| step.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, ["app:visual studio code", "app:terminal"]);
        assert_eq!(steps[0].seconds, 180);
        assert_eq!(steps[0].label, "Visual Studio Code");
        assert!(steps
            .iter()
            .all(|step| !step.label.contains("window") && step.locator.is_none()));
    }

    #[test]
    fn browser_focus_yields_to_page_level_steps_and_queries_are_dropped() {
        let events = vec![
            focus(1, 100, "Google Chrome", 300),
            visit(
                2,
                150,
                "https://github.com/acme/repo/issues/4?token=secret#frag",
            ),
            visit(3, 200, "https://www.google.com/search?q=private+query"),
        ];
        let steps = normalize_events(&events, &Settings::default());
        let keys = steps
            .iter()
            .map(|step| step.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, ["web:github.com", "search"]);
        assert_eq!(
            steps[0].locator.as_deref(),
            Some("https://github.com/acme/repo/issues/4")
        );
        assert_eq!(steps[0].category, "code-hosting");
        assert_eq!(steps[1].label, "Web search");
    }

    #[test]
    fn searching_inside_a_site_stays_part_of_that_site() {
        let mut youtube_search = visit(2, 200, "https://www.youtube.com/results?search_query=rust");
        youtube_search.search_query = Some("rust".into());
        let events = vec![
            visit(1, 100, "https://www.youtube.com/watch?v=abc"),
            youtube_search,
            visit(3, 300, "https://www.youtube.com/watch?v=def"),
        ];
        let steps = normalize_events(&events, &Settings::default());
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].key, "web:youtube.com");
        assert_eq!(steps[0].category, "video");
    }

    #[test]
    fn exclusions_apply_to_apps_domains_and_subdomains() {
        let events = vec![
            focus(1, 100, "Slack", 60),
            visit(2, 200, "https://app.bank.example/statement"),
            visit(3, 300, "https://docs.rs/serde"),
        ];
        let settings = Settings {
            excluded_apps: vec!["slack".into()],
            excluded_domains: vec!["bank.example".into()],
            ..Settings::default()
        };
        let steps = normalize_events(&events, &settings);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].key, "web:docs.rs");
        assert_eq!(steps[0].category, "docs");
    }

    #[test]
    fn sessions_split_on_idle_gaps() {
        let events = vec![
            focus(1, 0, "Code", 60),
            focus(2, 100, "Terminal", 60),
            focus(3, 100 + SESSION_GAP_SECONDS + 200, "Code", 60),
        ];
        let sessions = split_sessions(normalize_events(&events, &Settings::default()));
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].len(), 2);
    }

    #[test]
    fn sensitive_sites_are_classified_for_manual_handling() {
        assert_eq!(domain_category("online.mybank.com", "/"), "sensitive");
        assert_eq!(
            domain_category("docs.google.com", "/spreadsheets/d/1"),
            "spreadsheet"
        );
        assert_eq!(app_category("1Password"), "sensitive");
    }
}
