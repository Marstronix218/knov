import { FormEvent, useState } from "react";
import { LoaderCircle, MessageSquare, Pause, Play, Plus, SkipForward, Trash2 } from "lucide-react";
import { EmptyState, errorMessage, PageHeader, ResourceState } from "../components/ui";
import { MarkdownMessage } from "../components/MarkdownMessage";
import { useResource } from "../hooks/useResource";
import { api, isDesktopRuntime } from "../lib/api";
import type { DashboardData, InterviewSession, ThreadContext, WorkflowDocument } from "../types";
import { WorkflowEditor } from "./WorkflowEditor";
import "./discovery.css";

export function DiscoveryPage({ threadContexts }: { threadContexts: (dashboard: DashboardData) => ThreadContext[] }) {
  const sessions = useResource(() => api.discoverySessions(), []);
  const dashboard = useResource(() => api.dashboard("7d"), []);
  const settings = useResource(() => api.settings(), []);
  const [selected, setSelected] = useState<string>();
  const [description, setDescription] = useState("");
  const [contextIndex, setContextIndex] = useState("");
  const [answer, setAnswer] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [editing, setEditing] = useState<WorkflowDocument>();
  const contexts = dashboard.data ? threadContexts(dashboard.data) : [];
  const session = sessions.data?.find((item) => item.id === selected);
  const providerReady = settings.data?.hasProviderKey;

  const run = async (operation: () => Promise<InterviewSession>) => {
    setBusy(true);
    setError("");
    try {
      const updated = await operation();
      sessions.setData((current) => [updated, ...(current ?? []).filter((item) => item.id !== updated.id)]);
      setSelected(updated.id);
      setConfirmDelete(false);
      setAnswer("");
    } catch (cause) { setError(errorMessage(cause)); }
    finally { setBusy(false); }
  };
  const deleteSession = async () => {
    if (!session) return;
    setBusy(true); setError("");
    try {
      await api.deleteDiscovery(session.id);
      sessions.setData((items) => items?.filter((item) => item.id !== session.id));
      setSelected(undefined); setConfirmDelete(false); setAnswer("");
    } catch (cause) { setError(errorMessage(cause)); }
    finally { setBusy(false); }
  };
  const start = (event: FormEvent) => {
    event.preventDefault();
    void run(() => api.startDiscovery(description.trim(), contextIndex === "" ? undefined : contexts[Number(contextIndex)]));
  };
  const advance = (action: "answer" | "skip" | "end") => {
    if (session) void run(() => api.advanceDiscovery(session.id, action, action === "answer" ? answer.trim() : undefined));
  };

  return <div className="page discovery-page">
    <PageHeader eyebrow="Observe → ask → understand" title="Workflow Discovery" description="Explain how your work happens. Knov asks focused follow-up questions and saves an editable workflow on this Mac." actions={<button className="ghost-button" disabled={busy} onClick={() => { setSelected(undefined); setError(""); setConfirmDelete(false); }}><Plus size={15} /> New interview</button>} />
    {!isDesktopRuntime() ? <EmptyState title="Open Workflow Discovery in the desktop app" detail="Interviews and workflow evidence are stored in your local database. Browser preview does not run or save discovery sessions." /> : <>
      <div className="discovery-boundary">Saved locally · Answers, the current workflow, and selected thread metadata are sent to your configured {settings.data?.provider ?? "AI"} provider when you answer or skip a question. Starting, pausing, finishing, and editing stay local. No actions are executed.</div>
      {!providerReady && !settings.loading && <p className="notice info">{settings.error ?? "Add a provider key in Settings to answer or skip questions."} <a href="#/settings">Open Settings</a></p>}
      {error && <p role="alert" className="notice error">{error}</p>}
      <div className="discovery-layout">
        <aside className="discovery-sessions" aria-label="Saved interviews"><h2>Saved interviews</h2>
          {sessions.error && <p role="alert" className="notice error">{sessions.error} <button className="ghost-button" onClick={() => void sessions.reload()}>Retry</button></p>}
          <ResourceState {...sessions}>{(items) => items.length ? items.map((item) => <button key={item.id} className={`discovery-session ${selected === item.id ? "selected" : ""}`} aria-pressed={selected === item.id} disabled={busy} onClick={() => { setSelected(item.id); setAnswer(""); setError(""); setConfirmDelete(false); }}><strong>{item.workflow.name || item.threadContext?.subject || "New workflow"}</strong><span>{item.status} · {new Date(item.updatedAt * 1000).toLocaleDateString()}</span></button>) : <p className="field-hint">Start a conversation to save your first interview. You can pause and return anytime.</p>}</ResourceState>
        </aside>
        <section className="discovery-conversation" aria-label="Workflow interview">
          {!session ? <form className="stack-form discovery-start" onSubmit={start}>
            <MessageSquare size={28} /><h2>Walk through a real workflow</h2><p>Allow about 10–15 minutes. Start with one recurring task; you can stop whenever you have enough detail.</p>
            <label>Optional work thread<select value={contextIndex} disabled={dashboard.loading} onChange={(event) => setContextIndex(event.target.value)}><option value="">Describe a workflow from scratch</option>{contexts.map((context, index) => <option key={`${context.subject}-${index}`} value={index}>{context.subject} · {context.signalCount} signals</option>)}</select></label>
            {dashboard.error && <p className="notice error" role="alert">Thread context could not load: {dashboard.error} <button type="button" onClick={() => void dashboard.reload()}>Retry</button></p>}
            {contextIndex !== "" && contexts[Number(contextIndex)] && <details><summary>Review selected metadata before sharing</summary><pre>{JSON.stringify(contexts[Number(contextIndex)], null, 2)}</pre><p>Related intent is a hypothesis until you confirm it.</p></details>}
            <label>What work would you like to understand?<textarea rows={4} required={contextIndex === ""} value={description} maxLength={4000} onChange={(event) => setDescription(event.target.value)} placeholder="For example: I reconcile incoming invoices every Friday…" /></label>
            <button className="primary-button" disabled={busy || (!description.trim() && contextIndex === "")}>{busy ? <LoaderCircle className="spin" size={16} /> : <Plus size={16} />} Start interview</button>
          </form> : <>
            <header className="discovery-session-heading"><div><h2>{session.workflow.name || "Your workflow"}</h2><span>{session.status}{session.threadContext ? ` · Context: ${session.threadContext.subject}` : " · From scratch"}</span></div><div className="discovery-actions"><button className="ghost-button" disabled={busy} onClick={() => setEditing(session.workflow)}>Review & edit workflow</button>{confirmDelete ? <><span className="field-hint">Delete interview, workflow, evidence and revision history from this Mac?</span><button className="danger-button" disabled={busy} onClick={() => void deleteSession()}>Delete interview</button><button className="ghost-button" disabled={busy} onClick={() => setConfirmDelete(false)}>Keep interview</button></> : <button className="ghost-button" disabled={busy} onClick={() => setConfirmDelete(true)}><Trash2 size={14} /> Delete</button>}</div></header>
            <div className="discovery-messages" aria-label="Interview messages" aria-live="polite">{session.messages.map((message, index) => <article key={`${session.id}-${index}`} className={`discovery-message ${message.role}`}><strong>{message.role === "assistant" ? "Knov · interviewer" : "You"}</strong><MarkdownMessage>{message.content}</MarkdownMessage></article>)}{busy && <p role="status"><LoaderCircle className="spin" size={16} /> Saving and preparing your next question…</p>}</div>
            {session.missingInformation.length > 0 && <details className="discovery-gaps"><summary>{session.missingInformation.length} areas still need clarification</summary><ul>{session.missingInformation.map((item) => <li key={item}>{item}</li>)}</ul></details>}
            {session.status === "active" ? <form className="stack-form discovery-answer" onSubmit={(event) => { event.preventDefault(); advance("answer"); }}><label>Your answer<textarea rows={3} value={answer} disabled={busy} maxLength={6000} onChange={(event) => setAnswer(event.target.value)} placeholder="Describe what happens, including a concrete example." /></label><div className="discovery-actions"><button className="primary-button" disabled={busy || !providerReady || !answer.trim()}>Send answer</button><button type="button" className="ghost-button" disabled={busy || !providerReady} onClick={() => advance("skip")}><SkipForward size={14} /> Skip question</button><button type="button" className="ghost-button" disabled={busy} onClick={() => void run(() => api.setDiscoveryStatus(session.id, "paused"))}><Pause size={14} /> Pause</button><button type="button" className="ghost-button" disabled={busy} onClick={() => advance("end")}>Finish & review</button></div></form> : session.status === "paused" ? <div className="discovery-actions"><p>Progress is saved. Resume when you are ready.</p><button className="primary-button" disabled={busy} onClick={() => void run(() => api.setDiscoveryStatus(session.id, "active"))}><Play size={15} /> Resume interview</button></div> : <div className="discovery-actions"><p>Interview complete. Review the draft before confirming its conclusions.</p><button className="primary-button" disabled={busy} onClick={() => setEditing(session.workflow)}>Review workflow</button><a className="ghost-button" href="#/knowledge">Inspect knowledge</a></div>}
          </>}
        </section>
      </div>
    </>}
    {editing && <WorkflowEditor workflow={editing} onClose={() => setEditing(undefined)} onSaved={(updated) => { sessions.setData((items) => items?.map((item) => item.id === updated.sessionId ? { ...item, workflow: updated } : item)); setEditing(undefined); void sessions.reload(); }} />}
  </div>;
}
