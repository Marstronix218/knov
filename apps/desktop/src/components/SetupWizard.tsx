import { Check, ChevronRight, Eye, Globe, LoaderCircle, LockKeyhole, MessageSquareText, Sparkles, X } from "lucide-react";
import { ReactNode, useEffect, useState } from "react";
import { useResource } from "../hooks/useResource";
import { api } from "../lib/api";
import type { PermissionProbe, SettingsData } from "../types";
import { AiProviderPicker } from "./AiProviderPicker";
import { errorMessage, LogoMark, ResourceState } from "./ui";

const steps = ["Welcome", "Permissions", "Browser history", "AI"];

export function SetupWizard({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState(0);
  const [selectedProfiles, setSelectedProfiles] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [progress, setProgress] = useState("");
  const browsers = useResource(() => api.browserProfiles(), []);
  const settings = useResource(() => api.settings(), []);
  const aiReady = Boolean(settings.data?.aiConfigured);

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
        <div className="setup-progress">
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
            <h1>Stop re-explaining your work to AI.</h1>
            <p>Knov quietly notices what you work on across apps and websites, then gives you that context when you need it.</p>
            <div className="value-list">
              <article><Sparkles size={18} /><span><strong>Pick up where you left off</strong><small>See your active projects and reopen the last page or app in one click.</small></span></article>
              <article><MessageSquareText size={18} /><span><strong>Ask with context</strong><small>“What did I do on the pitch deck this week?” Knov answers from your real activity.</small></span></article>
              <article><LockKeyhole size={18} /><span><strong>Private by design</strong><small>Raw activity stays on this Mac. Pause, exclude apps or sites, or delete everything anytime.</small></span></article>
            </div>
            <div className="collect-grid">
              <div><strong>Knov records</strong><ul><li>Which app is in front, and for how long</li><li>Window and page titles</li><li>History from browsers you choose</li></ul></div>
              <div><strong>Knov never records</strong><ul><li>Keystrokes or screenshots</li><li>Page or document contents</li><li>Audio, camera, or clipboard</li></ul></div>
            </div>
          </div>
        )}

        {step === 1 && <PermissionStep />}

        {step === 2 && (
          <div className="setup-content">
            <div className="eyebrow">Optional</div>
            <h1>Add your browser history?</h1>
            <p>Importing recent history lets Knov understand your projects on day one instead of after a few days of use. You can skip this and change it later in Settings.</p>
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
            <span className="setup-skip">Up to 90 days are read once to build your first profile; anything older than 30 days is then deleted.</span>
          </div>
        )}

        {step === 3 && (
          <div className="setup-content">
            <div className="eyebrow">Connect an AI</div>
            <h1>Choose where the AI runs.</h1>
            <p>The AI turns your activity into a profile and answers your questions. Everything else works without it, so you can also skip this.</p>
            <ResourceState {...settings}>
              {(data) => <AiProviderPicker compact settings={data} onChange={(next: SettingsData) => settings.setData(next)} />}
            </ResourceState>
            {progress && <p className="setup-progress-note" role="status"><LoaderCircle size={14} className="spin" /> {progress}</p>}
            {message && <p className="error-message" role="alert">{message}</p>}
          </div>
        )}

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
    <div className="setup-content narrow">
      <div className="eyebrow">macOS permissions</div>
      <h1>Let Knov see which app you’re using.</h1>
      <p>macOS will ask twice: once to let Knov ask <strong>System Events</strong> which app is in front, and once for <strong>Accessibility</strong> so it can read window titles. Knov never reads keystrokes, screen contents, or documents.</p>
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
      <span className="setup-skip">Already enabled but still showing off? Quit Knov from the menu bar icon and open it again.</span>
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
