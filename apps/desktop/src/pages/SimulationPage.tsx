import { useRef, useState } from "react";
import { ArrowRight, Check, ClipboardList, FileText, Hash, LoaderCircle, Mail, MonitorSmartphone, Receipt, RotateCcw } from "lucide-react";
import { EmptyState, errorMessage, PageHeader, Segmented } from "../components/ui";
import { useResource } from "../hooks/useResource";
import { isDesktopRuntime } from "../lib/api";
import { isOpen, issueCopy, kindLabel } from "../lib/revenueCopy";
import { revenueApi, SimulationStep } from "../lib/revenueApi";
import { setSimulationMode, useSimulationMode } from "../state/simulationMode";
import type { RevenueEvidence, RevenueOverview, SimulationBusiness } from "../revenueTypes";
import { revenueMoney } from "./RevenuePage";
import "./home.css";

const STEPS: { step: SimulationStep; source?: string; title: string; detail: (business: SimulationBusiness) => string; icon: typeof Mail }[] = [
  { step: "contracts", title: "Contracts", detail: (b) => `${b.clientCount} clients and what each one agreed to pay for.`, icon: FileText },
  { step: "gmail", source: "demo_gmail", title: "Gmail", detail: () => "Client emails: sign-offs, approvals, requests, and promises the owner made.", icon: Mail },
  { step: "slack", source: "demo_slack", title: "Slack", detail: () => "Shared client channels where requests arrive.", icon: Hash },
  { step: "work", source: "demo_thread", title: "Work activity", detail: () => "What Knov saw the team working on, from Mac activity.", icon: MonitorSmartphone },
  { step: "billing", source: "demo_billing", title: "Billing", detail: (b) => `Invoices from a simulated ${b.tools.find((tool) => /quickbooks|xero|freshbooks|wave|bill\.com/i.test(tool)) ?? "accounting"} account.`, icon: Receipt },
];

type Tab = "inbox" | "slack" | "contracts" | "work" | "billing";

/** Describes how one step changed what Knov found, so cause and effect are visible. */
function describeChanges(before: RevenueOverview | undefined, after: RevenueOverview): string[] {
  const previous = new Map((before?.opportunities ?? []).map((item) => [item.id, item]));
  const changes: string[] = [];
  for (const item of after.opportunities) {
    const old = previous.get(item.id);
    const copy = issueCopy(item);
    if (!old && isOpen(item)) changes.push(`New problem: ${copy.label} at ${item.clientName}`);
    else if (!old) changes.push(`${item.clientName}: checked and found no problem`);
    else if (isOpen(old) && !isOpen(item)) changes.push(`Resolved: ${item.clientName}. New evidence shows it is already handled`);
    else if (old.type !== item.type || old.amountCents !== item.amountCents) changes.push(`Sharper: ${item.clientName} is now "${copy.label}"${item.amountCents !== null ? ` worth ${revenueMoney(item.amountCents, item.currency)}` : ""}`);
  }
  return changes.length ? changes : ["No change in findings. This source added context but no new evidence of a problem."];
}

const INTERVIEW_STATUS: Record<string, string> = { active: "In progress", paused: "Paused", completed: "Complete" };

