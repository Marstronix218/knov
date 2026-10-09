//! Relational, revisioned views of interview documents. The interview document
//! remains canonical; edits append a snapshot instead of erasing earlier claims.
use std::collections::HashSet;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::{InterviewSession, WorkflowDocument};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEvidence {
    pub source_type: String,
    pub source_ref: String,
    pub timestamp: String,
    pub confidence: f64,
    pub status: String,
    pub user_confirmed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub description: String,
    pub evidence: Vec<GraphEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relationship: String,
    pub evidence: Vec<GraphEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowGraph {
    pub session_id: String,
    pub revision: i64,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRevision {
    pub revision: i64,
    pub timestamp: String,
    pub nodes: usize,
    pub edges: usize,
}

pub fn initialize_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS discovery_graph_revisions (
           session_id TEXT NOT NULL REFERENCES discovery_sessions(id) ON DELETE CASCADE,
           revision INTEGER NOT NULL, timestamp TEXT NOT NULL, document_json TEXT NOT NULL,
           PRIMARY KEY(session_id,revision)
         );
         CREATE TABLE IF NOT EXISTS discovery_graph_nodes (
           session_id TEXT NOT NULL, revision INTEGER NOT NULL, id TEXT NOT NULL,
           kind TEXT NOT NULL, label TEXT NOT NULL, description TEXT NOT NULL,
           PRIMARY KEY(session_id,revision,id),
           FOREIGN KEY(session_id,revision) REFERENCES discovery_graph_revisions(session_id,revision) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS discovery_graph_node_kind_idx
           ON discovery_graph_nodes(kind,session_id,revision);
         CREATE TABLE IF NOT EXISTS discovery_graph_edges (
           session_id TEXT NOT NULL, revision INTEGER NOT NULL, id TEXT NOT NULL,
           source TEXT NOT NULL, target TEXT NOT NULL, relationship TEXT NOT NULL,
           PRIMARY KEY(session_id,revision,id),
           FOREIGN KEY(session_id,revision,source) REFERENCES discovery_graph_nodes(session_id,revision,id) ON DELETE CASCADE,
           FOREIGN KEY(session_id,revision,target) REFERENCES discovery_graph_nodes(session_id,revision,id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS discovery_graph_edge_source_idx
           ON discovery_graph_edges(session_id,revision,source,relationship);
         CREATE INDEX IF NOT EXISTS discovery_graph_edge_target_idx
           ON discovery_graph_edges(session_id,revision,target,relationship);
         CREATE TABLE IF NOT EXISTS discovery_graph_evidence (
           session_id TEXT NOT NULL, revision INTEGER NOT NULL,
           fact_kind TEXT NOT NULL CHECK(fact_kind IN ('node','edge')), fact_id TEXT NOT NULL,
           position INTEGER NOT NULL, source_type TEXT NOT NULL, source_ref TEXT NOT NULL,
           timestamp TEXT NOT NULL, confidence REAL NOT NULL CHECK(confidence BETWEEN 0 AND 1),
           status TEXT NOT NULL CHECK(status IN ('observed','hypothesis','user_reported','user_confirmed')),
           user_confirmed INTEGER NOT NULL CHECK(user_confirmed IN (0,1)), detail TEXT NOT NULL,
           PRIMARY KEY(session_id,revision,fact_kind,fact_id,position),
           FOREIGN KEY(session_id,revision) REFERENCES discovery_graph_revisions(session_id,revision) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS discovery_graph_evidence_source_idx
           ON discovery_graph_evidence(source_type,source_ref);",
    )?;
    Ok(())
}

/// Call inside the same transaction that stores the canonical document.
pub fn replace_document(
    conn: &Connection,
    session_id: &str,
    document: &WorkflowDocument,
    timestamp: &str,
) -> AppResult<()> {
    if document.session_id != session_id {
        return Err(AppError::InvalidInput(
            "workflow belongs to another interview".into(),
        ));
    }
    let ids: HashSet<_> = document.steps.iter().map(|step| step.id.as_str()).collect();
    if ids.len() != document.steps.len()
        || document.steps.iter().any(|step| {
            step.id.trim().is_empty()
                || step
                    .depends_on
                    .iter()
                    .any(|id| id == &step.id || !ids.contains(id.as_str()))
        })
    {
        return Err(AppError::InvalidInput(
            "workflow graph has invalid step dependencies".into(),
        ));
    }
    let revision: i64 = conn.query_row(
        "SELECT COALESCE(MAX(revision),0)+1 FROM discovery_graph_revisions WHERE session_id=?1",
        [session_id],
        |row| row.get(0),
    )?;
    let mut graph = derive_graph(session_id, revision, document, timestamp);
    // The canonical session owns native-validated context. Provider labels alone
    // never establish observation. Context is read after the session transaction
    // stores it, and is retained independently from editable workflow claims.
    let raw: String = conn.query_row(
        "SELECT document FROM discovery_sessions WHERE id=?1",
        [session_id],
        |row| row.get(0),
    )?;
    if let Ok(session) = serde_json::from_str::<InterviewSession>(&raw) {
        if let Some(context) = session.thread_context {
            for (index, event) in context.events.iter().enumerate() {
                let detail = format!(
                    "Recorded {} activity in {} at {}",
                    event.source, event.app_name, event.observed_at
                );
                if let Some(evidence) = graph.nodes[0]
                    .evidence
                    .iter_mut()
                    .find(|evidence| evidence.detail == detail)
                {
                    evidence.source_type = event.source.clone();
                    evidence.source_ref = format!("{session_id}/context/{index}");
                    evidence.timestamp = event.observed_at.clone();
                    evidence.status = "observed".into();
                    evidence.confidence = 1.0;
                }
                let observation = GraphEvidence {
                    source_type: event.source.clone(),
                    source_ref: format!("{session_id}/context/{index}"),
                    timestamp: event.observed_at.clone(),
                    confidence: 1.0,
                    status: "observed".into(),
                    user_confirmed: false,
                    detail: detail.clone(),
                };
                let id = format!("{}:activity:{index}", document.id);
                add_node(
                    &mut graph,
                    &id,
                    "ActivityObservation",
                    &event.app_name,
                    &detail,
                    &observation,
                );
                let workflow = graph.nodes[0].id.clone();
                let mut relevance = graph.nodes[0].evidence[0].clone();
                relevance.status = "hypothesis".into();
                relevance.user_confirmed = false;
                relevance.detail =
                    "Selected activity may support this workflow; its purpose is inferred".into();
                add_edge(&mut graph, &workflow, &id, "EVIDENCED_BY", &relevance);
            }
        }
    }
    conn.execute(
        "INSERT INTO discovery_graph_revisions(session_id,revision,timestamp,document_json) VALUES(?1,?2,?3,?4)",
        params![session_id, revision, timestamp, serde_json::to_string(document)?],
    )?;
    for node in &graph.nodes {
        conn.execute(
            "INSERT INTO discovery_graph_nodes(session_id,revision,id,kind,label,description) VALUES(?1,?2,?3,?4,?5,?6)",
            params![session_id, revision, node.id, node.kind, node.label, node.description],
        )?;
        insert_evidence(conn, session_id, revision, "node", &node.id, &node.evidence)?;
    }
    for edge in &graph.edges {
        conn.execute(
            "INSERT INTO discovery_graph_edges(session_id,revision,id,source,target,relationship) VALUES(?1,?2,?3,?4,?5,?6)",
            params![session_id, revision, edge.id, edge.source, edge.target, edge.relationship],
        )?;
        insert_evidence(conn, session_id, revision, "edge", &edge.id, &edge.evidence)?;
    }
    Ok(())
}

fn insert_evidence(
    conn: &Connection,
    session: &str,
    revision: i64,
    kind: &str,
    id: &str,
    evidence: &[GraphEvidence],
) -> AppResult<()> {
    for (position, item) in evidence.iter().enumerate() {
        conn.execute(
            "INSERT INTO discovery_graph_evidence(session_id,revision,fact_kind,fact_id,position,source_type,source_ref,timestamp,confidence,status,user_confirmed,detail)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![session,revision,kind,id,position as i64,item.source_type,item.source_ref,item.timestamp,item.confidence,item.status,item.user_confirmed,item.detail],
        )?;
    }
    Ok(())
}

fn derive_graph(
    session: &str,
    revision: i64,
    doc: &WorkflowDocument,
    timestamp: &str,
) -> WorkflowGraph {
    let provenance = GraphEvidence {
        source_type: "interview_document".into(),
        source_ref: format!("{session}/graph-revision/{revision}"),
        timestamp: timestamp.into(),
        confidence: doc.confidence.clamp(0.0, 1.0),
        status: if doc.confirmed {
            "user_confirmed"
        } else {
            "hypothesis"
        }
        .into(),
        user_confirmed: doc.confirmed,
        detail: if doc.confirmed {
            "Workflow explicitly confirmed by the user"
        } else {
            "Workflow synthesized from interview; requires user review"
        }
        .into(),
    };
    let mut graph = WorkflowGraph {
        session_id: session.into(),
        revision,
        nodes: vec![],
        edges: vec![],
    };
    let workflow = format!("{}:workflow", doc.id);
    let mut workflow_evidence = vec![provenance.clone()];
    for (index, evidence) in doc.evidence.iter().enumerate() {
        // Evidence text without a native event reference cannot prove observation.
        let status = match evidence.source.as_str() {
            "user_reported" => "user_reported",
            "user_confirmed" => "user_confirmed",
            _ => "hypothesis",
        };
        workflow_evidence.push(GraphEvidence {
            source_type: "interview_evidence".into(),
            source_ref: format!("{session}/graph-revision/{revision}/evidence/{index}"),
            timestamp: timestamp.into(),
            confidence: provenance.confidence,
            status: status.into(),
            user_confirmed: status == "user_confirmed",
            detail: evidence.detail.clone(),
        });
    }
    graph.nodes.push(GraphNode {
        id: workflow.clone(),
        kind: "Workflow".into(),
        label: doc.name.clone(),
        description: doc.description.clone(),
        evidence: workflow_evidence,
    });
    if !doc.business_goal.trim().is_empty() {
        let goal = format!("{}:goal", doc.id);
        add_node(
            &mut graph,
            &goal,
            "Goal",
            &doc.business_goal,
            "",
            &provenance,
        );
        add_edge(&mut graph, &workflow, &goal, "SUPPORTS_GOAL", &provenance);
    }
    for step in &doc.steps {
        let id = format!("{}:step:{}", doc.id, step.id);
        let mut evidence = provenance.clone();
        evidence.confidence = step.confidence.clamp(0.0, 1.0);
        add_node(
            &mut graph,
            &id,
            "WorkflowStep",
            &step.name,
            &step.description,
            &evidence,
        );
        if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == id) {
            for (index, detail) in step.evidence.iter().enumerate() {
                node.evidence.push(GraphEvidence {
                    source_type: "interview_step_evidence".into(),
                    source_ref: format!(
                        "{session}/graph-revision/{revision}/step/{}/evidence/{index}",
                        step.id
                    ),
                    timestamp: timestamp.into(),
                    confidence: evidence.confidence,
                    status: "hypothesis".into(),
                    user_confirmed: false,
                    detail: detail.clone(),
                });
            }
        }
        add_edge(&mut graph, &id, &workflow, "BELONGS_TO", &evidence);
        if !step.application.trim().is_empty() {
            let app = format!("{}:application:{}", doc.id, step.application);
            add_node(
                &mut graph,
                &app,
                "Application",
                &step.application,
                "",
                &evidence,
            );
            add_edge(&mut graph, &id, &app, "USES_RESOURCE", &evidence);
        }
        if !step.actor.trim().is_empty() {
            let actor = format!("{}:person:{}", doc.id, step.actor);
            add_node(&mut graph, &actor, "Person", &step.actor, "", &evidence);
            add_edge(&mut graph, &id, &actor, "PERFORMED_BY", &evidence);
        }
        if let Some(decision) = step
            .decision
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            let decision_id = format!("{id}:decision");
            add_node(
                &mut graph,
                &decision_id,
                "Decision",
                decision,
                "",
                &evidence,
            );
            add_edge(&mut graph, &id, &decision_id, "DEPENDS_ON", &evidence);
        }
        if step.requires_approval {
            let approval = format!("{id}:approval");
            add_node(
                &mut graph,
                &approval,
                "Approval",
                "User approval",
                "Approval required before this step",
                &evidence,
            );
            add_edge(&mut graph, &id, &approval, "REQUIRES_APPROVAL", &evidence);
        }
        for dependency in &step.depends_on {
            add_edge(
                &mut graph,
                &id,
                &format!("{}:step:{dependency}", doc.id),
                "DEPENDS_ON",
                &evidence,
            );
        }
    }
    for pair in doc.steps.windows(2) {
        add_edge(
            &mut graph,
            &format!("{}:step:{}", doc.id, pair[0].id),
            &format!("{}:step:{}", doc.id, pair[1].id),
            "PRECEDES",
            &provenance,
        );
    }
    for (kind, labels, relation) in [
        ("Application", &doc.applications, "USES_RESOURCE"),
        ("Resource", &doc.resources, "USES_RESOURCE"),
        ("PainPoint", &doc.bottlenecks, "BLOCKED_BY"),
        ("Decision", &doc.decisions, "DEPENDS_ON"),
    ] {
        for label in labels.iter().filter(|value| !value.trim().is_empty()) {
            let id = format!("{}:{}:{label}", doc.id, kind.to_lowercase());
            add_node(&mut graph, &id, kind, label, "", &provenance);
            add_edge(&mut graph, &workflow, &id, relation, &provenance);
        }
    }
    graph
}

fn add_node(
    graph: &mut WorkflowGraph,
    id: &str,
    kind: &str,
    label: &str,
    description: &str,
    evidence: &GraphEvidence,
) {
    if !graph.nodes.iter().any(|node| node.id == id) {
        graph.nodes.push(GraphNode {
            id: id.into(),
            kind: kind.into(),
            label: label.into(),
            description: description.into(),
            evidence: vec![evidence.clone()],
        });
    }
}

fn add_edge(
    graph: &mut WorkflowGraph,
    source: &str,
    target: &str,
    relation: &str,
    evidence: &GraphEvidence,
) {
    if !graph
        .edges
        .iter()
        .any(|edge| edge.source == source && edge.target == target && edge.relationship == relation)
    {
        graph.edges.push(GraphEdge {
            id: format!("edge-{}", graph.edges.len()),
            source: source.into(),
            target: target.into(),
            relationship: relation.into(),
            evidence: vec![evidence.clone()],
        });
    }
}

pub fn get_graph(conn: &Connection, session: &str) -> AppResult<WorkflowGraph> {
    let revision: Option<i64> = conn.query_row(
        "SELECT MAX(revision) FROM discovery_graph_revisions WHERE session_id=?1",
        [session],
        |row| row.get(0),
    )?;
    let revision = revision.unwrap_or(0);
    read_graph(conn, session, revision)
}

pub fn get_graph_revision(
    conn: &Connection,
    session: &str,
    revision: i64,
) -> AppResult<WorkflowGraph> {
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM discovery_graph_revisions WHERE session_id=?1 AND revision=?2)", params![session,revision], |row| row.get(0))?;
    if !exists {
        return Err(AppError::InvalidInput(
            "workflow graph revision not found".into(),
        ));
    }
    read_graph(conn, session, revision)
}

fn read_graph(conn: &Connection, session: &str, revision: i64) -> AppResult<WorkflowGraph> {
    let mut statement = conn.prepare("SELECT id,kind,label,description FROM discovery_graph_nodes WHERE session_id=?1 AND revision=?2 ORDER BY rowid")?;
    let mut nodes = statement
        .query_map(params![session, revision], |row| {
            Ok(GraphNode {
                id: row.get(0)?,
                kind: row.get(1)?,
                label: row.get(2)?,
                description: row.get(3)?,
                evidence: vec![],
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for node in &mut nodes {
        node.evidence = get_evidence(conn, session, revision, "node", &node.id)?;
    }
    let mut statement = conn.prepare("SELECT id,source,target,relationship FROM discovery_graph_edges WHERE session_id=?1 AND revision=?2 ORDER BY rowid")?;
    let mut edges = statement
        .query_map(params![session, revision], |row| {
            Ok(GraphEdge {
                id: row.get(0)?,
                source: row.get(1)?,
                target: row.get(2)?,
                relationship: row.get(3)?,
                evidence: vec![],
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for edge in &mut edges {
        edge.evidence = get_evidence(conn, session, revision, "edge", &edge.id)?;
    }
    Ok(WorkflowGraph {
        session_id: session.into(),
        revision,
        nodes,
        edges,
    })
}

fn get_evidence(
    conn: &Connection,
    session: &str,
    revision: i64,
    kind: &str,
    id: &str,
) -> AppResult<Vec<GraphEvidence>> {
    let mut statement = conn.prepare("SELECT source_type,source_ref,timestamp,confidence,status,user_confirmed,detail FROM discovery_graph_evidence WHERE session_id=?1 AND revision=?2 AND fact_kind=?3 AND fact_id=?4 ORDER BY position")?;
    let rows = statement.query_map(params![session, revision, kind, id], |row| {
        Ok(GraphEvidence {
            source_type: row.get(0)?,
            source_ref: row.get(1)?,
            timestamp: row.get(2)?,
            confidence: row.get(3)?,
            status: row.get(4)?,
            user_confirmed: row.get(5)?,
            detail: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_history(conn: &Connection, session: &str) -> AppResult<Vec<GraphRevision>> {
    let mut statement = conn.prepare("SELECT revision,timestamp,
        (SELECT COUNT(*) FROM discovery_graph_nodes n WHERE n.session_id=r.session_id AND n.revision=r.revision),
        (SELECT COUNT(*) FROM discovery_graph_edges e WHERE e.session_id=r.session_id AND e.revision=r.revision)
        FROM discovery_graph_revisions r WHERE session_id=?1 ORDER BY revision DESC")?;
    let rows = statement.query_map([session], |row| {
        Ok(GraphRevision {
            revision: row.get(0)?,
            timestamp: row.get(1)?,
            nodes: row.get::<_, i64>(2)? as usize,
            edges: row.get::<_, i64>(3)? as usize,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Explicit purge also works for test/legacy connections without foreign keys.
pub fn delete_session(conn: &Connection, session: &str) -> AppResult<()> {
    for table in [
        "discovery_graph_evidence",
        "discovery_graph_edges",
        "discovery_graph_nodes",
        "discovery_graph_revisions",
    ] {
        conn.execute(
            &format!("DELETE FROM {table} WHERE session_id=?1"),
            [session],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE discovery_sessions(id TEXT PRIMARY KEY, document TEXT NOT NULL DEFAULT '{}'); INSERT INTO discovery_sessions(id) VALUES('session');").unwrap();
        initialize_schema(&conn).unwrap();
        conn
    }

    fn document() -> WorkflowDocument {
        serde_json::from_value(json!({
            "id":"workflow", "sessionId":"session", "name":"Weekly report",
            "description":"Assemble a report", "businessGoal":"Share progress", "trigger":"Friday",
            "actors":["Owner"], "steps":[
                {"id":"collect","name":"Collect data","description":"Gather inputs","actor":"Owner",
                 "application":"Sheets","inputs":["Metrics"],"outputs":["Data"],"dependsOn":[],
                 "decision":null,"requiresApproval":false,"evidence":[],"confidence":0.6},
                {"id":"send","name":"Send report","description":"Deliver output","actor":"Owner",
                 "application":"Mail","inputs":["Data"],"outputs":["Report"],"dependsOn":["collect"],
                 "decision":"Is report ready?","requiresApproval":true,"evidence":[],"confidence":0.7}
            ], "applications":["Sheets"], "resources":["Metrics"], "inputs":["Metrics"],
            "outputs":["Report"], "decisions":[], "dependencies":[], "approvals":[],
            "exceptions":[], "bottlenecks":["Manual copy"], "frequency":"Weekly",
            "estimatedMinutes":30.0,"desiredOutcome":"Report delivered","automationOpportunities":[],
            "evidence":[{"source":"user_reported","detail":"I prepare a report every Friday"},
                        {"source":"observed","detail":"Provider asserted observation"}],
            "confidence":0.65,"confirmed":false,"updatedAt":100
        })).unwrap()
    }

    #[test]
    fn persists_relations_with_honest_provenance() {
        let conn = connection();
        let mut doc = document();
        doc.steps[0].evidence = vec!["I usually copy metrics manually".into()];
        replace_document(&conn, "session", &doc, "100").unwrap();
        let graph = get_graph(&conn, "session").unwrap();
        assert_eq!(graph.revision, 1);
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "DEPENDS_ON"
                && edge.source == "workflow:step:send"
                && edge.target == "workflow:step:collect"));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.relationship == "REQUIRES_APPROVAL"));
        assert!(graph.nodes.iter().any(|node| node.kind == "PainPoint"));
        for evidence in graph
            .nodes
            .iter()
            .flat_map(|node| &node.evidence)
            .chain(graph.edges.iter().flat_map(|edge| &edge.evidence))
        {
            assert!(!evidence.user_confirmed);
            assert_ne!(evidence.status, "observed");
            assert!(evidence.source_ref.starts_with("session/graph-revision/1"));
            assert_eq!(evidence.timestamp, "100");
        }
        let workflow = graph
            .nodes
            .iter()
            .find(|node| node.kind == "Workflow")
            .unwrap();
        assert_eq!(workflow.evidence[1].status, "user_reported");
        assert_eq!(workflow.evidence[2].status, "hypothesis");
        let step = graph
            .nodes
            .iter()
            .find(|node| node.id == "workflow:step:collect")
            .unwrap();
        assert_eq!(step.evidence[1].detail, "I usually copy metrics manually");
        assert_eq!(step.evidence[1].status, "hypothesis");
    }

    #[test]
    fn explicit_evidence_confirmation_survives_graph_projection() {
        let conn = connection();
        let mut doc = document();
        doc.evidence.push(super::super::WorkflowEvidence {
            source: "user_confirmed".into(),
            detail: "I reviewed this invoice rule".into(),
        });
        replace_document(&conn, "session", &doc, "100").unwrap();
        let graph = get_graph(&conn, "session").unwrap();
        let evidence = graph
            .nodes
            .iter()
            .flat_map(|node| &node.evidence)
            .find(|evidence| evidence.detail == "I reviewed this invoice rule")
            .unwrap();
        assert_eq!(evidence.status, "user_confirmed");
        assert!(evidence.user_confirmed);
        assert!(!doc.confirmed);
    }

    #[test]
    fn corrections_keep_prior_relationships_and_underlying_document() {
        let conn = connection();
        let mut doc = document();
        replace_document(&conn, "session", &doc, "100").unwrap();
        doc.steps[1].depends_on.clear();
        doc.steps[1].name = "Review report".into();
        doc.confirmed = true;
        replace_document(&conn, "session", &doc, "200").unwrap();
        let graph = get_graph(&conn, "session").unwrap();
        assert_eq!(graph.revision, 2);
        assert!(!graph.edges.iter().any(
            |edge| edge.relationship == "DEPENDS_ON" && edge.target == "workflow:step:collect"
        ));
        assert!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == "workflow:step:send")
                .unwrap()
                .evidence[0]
                .user_confirmed
        );
        let old: String=conn.query_row("SELECT document_json FROM discovery_graph_revisions WHERE session_id='session' AND revision=1",[],|row|row.get(0)).unwrap();
        let old: WorkflowDocument = serde_json::from_str(&old).unwrap();
        assert_eq!(old.steps[1].depends_on, vec!["collect"]);
        assert!(!old.confirmed);
        let previous:i64=conn.query_row("SELECT COUNT(*) FROM discovery_graph_edges WHERE revision=1 AND relationship='DEPENDS_ON' AND target='workflow:step:collect'",[],|row|row.get(0)).unwrap();
        assert_eq!(previous, 1);
        assert_eq!(get_history(&conn, "session").unwrap().len(), 2);
        let old_graph = get_graph_revision(&conn, "session", 1).unwrap();
        assert_eq!(
            old_graph
                .nodes
                .iter()
                .find(|node| node.id == "workflow:step:send")
                .unwrap()
                .label,
            "Send report"
        );
        assert!(get_graph_revision(&conn, "session", 3).is_err());
    }

    #[test]
    fn native_context_observations_do_not_confirm_workflow_relevance() {
        let conn = connection();
        let mut doc = document();
        doc.confirmed = true;
        let detail = "Recorded collector activity in Sheets at 2026-10-09T10:00:00+00:00";
        doc.evidence.push(super::super::WorkflowEvidence {
            source: "observed".into(),
            detail: detail.into(),
        });
        let session = json!({"id":"session","status":"active","workflow":doc,"messages":[],"missingInformation":[],"createdAt":100,"updatedAt":100,"revision":0,
            "threadContext":{"version":1,"subject":"Report","signalCount":1,"apps":["Sheets"],"modifiedFiles":[],"observedFrom":null,"observedThrough":null,
                "events":[{"observedAt":"2026-10-09T10:00:00+00:00","appName":"Sheets","source":"collector","title":null,"resource":null,"searchQuery":null,"observedActiveSeconds":null}]}});
        conn.execute(
            "UPDATE discovery_sessions SET document=?1 WHERE id='session'",
            [session.to_string()],
        )
        .unwrap();
        replace_document(&conn, "session", &doc, "200").unwrap();
        let graph = get_graph(&conn, "session").unwrap();
        let observation = graph
            .nodes
            .iter()
            .find(|node| node.kind == "ActivityObservation")
            .unwrap();
        assert_eq!(observation.evidence[0].status, "observed");
        assert_eq!(
            observation.evidence[0].timestamp,
            "2026-10-09T10:00:00+00:00"
        );
        assert_eq!(observation.evidence[0].source_ref, "session/context/0");
        let relevance = graph
            .edges
            .iter()
            .find(|edge| edge.relationship == "EVIDENCED_BY")
            .unwrap();
        assert_eq!(relevance.evidence[0].status, "hypothesis");
        assert!(!relevance.evidence[0].user_confirmed);
    }

    #[test]
    fn deleting_a_session_cascades_all_revisions_and_evidence() {
        let conn = connection();
        replace_document(&conn, "session", &document(), "100").unwrap();
        conn.execute("DELETE FROM discovery_sessions WHERE id='session'", [])
            .unwrap();
        assert!(get_graph(&conn, "session").unwrap().nodes.is_empty());
        assert!(get_history(&conn, "session").unwrap().is_empty());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM discovery_graph_evidence", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn explicit_purge_works_without_foreign_keys_and_preserves_other_sessions() {
        let conn = connection();
        conn.execute("INSERT INTO discovery_sessions(id) VALUES('other')", [])
            .unwrap();
        let mut other = document();
        other.session_id = "other".into();
        replace_document(&conn, "session", &document(), "100").unwrap();
        replace_document(&conn, "other", &other, "100").unwrap();
        conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        delete_session(&conn, "session").unwrap();
        assert!(get_graph(&conn, "session").unwrap().nodes.is_empty());
        assert!(!get_graph(&conn, "other").unwrap().nodes.is_empty());
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM discovery_graph_evidence WHERE session_id='session'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn invalid_dependencies_do_not_create_a_revision() {
        let conn = connection();
        let mut doc = document();
        doc.steps[1].depends_on = vec!["missing".into()];
        assert!(replace_document(&conn, "session", &doc, "100").is_err());
        assert!(get_history(&conn, "session").unwrap().is_empty());
    }

    #[test]
    fn failure_rolls_back_snapshot_with_the_canonical_transaction() {
        let mut conn = connection();
        conn.execute_batch("CREATE TRIGGER reject_graph BEFORE INSERT ON discovery_graph_edges BEGIN SELECT RAISE(ABORT,'reject'); END;").unwrap();
        {
            let tx = conn.transaction().unwrap();
            assert!(replace_document(&tx, "session", &document(), "100").is_err());
        }
        assert!(get_graph(&conn, "session").unwrap().nodes.is_empty());
        assert!(get_history(&conn, "session").unwrap().is_empty());
    }
}
