import { Check, Copy, LoaderCircle, Puzzle } from "lucide-react";
import { FormEvent, useEffect, useState } from "react";
import { useResource } from "../hooks/useResource";
import { api } from "../lib/api";
import type { CollectionStatus } from "../types";
import { errorMessage, SettingsHeading } from "./ui";

type Status = { tone: "ok" | "error"; text: string };

function StatusLine({ status }: { status?: Status }) {
  if (!status) return null;
  return status.tone === "ok"
    ? <p className="success-message" role="status"><Check size={14} />{status.text}</p>
    : <p className="error-message" role="alert">{status.text}</p>;
}

/** A labeled number field that saves one setting and reports the result inline. */
export function NumberSetting({
  label,
  detail,
  value,
  min,
  max,
  step = 1,
  onSave,
}: {
  label: string;
  detail: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onSave: (value: number) => Promise<void>;
}) {
  const [draft, setDraft] = useState(String(value));
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<Status>();
  useEffect(() => setDraft(String(value)), [value]);
  const parsed = Number(draft);
  const inRange = draft.trim() !== "" && Number.isFinite(parsed) && parsed >= min && parsed <= max;
  // The backend stores whole-step settings as integers; a decimal would not persist as typed.
  const whole = step !== 1 || Number.isInteger(parsed);
  const valid = inRange && whole;

  const save = async (event: FormEvent) => {
    event.preventDefault();
    if (!valid) return;
    setSaving(true);
    setStatus(undefined);
    try {
      await onSave(parsed);
      setStatus({ tone: "ok", text: "Saved." });
    } catch (cause) {
      setStatus({ tone: "error", text: errorMessage(cause) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <form className="stack-form" onSubmit={(event) => void save(event)}>
      <label>
        {label}
        <input
          type="number"
          inputMode="numeric"
          min={min}
          max={max}
          step={step}
          value={draft}
          aria-invalid={!valid}
          onChange={(event) => { setDraft(event.target.value); setStatus(undefined); }}
        />
      </label>
      <p className="status-detail">{detail}</p>
      {!valid && <p className="error-message" role="alert">Enter a {inRange ? "whole " : ""}number from {min} to {max}.</p>}
      <button className="ghost-button" disabled={!valid || saving || parsed === value}>
        {saving && <LoaderCircle size={14} className="spin" />} Save
      </button>
      <StatusLine status={status} />
    </form>
  );
}

const EXTENSION_ID = /^[a-p]{32}$/;

/** Registers the Native Messaging host and shows the pairing token for the companion extension. */
export function ChromeCompanionCard({ status }: { status: CollectionStatus }) {
  const pairing = useResource(() => api.getPairingInfo(), []);
  const [extensionId, setExtensionId] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const [registering, setRegistering] = useState(false);
  const [registerStatus, setRegisterStatus] = useState<Status>();
  const [copyStatus, setCopyStatus] = useState<Status>();
  const id = extensionId.trim();
  const idError = (submitted || id) && !EXTENSION_ID.test(id)
    ? "An extension ID is 32 lowercase letters from a to p. Copy it from the Knov card on chrome://extensions."
    : "";
  // Loading or failed pairing info is not proof of a bridge, so Register stays off until it says so.
  const bridgeEnabled = pairing.data?.bridgeEnabled === true;

  const register = async (event: FormEvent) => {
    event.preventDefault();
    setSubmitted(true);
    setRegisterStatus(undefined);
    if (!EXTENSION_ID.test(id)) return;
    setRegistering(true);
    try {
      const manifestPath = await api.installNativeHost(id);
      setRegisterStatus({ tone: "ok", text: `Native host registered at ${manifestPath}. Restart Chrome so it sees the registration.` });
    } catch (cause) {
      setRegisterStatus({ tone: "error", text: errorMessage(cause) });
    } finally {
      setRegistering(false);
    }
  };

  const copyToken = async (token: string) => {
    if (!navigator.clipboard?.writeText) {
      setCopyStatus({ tone: "error", text: "Clipboard access is unavailable. Select the token and copy it manually." });
      return;
    }
    try {
      await navigator.clipboard.writeText(token);
      setCopyStatus({ tone: "ok", text: "Pairing token copied." });
    } catch {
      setCopyStatus({ tone: "error", text: "Could not copy. Select the token and copy it manually." });
    }
  };

  const lastSeen = status.extensionLastSeenAt
    ? new Date(status.extensionLastSeenAt * 1000).toLocaleString()
    : undefined;

  return (
    <section className="panel settings-card" aria-label="Chrome companion">
      <SettingsHeading icon={<Puzzle />} title="Chrome companion" detail="Optional experimental extension that adds active-tab timing. Load it unpacked first; see the README." />
      <div className="permission-row">
        <div>
          <strong>Extension connection</strong>
          <p>{status.extensionConnected
            ? `Reported ${lastSeen ? `at ${lastSeen}` : "recently"}.`
            : lastSeen ? `Last report ${lastSeen}.` : "No report from the extension yet."}</p>
        </div>
        <span className={status.extensionConnected ? "status-ok" : "status-warn"}>{status.extensionConnected ? "Connected" : "Not connected"}</span>
      </div>
      {pairing.data && !bridgeEnabled && (
        <p className="notice info" role="note">This build doesn’t include the companion bridge, so the extension can’t deliver activity even after pairing. Use a build made with the chrome-extension feature (npm run dev:with-extension).</p>
      )}
      <form className="stack-form" onSubmit={(event) => void register(event)} noValidate>
        <label>
          Extension ID
          <input
            value={extensionId}
            onChange={(event) => { setExtensionId(event.target.value); setRegisterStatus(undefined); }}
            placeholder="32 letters from chrome://extensions"
            autoComplete="off"
            spellCheck={false}
            maxLength={64}
            aria-invalid={Boolean(idError)}
          />
        </label>
        {idError && <p className="error-message" role="alert">{idError}</p>}
        <button className="ghost-button" disabled={registering || !bridgeEnabled}>
          {registering && <LoaderCircle size={14} className="spin" />} Register native host
        </button>
        <StatusLine status={registerStatus} />
      </form>
      {pairing.loading && !pairing.data ? (
        <p className="status-detail" role="status">Loading pairing token…</p>
      ) : pairing.error && !pairing.data ? (
        <p className="error-message" role="alert">{pairing.error}</p>
      ) : pairing.data ? (
        <div className="stack-form">
          <label>
            Pairing token
            <input readOnly value={pairing.data.pairingToken} onFocus={(event) => event.currentTarget.select()} spellCheck={false} />
          </label>
          <p className="status-detail">Paste this token in the extension’s options page, then choose Save and verify.</p>
          <button type="button" className="ghost-button" onClick={() => void copyToken(pairing.data!.pairingToken)}>
            <Copy size={14} /> Copy
          </button>
          <StatusLine status={copyStatus} />
        </div>
      ) : null}
    </section>
  );
}
