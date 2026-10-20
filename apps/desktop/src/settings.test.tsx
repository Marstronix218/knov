import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";
import { mockBrowsers, mockDashboard, mockPredictionDashboard, mockProfile, mockSettings } from "./lib/mockData";
import type { SettingsData } from "./types";

function clone<T>(value: T): T {
  return structuredClone(value);
}

function stubApi(settings: Partial<SettingsData> = {}) {
  sessionStorage.clear();
  vi.spyOn(api, "settings").mockResolvedValue({ ...clone(mockSettings), ...settings });
  vi.spyOn(api, "predictionsDashboard").mockResolvedValue(clone(mockPredictionDashboard));
  vi.spyOn(api, "predictionHistory").mockResolvedValue(clone(mockPredictionDashboard.predictions));
  vi.spyOn(api, "activityIcon").mockResolvedValue(null);
  vi.spyOn(api, "dashboard").mockResolvedValue(clone(mockDashboard));
  vi.spyOn(api, "activity").mockResolvedValue(clone(mockDashboard.recentActivity));
  vi.spyOn(api, "profile").mockResolvedValue(clone(mockProfile));
  vi.spyOn(api, "browserProfiles").mockResolvedValue(clone(mockBrowsers));
  vi.spyOn(api, "recordProductEvent").mockResolvedValue(undefined);
  vi.spyOn(api, "saveSettings").mockImplementation(async (changes) => ({ ...clone(mockSettings), ...settings, ...changes }));
  vi.spyOn(api, "getPairingInfo").mockResolvedValue({
    nativeHost: "com.knov.companion",
    pairingToken: "pairing-token-123",
    localhostEndpoint: "http://127.0.0.1:48321",
    protocolVersion: 1,
    bridgeEnabled: true,
  });
  vi.spyOn(api, "installNativeHost").mockResolvedValue("/Users/me/Library/NativeMessagingHosts/com.knov.companion.json");
}

async function renderRoute(hash: string) {
  window.location.hash = hash;
  render(<App />);
  await screen.findByText("Knov");
}

function formFor(label: string): HTMLElement {
  return screen.getByLabelText(label).closest("form")!;
}

describe("collection sampling interval", () => {
  beforeEach(() => stubApi());

  it("saves a new interval and rejects values outside 2–300 seconds", async () => {
    await renderRoute("#/settings");
    const input = await screen.findByLabelText("Sampling interval (seconds)");
    expect(input).toHaveValue(5);
    const form = formFor("Sampling interval (seconds)");

    fireEvent.change(input, { target: { value: "1" } });
    expect(within(form).getByRole("alert")).toHaveTextContent("Enter a number from 2 to 300.");
    expect(within(form).getByRole("button", { name: "Save" })).toBeDisabled();

    // The backend stores whole seconds, so a decimal is refused instead of silently dropped.
    fireEvent.change(input, { target: { value: "12.5" } });
    expect(within(form).getByRole("alert")).toHaveTextContent("Enter a whole number from 2 to 300.");
    expect(within(form).getByRole("button", { name: "Save" })).toBeDisabled();

    fireEvent.change(input, { target: { value: "30" } });
    fireEvent.click(within(form).getByRole("button", { name: "Save" }));

    await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ samplingIntervalSeconds: 30 }));
    expect(await within(form).findByText("Saved.")).toBeInTheDocument();
    expect(within(form).getByText(/no restart needed/)).toBeInTheDocument();
  });
});

describe("prediction settings", () => {
  beforeEach(() => stubApi({ labsEnabled: true }));

  it("saves the display threshold as a fraction and the cooldown in minutes", async () => {
    await renderRoute("#/settings");
    const threshold = await screen.findByLabelText("Show on Now at confidence (%)");
    expect(threshold).toHaveValue(65);
    fireEvent.change(threshold, { target: { value: "80" } });
    fireEvent.click(within(formFor("Show on Now at confidence (%)")).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ predictionDisplayThreshold: 0.8 }));

    const cooldown = screen.getByLabelText("Minutes between prediction sets");
    expect(cooldown).toHaveValue(15);
    fireEvent.change(cooldown, { target: { value: "5" } });
    expect(within(formFor("Minutes between prediction sets")).getByRole("alert")).toHaveTextContent("Enter a number from 10 to 120.");
    fireEvent.change(cooldown, { target: { value: "45" } });
    fireEvent.click(within(formFor("Minutes between prediction sets")).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ predictionCooldownMinutes: 45 }));
  });

  // The top sample prediction is 78% confident: shown at a 70% threshold, hidden at 80%.
  const topPrediction = "Continue validating the Knov permission bridge";

  async function renderNowWithThreshold(predictionDisplayThreshold: number) {
    stubApi({ labsEnabled: true, predictionDisplayThreshold });
    vi.mocked(api.predictionsDashboard).mockResolvedValue({ ...clone(mockPredictionDashboard), enabled: true });
    await renderRoute("#/dashboard");
    await screen.findByRole("heading", { name: "Pick up where you left off." });
  }

  it("shows a prediction at or above the saved display threshold on Now", async () => {
    await renderNowWithThreshold(0.7);
    expect(await screen.findByText(topPrediction)).toBeInTheDocument();
  });

  it("hides predictions below the saved display threshold on Now", async () => {
    await renderNowWithThreshold(0.8);
    // Wait until the predictions have loaded, so absence means filtered rather than not rendered yet.
    await waitFor(() => expect(screen.queryByText("Checking for a likely next step…")).not.toBeInTheDocument());
    await waitFor(() => expect(screen.queryByText(topPrediction)).not.toBeInTheDocument());
    expect(screen.queryByRole("region", { name: "Likely next" })).not.toBeInTheDocument();
  });
});

