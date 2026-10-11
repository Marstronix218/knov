//! A fictional agency, Brightline Studio, whose sources can be connected one at
//! a time. Every record is demo-scoped: it never mixes with actual data, and no
//! request leaves this Mac. Rule-based analysis runs after each step so the
//! findings visibly sharpen as more sources arrive.
use super::{analyze, evidence, field, import_agreement, list, project, put, DEMO_NOW};
use crate::{
    commands::AppState,
    discovery,
    error::{AppError, AppResult},
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tauri::State;

const DAY: i64 = 86_400;

/// Steps in the order a business would usually connect them.
pub const STEPS: [&str; 5] = ["contracts", "gmail", "slack", "work", "billing"];

struct Record {
    project: &'static str,
    kind: &'static str,
    source: &'static str,
    reference: &'static str,
    text: &'static str,
    days_ago: i64,
    due_days_ago: Option<i64>,
    amount_cents: Option<i64>,
    meta: fn() -> Value,
}

const ACME: &str = "Analytics dashboards";
const NORTHSTAR: &str = "Discovery proposal";
const ATLAS: &str = "Portal delivery";
const HARBOR: &str = "Inventory dashboards";

const GMAIL: &[Record] = &[
    Record {
        project: ACME,
        kind: "communication",
        source: "demo_gmail",
        reference: "acme-feedback",
        text: "The two dashboards look great. The sales team already loves the filters.",
        days_ago: 9,
        due_days_ago: None,
        amount_cents: None,
        meta: || json!({"from":"Priya Shah <priya@acmeanalytics.com>","to":"you@brightline.studio","subject":"Dashboards 1 & 2 feedback"}),
    },
    Record {
        project: NORTHSTAR,
        kind: "commitment",
        source: "demo_gmail",
        reference: "northstar-promise",
        text: "Thanks for the workshop, Maya. I will send the updated proposal by 2025-10-08T17:00:00-07:00.",
        days_ago: 5,
        due_days_ago: Some(1),
        amount_cents: None,
        meta: || json!({"from":"You <you@brightline.studio>","to":"Maya Chen <maya@northstarlabs.io>","subject":"Re: Discovery proposal next steps"}),
    },
    Record {
        project: NORTHSTAR,
        kind: "communication",
        source: "demo_gmail",
        reference: "northstar-board",
        text: "Our board meets next Tuesday. We'd love to review the revised proposal before then.",
        days_ago: 2,
        due_days_ago: None,
        amount_cents: None,
        meta: || json!({"from":"Maya Chen <maya@northstarlabs.io>","to":"you@brightline.studio","subject":"Board meeting next Tuesday"}),
    },
    Record {
        project: ATLAS,
        kind: "milestone_accepted",
        source: "demo_gmail",
        reference: "atlas-acceptance",
        text: "The portal milestone is accepted. Great work, the team is already using it.",
        days_ago: 2,
        due_days_ago: None,
        amount_cents: Some(500_000),
        meta: || json!({"from":"Luis Ortega <luis@atlassystems.com>","to":"you@brightline.studio","subject":"Portal sign-off"}),
    },
    Record {
        project: HARBOR,
        kind: "approval",
        source: "demo_gmail",
        reference: "approved-order",
        text: "Approved: go ahead with the third dashboard change order for USD 2,000.",
        days_ago: 6,
        due_days_ago: None,
        amount_cents: Some(200_000),
        meta: || json!({"from":"Sam Rivera <sam@harborgoods.com>","to":"you@brightline.studio","subject":"Change order #7 approved"}),
    },
];

const SLACK: &[Record] = &[
    Record {
        project: ACME,
        kind: "request",
        source: "demo_slack",
        reference: "acme-request",
        text: "Please build a third dashboard for the sales team, same style as the first two.",
        days_ago: 7,
        due_days_ago: None,
        amount_cents: None,
        meta: || json!({"channel":"#acme-dashboards","author":"Dana Lee (Acme)"}),
    },
    Record {
        project: ACME,
        kind: "communication",
        source: "demo_slack",
        reference: "acme-thanks",
        text: "Thanks for the quick turnaround on the filter fix!",
        days_ago: 8,
        due_days_ago: None,
        amount_cents: None,
        meta: || json!({"channel":"#acme-dashboards","author":"Dana Lee (Acme)"}),
    },
    Record {
        project: HARBOR,
        kind: "request",
        source: "demo_slack",
        reference: "approved-request",
        text: "Please build a third dashboard for inventory forecasting.",
        days_ago: 7,
        due_days_ago: None,
        amount_cents: None,
        meta: || json!({"channel":"#harbor-goods","author":"Sam Rivera (Harbor Goods)"}),
    },
    Record {
        project: ATLAS,
        kind: "communication",
        source: "demo_slack",
        reference: "atlas-live",
        text: "Atlas portal is live. Nice work, everyone!",
        days_ago: 2,
        due_days_ago: None,
        amount_cents: None,
        meta: || json!({"channel":"#team","author":"Jordan Park (Brightline)"}),
    },
];

const WORK: &[Record] = &[
    Record {
        project: ACME,
        kind: "activity",
        source: "demo_thread",
        reference: "demo-third-dashboard-thread",
        text: "Implemented the third dashboard (sales) in the linked developer thread.",
        days_ago: 4,
        due_days_ago: None,
        amount_cents: None,
        meta: || json!({"apps":"VS Code, Figma, Chrome","hours":11.5,"thread":"acme-sales-dashboard"}),
    },
    Record {
        project: HARBOR,
        kind: "activity",
        source: "demo_thread",
        reference: "approved-work",
        text: "Third dashboard implemented for Harbor inventory forecasting.",
        days_ago: 4,
        due_days_ago: None,
        amount_cents: None,
        meta: || json!({"apps":"VS Code, Chrome","hours":7.0,"thread":"harbor-inventory-dashboard"}),
    },
];

const BILLING: &[Record] = &[
    Record {
        project: ACME,
        kind: "invoice",
        source: "demo_billing",
        reference: "INV-1041",
        text: "Invoice INV-1041 · Acme Analytics · Dashboards 1 and 2 · paid in full.",
        days_ago: 10,
        due_days_ago: None,
        amount_cents: Some(1_200_000),
        meta: || json!({"number":"INV-1041","status":"Paid","customer":"Acme Analytics"}),
    },
    Record {
        project: ATLAS,
        kind: "invoice",
        source: "demo_billing",
        reference: "INV-1038",
        text: "Invoice INV-1038 · Atlas Systems · Discovery phase · paid.",
        days_ago: 30,
        due_days_ago: None,
        amount_cents: Some(300_000),
        meta: || json!({"number":"INV-1038","status":"Paid","customer":"Atlas Systems"}),
    },
    Record {
        project: ATLAS,
        kind: "billing_complete",
        source: "demo_billing",
        reference: "atlas-billing-sync",
        text: "QuickBooks sync covers every Atlas Systems invoice through 2025-10-10.",
        days_ago: 0,
        due_days_ago: None,
        amount_cents: None,
        meta: || json!({"status":"Sync complete","customer":"Atlas Systems"}),
    },
    Record {
        project: HARBOR,
        kind: "invoice",
        source: "demo_billing",
        reference: "INV-1043",
        text: "Invoice INV-1043 · Harbor Goods · Change order #7 (third dashboard) · sent.",
        days_ago: 5,
        due_days_ago: None,
        amount_cents: Some(200_000),
        meta: || json!({"number":"INV-1043","status":"Sent","customer":"Harbor Goods"}),
    },
];

fn contracts(conn: &Connection) -> AppResult<()> {
    if !list(conn, "project", true)?.is_empty() {
        return Ok(());
    }
    let engagements = [
        ("Acme Analytics", ACME, "Two analytics dashboards for the sales team", vec!["demo-third-dashboard-thread".to_string()], "Acme statement of work", "Scope: Build two dashboards.\nExclusions: third dashboard\nProject fee USD 12000.\nPayment: invoice after milestone acceptance."),
        ("Northstar Labs", NORTHSTAR, "Discovery workshop and written proposal", vec![], "Northstar engagement letter", "Scope: Discovery workshop and a written proposal.\nFee: USD 4500 on signature."),
        ("Atlas Systems", ATLAS, "Client portal build", vec![], "Atlas agreement", "Scope: Client portal.\nMilestone: acceptance of portal.\nPayment: USD 5000 after milestone acceptance."),
        ("Harbor Goods", HARBOR, "Inventory dashboards with an approved expansion", vec![], "Harbor Goods statement of work", "Scope: Build two dashboards."),
    ];
    for (client, name, description, threads, title, text) in engagements {
        let p = project(conn, client, name, description, threads, true)?;
        import_agreement(conn, field(&p, "id"), title, text)?;
    }
    Ok(())
}

fn feed(conn: &Connection, records: &[Record]) -> AppResult<()> {
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
            record.kind,
            record.source,
            record.reference,
            record.text,
            DEMO_NOW - record.days_ago * DAY,
            record.due_days_ago.map(|days| DEMO_NOW - days * DAY),
            record.amount_cents,
            record.amount_cents.map(|_| "USD"),
            "observed",
        )?;
        v["meta"] = (record.meta)();
        put(conn, "evidence", &v)?;
    }
    Ok(())
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

