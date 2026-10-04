import { CirclePause, FolderGit2, History, Inbox, LoaderCircle, Play, ShieldCheck, Target, Trash2 } from "lucide-react";
import { FormEvent, useState } from "react";
import { RunReview } from "../components/agent/RunReview";
import { EmptyState, errorMessage, PageHeader, ResourceState, Segmented, SettingsHeading } from "../components/ui";
import { useResource } from "../hooks/useResource";
import { api } from "../lib/api";
import { isTerminalRun, originLabel, percent, relativeTime, riskLabel, runStatus } from "../lib/agentFormat";
import { useAppStatus } from "../state/AppStatus";
import type { AgentRun, AutonomyOverview, Goal, GrantRequest } from "../types";

type Tab = "work" | "permissions" | "insights";

export function AgentPage() {
  const { agent, setAgentPaused, refreshAgent } = useAppStatus();
  const runs = useResource(() => api.agentRuns(40), []);
  const autonomy = useResource(() => api.autonomy(), []);
  const [tab, setTab] = useState<Tab>("work");
  const [reviewing, setReviewing] = useState<AgentRun>();
  const [toggling, setToggling] = useState(false);
  const [error, setError] = useState("");
  const paused = autonomy.data?.paused ?? agent.data?.paused ?? false;
  const awaiting = runs.data?.filter((run) => run.status === "awaiting_approval") ?? [];

  const toggleAgent = async () => {
    setToggling(true);
    setError("");
    try {
      await setAgentPaused(!paused);
      await autonomy.reload();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setToggling(false);
    }
  };

  return (
    <div className="page agent-page">
      <PageHeader
        eyebrow="Your delegate"
        title="Delegated work"
        description="Everything Knov prepared or did for you, what it believed at the time, the permission that allowed it, and how to undo it."
        actions={
          <button type="button" className={paused ? "primary-button" : "ghost-button"} disabled={toggling} onClick={() => void toggleAgent()}>
            {toggling ? <LoaderCircle size={15} className="spin" /> : paused ? <Play size={15} /> : <CirclePause size={15} />}
            {paused ? "Resume agent" : "Pause agent"}
          </button>
        }
      />
      <div className={`agent-state-banner ${paused ? "paused" : "active"}`}>
        <span className="status-light" aria-hidden="true" />
        <strong>{paused ? "Agent paused" : "Agent active"}</strong>
        <span>{paused
          ? "Nothing runs, even with permission. Activity collection is separate and unaffected."
          : "Knov runs only what you approve or have explicitly allowed. It never sends messages, deletes data, or changes repositories."}</span>
      </div>
      {error && <p className="notice error" role="alert">{error}</p>}

      <Segmented
        label="Delegated work views"
        value={tab}
        onChange={setTab}
        options={[
          { value: "work", label: "Work", count: awaiting.length || undefined },
          { value: "permissions", label: "Permissions" },
          { value: "insights", label: "Insights" },
        ]}
      />

      <div className="tab-body">
        {tab === "work" && (
          <ResourceState {...runs}>
            {(list) => list.length ? (
              <div className="run-columns">
                {awaiting.length > 0 && (
                  <section aria-label="Needs your decision">
                    <h2 className="section-heading"><Inbox size={16} aria-hidden="true" /> Needs your decision</h2>
                    <div className="run-list">{awaiting.map((run) => <RunRow key={run.id} run={run} onOpen={() => setReviewing(run)} />)}</div>
                  </section>
                )}
                <section aria-label="Action history">
                  <h2 className="section-heading"><History size={16} aria-hidden="true" /> History</h2>
                  <div className="run-list">{list.filter((run) => run.status !== "awaiting_approval").map((run) => <RunRow key={run.id} run={run} onOpen={() => setReviewing(run)} />)}</div>
                </section>
              </div>
            ) : (
              <EmptyState
                icon={<Inbox size={22} />}
                title="No delegated work yet"
                detail="Turn a confirmed workflow into a skill, then run it. Every action waits for your approval until you allow otherwise."
                action={<a className="ghost-button" href="#/workflows">Go to Workflows</a>}
              />
            )}
          </ResourceState>
        )}
        {tab === "permissions" && (
          <ResourceState {...autonomy}>
            {(data) => <PermissionsPanel data={data} onChange={(next) => { autonomy.setData(next); void refreshAgent(); }} />}
          </ResourceState>
        )}
        {tab === "insights" && (
          <InsightsPanel goals={agent.data?.state.goals ?? []} autonomy={autonomy.data} onGoalsChanged={() => void refreshAgent()} />
        )}
      </div>

      {reviewing && (
        <RunReview
          run={reviewing}
          onClose={() => {
            setReviewing(undefined);
            void runs.reload();
            void refreshAgent();
          }}
          onChanged={(next) => runs.setData((current) => current?.map((run) => run.id === next.id ? next : run))}
        />
      )}
    </div>
  );
}