export function SimulationPage() {
  const simulation = useSimulationMode();
  const resource = useResource(() => revenueApi.overview(true), []);
  const catalog = useResource(() => revenueApi.simulationCatalog(), []);
  const [busy, setBusy] = useState<SimulationStep | "all" | "reset" | "select" | "interview">();
  const [error, setError] = useState("");
  const [changes, setChanges] = useState<{ title: string; lines: string[] }>();
  const [tab, setTab] = useState<Tab>("inbox");
  const [switching, setSwitching] = useState<string>();
  // The business whose switch confirm was cancelled, so focus returns to its Choose button.
  const refocusChoose = useRef<string>(undefined);
  const cancelSwitch = () => { refocusChoose.current = switching; setSwitching(undefined); };
  const data = resource.data;
  const businesses = catalog.data?.businesses ?? [];
  const business = businesses.find((item) => item.id === catalog.data?.active);
  const loaded = (step: (typeof STEPS)[number]) => Boolean(data && (step.source ? data.evidence.some((item) => item.source === step.source) : data.projects.length));
  const open = (data?.opportunities ?? []).filter(isOpen);
  const hasData = Boolean(data?.projects.length || catalog.data?.interview);

  const run = async (key: NonNullable<typeof busy>, title: string, operation: () => Promise<RevenueOverview>) => {
    setBusy(key); setError("");
    try {
      const before = data;
      const after = await operation();
      resource.setData(after);
      setChanges(key === "reset" || key === "select" ? undefined : { title, lines: describeChanges(before, after) });
      setSimulationMode(key !== "reset" || simulation);
      if (key === "reset" || key === "select") void catalog.reload();
    } catch (cause) { setError(errorMessage(cause)); }
    finally { setBusy(undefined); }
  };

  const choose = (id: string) => {
    setSwitching(undefined);
    void run("select", "", () => revenueApi.simulationSelect(id));
  };

  const startInterview = async () => {
    setBusy("interview"); setError("");
    try {
      await revenueApi.simulationInterview();
      setSimulationMode(true);
      window.location.hash = "/discovery";
    } catch (cause) { setError(errorMessage(cause)); }
    finally { setBusy(undefined); }
  };

  if (!isDesktopRuntime()) {
    return <div className="page"><PageHeader eyebrow="Sandbox" title="Simulation" description="Run Knov against a fictional business." /><EmptyState title="Open the simulation in the desktop app" detail="The simulated business is stored in the local database, separately from your actual data." /></div>;
  }

  return <div className="page simulation-page">
    <PageHeader
      eyebrow="Sandbox · fictional data"
      title={business ? `Simulation: ${business.name}` : "Simulation"}
      description={business
        ? `${business.profile} Connect its sources one at a time and watch what Knov finds. Nothing here touches your actual data, and nothing leaves this Mac.`
        : "Ten made-up businesses, each a kind of company Knov is built for. Pick one, connect its sources one at a time, and watch what Knov finds. Nothing here touches your actual data, and nothing leaves this Mac."}
      actions={<>
        <button className="ghost-button" disabled={Boolean(busy) || !hasData} onClick={() => void run("reset", "", revenueApi.simulationReset)}><RotateCcw size={14} /> Reset</button>
        <a className="primary-button" href="#/home" onClick={() => setSimulationMode(true)}>See findings on Home <ArrowRight size={14} /></a>
      </>}
    />
    {!simulation && data?.projects.length ? <p className="notice info">Home currently shows your actual data. <button className="link-button" onClick={() => setSimulationMode(true)}>Show the simulation on Home instead</button></p> : null}
    {error && <p className="notice error" role="alert">{error}</p>}
    {resource.error && <p className="notice error" role="alert">{resource.error} <button onClick={() => void resource.reload()}>Retry</button></p>}
    {catalog.error && <p className="notice error" role="alert">{catalog.error} <button onClick={() => void catalog.reload()}>Retry</button></p>}

    <section className="simulation-businesses" aria-label="Simulated businesses">
      <h2>{business ? "Business" : "1. Choose a business"}</h2>
      <p className="field-hint">Each one is a small service business that sells project work and could lose money the way Knov looks for: unbilled extras, late promises, and milestones nobody invoiced.</p>
      {catalog.loading && !catalog.data && <p role="status">Loading…</p>}
      <div className="business-grid">{businesses.map((item) => {
        const active = item.id === business?.id;
        const confirm = switching === item.id;
        return <article key={item.id} className={active ? "active" : ""} aria-current={active ? "true" : undefined}>
          <strong>{item.name}</strong>
          <small>{item.industry} · {item.location}</small>
          <small>{item.team} · {item.clientCount} clients · {item.currency}</small>
          {active ? <span className="step-state">Loaded</span>
            : confirm ? <div className="business-confirm" onKeyDown={(event) => { if (event.key === "Escape") cancelSwitch(); }}><small>Switching clears {business?.name}'s sandbox findings, drafts, and interview.</small><button className="primary-button" disabled={Boolean(busy)} onClick={() => choose(item.id)}>Switch</button><button className="ghost-button" autoFocus disabled={Boolean(busy)} onClick={cancelSwitch}>Keep</button></div>
            : <button className="ghost-button" ref={(node) => { if (node && refocusChoose.current === item.id) { refocusChoose.current = undefined; node.focus(); } }} disabled={Boolean(busy)} onClick={() => (hasData ? setSwitching(item.id) : choose(item.id))}>{busy === "select" ? <LoaderCircle size={14} className="spin" /> : null}Choose</button>}
        </article>;
      })}</div>
    </section>

    {business && <div className="simulation-layout">
      <section className="simulation-steps" aria-label="Simulated sources">
        <h2>2. Connect sources</h2>
        <ol>{STEPS.map((step, index) => {
          const done = loaded(step);
          const Icon = step.icon;
          const blocked = index > 0 && !data?.projects.length;
          return <li key={step.step} className={done ? "done" : ""}>
            <span className="step-icon">{done ? <Check size={16} /> : <Icon size={16} />}</span>
            <div><strong>{step.title}</strong><small>{step.detail(business)}</small></div>
            {done ? <span className="step-state">Connected</span>
              : <button className="ghost-button" disabled={Boolean(busy) || blocked} title={blocked ? "Load the contracts first" : undefined} onClick={() => void run(step.step, `After connecting ${step.title}`, () => revenueApi.simulationLoad(step.step))}>{busy === step.step && <LoaderCircle size={14} className="spin" />}{index === 0 ? "Load" : "Connect"}</button>}
          </li>;
        })}
          <li className={catalog.data?.interview?.status === "completed" ? "done" : ""}>
            <span className="step-icon">{catalog.data?.interview?.status === "completed" ? <Check size={16} /> : <ClipboardList size={16} />}</span>
            <div><strong>Interview</strong><small>Knov interviews {business.owner.name} about {business.interviewName.toLowerCase()}. Scripted here, so no AI key is needed; your answers are saved to the sandbox.</small></div>
            {catalog.data?.interview
              ? <a className="ghost-button" href="#/discovery" onClick={() => setSimulationMode(true)}>{INTERVIEW_STATUS[catalog.data.interview.status] ?? "Open"} <ArrowRight size={14} /></a>
              : <button className="ghost-button" disabled={Boolean(busy)} onClick={() => void startInterview()}>{busy === "interview" && <LoaderCircle size={14} className="spin" />}Start</button>}
          </li>
        </ol>
        {!STEPS.every(loaded) && <button className="ghost-button full" disabled={Boolean(busy)} onClick={() => void run("all", "After connecting everything", () => revenueApi.simulationLoad("all"))}>{busy === "all" && <LoaderCircle size={14} className="spin" />}Connect everything at once</button>}
        <p className="field-hint">In the simulation, each message comes already labeled (request, approval, promise). With your actual Gmail or Slack, you confirm the label for each imported message.</p>
      </section>

      <section className="simulation-findings" aria-label="What Knov found" aria-live="polite">
        <h2>3. What Knov found</h2>
        {changes && <div className="simulation-changes"><strong>{changes.title}</strong><ul>{changes.lines.map((line) => <li key={line}>{line}</li>)}</ul></div>}
        {!data?.projects.length ? <p className="field-hint">Load the contracts to begin. Findings appear here as each source connects.</p>
          : open.length ? <ul className="simulation-issues">{open.map((item) => <li key={item.id}><span className="issue-badge">{issueCopy(item).label}</span><strong>{item.clientName}</strong><small>{item.amountCents === null ? "Amount unknown" : revenueMoney(item.amountCents, item.currency)}</small></li>)}</ul>
          : <p className="field-hint">No problems yet. Contracts alone don't show what was requested, delivered, or invoiced. Connect more sources.</p>}
        {open.length > 0 && <a className="primary-button" href="#/home" onClick={() => setSimulationMode(true)}>See what to do about {open.length === 1 ? "it" : "them"} <ArrowRight size={14} /></a>}
        <details className="simulation-expectations">
          <summary>What to look for once everything is connected</summary>
          <ul>{business.whatToExpect.map((line) => <li key={line}>{line}</li>)}</ul>
        </details>
      </section>
    </div>}

    {business && data && data.projects.length > 0 && <section className="simulation-browser" aria-label="Simulated source data">
      <div className="simulation-browser-heading"><h2>4. Look inside the simulated sources</h2><Segmented label="Source" value={tab} onChange={setTab} options={[
        { value: "inbox", label: "Inbox", count: data.evidence.filter((item) => item.source === "demo_gmail").length },
        { value: "slack", label: "Slack", count: data.evidence.filter((item) => item.source === "demo_slack").length },
        { value: "contracts", label: "Contracts", count: data.agreements.length },
        { value: "work", label: "Work activity", count: data.evidence.filter((item) => item.source === "demo_thread").length },
        { value: "billing", label: "Billing", count: data.evidence.filter((item) => item.source === "demo_billing").length },
      ]} /></div>
      <SourceView tab={tab} data={data} />
    </section>}
  </div>;
}

function Label({ evidence }: { evidence: RevenueEvidence }) {
  return evidence.kind === "communication"
    ? <span className="knov-label quiet">Ordinary message · not evidence</span>
    : <span className="knov-label">Knov reads this as: {kindLabel(evidence.kind)}</span>;
}

function SourceView({ tab, data }: { tab: Tab; data: RevenueOverview }) {
  const source = { inbox: "demo_gmail", slack: "demo_slack", work: "demo_thread", billing: "demo_billing", contracts: "" }[tab];
  const client = (projectId: string) => data.clients.find((item) => item.id === data.projects.find((project) => project.id === projectId)?.clientId)?.name ?? "";
  const when = (seconds: number) => new Date(seconds * 1000).toLocaleDateString();
  if (tab === "contracts") {
    return <div className="source-records">{data.agreements.map((agreement) => <article key={agreement.id}><header><strong>{agreement.title}</strong><small>{client(agreement.projectId)}</small></header><pre>{agreement.text}</pre><span className="knov-label">Knov reads this as: Contract (what was agreed)</span></article>)}</div>;
  }
  const records = data.evidence.filter((item) => item.source === source).sort((a, b) => b.occurredAt - a.occurredAt);
  if (!records.length) return <p className="field-hint">Not connected yet. Connect this source in step 2.</p>;
  return <div className="source-records">{records.map((item) => {
    const meta = item.meta ?? {};
    return <article key={item.id}>
      {tab === "inbox" && <header><strong>{meta.subject}</strong><small>{meta.from} → {meta.to} · {when(item.occurredAt)}</small></header>}
      {tab === "slack" && <header><strong>{meta.channel}</strong><small>{meta.author} · {when(item.occurredAt)}</small></header>}
      {tab === "work" && <header><strong>{meta.thread}</strong><small>{meta.apps} · {meta.hours} hours · {when(item.occurredAt)}</small></header>}
      {tab === "billing" && <header><strong>{meta.number ?? "Billing sync"} · {meta.customer}</strong><small>{meta.status}{item.amountCents ? ` · ${revenueMoney(item.amountCents, item.currency ?? "USD")}` : ""} · {when(item.occurredAt)}</small></header>}
      <p>{item.excerpt}</p>
      <Label evidence={item} />
    </article>;
  })}</div>;
}
