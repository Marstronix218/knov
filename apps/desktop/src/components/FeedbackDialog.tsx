import { Check, Copy, LoaderCircle, Send } from "lucide-react";
import { useState } from "react";
import { FEEDBACK_EMAIL, FEEDBACK_FORM_URL, REPOSITORY_URL } from "../config";
import { useResource } from "../hooks/useResource";
import { api } from "../lib/api";
import type { TesterSummary } from "../types";
import { errorMessage, Modal } from "./ui";

const disappointment = [
  { value: "very", label: "Very disappointed" },
  { value: "somewhat", label: "Somewhat disappointed" },
  { value: "not", label: "Not disappointed" },
] as const;

type Disappointment = (typeof disappointment)[number]["value"];

function summaryLines(summary: TesterSummary): string[] {
  const events = Object.entries(summary.events)
    .map(([name, count]) => `${name.replace(/_/g, " ")}: ${count}`)
    .join(", ");
  return [
    `Knov ${summary.appVersion} · macOS ${summary.macOS}`,
    `AI: ${summary.aiProvider} · browser profiles: ${summary.browserProfiles} · labs: ${summary.labsEnabled ? "on" : "off"}`,
    `Days with activity: ${summary.daysWithActivity}${summary.daysSinceFirstActivity != null ? ` (first seen ${summary.daysSinceFirstActivity} days ago)` : ""}`,
    `Questions asked: ${summary.questionsAsked}`,
    `Actions: ${events || "none yet"}`,
  ];
}

export function composeFeedback({
  feeling,
  benefit,
  improve,
  summary,
}: {
  feeling?: Disappointment;
  benefit: string;
  improve: string;
  summary?: TesterSummary;
}): string {
  const label = disappointment.find((option) => option.value === feeling)?.label ?? "No answer";
  return [
    `How would you feel if you could no longer use Knov? ${label}`,
    "",
    "What is the main benefit you get from Knov?",
    benefit.trim() || "—",
    "",
    "What should we improve or fix?",
    improve.trim() || "—",
    ...(summary ? ["", "Anonymous usage counts (no titles, URLs, apps, or chat text):", ...summaryLines(summary)] : []),
  ].join("\n");
}

/**
 * Tester feedback that reaches the team. Nothing is sent from Knov itself:
 * the tester reviews the text and sends it from their own mail app or form.
 */
export function FeedbackDialog({ onClose }: { onClose: () => void }) {
  const summary = useResource(() => api.testerSummary(), []);
  const [feeling, setFeeling] = useState<Disappointment>();
  const [benefit, setBenefit] = useState("");
  const [improve, setImprove] = useState("");
  const [includeSummary, setIncludeSummary] = useState(true);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<{ tone: "ok" | "error"; text: string }>();

  const body = composeFeedback({
    feeling,
    benefit,
    improve,
    summary: includeSummary ? summary.data : undefined,
  });
  const subject = `Knov feedback${feeling ? ` (${disappointment.find((option) => option.value === feeling)?.label})` : ""}`;
  const hasContent = Boolean(feeling || benefit.trim() || improve.trim());

  const send = async () => {
    setBusy(true);
    setStatus(undefined);
    try {
      if (FEEDBACK_FORM_URL) {
        await navigator.clipboard.writeText(body).catch(() => undefined);
        await api.openResource(FEEDBACK_FORM_URL);
        setStatus({ tone: "ok", text: "Opened the feedback form. Your answers are copied—paste them in." });
      } else if (FEEDBACK_EMAIL) {
        await api.openMailDraft(FEEDBACK_EMAIL, subject, body);
        setStatus({ tone: "ok", text: "Opened a draft in your mail app. Review it and press Send." });
      } else {
        const issue = `${REPOSITORY_URL}/issues/new?title=${encodeURIComponent(subject)}&body=${encodeURIComponent(body)}`;
        await api.openResource(issue);
        setStatus({ tone: "ok", text: "Opened a feedback page in your browser." });
      }
    } catch (cause) {
      setStatus({ tone: "error", text: `${errorMessage(cause)} You can copy the feedback instead.` });
    } finally {
      setBusy(false);
    }
  };

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(`${subject}\n\n${body}`);
      setStatus({ tone: "ok", text: `Copied. Paste it into a message${FEEDBACK_EMAIL ? ` to ${FEEDBACK_EMAIL}` : ""}.` });
    } catch {
      setStatus({ tone: "error", text: "Could not copy to the clipboard." });
    }
  };

  return (
    <Modal title="Share feedback" onClose={onClose} wide>
      <div className="feedback-form">
        <p className="modal-copy">You’re testing an early version of Knov. Two minutes of honest feedback shapes what gets built next.</p>
        <fieldset className="feedback-feeling">
          <legend>How would you feel if you could no longer use Knov?</legend>
          <div>
            {disappointment.map((option) => (
              <label key={option.value} className={feeling === option.value ? "selected" : ""}>
                <input type="radio" name="feeling" value={option.value} checked={feeling === option.value} onChange={() => setFeeling(option.value)} />
                {option.label}
              </label>
            ))}
          </div>
        </fieldset>
        <label>What is the main benefit you get from Knov?<textarea value={benefit} onChange={(event) => setBenefit(event.target.value)} placeholder="e.g. I stopped re-explaining my project every time I open ChatGPT" /></label>
        <label>What should we improve or fix?<textarea value={improve} onChange={(event) => setImprove(event.target.value)} placeholder="Anything confusing, broken, missing, or creepy" /></label>
        <label className="feedback-include">
          <input type="checkbox" checked={includeSummary} onChange={(event) => setIncludeSummary(event.target.checked)} />
          <span>Include anonymous usage counts<small>Version, days used, and how often features were used. Never titles, URLs, apps, or chat text.</small></span>
        </label>
        {includeSummary && summary.data && (
          <details className="feedback-preview"><summary>See exactly what’s included</summary><pre>{summaryLines(summary.data).join("\n")}</pre></details>
        )}
        {status && (status.tone === "ok"
          ? <p className="success-message" role="status"><Check size={14} />{status.text}</p>
          : <p className="error-message" role="alert">{status.text}</p>)}
        <div className="modal-actions">
          <button type="button" className="ghost-button" onClick={() => void copy()}><Copy size={15} /> Copy</button>
          <button type="button" className="primary-button" disabled={busy || !hasContent} onClick={() => void send()}>
            {busy ? <LoaderCircle size={15} className="spin" /> : <Send size={15} />} Send feedback
          </button>
        </div>
      </div>
    </Modal>
  );
}
