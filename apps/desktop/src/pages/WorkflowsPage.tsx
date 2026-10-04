import { Check, LoaderCircle, Pencil, Play, Radar, RefreshCw, Trash2, Workflow as WorkflowIcon } from "lucide-react";
import { FormEvent, useState } from "react";
import { CategoryIcon, WorkflowChain } from "../components/agent/WorkflowChain";
import { RunReview } from "../components/agent/RunReview";
import { EmptyState, errorMessage, Modal, PageHeader, ResourceState, Segmented, Toggle } from "../components/ui";
import { useResource } from "../hooks/useResource";
import { api } from "../lib/api";
import { hourLabel, minutesLabel, percent, relativeTime, triggerLabel, weekdayName } from "../lib/agentFormat";
import { formatTime } from "../lib/format";
import { useAppStatus } from "../state/AppStatus";
import type { AgentRun, ApprovedWorkspace, Skill, SkillStep, SkillStepUpdate, TriggerKind, Workflow } from "../types";

type Tab = "opportunities" | "workflows" | "skills";
const TAB_KEY = "knov.workflows-tab";

function storedTab(): Tab {
  try {
    const value = localStorage.getItem(TAB_KEY);
    return value === "workflows" || value === "skills" ? value : "opportunities";
  } catch {
    return "opportunities";
  }
}

