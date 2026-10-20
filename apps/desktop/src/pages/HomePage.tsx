import { useState } from "react";
import { ArrowRight, Check, CircleCheck, FileUp, FlaskConical, Mail, RefreshCw } from "lucide-react";
import { EmptyState, errorMessage, PageHeader } from "../components/ui";
import { useResource } from "../hooks/useResource";
import { api, isDesktopRuntime } from "../lib/api";
import { evidenceByline, isOpen, issueCopy, kindLabel } from "../lib/revenueCopy";
import { focusOpportunity, revenueApi } from "../lib/revenueApi";
import { setSimulationMode, useSimulationMode } from "../state/simulationMode";
import type { RevenueEvidence, RevenueOpportunity, RevenueOverview } from "../revenueTypes";
import { revenueMoney } from "./RevenuePage";
import "./home.css";

const totals = (amounts: Record<string, number>) => Object.entries(amounts).map(([currency, cents]) => revenueMoney(cents, currency)).join(" · ");

function byPriority(a: RevenueOpportunity, b: RevenueOpportunity) {
  return (b.amountCents ?? -1) - (a.amountCents ?? -1) || b.createdAt - a.createdAt;
}

function openDetails(id: string) {
  focusOpportunity(id);
  window.location.hash = "/revenue";
}

/**
 * The first thing a user sees: what is wrong in their business right now, why
 * Knov thinks so, and the one thing to do about each problem.
 */
