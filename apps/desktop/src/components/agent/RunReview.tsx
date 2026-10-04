import { Ban, CircleCheck, FileText, LoaderCircle, ShieldCheck, TriangleAlert, Undo2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "../../lib/api";
import {
  actionStatus,
  decisionLabel,
  originLabel,
  relativeTime,
  riskLabel,
  runStatus,
} from "../../lib/agentFormat";
import type { AgentAction, AgentRun } from "../../types";
import { errorMessage, Modal } from "../ui";

const POLL_MS = 1_200;

export function RunReview({
  run: initial,
  onClose,
  onChanged,
}: {
  run: AgentRun;
  onClose: () => void;
  onChanged?: (run: AgentRun) => void;
}) {
  const [run, setRun] = useState(initial);
  const [selection, setSelection] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(initial.actions.filter((action) => action.status === "awaiting_approval").map((action) => [action.id, true])),
  );
  const [remember, setRemember] = useState<Record<string, boolean>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const decided = useRef(false);
  const onChangedRef = useRef(onChanged);
  onChangedRef.current = onChanged;

  const awaiting = run.actions.filter((action) => action.status === "awaiting_approval");
  const selectedCount = awaiting.filter((action) => selection[action.id]).length;
  const inFlight = run.status === "running" || run.status === "ready";

  useEffect(() => {
    if (!inFlight) return;
    const timer = window.setTimeout(async () => {
      try {
        const next = await api.agentRun(run.id);
        setRun(next);
        onChangedRef.current?.(next);
      } catch {
        // Try again on the next tick.
        setRun((current) => ({ ...current }));
      }
    }, POLL_MS);
    return () => window.clearTimeout(timer);
  }, [run, inFlight]);

  const update = (next: AgentRun) => {
    setRun(next);
    onChangedRef.current?.(next);
  };

  const decide = async (approveSelected: boolean) => {
    setBusy(true);
    setError("");
    try {
      const next = await api.decideRun(
        run.id,
        awaiting.map((action) => ({
          actionId: action.id,
          approved: approveSelected && Boolean(selection[action.id]),
          remember: approveSelected && Boolean(selection[action.id]) && Boolean(remember[action.id]),
        })),
      );
      decided.current = true;
      update(next);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  const close = async () => {
    // A manual preview nobody acted on is closed so it does not linger as
    // pending work. Closing is not counted as declining.
    if (!decided.current && run.origin === "manual" && run.status === "awaiting_approval") {
      try {
        // Not `onChanged?.(await …)`: optional calls skip evaluating their
        // arguments, which would silently skip the cancellation.
        const cancelled = await api.cancelRun(run.id);
        onChangedRef.current?.(cancelled);
      } catch {
        // The run stays visible in Delegated work and can be closed there.
      }
    }
    onClose();
  };

  const undo = async (action: AgentAction) => {
    setError("");
    try {
      update(await api.rollbackAction(action.id));
    } catch (cause) {
      setError(errorMessage(cause));
    }
  };

  const openDraft = async (action: AgentAction) => {
    setError("");
    try {
      await api.openDraft(action.id);
    } catch (cause) {
      setError(errorMessage(cause));
    }
  };

  const status = runStatus(run.status);
  return (
    <Modal title={run.title} onClose={() => void close()} wide>
      <div className="run-review">
        <div className="run-review-meta">
          <span className={`tone-pill ${status.tone}`}>{inFlight && <LoaderCircle size={12} className="spin" />}{status.label}</span>
          <span>{originLabel(run.origin)}</span>
          <span>{relativeTime(run.createdAt)}</span>
          <span>{run.onException === "stop" ? "Stops at the first problem" : "Continues past problems"}</span>
        </div>

        <section className="belief-card" aria-label="What Knov believed">
          <strong>What Knov believed when it planned this</strong>
          <dl>
            <div><dt>Thread</dt><dd>{run.state.thread ?? "Not identified"}</dd></div>
            <div><dt>Goal</dt><dd>{run.state.goal ?? "No goal inferred yet"}</dd></div>
            <div><dt>Workflow</dt><dd>{run.state.workflow ?? "Not mid-workflow"}</dd></div>
            {run.state.recentSteps.length > 0 && <div><dt>Recent steps</dt><dd>{run.state.recentSteps.join(" → ")}</dd></div>}
          </dl>
        </section>

        <ol className="action-review-list" aria-label="Planned actions">
          {run.actions.map((action) => (
            <ActionReview
              key={action.id}
              action={action}
              selected={Boolean(selection[action.id])}
              remember={Boolean(remember[action.id])}
              onSelect={(value) => setSelection((current) => ({ ...current, [action.id]: value }))}
              onRemember={(value) => setRemember((current) => ({ ...current, [action.id]: value }))}
              onUndo={() => void undo(action)}
              onOpen={() => void openDraft(action)}
            />
          ))}
        </ol>

        {run.manualSteps.length > 0 && (
          <div className="manual-steps">
            <strong>Left for you</strong>
            <ul>{run.manualSteps.map((step) => <li key={step}>{step}</li>)}</ul>
          </div>
        )}

        {error && <p className="notice error" role="alert">{error}</p>}

        <footer className="modal-actions">
          {awaiting.length > 0 ? (
            <>
              <button type="button" className="ghost-button" disabled={busy} onClick={() => void decide(false)}>Decline all</button>
              <button type="button" className="primary-button" disabled={busy || selectedCount === 0} onClick={() => void decide(true)}>
                {busy ? <LoaderCircle size={15} className="spin" /> : <ShieldCheck size={15} />}
                Approve and run {selectedCount} {selectedCount === 1 ? "action" : "actions"}
              </button>
            </>
          ) : (
            <button type="button" className="primary-button" onClick={() => void close()}>{inFlight ? "Keep running in background" : "Done"}</button>
          )}
        </footer>
      </div>
    </Modal>
  );
}

function ActionReview({
  action,
  selected,
  remember,
  onSelect,
  onRemember,
  onUndo,
  onOpen,
}: {
  action: AgentAction;
  selected: boolean;
  remember: boolean;
  onSelect: (value: boolean) => void;
  onRemember: (value: boolean) => void;
  onUndo: () => void;
  onOpen: () => void;
}) {
  const awaiting = action.status === "awaiting_approval";
  const status = actionStatus(action.status);
  return (
    <li className={`action-review tone-${status.tone}`}>
      <div className="action-review-head">
        {awaiting ? (
          <input type="checkbox" checked={selected} aria-label={`Approve ${action.title}`} onChange={(event) => onSelect(event.target.checked)} />
        ) : (
          <span className={`action-status-icon ${status.tone}`} aria-hidden="true">
            {status.tone === "ok" ? <CircleCheck size={16} /> : status.tone === "danger" ? <Ban size={16} /> : status.tone === "warn" ? <TriangleAlert size={16} /> : action.status === "running" ? <LoaderCircle size={16} className="spin" /> : <CircleCheck size={16} />}
          </span>
        )}
        <div className="action-review-title">
          <strong>{action.title}</strong>
          <small>{action.targetLabel}</small>
        </div>
        <span className={`risk-badge ${action.riskClass}`}>{riskLabel(action.riskClass)}</span>
        <span className={`tone-pill ${status.tone}`}>{status.label}</span>
      </div>
      <p className="action-why">{action.rationale}</p>
      <p className="action-permission"><ShieldCheck size={13} aria-hidden="true" /> <span><strong>{decisionLabel(action)}.</strong> {action.decisionReason}</span></p>
      {awaiting && action.decision !== "auto" && (
        <label className="remember-choice">
          <input type="checkbox" checked={remember} disabled={!selected} onChange={(event) => onRemember(event.target.checked)} />
          Allow this automatically for {action.scopeLabel} from now on
        </label>
      )}
      {action.actionType === "write_draft" && action.preview && (
        <details className="action-detail"><summary>Preview the draft</summary><pre>{action.preview}</pre></details>
      )}
      {action.actionType === "open_url" && action.preview && awaiting && (
        <p className="action-target"><code>{action.preview}</code></p>
      )}
      {action.resultSummary && <p className="action-result">{action.resultSummary}</p>}
      {action.verification && (
        <ul className={`verification-list${action.verification.passed ? "" : " failed"}`} aria-label="Verification">
          {action.verification.checks.map((check) => <li key={check}>{action.verification?.passed ? <CircleCheck size={13} aria-hidden="true" /> : <TriangleAlert size={13} aria-hidden="true" />}{check}</li>)}
        </ul>
      )}
      {action.outputExcerpt && (
        <details className="action-detail" open={action.status === "needs_attention"}>
          <summary>Command output · stays on this Mac</summary>
          <pre>{action.outputExcerpt}</pre>
        </details>
      )}
      {(action.canOpen || action.rollbackAvailable) && (
        <div className="action-review-buttons">
          {action.canOpen && <button type="button" className="ghost-button" onClick={onOpen}><FileText size={14} /> Open draft</button>}
          {action.rollbackAvailable && <button type="button" className="ghost-button" onClick={onUndo}><Undo2 size={14} /> Undo</button>}
        </div>
      )}
    </li>
  );
}
