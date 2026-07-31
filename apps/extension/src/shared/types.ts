export const PROTOCOL_VERSION = 1 as const;
export const NATIVE_HOST_NAME = "com.knov.companion";

export type TransportMode = "native" | "localhost";

export type ConnectionState =
  | "connected"
  | "disconnected"
  | "unpaired"
  | "authentication_error"
  | "permission_error";

export interface ExtensionConfig {
  collectionEnabled: boolean;
  browserProfileId: string;
  transport: TransportMode;
  endpoint: string;
  pairingToken: string;
  excludedDomains: string[];
}

export interface ActivePage {
  tabId: number;
  windowId: number;
  url: string;
  title: string;
  incognito: boolean;
}

export interface ActiveSession extends ActivePage {
  startedAt: string;
  lastObservedAt: string;
}

export interface BrowserActivityEvent {
  id: string;
  kind: "browser_focus";
  source: "chrome_extension";
  browser: "chrome";
  browserProfileId: string;
  url: string;
  title: string;
  startedAt: string;
  endedAt: string;
  durationMs: number;
  incognito: boolean;
}

export interface RuntimeStatus {
  collectionEnabled: boolean;
  connection: ConnectionState;
  connectionMessage: string;
  lastConnectedAt?: string;
  queueSize: number;
  currentPage?: Pick<ActivePage, "title" | "url">;
}

export type ExtensionMessage =
  | { type: "get_status" }
  | { type: "get_config" }
  | { type: "set_collection_enabled"; enabled: boolean }
  | { type: "save_config"; config: Partial<ExtensionConfig> }
  | { type: "test_connection" }
  | { type: "flush" };

export interface EventBatch {
  protocolVersion: typeof PROTOCOL_VERSION;
  source: "chrome_extension";
  extensionId: string;
  sentAt: string;
  events: BrowserActivityEvent[];
}

export interface NativeRequest {
  protocolVersion: typeof PROTOCOL_VERSION;
  requestId: string;
  extensionId: string;
  pairingToken: string;
  sentAt: string;
  type: "status" | "events";
  payload?: EventBatch;
}

export interface NativeResponse {
  protocolVersion: typeof PROTOCOL_VERSION;
  requestId: string;
  ok: boolean;
  errorCode?: "authentication" | "protocol" | "unavailable" | "internal";
  message?: string;
  acceptedEventIds?: string[];
  collectionEnabled?: boolean;
}
