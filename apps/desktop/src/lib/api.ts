import { invoke } from "@tauri-apps/api/core";
import type {
  ActivityEvent,
  ActivityPreview,
  BootstrapStatus,
  BrowserProfile,
  ChatMode,
  ChatMessage,
  ChatRunResult,
  DashboardData,
  ProfileData,
  Provider,
  RangeKey,
  SettingsData,
  ThreadContext,
} from "../types";
import {
  mockActivity,
  mockBrowsers,
  mockDashboard,
  mockProfile,
  mockSettings,
} from "./mockData";

const isTauri = () => "__TAURI_INTERNALS__" in window;
export const isDesktopRuntime = isTauri;

function browserPreview(url: string): ActivityPreview {
  try {
    const parsed = new URL(url);
    const host = parsed.hostname.replace(/^www\./, "").toLowerCase();
    let videoId: string | undefined;
    if (["youtube.com", "m.youtube.com"].includes(host)) {
      videoId = parsed.pathname === "/watch"
        ? parsed.searchParams.get("v") ?? undefined
        : parsed.pathname.match(/^\/(?:shorts|embed|live)\/([^/]+)/)?.[1];
    } else if (host === "youtu.be") {
      videoId = parsed.pathname.split("/").filter(Boolean)[0];
    }
    if (videoId && /^[A-Za-z0-9_-]{6,64}$/.test(videoId)) {
      return { kind: "youtube", url };
    }
  } catch {
    // Invalid preview URLs fall back to a metadata-only resource card.
  }
  return { kind: "link", url };
}

async function call<T>(command: string, args?: Record<string, unknown>, fallback?: T): Promise<T> {
  if (!isTauri()) {
    return fallback === undefined ? (undefined as T) : structuredClone(fallback);
  }
  return invoke<T>(command, args);
}

export const api = {
  openResource: async (url: string) => {
    if (!isTauri()) {
      const opened = window.open(url, "_blank");
      if (!opened) throw new Error("The browser blocked the new tab.");
      opened.opener = null;
      return;
    }
    await invoke<void>("open_resource", { url });
  },
  openApplication: async (appName: string) => {
    if (!isTauri()) {
      throw new Error("Opening local applications is only available in the desktop app.");
    }
    await invoke<void>("open_application", { appName });
  },
  activityPreview: (url: string) =>
    call<ActivityPreview>("get_activity_preview", { url }, browserPreview(url)),
  activityIcon: (appName: string, url?: string) =>
    call<string | null>("get_activity_icon", { appName, url }, null),
  dashboard: (range: RangeKey) =>
    call<DashboardData>("get_dashboard", { range }, { ...mockDashboard, range }),
  activity: (range: RangeKey, query = "") =>
    call<ActivityEvent[]>("get_activity_history", { range, query }, mockActivity),
  profile: () => call<ProfileData>("get_profile", undefined, mockProfile),
  settings: () => call<SettingsData>("get_settings", undefined, mockSettings),
  browserProfiles: () => call<BrowserProfile[]>("get_browser_profiles", undefined, mockBrowsers),
  bootstrapStatus: () =>
    call<BootstrapStatus>(
      "get_bootstrap_status",
      undefined,
      { phase: "not-started", importedEvents: 0, progress: 0, message: "Ready to import browser history." },
    ),
  setCollectionEnabled: (enabled: boolean) =>
    call<SettingsData>("set_collection_enabled", { enabled }, { ...mockSettings, collectionStatus: { ...mockSettings.collectionStatus, enabled } }),
  requestAccessibility: () => call<boolean>("request_accessibility_permission", undefined, false),
  setBrowserProfiles: (profileIds: string[]) =>
    call<void>("set_browser_profiles", { profileIds }, undefined),
  startBootstrap: () => call<BootstrapStatus>("start_bootstrap", undefined, undefined),
  reimportChromeHistory: () =>
    call<ProfileData>("reimport_chrome_history", undefined, mockProfile),
  refreshProfile: () => call<ProfileData>("refresh_profile", undefined, mockProfile),
  saveCorrection: (label: string, description?: string, id?: string) =>
    call<ProfileData>("save_profile_correction", { id, label, description }, mockProfile),
  removeCorrection: (id: string) =>
    call<ProfileData>("remove_profile_correction", { id }, mockProfile),
  dismissInference: (id: string) =>
    call<ProfileData>("dismiss_profile_inference", { id }, mockProfile),
  saveProfileSummary: (summary: string) =>
    call<ProfileData>("save_profile_summary", { summary }, { ...mockProfile, summary }),
  saveProviderKey: (provider: Provider, key: string) =>
    call<void>("save_provider_key", { provider, key }, undefined),
  removeProviderKey: (provider: Provider) =>
    call<void>("remove_provider_key", { provider }, undefined),
  testProvider: (provider: Provider) =>
    call<string>("test_provider", { provider }, "Connection successful."),
  saveSettings: (settings: Partial<SettingsData>) =>
    call<SettingsData>("save_settings", { settings }, { ...mockSettings, ...settings }),
  dismissRecommendation: (id: string, feedback?: string) =>
    call<void>("dismiss_recommendation", { id, feedback }, undefined),
  recordProductEvent: (eventType: string, threadId?: string) =>
    call<void>("record_product_event", { eventType, threadId }, undefined),
  chat: (messages: ChatMessage[], mode: ChatMode = "optimized", threadContext?: ThreadContext) =>
    call<ChatRunResult>(
      "chat",
      { messages, mode, threadContext },
      {
        message: {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            "I’m running in browser preview mode, so this is a sample Knov answer. The native app retrieves relevant profile memories and records aggregate context economics locally.",
          createdAt: new Date().toISOString(),
        },
        retrievedMemories: [
          {
            id: "preview-memory",
            text: "Prefers local-first architecture and explicit privacy boundaries.",
            memoryType: "preference",
            source: "preview",
            createdAt: Math.floor(Date.now() / 1000),
            score: 0.94,
          },
        ],
        economics: {
          queryId: crypto.randomUUID(),
          mode,
          model: "preview-model",
          baselineInputTokens: 3842,
          optimizedInputTokens: 721,
          tokensSaved: 3121,
          reductionPercent: 81.23,
          outputTokens: 84,
          latencyMs: 620,
          memoryCount: 1,
          contextBudgetTokens: 6000,
          contextEstimatedTokens: 721,
          contextUnitsConsidered: 8,
          contextUnitsSent: 8,
          contextUnitsOmitted: 0,
          contextDetailLevel: "selected-event-metadata",
          measurementMethod: "preview_sample",
          telemetryStatus: "preview-only",
          baselineContextPreview: "Sample full profile and summarized activity context.",
          optimizedContextPreview: "Sample local profile memory plus compact query-specific local activity facts.",
        },
      },
    ),
  deleteAllData: () => call<void>("delete_all_data", undefined, undefined),
};
