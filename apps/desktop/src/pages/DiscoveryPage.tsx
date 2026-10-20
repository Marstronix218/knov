import { FormEvent, KeyboardEvent, useState } from "react";
import { ArrowLeft, BadgeDollarSign, CalendarClock, Check, ClipboardList, FilePenLine, LoaderCircle, MessageSquare, Pause, Play, Plus, ReceiptText, SkipForward, Trash2 } from "lucide-react";
import { EmptyState, errorMessage, PageHeader, ResourceState } from "../components/ui";
import { MarkdownMessage } from "../components/MarkdownMessage";
import { useResource } from "../hooks/useResource";
import { api, isDesktopRuntime } from "../lib/api";
import { revenueApi } from "../lib/revenueApi";
import { setSimulationMode, useSimulationMode } from "../state/simulationMode";
import type { DashboardData, InterviewSession, ThreadContext, WorkflowDocument } from "../types";
import { WorkflowEditor } from "./WorkflowEditor";
import "./discovery.css";

/** Mirrors the backend's revenue topics; the backend owns the questions. */
const TOPICS = [
  { id: "change_requests", title: "Change requests and extra work", blurb: "What counts as extra, when you charge for it, and who approves it.", icon: FilePenLine },
  { id: "invoicing", title: "Invoicing and getting paid", blurb: "When invoices go out, payment terms, and what gets forgotten.", icon: ReceiptText },
  { id: "follow_ups", title: "Proposals and follow-ups", blurb: "The promises you make to clients and how you keep track of them.", icon: CalendarClock },
  { id: "pricing", title: "Pricing and quoting", blurb: "How you price work and how the quote becomes the contract.", icon: BadgeDollarSign },
];

const STATUS_LABEL: Record<InterviewSession["status"], string> = { active: "in progress", paused: "paused", completed: "complete" };

/** The essentials Knov needs to reason about one part of the business. */
function coverage(workflow: WorkflowDocument) {
  const essentials = [
    workflow.businessGoal.trim(),
    workflow.trigger.trim(),
    workflow.steps.length,
    workflow.decisions.length || workflow.approvals.length,
    workflow.bottlenecks.length,
    workflow.frequency.trim(),
    workflow.desiredOutcome.trim(),
  ];
  return { known: essentials.filter(Boolean).length, total: essentials.length };
}

