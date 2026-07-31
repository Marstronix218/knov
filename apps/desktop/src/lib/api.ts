import { invoke } from "@tauri-apps/api/core";
import type {
  ActivityEvent,
  BootstrapStatus,
  BrowserProfile,
  ChatMessage,
  DashboardData,
  PairingInfo,
  ProfileData,
  Provider,
  RangeKey,
  SettingsData,
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

async function call<T>(command: string, args?: Record<string, unknown>, fallback?: T): Promise<T> {
  if (!isTauri()) {
    return fallback === undefined ? (undefined as T) : structuredClone(fallback);
  }
  return invoke<T>(command, args);
}

export const api = {
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
  pairingInfo: () =>
    call<PairingInfo>(
      "get_pairing_info",
      undefined,
      {
        nativeHost: "com.knov.companion",
        pairingToken: "preview-only",
        localhostEndpoint: "http://127.0.0.1:48321",
        protocolVersion: 1,
      },
    ),
  installNativeHost: (extensionId: string) =>
    call<string>("install_native_host", { extensionId }, "Preview mode does not install a native host."),
  saveSettings: (settings: Partial<SettingsData>) =>
    call<SettingsData>("save_settings", { settings }, { ...mockSettings, ...settings }),
  dismissRecommendation: (id: string, feedback?: string) =>
    call<void>("dismiss_recommendation", { id, feedback }, undefined),
  chat: (messages: ChatMessage[]) =>
    call<ChatMessage>(
      "chat",
      { messages },
      {
        id: crypto.randomUUID(),
        role: "assistant",
        content:
          "I’m running in browser preview mode, so this is sample context. In the desktop app I’ll answer using your local profile and the provider you configured.",
        createdAt: new Date().toISOString(),
      },
    ),
  deleteAllData: () => call<void>("delete_all_data", undefined, undefined),
};
