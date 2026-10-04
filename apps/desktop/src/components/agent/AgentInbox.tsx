import { ChevronRight, CirclePause, Inbox, ShieldCheck, Target, Workflow as WorkflowIcon, X } from "lucide-react";
import { useState } from "react";
import { api } from "../../lib/api";
import { isTerminalRun, percent, relativeTime, runStatus } from "../../lib/agentFormat";
import { useAppStatus } from "../../state/AppStatus";
import type { AgentRun, AutonomyProposal, Goal, WorkflowProgress } from "../../types";
import { errorMessage } from "../ui";
import { CategoryIcon } from "./WorkflowChain";
import { RunReview } from "./RunReview";

/** The agent's slice of Now: one goal line and at most a few decisions. */
export function AgentInbox() {
  const { agent, refreshAgent, setAgentPaused } = useAppStatus();
  const [reviewing, setReviewing] = useState<AgentRun>();
  const [pending, setPending] = useState<string>();
  const [error, setError] = useState("");
  const data = agent.data;
  if (!data) return null;

  const { goal, workflowProgress: progress } = data.state;
  const runs = data.attention.slice(0, 3);
  const progressCovered = progress?.skillId
    ? runs.some((run) => run.skillId === progress.skillId && !isTerminalRun(run.status))
    : false;
  const proposal = data.proposals[0];
  const showProgress = progress && !progressCovered;
  if (!goal && !runs.length && !showProgress && !proposal && !data.paused) return null;

  const act = async (key: string, work: () => Promise<unknown>) => {
    setPending(key);
    setError("");
    try {
      await work();
      await refreshAgent();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setPending(undefined);
    }
  };

  const stage = (value: WorkflowProgress) => act("stage", async () => {
    if (!value.skillId) return;
    setReviewing(await api.previewSkillRun(value.skillId));
  });

  return (
    <section className="agent-inbox" aria-label="Ready for you">
      <div className="agent-inbox-head">
        <span><Inbox size={15} aria-hidden="true" /> Ready for you</span>
        <a href="#/agent">Delegated work <ChevronRight size={14} aria-hidden="true" /></a>
      </div>

      {goal && (
        <GoalLine
          goal={goal}
          busy={pending === "goal"}
          onReview={(status) => act("goal", () => api.reviewGoal(goal.id, status))}
        />
      )}

      <div className="inbox-items">
        {data.paused && (
          <article className="inbox-item warn">
            <span className="inbox-icon"><CirclePause size={16} aria-hidden="true" /></span>
            <div><strong>Agent execution is paused</strong><small>Nothing runs, even with permission, until you resume. Collection is unaffected.</small></div>
            <div className="inbox-actions">
              <button type="button" className="ghost-button" disabled={pending === "resume"} onClick={() => void act("resume", () => setAgentPaused(false))}>Resume agent</button>
            </div>
          </article>
        )}
        {runs.map((run) => <InboxRun key={run.id} run={run} busy={pending === run.id} onReview={() => setReviewing(run)} onDismiss={() => void act(run.id, () => api.acknowledgeRun(run.id))} />)}
        {showProgress && progress && (
          <article className="inbox-item info">
            <span className="inbox-icon"><WorkflowIcon size={16} aria-hidden="true" /></span>
            <div>
              <strong><CategoryIcon category={progress.nextStep.category} /> Next: {progress.nextStep.title}</strong>
              <small>Step {progress.matchedSteps} of {progress.totalSteps} in “{progress.title}” · continues this way {percent(progress.confidence)} of the time</small>
            </div>
            <div className="inbox-actions">
              {progress.skillId
                ? <button type="button" className="primary-button" disabled={pending === "stage"} onClick={() => void stage(progress)}>Stage next steps</button>
                : <a className="ghost-button" href="#/workflows">Make it a skill</a>}
            </div>
          </article>
        )}
        {proposal && <InboxProposal proposal={proposal} busy={pending === "proposal"} onRespond={(accept) => void act("proposal", () => api.respondProposal({ actionType: proposal.actionType, scopeKind: proposal.scopeKind, scopeValue: proposal.scopeValue, accept }))} />}
      </div>
      {error && <p className="notice error" role="alert">{error}</p>}
      {reviewing && (
        <RunReview
          run={reviewing}
          onClose={() => {
            setReviewing(undefined);
            void refreshAgent();
          }}
        />
      )}
    </section>
  );
}

function GoalLine({
  goal,
  busy,
  onReview,
}: {
  goal: Goal;
  busy: boolean;
  onReview: (status: "confirmed" | "dismissed") => void;
}) {
  return (
    <div className="goal-line">
      <Target size={16} aria-hidden="true" />
      <span className="goal-copy"><small>Current goal</small><strong>{goal.title}</strong></span>
      <span className={`goal-chip ${goal.status}`}>{goal.status === "confirmed" ? "Confirmed by you" : `Inferred · ${percent(goal.confidence)}`}</span>
      <details className="goal-why">
        <summary>Why?</summary>
        <ul>{goal.evidence.map((line) => <li key={line}>{line}</li>)}</ul>
      </details>
      <span className="goal-actions">
        {goal.status !== "confirmed" && <button type="button" className="feedback-button" disabled={busy} onClick={() => onReview("confirmed")}>Confirm goal</button>}
        <button type="button" className="feedback-button" disabled={busy} onClick={() => onReview("dismissed")}>Not a goal</button>
      </span>
    </div>
  );
}

function InboxRun({ run, busy, onReview, onDismiss }: { run: AgentRun; busy: boolean; onReview: () => void; onDismiss: () => void }) {
  const status = runStatus(run.status);
  const terminal = isTerminalRun(run.status);
  return (
    <article className={`inbox-item ${status.tone}`}>
      <span className="inbox-icon"><ShieldCheck size={16} aria-hidden="true" /></span>
      <div>
        <strong>{run.title}</strong>
        <small>{status.label} · {run.summary} · {relativeTime(run.createdAt)}</small>
      </div>
      <div className="inbox-actions">
        <button type="button" className={run.status === "awaiting_approval" ? "primary-button" : "ghost-button"} onClick={onReview}>
          {run.status === "awaiting_approval" ? "Review" : "View"}
        </button>
        {terminal && <button type="button" className="icon-button" disabled={busy} aria-label={`Dismiss ${run.title}`} onClick={onDismiss}><X size={15} /></button>}
      </div>
    </article>
  );
}

function InboxProposal({ proposal, busy, onRespond }: { proposal: AutonomyProposal; busy: boolean; onRespond: (accept: boolean) => void }) {
  return (
    <article className="inbox-item proposal">
      <span className="inbox-icon"><ShieldCheck size={16} aria-hidden="true" /></span>
      <div><strong>Permission suggestion</strong><small>{proposal.message}</small></div>
      <div className="inbox-actions">
        <button type="button" className="primary-button" disabled={busy} onClick={() => onRespond(true)}>Allow</button>
        <button type="button" className="ghost-button" disabled={busy} onClick={() => onRespond(false)}>Keep asking</button>
      </div>
    </article>
  );
}
