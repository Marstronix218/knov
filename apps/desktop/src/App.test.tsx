import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";
import { mockBrowsers, mockDashboard, mockProfile, mockSettings } from "./lib/mockData";
import type { ActivityEvent, ChatMessage, ChatRunResult, ProfileData } from "./types";

function clone<T>(value: T): T {
  return structuredClone(value);
}

function chatRun(message: ChatMessage): ChatRunResult {
  return {
    message,
    retrievedMemories: [
      {
        id: "memory-1",
        text: "Privacy is more important than feature count.",
        memoryType: "preference",
        source: "explicit_user",
        createdAt: 1,
        score: 0.95,
      },
    ],
    economics: {
      queryId: "query-1234",
      mode: "optimized",
      model: "gpt-5-mini",
      baselineInputTokens: 1000,
      optimizedInputTokens: 250,
      tokensSaved: 750,
      reductionPercent: 75,
      outputTokens: 50,
      latencyMs: 400,
      memoryCount: 1,
      contextBudgetTokens: 3000,
      contextEstimatedTokens: 250,
      contextUnitsConsidered: 6,
      contextUnitsSent: 6,
      contextUnitsOmitted: 0,
      contextDetailLevel: "selected-event-metadata",
      measurementMethod: "provider_usage_scaled_estimate",
      telemetryStatus: "stored-locally",
      baselineContextPreview: "Full approved profile context",
      optimizedContextPreview: "Privacy is more important than feature count.\nQUERY-SPECIFIC LOCAL ACTIVITY FACTS\nProject Atlas: matched events=220",
    },
  };
}

function stubApi() {
  sessionStorage.clear();
  localStorage.removeItem("knov.selected-thread");
  vi.spyOn(api, "settings").mockResolvedValue(clone(mockSettings));
  vi.spyOn(api, "openResource").mockResolvedValue(undefined);
  vi.spyOn(api, "openApplication").mockResolvedValue(undefined);
  vi.spyOn(api, "activityIcon").mockResolvedValue(null);
  vi.spyOn(api, "dashboard").mockResolvedValue(clone(mockDashboard));
  vi.spyOn(api, "activity").mockResolvedValue(clone(mockDashboard.recentActivity));
  vi.spyOn(api, "profile").mockResolvedValue(clone(mockProfile));
  vi.spyOn(api, "browserProfiles").mockResolvedValue(clone(mockBrowsers));
  vi.spyOn(api, "setCollectionEnabled").mockImplementation(async (enabled) => ({
    ...clone(mockSettings),
    collectionStatus: { ...clone(mockSettings.collectionStatus), enabled },
  }));
  vi.spyOn(api, "saveSettings").mockImplementation(async (settings) => ({
    ...clone(mockSettings),
    ...settings,
  }));
  vi.spyOn(api, "setBrowserProfiles").mockResolvedValue(undefined);
  vi.spyOn(api, "reimportChromeHistory").mockResolvedValue(clone(mockProfile));
  vi.spyOn(api, "dismissRecommendation").mockResolvedValue(undefined);
  vi.spyOn(api, "recordProductEvent").mockResolvedValue(undefined);
  vi.spyOn(api, "refreshProfile").mockResolvedValue(clone(mockProfile));
}

async function renderRoute(hash: string) {
  window.location.hash = hash;
  render(<App />);
  await screen.findByText("Knov");
}

function dashboardWithPreviewEvents(events: ActivityEvent[]) {
  vi.mocked(api.dashboard).mockResolvedValue({
    ...clone(mockDashboard),
    activeTopics: [{ name: "Classical music", count: events.length }],
    recentActivity: events,
    recommendations: [],
  });
}

function previewEvent(
  id: string,
  startedAt: string,
  url?: string,
  pageTitle = "Classical music",
): ActivityEvent {
  return {
    id,
    appName: url ? "Google Chrome" : "Music",
    pageTitle,
    url,
    startedAt,
    durationSeconds: 300,
    topic: "Classical music",
    source: url ? "history" : "collector",
  };
}

