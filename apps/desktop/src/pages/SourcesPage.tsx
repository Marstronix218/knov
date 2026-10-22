import { DragEvent, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileUp, FlaskConical, Hash, LoaderCircle, Mail, RefreshCw } from "lucide-react";
import { EmptyState, errorMessage, PageHeader } from "../components/ui";
import { useResource } from "../hooks/useResource";
import { api, isDesktopRuntime } from "../lib/api";
import { connectorApi, fileToBase64, revenueApi } from "../lib/revenueApi";
import { setSimulationMode, useSimulationMode } from "../state/simulationMode";
import type { ConnectorConfig, ConnectorProvider, ConnectorScope, ConnectorStatus, ExtractedDocument, RevenueOverview, SlackChannel, SlackOAuthStart } from "../revenueTypes";
import "./home.css";
import "./revenue.css";

const label = (value: string) => value.replace(/_/g, " ");
const ACCEPTED = ".pdf,.docx,.doc,.rtf,.odt,.txt,.md,.markdown";
type Run = (operation: () => Promise<RevenueOverview>) => Promise<RevenueOverview | undefined>;

/**
 * Everything Knov reads about the business: uploaded contracts, Gmail and
 * Slack, and clients. Kept off Home so Home can stay about problems and actions.
 */
export function SourcesPage() {
  const simulation = useSimulationMode();
  const resource = useResource(() => revenueApi.overview(false), []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [projectId, setProjectId] = useState("");
  const data = resource.data;

  useEffect(() => {
    if (!projectId && data?.projects.length) setProjectId(data.projects[0].id);
  }, [data?.projects, projectId]);

  const run: Run = async (operation) => {
    setBusy(true); setError(""); setMessage("");
    try {
      const result = await operation();
      if (result.demo) throw new Error("Sources shows actual data only. Reload this view.");
      resource.setData(result);
      return result;
    } catch (cause) { setError(errorMessage(cause)); return undefined; }
    finally { setBusy(false); }
  };

  const tryDemo = async (step: "gmail" | "slack") => {
    setBusy(true); setError("");
    try {
      await revenueApi.simulationLoad(step);
      setSimulationMode(true);
      window.location.hash = "/simulation";
    } catch (cause) { setError(errorMessage(cause)); }
    finally { setBusy(false); }
  };

  const projectName = (id: string) => {
    const project = data?.projects.find((item) => item.id === id);
    const client = data?.clients.find((item) => item.id === project?.clientId);
    return project ? `${client?.name ?? "Client"} · ${project.name}` : "";
  };

  return <div className="page revenue-page sources-page">
    <PageHeader eyebrow="Your business data" title="Sources" description="Contracts, email, and Slack that Knov reads to find unbilled work, overdue promises, and missing invoices. Everything is analyzed on this Mac." />
    {!isDesktopRuntime() ? <EmptyState title="Open Sources in the desktop app" detail="Uploads and connections are stored in the local database. Browser preview cannot read files or connect accounts." /> : <>
      {simulation && <p className="notice info">Home is showing the simulated business. Sources you add here are your actual data and appear on Home once you <button className="link-button" onClick={() => setSimulationMode(false)}>switch back to your data</button>.</p>}
      {error && <p className="notice error" role="alert">{error}</p>}
      {message && <p className="notice ok" role="status">{message}</p>}
      {resource.error && <p className="notice error" role="alert">{resource.error} <button onClick={() => void resource.reload()}>Retry</button></p>}
      {resource.loading && !data && <p role="status">Loading…</p>}
      {data && <>
        <section className="revenue-panel" aria-label="Client">
          <h2>Which client is this for?</h2>
          {data.projects.length > 0 && <label>Client project<select value={projectId} onChange={(event) => setProjectId(event.target.value)}>{data.projects.map((project) => <option key={project.id} value={project.id}>{projectName(project.id)}</option>)}</select></label>}
          <details open={!data.projects.length}><summary>Add a new client</summary>
            <form className="stack-form" onSubmit={(event) => {
              event.preventDefault();
              const form = event.currentTarget;
              const fields = new FormData(form);
              const client = String(fields.get("client")).trim();
              const name = String(fields.get("name")).trim() || "General";
              void run(() => revenueApi.createProject(client, name, String(fields.get("description") ?? ""), [])).then((result) => {
                const created = result?.projects.find((project) => project.name === name && result.clients.find((item) => item.id === project.clientId)?.name.toLowerCase() === client.toLowerCase());
                if (created) { setProjectId(created.id); form.reset(); setMessage(`Added ${client}. Now upload a contract or connect a source for this client.`); }
              });
            }}>
              <div className="revenue-fields"><label>Client name<input name="client" required maxLength={160} placeholder="Acme Inc." /></label><label>Project (optional)<input name="name" maxLength={160} placeholder="Website redesign" /></label></div>
              <label>Description (optional)<input name="description" maxLength={2000} /></label>
              <button className="primary-button" disabled={busy}>Add client</button>
            </form>
          </details>
        </section>

        <DocumentUpload projectId={projectId} projectLabel={projectName(projectId)} busy={busy} run={run} onDone={setMessage} />

        <section className="revenue-panel" aria-label="Email and chat">
          <h2>Email & chat</h2>
          <p>Knov reads only the messages you select, read-only. It never sends email or posts in Slack.</p>
          <div className="connector-grid">
            <article><Mail size={18} /><div><strong>Gmail</strong><small>Client requests, approvals, and promises you made by email.</small></div><button className="ghost-button" disabled={busy} onClick={() => void tryDemo("gmail")}><FlaskConical size={14} /> Try with sample inbox</button></article>
            <article><Hash size={18} /><div><strong>Slack</strong><small>Requests that arrive in shared client channels.</small></div><button className="ghost-button" disabled={busy} onClick={() => void tryDemo("slack")}><FlaskConical size={14} /> Try with sample Slack</button></article>
          </div>
          <p className="field-hint">To connect your own account, Knov signs in with a Google or Slack app you create once. Sign-in details stay in macOS Keychain.</p>
          <GmailCard projectId={projectId} refresh={() => void resource.reload()} />
          <SlackCard projectId={projectId} refresh={() => void resource.reload()} />
        </section>

        <details className="revenue-panel"><summary>Advanced: review imported content and add evidence by hand</summary>
          <h3>Contracts for {projectName(projectId) || "the selected client"}</h3>
          {data.agreements.filter((item) => item.projectId === projectId).map((item) => <article key={item.id}><strong>{item.title}</strong><form className="stack-form" onSubmit={(event) => { event.preventDefault(); const text = String(new FormData(event.currentTarget).get("text")); void run(() => invoke<RevenueOverview>("revenue_update_agreement", { agreementId: item.id, text })); }}><label>Correct the extracted text<textarea name="text" defaultValue={String(item.text ?? "")} rows={5} required maxLength={64000} /></label><button className="ghost-button" disabled={busy}>Save correction</button></form></article>)}
          <EvidenceEntry projectId={projectId} busy={busy} run={run} />
          <h3>Synchronized messages to label</h3>
          <p>Imported messages are untrusted content. Label what each one means before Knov uses it; no message can trigger an action.</p>
          {data.evidence.filter((item) => item.projectId === projectId && item.kind === "communication").map((item) => <article className="revenue-evidence" key={item.id}><strong>{item.source} · {label(item.provenance)}</strong><blockquote>{item.excerpt}</blockquote><small>{item.sourceRef} · {new Date(item.occurredAt * 1000).toLocaleString()}</small><form className="stack-form" onSubmit={(e) => { e.preventDefault(); const fields = new FormData(e.currentTarget); const due = String(fields.get("due")); const amount = String(fields.get("amount")).trim(); void run(() => invoke<RevenueOverview>("revenue_classify_evidence", { evidenceId: item.id, kind: fields.get("kind"), dueAt: due ? Date.parse(due) / 1000 : null, amountCents: amount ? Math.round(Number(amount) * 100) : null, currency: amount ? String(fields.get("currency")) : null })); }}><label>What this message is<select name="kind">{["request", "activity", "approval", "commitment", "milestone_accepted", "invoice", "billing_complete"].map((kind) => <option value={kind} key={kind}>{label(kind)}</option>)}</select></label><label>Explicit due date, if stated (your local time)<input name="due" type="datetime-local" /></label><label>Amount explicitly supported by this source<input name="amount" type="number" min="0" step="0.01" /></label><label>Currency<input name="currency" defaultValue="USD" pattern="[A-Z]{3}" maxLength={3} /></label><button className="ghost-button" disabled={busy}>Save source classification</button></form></article>)}
        </details>
      </>}
    </>}
  </div>;
}

/** The revenue_import_agreement limits, which count UTF-8 bytes. */
const MAX_AGREEMENT_BYTES = 64_000;
const MAX_TITLE_BYTES = 160;
const byteLength = (text: string) => new TextEncoder().encode(text).length;

function DocumentUpload({ projectId, projectLabel, busy, run, onDone }: { projectId: string; projectLabel: string; busy: boolean; run: Run; onDone: (message: string) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const [uploading, setUploading] = useState("");
  const [uploadError, setUploadError] = useState("");
  // Extracted documents waiting for review, first one shown; nothing is stored until imported.
  const [reviews, setReviews] = useState<ExtractedDocument[]>([]);
  const [importedCount, setImportedCount] = useState(0);
  const current = reviews[0];
  const tooLong = current ? byteLength(current.text) > MAX_AGREEMENT_BYTES : false;
  const titleTooLong = current ? byteLength(current.title.trim()) > MAX_TITLE_BYTES : false;

  const upload = async (files: File[]) => {
    if (!projectId || !files.length) return;
    setUploadError("");
    const extracted: ExtractedDocument[] = [];
    for (const file of files) {
      setUploading(file.name);
      try {
        const data = await fileToBase64(file);
        extracted.push(await revenueApi.extractUpload(file.name, data));
      } catch (cause) {
        setUploadError(`${file.name}: ${errorMessage(cause)}`);
        break;
      }
    }
    setUploading("");
    if (extracted.length) setReviews((queue) => [...queue, ...extracted]);
  };

  const advance = async (imported: boolean) => {
    const count = importedCount + (imported ? 1 : 0);
    // Functional update: files dropped while this one was importing stay queued.
    setReviews((queue) => queue.slice(1));
    if (reviews.length > 1) { setImportedCount(count); return; }
    setImportedCount(0);
    if (count) {
      await run(() => revenueApi.analyze(false));
      onDone(`Imported ${count} ${count === 1 ? "document" : "documents"} for ${projectLabel}. Home shows anything Knov found.`);
    }
  };

  const importReviewed = async () => {
    if (!current || tooLong || titleTooLong || !current.text.trim()) return;
    if (await run(() => revenueApi.importAgreement(projectId, current.title.trim() || "Imported agreement", current.text))) await advance(true);
  };

  const editCurrent = (changes: Partial<ExtractedDocument>) => setReviews(([first, ...rest]) => [{ ...first, ...changes }, ...rest]);

  const drop = (event: DragEvent) => {
    event.preventDefault();
    setDragging(false);
    // Same guard as the Choose files button.
    if (busy || uploading) return;
    void upload([...event.dataTransfer.files]);
  };

  return <section className="revenue-panel" aria-label="Documents">
    <h2>Contracts & documents</h2>
    <p>Upload what the client agreed to: statements of work, proposals, contracts, or change orders. Knov extracts scope, exclusions, milestones, and payment terms.</p>
    <div className={`upload-zone${dragging ? " dragging" : ""}${projectId ? "" : " disabled"}`} onDragOver={(event) => { event.preventDefault(); setDragging(true); }} onDragLeave={() => setDragging(false)} onDrop={drop}>
      {uploading ? <LoaderCircle className="spin" size={22} /> : <FileUp size={22} />}
      <strong>{uploading ? `Reading ${uploading}…` : projectId ? `Drop files here for ${projectLabel}` : "Add a client first"}</strong>
      <small>PDF, Word, RTF, or text, up to 2 MB each. Scanned PDFs without a text layer can't be read yet.</small>
      <button type="button" className="primary-button" disabled={busy || !projectId || Boolean(uploading)} onClick={() => input.current?.click()}>Choose files</button>
      <input ref={input} type="file" multiple accept={ACCEPTED} hidden aria-label="Upload documents" onChange={(event) => { void upload([...(event.target.files ?? [])]); event.target.value = ""; }} />
    </div>
    {uploadError && <p className="notice error" role="alert">{uploadError}</p>}
    {current && <section className="stack-form" aria-label="Review extracted text">
      <h3>Review the extracted text{reviews.length > 1 ? ` (1 of ${reviews.length})` : ""}</h3>
      <p className="field-hint">Nothing is saved yet. Fix anything the extractor got wrong, then import it for {projectLabel}.</p>
      <label>Document title<input value={current.title} maxLength={160} onChange={(event) => editCurrent({ title: event.target.value })} /></label>
      <label>Extracted text<textarea value={current.text} rows={12} onChange={(event) => editCurrent({ text: event.target.value })} /></label>
      <small aria-live="polite">{current.text.length.toLocaleString()} characters</small>
      {titleTooLong && <p className="notice error" role="alert">This title is longer than the 160-byte import limit. Shorten it; characters outside basic Latin count as more than one byte.</p>}
      {tooLong && <p className="notice error" role="alert">This text is longer than the 64,000-byte import limit. Trim it to the parts that describe scope, milestones, and payment terms.</p>}
      <div className="revenue-actions">
        <button type="button" className="primary-button" disabled={busy || tooLong || titleTooLong || !current.text.trim() || !projectId} onClick={() => void importReviewed()}>Import this text</button>
        <button type="button" className="ghost-button" disabled={busy} onClick={() => void advance(false)}>Cancel</button>
      </div>
    </section>}
    <details><summary>Or paste the text instead</summary>
      <form className="stack-form" onSubmit={(event) => { event.preventDefault(); const form = event.currentTarget; const fields = new FormData(form); void run(() => revenueApi.importAgreement(projectId, String(fields.get("title")), String(fields.get("text")))).then((result) => { if (result) { form.reset(); onDone("Document saved."); } }); }}>
        <label>Document title<input name="title" required maxLength={160} /></label>
        <label>Plain text or Markdown<textarea name="text" rows={5} required maxLength={64000} /></label>
        <button className="ghost-button" disabled={busy || !projectId}>Save document</button>
      </form>
    </details>
  </section>;
}

function EvidenceEntry({ projectId, busy, run }: { projectId: string; busy: boolean; run: Run }) {
  return <details><summary>Add reviewed work or billing evidence</summary><form className="stack-form" onSubmit={(e) => { e.preventDefault(); const fields = new FormData(e.currentTarget); const due = String(fields.get("due")); void run(() => invoke<RevenueOverview>("revenue_add_evidence", { projectId, kind: fields.get("kind"), sourceRef: fields.get("source"), text: fields.get("text"), occurredAt: Date.parse(String(fields.get("occurred"))) / 1000, dueAt: due ? Date.parse(due) / 1000 : null, amountCents: String(fields.get("amount")).trim() ? Math.round(Number(fields.get("amount")) * 100) : null, currency: String(fields.get("amount")).trim() ? String(fields.get("currency")) : null })); }}><label>Kind<select name="kind">{["request", "activity", "approval", "milestone_accepted", "invoice", "commitment", "billing_complete"].map((kind) => <option value={kind} key={kind}>{label(kind)}</option>)}</select></label><label>Source reference<input name="source" required /></label><label>Exact evidence<textarea name="text" required rows={3} /></label><label>Occurred at (your local time)<input name="occurred" type="datetime-local" required /></label><label>Explicit commitment due date (optional, your local time)<input name="due" type="datetime-local" /></label><div className="revenue-fields"><label>Evidence-supported amount (optional)<input name="amount" type="number" min="0" step="0.01" /></label><label>Currency<input name="currency" defaultValue="USD" pattern="[A-Z]{3}" maxLength={3} /></label></div><p className="field-hint">Only mark billing complete when the imported billing records actually cover this project and period.</p><button className="primary-button" disabled={busy || !projectId}>Add evidence</button></form></details>;
}

const DAY_MS = 86_400_000;
/** A UTC calendar day `offset` days from today, in the date input's format. */
const utcDay = (offset = 0) => new Date(Date.now() + offset * DAY_MS).toISOString().slice(0, 10);
const dayOf = (seconds: number) => new Date(seconds * 1000).toISOString().slice(0, 10);
const MAX_CHANNELS = 10;
const GMAIL_API_URL = "https://console.cloud.google.com/apis/library/gmail.googleapis.com";
const GOOGLE_CLIENTS_URL = "https://console.cloud.google.com/apis/credentials";
const SLACK_APPS_URL = "https://api.slack.com/apps";
const CONNECTOR_GUIDE_URL = "https://github.com/Marstronix218/knov/blob/main/docs/revenue-connectors.md";
const providerName = (provider: ConnectorProvider) => provider === "gmail" ? "Gmail" : "Slack";

interface RangeValue { after: string; before: string; limit: number }
/** Last 90 days through today; `before` is exclusive, so it is tomorrow. */
const defaultRange = (): RangeValue => ({ after: utcDay(-90), before: utcDay(1), limit: 50 });
const rangeFromScope = (scope: ConnectorScope | null | undefined): RangeValue => scope ? { after: dayOf(scope.after), before: dayOf(scope.before), limit: scope.maxItems } : defaultRange();
const scopeOf = (range: RangeValue, extra: Partial<ConnectorScope> = {}): ConnectorScope => ({ query: "", channels: [], after: Date.parse(range.after) / 1000, before: Date.parse(range.before) / 1000, maxItems: range.limit, ...extra });
const describeRange = (scope: ConnectorScope) => `${dayOf(scope.after)} to ${dayOf(scope.before - 1)}, up to ${scope.maxItems} per sync`;
const openLink = (url: string) => () => void api.openResource(url).catch(() => undefined);

type Operate = (operation: () => Promise<string | void>) => Promise<void>;

/** Status, progress and messages for one provider's card. */
function useConnector(provider: ConnectorProvider, refresh: () => void) {
  const [status, setStatus] = useState<ConnectorStatus>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  useEffect(() => {
    let active = true;
    connectorApi.status(provider).then((value) => { if (active) setStatus(value); }).catch((cause) => { if (active) setError(errorMessage(cause)); });
    return () => { active = false; };
  }, [provider]);
  const operate: Operate = async (operation) => {
    setBusy(true); setError(""); setMessage("");
    try { const done = await operation(); if (done) setMessage(done); }
    catch (cause) { setError(errorMessage(cause)); }
    finally {
      try { setStatus(await connectorApi.status(provider)); }
      catch { setStatus(undefined); }
      refresh();
      setBusy(false);
    }
  };
  return { status, busy, error, message, setMessage, operate };
}

function RangeFields({ value, onChange, noun }: { value: RangeValue; onChange: (value: RangeValue) => void; noun: string }) {
  return <>
    <div className="revenue-fields">
      <label>Read from (UTC date)<input type="date" required value={value.after} onChange={(event) => onChange({ ...value, after: event.target.value })} /></label>
      <label>Read before (UTC date)<input type="date" required value={value.before} onChange={(event) => onChange({ ...value, before: event.target.value })} /></label>
    </div>
    <label>Maximum {noun} per sync<input type="number" min={1} max={100} required value={value.limit} onChange={(event) => onChange({ ...value, limit: Number(event.target.value) })} /></label>
  </>;
}

function SyncActions({ provider, projectId, canSync, busy, operate }: { provider: ConnectorProvider; projectId: string; canSync: boolean; busy: boolean; operate: Operate }) {
  const sync = () => operate(async () => {
    const result = await connectorApi.sync(provider, projectId);
    const noun = result.imported === 1 ? "message" : "messages";
    return `Imported ${result.imported} ${noun}${result.incremental ? " new since the last sync" : ""}.${result.truncated ? " The per-sync limit was reached, so some messages were not imported; raise the limit or narrow the dates." : ""}`;
  });
  const disconnect = () => operate(async () => { await connectorApi.disconnect(provider); return `${providerName(provider)} disconnected and Knov's access revoked.`; });
  return <>
    {!projectId && <p className="field-hint">Add or choose a client above to sync into.</p>}
    <div className="revenue-actions">
      <button type="button" className="primary-button" disabled={busy || !projectId || !canSync} onClick={() => void sync()}>{busy ? <LoaderCircle className="spin" size={14} /> : <RefreshCw size={14} />} Sync now</button>
      <button type="button" className="ghost-button" disabled={busy} onClick={() => void disconnect()}>Disconnect</button>
    </div>
  </>;
}

function DeleteImported({ provider, busy, operate }: { provider: ConnectorProvider; busy: boolean; operate: Operate }) {
  const [confirmed, setConfirmed] = useState(false);
  const name = providerName(provider);
  return <details><summary>Delete imported {name} content</summary>
    <p>Removes {name}'s imported excerpts and copied content in dependent drafts. Financial review history remains. Disconnect first to prevent future imports.</p>
    <label className="revenue-check"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} /> Delete imported {name} content from this Mac</label>
    <button type="button" className="danger-button" disabled={busy || !confirmed} onClick={() => void operate(async () => { await connectorApi.deleteImported(provider); setConfirmed(false); return `Imported ${name} content deleted from this Mac.`; })}>Delete imported {name} content</button>
  </details>;
}

function Notices({ error, message }: { error: string; message: string }) {
  return <>{error && <p role="alert" className="notice error">{error}</p>}{message && <p role="status" className="notice ok">{message}</p>}</>;
}

function GmailCard({ projectId, refresh }: { projectId: string; refresh: () => void }) {
  const { status, busy, error, message, operate } = useConnector("gmail", refresh);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [query, setQuery] = useState("newer_than:90d");
  const [range, setRange] = useState(defaultRange);
  const [authorized, setAuthorized] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const [token, setToken] = useState("");
  const scope = scopeOf(range, { query: query.trim() });
  const connected = (how: Promise<ConnectorStatus>) => operate(async () => {
    await how;
    setClientSecret(""); setToken("");
    return "Gmail connected. Choose the client above and press Sync now.";
  });
  const signIn = async () => {
    setWaiting(true);
    try { await connected(connectorApi.gmailSignIn(clientId.trim(), clientSecret.trim() || null, scope, authorized)); }
    finally { setWaiting(false); }
  };

  return <section className="revenue-sources" aria-label="Gmail connection">
    <h3><Mail size={15} /> Gmail {status?.connected ? "· Connected" : "· Not connected"}</h3>
    <Notices error={error} message={message} />
    {!status ? !error && <p>Checking connection…</p> : status.connected ? <>
      <p><strong>{status.account}</strong></p>
      {status.scope && <p className="field-hint">Reads emails matching <code>{status.scope.query}</code>, {describeRange(status.scope)}. Read-only; Knov never sends email.</p>}
      <p>{status.lastSync ? `Last sync: ${new Date(status.lastSync * 1000).toLocaleString()}` : "Not synced yet."}</p>
      {status.watermark ? <p className="field-hint">Next sync fetches only mail newer than {new Date(status.watermark * 1000).toLocaleString()}.</p> : null}
      <SyncActions provider="gmail" projectId={projectId} canSync busy={busy} operate={operate} />
    </> : <form className="stack-form" onSubmit={(event) => { event.preventDefault(); void signIn(); }}>
      <ol>
        <li><strong>Create a Google sign-in for Knov</strong> (one time, about five minutes)
          <ol>
            <li>In Google Cloud, <button type="button" className="link-button" onClick={openLink(GMAIL_API_URL)}>enable the Gmail API</button> for a project.</li>
            <li>Set up the OAuth consent screen (External, Testing) and add your Gmail address as a test user.</li>
            <li><button type="button" className="link-button" onClick={openLink(GOOGLE_CLIENTS_URL)}>Create an OAuth client ID</button> with application type <b>Desktop app</b>.</li>
            <li>Copy its Client ID, and Client secret if shown, into step 2. <button type="button" className="link-button" onClick={openLink(CONNECTOR_GUIDE_URL)}>Step-by-step guide</button></li>
          </ol>
        </li>
        <li><strong>Choose what Knov may read</strong>
          <label>Google client ID<input value={clientId} onChange={(event) => setClientId(event.target.value)} required maxLength={256} placeholder="…apps.googleusercontent.com" autoComplete="off" /></label>
          <label>Google client secret (optional)<input type="password" value={clientSecret} onChange={(event) => setClientSecret(event.target.value)} maxLength={8192} autoComplete="off" /></label>
          <label>Which emails (Gmail search)<input value={query} onChange={(event) => setQuery(event.target.value)} required maxLength={512} /></label>
          <p className="field-hint">Narrow it to one client with, for example, <code>from:@theirdomain.com</code>. The dates below also apply.</p>
          <RangeFields value={range} onChange={setRange} noun="emails" />
          <label className="revenue-check"><input type="checkbox" checked={authorized} onChange={(event) => setAuthorized(event.target.checked)} /> Knov may read only emails matching this search and these dates, read-only</label>
        </li>
        <li><strong>Sign in</strong>
          <div className="revenue-actions"><button className="primary-button" disabled={busy || !authorized || !clientId.trim()}>{waiting ? <LoaderCircle className="spin" size={14} /> : <Mail size={14} />} Sign in with Google</button></div>
          {waiting && <p className="notice info">Waiting for you to approve access in your browser. This times out after 3 minutes.</p>}
        </li>
      </ol>
      <details><summary>Advanced: connect with an access token</summary>
        <p className="field-hint">An access token issued with the gmail.readonly scope. It is not renewed; reconnect when it expires.</p>
        <label>Gmail access token<input type="password" value={token} onChange={(event) => setToken(event.target.value)} autoComplete="off" maxLength={8192} /></label>
        <button type="button" className="ghost-button" disabled={busy || !authorized || !token.trim()} onClick={() => void connected(connectorApi.connectWithToken("gmail", token.trim(), scope, authorized))}>Connect with token</button>
      </details>
    </form>}
    <DeleteImported provider="gmail" busy={busy} operate={operate} />
  </section>;
}

function SlackCard({ projectId, refresh }: { projectId: string; refresh: () => void }) {
  const { status, busy, error, message, setMessage, operate } = useConnector("slack", refresh);
  const [config, setConfig] = useState<ConnectorConfig>();
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [range, setRange] = useState(defaultRange);
  const [authorized, setAuthorized] = useState(false);
  const [started, setStarted] = useState<SlackOAuthStart>();
  const [code, setCode] = useState("");
  const [token, setToken] = useState("");
  const [tokenChannels, setTokenChannels] = useState("");
  useEffect(() => { void connectorApi.config().then(setConfig).catch(() => undefined); }, []);
  const redirectUrl = config?.slackRedirectUrl;

  const start = () => operate(async () => {
    const result = await connectorApi.slackStart(clientId.trim(), clientSecret.trim(), scopeOf(range), authorized);
    setStarted(result); setClientSecret("");
  });
  const complete = () => operate(async () => {
    if (!started) return;
    await connectorApi.slackComplete(code.trim(), started.state);
    setStarted(undefined); setCode("");
    return "Slack connected. Pick the channels Knov may read.";
  });
  const connectWithToken = () => operate(async () => {
    const channels = tokenChannels.split(",").map((item) => item.trim()).filter(Boolean);
    await connectorApi.connectWithToken("slack", token.trim(), scopeOf(range, { channels }), authorized);
    setToken("");
    return "Slack connected.";
  });
  const copyRedirect = () => { if (redirectUrl) void navigator.clipboard?.writeText(redirectUrl).then(() => setMessage("Redirect URL copied.")).catch(() => undefined); };

  return <section className="revenue-sources" aria-label="Slack connection">
    <h3><Hash size={15} /> Slack {status?.connected ? "· Connected" : "· Not connected"}</h3>
    <Notices error={error} message={message} />
    {!status ? !error && <p>Checking connection…</p> : status.connected ? <>
      <p><strong>{status.account}</strong></p>
      {status.scope && <p className="field-hint">{status.scope.channels.length ? `Reads ${status.scope.channels.length} selected ${status.scope.channels.length === 1 ? "channel" : "channels"}, ${describeRange(status.scope)}.` : "No channels selected yet."} Read-only; Knov never posts in Slack.</p>}
      <p>{status.lastSync ? `Last sync: ${new Date(status.lastSync * 1000).toLocaleString()}` : "Not synced yet."}</p>
      <SlackChannelPicker status={status} busy={busy} operate={operate} />
      <SyncActions provider="slack" projectId={projectId} canSync={Boolean(status.scope?.channels.length)} busy={busy} operate={operate} />
    </> : started ? <form className="stack-form" aria-label="Finish Slack sign-in" onSubmit={(event) => { event.preventDefault(); void complete(); }}>
      <p>{started.opened ? "Slack is open in your browser." : "Open Slack to approve access."} After you click <b>Allow</b>, a Knov page shows a one-time code.</p>
      {!started.opened && <p><button type="button" className="link-button" onClick={openLink(started.authorizeUrl)}>Open the Slack approval page</button></p>}
      <label>Paste the code from the browser<input value={code} onChange={(event) => setCode(event.target.value)} required maxLength={256} autoComplete="off" spellCheck={false} /></label>
      <div className="revenue-actions">
        <button className="primary-button" disabled={busy || !code.trim()}>Finish connecting</button>
        <button type="button" className="ghost-button" disabled={busy} onClick={() => setStarted(undefined)}>Start over</button>
      </div>
    </form> : <form className="stack-form" onSubmit={(event) => { event.preventDefault(); void start(); }}>
      {status.pending && <p className="notice info">A Slack sign-in was started but not finished. Start it again below.</p>}
      <ol>
        <li><strong>Create a Slack app for Knov</strong> (one time)
          <ol>
            <li><button type="button" className="link-button" onClick={openLink(SLACK_APPS_URL)}>Open your Slack apps</button>, choose <b>Create New App → From scratch</b>, and pick your workspace.</li>
            <li>In <b>OAuth &amp; Permissions → Redirect URLs</b>, add this URL and save: <code>{redirectUrl ?? "…"}</code> <button type="button" className="link-button" disabled={!redirectUrl} onClick={copyRedirect}>Copy URL</button></li>
            <li>Under <b>User Token Scopes</b> (not Bot Token Scopes), add {(config?.slackUserScopes ?? []).map((scope, index) => <span key={scope}>{index ? ", " : ""}<code>{scope}</code></span>)}.</li>
            <li>In <b>Basic Information</b>, copy the Client ID and Client Secret into step 2.</li>
          </ol>
        </li>
        <li><strong>Connect</strong>
          <label>Slack client ID<input value={clientId} onChange={(event) => setClientId(event.target.value)} required maxLength={64} placeholder="1234567890.1234567890" autoComplete="off" /></label>
          <label>Slack client secret<input type="password" value={clientSecret} onChange={(event) => setClientSecret(event.target.value)} required maxLength={256} autoComplete="off" /></label>
          <RangeFields value={range} onChange={setRange} noun="messages" />
          <label className="revenue-check"><input type="checkbox" checked={authorized} onChange={(event) => setAuthorized(event.target.checked)} /> Knov may read only the channels I pick next, within these dates, read-only</label>
          <div className="revenue-actions"><button className="primary-button" disabled={busy || !authorized || !clientId.trim() || !clientSecret.trim()}><Hash size={14} /> Open Slack to authorize</button></div>
        </li>
      </ol>
      <details><summary>Advanced: connect with a token</summary>
        <p className="field-hint">A Slack token with only the read scopes above, plus the channel IDs to read.</p>
        <label>Slack token<input type="password" value={token} onChange={(event) => setToken(event.target.value)} autoComplete="off" maxLength={8192} /></label>
        <label>Channel IDs (comma-separated, up to 10)<input value={tokenChannels} onChange={(event) => setTokenChannels(event.target.value)} placeholder="C0123ABCD, C0456EFGH" /></label>
        <button type="button" className="ghost-button" disabled={busy || !authorized || !token.trim() || !tokenChannels.trim()} onClick={() => void connectWithToken()}>Connect with token</button>
      </details>
    </form>}
    <DeleteImported provider="slack" busy={busy} operate={operate} />
  </section>;
}

/** Lists channels by name; sync still reads only the 1–10 saved here. */
function SlackChannelPicker({ status, busy, operate }: { status: ConnectorStatus; busy: boolean; operate: Operate }) {
  const [channels, setChannels] = useState<SlackChannel[]>();
  const [listError, setListError] = useState("");
  const [selected, setSelected] = useState<string[]>(() => status.scope?.channels ?? []);
  const [manual, setManual] = useState(() => (status.scope?.channels ?? []).join(", "));
  const [range, setRange] = useState(() => rangeFromScope(status.scope));
  useEffect(() => {
    let active = true;
    connectorApi.slackChannels().then((list) => { if (active) setChannels(list); }).catch((cause) => { if (active) setListError(errorMessage(cause)); });
    return () => { active = false; };
  }, []);
  const toggle = (id: string) => setSelected((current) => current.includes(id) ? current.filter((item) => item !== id) : current.length >= MAX_CHANNELS ? current : [...current, id]);
  const chosen = channels ? selected : manual.split(",").map((item) => item.trim()).filter(Boolean);
  const save = () => operate(async () => {
    await connectorApi.updateScope("slack", scopeOf(range, { channels: chosen }));
    return `Saved ${chosen.length} ${chosen.length === 1 ? "channel" : "channels"}. Sync reads only these.`;
  });
  return <form className="stack-form" aria-label="Slack channels" onSubmit={(event) => { event.preventDefault(); void save(); }}>
    <strong>Channels Knov may read</strong>
    {channels ? channels.length ? <fieldset>
      <legend>Pick up to {MAX_CHANNELS} ({selected.length} of {MAX_CHANNELS} selected)</legend>
      {channels.map((channel) => {
        const checked = selected.includes(channel.id);
        return <label className="revenue-check" key={channel.id}><input type="checkbox" checked={checked} disabled={!checked && selected.length >= MAX_CHANNELS} onChange={() => toggle(channel.id)} /> #{channel.name}{channel.isPrivate ? " (private)" : ""}{channel.isMember ? "" : " · you're not a member"}</label>;
      })}
    </fieldset> : <p>No channels found in this workspace.</p> : listError ? <>
      <p className="notice error" role="alert">{listError}</p>
      <label>Channel IDs (comma-separated, up to 10)<input value={manual} onChange={(event) => setManual(event.target.value)} placeholder="C0123ABCD, C0456EFGH" /></label>
    </> : <p>Loading channels…</p>}
    <RangeFields value={range} onChange={setRange} noun="messages" />
    <div className="revenue-actions"><button className="ghost-button" disabled={busy || !chosen.length || chosen.length > MAX_CHANNELS}>Save selected channels</button></div>
  </form>;
}
