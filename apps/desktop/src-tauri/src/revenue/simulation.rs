//! Ten fictional businesses, each a plausible Knov customer, whose sources can
//! be connected one at a time. Every record is demo-scoped: it never mixes with
//! actual data, and no request leaves this Mac. Rule-based analysis runs after
//! each step so the findings visibly sharpen as more sources arrive. The
//! dataset lives in `simulation_businesses.json`; see docs/customer-profiles.md.
use super::{analyze, evidence, field, import_agreement, list, project, put, DEMO_NOW};
use crate::{
    commands::AppState,
    db::Database,
    discovery,
    error::{AppError, AppResult},
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::LazyLock;
use tauri::State;

const DAY: i64 = 86_400;

/// Steps in the order a business would usually connect them.
pub const STEPS: [&str; 5] = ["contracts", "gmail", "slack", "work", "billing"];

/// The business loaded when none has been chosen; its data backs the fixed tests.
pub const DEFAULT_BUSINESS: &str = "brightline";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Business {
    pub id: String,
    pub name: String,
    pub industry: String,
    pub location: String,
    pub team: String,
    pub currency: String,
    pub owner: Owner,
    pub tools: Vec<String>,
    pub profile: String,
    pub what_to_expect: Vec<String>,
    pub engagements: Vec<Engagement>,
    pub records: Records,
    pub expected: Vec<Expected>,
    pub interview: discovery::Script,
}

#[derive(Deserialize)]
pub struct Owner {
    pub name: String,
    pub email: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Engagement {
    pub client: String,
    pub project: String,
    pub description: String,
    #[serde(default)]
    pub threads: Vec<String>,
    pub agreement_title: String,
    pub agreement: String,
}

#[derive(Deserialize)]
pub struct Records {
    pub gmail: Vec<Record>,
    pub slack: Vec<Record>,
    pub work: Vec<Record>,
    pub billing: Vec<Record>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub project: String,
    pub kind: String,
    pub reference: String,
    pub text: String,
    pub days_ago: i64,
    #[serde(default)]
    pub due_days_ago: Option<i64>,
    #[serde(default)]
    pub amount_cents: Option<i64>,
    #[serde(default)]
    pub meta: Value,
}

/// What the detector should find after every source is connected in order.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Expected {
    pub client: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
    #[serde(default)]
    pub amount_cents: Option<i64>,
    #[serde(default)]
    pub currency: Option<String>,
}

#[derive(Deserialize)]
struct Dataset {
    businesses: Vec<Business>,
}

static DATASET: LazyLock<Dataset> = LazyLock::new(|| {
    serde_json::from_str(include_str!("simulation_businesses.json"))
        .expect("the bundled simulation dataset is valid")
});

pub fn businesses() -> &'static [Business] {
    &DATASET.businesses
}

pub fn business(id: &str) -> Option<&'static Business> {
    businesses().iter().find(|b| b.id == id)
}

/// The scripted interview for a business, used by the provider-free interviewer.
pub fn interview_script(business_id: &str) -> Option<&'static discovery::Script> {
    business(business_id).map(|b| &b.interview)
}

fn summary(b: &Business) -> Value {
    json!({
        "id": b.id, "name": b.name, "industry": b.industry, "location": b.location,
        "team": b.team, "currency": b.currency, "owner": {"name": b.owner.name, "email": b.owner.email},
        "tools": b.tools, "profile": b.profile, "whatToExpect": b.what_to_expect,
        "clientCount": b.engagements.len(), "interviewTopic": b.interview.topic,
        "interviewName": b.interview.name, "expected": b.expected,
    })
}

const ACTIVE: &str = "active";

/// The business currently loaded in the sandbox, if one was chosen.
pub(super) fn active(conn: &Connection) -> AppResult<Option<&'static Business>> {
    Ok(super::find(conn, "simulation", ACTIVE)?
        .and_then(|v| business(field(&v, "business"))))
}

/// Summary of the active business for the overview, or null outside the sandbox.
pub(super) fn active_summary(conn: &Connection) -> AppResult<Value> {
    Ok(active(conn)?.map(summary).unwrap_or(Value::Null))
}

fn set_active(conn: &Connection, id: &str) -> AppResult<()> {
    put(
        conn,
        "simulation",
        &json!({"id": ACTIVE, "business": id, "demo": true, "createdAt": DEMO_NOW}),
    )
}

