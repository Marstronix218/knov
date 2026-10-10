import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";
import {
  mockAgentOverview,
  mockAutonomy,
  mockBrowsers,
  mockDashboard,
  mockPredictionDashboard,
  mockProfile,
  mockRuns,
  mockSettings,
  mockSkills,
  mockWorkflows,
} from "./lib/mockData";
import type { AgentRun } from "./types";

function clone<T>(value: T): T {
  return structuredClone(value);
}

function manualRun(): AgentRun {
  const run = clone(mockRuns[0]);
  return {
    ...run,
    id: "run-manual",
    origin: "manual",
    status: "awaiting_approval",
    actions: run.actions.slice(0, 2),
  };
}

function stubApi() {
  sessionStorage.clear();
  localStorage.clear();
  localStorage.setItem("knov.setup-complete", "true");
  vi.restoreAllMocks();
  // The work agent and workflow pages are Labs features.
  vi.spyOn(api, "settings").mockResolvedValue({ ...clone(mockSettings), labsEnabled: true });
  vi.spyOn(api, "dashboard").mockResolvedValue(clone(mockDashboard));
  vi.spyOn(api, "activity").mockResolvedValue(clone(mockDashboard.recentActivity));
  vi.spyOn(api, "profile").mockResolvedValue(clone(mockProfile));
  vi.spyOn(api, "browserProfiles").mockResolvedValue(clone(mockBrowsers));
  vi.spyOn(api, "predictionsDashboard").mockResolvedValue(clone(mockPredictionDashboard));
  vi.spyOn(api, "predictionHistory").mockResolvedValue(clone(mockPredictionDashboard.predictions));
  vi.spyOn(api, "activityIcon").mockResolvedValue(null);
  vi.spyOn(api, "recordProductEvent").mockResolvedValue(undefined);
  vi.spyOn(api, "agentOverview").mockResolvedValue(clone(mockAgentOverview));
  vi.spyOn(api, "workflows").mockResolvedValue(clone(mockWorkflows));
  vi.spyOn(api, "skills").mockResolvedValue(clone(mockSkills));
  vi.spyOn(api, "autonomy").mockResolvedValue(clone(mockAutonomy));
  vi.spyOn(api, "agentRuns").mockResolvedValue(clone(mockRuns));
  vi.spyOn(api, "setCollectionEnabled").mockImplementation(async (enabled) => ({
    ...clone(mockSettings),
    collectionStatus: { ...clone(mockSettings.collectionStatus), enabled },
  }));
}

async function renderRoute(hash: string) {
  window.location.hash = hash;
  render(<App />);
  await screen.findByText("Knov");
}

describe("Now: Ready for you", () => {
  beforeEach(stubApi);

  it("shows the inferred goal with its evidence and records a confirmation", async () => {
    const reviewGoal = vi.spyOn(api, "reviewGoal").mockResolvedValue(clone(mockAgentOverview));
    await renderRoute("#/dashboard");

    const inbox = await screen.findByRole("region", { name: "Ready for you" });
    expect(within(inbox).getByText("Ship the Knov desktop alpha")).toBeInTheDocument();
    expect(within(inbox).getByText("Inferred · 83%")).toBeInTheDocument();
    expect(within(inbox).getByText("Active on 9 of the last 14 days")).toBeInTheDocument();

    fireEvent.click(within(inbox).getByRole("button", { name: "Confirm goal" }));
    await waitFor(() => expect(reviewGoal).toHaveBeenCalledWith("goal-knov-desktop", "confirmed"));
  });

  it("approves selected actions, remembers a permission, and declines the rest", async () => {
    const decideRun = vi.spyOn(api, "decideRun").mockImplementation(async (runId) => ({
      ...clone(mockRuns[0]),
      id: runId,
      status: "completed",
      actions: [],
    }));
    await renderRoute("#/dashboard");

    const inbox = await screen.findByRole("region", { name: "Ready for you" });
    fireEvent.click(within(inbox).getByRole("button", { name: "Review" }));
    const dialog = await screen.findByRole("dialog", { name: "Build and test · Knov desktop" });
    expect(within(dialog).getByRole("region", { name: "What Knov believed" })).toHaveTextContent("Ship the Knov desktop alpha");
    expect(within(dialog).getByText("Checks failed: `npm test` exited with 1")).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("checkbox", { name: "Approve Edit code in Visual Studio Code" }));
    fireEvent.click(within(dialog).getAllByRole("checkbox", { name: /Allow this automatically/ })[0]);
    fireEvent.click(within(dialog).getByRole("button", { name: /Approve and run 1 action/ }));

    await waitFor(() => expect(decideRun).toHaveBeenCalledWith("run-context-1", [
      { actionId: "act-open-issue", approved: true, remember: true },
      { actionId: "act-open-editor", approved: false, remember: false },
    ]));
  });

  it("closes an untouched manual preview instead of leaving it pending", async () => {
    vi.spyOn(api, "agentOverview").mockResolvedValue({
      ...clone(mockAgentOverview),
      attention: [manualRun()],
    });
    const cancelRun = vi.spyOn(api, "cancelRun").mockResolvedValue({ ...manualRun(), status: "cancelled" });
    const decideRun = vi.spyOn(api, "decideRun");
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: "Review" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Close dialog" }));

    await waitFor(() => expect(cancelRun).toHaveBeenCalledWith("run-manual"));
    expect(decideRun).not.toHaveBeenCalled();
  });

  it("stages the next workflow steps through a reviewed run", async () => {
    vi.spyOn(api, "agentOverview").mockResolvedValue({ ...clone(mockAgentOverview), attention: [] });
    const preview = vi.spyOn(api, "previewSkillRun").mockResolvedValue(manualRun());
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: "Stage next steps" }));

    await waitFor(() => expect(preview).toHaveBeenCalledWith("skill-build-test"));
    expect(await screen.findByRole("button", { name: /Approve and run 2 actions/ })).toBeInTheDocument();
  });
});

