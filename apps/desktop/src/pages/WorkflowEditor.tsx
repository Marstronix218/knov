import { FormEvent, useState } from "react";
import { ArrowDown, ArrowUp, LoaderCircle, Plus, Trash2 } from "lucide-react";
import { errorMessage, Modal } from "../components/ui";
import { api } from "../lib/api";
import type { DiscoveredStep, WorkflowDocument } from "../types";

const list = (value: string) => value.split("\n").map((item) => item.trim()).filter(Boolean);
const textFields = [
  ["name", "Workflow name"], ["description", "Description"], ["businessGoal", "Business goal"], ["trigger", "Trigger"], ["frequency", "Frequency"], ["desiredOutcome", "Desired outcome / success criteria"],
] as const;
const listFields = [
  ["actors", "Actors"], ["applications", "Applications"], ["resources", "Resources"], ["inputs", "Inputs"], ["outputs", "Outputs"], ["decisions", "Decision points / business rules"], ["dependencies", "Dependencies"], ["approvals", "Approvals"], ["exceptions", "Exceptions"], ["bottlenecks", "Frustrations / bottlenecks"], ["automationOpportunities", "Possible automation (hypotheses)"],
] as const;

export function WorkflowDiagram({ workflow }: { workflow: WorkflowDocument }) {
  const [selected, setSelected] = useState<string>();
  const step = workflow.steps.find((item) => item.id === selected);
  return <div className="discovery-diagram"><h3>Process · {workflow.confirmed ? "User confirmed" : "Draft hypothesis"}</h3>
    {!workflow.steps.length ? <p className="field-hint">Steps will appear as the workflow becomes clearer.</p> : <ol className="discovery-process" aria-label="Ordered workflow steps">{workflow.steps.map((item, index) => <li key={item.id}><button type="button" aria-pressed={selected === item.id} onClick={() => setSelected(selected === item.id ? undefined : item.id)}><small>Step {index + 1} · {Math.round(item.confidence * 100)}% confidence</small><strong>{item.name}</strong><span>{[item.actor, item.application].filter(Boolean).join(" · ") || "Actor / tool not yet specified"}</span>{item.decision && <em>Decision: {item.decision}</em>}{item.requiresApproval && <em>Human approval required</em>}{item.dependsOn.length > 0 && <span>Depends on: {item.dependsOn.map((id) => workflow.steps.find((candidate) => candidate.id === id)?.name ?? id).join(", ")}</span>}</button></li>)}</ol>}
    {step && <section className="discovery-step-detail" aria-label={`Details: ${step.name}`}><h4>{step.name}</h4><p>{step.description}</p><p>Inputs: {step.inputs.join(", ") || "Not specified"}</p><p>Outputs: {step.outputs.join(", ") || "Not specified"}</p><h4>Supporting evidence</h4>{step.evidence.length ? <ul>{step.evidence.map((value, index) => <li key={index}>{value}</li>)}</ul> : <p>No supporting evidence provided yet.</p>}</section>}
    {workflow.bottlenecks.length > 0 && <p className="notice info">Bottlenecks: {workflow.bottlenecks.join(" · ")}</p>}
  </div>;
}

export function WorkflowEvidence({ workflow }: { workflow: WorkflowDocument }) {
  return <details className="discovery-evidence"><summary>Evidence & provenance · {workflow.evidence.length} sources · {Math.round(workflow.confidence * 100)}% confidence</summary><p>Confidence is a model estimate. Confirmation records your review; underlying source history is retained.</p>{workflow.evidence.length ? <ul>{workflow.evidence.map((item, index) => <li key={index}><strong>{item.source.replace(/_/g, " ")}</strong><span>{item.detail}</span></li>)}</ul> : <p>No evidence recorded yet.</p>}</details>;
}

