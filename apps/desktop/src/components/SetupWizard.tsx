import { BadgeDollarSign, Briefcase, Check, ChevronRight, Clock, Eye, FileWarning, FlaskConical, Globe, LoaderCircle, LockKeyhole, Sparkles, X } from "lucide-react";
import { ReactNode, useEffect, useState } from "react";
import { useResource } from "../hooks/useResource";
import { api } from "../lib/api";
import { setSimulationMode } from "../state/simulationMode";
import type { PermissionProbe, SettingsData } from "../types";
import { AiProviderPicker } from "./AiProviderPicker";
import { errorMessage, LogoMark, ResourceState } from "./ui";

type Start = "simulation" | "own";

export function SetupWizard({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState(0);
  const [start, setStart] = useState<Start>("simulation");
  const [selectedProfiles, setSelectedProfiles] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [progress, setProgress] = useState("");
  const browsers = useResource(() => api.browserProfiles(), []);
  const settings = useResource(() => api.settings(), []);
  const aiReady = Boolean(settings.data?.aiConfigured);
  // When Knov provides the AI there is nothing for the user to set up.
  const steps = settings.data?.managedAi ? ["Welcome", "Start", "Mac activity"] : ["Welcome", "Start", "Mac activity", "AI"];

  const finish = async () => {
    setBusy(true);
    setMessage("");
    try {
      await api.setBrowserProfiles(selectedProfiles);
      await api.setCollectionEnabled(true);
      if (aiReady) {
        // Importing history and building the first profile can take minutes on a
        // local model; it continues in the background and Now shows progress.
        // Wait until the build has registered so Now doesn't offer to start it again.
        setProgress("Starting your first profile…");
        void api.startBootstrap().catch(() => undefined);
        for (let attempt = 0; attempt < 20; attempt += 1) {
          if ((await api.bootstrapStatus()).phase !== "not-started") break;
          await new Promise((resolve) => window.setTimeout(resolve, 150));
        }
      } else {
        if (selectedProfiles.length) setProgress("Importing the last 30 days of browser history…");
        await api.startLocalBootstrap();
      }
      void api.recordProductEvent("setup_completed").catch(() => undefined);
      setSimulationMode(start === "simulation");
      window.location.hash = start === "simulation" ? "/simulation" : "/sources";
      onComplete();
    } catch (cause) {
      setMessage(errorMessage(cause));
    } finally {
      setBusy(false);
      setProgress("");
    }
  };

  return (
    <div className="setup-shell">
      <section className="setup-panel">
        <div className="setup-brand"><LogoMark /><strong>Knov</strong><span className="beta-pill">Early access</span></div>
        <div className="setup-progress" style={{ gridTemplateColumns: `repeat(${steps.length}, 1fr)` }}>
          {steps.map((label, index) => (
            <div className={index <= step ? "active" : ""} key={label}>
              <span>{index < step ? <Check size={12} /> : index + 1}</span>
              <small>{label}</small>
            </div>
          ))}
        </div>

        {step === 0 && (
          <div className="setup-content">
            <div className="eyebrow">Welcome to Knov</div>
            <h1>Find the revenue your business is missing.</h1>
            <p>Knov compares what clients agreed to pay for with what they asked for, what your team delivered, and what you invoiced. Then it tells you what is wrong and what to do about it.</p>
            <div className="value-list">
              <article><BadgeDollarSign size={18} /><span><strong>Unbilled extra work</strong><small>A client asked for something outside the contract, and your team already did it.</small></span></article>
              <article><FileWarning size={18} /><span><strong>Missing invoices</strong><small>A milestone was accepted, but no invoice went out.</small></span></article>
              <article><Clock size={18} /><span><strong>Overdue promises</strong><small>You told a client you would send something, and the date passed.</small></span></article>
              <article><LockKeyhole size={18} /><span><strong>You stay in control</strong><small>Analysis runs on this Mac. Knov drafts change orders, invoices, and follow-ups, and never sends anything for you.</small></span></article>
            </div>
          </div>
        )}

        {step === 1 && (
          <div className="setup-content narrow">
            <div className="eyebrow">Get started</div>
            <h1>How do you want to start?</h1>
            <p>You can switch between the two at any time.</p>
            <div className="start-choice" role="radiogroup" aria-label="How to start">
              <button type="button" role="radio" aria-checked={start === "simulation"} className={start === "simulation" ? "selected" : ""} onClick={() => setStart("simulation")}>
                <FlaskConical size={20} />
                <span><strong>Explore a simulated business</strong><small>Recommended first. Watch Knov read a fictional agency's contracts, inbox, Slack, and invoices, and find what it is missing.</small></span>
              </button>
              <button type="button" role="radio" aria-checked={start === "own"} className={start === "own" ? "selected" : ""} onClick={() => setStart("own")}>
                <Briefcase size={20} />
                <span><strong>Use my own business data</strong><small>Upload a contract and connect Gmail or Slack. Takes a few minutes.</small></span>
              </button>
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="setup-content">
            <div className="eyebrow">Optional</div>
            <h1>Let Knov see the work you do.</h1>
            <p>Activity on this Mac is evidence that work was delivered, which helps Knov spot work you haven't billed. You can skip this and turn it on later in Settings.</p>
            <PermissionStep />
            <h2 className="setup-subheading">Browser history</h2>
            <ResourceState {...browsers}>
              {(profiles) => profiles.length ? (
                <div className="setup-browser-grid">
                  {profiles.map((profile) => (
                    <label className={selectedProfiles.includes(profile.id) ? "selected" : ""} key={profile.id}>
                      <input
                        type="checkbox"
                        checked={selectedProfiles.includes(profile.id)}
                        onChange={(event) => setSelectedProfiles(event.target.checked ? [...selectedProfiles, profile.id] : selectedProfiles.filter((id) => id !== profile.id))}
                      />
                      <div className="browser-icon">{profile.browser.slice(0, 1).toUpperCase()}</div>
                      <span><strong>{profile.name}</strong><small>{profile.browser}</small></span>
                      {selectedProfiles.includes(profile.id) && <Check size={16} />}
                    </label>
                  ))}
                </div>
              ) : (
                <p className="setup-note"><Globe size={16} /> No Chrome, Arc, Brave, Edge, or Vivaldi profiles were found. That’s fine—Knov still learns from app and window activity, including Safari page titles.</p>
              )}
            </ResourceState>
            <span className="setup-skip">Knov records which app is in front, window and page titles, and history from browsers you choose. It never records keystrokes, screenshots, or page contents. Up to 90 days of history are read once; anything older than 30 days is then deleted.</span>
          </div>
        )}

        {step === 3 && (
          <div className="setup-content">
            <div className="eyebrow">Connect an AI</div>
            <h1>Choose where the AI runs.</h1>
            <p>The AI powers the interview and Ask. Finding revenue problems works without it, so you can also skip this.</p>
            <ResourceState {...settings}>
              {(data) => <AiProviderPicker compact settings={data} onChange={(next: SettingsData) => settings.setData(next)} />}
            </ResourceState>
          </div>
        )}
        {progress && <p className="setup-progress-note" role="status"><LoaderCircle size={14} className="spin" /> {progress}</p>}
        {message && <p className="error-message" role="alert">{message}</p>}

        <footer className="setup-footer">
          <button className="ghost-button" disabled={step === 0 || busy} onClick={() => setStep((value) => value - 1)}>Back</button>
          <span>{step + 1} of {steps.length}</span>
          {step < steps.length - 1
            ? <button className="primary-button" onClick={() => setStep((value) => value + 1)}>{step === 2 && !selectedProfiles.length ? "Skip" : "Continue"} <ChevronRight size={15} /></button>
            : (
              <button className="primary-button" disabled={busy} onClick={() => void finish()}>
                {busy ? <LoaderCircle size={15} className="spin" /> : <Sparkles size={15} />}
                {aiReady ? "Finish setup" : "Skip AI and finish"}
              </button>
            )}
        </footer>
      </section>
    </div>
  );
}

function PermissionStep() {
  const [probe, setProbe] = useState<PermissionProbe>();
  const [checking, setChecking] = useState(false);
  const [watching, setWatching] = useState(false);

  const check = async () => {
    setChecking(true);
    try {
      setProbe(await api.probePermissions());
    } catch (cause) {
      setProbe({ foregroundApps: false, windowTitles: false, message: errorMessage(cause) });
    } finally {
      setChecking(false);
    }
  };

  // After the user opens System Settings, keep checking so the status flips
  // to "On" without them having to come back and click again.
  useEffect(() => {
    if (!watching || probe?.windowTitles) return;
    const interval = window.setInterval(() => void api.probePermissions().then(setProbe).catch(() => undefined), 3000);
    return () => window.clearInterval(interval);
  }, [watching, probe?.windowTitles]);

  return (
    <div className="setup-permissions">
      <p className="status-detail">macOS asks twice: once to let Knov ask <strong>System Events</strong> which app is in front, and once for <strong>Accessibility</strong> so it can read window titles.</p>
      <div className="permission-checklist">
        <PermissionRow
          label="Apps in front"
          detail="Required. Click Allow when macOS asks about System Events."
          state={probe ? probe.foregroundApps : undefined}
          action={<button className="ghost-button" disabled={checking} onClick={() => void check()}>{checking && <LoaderCircle size={14} className="spin" />}{probe ? "Check again" : "Allow"}</button>}
        />
        <PermissionRow
          label="Window titles"
          detail="Recommended. Turn on Knov under Privacy & Security → Accessibility."
          state={probe ? probe.windowTitles : undefined}
          action={<button className="ghost-button" onClick={() => { void api.requestAccessibility(); setWatching(true); void check(); }}>Open settings</button>}
        />
      </div>
      {probe?.message && !probe.foregroundApps && <p className="status-detail">{probe.message}. If you clicked “Don’t Allow”, enable Knov under System Settings → Privacy & Security → Automation.</p>}
    </div>
  );
}

function PermissionRow({ label, detail, state, action }: { label: string; detail: string; state?: boolean; action: ReactNode }) {
  return (
    <div className="permission-check">
      <span className={`permission-state ${state === undefined ? "unknown" : state ? "on" : "off"}`}>
        {state === undefined ? <Eye size={15} /> : state ? <Check size={15} /> : <X size={15} />}
      </span>
      <span><strong>{label}</strong><small>{detail}</small></span>
      <span className="permission-status">{state === undefined ? "" : state ? "On" : "Off"}</span>
      {!state && action}
    </div>
  );
}
