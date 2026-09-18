//! Local evidence compiler and the explicit certification/export boundary.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::OpenOptions,
    process::Command,
    sync::{Mutex, MutexGuard, OnceLock},
};

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;
use url::Url;
use uuid::Uuid;

use crate::{
    commands::AppState,
    db::Database,
    error::{AppError, AppResult},
    models::{ActivityEvent, ActivitySource},
};

pub const CERTIFICATION_STATEMENT: &str = "I reviewed this record and confirm that, to the best of my knowledge, it reasonably represents my work during this period.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Pending,
    Accepted,
    Corrected,
    Excluded,
    Personal,
    Uncertain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub color: String,
    pub status: String,
    pub aliases: Vec<String>,
    pub keywords: Vec<String>,
    pub domains: Vec<String>,
    pub repositories: Vec<String>,
    pub paths: Vec<String>,
    pub client: String,
    pub external_identifier: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    pub id: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub duration_seconds: i64,
    pub source_types: Vec<String>,
    pub application: String,
    pub sanitized_context: String,
    pub suggested_project_id: Option<String>,
    pub project_confidence: Confidence,
    pub suggested_category: String,
    pub category_confidence: Confidence,
    pub explanation: String,
    pub source_event_ids: Vec<i64>,
    pub source_available: bool,
    pub inference_method: String,
    pub review_status: ReviewStatus,
    pub project_override: Option<String>,
    pub category_override: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub categories: Vec<String>,
    pub dimensions: Vec<String>,
    pub required_fields: Vec<String>,
    pub export_columns: Vec<String>,
    pub experimental: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Allocation {
    pub project_id: Option<String>,
    pub project: String,
    pub client: String,
    pub category: String,
    pub seconds: i64,
    pub hours: f64,
    pub percentage: f64,
    pub review_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordTotals {
    pub tracked_seconds: i64,
    pub reviewed_seconds: i64,
    pub unreviewed_seconds: i64,
    pub excluded_seconds: i64,
    pub unallocated_seconds: i64,
    pub allocations: Vec<Allocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessRecord {
    pub id: String,
    pub name: String,
    pub template_id: String,
    pub start_at: i64,
    pub end_at: i64,
    pub project_ids: Vec<String>,
    pub version: i64,
    pub status: String,
    pub totals: RecordTotals,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertifiedSnapshot {
    pub schema_version: i64,
    pub record_id: String,
    pub record_version: i64,
    pub record_name: String,
    pub certified_by: String,
    pub period_start: i64,
    pub period_end: i64,
    pub template: String,
    pub certified_at: i64,
    pub certification_statement: String,
    pub totals: RecordTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Certification {
    pub id: String,
    pub record_id: String,
    pub record_version: i64,
    pub certified_at: i64,
    pub certification_statement: String,
    pub snapshot: CertifiedSnapshot,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: String,
    pub action: String,
    pub subject_id: String,
    pub occurred_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessWorkspace {
    pub projects: Vec<Project>,
    pub evidence: Vec<EvidenceItem>,
    pub templates: Vec<RecordTemplate>,
    pub records: Vec<BusinessRecord>,
    pub certifications: Vec<Certification>,
    pub audit: Vec<AuditEntry>,
    pub synthetic: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum BusinessAction {
    Refresh,
    SaveProject {
        project: Project,
    },
    DeleteProject {
        project_id: String,
    },
    Review {
        evidence_id: String,
        status: ReviewStatus,
        project_id: Option<String>,
        category: Option<String>,
    },
    Split {
        evidence_id: String,
        split_at: i64,
    },
    BulkAccept {
        evidence_ids: Vec<String>,
    },
    GenerateRecord {
        name: String,
        template_id: String,
        start_at: i64,
        end_at: i64,
        project_ids: Vec<String>,
    },
    ReviseRecord {
        record_id: String,
    },
    Certify {
        record_id: String,
        certified_by: String,
        statement_accepted: bool,
        expected_version: i64,
        expected_updated_at: i64,
    },
    DeleteCertification {
        certification_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArtifact {
    pub certification_id: String,
    pub format: String,
    pub file_name: String,
    pub content: String,
    pub sha256: String,
}

fn templates() -> Vec<RecordTemplate> {
    vec![
        RecordTemplate {
            id: "generic-allocation".into(),
            name: "Generic Project Allocation".into(),
            description: "A reusable project and category time allocation record.".into(),
            categories: vec![
                "Delivery".into(),
                "Planning".into(),
                "Internal / Administrative".into(),
                "Unallocated".into(),
            ],
            dimensions: vec!["project".into(), "category".into(), "review status".into()],
            required_fields: vec![
                "period".into(),
                "project".into(),
                "hours".into(),
                "review status".into(),
            ],
            export_columns: vec![
                "project".into(),
                "category".into(),
                "hours".into(),
                "percentage".into(),
                "review_status".into(),
            ],
            experimental: false,
        },
        RecordTemplate {
            id: "rnd-allocation".into(),
            name: "R&D Allocation — Demo".into(),
            description:
                "A prototype allocation record; it is not tax, legal, or accounting advice.".into(),
            categories: vec![
                "Direct Research".into(),
                "Direct Supervision".into(),
                "Direct Support".into(),
                "Non-R&D / Unqualified".into(),
                "Unallocated".into(),
            ],
            dimensions: vec![
                "business component".into(),
                "activity category".into(),
                "review status".into(),
            ],
            required_fields: vec![
                "period".into(),
                "business component".into(),
                "category".into(),
                "hours".into(),
            ],
            export_columns: vec![
                "project".into(),
                "category".into(),
                "hours".into(),
                "percentage".into(),
                "review_status".into(),
            ],
            experimental: true,
        },
        RecordTemplate {
            id: "professional-services".into(),
            name: "Professional Services Allocation".into(),
            description: "A thin client, matter, and billable/non-billable allocation.".into(),
            categories: vec![
                "Billable".into(),
                "Non-billable".into(),
                "Business development".into(),
                "Unallocated".into(),
            ],
            dimensions: vec![
                "client".into(),
                "project or matter".into(),
                "billing category".into(),
            ],
            required_fields: vec![
                "period".into(),
                "client".into(),
                "project".into(),
                "hours".into(),
            ],
            export_columns: vec![
                "client".into(),
                "project".into(),
                "category".into(),
                "hours".into(),
                "percentage".into(),
                "review_status".into(),
            ],
            experimental: false,
        },
    ]
}

fn json<T: Serialize>(value: &T) -> AppResult<String> {
    Ok(serde_json::to_string(value)?)
}

fn business_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}
fn decode<T: serde::de::DeserializeOwned>(raw: String) -> rusqlite::Result<T> {
    serde_json::from_str(&raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            raw.len(),
            rusqlite::types::Type::Text,
            Box::new(e),
        )
    })
}

fn projects(db: &Database) -> AppResult<Vec<Project>> {
    let conn = db.conn();
    let mut q = conn.prepare("SELECT document FROM business_projects ORDER BY updated_at DESC")?;
    let values = q
        .query_map([], |r| decode(r.get(0)?))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values)
}
fn evidence(db: &Database) -> AppResult<Vec<EvidenceItem>> {
    let conn = db.conn();
    let mut q =
        conn.prepare("SELECT document FROM business_evidence ORDER BY started_at DESC,id")?;
    let values = q
        .query_map([], |r| decode(r.get(0)?))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values)
}

fn recent_evidence(db: &Database, limit: usize) -> AppResult<Vec<EvidenceItem>> {
    let conn = db.conn();
    let mut q = conn
        .prepare("SELECT document FROM business_evidence ORDER BY started_at DESC,id LIMIT ?1")?;
    let values = q
        .query_map([limit as i64], |r| decode(r.get(0)?))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values)
}
fn records(db: &Database) -> AppResult<Vec<BusinessRecord>> {
    let conn = db.conn();
    let mut q = conn.prepare("SELECT document FROM business_records ORDER BY updated_at DESC")?;
    let values = q
        .query_map([], |r| decode(r.get(0)?))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values)
}
fn certifications(db: &Database) -> AppResult<Vec<Certification>> {
    let conn = db.conn();
    let mut q=conn.prepare("SELECT id,record_id,record_version,certified_at,snapshot_json,sha256 FROM business_certifications ORDER BY certified_at DESC")?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(
            |(id, record_id, record_version, certified_at, canonical, sha256)| {
                let actual = format!("{:x}", Sha256::digest(canonical.as_bytes()));
                if actual != sha256 {
                    return Err(AppError::InvalidInput(
                        "A certified snapshot failed its integrity check.".into(),
                    ));
                }
                let snapshot: CertifiedSnapshot = serde_json::from_str(&canonical)?;
                if snapshot.record_id != record_id
                    || snapshot.record_version != record_version
                    || snapshot.certified_at != certified_at
                {
                    return Err(AppError::InvalidInput(
                        "Certified snapshot metadata is inconsistent.".into(),
                    ));
                }
                Ok(Certification {
                    id,
                    record_id,
                    record_version,
                    certified_at,
                    certification_statement: snapshot.certification_statement.clone(),
                    snapshot,
                    sha256,
                })
            },
        )
        .collect()
}
fn audit(db: &Database) -> AppResult<Vec<AuditEntry>> {
    let conn = db.conn();
    let mut q=conn.prepare("SELECT id,action,subject_id,occurred_at FROM business_audit ORDER BY occurred_at DESC,id DESC LIMIT 250")?;
    let values = q
        .query_map([], |r| {
            Ok(AuditEntry {
                id: r.get(0)?,
                action: r.get(1)?,
                subject_id: r.get(2)?,
                occurred_at: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values)
}

fn raw_events(db: &Database) -> AppResult<Vec<ActivityEvent>> {
    let conn = db.conn();
    let mut q=conn.prepare("SELECT id,occurred_at,ended_at,duration_seconds,app_name,window_title,url,page_title,search_query,browser_profile_id,source,is_bootstrap FROM activity_events ORDER BY occurred_at,id")?;
    let rows = q.query_map([], |r| {
        let source: String = r.get(10)?;
        Ok(ActivityEvent {
            id: r.get(0)?,
            occurred_at: r.get(1)?,
            ended_at: r.get(2)?,
            duration_seconds: r.get(3)?,
            app_name: r.get(4)?,
            window_title: r.get(5)?,
            url: r.get(6)?,
            page_title: r.get(7)?,
            search_query: r.get(8)?,
            browser_profile_id: r.get(9)?,
            source: ActivitySource::try_from(source.as_str()).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    source.len(),
                    rusqlite::types::Type::Text,
                    e.into(),
                )
            })?,
            is_bootstrap: r.get::<_, i64>(11)? != 0,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn domain(value: Option<&str>) -> Option<String> {
    value.and_then(|v| Url::parse(v).ok()).and_then(|u| {
        u.host_str()
            .map(|h| h.trim_start_matches("www.").to_ascii_lowercase())
    })
}
fn excluded(event: &ActivityEvent, apps: &[String], domains: &[String]) -> bool {
    apps.iter()
        .any(|v| event.app_name.eq_ignore_ascii_case(v.trim()))
        || domain(event.url.as_deref()).is_some_and(|host| {
            domains.iter().any(|v| {
                host == v.trim().trim_start_matches("www.").to_ascii_lowercase()
                    || host.ends_with(&format!(
                        ".{}",
                        v.trim().trim_start_matches("www.").to_ascii_lowercase()
                    ))
            })
        })
}
fn safe_context(event: &ActivityEvent) -> String {
    if let Some(url) = event.url.as_deref().and_then(|v| Url::parse(v).ok()) {
        return url
            .host_str()
            .map(|v| v.trim_start_matches("www.").to_string())
            .unwrap_or_default();
    }
    let raw = event
        .page_title
        .as_deref()
        .or(event.window_title.as_deref())
        .unwrap_or("");
    sanitize_text(raw)
}
fn sanitize_text(raw: &str) -> String {
    let mut out = raw
        .split_whitespace()
        .map(|token| {
            let low = token.to_ascii_lowercase();
            if (token.starts_with('/') && token != "/")
                || token.contains("\\")
                || low.contains("http://")
                || low.contains("https://")
                || low.contains("token=")
                || low.contains("password=")
                || low.contains("api_key")
                || low.contains("apikey")
                || low.contains("secret=")
                || low.starts_with("sk-")
            {
                "[redacted]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if out.len() > 160 {
        out.truncate(160);
    }
    out
}
fn event_text(event: &ActivityEvent, topic: Option<&String>) -> String {
    [
        Some(event.app_name.as_str()),
        event.window_title.as_deref(),
        event.page_title.as_deref(),
        event.url.as_deref(),
        event.search_query.as_deref(),
        topic.map(String::as_str),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_ascii_lowercase()
}

fn classify(
    event: &ActivityEvent,
    projects: &[Project],
    topic: Option<&String>,
) -> (Option<String>, Confidence, String, String, Confidence) {
    let text = event_text(event, topic);
    let host = domain(event.url.as_deref());
    let mut scored = Vec::new();
    for p in projects.iter().filter(|p| p.status == "active") {
        let mut score = 0;
        let mut why = Vec::new();
        if p.repositories
            .iter()
            .any(|v| !v.is_empty() && text.contains(&v.to_ascii_lowercase()))
        {
            score += 6;
            why.push("repository");
        }
        if p.paths
            .iter()
            .any(|v| !v.is_empty() && text.contains(&v.to_ascii_lowercase()))
        {
            score += 5;
            why.push("path");
        }
        if host.as_ref().is_some_and(|h| {
            p.domains.iter().any(|v| {
                h == &v.to_ascii_lowercase() || h.ends_with(&format!(".{}", v.to_ascii_lowercase()))
            })
        }) {
            score += 5;
            why.push("domain");
        }
        if p.aliases
            .iter()
            .chain(std::iter::once(&p.name))
            .any(|v| v.trim().len() >= 2 && text.contains(&v.to_ascii_lowercase()))
        {
            score += 3;
            why.push("alias");
        }
        if p.keywords
            .iter()
            .any(|v| !v.is_empty() && text.contains(&v.to_ascii_lowercase()))
        {
            score += 2;
            why.push("keyword");
        }
        if topic.is_some_and(|t| {
            p.aliases
                .iter()
                .chain(std::iter::once(&p.name))
                .any(|v| t.to_ascii_lowercase().contains(&v.to_ascii_lowercase()))
        }) {
            score += 2;
            why.push("work thread");
        }
        if score > 0 {
            scored.push((score, p.id.clone(), why));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let (project, confidence, explanation) = match scored.first() {
        None => (
            None,
            Confidence::Low,
            "No deterministic project signal matched; left unallocated.".into(),
        ),
        Some((score, _, _)) if scored.get(1).is_some_and(|v| v.0 == *score) => (
            None,
            Confidence::Low,
            "Conflicting project signals matched equally; left unallocated.".into(),
        ),
        Some((score, id, why)) if *score >= 5 => (
            Some(id.clone()),
            Confidence::High,
            format!("Local {} signal matched.", why.join(" + ")),
        ),
        Some((_, id, why)) => (
            Some(id.clone()),
            Confidence::Medium,
            format!(
                "Local {} signal matched; review recommended.",
                why.join(" + ")
            ),
        ),
    };
    let (category, cc) =
        if text.contains("research") || text.contains("experiment") || text.contains("prototype") {
            ("Direct Research", Confidence::High)
        } else if text.contains("review") || text.contains("support") {
            ("Direct Support", Confidence::Medium)
        } else if project.is_some() {
            ("Delivery", Confidence::Medium)
        } else {
            ("Unallocated", Confidence::Low)
        };
    (project, confidence, category.into(), explanation, cc)
}

fn intervals_subtract(start: i64, end: i64, covers: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut parts = vec![(start, end)];
    for &(a, b) in covers {
        let mut next = Vec::new();
        for (x, y) in parts {
            if b <= x || a >= y {
                next.push((x, y));
            } else {
                if x < a {
                    next.push((x, a));
                }
                if b < y {
                    next.push((b, y));
                }
            }
        }
        parts = next;
    }
    parts
}

pub fn refresh_evidence(db: &Database) -> AppResult<()> {
    let settings = db.settings()?;
    let project_list = projects(db)?;
    let events = raw_events(db)?;
    let topics = crate::threading::semantic_topics(&events);
    let active: Vec<(i64, i64)> = events
        .iter()
        .filter(|e| {
            e.source == ActivitySource::AppFocus
                && !excluded(e, &settings.excluded_apps, &settings.excluded_domains)
        })
        .map(|e| {
            (
                e.occurred_at,
                e.ended_at
                    .unwrap_or(e.occurred_at + e.duration_seconds.max(0)),
            )
        })
        .collect();
    let existing = evidence(db)?;
    let events_by_id: HashMap<i64, &ActivityEvent> = events
        .iter()
        .filter_map(|event| event.id.map(|id| (id, event)))
        .collect();
    let covered: HashSet<i64> = existing
        .iter()
        .flat_map(|e| e.source_event_ids.iter().copied())
        .collect();
    let now = Utc::now().timestamp();
    let mut generated = Vec::new();
    for event in &events {
        let Some(source_id) = event.id else { continue };
        if excluded(event, &settings.excluded_apps, &settings.excluded_domains) {
            continue;
        }
        let end = event
            .ended_at
            .unwrap_or(event.occurred_at + event.duration_seconds.max(0));
        let ranges = match event.source {
            ActivitySource::AppFocus => vec![(event.occurred_at, end)],
            ActivitySource::ChromeExtension => intervals_subtract(event.occurred_at, end, &active),
            _ => vec![(event.occurred_at, event.occurred_at)],
        };
        for (start, stop) in ranges {
            if covered.contains(&source_id) {
                continue;
            }
            let (pid, pc, cat, explain, cc) =
                classify(event, &project_list, topics.get(&source_id));
            let id = if ranges_len(event, &active) > 1 {
                format!("e-{source_id}-{start}-{stop}")
            } else {
                format!("e-{source_id}")
            };
            generated.push(EvidenceItem {
                id,
                started_at: start,
                ended_at: stop,
                duration_seconds: (stop - start).max(0),
                source_types: vec![event.source.as_str().into()],
                application: sanitize_text(&event.app_name),
                sanitized_context: safe_context(event),
                suggested_project_id: pid,
                project_confidence: pc,
                suggested_category: cat,
                category_confidence: cc,
                explanation: explain,
                source_event_ids: vec![source_id],
                source_available: true,
                inference_method: "local".into(),
                review_status: ReviewStatus::Pending,
                project_override: None,
                category_override: None,
                created_at: now,
                updated_at: now,
            });
        }
    }
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    for mut item in existing {
        let previous = json(&item)?;
        if let Some(event) = item
            .source_event_ids
            .iter()
            .find_map(|source_id| events_by_id.get(source_id).copied())
        {
            let source_id = event.id.unwrap();
            let is_unsplit_source = item.id == format!("e-{source_id}");
            if is_unsplit_source {
                let end = event
                    .ended_at
                    .unwrap_or(event.occurred_at + event.duration_seconds.max(0));
                let duration_changed = item.started_at != event.occurred_at
                    || item.ended_at != end
                    || item.duration_seconds != (end - event.occurred_at).max(0);
                item.started_at = event.occurred_at;
                item.ended_at = end;
                item.duration_seconds = (end - event.occurred_at).max(0);
                if duration_changed
                    && matches!(
                        item.review_status,
                        ReviewStatus::Accepted | ReviewStatus::Corrected
                    )
                {
                    item.review_status = ReviewStatus::Pending;
                }
            }
            if excluded(event, &settings.excluded_apps, &settings.excluded_domains) {
                item.review_status = ReviewStatus::Excluded;
                item.updated_at = now;
            } else if matches!(
                item.review_status,
                ReviewStatus::Pending | ReviewStatus::Uncertain
            ) && item.project_override.is_none()
                && item.category_override.is_none()
            {
                let (project, confidence, category, explanation, category_confidence) =
                    classify(event, &project_list, topics.get(&source_id));
                item.suggested_project_id = project;
                item.project_confidence = confidence;
                item.suggested_category = category;
                item.category_confidence = category_confidence;
                item.explanation = explanation;
                item.updated_at = now;
            }
            let document = json(&item)?;
            if document != previous {
                tx.execute(
                    "UPDATE business_evidence SET started_at=?2,ended_at=?3,duration_seconds=?4,document=?5,updated_at=?6 WHERE id=?1",
                    params![item.id, item.started_at, item.ended_at, item.duration_seconds, document, item.updated_at],
                )?;
            }
        }
    }
    for item in generated {
        tx.execute("INSERT OR IGNORE INTO business_evidence(id,started_at,ended_at,duration_seconds,document,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6)",params![item.id,item.started_at,item.ended_at,item.duration_seconds,json(&item)?,now])?;
    }
    tx.commit()?;
    Ok(())
}
fn ranges_len(event: &ActivityEvent, active: &[(i64, i64)]) -> usize {
    if event.source == ActivitySource::ChromeExtension {
        intervals_subtract(
            event.occurred_at,
            event
                .ended_at
                .unwrap_or(event.occurred_at + event.duration_seconds.max(0)),
            active,
        )
        .len()
    } else {
        1
    }
}

pub(crate) fn scrub_expired_evidence_with_conn(conn: &rusqlite::Connection) -> AppResult<()> {
    let available_source_ids = {
        let mut ids = conn.prepare("SELECT id FROM activity_events")?;
        let values = ids
            .query_map([], |row| row.get(0))?
            .collect::<Result<HashSet<i64>, _>>()?;
        values
    };
    let mut q =
        conn.prepare("SELECT document FROM business_evidence ORDER BY started_at DESC,id")?;
    let existing = q
        .query_map([], |r| decode(r.get(0)?))?
        .collect::<Result<Vec<EvidenceItem>, _>>()?;
    drop(q);
    for mut item in existing {
        let old_count = item.source_event_ids.len();
        let mut available_ids = Vec::new();
        for source_id in &item.source_event_ids {
            if available_source_ids.contains(source_id) {
                available_ids.push(*source_id);
            }
        }
        if available_ids.len() != old_count && item.source_available {
            item.source_event_ids = available_ids;
            item.source_available = false;
            item.source_types.clear();
            item.application.clear();
            item.sanitized_context.clear();
            item.explanation =
                "Underlying detailed evidence expired under local retention policy.".into();
            item.updated_at = Utc::now().timestamp();
            conn.execute(
                "UPDATE business_evidence SET document=?2,updated_at=?3 WHERE id=?1",
                params![item.id, json(&item)?, item.updated_at],
            )?;
        }
    }
    Ok(())
}

fn effective_project(e: &EvidenceItem) -> Option<String> {
    e.project_override
        .clone()
        .or_else(|| e.suggested_project_id.clone())
}
fn effective_category(e: &EvidenceItem) -> String {
    e.category_override
        .clone()
        .unwrap_or_else(|| e.suggested_category.clone())
}
fn totals(
    record: &BusinessRecord,
    evidence: &[EvidenceItem],
    projects: &[Project],
) -> RecordTotals {
    let rows: Vec<&EvidenceItem> = evidence
        .iter()
        .filter(|e| e.started_at < record.end_at && e.ended_at > record.start_at)
        .collect();
    let seconds_in_range = |e: &EvidenceItem| {
        e.ended_at
            .min(record.end_at)
            .saturating_sub(e.started_at.max(record.start_at))
    };
    let tracked: i64 = rows.iter().map(|e| seconds_in_range(e)).sum();
    let allowed_categories = templates()
        .into_iter()
        .find(|template| template.id == record.template_id)
        .map(|template| template.categories)
        .unwrap_or_default();
    let excluded: i64 = rows
        .iter()
        .filter(|e| {
            matches!(
                e.review_status,
                ReviewStatus::Excluded | ReviewStatus::Personal
            )
        })
        .map(|e| seconds_in_range(e))
        .sum();
    let reviewed = rows
        .iter()
        .filter(|e| {
            matches!(
                e.review_status,
                ReviewStatus::Accepted | ReviewStatus::Corrected
            )
        })
        .map(|e| seconds_in_range(e))
        .sum();
    let unallocated = rows
        .iter()
        .filter(|e| {
            !matches!(
                e.review_status,
                ReviewStatus::Excluded | ReviewStatus::Personal
            ) && (effective_project(e).is_none()
                || effective_category(e) == "Unallocated"
                || !allowed_categories.contains(&effective_category(e))
                || (!record.project_ids.is_empty()
                    && effective_project(e)
                        .as_ref()
                        .is_some_and(|id| !record.project_ids.contains(id))))
        })
        .map(|e| seconds_in_range(e))
        .sum();
    let mut grouped: BTreeMap<(Option<String>, String, String), i64> = BTreeMap::new();
    for e in rows.iter().filter(|e| {
        !matches!(
            e.review_status,
            ReviewStatus::Excluded | ReviewStatus::Personal
        )
    }) {
        let resolved = effective_project(e);
        let category = effective_category(e);
        let valid = resolved.is_some()
            && (record.project_ids.is_empty()
                || resolved
                    .as_ref()
                    .is_some_and(|id| record.project_ids.contains(id)))
            && category != "Unallocated"
            && allowed_categories.contains(&category);
        let pid = if valid { resolved } else { None };
        let cat = if valid {
            category
        } else {
            "Unallocated".into()
        };
        let status = match e.review_status {
            ReviewStatus::Accepted | ReviewStatus::Corrected => "reviewed",
            _ => "unreviewed",
        }
        .to_string();
        *grouped.entry((pid, cat, status)).or_default() += seconds_in_range(e);
    }
    let denominator = (tracked - excluded).max(0);
    let allocations = grouped
        .into_iter()
        .map(|((pid, category, status), seconds)| {
            let p = pid
                .as_ref()
                .and_then(|id| projects.iter().find(|p| &p.id == id));
            Allocation {
                project_id: pid,
                project: p
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "Unallocated".into()),
                client: p.map(|p| p.client.clone()).unwrap_or_default(),
                category,
                seconds,
                hours: seconds as f64 / 3600.0,
                percentage: if denominator == 0 {
                    0.0
                } else {
                    seconds as f64 * 100.0 / denominator as f64
                },
                review_status: status,
            }
        })
        .collect();
    RecordTotals {
        tracked_seconds: tracked,
        reviewed_seconds: reviewed,
        unreviewed_seconds: tracked - reviewed - excluded,
        excluded_seconds: excluded,
        unallocated_seconds: unallocated,
        allocations,
    }
}

fn recompute_drafts(db: &Database) -> AppResult<Vec<BusinessRecord>> {
    let mut rs = records(db)?;
    if rs.is_empty() {
        return Ok(rs);
    }
    let ev = evidence(db)?;
    let ps = projects(db)?;
    let conn = db.conn();
    for r in &mut rs {
        if r.status != "certified" && r.status != "exported" {
            let previous = json(r)?;
            r.totals = totals(r, &ev, &ps);
            r.status = if r.totals.unreviewed_seconds > 0 || r.totals.unallocated_seconds > 0 {
                "needs_review"
            } else {
                "ready"
            }
            .into();
            if json(r)? != previous {
                r.updated_at = Utc::now().timestamp().max(r.updated_at.saturating_add(1));
                conn.execute(
                    "UPDATE business_records SET document=?2,updated_at=?3 WHERE id=?1",
                    params![r.id, json(r)?, r.updated_at],
                )?;
            }
        }
    }
    Ok(rs)
}

fn workspace(db: &Database) -> AppResult<BusinessWorkspace> {
    Ok(BusinessWorkspace {
        projects: projects(db)?,
        // Keep IPC and React rendering bounded. Record aggregation still uses all
        // retained evidence in `recompute_drafts` and native mutations.
        evidence: recent_evidence(db, 500)?,
        templates: templates(),
        records: recompute_drafts(db)?,
        certifications: certifications(db)?,
        audit: audit(db)?,
        synthetic: false,
    })
}

fn load_workspace(db: &Database) -> AppResult<BusinessWorkspace> {
    let evidence_exists: bool = db.conn().query_row(
        "SELECT EXISTS(SELECT 1 FROM business_evidence LIMIT 1)",
        [],
        |row| row.get(0),
    )?;
    if !evidence_exists {
        refresh_evidence(db)?;
    }
    workspace(db)
}

#[tauri::command]
pub fn business_workspace(state: State<'_, AppState>) -> AppResult<BusinessWorkspace> {
    let _guard = business_guard();
    load_workspace(&state.db)
}

fn audit_tx(
    tx: &rusqlite::Transaction<'_>,
    action: &str,
    subject: &str,
    now: i64,
) -> AppResult<()> {
    tx.execute(
        "INSERT INTO business_audit(id,action,subject_id,occurred_at) VALUES(?1,?2,?3,?4)",
        params![Uuid::new_v4().to_string(), action, subject, now],
    )?;
    Ok(())
}

pub fn apply_action(db: &Database, action: BusinessAction) -> AppResult<BusinessWorkspace> {
    let _guard = business_guard();
    let now = Utc::now().timestamp();
    // Compiling evidence walks the complete retained activity history. Routine
    // mutations already operate on compiled evidence, so repeating that scan on
    // every click only adds latency. Project changes still need reclassification;
    // the explicit refresh action remains the boundary for newly collected data.
    let refresh_after_action = matches!(
        &action,
        BusinessAction::Refresh | BusinessAction::SaveProject { .. }
    );
    match action {
        BusinessAction::Refresh => {}
        BusinessAction::SaveProject { mut project } => {
            if project.name.trim().is_empty() {
                return Err(AppError::InvalidInput("Project name is required.".into()));
            }
            if project.id.is_empty() {
                project.id = Uuid::new_v4().to_string();
                project.created_at = now;
            }
            project.updated_at = now;
            if project.status != "active" && project.status != "archived" {
                return Err(AppError::InvalidInput(
                    "Project status must be active or archived.".into(),
                ));
            }
            let mut conn = db.conn();
            let tx = conn.transaction()?;
            let existed: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM business_projects WHERE id=?1)",
                [&project.id],
                |r| r.get(0),
            )?;
            tx.execute("INSERT INTO business_projects(id,document,created_at,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET document=excluded.document,updated_at=excluded.updated_at",params![project.id,json(&project)?,project.created_at,project.updated_at])?;
            audit_tx(
                &tx,
                if existed {
                    "project_changed"
                } else {
                    "project_created"
                },
                &project.id,
                now,
            )?;
            tx.commit()?;
        }
        BusinessAction::DeleteProject { project_id } => {
            let conn = db.conn();
            let referenced:i64=conn.query_row("SELECT COUNT(*) FROM business_evidence WHERE json_extract(document,'$.projectOverride')=?1 OR json_extract(document,'$.suggestedProjectId')=?1",[&project_id],|r|r.get(0))?;
            let in_records:i64=conn.query_row("SELECT COUNT(*) FROM business_records WHERE EXISTS(SELECT 1 FROM json_each(json_extract(document,'$.projectIds')) WHERE value=?1)",[&project_id],|r|r.get(0))?;
            if referenced + in_records > 0 {
                return Err(AppError::InvalidInput(
                    "Archive this project instead; it is referenced by evidence or records.".into(),
                ));
            }
            let mut conn = conn;
            let tx = conn.transaction()?;
            let changed = tx.execute("DELETE FROM business_projects WHERE id=?1", [&project_id])?;
            if changed == 0 {
                return Err(AppError::InvalidInput("Project not found.".into()));
            }
            audit_tx(&tx, "project_deleted", &project_id, now)?;
            tx.commit()?;
        }
        BusinessAction::Review {
            evidence_id,
            status,
            project_id,
            category,
        } => {
            if let Some(ref p) = project_id {
                if !projects(db)?.iter().any(|v| &v.id == p) {
                    return Err(AppError::InvalidInput(
                        "Selected project does not exist.".into(),
                    ));
                }
            }
            let mut item = evidence(db)?
                .into_iter()
                .find(|e| e.id == evidence_id)
                .ok_or_else(|| AppError::InvalidInput("Evidence item not found.".into()))?;
            item.review_status = status.clone();
            item.project_override = project_id;
            item.category_override = category.filter(|v| !v.trim().is_empty());
            item.updated_at = now;
            let action = if status == ReviewStatus::Excluded || status == ReviewStatus::Personal {
                "evidence_excluded"
            } else if item.project_override.is_some() || item.category_override.is_some() {
                "inference_overridden"
            } else {
                "inference_accepted"
            };
            let mut conn = db.conn();
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE business_evidence SET document=?2,updated_at=?3 WHERE id=?1",
                params![item.id, json(&item)?, now],
            )?;
            audit_tx(&tx, action, &item.id, now)?;
            tx.commit()?;
        }
        BusinessAction::BulkAccept { evidence_ids } => {
            let wanted: HashSet<_> = evidence_ids.into_iter().collect();
            let items = evidence(db)?;
            let mut conn = db.conn();
            let tx = conn.transaction()?;
            for mut item in items.into_iter().filter(|e| {
                wanted.contains(&e.id)
                    && e.project_confidence == Confidence::High
                    && e.suggested_project_id.is_some()
            }) {
                item.review_status = ReviewStatus::Accepted;
                item.updated_at = now;
                tx.execute(
                    "UPDATE business_evidence SET document=?2,updated_at=?3 WHERE id=?1",
                    params![item.id, json(&item)?, now],
                )?;
                audit_tx(&tx, "inference_accepted", &item.id, now)?;
            }
            tx.commit()?;
        }
        BusinessAction::Split {
            evidence_id,
            split_at,
        } => {
            let item = evidence(db)?
                .into_iter()
                .find(|e| e.id == evidence_id)
                .ok_or_else(|| AppError::InvalidInput("Evidence item not found.".into()))?;
            if split_at <= item.started_at || split_at >= item.ended_at {
                return Err(AppError::InvalidInput(
                    "Split point must be inside the evidence time range.".into(),
                ));
            }
            let mut left = item.clone();
            left.id = Uuid::new_v4().to_string();
            left.ended_at = split_at;
            left.duration_seconds = split_at - left.started_at;
            left.updated_at = now;
            let mut right = item.clone();
            right.id = Uuid::new_v4().to_string();
            right.started_at = split_at;
            right.duration_seconds = right.ended_at - split_at;
            right.updated_at = now;
            let mut conn = db.conn();
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM business_evidence WHERE id=?1", [item.id])?;
            for e in [&left, &right] {
                tx.execute("INSERT INTO business_evidence(id,started_at,ended_at,duration_seconds,document,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![e.id,e.started_at,e.ended_at,e.duration_seconds,json(e)?,e.created_at,e.updated_at])?;
            }
            audit_tx(&tx, "evidence_split", &evidence_id, now)?;
            tx.commit()?;
        }
        BusinessAction::GenerateRecord {
            name,
            template_id,
            start_at,
            end_at,
            project_ids,
        } => {
            if name.trim().is_empty()
                || start_at >= end_at
                || !templates().iter().any(|t| t.id == template_id)
            {
                return Err(AppError::InvalidInput(
                    "A name, valid template, and non-empty time range are required.".into(),
                ));
            }
            let mut record = BusinessRecord {
                id: Uuid::new_v4().to_string(),
                name,
                template_id,
                start_at,
                end_at,
                project_ids,
                version: 1,
                status: "draft".into(),
                totals: RecordTotals::default(),
                created_at: now,
                updated_at: now,
            };
            record.totals = totals(&record, &evidence(db)?, &projects(db)?);
            record.status =
                if record.totals.unreviewed_seconds > 0 || record.totals.unallocated_seconds > 0 {
                    "needs_review"
                } else {
                    "ready"
                }
                .into();
            let mut conn = db.conn();
            let tx = conn.transaction()?;
            tx.execute("INSERT INTO business_records(id,document,created_at,updated_at) VALUES(?1,?2,?3,?4)",params![record.id,json(&record)?,now,now])?;
            audit_tx(&tx, "record_generated", &record.id, now)?;
            tx.commit()?;
        }
        BusinessAction::ReviseRecord { record_id } => {
            let mut record = records(db)?
                .into_iter()
                .find(|r| r.id == record_id)
                .ok_or_else(|| AppError::InvalidInput("Record not found.".into()))?;
            if record.status != "certified" && record.status != "exported" {
                return Err(AppError::InvalidInput(
                    "Only a certified record needs a new revision.".into(),
                ));
            }
            record.version += 1;
            record.status = "draft".into();
            record.updated_at = now;
            record.totals = totals(&record, &evidence(db)?, &projects(db)?);
            record.status =
                if record.totals.unreviewed_seconds > 0 || record.totals.unallocated_seconds > 0 {
                    "needs_review"
                } else {
                    "ready"
                }
                .into();
            let mut conn = db.conn();
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE business_records SET document=?2,updated_at=?3 WHERE id=?1",
                params![record.id, json(&record)?, now],
            )?;
            audit_tx(&tx, "record_superseded", &record.id, now)?;
            tx.commit()?;
        }
        BusinessAction::Certify {
            record_id,
            certified_by,
            statement_accepted,
            expected_version,
            expected_updated_at,
        } => certify(
            db,
            &record_id,
            &certified_by,
            statement_accepted,
            expected_version,
            expected_updated_at,
            now,
        )?,
        BusinessAction::DeleteCertification { certification_id } => {
            let mut conn = db.conn();
            let tx = conn.transaction()?;
            let linked: Option<(String, i64)> = tx
                .query_row(
                    "SELECT record_id,record_version FROM business_certifications WHERE id=?1",
                    [&certification_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let changed = tx.execute(
                "DELETE FROM business_certifications WHERE id=?1",
                [&certification_id],
            )?;
            if changed == 0 {
                return Err(AppError::InvalidInput("Certification not found.".into()));
            }
            if let Some((record_id, version)) = linked {
                let raw: Option<String> = tx
                    .query_row(
                        "SELECT document FROM business_records WHERE id=?1",
                        [&record_id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(raw) = raw {
                    let mut record: BusinessRecord = serde_json::from_str(&raw)?;
                    if record.version == version
                        && matches!(record.status.as_str(), "certified" | "exported")
                    {
                        record.status = "draft".into();
                        record.updated_at = now.max(record.updated_at.saturating_add(1));
                        tx.execute(
                            "UPDATE business_records SET document=?2,updated_at=?3 WHERE id=?1",
                            params![record.id, json(&record)?, record.updated_at],
                        )?;
                    }
                }
            }
            audit_tx(&tx, "certification_deleted", &certification_id, now)?;
            tx.commit()?;
        }
    }
    if refresh_after_action {
        refresh_evidence(db)?;
    }
    workspace(db)
}

#[tauri::command]
pub fn business_action(
    state: State<'_, AppState>,
    request: BusinessAction,
) -> AppResult<BusinessWorkspace> {
    apply_action(&state.db, request)
}

fn certify(
    db: &Database,
    record_id: &str,
    certified_by: &str,
    accepted: bool,
    version: i64,
    updated: i64,
    now: i64,
) -> AppResult<()> {
    if !accepted || certified_by.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "Certification requires an attestor and explicit acceptance of the statement.".into(),
        ));
    }
    let mut record = records(db)?
        .into_iter()
        .find(|r| r.id == record_id)
        .ok_or_else(|| AppError::InvalidInput("Record not found.".into()))?;
    if record.version != version || record.updated_at != updated {
        return Err(AppError::InvalidInput(
            "The draft changed. Review the current version before certifying.".into(),
        ));
    }
    if record.status != "ready" {
        return Err(AppError::InvalidInput(
            "Only the current ready draft can be certified.".into(),
        ));
    }
    if certifications(db)?
        .iter()
        .any(|c| c.record_id == record.id && c.record_version == record.version)
    {
        return Err(AppError::InvalidInput(
            "This record version is already certified.".into(),
        ));
    }
    record.totals = totals(&record, &evidence(db)?, &projects(db)?);
    if record.totals.reviewed_seconds <= 0
        || record.totals.unreviewed_seconds > 0
        || record.totals.unallocated_seconds > 0
    {
        return Err(AppError::InvalidInput("Review or exclude all tracked time and resolve every project/category before certifying.".into()));
    }
    let template = templates()
        .into_iter()
        .find(|t| t.id == record.template_id)
        .map(|t| t.name)
        .unwrap_or(record.template_id.clone());
    let snapshot = CertifiedSnapshot {
        schema_version: 1,
        record_id: record.id.clone(),
        record_version: record.version,
        record_name: sanitize_text(&record.name),
        certified_by: sanitize_text(certified_by.trim()),
        period_start: record.start_at,
        period_end: record.end_at,
        template: sanitize_text(&template),
        certified_at: now,
        certification_statement: CERTIFICATION_STATEMENT.into(),
        totals: sanitize_totals(&record.totals),
    };
    let canonical = json(&snapshot)?;
    let hash = format!("{:x}", Sha256::digest(canonical.as_bytes()));
    let id = Uuid::new_v4().to_string();
    record.status = "certified".into();
    record.updated_at = now;
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    tx.execute("INSERT INTO business_certifications(id,record_id,record_version,certified_at,snapshot_json,sha256) VALUES(?1,?2,?3,?4,?5,?6)",params![id,record.id,record.version,now,canonical,hash])?;
    tx.execute(
        "UPDATE business_records SET document=?2,updated_at=?3 WHERE id=?1",
        params![record.id, json(&record)?, now],
    )?;
    audit_tx(&tx, "record_certified", &record.id, now)?;
    tx.commit()?;
    Ok(())
}
fn sanitize_totals(value: &RecordTotals) -> RecordTotals {
    let mut v = value.clone();
    for a in &mut v.allocations {
        a.project = sanitize_text(&a.project);
        a.client = sanitize_text(&a.client);
        a.category = sanitize_text(&a.category);
        a.review_status = sanitize_text(&a.review_status);
    }
    v
}

fn csv_cell(v: &str) -> String {
    let safe = if matches!(v.chars().next(), Some('=' | '+' | '-' | '@' | '\t' | '\r')) {
        format!("'{v}")
    } else {
        v.to_string()
    };
    format!("\"{}\"", safe.replace('"', "\"\""))
}
fn artifact(db: &Database, id: &str, format: &str) -> AppResult<ExportArtifact> {
    let cert = certifications(db)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::InvalidInput("Certification not found.".into()))?;
    let content = match format {
        "json" => {
            let mut value = serde_json::to_value(&cert.snapshot)?;
            let object = value
                .as_object_mut()
                .ok_or_else(|| AppError::InvalidInput("Certified snapshot is invalid.".into()))?;
            object.insert(
                "certificationId".into(),
                serde_json::Value::String(cert.id.clone()),
            );
            object.insert(
                "integrityHash".into(),
                serde_json::Value::String(cert.sha256.clone()),
            );
            serde_json::to_string_pretty(&value)?
        }
        "csv" => {
            let mut out="certificationId,recordId,recordVersion,recordName,certifiedBy,periodStart,periodEnd,template,certifiedAt,certificationStatement,project,client,category,hours,percentage,reviewStatus,integrityHash\n".to_string();
            for a in &cert.snapshot.totals.allocations {
                let cells = vec![
                    cert.id.clone(),
                    cert.snapshot.record_id.clone(),
                    cert.snapshot.record_version.to_string(),
                    cert.snapshot.record_name.clone(),
                    cert.snapshot.certified_by.clone(),
                    cert.snapshot.period_start.to_string(),
                    cert.snapshot.period_end.to_string(),
                    cert.snapshot.template.clone(),
                    cert.snapshot.certified_at.to_string(),
                    cert.snapshot.certification_statement.clone(),
                    a.project.clone(),
                    a.client.clone(),
                    a.category.clone(),
                    format!("{:.4}", a.hours),
                    format!("{:.2}", a.percentage),
                    a.review_status.clone(),
                    cert.sha256.clone(),
                ];
                out.push_str(
                    &cells
                        .iter()
                        .map(|v| csv_cell(v))
                        .collect::<Vec<_>>()
                        .join(","),
                );
                out.push('\n');
            }
            out
        }
        _ => {
            return Err(AppError::InvalidInput(
                "Export format must be csv or json.".into(),
            ))
        }
    };
    let sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
    Ok(ExportArtifact {
        certification_id: cert.id.clone(),
        format: format.into(),
        file_name: format!(
            "knov-{}-v{}.{}",
            cert.record_id, cert.record_version, format
        ),
        content,
        sha256,
    })
}

#[tauri::command]
pub fn preview_business_export(
    state: State<'_, AppState>,
    certification_id: String,
    format: String,
) -> AppResult<ExportArtifact> {
    let _guard = business_guard();
    artifact(&state.db, &certification_id, &format)
}

#[tauri::command]
pub fn save_business_export(
    state: State<'_, AppState>,
    certification_id: String,
    format: String,
    expected_content: String,
) -> AppResult<Option<String>> {
    let _guard = business_guard();
    let value = artifact(&state.db, &certification_id, &format)?;
    if value.content != expected_content {
        return Err(AppError::InvalidInput(
            "Export preview changed; preview it again before saving.".into(),
        ));
    }
    #[cfg(target_os = "macos")]
    let path = {
        let script="on run argv\nset chosenFile to choose file name with prompt \"Save certified business record\" default name (item 1 of argv)\nreturn POSIX path of chosenFile\nend run";
        let output = Command::new("osascript")
            .args(["-e", script, "--", &value.file_name])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    #[cfg(not(target_os = "macos"))]
    let path = {
        return Err(AppError::InvalidInput(
            "Native save dialog is currently available on macOS.".into(),
        ));
    };
    if path.is_empty() {
        return Ok(None);
    }
    if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(AppError::InvalidInput(
            "Refusing to overwrite a symbolic link.".into(),
        ));
    }
    let current = artifact(&state.db, &certification_id, &format)?;
    if current.content != expected_content || current.sha256 != value.sha256 {
        return Err(AppError::InvalidInput(
            "Export changed after preview; preview it again before saving.".into(),
        ));
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options.open(&path)?;
    file.write_all(value.content.as_bytes())?;
    file.sync_all()?;
    let now = Utc::now().timestamp();
    let mut conn = state.db.conn();
    let tx = conn.transaction()?;
    audit_tx(&tx, "export_created", &certification_id, now)?;
    let changed = tx.execute("UPDATE business_records SET document=json_set(document,'$.status','exported'),updated_at=?2 WHERE id=(SELECT record_id FROM business_certifications WHERE id=?1)",params![certification_id,now])?;
    if changed != 1 {
        return Err(AppError::InvalidInput(
            "The certification no longer exists; the saved file was not recorded in Knov.".into(),
        ));
    }
    tx.commit()?;
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbound_sanitizer_preserves_category_separator_but_removes_paths() {
        assert_eq!(
            sanitize_text("Non-R&D / Unqualified"),
            "Non-R&D / Unqualified"
        );
        assert_eq!(
            sanitize_text("open /Users/ada/private.txt"),
            "open [redacted]"
        );
    }
    fn event(
        at: i64,
        duration: i64,
        app: &str,
        title: &str,
        url: Option<&str>,
        source: ActivitySource,
    ) -> ActivityEvent {
        ActivityEvent {
            id: None,
            occurred_at: at,
            ended_at: Some(at + duration),
            duration_seconds: duration,
            app_name: app.into(),
            window_title: Some(title.into()),
            url: url.map(str::to_string),
            page_title: None,
            search_query: None,
            browser_profile_id: None,
            source,
            is_bootstrap: false,
        }
    }
    fn project(id: &str, name: &str) -> Project {
        Project {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            color: "#123456".into(),
            status: "active".into(),
            aliases: vec![],
            keywords: vec![],
            domains: vec![],
            repositories: vec![],
            paths: vec![],
            client: String::new(),
            external_identifier: String::new(),
            created_at: 1,
            updated_at: 1,
        }
    }
    #[test]
    fn compiler_deduplicates_overlap_and_classifies() {
        let db = Database::in_memory().unwrap();
        let mut p = project("p", "Mars");
        p.domains = vec!["mars.test".into()];
        apply_action(&db, BusinessAction::SaveProject { project: p }).unwrap();
        db.insert_event(
            &event(
                10,
                100,
                "Chrome",
                "Mars",
                Some("https://mars.test/a?token=secret"),
                ActivitySource::AppFocus,
            ),
            "a",
        )
        .unwrap();
        db.insert_event(
            &event(
                20,
                50,
                "Chrome",
                "Mars",
                Some("https://mars.test/private"),
                ActivitySource::ChromeExtension,
            ),
            "b",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let e = evidence(&db).unwrap();
        assert_eq!(e.iter().map(|v| v.duration_seconds).sum::<i64>(), 100);
        assert_eq!(e[0].suggested_project_id.as_deref(), Some("p"));
        assert!(!e[0].sanitized_context.contains("token"));
    }
    #[test]
    fn ambiguity_and_override_are_authoritative() {
        let db = Database::in_memory().unwrap();
        for id in ["a", "b"] {
            let mut p = project(id, id);
            p.keywords = vec!["shared".into()];
            apply_action(&db, BusinessAction::SaveProject { project: p }).unwrap();
        }
        db.insert_event(
            &event(10, 60, "Code", "shared", None, ActivitySource::AppFocus),
            "x",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let e = evidence(&db).unwrap().remove(0);
        assert_eq!(e.project_confidence, Confidence::Low);
        assert!(e.suggested_project_id.is_none());
        apply_action(
            &db,
            BusinessAction::Review {
                evidence_id: e.id.clone(),
                status: ReviewStatus::Corrected,
                project_id: Some("a".into()),
                category: Some("Delivery".into()),
            },
        )
        .unwrap();
        let fixed = evidence(&db).unwrap().remove(0);
        assert_eq!(effective_project(&fixed).as_deref(), Some("a"));
        assert_eq!(effective_category(&fixed), "Delivery");
    }
    #[test]
    fn exclusion_and_zero_time_signals_work() {
        let db = Database::in_memory().unwrap();
        let mut s = db.settings().unwrap();
        s.excluded_apps = vec!["Private".into()];
        db.save_settings(&s).unwrap();
        db.insert_event(
            &event(10, 50, "Private", "secret", None, ActivitySource::AppFocus),
            "p",
        )
        .unwrap();
        db.insert_event(
            &event(20, 99, "Code", "file", None, ActivitySource::EditorHistory),
            "e",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let e = evidence(&db).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].duration_seconds, 0);
    }
    #[test]
    fn split_and_totals_preserve_ranges_and_unallocated() {
        let db = Database::in_memory().unwrap();
        db.insert_event(
            &event(10, 100, "Code", "work", None, ActivitySource::AppFocus),
            "x",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let id = evidence(&db).unwrap()[0].id.clone();
        apply_action(
            &db,
            BusinessAction::Split {
                evidence_id: id,
                split_at: 40,
            },
        )
        .unwrap();
        let e = evidence(&db).unwrap();
        assert_eq!(e.iter().map(|v| v.duration_seconds).sum::<i64>(), 100);
        let r = BusinessRecord {
            id: "r".into(),
            name: "r".into(),
            template_id: "generic-allocation".into(),
            start_at: 0,
            end_at: 200,
            project_ids: vec![],
            version: 1,
            status: "draft".into(),
            totals: Default::default(),
            created_at: 0,
            updated_at: 0,
        };
        let t = totals(&r, &e, &[]);
        assert_eq!(t.tracked_seconds, 100);
        assert_eq!(t.unallocated_seconds, 100);
    }

    #[test]
    fn routine_mutations_and_workspace_loads_do_not_recompile_complete_history() {
        let db = Database::in_memory().unwrap();
        db.insert_event(
            &event(10, 20, "Code", "first", None, ActivitySource::AppFocus),
            "first",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let first = evidence(&db).unwrap().remove(0);

        // Simulate activity arriving after the workspace was loaded. Reviewing an
        // existing item must stay cheap and leave ingestion to explicit refresh.
        db.insert_event(
            &event(40, 20, "Code", "second", None, ActivitySource::AppFocus),
            "second",
        )
        .unwrap();
        apply_action(
            &db,
            BusinessAction::Review {
                evidence_id: first.id,
                status: ReviewStatus::Accepted,
                project_id: None,
                category: None,
            },
        )
        .unwrap();
        assert_eq!(evidence(&db).unwrap().len(), 1);

        // Reopening a screen should read the already-compiled workspace. New
        // activity remains pending until the user explicitly asks for a refresh.
        load_workspace(&db).unwrap();
        assert_eq!(evidence(&db).unwrap().len(), 1);

        apply_action(&db, BusinessAction::Refresh).unwrap();
        assert_eq!(evidence(&db).unwrap().len(), 2);
    }

    #[test]
    fn certification_is_versioned_immutable_and_export_safe() {
        let db = Database::in_memory().unwrap();
        let mut p = project("p", "=https://secret.test/a");
        p.client = "/Users/me/client".into();
        apply_action(&db, BusinessAction::SaveProject { project: p }).unwrap();
        db.insert_event(
            &event(
                10,
                60,
                "Code",
                "password=hunter2",
                None,
                ActivitySource::AppFocus,
            ),
            "x",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let id = evidence(&db).unwrap()[0].id.clone();
        apply_action(
            &db,
            BusinessAction::Review {
                evidence_id: id,
                status: ReviewStatus::Corrected,
                project_id: Some("p".into()),
                category: Some("Delivery".into()),
            },
        )
        .unwrap();
        let ws = apply_action(
            &db,
            BusinessAction::GenerateRecord {
                name: "https://secret.test/path".into(),
                template_id: "generic-allocation".into(),
                start_at: 0,
                end_at: 100,
                project_ids: vec![],
            },
        )
        .unwrap();
        let r = &ws.records[0];
        apply_action(
            &db,
            BusinessAction::Certify {
                record_id: r.id.clone(),
                certified_by: "sk-secret".into(),
                statement_accepted: true,
                expected_version: r.version,
                expected_updated_at: r.updated_at,
            },
        )
        .unwrap();
        let first = certifications(&db).unwrap()[0].clone();
        assert!(!json(&first.snapshot).unwrap().contains("secret.test"));
        assert!(!json(&first.snapshot).unwrap().contains("/Users"));
        let a = artifact(&db, &first.id, "csv").unwrap();
        assert!(!a.content.contains("hunter2"));
        assert!(a.content.contains("certificationStatement"));
        assert!(a.content.contains("integrityHash"));
        let json_export = artifact(&db, &first.id, "json").unwrap().content;
        let parsed = serde_json::from_str::<serde_json::Value>(&json_export).unwrap();
        assert_eq!(parsed["integrityHash"], first.sha256);
        assert!(parsed.get("sourceEventIds").is_none());
        assert!(!json_export.contains("hunter2"));
        let revised = apply_action(
            &db,
            BusinessAction::ReviseRecord {
                record_id: r.id.clone(),
            },
        )
        .unwrap();
        assert_eq!(certifications(&db).unwrap()[0].sha256, first.sha256);
        assert_eq!(records(&db).unwrap()[0].version, 2);
        let revision = &revised.records[0];
        let recertified = apply_action(
            &db,
            BusinessAction::Certify {
                record_id: revision.id.clone(),
                certified_by: "Ada".into(),
                statement_accepted: true,
                expected_version: revision.version,
                expected_updated_at: revision.updated_at,
            },
        )
        .unwrap();
        let second = recertified
            .certifications
            .iter()
            .find(|certification| certification.record_version == 2)
            .unwrap();
        let after_delete = apply_action(
            &db,
            BusinessAction::DeleteCertification {
                certification_id: second.id.clone(),
            },
        )
        .unwrap();
        assert_eq!(after_delete.certifications.len(), 1);
        assert_eq!(after_delete.certifications[0].sha256, first.sha256);
        assert_eq!(after_delete.records[0].status, "ready");
    }
    #[test]
    fn retention_scrubs_sources_but_preserves_certifications() {
        let db = Database::in_memory().unwrap();
        db.insert_event(
            &event(1, 60, "Code", "work", None, ActivitySource::AppFocus),
            "x",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let conn = db.conn();
        let canonical = json(&CertifiedSnapshot {
            schema_version: 1,
            record_id: "r".into(),
            record_version: 1,
            record_name: "r".into(),
            certified_by: "me".into(),
            period_start: 0,
            period_end: 1,
            template: "t".into(),
            certified_at: 1,
            certification_statement: CERTIFICATION_STATEMENT.into(),
            totals: Default::default(),
        })
        .unwrap();
        let digest = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        conn.execute("INSERT INTO business_certifications(id,record_id,record_version,certified_at,snapshot_json,sha256) VALUES('c','r',1,1,?1,?2)",params![canonical,digest]).unwrap();
        drop(conn);
        db.purge_expired(40 * 86400, true).unwrap();
        let e = evidence(&db).unwrap().remove(0);
        assert!(!e.source_available);
        assert!(e.source_event_ids.is_empty() && e.sanitized_context.is_empty());
        assert_eq!(certifications(&db).unwrap().len(), 1);
    }

    #[test]
    fn certification_load_fails_closed_on_snapshot_or_envelope_tampering() {
        let db = Database::in_memory().unwrap();
        let canonical = json(&CertifiedSnapshot {
            schema_version: 1,
            record_id: "r".into(),
            record_version: 1,
            record_name: "record".into(),
            certified_by: "Ada".into(),
            period_start: 0,
            period_end: 60,
            template: "Generic Project Allocation".into(),
            certified_at: 10,
            certification_statement: CERTIFICATION_STATEMENT.into(),
            totals: Default::default(),
        })
        .unwrap();
        let digest = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        let conn = db.conn();
        conn.execute("INSERT INTO business_certifications(id,record_id,record_version,certified_at,snapshot_json,sha256) VALUES('c','r',1,10,?1,?2)", params![canonical, digest]).unwrap();
        conn.execute("UPDATE business_certifications SET snapshot_json=json_set(snapshot_json,'$.recordName','changed') WHERE id='c'", []).unwrap();
        drop(conn);
        assert!(certifications(&db)
            .unwrap_err()
            .to_string()
            .contains("integrity"));

        let conn = db.conn();
        let canonical = json(&CertifiedSnapshot {
            schema_version: 1,
            record_id: "r".into(),
            record_version: 1,
            record_name: "record".into(),
            certified_by: "Ada".into(),
            period_start: 0,
            period_end: 60,
            template: "Generic Project Allocation".into(),
            certified_at: 10,
            certification_statement: CERTIFICATION_STATEMENT.into(),
            totals: Default::default(),
        })
        .unwrap();
        let digest = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        conn.execute("UPDATE business_certifications SET snapshot_json=?1,sha256=?2,record_version=2 WHERE id='c'", params![canonical,digest]).unwrap();
        drop(conn);
        assert!(certifications(&db)
            .unwrap_err()
            .to_string()
            .contains("inconsistent"));
    }

    #[test]
    fn record_range_clips_duration_and_keeps_nonselected_time_visible() {
        let db = Database::in_memory().unwrap();
        let p = project("p", "Mars");
        apply_action(&db, BusinessAction::SaveProject { project: p }).unwrap();
        db.insert_event(
            &event(0, 100, "Code", "Mars", None, ActivitySource::AppFocus),
            "range",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let r = BusinessRecord {
            id: "r".into(),
            name: "r".into(),
            template_id: "generic-allocation".into(),
            start_at: 25,
            end_at: 75,
            project_ids: vec!["different".into()],
            version: 1,
            status: "draft".into(),
            totals: Default::default(),
            created_at: 0,
            updated_at: 0,
        };
        let value = totals(&r, &evidence(&db).unwrap(), &projects(&db).unwrap());
        assert_eq!(value.tracked_seconds, 50);
        assert_eq!(value.unallocated_seconds, 50);
    }

    #[test]
    fn refresh_updates_an_ongoing_reviewed_source_and_requires_review_again() {
        let db = Database::in_memory().unwrap();
        db.insert_event(
            &event(10, 20, "Code", "work", None, ActivitySource::AppFocus),
            "live",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let mut item = evidence(&db).unwrap().remove(0);
        item.review_status = ReviewStatus::Accepted;
        db.conn()
            .execute(
                "UPDATE business_evidence SET document=?2 WHERE id=?1",
                params![item.id, json(&item).unwrap()],
            )
            .unwrap();
        db.insert_event(
            &event(10, 50, "Code", "work", None, ActivitySource::AppFocus),
            "live",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let updated = evidence(&db).unwrap().remove(0);
        assert_eq!(updated.duration_seconds, 50);
        assert_eq!(updated.review_status, ReviewStatus::Pending);
    }

    #[test]
    fn certification_rejects_unaccepted_stale_and_template_invalid_drafts() {
        let db = Database::in_memory().unwrap();
        let mut record = BusinessRecord {
            id: "r".into(),
            name: "r".into(),
            template_id: "rnd-allocation".into(),
            start_at: 0,
            end_at: 100,
            project_ids: vec![],
            version: 1,
            status: "ready".into(),
            totals: Default::default(),
            created_at: 1,
            updated_at: 1,
        };
        db.conn().execute("INSERT INTO business_records(id,document,created_at,updated_at) VALUES('r',?1,1,1)", [json(&record).unwrap()]).unwrap();
        assert!(certify(&db, "r", "me", false, 1, 1, 2).is_err());
        assert!(certify(&db, "r", "me", true, 2, 1, 2).is_err());
        let p = project("p", "Mars");
        apply_action(&db, BusinessAction::SaveProject { project: p }).unwrap();
        db.insert_event(
            &event(10, 20, "Code", "Mars", None, ActivitySource::AppFocus),
            "bad-category",
        )
        .unwrap();
        refresh_evidence(&db).unwrap();
        let mut item = evidence(&db).unwrap().remove(0);
        item.review_status = ReviewStatus::Accepted;
        db.conn()
            .execute(
                "UPDATE business_evidence SET document=?2 WHERE id=?1",
                params![item.id, json(&item).unwrap()],
            )
            .unwrap();
        record.updated_at = 1;
        assert!(certify(&db, "r", "me", true, 1, 1, 2).is_err());
    }
}