export function WorkflowEditor({ workflow, onClose, onSaved }: { workflow: WorkflowDocument; onClose: () => void; onSaved: (updated: WorkflowDocument) => void }) {
  const [draft, setDraft] = useState(() => structuredClone(workflow));
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const patch = (value: Partial<WorkflowDocument>) => setDraft((current) => ({ ...current, ...value }));
  const patchStep = (id: string, value: Partial<DiscoveredStep>) => setDraft((current) => ({ ...current, steps: current.steps.map((step) => step.id === id ? { ...step, ...value } : step) }));
  const move = (index: number, delta: number) => {
    const steps = [...draft.steps];
    [steps[index], steps[index + delta]] = [steps[index + delta], steps[index]];
    patch({ steps: steps.map((step, position) => ({ ...step, dependsOn: step.dependsOn.filter((id) => steps.slice(0, position).some((prior) => prior.id === id)) })) });
  };
  const remove = (id: string) => patch({ steps: draft.steps.filter((step) => step.id !== id).map((step) => ({ ...step, dependsOn: step.dependsOn.filter((dependency) => dependency !== id) })) });
  const add = () => patch({ steps: [...draft.steps, { id: crypto.randomUUID(), name: "", description: "", actor: "", application: "", inputs: [], outputs: [], dependsOn: [], decision: null, requiresApproval: false, evidence: [], confidence: 1 }] });
  const submit = async (event: FormEvent) => {
    event.preventDefault(); setSaving(true); setError("");
    try { onSaved(await api.saveDiscoveredWorkflow(draft)); }
    catch (cause) { setError(errorMessage(cause)); }
    finally { setSaving(false); }
  };
  return <Modal title={`Review workflow: ${workflow.name || "Untitled"}`} wide onClose={() => { if (!saving) onClose(); }}>
    <form className="stack-form discovery-editor" onSubmit={(event) => void submit(event)}>
      <p className="field-hint">Changes are saved locally as user corrections. Review conclusions before confirming. This does not authorize an automation.</p>
      <fieldset disabled={saving}><legend>Workflow understanding</legend><div className="discovery-field-grid">{textFields.map(([key, label]) => <label key={key}>{label}<textarea rows={key === "description" ? 3 : 2} required={key === "name"} maxLength={4000} value={draft[key]} onChange={(event) => patch({ [key]: event.target.value })} /></label>)}<label>Estimated minutes per run<input type="number" min="0" max="100000" value={draft.estimatedMinutes ?? ""} onChange={(event) => patch({ estimatedMinutes: event.target.value === "" ? null : Number(event.target.value) })} /></label></div></fieldset>
      <fieldset disabled={saving}><legend>Steps, in order</legend><p className="field-hint">Dependencies refer to earlier steps. Moving a step removes dependencies that no longer precede it.</p>{draft.steps.map((step, index) => <section className="discovery-edit-step" key={step.id}><header><h3>Step {index + 1}</h3><button type="button" className="ghost-button" aria-label={`Move step ${index + 1} up`} disabled={index === 0} onClick={() => move(index, -1)}><ArrowUp size={14} /></button><button type="button" className="ghost-button" aria-label={`Move step ${index + 1} down`} disabled={index === draft.steps.length - 1} onClick={() => move(index, 1)}><ArrowDown size={14} /></button><button type="button" className="ghost-button" aria-label={`Remove step ${index + 1}`} onClick={() => remove(step.id)}><Trash2 size={14} /></button></header><div className="discovery-field-grid">
        <label>Step name<input required maxLength={300} value={step.name} onChange={(event) => patchStep(step.id, { name: event.target.value })} /></label><label>What happens<textarea value={step.description} maxLength={4000} onChange={(event) => patchStep(step.id, { description: event.target.value })} /></label><label>Actor<input value={step.actor} onChange={(event) => patchStep(step.id, { actor: event.target.value })} /></label><label>Application<input value={step.application} onChange={(event) => patchStep(step.id, { application: event.target.value })} /></label><label>Inputs (one per line)<textarea value={step.inputs.join("\n")} onChange={(event) => patchStep(step.id, { inputs: event.target.value.split("\n") })} onBlur={() => patchStep(step.id, { inputs: list(step.inputs.join("\n")) })} /></label><label>Outputs (one per line)<textarea value={step.outputs.join("\n")} onChange={(event) => patchStep(step.id, { outputs: event.target.value.split("\n") })} onBlur={() => patchStep(step.id, { outputs: list(step.outputs.join("\n")) })} /></label><label>Decision / rule<textarea value={step.decision ?? ""} onChange={(event) => patchStep(step.id, { decision: event.target.value || null })} /></label>
        <fieldset><legend>Depends on</legend>{draft.steps.slice(0, index).map((other) => <label className="discovery-check" key={other.id}><input type="checkbox" checked={step.dependsOn.includes(other.id)} onChange={(event) => patchStep(step.id, { dependsOn: event.target.checked ? [...step.dependsOn, other.id] : step.dependsOn.filter((id) => id !== other.id) })} />{other.name || "Unnamed step"}</label>)}</fieldset>
        <label className="discovery-check"><input type="checkbox" checked={step.requiresApproval} onChange={(event) => patchStep(step.id, { requiresApproval: event.target.checked })} />Requires human approval</label>
      </div></section>)}<button type="button" className="ghost-button" onClick={add}><Plus size={14} /> Add step</button></fieldset>
      <details><summary>Actors, resources, exceptions and opportunities</summary><div className="discovery-field-grid">{listFields.map(([key, label]) => <label key={key}>{label} (one per line)<textarea disabled={saving} value={draft[key].join("\n")} maxLength={8000} onChange={(event) => patch({ [key]: event.target.value.split("\n") })} onBlur={() => patch({ [key]: list(draft[key].join("\n")) })} /></label>)}</div></details>
      <WorkflowDiagram workflow={draft} /><WorkflowEvidence workflow={draft} />
      <label className="discovery-check"><input disabled={saving} type="checkbox" checked={draft.confirmed} onChange={(event) => patch({ confirmed: event.target.checked })} />I confirm this workflow describes my work</label>
      {error && <p className="notice error" role="alert">{error}</p>}<div className="modal-actions"><button type="button" className="ghost-button" disabled={saving} onClick={onClose}>Cancel</button><button className="primary-button" disabled={saving || !draft.name.trim()}>{saving && <LoaderCircle className="spin" size={15} />}Save workflow</button></div>
    </form>
  </Modal>;
}