describe("Chrome companion", () => {
  it("stays out of Settings until Labs is on", async () => {
    stubApi();
    await renderRoute("#/settings");
    await screen.findByLabelText("Sampling interval (seconds)");
    expect(screen.queryByRole("region", { name: "Chrome companion" })).not.toBeInTheDocument();
    expect(api.getPairingInfo).not.toHaveBeenCalled();
  });

  it("validates the extension ID inline before registering the native host", async () => {
    stubApi({ labsEnabled: true });
    await renderRoute("#/settings");
    const card = await screen.findByRole("region", { name: "Chrome companion" });
    expect(within(card).getByText("Not connected")).toBeInTheDocument();
    await within(card).findByLabelText("Pairing token");
    const register = within(card).getByRole("button", { name: "Register native host" });

    fireEvent.click(register);
    expect(within(card).getByRole("alert")).toHaveTextContent("32 lowercase letters from a to p");
    fireEvent.change(within(card).getByLabelText("Extension ID"), { target: { value: "ABCDEFGHIJKLMNOPABCDEFGHIJKLMNOP" } });
    fireEvent.click(register);
    expect(api.installNativeHost).not.toHaveBeenCalled();

    fireEvent.change(within(card).getByLabelText("Extension ID"), { target: { value: "abcdefghijklmnopabcdefghijklmnop" } });
    expect(within(card).queryByRole("alert")).not.toBeInTheDocument();
    fireEvent.click(register);
    await waitFor(() => expect(api.installNativeHost).toHaveBeenCalledWith("abcdefghijklmnopabcdefghijklmnop"));
    expect(await within(card).findByText(/Native host registered at .*com\.knov\.companion\.json/)).toBeInTheDocument();
  });

  it("reports a registration failure from the desktop app", async () => {
    stubApi({ labsEnabled: true });
    vi.mocked(api.installNativeHost).mockRejectedValue(new Error("The Native Messaging helper is missing from this build"));
    await renderRoute("#/settings");
    const card = await screen.findByRole("region", { name: "Chrome companion" });
    await within(card).findByLabelText("Pairing token");
    fireEvent.change(within(card).getByLabelText("Extension ID"), { target: { value: "abcdefghijklmnopabcdefghijklmnop" } });
    fireEvent.click(within(card).getByRole("button", { name: "Register native host" }));
    expect(await within(card).findByRole("alert")).toHaveTextContent("helper is missing");
  });

  it("keeps Register off until pairing info says the bridge is present", async () => {
    stubApi({ labsEnabled: true });
    vi.mocked(api.getPairingInfo).mockReturnValue(new Promise(() => undefined));
    await renderRoute("#/settings");
    const card = await screen.findByRole("region", { name: "Chrome companion" });
    expect(within(card).getByText("Loading pairing token…")).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Register native host" })).toBeDisabled();
    expect(within(card).queryByText(/doesn’t include the companion bridge/)).not.toBeInTheDocument();
  });

  it("shows the pairing token and copies it, with a fallback when the clipboard is unavailable", async () => {
    stubApi({ labsEnabled: true, collectionStatus: { ...clone(mockSettings.collectionStatus), extensionConnected: true, extensionLastSeenAt: 1_800_000_000 } });
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    await renderRoute("#/settings");
    const card = await screen.findByRole("region", { name: "Chrome companion" });
    expect(within(card).getByText("Connected")).toBeInTheDocument();
    expect(await within(card).findByLabelText("Pairing token")).toHaveValue("pairing-token-123");

    fireEvent.click(within(card).getByRole("button", { name: "Copy" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith("pairing-token-123"));
    expect(await within(card).findByText("Pairing token copied.")).toBeInTheDocument();

    Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });
    fireEvent.click(within(card).getByRole("button", { name: "Copy" }));
    expect(await within(card).findByRole("alert")).toHaveTextContent("Clipboard access is unavailable");
  });

  it("explains when this build has no companion bridge", async () => {
    stubApi({ labsEnabled: true });
    vi.mocked(api.getPairingInfo).mockResolvedValue({
      nativeHost: "com.knov.companion",
      pairingToken: "pairing-token-123",
      localhostEndpoint: "http://127.0.0.1:48321",
      protocolVersion: 1,
      bridgeEnabled: false,
    });
    await renderRoute("#/settings");
    const card = await screen.findByRole("region", { name: "Chrome companion" });
    expect(await within(card).findByText(/doesn’t include the companion bridge/)).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Register native host" })).toBeDisabled();
  });
});