function RunRow({ run, onOpen }: { run: AgentRun; onOpen: () => void }) {
  const status = runStatus(run.status);
  return (
    <button type="button" className="run-row" onClick={onOpen}>
      <span className={`tone-pill ${status.tone}`}>{!isTerminalRun(run.status) && run.status !== "awaiting_approval" && <LoaderCircle size={12} className="spin" />}{status.label}</span>
      <span className="run-row-copy">
        <strong>{run.title}</strong>
        <small>{originLabel(run.origin)} · {relativeTime(run.createdAt)} · {run.summary}</small>
      </span>
      <span className="run-row-count">{run.actions.length} action{run.actions.length === 1 ? "" : "s"}</span>
    </button>
  );
}

function PermissionsPanel({ data, onChange }: { data: AutonomyOverview; onChange: (next: AutonomyOverview) => void }) {
  const { settings } = useAppStatus();
  const skills = useResource(() => api.skills(), []);
  const [busy, setBusy] = useState<string>();
  const [notice, setNotice] = useState<{ tone: "ok" | "error"; text: string }>();
  const [path, setPath] = useState("");
  const [budget, setBudget] = useState(String(data.maxActionsPerHour));
  const [grant, setGrant] = useState<GrantRequest>({ actionType: "write_draft", scopeKind: "global", mode: "auto" });
  const [duration, setDuration] = useState("");
  const available = data.catalog.filter((kind) => kind.available);

  const run = async (key: string, work: () => Promise<AutonomyOverview | void>, success?: string) => {
    setBusy(key);
    setNotice(undefined);
    try {
      const next = await work();
      if (next) onChange(next);
      if (success) setNotice({ tone: "ok", text: success });
    } catch (cause) {
      setNotice({ tone: "error", text: errorMessage(cause) });
    } finally {
      setBusy(undefined);
    }
  };

  const saveBudget = (event: FormEvent) => {
    event.preventDefault();
    const value = Math.round(Number(budget));
    void run("budget", async () => {
      if (!Number.isFinite(value) || value < 1 || value > 200) throw new Error("Choose between 1 and 200 actions per hour.");
      const next = await api.saveSettings({ agentMaxActionsPerHour: value });
      settings.setData(next);
      return { ...data, maxActionsPerHour: next.agentMaxActionsPerHour ?? value };
    }, "Budget saved.");
  };

  const addGrant = (event: FormEvent) => {
    event.preventDefault();
    void run("grant", () => api.saveGrant({ ...grant, durationDays: duration ? Number(duration) : undefined }), "Permission saved.");
  };

  const approveWorkspace = (value: string) =>
    run(`workspace-${value}`, () => api.approveWorkspace(value), "Folder approved for checks.").then(() => setPath(""));

  return (
    <div className="permissions-grid">
      <section className="panel settings-card">
        <SettingsHeading icon={<ShieldCheck />} title="Budget" detail="A ceiling for actions that run under an automatic permission. Actions you approve one by one are not limited." />
        <form className="inline-form" onSubmit={saveBudget}>
          <label>Automatic actions per hour<input type="number" min={1} max={200} value={budget} onChange={(event) => setBudget(event.target.value)} /></label>
          <button className="ghost-button" disabled={busy === "budget"}>Save</button>
        </form>
        <p className="status-detail">{data.autoActionsLastHour} automatic action{data.autoActionsLastHour === 1 ? "" : "s"} in the last hour. When the budget is reached, Knov blocks further automatic actions until the hour passes.</p>
      </section>

      <section className="panel settings-card">
        <SettingsHeading icon={<ShieldCheck />} title="Suggestions" detail="Earned from repeated approvals with no declines, failures, or undos. Knov never widens permission by itself." />
        {data.proposals.length ? data.proposals.map((proposal) => (
          <article className="proposal-row" key={`${proposal.actionType}-${proposal.scopeValue ?? "global"}`}>
            <p>{proposal.message}</p>
            <div className="inline-actions">
              <button type="button" className="primary-button" disabled={busy === "proposal"} onClick={() => void run("proposal", () => api.respondProposal({ actionType: proposal.actionType, scopeKind: proposal.scopeKind, scopeValue: proposal.scopeValue, accept: true }), "Permission granted.")}>Allow there</button>
              <button type="button" className="ghost-button" disabled={busy === "proposal"} onClick={() => void run("proposal", () => api.respondProposal({ actionType: proposal.actionType, scopeKind: proposal.scopeKind, scopeValue: proposal.scopeValue, accept: false }))}>Keep asking</button>
            </div>
          </article>
        )) : <p className="status-detail">No suggestions right now. After five clean approvals of the same action in the same place, Knov will ask whether to allow it automatically.</p>}
      </section>

      <section className="panel settings-card full-width">
        <SettingsHeading icon={<ShieldCheck />} title="Active permissions" detail="What Knov may do without asking, where, and until when. Revoking takes effect immediately." />
        {data.grants.length ? (
          <div className="grant-table" role="table" aria-label="Active permissions">
            <div className="grant-row heading" role="row"><span>Action</span><span>Where</span><span>Mode</span><span>Granted</span><span>Expires</span><span /></div>
            {data.grants.map((item) => (
              <div className="grant-row" role="row" key={item.id}>
                <strong>{data.catalog.find((kind) => kind.actionType === item.actionType)?.title ?? item.actionType}</strong>
                <span>{item.scopeLabel}</span>
                <span className={`mode-chip ${item.mode}`}>{item.mode === "auto" ? "Automatic" : item.mode === "ask" ? "Always ask" : "Never"}</span>
                <span>{relativeTime(item.createdAt)} · {item.source === "proposal" ? "suggestion" : item.source === "approval" ? "while approving" : "by you"}</span>
                <span>{item.expiresAt ? new Date(item.expiresAt * 1000).toLocaleDateString() : "No expiry"}</span>
                <button type="button" className="ghost-button" disabled={busy === item.id} onClick={() => void run(item.id, () => api.revokeGrant(item.id), "Permission revoked.")}>Revoke</button>
              </div>
            ))}
          </div>
        ) : <p className="status-detail">No permissions yet: Knov asks before every action.</p>}
        <form className="grant-form" onSubmit={addGrant}>
          <label>Action
            <select value={grant.actionType} onChange={(event) => setGrant({ ...grant, actionType: event.target.value, scopeKind: "global", scopeValue: undefined })}>
              {available.map((kind) => <option key={kind.actionType} value={kind.actionType}>{kind.title}</option>)}
            </select>
          </label>
          <label>Where
            <select
              value={`${grant.scopeKind}:${grant.scopeValue ?? ""}`}
              onChange={(event) => {
                const [scopeKind, scopeValue] = event.target.value.split(/:(.*)/s);
                setGrant({ ...grant, scopeKind: scopeKind as GrantRequest["scopeKind"], scopeValue: scopeValue || undefined });
              }}
            >
              <option value="global:">Everywhere</option>
              {skills.data?.map((skill) => <option key={skill.id} value={`skill:${skill.id}`}>Skill: {skill.name}</option>)}
              {grant.actionType === "run_checks" && data.workspaces.map((workspace) => <option key={workspace.id} value={`workspace:${workspace.id}`}>Workspace: {workspace.label}</option>)}
            </select>
          </label>
          <label>Mode
            <select value={grant.mode} onChange={(event) => setGrant({ ...grant, mode: event.target.value as GrantRequest["mode"] })}>
              <option value="auto">Allow automatically</option>
              <option value="ask">Always ask</option>
              <option value="never">Never allow</option>
            </select>
          </label>
          <label>For
            <select value={duration} onChange={(event) => setDuration(event.target.value)}>
              <option value="">Until revoked</option>
              <option value="7">7 days</option>
              <option value="30">30 days</option>
            </select>
          </label>
          <button className="ghost-button" disabled={busy === "grant"}>Add permission</button>
        </form>
      </section>

      <section className="panel settings-card full-width">
        <SettingsHeading icon={<FolderGit2 />} title="Approved workspaces" detail="Project folders where Knov may run allow-listed test commands. No shell, no custom commands; output stays on this Mac." />
        {data.workspaces.length ? (
          <ul className="workspace-list">
            {data.workspaces.map((workspace) => (
              <li key={workspace.id}>
                <span><strong>{workspace.label}</strong><code>{workspace.path}</code></span>
                <span className="preset-list">{workspace.checkPresets.map((preset) => <code key={preset.id}>{preset.label}</code>)}</span>
                <button type="button" className="row-action" aria-label={`Remove ${workspace.label}`} disabled={busy === workspace.id} onClick={() => void run(workspace.id, () => api.removeWorkspace(workspace.id), "Workspace removed and its permissions revoked.")}><Trash2 size={15} /></button>
              </li>
            ))}
          </ul>
        ) : <p className="status-detail">No folders approved. Terminal steps in skills will simply open your terminal app.</p>}
        {data.detectedWorkspaces.length > 0 && (
          <div className="detected-workspaces">
            <strong>Folders your editors know about</strong>
            {data.detectedWorkspaces.map((workspace) => (
              <div key={workspace.path}>
                <span><strong>{workspace.label}</strong><code>{workspace.path}</code></span>
                <button type="button" className="ghost-button" disabled={busy === `workspace-${workspace.path}`} onClick={() => void approveWorkspace(workspace.path)}>Approve</button>
              </div>
            ))}
          </div>
        )}
        <form className="inline-form" onSubmit={(event) => { event.preventDefault(); void approveWorkspace(path); }}>
          <label>Folder path<input value={path} placeholder="~/code/my-project" onChange={(event) => setPath(event.target.value)} /></label>
          <button className="ghost-button" disabled={!path.trim() || busy === `workspace-${path}`}>Approve folder</button>
        </form>
      </section>

      <section className="panel settings-card full-width">
        <SettingsHeading icon={<ShieldCheck />} title="How Knov decides" detail="Risk and reversibility set the default. Model confidence never grants authority; only you do." />
        <div className="risk-table" role="table" aria-label="Action risk classes">
          <div className="risk-row heading" role="row"><span>Action</span><span>Risk class</span><span>Default</span><span>Undo</span></div>
          {data.catalog.map((kind) => (
            <div className={`risk-row${kind.available ? "" : " unavailable"}`} role="row" key={kind.actionType}>
              <span><strong>{kind.title}</strong><small>{kind.description}</small></span>
              <span className={`risk-badge ${kind.riskClass}`}>{riskLabel(kind.riskClass)}</span>
              <span>{kind.defaultPolicy}{kind.interruptsUser ? " · never unattended" : ""}</span>
              <span>{kind.rollback}</span>
            </div>
          ))}
        </div>
      </section>
      {notice && <p className={`notice ${notice.tone} full-width`} role={notice.tone === "error" ? "alert" : "status"}>{notice.text}</p>}
    </div>
  );
}