function editorActivity(): ActivityEvent[] {
  return [
    { id: "editor-1", appName: "Visual Studio Code", pageTitle: "src/App.tsx", windowTitle: "Knov — src/App.tsx", startedAt: "2026-08-07T18:00:00.000Z", durationSeconds: 0, topic: "Software development", source: "editor" },
    { id: "editor-2", appName: "Visual Studio Code", pageTitle: "src/App.tsx", windowTitle: "Knov — src/App.tsx", startedAt: "2026-08-07T17:45:00.000Z", durationSeconds: 0, topic: "Software development", source: "editor" },
    { id: "editor-3", appName: "Visual Studio Code", pageTitle: "src/App.css", windowTitle: "Knov — src/App.css", startedAt: "2026-08-07T17:30:00.000Z", durationSeconds: 0, topic: "Software development", source: "editor" },
  ];
}

describe("application navigation", () => {
  beforeEach(stubApi);

  it("redirects unknown routes to the dashboard", async () => {
    await renderRoute("#/unknown");

    expect(await screen.findByRole("heading", { name: "Pick up where you left off." })).toBeInTheDocument();
  });

  it("navigates from the dashboard to activity history", async () => {
    await renderRoute("#/dashboard");

    fireEvent.click(screen.getByRole("link", { name: "Activity" }));

    expect(await screen.findByRole("heading", { name: "Your local timeline" })).toBeInTheDocument();
  });

  it("navigates to reconstructed work threads", async () => {
    await renderRoute("#/dashboard");

    fireEvent.click(screen.getByRole("link", { name: "Threads" }));

    expect(await screen.findByRole("heading", { name: "Your threads" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Knov implementation/ }));
    expect(screen.getByText("Thread evidence")).toBeInTheDocument();
  });
});

describe("onboarding", () => {
  beforeEach(stubApi);

  it("completes consent without changing the hook order", async () => {
    localStorage.clear();
    vi.mocked(api.recordProductEvent).mockRejectedValueOnce(new Error("local metrics unavailable"));
    window.location.hash = "";
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: /Continue/ }));
    fireEvent.click(screen.getByRole("button", { name: /Continue/ }));
    const profiles = await screen.findAllByRole("checkbox");
    fireEvent.click(profiles[0]);
    fireEvent.click(screen.getByRole("button", { name: /Continue/ }));
    fireEvent.change(screen.getByLabelText("API key"), {
      target: { value: "sk-test-only" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Build my first profile/ }));

    expect(await screen.findByRole("heading", { name: "Pick up where you left off." })).toBeInTheDocument();
    expect(localStorage.getItem("knov.setup-complete")).toBe("true");
  });
});

