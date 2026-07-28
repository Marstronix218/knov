import { DEFAULT_CONFIG, normalizeDomains } from "../shared/config";
import type {
  ActiveSession,
  ConnectionState,
  ExtensionConfig
} from "../shared/types";

const CONFIG_KEY = "config";
const SESSION_KEY = "activeSession";
const CONNECTION_KEY = "connection";

export interface StoredConnection {
  state: ConnectionState;
  message: string;
  lastConnectedAt?: string;
}

export async function loadConfig(): Promise<ExtensionConfig> {
  const result = await chrome.storage.local.get(CONFIG_KEY);
  const stored = result[CONFIG_KEY] as Partial<ExtensionConfig> | undefined;
  return {
    ...DEFAULT_CONFIG,
    ...stored,
    excludedDomains: normalizeDomains(stored?.excludedDomains ?? [])
  };
}

export async function saveConfig(config: ExtensionConfig): Promise<void> {
  await chrome.storage.local.set({ [CONFIG_KEY]: config });
}

export async function loadSession(): Promise<ActiveSession | undefined> {
  const result = await chrome.storage.session.get(SESSION_KEY);
  return result[SESSION_KEY] as ActiveSession | undefined;
}

export async function saveSession(session?: ActiveSession): Promise<void> {
  if (session) {
    await chrome.storage.session.set({ [SESSION_KEY]: session });
  } else {
    await chrome.storage.session.remove(SESSION_KEY);
  }
}

export async function loadConnection(): Promise<StoredConnection> {
  const result = await chrome.storage.local.get(CONNECTION_KEY);
  return (
    (result[CONNECTION_KEY] as StoredConnection | undefined) ?? {
      state: "unpaired",
      message: "Pair with the Knoveyla Mac app"
    }
  );
}

export async function saveConnection(value: StoredConnection): Promise<void> {
  await chrome.storage.local.set({ [CONNECTION_KEY]: value });
}