export function WorkflowsPage() {
  const workflows = useResource(() => api.workflows(), []);
  const skills = useResource(() => api.skills(), []);
  const autonomy = useResource(() => api.autonomy(), []);
  const { refreshAgent } = useAppStatus();
  const [tab, setTabState] = useState<Tab>(storedTab);
  const [scanning, setScanning] = useState(false);
  const [notice, setNotice] = useState<{ tone: "ok" | "error"; text: string }>();
  const [editing, setEditing] = useState<Skill>();
  const [reviewing, setReviewing] = useState<AgentRun>();
  const [busyId, setBusyId] = useState<string>();

  const setTab = (value: Tab) => {
    setTabState(value);
    try {
      localStorage.setItem(TAB_KEY, value);
    } catch {
      // Remembering the tab is a convenience only.
    }
  };

  const guarded = async (key: string, work: () => Promise<void>) => {
    setBusyId(key);
    setNotice(undefined);
    try {
      await work();
    } catch (cause) {
      setNotice({ tone: "error", text: errorMessage(cause) });
    } finally {
      setBusyId(undefined);
    }
  };

  const rescan = () => guarded("rescan", async () => {
    setScanning(true);
    try {
      const next = await api.rescanWorkflows();
      workflows.setData(next);
      const active = next.filter((workflow) => workflow.status !== "dismissed" && workflow.active).length;
      setNotice({ tone: "ok", text: `Scanned the last 30 days locally: ${active} workflow${active === 1 ? "" : "s"} found.` });
      void refreshAgent();
    } finally {
      setScanning(false);
    }
  });

  const replaceWorkflow = (updated: Workflow) =>
    workflows.setData((current) => current?.map((workflow) => workflow.id === updated.id ? updated : workflow));

  const review = (workflow: Workflow, status: Workflow["status"], title?: string) => guarded(workflow.id, async () => {
    replaceWorkflow(await api.reviewWorkflow(workflow.id, status, title));
    void refreshAgent();
  });

  const createSkill = (workflow: Workflow) => guarded(workflow.id, async () => {
    const skill = await api.createSkill(workflow.id);
    skills.setData((current) => [skill, ...(current ?? []).filter((item) => item.id !== skill.id)]);
    replaceWorkflow({ ...workflow, status: "confirmed", skillId: skill.id });
    setTab("skills");
    setEditing(skill);
    void refreshAgent();
  });

  const runSkill = (skill: Skill) => guarded(skill.id, async () => {
    setReviewing(await api.previewSkillRun(skill.id));
  });

  const saveSkill = (skill: Skill) =>
    skills.setData((current) => current?.map((item) => item.id === skill.id ? skill : item));

  const toggleSkill = (skill: Skill, enabled: boolean) => guarded(skill.id, async () => {
    saveSkill(await api.updateSkill({
      id: skill.id,
      name: skill.name,
      trigger: skill.trigger,
      onException: skill.onException,
      enabled,
      steps: skill.steps.filter((step) => step.id !== "brief").map(stepUpdate),
      includeBrief: skill.steps.some((step) => step.id === "brief" && step.enabled),
    }));
  });

  const deleteSkill = (skill: Skill) => guarded(skill.id, async () => {
    await api.deleteSkill(skill.id);
    skills.setData((current) => current?.filter((item) => item.id !== skill.id));
    workflows.setData((current) => current?.map((workflow) => workflow.skillId === skill.id ? { ...workflow, skillId: undefined } : workflow));
    setNotice({ tone: "ok", text: `Deleted “${skill.name}” and revoked its permissions.` });
  });

  const all = workflows.data ?? [];
  const visible = all.filter((workflow) => workflow.status !== "dismissed");
  const opportunities = visible.filter((workflow) => workflow.opportunity.surfaced);
  const minutes = opportunities.reduce((sum, workflow) => sum + workflow.opportunity.estimatedMinutesSavedPerWeek, 0);

  return (
    <div className="page workflows-page">
      <PageHeader
        eyebrow="Learned from your activity"
        title="Workflows"
        description="Repeated work Knov noticed on this Mac. Confirm the real ones, then turn them into skills that prepare or run safe steps for you."
        actions={
          <button type="button" className="ghost-button" disabled={scanning} onClick={() => void rescan()}>
            {scanning ? <LoaderCircle size={15} className="spin" /> : <RefreshCw size={15} />} Rescan
          </button>
        }
      />
      <div className="summary-strip" aria-label="Workflow summary">
        <span><strong>{visible.filter((workflow) => workflow.active).length}</strong> workflows</span>
        <span><strong>{opportunities.length}</strong> automation opportunities</span>
        <span><strong>{skills.data?.length ?? 0}</strong> skills</span>
        <span><strong>~{minutes.toFixed(1)} min</strong> estimated setup saved per week</span>
        <span className="summary-boundary">Mined locally from app names, sites, and timing. No provider involved.</span>
      </div>
      {notice && <p className={`notice ${notice.tone}`} role={notice.tone === "error" ? "alert" : "status"}>{notice.text}</p>}

      <Segmented
        label="Workflow views"
        value={tab}
        onChange={setTab}
        options={[
          { value: "opportunities", label: "Opportunities", count: opportunities.length },
          { value: "workflows", label: "All workflows", count: visible.length },
          { value: "skills", label: "Skills", count: skills.data?.length ?? 0 },
        ]}
      />

      <div className="tab-body">
        {tab !== "skills" && (
          <ResourceState {...workflows}>
            {() => {
              const list = tab === "opportunities" ? opportunities : [...visible, ...all.filter((workflow) => workflow.status === "dismissed")];
              if (!list.length) {
                return tab === "opportunities" ? (
                  <EmptyState
                    icon={<Radar size={22} />}
                    title="No automation opportunities yet"
                    detail="Knov suggests automation only for workflows it has seen at least four times and can prepare safely. Confirming workflows you recognize helps."
                    action={visible.length ? <button type="button" className="ghost-button" onClick={() => setTab("workflows")}>Review {visible.length} workflow{visible.length === 1 ? "" : "s"}</button> : undefined}
                  />
                ) : (
                  <EmptyState
                    icon={<WorkflowIcon size={22} />}
                    title="No repeated workflows yet"
                    detail="A workflow appears once the same three or more steps happen in order at least three times on two or more days. Keep collection on and rescan later."
                    action={<button type="button" className="ghost-button" onClick={() => void rescan()}>Rescan now</button>}
                  />
                );
              }
              return (
                <div className="workflow-list">
                  {list.map((workflow) => (
                    <WorkflowCard
                      key={workflow.id}
                      workflow={workflow}
                      busy={busyId === workflow.id}
                      emphasizeScore={tab === "opportunities"}
                      onReview={(status, title) => void review(workflow, status, title)}
                      onCreateSkill={() => void createSkill(workflow)}
                      onOpenSkill={() => {
                        setTab("skills");
                        const skill = skills.data?.find((item) => item.id === workflow.skillId);
                        if (skill) setEditing(skill);
                      }}
                    />
                  ))}
                </div>
              );
            }}
          </ResourceState>
        )}
        {tab === "skills" && (
          <ResourceState {...skills}>
            {(list) => list.length ? (
              <div className="skill-list">
                {list.map((skill) => (
                  <SkillCard
                    key={skill.id}
                    skill={skill}
                    busy={busyId === skill.id}
                    onRun={() => void runSkill(skill)}
                    onEdit={() => setEditing(skill)}
                    onToggle={(enabled) => void toggleSkill(skill, enabled)}
                    onDelete={() => void deleteSkill(skill)}
                  />
                ))}
              </div>
            ) : (
              <EmptyState
                icon={<Play size={22} />}
                title="No skills yet"
                detail="A skill is a confirmed workflow Knov can prepare for you. Create one from a workflow; every action still waits for your approval until you allow otherwise."
                action={<button type="button" className="ghost-button" onClick={() => setTab("workflows")}>Browse workflows</button>}
              />
            )}
          </ResourceState>
        )}
      </div>

      {editing && (
        <SkillEditor
          skill={editing}
          workspaces={autonomy.data?.workspaces ?? []}
          onClose={() => setEditing(undefined)}
          onSaved={(skill) => {
            saveSkill(skill);
            setEditing(undefined);
            setNotice({ tone: "ok", text: `Saved “${skill.name}”.` });
          }}
        />
      )}
      {reviewing && (
        <RunReview
          run={reviewing}
          onClose={() => {
            setReviewing(undefined);
            void skills.reload();
            void refreshAgent();
          }}
        />
      )}
    </div>
  );
}

