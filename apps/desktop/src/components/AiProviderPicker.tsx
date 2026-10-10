import { Check, Copy, Cpu, KeyRound, LoaderCircle, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { OLLAMA_DOWNLOAD_URL, SUGGESTED_LOCAL_MODEL } from "../config";
import { api } from "../lib/api";
import type { LocalModelDetection, Provider, SettingsData } from "../types";
import { errorMessage } from "./ui";

const cloudProviders: Provider[] = ["openai", "anthropic", "bedrock"];

export function providerLabel(provider: Provider): string {
  if (provider === "local") return "Local AI";
  if (provider === "openai") return "OpenAI";
  if (provider === "anthropic") return "Anthropic";
  return "AWS Bedrock";
}

function providerKeyPlaceholder(provider: Provider): string {
  if (provider === "openai") return "sk-…";
  if (provider === "anthropic") return "sk-ant-…";
  return "ABSK…";
}

type Status = { tone: "ok" | "error"; text: string };

/**
 * Connects Knov to an AI: a model running on this Mac (Ollama, LM Studio) or a
 * cloud provider with the user's own key. Local is the default because
 * nothing leaves the Mac and no account or key is needed.
 */
export function AiProviderPicker({
  settings,
  onChange,
  compact = false,
}: {
  settings: SettingsData;
  onChange: (settings: SettingsData) => void;
  compact?: boolean;
}) {
  const [mode, setMode] = useState<"local" | "cloud">(settings.provider === "local" ? "local" : "cloud");
  const [cloudProvider, setCloudProvider] = useState<Provider>(settings.provider === "local" ? "openai" : settings.provider);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<Status>();

  const run = async (work: () => Promise<string>) => {
    setBusy(true);
    setStatus(undefined);
    try {
      setStatus({ tone: "ok", text: await work() });
    } catch (cause) {
      setStatus({ tone: "error", text: errorMessage(cause) });
    } finally {
      setBusy(false);
    }
  };

  const connectedLabel = settings.aiConfigured
    ? settings.provider === "local"
      ? `Connected to ${settings.localModel || "your local model"} on this Mac`
      : `Connected to ${providerLabel(settings.provider)}`
    : undefined;

  return (
    <div className={`ai-picker${compact ? " compact" : ""}`}>
      <div className="ai-mode-grid" role="radiogroup" aria-label="Where AI runs">
        <button type="button" role="radio" aria-checked={mode === "local"} className={mode === "local" ? "selected" : ""} onClick={() => { setMode("local"); setStatus(undefined); }}>
          <Cpu size={18} />
          <span><strong>On this Mac</strong><small>Free and private. Uses Ollama or LM Studio.</small></span>
        </button>
        <button type="button" role="radio" aria-checked={mode === "cloud"} className={mode === "cloud" ? "selected" : ""} onClick={() => { setMode("cloud"); setStatus(undefined); }}>
          <KeyRound size={18} />
          <span><strong>Cloud API key</strong><small>OpenAI, Anthropic, or Bedrock. Smarter, needs a paid key.</small></span>
        </button>
      </div>

      {connectedLabel && <p className="ai-connected"><Check size={14} /> {connectedLabel}</p>}

      {mode === "local" ? (
        <LocalModelSetup settings={settings} busy={busy} onRun={run} onChange={onChange} />
      ) : (
        <div className="cloud-setup">
          <div className="provider-tabs">
            {cloudProviders.map((item) => (
              <button type="button" className={cloudProvider === item ? "selected" : ""} key={item} onClick={() => { setCloudProvider(item); setStatus(undefined); }}>
                {providerLabel(item)}
              </button>
            ))}
          </div>
          <label className="secret-field">
            API key
            <input
              type="password"
              value={key}
              autoComplete="off"
              onChange={(event) => setKey(event.target.value)}
              placeholder={settings.provider === cloudProvider && settings.hasProviderKey ? "Key stored in macOS Keychain" : providerKeyPlaceholder(cloudProvider)}
            />
          </label>
          <p className="status-detail">Stored in macOS Keychain and sent only to {providerLabel(cloudProvider)}. Only a minimized summary and your questions are sent—never your raw activity.</p>
          <div className="inline-actions">
            <button type="button" className="primary-button" disabled={busy || !key.trim()} onClick={() => void run(async () => {
              await api.saveProviderKey(cloudProvider, key.trim());
              setKey("");
              const next = await api.saveSettings({ provider: cloudProvider });
              onChange(next);
              await api.testProvider(cloudProvider);
              return `Connected to ${providerLabel(cloudProvider)}.`;
            })}>
              {busy && <LoaderCircle size={14} className="spin" />} Save and connect
            </button>
            {settings.provider === cloudProvider && settings.hasProviderKey && (
              <>
                <button type="button" className="ghost-button" disabled={busy} onClick={() => void run(async () => {
                  await api.testProvider(cloudProvider);
                  return "Connection successful.";
                })}>Test connection</button>
                <button type="button" className="ghost-button" disabled={busy} onClick={() => void run(async () => {
                  await api.removeProviderKey(cloudProvider);
                  onChange(await api.settings());
                  return "Key removed from Keychain.";
                })}>Remove key</button>
              </>
            )}
          </div>
        </div>
      )}

      {status && (status.tone === "ok"
        ? <p className="success-message" role="status"><Check size={14} />{status.text}</p>
        : <p className="error-message" role="alert">{status.text}</p>)}
    </div>
  );
}

function LocalModelSetup({
  settings,
  busy,
  onRun,
  onChange,
}: {
  settings: SettingsData;
  busy: boolean;
  onRun: (work: () => Promise<string>) => Promise<void>;
  onChange: (settings: SettingsData) => void;
}) {
  const [baseUrl, setBaseUrl] = useState(settings.localBaseUrl);
  const [detection, setDetection] = useState<LocalModelDetection>();
  const [detecting, setDetecting] = useState(false);
  const [model, setModel] = useState(settings.localModel ?? "");
  const [copied, setCopied] = useState(false);

  const detect = useCallback(async (address?: string) => {
    setDetecting(true);
    try {
      const result = await api.detectLocalModels(address);
      setDetection(result);
      setModel((current) => (current && result.models.includes(current) ? current : result.models[0] ?? ""));
    } catch (cause) {
      setDetection({ baseUrl: address ?? "", reachable: false, models: [], error: errorMessage(cause) });
    } finally {
      setDetecting(false);
    }
  }, []);

  useEffect(() => {
    void detect(settings.localBaseUrl);
  }, [detect, settings.localBaseUrl]);

  const pullCommand = `ollama pull ${SUGGESTED_LOCAL_MODEL}`;
  const copyPull = async () => {
    await navigator.clipboard.writeText(pullCommand);
    setCopied(true);
  };

  const ready = Boolean(detection?.reachable && detection.models.length);

  return (
    <div className="local-setup">
      {detecting && !detection ? (
        <p className="status-detail"><LoaderCircle size={14} className="spin" /> Looking for a local AI server…</p>
      ) : ready ? (
        <>
          <label className="secret-field">
            Model
            <select value={model} onChange={(event) => setModel(event.target.value)}>
              {detection!.models.map((name) => <option key={name} value={name}>{name}</option>)}
            </select>
          </label>
          <p className="status-detail">Everything—profile building and chat—runs on this Mac. Larger models give better answers but are slower.</p>
          <div className="inline-actions">
            <button type="button" className="primary-button" disabled={busy || !model} onClick={() => void onRun(async () => {
              const next = await api.saveSettings({ provider: "local", localBaseUrl: detection!.baseUrl, localModel: model });
              onChange(next);
              await api.testProvider("local");
              return `Using ${model} on this Mac.`;
            })}>
              {busy && <LoaderCircle size={14} className="spin" />} Use this model
            </button>
            <button type="button" className="ghost-button" disabled={detecting} onClick={() => void detect(baseUrl)}>
              <RefreshCw size={14} className={detecting ? "spin" : ""} /> Refresh list
            </button>
          </div>
        </>
      ) : (
        <div className="local-steps">
          <p>{detection?.reachable
            ? "Your local AI server is running but has no models yet."
            : "No local AI server found. Two quick steps:"}</p>
          <ol>
            {!detection?.reachable && (
              <li>
                Install and open <strong>Ollama</strong>.{" "}
                <button type="button" className="link-button" onClick={() => void api.openResource(OLLAMA_DOWNLOAD_URL)}>Download Ollama</button>
              </li>
            )}
            <li>
              Download a model in Terminal (about 2 GB):
              <span className="command-copy"><code>{pullCommand}</code><button type="button" className="link-button" onClick={() => void copyPull()}>{copied ? <Check size={13} /> : <Copy size={13} />}{copied ? "Copied" : "Copy"}</button></span>
            </li>
          </ol>
          <button type="button" className="ghost-button" disabled={detecting} onClick={() => void detect(baseUrl)}>
            <RefreshCw size={14} className={detecting ? "spin" : ""} /> Check again
          </button>
        </div>
      )}
      <details className="local-advanced">
        <summary>Server address (for LM Studio or a custom port)</summary>
        <div className="inline-actions">
          <input aria-label="Local AI server address" value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} placeholder="http://localhost:11434" />
          <button type="button" className="ghost-button" disabled={detecting} onClick={() => void detect(baseUrl)}>Check</button>
        </div>
        <small>Ollama uses http://localhost:11434. LM Studio’s server uses http://localhost:1234. Only addresses on this Mac are allowed.</small>
        {detection?.error && !ready && <small className="local-error">{detection.error}</small>}
      </details>
    </div>
  );
}