pub(super) fn load(conn: &Connection, step: &str) -> AppResult<()> {
    let source = match step {
        "contracts" => None,
        "gmail" => Some(("demo_gmail", GMAIL)),
        "slack" => Some(("demo_slack", SLACK)),
        "work" => Some(("demo_thread", WORK)),
        "billing" => Some(("demo_billing", BILLING)),
        _ => return Err(AppError::InvalidInput("Unknown simulation step.".into())),
    };
    contracts(conn)?;
    if let Some((name, records)) = source {
        let loaded = list(conn, "evidence", true)?
            .iter()
            .any(|e| field(e, "source") == name);
        if !loaded {
            feed(conn, records)?;
        }
    }
    normalize_timestamps(conn)?;
    analyze(conn, true, DEMO_NOW)
}

#[tauri::command]
pub fn revenue_simulation_load(state: State<'_, AppState>, step: String) -> AppResult<Value> {
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

#[tauri::command]
pub fn revenue_simulation_reset(state: State<'_, AppState>) -> AppResult<Value> {
    let interviews: Vec<String> = list(&state.db.conn(), "opportunity", true)?
        .iter()
        .filter_map(|o| o["interviewId"].as_str().map(str::to_owned))
        .collect();
    for interview in interviews {
        discovery::delete_session(&state.db, &interview)?;
    }
    let conn = state.db.conn();
    conn.execute("DELETE FROM revenue_records WHERE demo=?1", params![true])?;
    super::overview(&conn, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

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
        let expected = GMAIL.len() + SLACK.len() + WORK.len() + BILLING.len() + 4;
        assert_eq!(evidence.len(), expected);
        assert!(evidence
            .iter()
            .filter(|e| field(e, "source") == "demo_gmail")
            .all(|e| e["meta"]["subject"].is_string()));
        assert!(load(&c, "unknown").is_err());
    }

    fn get_opportunity(conn: &Connection, id: &str) -> Value {
        super::super::get(conn, "opportunity", id).unwrap()
    }
}