fn contracts(conn: &Connection, b: &Business) -> AppResult<()> {
    if !list(conn, "project", true)?.is_empty() {
        return Ok(());
    }
    for e in &b.engagements {
        let p = project(
            conn,
            &e.client,
            &e.project,
            &e.description,
            e.threads.clone(),
            true,
        )?;
        import_agreement(conn, field(&p, "id"), &e.agreement_title, &e.agreement)?;
    }
    Ok(())
}

fn feed(conn: &Connection, source: &str, records: &[Record]) -> AppResult<()> {
    let projects = list(conn, "project", true)?;
    if projects.is_empty() {
        return Err(AppError::InvalidInput(
            "Create the simulated business before connecting its sources.".into(),
        ));
    }
    for record in records {
        let Some(p) = projects.iter().find(|p| field(p, "name") == record.project) else {
            continue;
        };
        let mut v = evidence(
            conn,
            field(p, "id"),
            &record.kind,
            source,
            &record.reference,
            &record.text,
            DEMO_NOW - record.days_ago * DAY,
            record.due_days_ago.map(|days| DEMO_NOW - days * DAY),
            record.amount_cents,
            record.amount_cents.map(|_| b_currency(conn)),
            "observed",
        )?;
        v["meta"] = record.meta.clone();
        put(conn, "evidence", &v)?;
    }
    Ok(())
}

/// Every amount in a business uses that business's currency.
fn b_currency(conn: &Connection) -> &'static str {
    active(conn)
        .ok()
        .flatten()
        .map(|b| b.currency.as_str())
        .unwrap_or("USD")
}

/// Fixed timestamps keep the simulation reproducible across runs.
fn normalize_timestamps(conn: &Connection) -> AppResult<()> {
    for kind in ["client", "project", "agreement", "evidence", "commitment"] {
        for mut v in list(conn, kind, true)? {
            v["createdAt"] = json!(DEMO_NOW - 10 * DAY);
            v["updatedAt"] = json!(DEMO_NOW - 10 * DAY);
            if kind == "evidence" && field(&v, "kind") == "agreement" {
                v["occurredAt"] = json!(DEMO_NOW - 10 * DAY);
            }
            put(conn, kind, &v)?;
        }
    }
    Ok(())
}

/// Loads one source of the active business (Brightline when none was chosen).
pub(super) fn load(conn: &Connection, step: &str) -> AppResult<()> {
    let b = match active(conn)? {
        Some(b) => b,
        None => {
            set_active(conn, DEFAULT_BUSINESS)?;
            business(DEFAULT_BUSINESS).expect("default business exists")
        }
    };
    let source = match step {
        "contracts" => None,
        "gmail" => Some(("demo_gmail", &b.records.gmail)),
        "slack" => Some(("demo_slack", &b.records.slack)),
        "work" => Some(("demo_thread", &b.records.work)),
        "billing" => Some(("demo_billing", &b.records.billing)),
        _ => return Err(AppError::InvalidInput("Unknown simulation step.".into())),
    };
    contracts(conn, b)?;
    if let Some((name, records)) = source {
        let loaded = list(conn, "evidence", true)?
            .iter()
            .any(|e| field(e, "source") == name);
        if !loaded {
            feed(conn, name, records)?;
        }
    }
    normalize_timestamps(conn)?;
    analyze(conn, true, DEMO_NOW)
}

/// Clears every sandbox record and interview; actual data is untouched.
fn reset(db: &Database) -> AppResult<()> {
    discovery::delete_demo_sessions(db)?;
    db.conn()
        .execute("DELETE FROM revenue_records WHERE demo=?1", params![true])?;
    Ok(())
}

/// Makes `id` the sandbox business, clearing the previous one if it differs.
fn select(db: &Database, id: &str) -> AppResult<()> {
    if business(id).is_none() {
        return Err(AppError::InvalidInput("Unknown simulated business.".into()));
    }
    let current = active(&db.conn())?.map(|b| b.id.as_str());
    if current != Some(id) {
        reset(db)?;
        set_active(&db.conn(), id)?;
    }
    Ok(())
}

