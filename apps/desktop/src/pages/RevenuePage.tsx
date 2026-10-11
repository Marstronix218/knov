import { useEffect, useState } from "react";
import { ArrowLeft } from "lucide-react";
import { EmptyState, errorMessage, PageHeader } from "../components/ui";
import { useResource } from "../hooks/useResource";
import { isDesktopRuntime } from "../lib/api";
import { issueCopy } from "../lib/revenueCopy";
import { clearFocusedOpportunity, pendingFocusedOpportunity, revenueApi } from "../lib/revenueApi";
import { setSimulationMode, useSimulationMode } from "../state/simulationMode";
import type { RevenueOverview } from "../revenueTypes";
import "./revenue.css";

const label = (value: string) => value.replace(/_/g, " ");
export function revenueMoney(cents: number | null, currency: string | null): string {
  if (cents === null || !currency) return "Unknown";
  try { return new Intl.NumberFormat(undefined, { style: "currency", currency }).format(cents / 100); }
  catch { return `${(cents / 100).toFixed(2)} ${currency}`; }
}
const totals = (amounts: Record<string, number>, empty: string) => Object.keys(amounts).length ? Object.entries(amounts).map(([currency, cents]) => revenueMoney(cents, currency)).join(" · ") : empty;

/** Every opportunity with its full evidence, review, draft, and outcome history. */
export function RevenuePage() {
  const demo = useSimulationMode();
  const resource = useResource(() => revenueApi.overview(demo), [demo]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState(pendingFocusedOpportunity);
  const [fromHome] = useState(() => Boolean(pendingFocusedOpportunity()));
  useEffect(clearFocusedOpportunity, []);
  const [clientFilter, setClientFilter] = useState("");
  const [typeFilter, setTypeFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const [confidenceFilter, setConfidenceFilter] = useState("");
  const [dateFilter, setDateFilter] = useState("");
  const [answer, setAnswer] = useState("");
  const [draftBody, setDraftBody] = useState("");
  const [clarifying, setClarifying] = useState(false);
  const [correcting, setCorrecting] = useState(false);
  const data = resource.data?.demo === demo ? resource.data : undefined;
  const opportunity = data?.opportunities.find((item) => item.id === selected);
  const draft = data?.drafts.find((item) => item.opportunityId === selected);
  useEffect(() => { setDraftBody(draft?.body ?? ""); }, [draft?.id, draft?.body]);
  const [scope, setScope] = useState(demo);
  if (scope !== demo) { setScope(demo); setSelected(""); setAnswer(""); setClarifying(false); setCorrecting(false); }
  const run = async (operation: () => Promise<RevenueOverview>) => {
    setBusy(true); setError("");
    try {
      const result = await operation();
      if (result.demo !== demo) throw new Error("Revenue response belongs to a different data scope. Reload this view.");
      resource.setData(result);
    }
    catch (cause) { setError(errorMessage(cause)); }
    finally { setBusy(false); }
  };
  const items = (data?.opportunities ?? []).filter((item) => (!clientFilter || item.clientName === clientFilter) && (!typeFilter || item.type === typeFilter) && (!statusFilter || item.status === statusFilter) && (!confidenceFilter || item.confidence >= Number(confidenceFilter)) && (!dateFilter || item.createdAt >= Date.parse(dateFilter) / 1000));
  // Arriving from Home: bring the chosen problem's details into view once loaded.
  useEffect(() => {
    if (fromHome && opportunity) document.querySelector(".revenue-detail")?.scrollIntoView?.({ behavior: "smooth", block: "start" });
  }, [fromHome, opportunity?.id]);
  return <div className="page revenue-page">
    {fromHome && <a className="back-link" href="#/home"><ArrowLeft size={14} /> Back to Home</a>}
    <PageHeader eyebrow={demo ? "Simulation · Brightline Studio" : "Evidence → review → outcome"} title="Opportunities" description="Every problem Knov found, with the exact evidence, what would disprove it, and your drafts and outcomes." actions={isDesktopRuntime() && <button className="primary-button" disabled={busy || !data} onClick={() => void run(() => revenueApi.analyze(demo))}>Check sources again</button>} />
    {!isDesktopRuntime() ? <EmptyState title="Open Revenue in the desktop app" detail="Commercial records and the simulation use the local database. Browser preview cannot connect sources or save revenue decisions." /> : <>
      <p className={`notice ${demo ? "info" : "ok"}`}>{demo ? <>Simulation · Fictional clients and fixed timestamps, kept separate from your actual data. <button className="link-button" onClick={() => setSimulationMode(false)}>Show my actual data</button></> : "Actual local data · Analysis and drafts stay on this Mac. Approval records review only; it does not send messages or issue invoices."}</p>
      {error && <p className="notice error" role="alert">{error}</p>}
      {resource.error && <p className="notice error" role="alert">{resource.error} <button onClick={() => void resource.reload()}>Retry</button></p>}
      {resource.loading && !data && <p role="status">Loading local revenue records…</p>}
      {data && <>
        <section className="revenue-metrics" aria-label="Revenue overview">
          {[["Open opportunities", data.metrics.open], ["Need clarification", data.metrics.needsClarification], ["Drafts awaiting review", data.metrics.draftsAwaitingReview], ["Resolved opportunities", data.metrics.resolved], ["Evidence-supported potential", totals(data.metrics.potentialByCurrency, "Unknown")], ["Recovery with user-checked receipts", totals(data.metrics.verifiedRecoveredByCurrency, "No verified payments")]].map(([name, value]) => <article key={name}><span>{name}</span><strong>{value}</strong></article>)}
        </section>
        {!!data.skillSuggestions?.length && <section className="revenue-panel" aria-label="Commercial Skill suggestions"><h2>Suggested commercial procedures</h2>{data.skillSuggestions.map((suggestion) => <article key={suggestion.projectId}><h3>{suggestion.title}</h3><p>{suggestion.description}</p><p>{suggestion.confirmedOpportunityCount} distinct confirmations in this project. Review the procedure in the existing Workflows and Skill tools; no executable Skill has been created.</p>{!demo && <a className="ghost-button" href="#/workflows">Review in Workflows (Labs)</a>}</article>)}</section>}
        <details className="revenue-panel"><summary>Operational metrics and attribution</summary><p>Reviewed: {data.metrics.reviewed} · Confirmed: {data.metrics.confirmed} · Dismissed: {data.metrics.dismissed} · Drafts approved: {data.metrics.draftsApproved} · Actions recorded: {data.metrics.actionsRecorded} · Payments confirmed: {data.metrics.paymentsConfirmed}</p><p>Recovery is based on user-attested external receipts, not independent connector verification. Potential value, approval, and invoice issuance are separate from payments. A subsequent payment does not prove Knov caused it.</p></details>
        <div className="revenue-layout"><section className="revenue-panel" aria-label="Opportunity inbox"><h2>Opportunity inbox</h2><div className="revenue-filters">
          <label>Client<select value={clientFilter} onChange={(e) => setClientFilter(e.target.value)}><option value="">All clients</option>{[...new Set(data.opportunities.map((item) => item.clientName))].map((name) => <option key={name}>{name}</option>)}</select></label>
          <label>Type<select value={typeFilter} onChange={(e) => setTypeFilter(e.target.value)}><option value="">All types</option>{[...new Set(data.opportunities.map((item) => item.type))].map((type) => <option key={type} value={type}>{label(type)}</option>)}</select></label>
          <label>Status<select value={statusFilter} onChange={(e) => setStatusFilter(e.target.value)}><option value="">All statuses</option>{[...new Set(data.opportunities.map((item) => item.status))].map((status) => <option key={status} value={status}>{label(status)}</option>)}</select></label>
          <label>Confidence<select value={confidenceFilter} onChange={(e) => setConfidenceFilter(e.target.value)}><option value="">Any confidence</option><option value="0.8">At least 80%</option><option value="0.5">At least 50%</option></select></label><label>Detected on or after<input type="date" value={dateFilter} onChange={(e) => setDateFilter(e.target.value)} /></label>
        </div>{items.length ? items.map((item) => <button key={item.id} className="revenue-item" aria-pressed={selected === item.id} disabled={busy} onClick={() => { setSelected(item.id); setAnswer(""); setClarifying(false); setCorrecting(false); }}><small>{issueCopy(item).label} · {item.clientName} · {item.projectName}</small><strong>{item.title}</strong><span>{label(item.status)} · {Math.round(item.confidence * 100)}% confidence · {revenueMoney(item.amountCents, item.currency)}</span><span>{item.recommendedAction}</span></button>) : <EmptyState title="No opportunities in this view" detail={demo ? "Connect the simulated sources in the Simulation tab, or adjust your filters." : "Add a client and upload a contract in Sources, then connect email or Slack. Missing billing records will be shown as uncertainty."} />}</section>
        <section className="revenue-panel revenue-detail" aria-label="Opportunity detail">{!opportunity ? <EmptyState title="Select an opportunity" detail="Inspect exact evidence and uncertainty before deciding what to do." /> : <>
          <span className="eyebrow">{label(opportunity.provenance)} · {label(opportunity.type)}</span><h2>{opportunity.title}</h2><p>{opportunity.explanation}</p><h3>Supporting evidence</h3>{opportunity.evidenceIds.map((id) => { const evidence = data.evidence.find((item) => item.id === id); return evidence ? <article className="revenue-evidence" key={id}><strong>{label(evidence.kind)} · {label(evidence.provenance)}</strong><blockquote>{evidence.excerpt}</blockquote><small>{evidence.source} · {evidence.sourceRef} · {new Date(evidence.occurredAt * 1000).toLocaleString()}</small></article> : <p key={id}>Evidence {id} is unavailable.</p>; })}
          <h3>Uncertainty</h3><ul>{opportunity.uncertainty.map((text) => <li key={text}>{text}</li>)}</ul><h3>Evidence that would disprove this</h3><ul>{opportunity.disproves.map((text) => <li key={text}>{text}</li>)}</ul><h3>Business impact</h3><p>Evidence-supported amount: <strong>{revenueMoney(opportunity.amountCents, opportunity.currency)}</strong>. This is potential value, pending commercial review.</p><h3>Recommended action</h3><p>{opportunity.recommendedAction}</p>
          <div className="revenue-actions"><button disabled={busy} className="ghost-button" onClick={() => void run(() => revenueApi.review(opportunity.id, "confirm"))}>Confirm</button><button disabled={busy} className="ghost-button" onClick={() => { setCorrecting(!correcting); setClarifying(false); }}>Correct</button><button disabled={busy} className="ghost-button" onClick={() => void run(() => revenueApi.review(opportunity.id, "dismiss"))}>Dismiss</button><button disabled={busy} className="ghost-button" onClick={() => { setClarifying(!clarifying); setCorrecting(false); }}>Ask Knov</button><button disabled={busy} className="primary-button" onClick={() => void run(() => revenueApi.prepare(opportunity.id))}>Prepare action</button></div>
          {(clarifying || correcting) && <form className="stack-form" onSubmit={(e) => { e.preventDefault(); void run(() => revenueApi.review(opportunity.id, correcting ? "correct" : "clarify", answer.trim())); }}><h3>{correcting ? "Correct this opportunity" : "Contextual clarification"}</h3><p>{opportunity.clarificationQuestion ?? "What evidence changes your understanding of this opportunity?"}</p>{!correcting && opportunity.type === "scope_change" && <label>Scope decision<select value={answer} onChange={(e) => setAnswer(e.target.value)}><option value="">Choose or explain below</option><option value="already_included">Already included</option><option value="separately_billable">Separately billable</option><option value="free_addition">Approved as a free addition</option><option value="not_sure">Not sure</option></select></label>}<label>Your answer<textarea value={answer} onChange={(e) => setAnswer(e.target.value)} maxLength={6000} rows={3} /></label><p className="field-hint">Saved using Workflow Discovery. Your statement is user-confirmed context, not independent financial verification.</p><button className="primary-button" disabled={busy || !answer.trim()}>Save clarification</button>{!demo && opportunity.interviewId && <a href="#/discovery">Open saved Workflow Discovery interview</a>}</form>}
          {draft && <section className="stack-form"><h3>{label(draft.kind)} draft · {label(draft.status)}</h3><label>Edit draft<textarea rows={12} value={draftBody} onChange={(e) => setDraftBody(e.target.value)} maxLength={30000} /></label><div className="revenue-actions"><button className="ghost-button" disabled={busy || !draftBody.trim()} onClick={() => void run(() => revenueApi.saveDraft(draft.id, draftBody))}>Save draft</button><button className="primary-button" disabled={busy || draft.status === "approved" || draft.body !== draftBody} onClick={() => void run(() => revenueApi.approveDraft(draft.id))}>Approve draft</button></div><p className="field-hint">Save edits before approval. Approval never sends the draft or marks an external action complete.</p></section>}
          <OutcomeEntry opportunityId={opportunity.id} busy={busy} run={run} />
          <h3>Outcome history</h3><p>Detected {new Date(opportunity.createdAt * 1000).toLocaleString()} · Current review: {label(opportunity.status)}</p>{draft && <p>Action prepared {new Date(draft.createdAt * 1000).toLocaleString()} · {draft.status === "approved" ? "Draft approved for human use; no external action taken" : "Awaiting draft review"}</p>}{data.outcomes.filter((item) => item.opportunityId === opportunity.id).map((item) => <article className="revenue-evidence" key={item.id}><strong>{label(item.kind)} · {item.verified ? "User-checked external evidence" : "Self-reported"}</strong><p>{item.evidence}</p><small>{revenueMoney(item.amountCents, item.currency)} · {new Date(item.createdAt * 1000).toLocaleString()}</small></article>)}
          <h3>Commercial workflow</h3><ol className="revenue-process" aria-label="Commercial workflow stages">{workflowStages(opportunity.evidenceIds.map((id) => data.evidence.find((item) => item.id === id)?.kind ?? ""), opportunity.status, draft?.status, data.outcomes.filter((item) => item.opportunityId === opportunity.id)).map(([stage, status]) => <li key={stage}><strong>{stage}</strong><span>{status}</span></li>)}</ol>
        </>}</section></div>
      </>}
    </>}
  </div>;
}
type Run = (operation: () => Promise<RevenueOverview>) => Promise<void>;
function OutcomeEntry({ opportunityId, busy, run }: { opportunityId: string; busy: boolean; run: Run }) {
  const [kind, setKind] = useState("actioned");
  const [amount, setAmount] = useState("");
  const [verified, setVerified] = useState(false);
  const canVerify = kind === "paid" && amount.trim() !== "" && Number(amount) > 0;
  return <form className="stack-form" onSubmit={(event) => { event.preventDefault(); const fields = new FormData(event.currentTarget); const amount = String(fields.get("amount")).trim(); void run(() => revenueApi.recordOutcome(opportunityId, String(fields.get("kind")), String(fields.get("evidence")), amount ? Math.round(Number(amount) * 100) : null, amount ? String(fields.get("currency")) : null, canVerify && verified)); }}><h3>Record what happened afterward</h3><label>Outcome<select name="kind" value={kind} onChange={(e) => { setKind(e.target.value); setVerified(false); }}><option value="actioned">Action actually completed</option><option value="invoiced">Invoice issued</option><option value="paid">Payment received</option><option value="not_paid">Not paid</option><option value="unknown">Unknown</option></select></label><label>Evidence and source reference<textarea name="evidence" required maxLength={6000} rows={3} placeholder="For example: receipt ID, payment date, and verified record location" /></label><div className="revenue-fields"><label>Amount (optional; currency units)<input name="amount" type="number" min="0" step="0.01" value={amount} onChange={(e) => { setAmount(e.target.value); setVerified(false); }} /></label><label>Currency<input name="currency" defaultValue="USD" pattern="[A-Z]{3}" maxLength={3} /></label></div><label className="revenue-check"><input type="checkbox" name="verified" disabled={!canVerify} checked={verified && canVerify} onChange={(e) => setVerified(e.target.checked)} /> I checked an external payment receipt and attest this recovery amount</label><p className="field-hint">Receipt verification is your attestation and is available only for a payment with an explicit amount. Knov does not independently validate the receipt. Invoicing does not establish payment or attribution.</p><button className="primary-button" disabled={busy}>Record outcome</button></form>;
}
function workflowStages(kinds: string[], status: string, draftStatus: string | undefined, outcomes: {kind: string; verified: boolean}[]): [string, string][] {
 const outcome = (kind: string) => { const match = outcomes.find((item) => item.kind === kind); return match ? match.verified ? "Recorded · user-checked external evidence" : "Recorded · self-reported, unverified" : "Missing from available records"; };
 return [["Client request", kinds.includes("request") || kinds.includes("commitment") ? "Source evidence available" : "Missing from available records"], ["Work planned", "Unverified · no linked planning record"], ["Work performed", kinds.includes("activity") || kinds.includes("milestone_accepted") ? "Source evidence available · inspect excerpt" : "Missing from available records"], ["Commercial review", draftStatus === "approved" ? "Internal draft reviewed · no external action" : status === "confirmed" ? "User confirmed opportunity" : "Review needed"], ["Client approval", kinds.includes("approval") ? "Approval evidence available" : "Missing · may exist outside connected sources"], ["Invoice", outcome("invoiced")], ["Payment", outcome("paid")]];
}
