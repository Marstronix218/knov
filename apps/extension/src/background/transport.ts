import { normalizeDomains, normalizeLoopbackEndpoint } from "../shared/config";
import {
  NATIVE_HOST_NAME,
  PROTOCOL_VERSION,
  type BrowserActivityEvent,
  type EventBatch,
  type ExtensionConfig,
  type NativeRequest,
  type NativeResponse
} from "../shared/types";
import {
  saveConnection,
  type StoredConnection
} from "./storage";

const REQUEST_TIMEOUT_MS = 8_000;

export class TransportError extends Error {
  constructor(
    message: string,
    readonly kind: "authentication" | "connection" | "configuration"
  ) {
    super(message);
  }
}

async function authenticatedFetch(
  config: ExtensionConfig,
  path: string,
  init?: RequestInit
): Promise<Response> {
  if (!config.pairingToken) {
    throw new TransportError("Pairing token is missing.", "configuration");
  }

  let endpoint: string;
  try {
    endpoint = normalizeLoopbackEndpoint(config.endpoint);
  } catch {
    throw new TransportError("Local app address is invalid.", "configuration");
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    const response = await fetch(`${endpoint}${path}`, {
      ...init,
      cache: "no-store",
      signal: controller.signal,
      headers: {
        Authorization: `Bearer ${config.pairingToken}`,
        "Content-Type": "application/json",
        "X-Knov-Protocol": String(PROTOCOL_VERSION),
        ...(init?.headers ?? {})
      }
    });
    if (response.status === 401 || response.status === 403) {
      throw new TransportError(
        "The Mac app rejected the pairing token.",
        "authentication"
      );
    }
    if (!response.ok) {
      throw new TransportError(
        `The local app returned HTTP ${response.status}.`,
        "connection"
      );
    }
    return response;
  } catch (error) {
    if (error instanceof TransportError) throw error;
    throw new TransportError("The local Knov app could not be reached.", "connection");
  } finally {
    clearTimeout(timeout);
  }
}

function sendNativeRequest(request: NativeRequest): Promise<NativeResponse> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const timeout = setTimeout(() => {
      if (settled) return;
      settled = true;
      reject(
        new TransportError(
          "The Knov Native Messaging host did not respond.",
          "connection"
        )
      );
    }, REQUEST_TIMEOUT_MS);

    chrome.runtime.sendNativeMessage(
      NATIVE_HOST_NAME,
      request,
      (response: NativeResponse | undefined) => {
        if (settled) return;
        settled = true;
        clearTimeout(timeout);
        if (chrome.runtime.lastError) {
          reject(
            new TransportError(
              "The Knov Mac app or Native Messaging host is unavailable.",
              "connection"
            )
          );
          return;
        }
        if (!response || response.protocolVersion !== PROTOCOL_VERSION) {
          reject(
            new TransportError(
              "The Mac app returned an incompatible protocol response.",
              "connection"
            )
          );
          return;
        }
        if (response.requestId !== request.requestId) {
          reject(
            new TransportError(
              "The Mac app returned a mismatched response.",
              "connection"
            )
          );
          return;
        }
        if (!response.ok) {
          reject(
            new TransportError(
              response.message ?? "The Mac app rejected the request.",
              response.errorCode === "authentication"
                ? "authentication"
                : "connection"
            )
          );
          return;
        }
        resolve(response);
      }
    );
  });
}

async function sendRequest(
  config: ExtensionConfig,
  type: NativeRequest["type"],
  payload?: EventBatch
): Promise<NativeResponse> {
  if (!config.pairingToken) {
    throw new TransportError("Pairing token is missing.", "configuration");
  }
  if (config.transport === "native") {
    return sendNativeRequest({
      protocolVersion: PROTOCOL_VERSION,
      requestId: crypto.randomUUID(),
      extensionId: chrome.runtime.id,
      pairingToken: config.pairingToken,
      sentAt: new Date().toISOString(),
      type,
      payload
    });
  }

  const response = await authenticatedFetch(
    config,
    type === "status" ? "/v1/extension/status" : "/v1/extension/events",
    type === "status"
      ? { method: "GET" }
      : { method: "POST", body: JSON.stringify(payload) }
  );
  return (await response.json()) as NativeResponse;
}

function connectionForError(error: unknown): StoredConnection {
  if (error instanceof TransportError) {
    if (error.kind === "authentication") {
      return { state: "authentication_error", message: error.message };
    }
    if (error.kind === "configuration") {
      return { state: "unpaired", message: error.message };
    }
    return { state: "disconnected", message: error.message };
  }
  return { state: "disconnected", message: "The local app could not be reached." };
}

export async function testConnection(
  config: ExtensionConfig
): Promise<NativeResponse> {
  try {
    const response = await sendRequest(config, "status");
    await saveConnection({
      state: "connected",
      message: "Connected to the local Knov app",
      lastConnectedAt: new Date().toISOString()
    });
    return response;
  } catch (error) {
    await saveConnection(connectionForError(error));
    throw error;
  }
}

export function configWithDesktopPolicy(
  config: ExtensionConfig,
  response: NativeResponse
): ExtensionConfig {
  return {
    ...config,
    collectionEnabled:
      response.collectionEnabled === false ? false : config.collectionEnabled,
    excludedDomains:
      response.excludedDomains === undefined
        ? config.excludedDomains
        : normalizeDomains(response.excludedDomains)
  };
}

export async function sendEvents(
  config: ExtensionConfig,
  events: BrowserActivityEvent[]
): Promise<void> {
  if (events.length === 0) {
    return;
  }

  const body: EventBatch = {
    protocolVersion: PROTOCOL_VERSION,
    source: "chrome_extension",
    extensionId: chrome.runtime.id,
    sentAt: new Date().toISOString(),
    events
  };

  try {
    const response = await sendRequest(config, "events", body);
    if (
      response?.acceptedEventIds &&
      events.some((event) => !response.acceptedEventIds?.includes(event.id))
    ) {
      throw new TransportError(
        "The Mac app did not acknowledge the complete event batch.",
        "connection"
      );
    }
    await saveConnection({
      state: "connected",
      message: "Connected to the local Knov app",
      lastConnectedAt: new Date().toISOString()
    });
  } catch (error) {
    await saveConnection(connectionForError(error));
    throw error;
  }
}
