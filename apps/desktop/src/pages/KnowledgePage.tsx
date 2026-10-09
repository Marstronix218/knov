import { useEffect, useState } from "react";
import { EmptyState, PageHeader, ResourceState } from "../components/ui";
import { useResource } from "../hooks/useResource";
import { api, isDesktopRuntime } from "../lib/api";
import type { GraphEvidence, InterviewGraph, WorkflowDocument } from "../types";
import { WorkflowDiagram, WorkflowEditor, WorkflowEvidence } from "./WorkflowEditor";
import "./discovery.css";

function graphTime(timestamp: string): string {
  const numeric = Number(timestamp);
  const date = new Date(timestamp.trim() && Number.isFinite(numeric) ? numeric * 1000 : timestamp);
  return Number.isFinite(date.getTime()) ? date.toLocaleString() : timestamp;
}

export function KnowledgePage() {
  const workflows = useResource(() => api.discoveredWorkflows(), []);
  const [selectedId, setSelectedId] = useState<string>();
  const [editing, setEditing] = useState<WorkflowDocument>();
  const selected = workflows.data?.find((workflow) => workflow.id === selectedId) ?? workflows.data?.[0];
  return <div className="page knowledge-page"><PageHeader eyebrow="Local workflow memory" title="Knowledge" description="Inspect the process, tools, dependencies, and evidence gathered through interviews. Correct the workflow to update its graph while keeping source history." actions={<a href="#/discovery" className="primary-button">Start discovery</a>} />
    {!isDesktopRuntime() ? <EmptyState title="Knowledge lives in the desktop app" detail="Open Knov on this Mac to inspect persisted workflows and their evidence graph." /> : <>
      {workflows.error && <p className="notice error" role="alert">{workflows.error} <button className="ghost-button" onClick={() => void workflows.reload()}>Retry</button></p>}
      <ResourceState {...workflows}>{(items) => !items.length ? <EmptyState title="No workflow knowledge yet" detail="Start an interview from a work thread or describe a recurring task. Its structured workflow and evidence will be saved here." action={<a href="#/discovery" className="primary-button">Start an interview</a>} /> : <div className="discovery-layout"><aside className="discovery-sessions" aria-label="Interview workflows"><h2>My workflows</h2>{items.map((workflow) => <button className={`discovery-session ${selected?.id === workflow.id ? "selected" : ""}`} aria-pressed={selected?.id === workflow.id} key={workflow.id} onClick={() => setSelectedId(workflow.id)}><strong>{workflow.name}</strong><span>{workflow.confirmed ? "User confirmed" : "Draft hypothesis"} · {workflow.steps.length} steps</span></button>)}</aside>{selected && <section className="discovery-conversation"><header className="discovery-session-heading"><div><h2>{selected.name}</h2><p>{selected.description}</p></div><button className="ghost-button" onClick={() => setEditing(selected)}>Correct workflow</button></header><div className="discovery-knowledge-body"><p><strong>Goal:</strong> {selected.businessGoal || "Not specified"}</p><p><strong>Trigger:</strong> {selected.trigger || "Not specified"}</p><p><strong>Desired outcome:</strong> {selected.desiredOutcome || "Not specified"}</p><WorkflowDiagram key={selected.id} workflow={selected} /><WorkflowEvidence workflow={selected} />{selected.automationOpportunities.length > 0 && <details><summary>Possible automation · hypotheses for review</summary><ul>{selected.automationOpportunities.map((idea, index) => <li key={index}>{idea}</li>)}</ul><p className="field-hint">These ideas do not create skills or authorize execution.</p></details>}<KnowledgeGraph key={selected.sessionId} workflow={selected} /></div></section>}</div>}</ResourceState>
    </>}{editing && <WorkflowEditor workflow={editing} onClose={() => setEditing(undefined)} onSaved={(updated) => { workflows.setData((items) => items?.map((item) => item.id === updated.id ? updated : item)); setEditing(undefined); }} />}
  </div>;
}