export function DiscoveryPage({ threadContexts }: { threadContexts: (dashboard: DashboardData) => ThreadContext[] }) {
  const simulation = useSimulationMode();
  const sessions = useResource(() => api.discoverySessions(simulation), [simulation]);
  const catalog = useResource(() => (simulation && isDesktopRuntime() ? revenueApi.simulationCatalog() : Promise.resolve(undefined)), [simulation]);
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
  // Scripted sandbox interviews need no provider: the questions are fixed and nothing is sent anywhere.
  const scripted = Boolean(session?.workflow.commercialContext?.simulation);
  const providerReady = Boolean(settings.data?.hasProviderKey) || scripted;
  const business = catalog.data?.businesses.find((item) => item.id === catalog.data?.active);

  const run = async (operation: () => Promise<InterviewSession>, keepAnswer = false) => {
    setBusy(true);
    setError("");
    try {
      const updated = await operation();
      sessions.setData((current) => [updated, ...(current ?? []).filter((item) => item.id !== updated.id)]);
      setSelected(updated.id);
      setConfirmDelete(false);
      if (!keepAnswer) setAnswer("");
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
  // A tapped suggestion is sent as the answer; any typed draft stays in the box.
  const send = (action: "answer" | "skip" | "end", text = answer.trim()) => {
    if (session) void run(() => api.advanceDiscovery(session.id, action, action === "answer" ? text : undefined), action === "answer" && text !== answer.trim());
  };
  const close = () => { setSelected(undefined); setError(""); setConfirmDelete(false); setAnswer(""); };

  return <div className="page discovery-page">
    <PageHeader eyebrow={simulation ? `Simulation · ${business?.name ?? "fictional business"}` : "Teach Knov your business"} title="AI interview" description="Answer a few questions about how you sell, deliver, and bill. Knov learns what counts as billable, who approves what, and where money slips through." actions={session && <button className="ghost-button" disabled={busy} onClick={close}><Plus size={15} /> New interview</button>} />
    {!isDesktopRuntime() ? <EmptyState title="Open the AI interview in the desktop app" detail="Interviews are stored in your local database. Browser preview does not run or save interviews." /> : <>
      {simulation && <p className="notice info">Simulation · Interviews here are scripted for {business?.name ?? "the simulated business"}: Knov's questions don't change with your answers, and nothing is sent to an AI provider. <button className="link-button" onClick={() => setSimulationMode(false)}>Show my actual interviews</button></p>}
      {!simulation && !providerReady && !settings.loading && <p className="notice info">{settings.error ?? "The interview needs an AI to ask follow-up questions."} <a href="#/settings">Connect an AI in Settings</a></p>}
      {error && <p role="alert" className="notice error">{error}</p>}
      {sessions.error && <p role="alert" className="notice error">{sessions.error} <button className="ghost-button" onClick={() => void sessions.reload()}>Retry</button></p>}

      {!session && simulation ? <div className="interview-start">
        <section aria-label="Scripted interview">
          <h2>{business ? `Interview ${business.owner.name} of ${business.name}` : "Choose a simulated business first"}</h2>
          {business ? <div className="interview-topics"><button type="button" disabled={busy} onClick={() => void run(() => revenueApi.simulationInterview())}>
            <ClipboardList size={20} /><strong>{business.interviewName}</strong><small>{business.profile}</small><span>About 5 minutes · scripted</span>
          </button></div> : <p className="field-hint">Pick a business on the Simulation page, then start its interview from there or here. <a href="#/simulation">Open the simulation</a></p>}
        </section>
        <section aria-label="Saved interviews">
          <h2>Sandbox interviews</h2>
          <ResourceState {...sessions}>{(items) => items.length ? <div className="interview-saved">{items.map((item) => {
            const { known, total } = coverage(item.workflow);
            return <button key={item.id} type="button" disabled={busy} onClick={() => { setSelected(item.id); setAnswer(""); setError(""); setConfirmDelete(false); }}>
              <strong>{item.workflow.name || "Sandbox interview"}</strong>
              <span>{STATUS_LABEL[item.status]} · {new Date(item.updatedAt * 1000).toLocaleDateString()}</span>
              <meter min={0} max={total} value={known} aria-label={`${known} of ${total} essentials known`} />
            </button>;
          })}</div> : <p className="field-hint">Nothing yet. Clarifications you answer on Opportunities also appear here.</p>}</ResourceState>
        </section>
      </div> : !session ? <div className="interview-start">
        <section aria-label="Interview topics">
          <h2>What should Knov learn first?</h2>
          <div className="interview-topics">{TOPICS.map((topic) => <button key={topic.id} type="button" disabled={busy} onClick={() => void run(() => api.startDiscovery("", undefined, topic.id))}>
            <topic.icon size={20} /><strong>{topic.title}</strong><small>{topic.blurb}</small><span>About 5 minutes</span>
          </button>)}</div>
        </section>

        <details className="interview-custom">
          <summary>Or describe something else in your own words</summary>
          <form className="stack-form" onSubmit={start}>
            <label>Describe it in your own words<textarea rows={3} required={contextIndex === ""} value={description} maxLength={4000} onChange={(event) => setDescription(event.target.value)} placeholder="For example: when a client asks for a change, I estimate it, get approval by email, and add it to the next invoice…" /></label>
            <label>Optional work thread<select value={contextIndex} disabled={dashboard.loading} onChange={(event) => setContextIndex(event.target.value)}><option value="">No recorded activity</option>{contexts.map((context, index) => <option key={`${context.subject}-${index}`} value={index}>{context.subject} · {context.signalCount} signals</option>)}</select></label>
            {dashboard.error && <p className="notice error" role="alert">Thread context could not load: {dashboard.error} <button type="button" onClick={() => void dashboard.reload()}>Retry</button></p>}
            {contextIndex !== "" && contexts[Number(contextIndex)] && <details><summary>Review selected metadata before sharing</summary><pre>{JSON.stringify(contexts[Number(contextIndex)], null, 2)}</pre><p>Related intent is a hypothesis until you confirm it.</p></details>}
            <button className="primary-button" disabled={busy || (!description.trim() && contextIndex === "")}>{busy ? <LoaderCircle className="spin" size={16} /> : <Plus size={16} />} Start interview</button>
          </form>
        </details>

        <section aria-label="Saved interviews">
          <h2>Your interviews</h2>
          <ResourceState {...sessions}>{(items) => items.length ? <div className="interview-saved">{items.map((item) => {
            const { known, total } = coverage(item.workflow);
            return <button key={item.id} type="button" disabled={busy} onClick={() => { setSelected(item.id); setAnswer(""); setError(""); setConfirmDelete(false); }}>
              <strong>{item.workflow.name || item.threadContext?.subject || "New interview"}</strong>
              <span>{STATUS_LABEL[item.status]} · {new Date(item.updatedAt * 1000).toLocaleDateString()}</span>
              <meter min={0} max={total} value={known} aria-label={`${known} of ${total} essentials known`} />
            </button>;
          })}</div> : <p className="field-hint">Nothing yet. Pick a topic above; you can pause and come back anytime.</p>}</ResourceState>
        </section>
      </div> : <>
        <header className="interview-heading">
          <button type="button" className="link-button" onClick={close}><ArrowLeft size={14} /> All interviews</button>
          <div><h2>{session.workflow.name || "Your interview"}</h2><span className={`interview-status ${session.status}`}>{STATUS_LABEL[session.status]}</span></div>
          <div className="discovery-actions">
            <button className="ghost-button" disabled={busy} onClick={() => setEditing(session.workflow)}>Review & edit</button>
            {confirmDelete ? <><span className="field-hint">Delete the interview and what it learned?</span><button className="danger-button" disabled={busy} onClick={() => void deleteSession()}>Delete interview</button><button className="ghost-button" disabled={busy} onClick={() => setConfirmDelete(false)}>Keep interview</button></> : <button className="ghost-button" disabled={busy} onClick={() => setConfirmDelete(true)}><Trash2 size={14} /> Delete</button>}
          </div>
        </header>
        <div className="interview-layout">
          <section className="discovery-conversation" aria-label="Interview">
            <div className="discovery-messages" aria-label="Interview messages" aria-live="polite">
              {session.messages.map((message, index) => <article key={`${session.id}-${index}`} className={`discovery-message ${message.role}`}><strong>{message.role === "assistant" ? "Knov" : "You"}</strong><MarkdownMessage>{message.content}</MarkdownMessage></article>)}
              {busy && <p role="status" className="interview-thinking"><LoaderCircle className="spin" size={16} /> Knov is thinking about your answer…</p>}
            </div>
            {session.status === "active" ? <form className="discovery-answer" onSubmit={(event) => { event.preventDefault(); send("answer"); }}>
              {!!session.suggestedAnswers?.length && <div className="interview-suggestions" aria-label="Suggested answers">{session.suggestedAnswers.map((suggestion) => <button key={suggestion} type="button" disabled={busy || !providerReady} onClick={() => send("answer", suggestion)}>{suggestion}</button>)}</div>}
              <label className="interview-answer">Your answer<textarea rows={3} value={answer} disabled={busy} maxLength={6000} onChange={(event) => setAnswer(event.target.value)} onKeyDown={(event: KeyboardEvent<HTMLTextAreaElement>) => { if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing && answer.trim() && providerReady && !busy) { event.preventDefault(); send("answer"); } }} placeholder="Answer in your own words. A real example helps most." /></label>
              <div className="discovery-actions">
                <button className="primary-button" disabled={busy || !providerReady || !answer.trim()}>Send answer</button>
                <small className="field-hint">Enter to send · Shift+Enter for a new line</small>
                <span className="interview-secondary">
                  <button type="button" className="ghost-button" disabled={busy || !providerReady} onClick={() => send("skip")}><SkipForward size={14} /> Skip question</button>
                  <button type="button" className="ghost-button" disabled={busy} onClick={() => void run(() => api.setDiscoveryStatus(session.id, "paused"))}><Pause size={14} /> Pause</button>
                  <button type="button" className="ghost-button" disabled={busy} onClick={() => send("end")}>Finish & review</button>
                </span>
              </div>
            </form> : session.status === "paused" ? <div className="discovery-actions"><p>Progress is saved. Resume when you are ready.</p><button className="primary-button" disabled={busy} onClick={() => void run(() => api.setDiscoveryStatus(session.id, "active"))}><Play size={15} /> Resume interview</button></div>
              : <div className="discovery-actions"><p><Check size={14} /> Interview complete. Check what Knov learned and correct anything that's wrong.</p><button className="primary-button" disabled={busy} onClick={() => setEditing(session.workflow)}>Review workflow</button><button className="ghost-button" onClick={close}>Start another topic</button></div>}
          </section>
          <Learned session={session} />
        </div>
      </>}
    </>}
    {editing && <WorkflowEditor workflow={editing} onClose={() => setEditing(undefined)} onSaved={(updated) => { sessions.setData((items) => items?.map((item) => item.id === updated.sessionId ? { ...item, workflow: updated } : item)); setEditing(undefined); void sessions.reload(); }} />}
  </div>;
}

/** Live summary of the workflow the interview is building, in plain words. */
function Learned({ session }: { session: InterviewSession }) {
  const workflow = session.workflow;
  const { known, total } = coverage(workflow);
  const rules = [...workflow.decisions, ...workflow.approvals];
  const fact = (label: string, value: string | string[]) => {
    const values = (Array.isArray(value) ? value : [value]).filter((item) => item.trim());
    return <div className="learned-fact" key={label}><dt>{label}</dt>{values.length ? values.map((item) => <dd key={item}>{item}</dd>) : <dd className="unknown">Not yet known</dd>}</div>;
  };
  return <aside className="interview-learned" aria-label="What Knov has learned">
    <h2>What Knov has learned</h2>
    <div className="learned-progress"><meter min={0} max={total} value={known} aria-label="Interview progress" /><small>{known} of {total} essentials</small></div>
    <dl>
      {fact("Goal", workflow.businessGoal)}
      {fact("Starts when", workflow.trigger)}
      <div className="learned-fact"><dt>Steps</dt>{workflow.steps.length ? <dd><ol>{workflow.steps.map((step) => <li key={step.id}>{step.name}{step.actor && <small> · {step.actor}</small>}</li>)}</ol></dd> : <dd className="unknown">Not yet known</dd>}</div>
      {fact("Billing and approval rules", rules)}
      {fact("Where money slips through", workflow.bottlenecks)}
      {fact("How often", workflow.frequency)}
    </dl>
    {session.status !== "completed" && session.missingInformation.length > 0 && <details className="learned-gaps"><summary>Still unclear ({session.missingInformation.length})</summary><ul>{session.missingInformation.slice(0, 8).map((item) => <li key={item}>{item}</li>)}</ul></details>}
    <p className="field-hint"><MessageSquare size={12} /> {session.workflow.commercialContext?.simulation ? "Scripted sandbox interview: nothing is sent to an AI provider. Your answers are saved on this Mac with the simulated business." : "Your answers and this summary go to your AI provider only when you answer or skip. Everything is saved on this Mac."}</p>
  </aside>;
}