describe("dashboard", () => {
  beforeEach(stubApi);

  it("leads with a resumable thread and supporting context status", async () => {
    await renderRoute("#/dashboard");

    expect(await screen.findByText("Continue where you left off")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Knov implementation" })).toBeInTheDocument();
    expect(screen.getByText("6h 06m observed")).toBeInTheDocument();
    expect(screen.getByText("76.2% sustained focus")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Resume thread/ })).toBeInTheDocument();
  });

  it("opens the latest web resource through the desktop API before reporting success", async () => {
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: /Resume thread/ }));

    await waitFor(() => {
      expect(api.openResource).toHaveBeenCalledWith("https://v2.tauri.app/security/capabilities/");
      expect(api.recordProductEvent).toHaveBeenCalledWith("thread_resumed", "knov-implementation");
      expect(api.openApplication).not.toHaveBeenCalled();
      expect(screen.getByRole("status")).toHaveTextContent("Opened the latest available resource.");
    });
  });

  it("records whether the resume suggestion avoided re-explaining context", async () => {
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: "Yes, useful" }));

    await waitFor(() => {
      expect(api.recordProductEvent).toHaveBeenCalledWith(
        "thread_feedback_helpful",
        "knov-implementation",
      );
      expect(screen.getByRole("status")).toHaveTextContent(
        "feedback was stored locally",
      );
    });
  });

  it("does not claim thread feedback was stored when local persistence fails", async () => {
    vi.mocked(api.recordProductEvent).mockRejectedValueOnce(new Error("storage unavailable"));
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: "Wrong thread" }));

    expect(await screen.findByText(/could not store that feedback/i)).toBeInTheDocument();
  });

  it("shows recommendation evidence and records irrelevant guidance", async () => {
    await renderRoute("#/dashboard");

    expect(await screen.findByRole("region", { name: "Recommendations" })).toBeInTheDocument();
    expect(screen.getByText("A short reset may help")).toBeInTheDocument();
    fireEvent.click(screen.getAllByRole("button", { name: "Not relevant" })[1]);

    await waitFor(() => {
      expect(api.dismissRecommendation).toHaveBeenCalledWith(
        "recommendation-2",
        "not-relevant",
      );
    });
    expect(screen.queryByText("A short reset may help")).not.toBeInTheDocument();
  });

  it("shows recommendations before enough activity exists to form a thread", async () => {
    vi.mocked(api.dashboard).mockResolvedValue({
      ...clone(mockDashboard),
      activeTopics: [],
      recentActivity: [],
    });

    await renderRoute("#/dashboard");

    expect(await screen.findByText("No work threads yet")).toBeInTheDocument();
    expect(screen.getByText("A short reset may help")).toBeInTheDocument();
  });

  it("keeps a recommendation visible when local dismissal fails", async () => {
    vi.mocked(api.dismissRecommendation).mockRejectedValueOnce(new Error("storage unavailable"));
    await renderRoute("#/dashboard");

    fireEvent.click(screen.getAllByRole("button", { name: "Not relevant" })[0]);

    expect(await screen.findByText(/could not save that change/i)).toBeInTheDocument();
    expect(screen.getAllByText("Continue the native collector")).not.toHaveLength(0);
  });

  it("reports when the latest web resource cannot be opened", async () => {
    vi.mocked(api.openResource).mockRejectedValueOnce(new Error("open failed"));
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: /Resume thread/ }));

    expect(await screen.findByText("Could not open the latest available resource.")).toBeInTheDocument();
  });

  it("opens the most recent local application when the thread has no web resource", async () => {
    dashboardWithPreviewEvents(editorActivity());
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: /Resume thread/ }));

    await waitFor(() => {
      expect(api.openResource).not.toHaveBeenCalled();
      expect(api.openApplication).toHaveBeenCalledWith("Visual Studio Code");
      expect(screen.getByRole("status")).toHaveTextContent("Opened Visual Studio Code.");
    });
  });

  it("skips a non-reopenable URL and falls back to its local application", async () => {
    dashboardWithPreviewEvents([{
      id: "local-file",
      appName: "Visual Studio Code",
      pageTitle: "src/App.tsx",
      url: "file:///Users/nori/Desktop/Knov/apps/desktop/src/App.tsx",
      startedAt: "2026-08-07T18:00:00.000Z",
      durationSeconds: 300,
      topic: "Classical music",
      source: "collector",
    }]);
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: /Resume thread/ }));

    await waitFor(() => {
      expect(api.openResource).not.toHaveBeenCalled();
      expect(api.openApplication).toHaveBeenCalledWith("Visual Studio Code");
      expect(screen.getByRole("status")).toHaveTextContent("Opened Visual Studio Code.");
    });
  });

  it("reports when the most recent local application cannot be opened", async () => {
    dashboardWithPreviewEvents(editorActivity());
    vi.mocked(api.openApplication).mockRejectedValueOnce(new Error("open failed"));
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: /Resume thread/ }));

    await waitFor(() => {
      expect(api.openApplication).toHaveBeenCalledWith("Visual Studio Code");
      expect(screen.getByRole("status")).toHaveTextContent("Could not open Visual Studio Code.");
    });
  });

  it("keeps the context brief fallback when there is no meaningful resume target", async () => {
    vi.mocked(api.dashboard).mockResolvedValue({
      ...clone(mockDashboard),
      activeTopics: [{ name: "Classical music", count: 1 }],
      recentActivity: [{
        ...previewEvent("no-target", "2026-08-07T18:00:00.000Z", undefined),
        appName: "Knov",
      }],
      recommendations: [],
    });
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: /Resume thread/ }));

    expect(await screen.findByRole("status")).toHaveTextContent(
      "No reopenable resource or local application is available. The context brief is ready to use.",
    );
    expect(api.openResource).not.toHaveBeenCalled();
    expect(api.openApplication).not.toHaveBeenCalled();
  });

  it("lists reconstructed work threads and their evidence boundary", async () => {
    await renderRoute("#/dashboard");

    expect(await screen.findByRole("heading", { name: "Active threads" })).toBeInTheDocument();
    expect(screen.getByText("Product planning")).toBeInTheDocument();
    expect(screen.getByText("Desktop development")).toBeInTheDocument();
    expect(screen.getByText("Why this thread?")).toBeInTheDocument();
    expect(screen.getByText("Detailed activity stays local")).toBeInTheDocument();
  });

  it("shows cross-app Snowflake activity as one subject thread", async () => {
    const startedAt = new Date().toISOString();
    const snowflakeActivity: ActivityEvent[] = [
      { id: "snow-1", appName: "Google Chrome", pageTitle: "Snowflake tutorial — YouTube", url: "https://youtube.com/watch?v=snow", startedAt, durationSeconds: 600, topic: "Snowflake", source: "history" },
      { id: "snow-2", appName: "Google Chrome", pageTitle: "snowflake architecture — Google Search", searchQuery: "snowflake architecture", url: "https://google.com/search?q=snowflake", startedAt, durationSeconds: 0, topic: "Snowflake", source: "history" },
      { id: "snow-3", appName: "Google Chrome", pageTitle: "Snowsight", url: "https://app.snowflake.com/example", startedAt, durationSeconds: 300, topic: "Snowflake", source: "chrome" },
      { id: "snow-4", appName: "Preview", pageTitle: "Snowflake migration notes.pdf", startedAt, durationSeconds: 180, topic: "Snowflake", source: "collector" },
      { id: "snow-5", appName: "Cursor", pageTitle: "src/snowflake_client.rs", startedAt, durationSeconds: 0, topic: "Snowflake", source: "editor" },
    ];
    vi.mocked(api.dashboard).mockResolvedValue({
      ...clone(mockDashboard),
      activeTopics: [{ name: "Snowflake", count: 5 }],
      recentActivity: snowflakeActivity,
      recommendations: [],
    });

    await renderRoute("#/dashboard");

    expect(await screen.findByRole("heading", { name: "Snowflake" })).toBeInTheDocument();
    expect(screen.getByText("5 signals")).toBeInTheDocument();
    expect(screen.getAllByText(/across Google Chrome, Preview, Cursor/).length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: /Video research/ })).not.toBeInTheDocument();
  });

  it("formats foreground application percentages to one decimal place", async () => {
    vi.mocked(api.dashboard).mockResolvedValue({
      ...clone(mockDashboard),
      appUsage: [
        { name: "Code", seconds: 60, percentage: 58.333333333333336, color: "#58c7ff" },
        { name: "Google Chrome", seconds: 60, percentage: 41.666666666666664, color: "#adff2f" },
        { name: "Other", seconds: 0, percentage: 0, color: "#78828f" },
      ],
      siteUsage: [
        { name: "example.com", seconds: 60, percentage: 66.66666666666667, color: "#58c7ff" },
        { name: "Other", seconds: 30, percentage: 33.333333333333336, color: "#78828f" },
      ],
    });

    await renderRoute("#/dashboard");

    expect(await screen.findByText("58.3%")).toBeInTheDocument();
    expect(screen.getByText("41.7%")).toBeInTheDocument();
    expect(screen.getByText("0.0%")).toBeInTheDocument();
    expect(screen.getByText("66.7%")).toBeInTheDocument();
    expect(screen.getByText("33.3%")).toBeInTheDocument();
  });

  it("labels observed activity separately from cautious inferences", async () => {
    await renderRoute("#/dashboard");

    expect(await screen.findByText("Observed facts from this Mac")).toBeInTheDocument();
    expect(screen.getByText("Cautious inferences, not conclusions")).toBeInTheDocument();
    expect(screen.getByText("Supporting evidence, not a productivity score")).toBeInTheDocument();
  });

  it("uses local app placeholders without requesting remote favicons", async () => {
    await renderRoute("#/dashboard");

    await screen.findByRole("heading", { name: "Pick up where you left off." });
    expect(document.querySelector(".activity-row .app-token img")).not.toBeInTheDocument();
    expect(document.querySelector(".activity-row .app-token")?.textContent).toMatch(/[A-Z]/);
  });

  it("requests new dashboard data when the range changes", async () => {
    const dashboardSpy = vi.mocked(api.dashboard);
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: "7 days" }));

    await waitFor(() => expect(dashboardSpy).toHaveBeenCalledWith("7d"));
  });

  it("shows modified-file context beside the editor app in Now evidence", async () => {
    await renderRoute("#/dashboard");

    expect(await screen.findAllByText(/Visual Studio Code · Modified App\.tsx ·/)).not.toHaveLength(0);
  });
});