describe("command menu", () => {
  beforeEach(() => stubApi());

  async function openPalette() {
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    return screen.findByRole("combobox", { name: "Search commands" });
  }

  it("attaches the selected thread when asking with context", async () => {
    await renderRoute("#/settings");
    const input = await openPalette();
    fireEvent.change(input, { target: { value: "ask with context" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(window.location.hash).toBe("#/assistant"));
    expect(await screen.findByText("Review local candidate evidence")).toBeInTheDocument();
    expect(JSON.parse(sessionStorage.getItem("knov.active-thread-context")!)).toMatchObject({ version: 1, subject: "Knov implementation" });
  });

  it("opens Ask without context when there is no thread, clearing a stale one", async () => {
    vi.mocked(api.dashboard).mockResolvedValue({ ...clone(mockDashboard), recentActivity: [], activeTopics: [] });
    sessionStorage.setItem("knov.active-thread-context", JSON.stringify({ version: 1, subject: "Old", events: [] }));
    await renderRoute("#/settings");
    const input = await openPalette();
    fireEvent.change(input, { target: { value: "ask with context" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(window.location.hash).toBe("#/assistant"));
    expect(sessionStorage.getItem("knov.active-thread-context")).toBeNull();
    expect(await screen.findByText(/Ask anything/)).toBeInTheDocument();
    expect(screen.queryByText("Review local candidate evidence")).not.toBeInTheDocument();
  });

  it("starts a new conversation, dropping the saved transcript", async () => {
    await renderRoute("#/settings");
    sessionStorage.setItem("knov.ask-transcript", JSON.stringify([
      { id: "old-question", role: "user", content: "An older question", createdAt: new Date().toISOString() },
    ]));
    const input = await openPalette();
    fireEvent.change(input, { target: { value: "ask with context" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(window.location.hash).toBe("#/assistant"));
    expect(await screen.findByText("Context: Knov implementation")).toBeInTheDocument();
    expect(screen.queryByText("An older question")).not.toBeInTheDocument();
    expect(sessionStorage.getItem("knov.ask-transcript")).toBeNull();
  });

  it("swaps the context in place when Ask is already open", async () => {
    await renderRoute("#/assistant");
    expect(await screen.findByText(/Ask anything/)).toBeInTheDocument();
    expect(screen.queryByText(/^Context:/)).not.toBeInTheDocument();

    const input = await openPalette();
    fireEvent.change(input, { target: { value: "ask with context" } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(await screen.findByText("Context: Knov implementation")).toBeInTheDocument();
    expect(screen.getByText("Review local candidate evidence")).toBeInTheDocument();
    expect(screen.getByText(/Your selected thread is ready/)).toBeInTheDocument();
    expect(window.location.hash).toBe("#/assistant");
  });

  it("shows ⌘-digit hints only for the first nine pages", async () => {
    await renderRoute("#/settings");
    await openPalette();
    const results = screen.getByRole("listbox", { name: "Commands" });
    const option = (name: string) => within(results).getByRole("option", { name: new RegExp(`^Go to ${name}`) });
    expect(option("Home")).toHaveTextContent("⌘1");
    expect(option("Activity")).toHaveTextContent("⌘9");
    expect(option("Settings").querySelector("kbd")).toBeNull();
    expect(results).not.toHaveTextContent(/⌘1\d/);
  });
});

describe("data export", () => {
  beforeEach(() => stubApi());

  it("exports to Downloads, reports the path and size, and reveals the file", async () => {
    const path = "/Users/me/Downloads/knov-export-20261020-090507.json";
    vi.spyOn(api, "exportAllData").mockResolvedValue({ path, bytes: 2_400_000, counts: { activityEvents: 1200 } });
    vi.spyOn(api, "revealInFinder").mockResolvedValue(undefined);
    await renderRoute("#/settings");
    const card = await screen.findByRole("region", { name: "Export your data" });

    fireEvent.click(within(card).getByRole("button", { name: "Export my data" }));
    expect(within(card).getByRole("button", { name: "Exporting…" })).toBeDisabled();
    expect(await within(card).findByText(`Saved 2.4 MB to ${path}`)).toBeInTheDocument();
    expect(api.exportAllData).toHaveBeenCalledTimes(1);

    fireEvent.click(within(card).getByRole("button", { name: "Show in Finder" }));
    await waitFor(() => expect(api.revealInFinder).toHaveBeenCalledWith(path));
  });

  it("shows the error when the export fails", async () => {
    vi.spyOn(api, "exportAllData").mockRejectedValue(new Error("The Downloads folder is unavailable."));
    await renderRoute("#/settings");
    const card = await screen.findByRole("region", { name: "Export your data" });

    fireEvent.click(within(card).getByRole("button", { name: "Export my data" }));
    expect(await within(card).findByRole("alert")).toHaveTextContent("The Downloads folder is unavailable.");
    expect(within(card).queryByText(/Saved/)).not.toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Export my data" })).toBeEnabled();
  });
});
