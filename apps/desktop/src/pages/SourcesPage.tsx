import { DragEvent, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileUp, FlaskConical, Hash, LoaderCircle, Mail } from "lucide-react";
import { EmptyState, errorMessage, PageHeader } from "../components/ui";
import { useResource } from "../hooks/useResource";
import { isDesktopRuntime } from "../lib/api";
import { fileToBase64, revenueApi } from "../lib/revenueApi";
import { setSimulationMode, useSimulationMode } from "../state/simulationMode";
import type { RevenueOverview } from "../revenueTypes";
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
          <details><summary>Connect your own account (developer setup required for now)</summary>
            <p className="field-hint">One-click sign-in needs Knov's own Google and Slack apps, which are not published yet. Until then, connecting requires your own Google OAuth client ID or a Slack token. See docs/revenue-connectors.md.</p>
            <RevenueSources projectId={projectId} refresh={() => void resource.reload()} />
          </details>
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

function DocumentUpload({ projectId, projectLabel, busy, run, onDone }: { projectId: string; projectLabel: string; busy: boolean; run: Run; onDone: (message: string) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const [uploading, setUploading] = useState("");

  const upload = async (files: File[]) => {
    if (!projectId || !files.length) return;
    let imported = 0;
    for (const file of files) {
      setUploading(file.name);
      const data = await fileToBase64(file);
      if (await run(() => revenueApi.importUpload(projectId, file.name, data))) imported += 1;
      else break;
    }
    setUploading("");
    if (imported) {
      await run(() => revenueApi.analyze(false));
      onDone(`Imported ${imported} ${imported === 1 ? "document" : "documents"} for ${projectLabel}. Home shows anything Knov found.`);
    }
  };

  const drop = (event: DragEvent) => {
    event.preventDefault();
    setDragging(false);
    void upload([...event.dataTransfer.files]);
  };

  return <section className="revenue-panel" aria-label="Documents">
    <h2>Contracts & documents</h2>
    <p>Upload what the client agreed to: statements of work, proposals, contracts, or change orders. Knov extracts scope, exclusions, milestones, and payment terms.</p>
    <div className={`upload-zone${dragging ? " dragging" : ""}${projectId ? "" : " disabled"}`} onDragOver={(event) => { event.preventDefault(); setDragging(true); }} onDragLeave={() => setDragging(false)} onDrop={drop}>
      {uploading ? <LoaderCircle className="spin" size={22} /> : <FileUp size={22} />}
      <strong>{uploading ? `Reading ${uploading}…` : projectId ? `Drop files here for ${projectLabel}` : "Add a client first"}</strong>
      <small>PDF, Word, RTF, or text, up to 2 MB each. Scanned PDFs without a text layer can't be read yet.</small>
      <button type="button" className="primary-button" disabled={busy || !projectId} onClick={() => input.current?.click()}>Choose files</button>
      <input ref={input} type="file" multiple accept={ACCEPTED} hidden aria-label="Upload documents" onChange={(event) => { void upload([...(event.target.files ?? [])]); event.target.value = ""; }} />
    </div>
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

interface SourceStatus { provider: string; connected: boolean; account: string | null; lastSync: number | null; scope: { query: string; after: number; before: number; channels: string[]; maxItems: number } | null }
function RevenueSources({ projectId, refresh }: { projectId: string; refresh: () => void }) {
  const [provider, setProvider] = useState("gmail");
  const [status, setStatus] = useState<SourceStatus>();
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [deleteSourceConfirmed, setDeleteSourceConfirmed] = useState(false);
  useEffect(() => { let active = true; setStatus(undefined); setError(""); setDeleteSourceConfirmed(false); void invoke<SourceStatus>("revenue_connector_status", { provider }).then((value) => { if (active) setStatus(value); }).catch((cause) => { if (active) setError(errorMessage(cause)); }); return () => { active = false; }; }, [provider]);
  const operate = async (operation: () => Promise<unknown>) => {
    setBusy(true); setError(""); setMessage("");
    try { await operation(); }
    catch (cause) { setError(errorMessage(cause)); }
    finally {
      try { setStatus(await invoke<SourceStatus>("revenue_connector_status", { provider })); }
      catch { setStatus(undefined); }
      refresh();
      setBusy(false);
    }
  };
  return <section className="revenue-sources"><label>Provider<select value={provider} disabled={busy} onChange={(e) => setProvider(e.target.value)}><option value="gmail">Gmail</option><option value="slack">Slack</option></select></label>{error && <p role="alert" className="notice error">{error}</p>}{message && <p role="status" className="notice info">{message}</p>}<p>{status?.connected ? `Connected: ${status.account ?? provider}` : "Disconnected · No live source is synchronized"}</p>{status?.scope && <pre>{JSON.stringify(status.scope, null, 2)}</pre>}{status?.lastSync && <p>Last sync: {new Date(status.lastSync * 1000).toLocaleString()}</p>}
    {!status?.connected ? <form className="stack-form" onSubmit={(e) => { e.preventDefault(); const fields = new FormData(e.currentTarget); const form = e.currentTarget; const token = String(fields.get("token")); const clientId = String(fields.get("clientId") ?? "").trim(); const clientSecret = String(fields.get("clientSecret") ?? "").trim() || null; void operate(async () => { await invoke(provider === "gmail" && clientId ? "revenue_gmail_oauth" : "revenue_connector_connect", { provider, token, clientId, clientSecret, authorized: fields.get("authorized") === "on", scope: { query: String(fields.get("query") ?? ""), channels: String(fields.get("channels") ?? "").split(",").map((item) => item.trim()).filter(Boolean), after: Date.parse(String(fields.get("after"))) / 1000, before: Date.parse(String(fields.get("before"))) / 1000, maxItems: Number(fields.get("limit")) } }); form.reset(); setMessage("Authorized scope saved. Sync reads only the selected sources."); }); }}>
      <p className="field-hint">Credentials are stored in macOS Keychain and never in browser storage. A Google Desktop client ID opens browser authorization; Slack requires a read-only authorized token.</p>{provider === "gmail" && <><label>Google Desktop OAuth client ID (preferred)<input name="clientId" placeholder="…apps.googleusercontent.com" /></label><label>Desktop client secret (optional, if required by your configuration)<input name="clientSecret" type="password" autoComplete="off" maxLength={8192} /></label></>}<label>{provider === "gmail" ? "Or an authorized access token" : "Authorized access token"}<input name="token" type="password" required={provider === "slack"} autoComplete="off" /></label>{provider === "gmail" ? <label>Gmail query<input name="query" required placeholder="from:client.example.com" /></label> : <label>Selected Slack channel IDs<input name="channels" required placeholder="C0123,C0456" /></label>}<div className="revenue-fields"><label>Read from (UTC date)<input type="date" name="after" required /></label><label>Read before (UTC date)<input type="date" name="before" required /></label></div><label>Maximum records per sync<input type="number" name="limit" min="1" max="100" defaultValue="50" required /></label><label className="revenue-check"><input name="authorized" type="checkbox" required /> I authorize this account and only the source scope above</label><button className="primary-button" disabled={busy}>Authorize selected scope</button>
    </form> : <div className="revenue-actions"><button className="primary-button" disabled={busy || !projectId} onClick={() => void operate(async () => { const result = await invoke<{ imported: number; truncated: boolean }>("revenue_connector_sync", { provider, projectId }); setMessage(`Imported ${result.imported} records${result.truncated ? "; selected limit reached, history may be incomplete" : ""}.`); })}>Sync to selected project</button><button className="ghost-button" disabled={busy} onClick={() => void operate(() => invoke("revenue_connector_disconnect", { provider, revoke: true }))}>Disconnect & revoke</button></div>}
    <details><summary>Delete imported {provider === "gmail" ? "Gmail" : "Slack"} content</summary><p>Removes this source's imported excerpts and copied content in dependent drafts. Financial review history remains. Disconnect first to prevent future imports.</p><label className="revenue-check"><input type="checkbox" checked={deleteSourceConfirmed} onChange={(event) => setDeleteSourceConfirmed(event.target.checked)} /> Delete this source's imported content from this Mac</label><button className="danger-button" disabled={busy || !deleteSourceConfirmed} onClick={() => void operate(async () => { await invoke("revenue_delete_source", { source: provider }); setDeleteSourceConfirmed(false); setMessage("Imported source content deleted from this Mac."); })}>Delete imported source content</button></details>
  </section>;
}