describe("editor file changes", () => {
  beforeEach(stubApi);

  it("renders a large timeline incrementally without discarding stored events", async () => {
    const events = Array.from({ length: 150 }, (_, index): ActivityEvent => ({
      id: `event-${index}`,
      appName: "Terminal",
      windowTitle: `Command ${index}`,
      startedAt: new Date(1_700_000_000_000 - index * 1_000).toISOString(),
      durationSeconds: 5,
      source: "collector",
    }));
    vi.mocked(api.activity).mockResolvedValue(events);

    await renderRoute("#/activity");

    expect(await screen.findByText("Showing 100 of 150 events")).toBeInTheDocument();
    expect(document.querySelectorAll(".activity-row")).toHaveLength(100);

    fireEvent.click(screen.getByRole("button", { name: "Show more" }));

    expect(document.querySelectorAll(".activity-row")).toHaveLength(150);
    expect(screen.queryByRole("button", { name: "Show more" })).not.toBeInTheDocument();
  });

  it("shows recent workspace changes when Code activity has no window metadata", async () => {
    vi.mocked(api.activity).mockResolvedValue([{
      id: "code-without-title",
      appName: "Code",
      startedAt: "2026-08-07T18:00:00.000Z",
      durationSeconds: 120,
      modifiedFiles: ["apps/desktop/src/App.tsx", "apps/desktop/src/App.css"],
      topic: "Software development",
      source: "collector",
    }]);

    await renderRoute("#/activity");

    expect(await screen.findByText("Code · Changed App.tsx, App.css")).toBeInTheDocument();
  });

  it("summarizes saved VS Code files in the activity timeline", async () => {
    const focusedEditorActivity: ActivityEvent = {
      id: "editor-focus",
      appName: "Visual Studio Code",
      windowTitle: "Knov — App.tsx",
      startedAt: "2026-08-07T17:20:00.000Z",
      durationSeconds: 3_000,
      topic: "Software development",
      source: "collector",
    };
    vi.mocked(api.activity).mockResolvedValue([focusedEditorActivity, ...editorActivity()]);

    await renderRoute("#/activity");

    const summary = await screen.findByRole("region", { name: "Saved files" });
    expect(within(summary).getByText("2 files · 3 saves")).toBeInTheDocument();
    expect(within(summary).getByText("src/App.tsx")).toBeInTheDocument();
    expect(within(summary).getByText("src/App.css")).toBeInTheDocument();
    expect(within(summary).getByText(/does not read code or compute line diffs/i)).toBeInTheDocument();
    expect(screen.getAllByText("file save")).toHaveLength(3);
    expect(screen.getByText("Visual Studio Code · Modified App.tsx, App.css")).toBeInTheDocument();
    expect(screen.getAllByText("Visual Studio Code · Modified App.tsx")).toHaveLength(2);
    expect(screen.getByText("Visual Studio Code · Modified App.css")).toBeInTheDocument();
  });

  it("shows saved files inside Software Development thread evidence", async () => {
    const events = editorActivity().map((event) => ({ ...event, topic: "Knov implementation" }));
    vi.mocked(api.dashboard).mockResolvedValue({
      ...clone(mockDashboard),
      activeTopics: [{ name: "Software development", count: events.length }],
      recentActivity: events,
      recommendations: [],
    });

    await renderRoute("#/threads");
    fireEvent.click(await screen.findByRole("button", { name: /Software development/i }));

    const summary = await screen.findByRole("region", { name: "Saved files" });
    expect(within(summary).getByText("2 files · 3 saves")).toBeInTheDocument();
    expect(within(summary).getByText("src/App.tsx")).toBeInTheDocument();
    expect(within(summary).getByText("src/App.css")).toBeInTheDocument();
  });
});