fn catalog(db: &Database) -> AppResult<Value> {
    let conn = db.conn();
    let active = active(&conn)?;
    drop(conn);
    let interview = match active {
        Some(b) => discovery::scripted_session(db, &b.id)?
            .map(|s| json!({"sessionId": s.id, "status": s.status}))
            .unwrap_or(Value::Null),
        None => Value::Null,
    };
    Ok(json!({
        "active": active.map(|b| b.id.clone()),
        "businesses": businesses().iter().map(summary).collect::<Vec<_>>(),
        "interview": interview,
    }))
}

#[tauri::command]
pub fn revenue_simulation_catalog(state: State<'_, AppState>) -> AppResult<Value> {
    catalog(&state.db)
}

#[tauri::command]
pub fn revenue_simulation_select(state: State<'_, AppState>, business: String) -> AppResult<Value> {
    select(&state.db, &business)?;
    super::overview(&state.db.conn(), true)
}

#[tauri::command]
pub fn revenue_simulation_load(
    state: State<'_, AppState>,
    step: String,
    business: Option<String>,
) -> AppResult<Value> {
    if let Some(id) = business {
        select(&state.db, &id)?;
    }
    let conn = state.db.conn();
    let steps: Vec<&str> = if step == "all" {
        STEPS.to_vec()
    } else {
        vec![step.as_str()]
    };
    for step in steps {
        load(&conn, step)?;
    }
    super::overview(&conn, true)
}

/// Starts (or returns) the scripted interview for the active business. It runs
/// without an AI provider: the questions are fixed, the answers are saved.
#[tauri::command]
pub fn revenue_simulation_interview(
    state: State<'_, AppState>,
) -> AppResult<discovery::InterviewSession> {
    let b = active(&state.db.conn())?.ok_or_else(|| {
        AppError::InvalidInput("Choose a simulated business before starting its interview.".into())
    })?;
    if let Some(existing) = discovery::scripted_session(&state.db, &b.id)? {
        return Ok(existing);
    }
    discovery::start_scripted(&state.db, &b.id, &b.interview)
}