function KnowledgeGraph({ workflow }: { workflow: WorkflowDocument }) {
  const [revision, setRevision] = useState<number>();
  const graph = useResource(() => api.interviewGraph(workflow.sessionId, revision), [workflow.sessionId, workflow.updatedAt, revision]);
  const history = useResource(() => api.interviewGraphHistory(workflow.sessionId), [workflow.sessionId, workflow.updatedAt]);
  const [selected, setSelected] = useState<string>();
  const node = graph.data?.nodes.find((item) => item.id === selected);
  useEffect(() => { setSelected(undefined); setRevision(undefined); }, [workflow.updatedAt]);
  return <section className="discovery-graph" aria-label="Workflow knowledge graph"><h3>Knowledge graph</h3><p className="field-hint">Select an entity to inspect its relationships and source evidence. Corrections append a new local graph revision.</p>
    {graph.error && <p className="notice error" role="alert">{graph.error} <button className="ghost-button" onClick={() => void graph.reload()}>Retry graph</button></p>}
    <label className="discovery-revision">Graph revision<select aria-label="Graph revision" value={revision ?? ""} onChange={(event) => { setRevision(event.target.value === "" ? undefined : Number(event.target.value)); setSelected(undefined); }}><option value="">Latest</option>{history.data?.map((item) => <option key={item.revision} value={item.revision}>Revision {item.revision} · {graphTime(item.timestamp)}</option>)}</select></label>{revision !== undefined && <p className="notice info">Inspecting historical evidence. Correct workflow edits the latest document.</p>}
    {graph.loading && graph.data && <p role="status" className="field-hint">Loading selected graph revision…</p>}
    <ResourceState {...graph} data={graph.loading ? undefined : graph.data}>{(data) => <><p className="field-hint">Revision {data.revision} · {data.nodes.length} entities · {data.edges.length} relationships</p><div className="discovery-graph-nodes">{data.nodes.map((item) => <button key={item.id} type="button" aria-pressed={selected === item.id} className={selected === item.id ? "selected" : ""} onClick={() => setSelected(selected === item.id ? undefined : item.id)}><small>{item.kind}</small><strong>{item.label}</strong></button>)}</div>{node && <section className="discovery-step-detail"><h4>{node.label}</h4><p>{node.description}</p><GraphSources evidence={node.evidence} /><h4>Relationships</h4><GraphRelationships graph={data} nodeId={node.id} /></section>}<details><summary>All relationships</summary><GraphRelationships graph={data} /></details></>}</ResourceState>
    <details><summary>Revision history</summary>{history.loading ? <p>Loading local revisions…</p> : history.error ? <p role="alert">{history.error}</p> : <ol>{history.data?.map((revision) => <li key={revision.revision}>Revision {revision.revision} · {graphTime(revision.timestamp)} · {revision.nodes} entities, {revision.edges} relationships</li>)}</ol>}</details>
  </section>;
}

function GraphSources({ evidence }: { evidence: GraphEvidence[] }) {
  return <details><summary>Source evidence · {evidence.length}</summary>{evidence.length ? <ul>{evidence.map((item, index) => <li key={index}><strong>{item.sourceType} · {item.status}{item.userConfirmed ? " · user confirmed" : ""}</strong><p>{item.detail}</p><small>{Math.round(item.confidence * 100)}% confidence · {graphTime(item.timestamp)} · {item.sourceRef}</small></li>)}</ul> : <p>No evidence recorded.</p>}</details>;
}

function GraphRelationships({ graph, nodeId }: { graph: InterviewGraph; nodeId?: string }) {
  const edges = graph.edges.filter((edge) => !nodeId || edge.source === nodeId || edge.target === nodeId);
  const label = (id: string) => graph.nodes.find((node) => node.id === id)?.label ?? id;
  return edges.length ? <ul className="discovery-relations">{edges.map((edge) => <li key={edge.id}><span>{label(edge.source)} <strong>→ {edge.relationship.replace(/_/g, " ").toLowerCase()} →</strong> {label(edge.target)}</span><GraphSources evidence={edge.evidence} /></li>)}</ul> : <p>No relationships recorded.</p>;
}