function stepUpdate(step: SkillStep): SkillStepUpdate {
  return {
    id: step.id,
    enabled: step.enabled,
    workspaceId: step.action?.type === "run_checks" ? step.action.workspaceId : undefined,
    checkPreset: step.action?.type === "run_checks" ? step.action.preset : undefined,
  };
}

function statusLabel(status: Workflow["status"]): string {
  if (status === "confirmed") return "Confirmed by you";
  if (status === "dismissed") return "Not a workflow";
  return "Discovered";
}

function WorkflowCard({
  workflow,
  busy,
  emphasizeScore,
  onReview,
  onCreateSkill,
  onOpenSkill,
}: {
  workflow: Workflow;
  busy: boolean;
  emphasizeScore: boolean;
  onReview: (status: Workflow["status"], title?: string) => void;
  onCreateSkill: () => void;
  onOpenSkill: () => void;
}) {
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState(workflow.title);
  const { stats, opportunity } = workflow;
  const rhythm = [
    stats.typicalWeekday && `usually ${stats.typicalWeekday}s`,
    stats.typicalHour !== undefined && `around ${hourLabel(stats.typicalHour)}`,
  ].filter(Boolean).join(" ");

  const submitTitle = (event: FormEvent) => {
    event.preventDefault();
    onReview(workflow.status, title);
    setRenaming(false);
  };

  return (
    <article className={`workflow-card ${workflow.status}${workflow.active ? "" : " inactive"}`} aria-label={workflow.title}>
      <div className="workflow-card-main">
        <div className="workflow-card-heading">
          <div className="chip-row">
            <span className={`status-chip ${workflow.status}`}>{statusLabel(workflow.status)}</span>
            {stats.thread && <span className="thread-chip">{stats.thread}</span>}
            {!workflow.active && <span className="status-chip dismissed">Not seen recently</span>}
          </div>
          {renaming ? (
            <form className="inline-rename" onSubmit={submitTitle}>
              <input aria-label="Workflow name" value={title} maxLength={80} autoFocus onChange={(event) => setTitle(event.target.value)} />
              <button className="primary-button" disabled={busy || !title.trim()}>Save</button>
              <button type="button" className="ghost-button" onClick={() => { setTitle(workflow.title); setRenaming(false); }}>Cancel</button>
            </form>
          ) : (
            <h3>{workflow.title}</h3>
          )}
          <p className="workflow-stats">
            Seen {stats.occurrences}× on {stats.distinctDays} days · about {stats.perWeek.toFixed(1)} per week · takes {minutesLabel(stats.averageDurationSeconds)} · finished {percent(stats.completionRate)} of the times it started{rhythm ? ` · ${rhythm}` : ""}
          </p>
        </div>
        <WorkflowChain steps={workflow.steps} />
        <details className="workflow-why">
          <summary>Why Knov thinks this is a workflow</summary>
          <ul>{opportunity.rationale.map((line) => <li key={line}>{line}</li>)}</ul>
          {workflow.evidence.length > 0 && (
            <>
              <h4>Recent occurrences</h4>
              <ul className="occurrence-list">
                {workflow.evidence.map((occurrence) => (
                  <li key={occurrence.startedAt}>
                    <time>{new Date(occurrence.startedAt * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric" })} · {formatTime(new Date(occurrence.startedAt * 1000).toISOString())}</time>
                    <span>{occurrence.details.join(" → ")}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
          <small>Evidence lists apps, sites, and page paths only. Titles and page contents are never used.</small>
        </details>
        <div className="workflow-actions">
          {workflow.status === "discovered" && (
            <>
              <button type="button" className="primary-button" disabled={busy} onClick={onCreateSkill}><Check size={15} /> Confirm and create skill</button>
              <button type="button" className="ghost-button" disabled={busy} onClick={() => onReview("confirmed")}>Yes, this is a workflow</button>
              <button type="button" className="ghost-button" disabled={busy} onClick={() => onReview("dismissed")}>Not a workflow</button>
            </>
          )}
          {workflow.status === "confirmed" && (workflow.skillId
            ? <button type="button" className="ghost-button" onClick={onOpenSkill}>Open skill</button>
            : <button type="button" className="primary-button" disabled={busy} onClick={onCreateSkill}>Create skill</button>)}
          {workflow.status === "dismissed" && <button type="button" className="ghost-button" disabled={busy} onClick={() => onReview("discovered")}>Restore</button>}
          {!renaming && <button type="button" className="ghost-button" onClick={() => setRenaming(true)}><Pencil size={14} /> Rename</button>}
        </div>
      </div>
      <OpportunityMeter workflow={workflow} emphasize={emphasizeScore} />
    </article>
  );
}

function OpportunityMeter({ workflow, emphasize }: { workflow: Workflow; emphasize: boolean }) {
  const { opportunity } = workflow;
  const factors: [string, number, boolean][] = [
    ["Frequency", opportunity.frequency, false],
    ["Time spent", opportunity.timeCost, false],
    ["Stability", opportunity.stability, false],
    ["Knov can help", opportunity.executability, false],
    ["Risk", opportunity.risk, true],
  ];
  return (
    <aside className={`opportunity-meter${emphasize || opportunity.surfaced ? " surfaced" : ""}`} aria-label={`Opportunity score ${percent(opportunity.score)}`}>
      <div className="opportunity-score"><strong>{percent(opportunity.score)}</strong><span>{opportunity.surfaced ? "automation opportunity" : "opportunity score"}</span></div>
      <dl>
        {factors.map(([label, value, inverse]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd><span className={`factor-bar${inverse ? " inverse" : ""}`}><i style={{ width: `${Math.round(value * 100)}%` }} /></span></dd>
          </div>
        ))}
      </dl>
      <small>
        {opportunity.preparableSteps + opportunity.executableSteps > 0
          ? `~${opportunity.estimatedMinutesSavedPerWeek.toFixed(1)} min/week of setup (rough local estimate)`
          : "No safe action for these steps yet"}
      </small>
    </aside>
  );
}

function actionLabel(step: SkillStep): string {
  switch (step.action?.type) {
    case "open_url": return "Knov opens it";
    case "open_application": return "Knov opens it";
    case "write_draft": return "Knov drafts it";
    case "run_checks": return "Knov runs it";
    default: return "You do this";
  }
}

function SkillCard({
  skill,
  busy,
  onRun,
  onEdit,
  onToggle,
  onDelete,
}: {
  skill: Skill;
  busy: boolean;
  onRun: () => void;
  onEdit: () => void;
  onToggle: (enabled: boolean) => void;
  onDelete: () => void;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const { stats } = skill;
  return (
    <article className={`skill-card${skill.enabled ? "" : " off"}`} aria-label={skill.name}>
      <header>
        <div>
          <h3>{skill.name}</h3>
          <p>{triggerLabel(skill.trigger)} · {skill.onException === "stop" ? "stops at the first problem" : "continues past problems"}</p>
        </div>
        <Toggle label="Enabled" detail="Triggers and runs" checked={skill.enabled} disabled={busy} onChange={onToggle} />
      </header>
      <ol className="skill-steps">
        {skill.steps.map((step) => (
          <li key={step.id} className={step.enabled ? "" : "off"}>
            <CategoryIcon category={step.category} />
            <span>{step.title}</span>
            <small className={`step-kind ${step.action ? "agent" : "manual"}`}>{step.enabled ? actionLabel(step) : "Turned off"}</small>
          </li>
        ))}
      </ol>
      <p className="skill-stats">
        {stats.runs ? `${stats.runs} runs · ${stats.completed} done · ${stats.needsAttention} needed a look · ${stats.failed} failed${stats.rolledBack ? ` · ${stats.rolledBack} undone` : ""}` : "Not run yet"}
        {stats.lastRunAt ? ` · last ${relativeTime(stats.lastRunAt)}` : ""}
      </p>
      <footer>
        <button type="button" className="primary-button" disabled={busy} onClick={onRun}>{busy ? <LoaderCircle size={15} className="spin" /> : <Play size={15} />} Run now</button>
        <button type="button" className="ghost-button" onClick={onEdit}><Pencil size={14} /> Edit</button>
        {confirmDelete ? (
          <span className="confirm-inline">
            Delete this skill and its permissions?
            <button type="button" className="danger-button" disabled={busy} onClick={onDelete}>Delete</button>
            <button type="button" className="ghost-button" onClick={() => setConfirmDelete(false)}>Keep</button>
          </span>
        ) : (
          <button type="button" className="ghost-button" onClick={() => setConfirmDelete(true)}><Trash2 size={14} /> Delete</button>
        )}
      </footer>
    </article>
  );
}

function SkillEditor({
  skill,
  workspaces,
  onClose,
  onSaved,
}: {
  skill: Skill;
  workspaces: ApprovedWorkspace[];
  onClose: () => void;
  onSaved: (skill: Skill) => void;
}) {
  const [name, setName] = useState(skill.name);
  const [triggerKind, setTriggerKind] = useState<TriggerKind>(skill.trigger.kind);
  const [weekday, setWeekday] = useState<string>(skill.trigger.weekday === undefined ? "" : String(skill.trigger.weekday));
  const [hour, setHour] = useState(skill.trigger.hour ?? 9);
  const [onException, setOnException] = useState(skill.onException);
  const [enabled, setEnabled] = useState(skill.enabled);
  const [steps, setSteps] = useState(() => skill.steps.filter((step) => step.id !== "brief").map(stepUpdate));
  const [includeBrief, setIncludeBrief] = useState(skill.steps.some((step) => step.id === "brief" && step.enabled));
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  const patchStep = (id: string, patch: Partial<SkillStepUpdate>) =>
    setSteps((current) => current.map((step) => step.id === id ? { ...step, ...patch } : step));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError("");
    try {
      onSaved(await api.updateSkill({
        id: skill.id,
        name,
        trigger: triggerKind === "schedule"
          ? { kind: "schedule", hour, weekday: weekday === "" ? undefined : Number(weekday) }
          : { kind: triggerKind },
        onException,
        enabled,
        steps,
        includeBrief,
      }));
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal title={`Edit skill: ${skill.name}`} onClose={onClose} wide>
      <form className="stack-form skill-editor" onSubmit={(event) => void submit(event)}>
        <label>Name<input value={name} maxLength={80} required onChange={(event) => setName(event.target.value)} /></label>
        <fieldset>
          <legend>When should it start?</legend>
          <div className="choice-row">
            <label><input type="radio" name="trigger" checked={triggerKind === "manual"} onChange={() => setTriggerKind("manual")} /> Only when I run it</label>
            {skill.workflowId && <label><input type="radio" name="trigger" checked={triggerKind === "context"} onChange={() => setTriggerKind("context")} /> When I start this workflow</label>}
            <label><input type="radio" name="trigger" checked={triggerKind === "schedule"} onChange={() => setTriggerKind("schedule")} /> On a schedule</label>
          </div>
          {triggerKind === "schedule" && (
            <div className="choice-row">
              <label>Day
                <select value={weekday} onChange={(event) => setWeekday(event.target.value)}>
                  <option value="">Every day</option>
                  {[0, 1, 2, 3, 4, 5, 6].map((day) => <option key={day} value={day}>{weekdayName(day)}</option>)}
                </select>
              </label>
              <label>Time
                <select value={hour} onChange={(event) => setHour(Number(event.target.value))}>
                  {Array.from({ length: 24 }, (_, value) => <option key={value} value={value}>{hourLabel(value)}</option>)}
                </select>
              </label>
            </div>
          )}
          <p className="field-hint">Background runs only do what you allowed automatically, and never open windows or apps on their own. Everything else waits in “Ready for you”.</p>
        </fieldset>
        <fieldset>
          <legend>If a step needs attention</legend>
          <div className="choice-row">
            <label><input type="radio" name="exception" checked={onException === "stop"} onChange={() => setOnException("stop")} /> Stop and report</label>
            <label><input type="radio" name="exception" checked={onException === "continue"} onChange={() => setOnException("continue")} /> Continue with the rest</label>
          </div>
        </fieldset>
        <fieldset>
          <legend>Steps</legend>
          <ol className="editor-steps">
            {skill.steps.filter((step) => step.id !== "brief").map((step) => {
              const value = steps.find((candidate) => candidate.id === step.id)!;
              return (
                <li key={step.id}>
                  <label className="editor-step-toggle">
                    <input type="checkbox" checked={value.enabled} onChange={(event) => patchStep(step.id, { enabled: event.target.checked })} />
                    <CategoryIcon category={step.category} /> {step.title}
                    <small>{actionLabel(step)}</small>
                  </label>
                  {step.category === "terminal" && (
                    <div className="choice-row">
                      <label>For this step
                        <select
                          value={value.workspaceId ?? ""}
                          onChange={(event) => {
                            const workspace = workspaces.find((candidate) => candidate.id === event.target.value);
                            patchStep(step.id, { workspaceId: workspace?.id, checkPreset: workspace?.checkPresets[0]?.id });
                          }}
                        >
                          <option value="">Open the terminal app</option>
                          {workspaces.map((workspace) => <option key={workspace.id} value={workspace.id}>Run checks in {workspace.label}</option>)}
                        </select>
                      </label>
                      {value.workspaceId && (
                        <label>Check
                          <select value={value.checkPreset ?? ""} onChange={(event) => patchStep(step.id, { checkPreset: event.target.value })}>
                            {workspaces.find((workspace) => workspace.id === value.workspaceId)?.checkPresets.map((preset) => <option key={preset.id} value={preset.id}>{preset.label}</option>)}
                          </select>
                        </label>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ol>
          {workspaces.length === 0 && skill.steps.some((step) => step.category === "terminal") && (
            <p className="field-hint">Approve a project folder under Delegated work → Permissions to let Knov run its tests for this step.</p>
          )}
          <label className="editor-step-toggle">
            <input type="checkbox" checked={includeBrief} onChange={(event) => setIncludeBrief(event.target.checked)} />
            <CategoryIcon category="notes" /> Save a resume brief to Drafts at the end
            <small>Local Markdown, undoable</small>
          </label>
        </fieldset>
        <Toggle label="Skill enabled" detail="Turned-off skills never trigger." checked={enabled} onChange={setEnabled} />
        {error && <p className="notice error" role="alert">{error}</p>}
        <div className="modal-actions">
          <button type="button" className="ghost-button" onClick={onClose}>Cancel</button>
          <button className="primary-button" disabled={saving || !name.trim()}>{saving && <LoaderCircle size={15} className="spin" />} Save skill</button>
        </div>
      </form>
    </Modal>
  );
}