#[tauri::command]
pub fn revenue_simulation_reset(state: State<'_, AppState>) -> AppResult<Value> {
    reset(&state.db)?;
    super::overview(&state.db.conn(), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opportunities(conn: &Connection) -> Vec<Value> {
        list(conn, "opportunity", true).unwrap()
    }

    #[test]
    fn findings_sharpen_as_simulated_sources_connect() {
        let db = Database::in_memory().unwrap();
        let c = db.conn();
        load(&c, "contracts").unwrap();
        assert!(opportunities(&c).is_empty());
        load(&c, "slack").unwrap();
        // A request alone is not evidence that unbilled work happened.
        assert!(opportunities(&c).is_empty());
        load(&c, "work").unwrap();
        let scope: Vec<_> = opportunities(&c)
            .into_iter()
            .filter(|o| field(o, "type") == "scope_change")
            .collect();
        assert_eq!(scope.len(), 2);
        load(&c, "gmail").unwrap();
        let harbor = opportunities(&c)
            .into_iter()
            .find(|o| field(o, "clientName") == "Harbor Goods")
            .unwrap();
        assert_eq!(harbor["status"], "resolved");
        let atlas = opportunities(&c)
            .into_iter()
            .find(|o| field(o, "clientName") == "Atlas Systems")
            .unwrap();
        assert_eq!(atlas["type"], "billing_verification");
        assert!(atlas["amountCents"].is_null());
        load(&c, "billing").unwrap();
        let atlas = get_opportunity(&c, field(&atlas, "id"));
        assert_eq!(atlas["type"], "unbilled_milestone");
        assert_eq!(atlas["amountCents"], 500_000);
        assert!(list(&c, "opportunity", false).unwrap().is_empty());
    }

    #[test]
    fn steps_are_idempotent_and_keep_source_metadata() {
        let db = Database::in_memory().unwrap();
        let c = db.conn();
        for step in STEPS {
            load(&c, step).unwrap();
            load(&c, step).unwrap();
        }
        let evidence = list(&c, "evidence", true).unwrap();
        let b = business(DEFAULT_BUSINESS).unwrap();
        let expected = b.records.gmail.len()
            + b.records.slack.len()
            + b.records.work.len()
            + b.records.billing.len()
            + b.engagements.len();
        assert_eq!(evidence.len(), expected);
        assert!(evidence
            .iter()
            .filter(|e| field(e, "source") == "demo_gmail")
            .all(|e| e["meta"]["subject"].is_string()));
        assert!(load(&c, "unknown").is_err());
    }

    /// Every business in the dataset produces exactly the findings it documents,
    /// so a dataset edit that silently stops triggering a rule fails here.
    #[test]
    fn every_business_matches_its_expected_findings() {
        assert_eq!(businesses().len(), 10);
        for b in businesses() {
            let db = Database::in_memory().unwrap();
            select(&db, &b.id).unwrap();
            let c = db.conn();
            for step in STEPS {
                load(&c, step).unwrap();
            }
            let found = opportunities(&c);
            let mut seen = std::collections::BTreeSet::new();
            for e in &b.expected {
                let o = found
                    .iter()
                    .find(|o| field(o, "clientName") == e.client && field(o, "type") == e.kind)
                    .unwrap_or_else(|| panic!("{}: expected {} for {}", b.id, e.kind, e.client));
                assert_eq!(o["status"], e.status, "{}: {} status", b.id, e.client);
                assert_eq!(o["amountCents"], json!(e.amount_cents), "{}: {} amount", b.id, e.client);
                assert_eq!(o["currency"], json!(e.currency), "{}: {} currency", b.id, e.client);
                seen.insert(field(o, "id").to_string());
            }
            let extra: Vec<_> = found
                .iter()
                .filter(|o| !seen.contains(field(o, "id")))
                .map(|o| format!("{} {}", field(o, "clientName"), field(o, "type")))
                .collect();
            assert!(extra.is_empty(), "{}: unexpected findings {:?}", b.id, extra);
            for e in list(&c, "evidence", true).unwrap() {
                if e["amountCents"].is_number() {
                    assert_eq!(e["currency"], b.currency, "{}: currency", b.id);
                }
            }
            assert_eq!(list(&c, "opportunity", false).unwrap().len(), 0);
        }
    }

    /// Approved extras produce no finding regardless of connection order, and a
    /// late approval resolves a finding that already exists.
    #[test]
    fn late_approvals_resolve_scope_findings_in_every_business() {
        for b in businesses() {
            let db = Database::in_memory().unwrap();
            select(&db, &b.id).unwrap();
            let c = db.conn();
            for step in ["contracts", "slack", "work"] {
                load(&c, step).unwrap();
            }
            let before: Vec<_> = opportunities(&c)
                .into_iter()
                .filter(|o| field(o, "type") == "scope_change")
                .map(|o| field(&o, "id").to_string())
                .collect();
            load(&c, "gmail").unwrap();
            load(&c, "billing").unwrap();
            let after = opportunities(&c);
            for id in before {
                let o = after.iter().find(|o| field(o, "id") == id).unwrap();
                let expected = b
                    .expected
                    .iter()
                    .any(|e| e.kind == "scope_change" && e.client == field(o, "clientName"));
                assert_eq!(
                    o["status"] != "resolved",
                    expected,
                    "{}: {} should {}be open",
                    b.id,
                    field(o, "clientName"),
                    if expected { "" } else { "not " }
                );
            }
        }
    }

    #[test]
    fn switching_business_resets_the_sandbox_and_its_interview() {
        let db = Database::in_memory().unwrap();
        select(&db, "lanternwood").unwrap();
        for step in STEPS {
            load(&db.conn(), step).unwrap();
        }
        let session = discovery::start_scripted(&db, "lanternwood", interview_script("lanternwood").unwrap()).unwrap();
        assert!(catalog(&db).unwrap()["interview"]["sessionId"].is_string());
        assert_eq!(catalog(&db).unwrap()["active"], "lanternwood");
        assert!(discovery::sessions_in_scope(&db, false).unwrap().is_empty());
        select(&db, "lanternwood").unwrap();
        assert!(!list(&db.conn(), "project", true).unwrap().is_empty());
        select(&db, "gridline").unwrap();
        assert!(list(&db.conn(), "project", true).unwrap().is_empty());
        assert!(discovery::session(&db, &session.id).is_err());
        assert!(catalog(&db).unwrap()["interview"].is_null());
        assert!(select(&db, "nowhere").is_err());
        let overview = super::super::overview(&db.conn(), true).unwrap();
        assert_eq!(overview["business"]["name"], "Gridline Architects");
        assert!(super::super::overview(&db.conn(), false).unwrap()["business"].is_null());
    }

    fn get_opportunity(conn: &Connection, id: &str) -> Value {
        super::super::get(conn, "opportunity", id).unwrap()
    }
}