export function HomePage() {
  const simulation = useSimulationMode();
  const resource = useResource(() => revenueApi.overview(simulation), [simulation]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const data = resource.data?.demo === simulation ? resource.data : undefined;

  const run = async (operation: () => Promise<RevenueOverview>, then?: () => void) => {
    setBusy(true); setError("");
    try {
      const result = await operation();
      if (result.demo === simulation) resource.setData(result);
      then?.();
    } catch (cause) { setError(errorMessage(cause)); }
    finally { setBusy(false); }
  };

  const open = (data?.opportunities ?? []).filter(isOpen).sort(byPriority);
  const handled = (data?.opportunities ?? []).filter((item) => !isOpen(item));
  const unknownAmounts = open.filter((item) => item.amountCents === null).length;
  const hasSources = Boolean(data && (data.projects.length || data.evidence.length));

  return <div className="page home-page">
    <PageHeader
      eyebrow={simulation ? `Simulation · ${data?.business?.name ?? "fictional business"}` : "Home"}
      title="What needs your attention"
      description="Knov reads your contracts, email, Slack, and work activity to find revenue you are about to miss, then tells you what to do."
      actions={isDesktopRuntime() && hasSources && <button className="ghost-button" disabled={busy} onClick={() => void run(() => revenueApi.analyze(simulation))}><RefreshCw size={14} /> Check again</button>}
    />
    {!isDesktopRuntime() ? <EmptyState title="Open Knov in the desktop app" detail="Your business data is analyzed on this Mac. Browser preview cannot read contracts, email, or Slack." /> : <>
      {error && <p className="notice error" role="alert">{error}</p>}
      {resource.error && <p className="notice error" role="alert">{resource.error} <button onClick={() => void resource.reload()}>Retry</button></p>}
      {resource.loading && !data && <p role="status">Checking your business data…</p>}
      {data && !hasSources && <GetStarted simulation={simulation} />}
      {data && hasSources && <>
        {!simulation && <SetupChecklist data={data} />}
        <section className="home-summary" aria-label="Summary">
          <article className={open.length ? "attention" : ""}><span>Problems to fix</span><strong>{open.length}</strong></article>
          <article><span>Revenue at stake</span><strong>{totals(data.metrics.potentialByCurrency) || (open.length ? "Not yet known" : "$0")}</strong>{unknownAmounts > 0 && <small>{unknownAmounts} {unknownAmounts === 1 ? "problem has" : "problems have"} no amount yet. Connect billing data to size {unknownAmounts === 1 ? "it" : "them"}.</small>}</article>
          <article><span>Recovered so far</span><strong>{totals(data.metrics.verifiedRecoveredByCurrency) || "$0"}</strong><small>Payments you confirmed after acting</small></article>
        </section>
        {open.length ? <div className="issue-list">{open.map((opportunity) => <IssueCard key={opportunity.id} opportunity={opportunity} data={data} busy={busy}
          onPrepare={() => void run(() => revenueApi.prepare(opportunity.id), () => openDetails(opportunity.id))}
          onDismiss={() => void run(() => revenueApi.review(opportunity.id, "dismiss"))} />)}</div>
          : <EmptyState icon={<CircleCheck size={22} />} title="Nothing needs your attention" detail="Knov found no unbilled work, overdue promises, or missing invoices in the sources you've added. It checks again whenever new data arrives." action={<a className="ghost-button" href="#/sources">Add more sources</a>} />}
        {handled.length > 0 && <details className="home-handled"><summary>Handled or dismissed ({handled.length})</summary>{handled.map((item) => <button key={item.id} className="handled-row" onClick={() => openDetails(item.id)}><span>{issueCopy(item).label}</span><strong>{item.clientName}</strong><small>{item.status === "dismissed" ? "Dismissed" : "Resolved"}</small></button>)}</details>}
      </>}
    </>}
  </div>;
}

function IssueCard({ opportunity, data, busy, onPrepare, onDismiss }: { opportunity: RevenueOpportunity; data: RevenueOverview; busy: boolean; onPrepare: () => void; onDismiss: () => void }) {
  const copy = issueCopy(opportunity);
  const draft = data.drafts.find((item) => item.opportunityId === opportunity.id);
  const evidence = opportunity.evidenceIds
    .map((id) => data.evidence.find((item) => item.id === id))
    .filter((item): item is RevenueEvidence => Boolean(item))
    .sort((a, b) => Number(a.kind === "agreement") - Number(b.kind === "agreement") || a.occurredAt - b.occurredAt);
  return <article className="issue-card">
    <header>
      <span className="issue-badge">{copy.label}</span>
      <span className="issue-client">{opportunity.clientName} · {opportunity.projectName}</span>
      <strong className="issue-amount">{opportunity.amountCents === null ? "Amount unknown" : revenueMoney(opportunity.amountCents, opportunity.currency)}</strong>
    </header>
    <div className="issue-body">
      <section><h3>What's wrong</h3><p>{copy.problem}</p></section>
      <section><h3>Why Knov thinks so</h3><ul className="issue-evidence">{evidence.slice(0, 4).map((item) => <li key={item.id}>
        <span>{kindLabel(item.kind)}</span>
        <q>{item.kind === "agreement" ? item.excerpt.split("\n").filter(Boolean).join(" · ").slice(0, 180) : item.excerpt}</q>
        <small>{evidenceByline(item)} · {new Date(item.occurredAt * 1000).toLocaleDateString()}</small>
      </li>)}</ul></section>
      <section className="issue-recommendation"><h3>Recommended</h3><p>{copy.recommendation}</p></section>
    </div>
    <footer>
      {draft ? <button className="primary-button" disabled={busy} onClick={() => openDetails(opportunity.id)}>Review draft <ArrowRight size={14} /></button>
        : <button className="primary-button" disabled={busy} onClick={onPrepare}>{copy.actionLabel} <ArrowRight size={14} /></button>}
      <button className="ghost-button" disabled={busy} onClick={() => openDetails(opportunity.id)}>See all evidence</button>
      <button className="ghost-button" disabled={busy} onClick={onDismiss}>Not a problem</button>
      <small>Drafts are never sent automatically.</small>
    </footer>
  </article>;
}

const CHECKLIST_HIDDEN = "knov.setup-checklist-hidden";

/** What is left to set up, each step one click from where it happens. */
function SetupChecklist({ data }: { data: RevenueOverview }) {
  const interviews = useResource(() => api.discoverySessions(), []);
  const [hidden, setHidden] = useState(() => { try { return localStorage.getItem(CHECKLIST_HIDDEN) === "true"; } catch { return false; } });
  const steps = [
    { label: "Add a client and upload a contract", done: data.agreements.length > 0, href: "#/sources" },
    { label: "Connect Gmail or Slack", done: data.evidence.some((item) => item.source === "gmail" || item.source === "slack"), href: "#/sources" },
    { label: "Teach Knov how you bill in the AI interview", done: Array.isArray(interviews.data) && interviews.data.length > 0, href: "#/discovery" },
    { label: "Act on your first problem", done: data.drafts.length > 0 || data.metrics.reviewed > 0, href: "#/revenue" },
  ];
  const done = steps.filter((step) => step.done).length;
  if (hidden || done === steps.length) return null;
  return <section className="setup-checklist" aria-label="Finish setting up">
    <header><strong>Finish setting up</strong><small>{done} of {steps.length} done. Each step helps Knov find more.</small><button type="button" className="link-button" onClick={() => { setHidden(true); try { localStorage.setItem(CHECKLIST_HIDDEN, "true"); } catch { /* hidden for this visit */ } }}>Hide</button></header>
    <ol>{steps.map((step) => <li key={step.label} className={step.done ? "done" : ""}>
      <span aria-hidden="true">{step.done ? <Check size={13} /> : null}</span>
      {step.done ? <s>{step.label}</s> : <a href={step.href}>{step.label} <ArrowRight size={13} /></a>}
    </li>)}</ol>
  </section>;
}

function GetStarted({ simulation }: { simulation: boolean }) {
  if (simulation) {
    return <EmptyState icon={<FlaskConical size={22} />} title="The simulated business has no data yet" detail="Connect its contracts, inbox, Slack, and billing in the Simulation tab to watch Knov find problems." action={<a className="primary-button" href="#/simulation">Open the simulation</a>} />;
  }
  return <section className="get-started" aria-label="Get started">
    <h2>Give Knov something to read</h2>
    <p>Knov compares what your clients agreed to pay for with what they asked for, what your team delivered, and what you invoiced. Start with any one of these.</p>
    <div className="get-started-grid">
      <a href="#/sources"><FileUp size={20} /><strong>Upload a contract</strong><small>A statement of work, proposal, or agreement (PDF, Word, or text).</small></a>
      <a href="#/sources"><Mail size={20} /><strong>Connect Gmail or Slack</strong><small>Knov looks for client requests, approvals, and promises you made.</small></a>
      <button type="button" onClick={() => { setSimulationMode(true); window.location.hash = "/simulation"; }}><FlaskConical size={20} /><strong>Try a simulated business</strong><small>No real data needed. See exactly what Knov finds and recommends.</small></button>
    </div>
  </section>;
}