describe("Workflows", () => {
  beforeEach(stubApi);

  it("lists opportunities with their score and evidence boundary", async () => {
    await renderRoute("#/workflows");

    const card = await screen.findByRole("article", { name: "Research loop · Tauri security" });
    expect(within(card).getByLabelText("Opportunity score 52%")).toBeInTheDocument();
    expect(within(card).getByText(/Titles and page contents are never used/)).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "Mail → github.com → Slack" })).not.toBeInTheDocument();
  });

  it("dismisses a workflow the user does not recognize", async () => {
    const review = vi.spyOn(api, "reviewWorkflow").mockImplementation(async (id, status) => ({
      ...clone(mockWorkflows.find((workflow) => workflow.id === id)!),
      status,
    }));
    await renderRoute("#/workflows");

    const card = await screen.findByRole("article", { name: "Reporting routine" });
    fireEvent.click(within(card).getByRole("button", { name: "Not a workflow" }));

    await waitFor(() => expect(review).toHaveBeenCalledWith("wf-friday-report", "dismissed", undefined));
    await waitFor(() => expect(screen.queryByRole("article", { name: "Reporting routine" })).not.toBeInTheDocument());
  });

  it("confirms a workflow, creates a skill, and opens it for editing", async () => {
    const created = { ...clone(mockSkills[0]), id: "skill-new", name: "Research loop · Tauri security", workflowId: "wf-research" };
    const createSkill = vi.spyOn(api, "createSkill").mockResolvedValue(created);
    const updateSkill = vi.spyOn(api, "updateSkill").mockImplementation(async (update) => ({ ...created, name: update.name, trigger: update.trigger }));
    await renderRoute("#/workflows");

    const card = await screen.findByRole("article", { name: "Research loop · Tauri security" });
    fireEvent.click(within(card).getByRole("button", { name: /Confirm and create skill/ }));
    await waitFor(() => expect(createSkill).toHaveBeenCalledWith("wf-research"));

    const editor = await screen.findByRole("dialog", { name: "Edit skill: Research loop · Tauri security" });
    fireEvent.click(within(editor).getByRole("radio", { name: "On a schedule" }));
    fireEvent.change(within(editor).getByRole("combobox", { name: "Day" }), { target: { value: "4" } });
    fireEvent.change(within(editor).getByRole("combobox", { name: "Time" }), { target: { value: "16" } });
    fireEvent.click(within(editor).getByRole("button", { name: /Save skill/ }));

    await waitFor(() => expect(updateSkill).toHaveBeenCalledWith(expect.objectContaining({
      id: "skill-new",
      trigger: { kind: "schedule", hour: 16, weekday: 4 },
    })));
    expect(await screen.findByText("Saved “Research loop · Tauri security”.")).toBeInTheDocument();
  });

  it("shows a helpful empty state when nothing repeats yet", async () => {
    vi.spyOn(api, "workflows").mockResolvedValue([]);
    await renderRoute("#/workflows");

    expect(await screen.findByText("No automation opportunities yet")).toBeInTheDocument();
  });
});