function InsightsPanel({
  goals,
  autonomy,
  onGoalsChanged,
}: {
  goals: Goal[];
  autonomy?: AutonomyOverview;
  onGoalsChanged: () => void;
}) {
  const [busy, setBusy] = useState<string>();
  const [renaming, setRenaming] = useState<string>();
  const [title, setTitle] = useState("");
  const [error, setError] = useState("");
  const metrics = autonomy?.metrics;

  const review = async (goal: Goal, status: "confirmed" | "dismissed" | "completed", newTitle?: string) => {
    setBusy(goal.id);
    setError("");
    try {
      await api.reviewGoal(goal.id, status, newTitle);
      setRenaming(undefined);
      onGoalsChanged();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(undefined);
    }
  };

  return (
    <div className="insights-grid">
      <section className="panel" aria-label="Goals">
        <h2 className="section-heading"><Target size={16} aria-hidden="true" /> Goals</h2>
        <p className="status-detail">Durable objectives inferred from threads that recur across days. Confirming one tells Knov what your work is for.</p>
        {goals.length ? goals.map((goal) => (
          <article className="goal-row" key={goal.id}>
            {renaming === goal.id ? (
              <form className="inline-rename" onSubmit={(event) => { event.preventDefault(); void review(goal, "confirmed", title); }}>
                <input aria-label="Goal name" value={title} maxLength={100} autoFocus onChange={(event) => setTitle(event.target.value)} />
                <button className="primary-button" disabled={!title.trim() || busy === goal.id}>Save</button>
                <button type="button" className="ghost-button" onClick={() => setRenaming(undefined)}>Cancel</button>
              </form>
            ) : (
              <div className="goal-row-head">
                <strong>{goal.title}</strong>
                <span className={`goal-chip ${goal.status}`}>{goal.status === "confirmed" ? "Confirmed by you" : `Inferred · ${percent(goal.confidence)}`}</span>
              </div>
            )}
            <ul>{goal.evidence.map((line) => <li key={line}>{line}</li>)}</ul>
            <div className="inline-actions">
              {goal.status !== "confirmed" && <button type="button" className="ghost-button" disabled={busy === goal.id} onClick={() => void review(goal, "confirmed")}>Confirm</button>}
              <button type="button" className="ghost-button" onClick={() => { setRenaming(goal.id); setTitle(goal.title); }}>Rename</button>
              <button type="button" className="ghost-button" disabled={busy === goal.id} onClick={() => void review(goal, "completed")}>Mark complete</button>
              <button type="button" className="ghost-button" disabled={busy === goal.id} onClick={() => void review(goal, "dismissed")}>Not a goal</button>
            </div>
          </article>
        )) : <p className="status-detail">No multi-day goals yet. Goals appear when the same thread recurs on at least two days.</p>}
        {error && <p className="notice error" role="alert">{error}</p>}
      </section>

      <section className="panel" aria-label="Outcomes">
        <h2 className="section-heading"><ShieldCheck size={16} aria-hidden="true" /> Outcomes</h2>
        {metrics ? (
          <div className="metric-tiles">
            <Metric label="Task completion" value={percent(metrics.taskCompletionRate)} detail={`${metrics.completedRuns} of ${metrics.finishedRuns} finished runs`} />
            <Metric label="Verified actions" value={percent(metrics.verificationRate)} detail={`${metrics.verifiedActions} of ${metrics.actionsExecuted} executed`} />
            <Metric label="Approval acceptance" value={percent(metrics.approvalAcceptanceRate)} detail="Approved ÷ approved + declined" />
            <Metric label="Undo rate" value={percent(metrics.rollbackRate)} detail="Lower is better" />
            <Metric label="Setup time saved" value={`~${metrics.estimatedMinutesSaved.toFixed(1)} min`} detail="Rough local estimate" />
            <Metric label="High-risk actions" value={String(metrics.highRiskActions)} detail="Must stay at zero" />
            <Metric label="Automatic permissions" value={String(metrics.activeAutoGrants)} detail="Currently active" />
            <Metric label="Actions this week" value={String(metrics.actionsLast7Days)} detail="Executed in 7 days" />
          </div>
        ) : <p className="status-detail">Loading outcomes…</p>}
      </section>

      <section className="panel full-width" aria-label="Learned work policy">
        <h2 className="section-heading"><ShieldCheck size={16} aria-hidden="true" /> What Knov has learned about your choices</h2>
        <p className="status-detail">Learned from your approvals and declines. Preferences shape suggestions; they never grant permission on their own.</p>
        {autonomy?.policy.length ? (
          <div className="policy-table" role="table" aria-label="Learned preferences">
            <div className="policy-row heading" role="row"><span>Action</span><span>Where</span><span>Approved</span><span>Declined</span><span>Automatic</span><span>Tendency</span></div>
            {autonomy.policy.map((row) => (
              <div className="policy-row" role="row" key={`${row.actionType}-${row.scopeLabel}`}>
                <strong>{row.actionTitle}</strong><span>{row.scopeLabel}</span><span>{row.approvals}</span><span>{row.rejections}</span><span>{row.automatic}</span><span className="tendency">{row.tendency}</span>
              </div>
            ))}
          </div>
        ) : <p className="status-detail">Nothing learned yet. Run a skill and approve or decline its steps.</p>}
      </section>
    </div>
  );
}

function Metric({ label, value, detail }: { label: string; value: string; detail: string }) {
  return <article className="metric-tile"><span>{label}</span><strong>{value}</strong><small>{detail}</small></article>;
}