describe("dashboard activity preview", () => {
  beforeEach(stubApi);

  it("requests a preview for the latest URL in the selected thread", async () => {
    const newestUrl = "https://www.youtube.com/watch?v=moonlight";
    dashboardWithPreviewEvents([
      previewEvent("latest-no-url", "2026-08-07T18:00:00.000Z"),
      previewEvent("latest-url", "2026-08-07T17:00:00.000Z", newestUrl, "Moonlight Sonata"),
      previewEvent("older-url", "2026-08-07T16:00:00.000Z", "https://example.com/classical"),
    ]);
    const activityPreview = vi.spyOn(api, "activityPreview").mockResolvedValue({
      kind: "youtube",
      title: "Moonlight Sonata",
      url: newestUrl,
    });

    await renderRoute("#/dashboard");

    await waitFor(() => expect(activityPreview).toHaveBeenCalledWith(newestUrl));
    expect(activityPreview).toHaveBeenCalledTimes(1);
  });

  it("keeps YouTube activity link-only without contacting the recorded site", async () => {
    const url = "https://www.youtube.com/watch?v=moonlight";
    dashboardWithPreviewEvents([
      previewEvent("youtube", "2026-08-07T17:00:00.000Z", url, "YouTube page title"),
    ]);
    vi.spyOn(api, "activityPreview").mockResolvedValue({
      kind: "youtube",
      title: "Beethoven — Moonlight Sonata",
      url,
    });

    await renderRoute("#/dashboard");

    expect(await screen.findByRole("heading", { name: "Beethoven — Moonlight Sonata" })).toBeInTheDocument();
    expect(screen.getByText(/does not contact the recorded site unless you open it/i)).toBeInTheDocument();
    expect(screen.queryByTitle("Beethoven — Moonlight Sonata")).not.toBeInTheDocument();
  });

  it("keeps generic site previews non-embedded and openable", async () => {
    const url = "https://example.com/classical-music";
    dashboardWithPreviewEvents([
      previewEvent("website", "2026-08-07T17:00:00.000Z", url, "A guide to classical music"),
    ]);
    vi.spyOn(api, "activityPreview").mockResolvedValue({
      kind: "link",
      title: "A guide to classical music",
      url,
    });

    await renderRoute("#/dashboard");

    const openResource = await screen.findByRole("link", { name: "Open resource" });
    expect(openResource).toHaveAttribute("href", url);
    expect(openResource).toHaveAttribute("target", "_blank");
    expect(document.querySelector("iframe")).not.toBeInTheDocument();
  });

  it("preserves an openable resource when preview loading fails", async () => {
    const url = "https://example.com/classical-music";
    dashboardWithPreviewEvents([
      previewEvent("website", "2026-08-07T17:00:00.000Z", url, "A guide to classical music"),
    ]);
    vi.spyOn(api, "activityPreview").mockRejectedValue(new Error("preview unavailable"));

    await renderRoute("#/dashboard");

    expect(await screen.findByText(/Preview unavailable/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open resource" })).toHaveAttribute("href", url);
  });
});

describe("profile corrections", () => {
  beforeEach(stubApi);

  it("saves a correction as authoritative user-authored truth", async () => {
    const correctedProfile: ProfileData = {
      ...clone(mockProfile),
      sections: [
        ...clone(mockProfile.sections),
        {
          id: "new-truth",
          title: "Authoritative truth",
          items: [
            {
              id: "correction-2",
              label: "Project Atlas is complete",
              description: "Do not infer that it is active.",
              provenance: "user",
            },
          ],
        },
      ],
    };
    const saveCorrection = vi.spyOn(api, "saveCorrection").mockResolvedValue(correctedProfile);
    await renderRoute("#/profile");
    await screen.findByText(mockProfile.summary);

    fireEvent.click(screen.getByRole("button", { name: "Add correction" }));
    const dialog = screen.getByRole("dialog", { name: "Add authoritative correction" });
    fireEvent.change(within(dialog).getByLabelText("What should Knov know?"), {
      target: { value: "Project Atlas is complete" },
    });
    fireEvent.change(within(dialog).getByLabelText("Optional context"), {
      target: { value: "Do not infer that it is active." },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save as truth" }));

    await waitFor(() =>
      expect(saveCorrection).toHaveBeenCalledWith(
        "Project Atlas is complete",
        "Do not infer that it is active.",
      ),
    );
    expect(await screen.findByText("Project Atlas is complete")).toBeInTheDocument();
    expect(screen.getAllByText("user").length).toBeGreaterThan(0);
  });

  it("removes a correction by its stable identifier", async () => {
    const updatedProfile: ProfileData = {
      ...clone(mockProfile),
      sections: mockProfile.sections.map((section) => ({
        ...section,
        items: section.items.filter((item) => item.id !== "truth-local"),
      })),
    };
    const removeCorrection = vi.spyOn(api, "removeCorrection").mockResolvedValue(updatedProfile);
    await renderRoute("#/profile");
    await screen.findByText("Knov is local-first");

    fireEvent.click(screen.getByRole("button", { name: "Remove correction" }));

    await waitFor(() => expect(removeCorrection).toHaveBeenCalledWith("truth-local"));
    await waitFor(() => expect(screen.queryByText("Knov is local-first")).not.toBeInTheDocument());
  });
});

describe("settings privacy disclosures", () => {
  beforeEach(stubApi);

  it("states that provider keys travel directly from the Mac", async () => {
    await renderRoute("#/settings");

    expect(await screen.findByText("Your key goes directly from this Mac to the selected provider.")).toBeInTheDocument();
  });

  it("states which metadata collection includes", async () => {
    await renderRoute("#/settings");

    expect(
      await screen.findByText(
        "Foreground app, window title, selected Chrome history, and editor workspace-change metadata.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText(/Git working-tree paths/i)).toBeInTheDocument();
    expect(
      screen.getByText(/never opens saved code snapshots or source contents/i),
    ).toBeInTheDocument();
  });

  it("keeps extension pairing out of the baseline settings flow", async () => {
    await renderRoute("#/settings");

    expect(
      await screen.findByText("Select local Chrome profiles for history import and continuous backfill."),
    ).toBeInTheDocument();
    expect(screen.queryByText("Chrome companion pairing")).not.toBeInTheDocument();
    expect(screen.queryByText(/Chrome extension is disconnected/i)).not.toBeInTheDocument();
  });

  it("re-imports Chrome history and refreshes the profile as one operation", async () => {
    const reimport = vi.mocked(api.reimportChromeHistory);
    const refresh = vi.mocked(api.refreshProfile);
    await renderRoute("#/settings");

    fireEvent.click(await screen.findByRole("button", { name: "Re-import Chrome history" }));

    await waitFor(() => expect(reimport).toHaveBeenCalledOnce());
    expect(refresh).not.toHaveBeenCalled();
    expect(
      await screen.findByText("Chrome durations imported and your profile was refreshed."),
    ).toBeInTheDocument();
  });

  it("discloses the full scope of permanent deletion", async () => {
    await renderRoute("#/settings");

    expect(
      await screen.findByText(
        "Permanently removes local activity, profiles, corrections, recommendations, telemetry, settings, and provider credentials.",
      ),
    ).toBeInTheDocument();
  });
});

describe("assistant chat", () => {
  beforeEach(stubApi);

  it("sends the full visible conversation and renders the assistant response", async () => {
    const response: ChatMessage = {
      id: "assistant-response",
      role: "assistant",
      content: "Continue validating the macOS permission bridge.",
      createdAt: "2026-07-27T17:05:00.000Z",
    };
    const chat = vi.spyOn(api, "chat").mockResolvedValue(chatRun(response));
    await renderRoute("#/assistant");

    fireEvent.change(screen.getByPlaceholderText(/What should I prioritize/), {
      target: { value: "What should I work on?" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Send/ }));

    await waitFor(() => expect(chat).toHaveBeenCalledTimes(1));
    expect(chat.mock.calls[0][1]).toBe("optimized");
    const sentMessages = chat.mock.calls[0][0];
    expect(sentMessages.map(({ role, content }) => ({ role, content }))).toEqual([
      {
        role: "assistant",
        content: "Ask anything. I’ll retrieve only the approved memories relevant to this request.",
      },
      { role: "user", content: "What should I work on?" },
    ]);
    expect(await screen.findByText(response.content)).toBeInTheDocument();
    expect(screen.getByText("75.0%")).toBeInTheDocument();
    expect(screen.getAllByText("Privacy is more important than feature count.").length).toBeGreaterThan(0);
    expect(screen.getByText("stored-locally")).toBeInTheDocument();
  });

  it("uses one query-complete answer path while preserving the full-context comparison", async () => {
    const response: ChatMessage = {
      id: "baseline-response",
      role: "assistant",
      content: "Baseline answer.",
      createdAt: "2026-07-27T17:05:00.000Z",
    };
    const chat = vi.spyOn(api, "chat").mockResolvedValue(chatRun(response));
    await renderRoute("#/assistant");

    fireEvent.change(screen.getByPlaceholderText(/What should I prioritize/), {
      target: { value: "Compare this." },
    });
    fireEvent.click(screen.getByRole("button", { name: /Send/ }));

    await waitFor(() => expect(chat).toHaveBeenCalled());
    expect(chat.mock.calls[0][1]).toBe("optimized");
    expect(screen.queryByRole("button", { name: "Full Context" })).not.toBeInTheDocument();
    expect(await screen.findByText("Baseline answer.")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Compare context payloads"));
    expect(screen.getByText(/QUERY-SPECIFIC LOCAL ACTIVITY FACTS/)).toBeInTheDocument();
  });

  it("passes the selected thread as inspectable local candidate evidence", async () => {
    const response: ChatMessage = {
      id: "context-response",
      role: "assistant",
      content: "I used the selected thread.",
      createdAt: "2026-07-27T17:05:00.000Z",
    };
    const chat = vi.spyOn(api, "chat").mockResolvedValue(chatRun(response));
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("link", { name: /Ask with context/ }));
    expect(await screen.findByText("Review local candidate evidence")).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText(/What should I prioritize/), { target: { value: "What next?" } });
    fireEvent.click(screen.getByRole("button", { name: /Send/ }));

    await waitFor(() => expect(chat).toHaveBeenCalledOnce());
    const sentMessages = chat.mock.calls[0][0];
    expect(sentMessages[sentMessages.length - 1]?.content).toBe("What next?");
    expect(chat.mock.calls[0][2]).toMatchObject({
      version: 1,
      subject: "Knov implementation",
      signalCount: expect.any(Number),
      modifiedFiles: ["apps/desktop/src/App.tsx"],
    });
    expect(screen.getByText(/Recent modified files \(newest first\): apps\/desktop\/src\/App\.tsx/)).toBeInTheDocument();
    expect(chat.mock.calls[0][2]?.events[0]).toMatchObject({
      appName: expect.any(String),
      source: expect.any(String),
    });
    expect(chat.mock.calls[0][2]?.events.some((event) => event.title === "Tauri 2 — Security Capabilities")).toBe(true);
    expect(chat.mock.calls[0][2]?.events.some((event) => event.resource === "v2.tauri.app/security/capabilities/")).toBe(true);
    expect(chat.mock.calls[0][2]?.events.every((event) => !event.resource?.includes("?"))).toBe(true);
  });

  it("sends a message when Enter is pressed", async () => {
    const response: ChatMessage = {
      id: "assistant-response",
      role: "assistant",
      content: "Sent from the keyboard.",
      createdAt: "2026-07-27T17:05:00.000Z",
    };
    const chat = vi.spyOn(api, "chat").mockResolvedValue(chatRun(response));
    await renderRoute("#/assistant");

    const composer = screen.getByPlaceholderText(/What should I prioritize/);
    fireEvent.change(composer, { target: { value: "Send this with Enter" } });
    fireEvent.keyDown(composer, { key: "Enter", code: "Enter" });

    await waitFor(() => expect(chat).toHaveBeenCalledTimes(1));
    const sentMessages = chat.mock.calls[0][0];
    expect(sentMessages[sentMessages.length - 1]?.content).toBe("Send this with Enter");
    expect(await screen.findByText(response.content)).toBeInTheDocument();
  });

  it("inserts a newline with Shift+Enter instead of sending", async () => {
    const chat = vi.spyOn(api, "chat");
    await renderRoute("#/assistant");

    const composer = screen.getByPlaceholderText(/What should I prioritize/);
    fireEvent.change(composer, { target: { value: "First line" } });
    fireEvent.keyDown(composer, { key: "Enter", code: "Enter", shiftKey: true });

    expect(chat).not.toHaveBeenCalled();
  });

  it("does not send whitespace-only messages", async () => {
    const chat = vi.spyOn(api, "chat");
    await renderRoute("#/assistant");

    fireEvent.change(screen.getByPlaceholderText(/What should I prioritize/), {
      target: { value: "   " },
    });

    expect(screen.getByRole("button", { name: /Send/ })).toBeDisabled();
    expect(chat).not.toHaveBeenCalled();
  });

  it("renders assistant Markdown as structured content", async () => {
    const response: ChatMessage = {
      id: "markdown-response",
      role: "assistant",
      content: [
        "Here are the **next steps**:",
        "",
        "## Immediate",
        "",
        "- Create a short TODO list",
        "- Update the README",
        "",
        "1. Run the app",
        "2. Check the tests",
      ].join("\n"),
      createdAt: "2026-07-27T17:05:00.000Z",
    };
    vi.spyOn(api, "chat").mockResolvedValue(chatRun(response));
    await renderRoute("#/assistant");

    fireEvent.change(screen.getByPlaceholderText(/What should I prioritize/), {
      target: { value: "What should I work on?" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Send/ }));

    expect(await screen.findByRole("heading", { name: "Immediate" })).toBeInTheDocument();
    expect(screen.getByText("next steps").tagName).toBe("STRONG");

    const lists = screen.getAllByRole("list");
    expect(lists).toHaveLength(2);
    expect(within(lists[0]).getAllByRole("listitem")).toHaveLength(2);
    expect(within(lists[1]).getAllByRole("listitem")).toHaveLength(2);
  });
});