describe("Delegated work", () => {
  beforeEach(stubApi);

  it("pauses the agent with the kill switch", async () => {
    const pause = vi.spyOn(api, "setAgentPaused").mockResolvedValue({ ...clone(mockAutonomy), paused: true });
    vi.mocked(api.autonomy).mockResolvedValueOnce(clone(mockAutonomy)).mockResolvedValue({ ...clone(mockAutonomy), paused: true });
    await renderRoute("#/agent");

    fireEvent.click(await screen.findByRole("button", { name: "Pause agent" }));

    await waitFor(() => expect(pause).toHaveBeenCalledWith(true));
    expect(await screen.findByText("Agent paused")).toBeInTheDocument();
  });

  it("revokes an active permission", async () => {
    const revoke = vi.spyOn(api, "revokeGrant").mockResolvedValue({
      ...clone(mockAutonomy),
      grants: mockAutonomy.grants.filter((grant) => grant.id !== "grant-checks"),
    });
    await renderRoute("#/agent");

    fireEvent.click(await screen.findByRole("button", { name: "Permissions" }));
    const table = await screen.findByRole("table", { name: "Active permissions" });
    fireEvent.click(within(table).getAllByRole("button", { name: "Revoke" })[0]);

    await waitFor(() => expect(revoke).toHaveBeenCalledWith("grant-checks"));
    expect(await screen.findByText("Permission revoked.")).toBeInTheDocument();
  });

  it("surfaces validation errors when approving a workspace", async () => {
    vi.spyOn(api, "approveWorkspace").mockRejectedValue(new Error("No supported test setup was found."));
    await renderRoute("#/agent");

    fireEvent.click(await screen.findByRole("button", { name: "Permissions" }));
    fireEvent.change(await screen.findByLabelText("Folder path"), { target: { value: "~/Desktop" } });
    fireEvent.click(screen.getByRole("button", { name: "Approve folder" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("No supported test setup was found.");
  });
});

describe("app-wide experience", () => {
  beforeEach(stubApi);

  it("opens the command menu with ⌘K and navigates from the keyboard", async () => {
    await renderRoute("#/dashboard");

    fireEvent.keyDown(window, { key: "k", metaKey: true });
    const input = await screen.findByRole("combobox", { name: "Search commands" });
    fireEvent.change(input, { target: { value: "workflows" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(window.location.hash).toBe("#/workflows"));
    expect(screen.queryByRole("dialog", { name: "Command menu" })).not.toBeInTheDocument();
  });

  it("keeps the sidebar in sync with the Settings collection toggle", async () => {
    await renderRoute("#/settings");

    const sidebarStatus = () => document.querySelector(".capture-status")?.textContent;
    await waitFor(() => expect(sidebarStatus()).toBe("Collection active"));
    fireEvent.click(await screen.findByRole("checkbox", { name: /Collection active/ }));

    await waitFor(() => expect(api.setCollectionEnabled).toHaveBeenCalledWith(false));
    await waitFor(() => expect(sidebarStatus()).toBe("Collection paused"));
  });

  it("reports provider connection failures instead of failing silently", async () => {
    vi.mocked(api.settings).mockResolvedValue({ ...clone(mockSettings), provider: "openai", hasProviderKey: true, aiConfigured: true });
    vi.spyOn(api, "testProvider").mockRejectedValue(new Error("The API key is invalid or revoked: unauthorized"));
    await renderRoute("#/settings");

    fireEvent.click(await screen.findByRole("button", { name: /Test connection/ }));

    expect(await screen.findByRole("alert")).toHaveTextContent("unauthorized");
  });

  it("lets setup finish without an AI provider using only local context", async () => {
    localStorage.clear();
    window.location.hash = "";
    const local = vi.spyOn(api, "startLocalBootstrap").mockResolvedValue({ phase: "complete", importedEvents: 4, progress: 100, message: "ready" });
    const bootstrap = vi.spyOn(api, "startBootstrap");
    vi.spyOn(api, "setBrowserProfiles").mockResolvedValue(undefined);
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: /Continue/ }));
    fireEvent.click(screen.getByRole("button", { name: /Continue/ }));
    fireEvent.click((await screen.findAllByRole("checkbox"))[0]);
    fireEvent.click(screen.getByRole("button", { name: /Continue/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Skip AI and finish/ }));

    await waitFor(() => expect(local).toHaveBeenCalledOnce());
    expect(bootstrap).not.toHaveBeenCalled();
    expect(await screen.findByRole("heading", { name: "Pick up where you left off." })).toBeInTheDocument();
  });
});

describe("browser profiles in Settings", () => {
  beforeEach(stubApi);

  it("adds a profile even when a previously selected one was deleted in Chrome", async () => {
    vi.mocked(api.settings).mockResolvedValue({ ...clone(mockSettings), selectedBrowserProfileIds: ["Profile 7", "chrome-default"] });
    const setProfiles = vi.spyOn(api, "setBrowserProfiles").mockResolvedValue(undefined);
    await renderRoute("#/settings");

    expect(await screen.findByRole("note")).toHaveTextContent("no longer exists in Chrome (Profile 7)");
    const work = await screen.findByRole("checkbox", { name: /ID: chrome-profile-1/ });
    expect(work).not.toBeChecked();
    fireEvent.click(work);

    await waitFor(() => expect(setProfiles).toHaveBeenCalledWith(["chrome-default", "chrome-profile-1"]));
    await waitFor(() => expect(work).toBeChecked());
    expect(screen.getByText(/Added Work/)).toBeInTheDocument();
  });

  it("shows why a profile change failed and keeps the previous selection", async () => {
    vi.mocked(api.settings).mockResolvedValue({ ...clone(mockSettings), selectedBrowserProfileIds: ["chrome-default"] });
    vi.spyOn(api, "setBrowserProfiles").mockRejectedValue(new Error("invalid input: A selected Chrome profile is unavailable."));
    await renderRoute("#/settings");

    const work = await screen.findByRole("checkbox", { name: /ID: chrome-profile-1/ });
    fireEvent.click(work);

    expect(await screen.findByRole("alert")).toHaveTextContent("A selected Chrome profile is unavailable.");
    expect(work).not.toBeChecked();
  });

  it("lets the last profile be unchecked because browser history is optional", async () => {
    vi.mocked(api.settings).mockResolvedValue({ ...clone(mockSettings), selectedBrowserProfileIds: ["chrome-default"] });
    const setProfiles = vi.spyOn(api, "setBrowserProfiles").mockResolvedValue(undefined);
    await renderRoute("#/settings");

    const defaultProfile = await screen.findByRole("checkbox", { name: /ID: chrome-default/ });
    fireEvent.click(defaultProfile);

    await waitFor(() => expect(setProfiles).toHaveBeenCalledWith([]));
    await waitFor(() => expect(defaultProfile).not.toBeChecked());
  });
});

describe("tester feedback", () => {
  beforeEach(stubApi);

  it("composes reviewable feedback with anonymous counts and opens it outside Knov", async () => {
    vi.spyOn(api, "testerSummary").mockResolvedValue({
      appVersion: "0.2.0",
      macOS: "26.0",
      aiProvider: "local",
      browserProfiles: 1,
      labsEnabled: false,
      daysWithActivity: 4,
      daysSinceFirstActivity: 6,
      questionsAsked: 9,
      events: { thread_resumed: 3 },
    });
    const open = vi.spyOn(api, "openResource").mockResolvedValue(undefined);
    await renderRoute("#/dashboard");

    fireEvent.click(screen.getByRole("button", { name: "Send feedback" }));
    const dialog = await screen.findByRole("dialog", { name: "Share feedback" });
    fireEvent.click(within(dialog).getByLabelText("Very disappointed"));
    fireEvent.change(within(dialog).getByLabelText(/main benefit/), { target: { value: "No more re-explaining" } });
    fireEvent.click(within(dialog).getByText("See exactly what’s included"));
    expect(within(dialog).getByText(/Questions asked: 9/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: /Send feedback/ }));

    await waitFor(() => expect(open).toHaveBeenCalledOnce());
    const url = decodeURIComponent(open.mock.calls[0][0]);
    expect(url).toContain("Very disappointed");
    expect(url).toContain("No more re-explaining");
    expect(url).toContain("thread resumed: 3");
  });
});
