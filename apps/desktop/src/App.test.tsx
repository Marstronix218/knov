import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";
import { mockBrowsers, mockDashboard, mockProfile, mockSettings } from "./lib/mockData";
import type { ChatMessage, ProfileData } from "./types";

function clone<T>(value: T): T {
  return structuredClone(value);
}

function stubApi() {
  vi.spyOn(api, "settings").mockResolvedValue(clone(mockSettings));
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
  vi.spyOn(api, "refreshProfile").mockResolvedValue(clone(mockProfile));
}

async function renderRoute(hash: string) {
  window.location.hash = hash;
  render(<App />);
  await screen.findByText("Knov");
}

describe("application navigation", () => {
  beforeEach(stubApi);

  it("redirects unknown routes to the dashboard", async () => {
    await renderRoute("#/unknown");

    expect(await screen.findByRole("heading", { name: "Good afternoon." })).toBeInTheDocument();
  });

  it("navigates from the dashboard to activity history", async () => {
    await renderRoute("#/dashboard");

    fireEvent.click(screen.getByRole("link", { name: "Activity" }));

    expect(await screen.findByRole("heading", { name: "Your local timeline" })).toBeInTheDocument();
  });
});

describe("onboarding", () => {
  beforeEach(stubApi);

  it("completes consent without changing the hook order", async () => {
    localStorage.clear();
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

    expect(await screen.findByRole("heading", { name: "Good afternoon." })).toBeInTheDocument();
    expect(localStorage.getItem("knov.setup-complete")).toBe("true");
  });
});

describe("dashboard", () => {
  beforeEach(stubApi);

  it("renders tracked time separately from sustained foreground sessions", async () => {
    await renderRoute("#/dashboard");

    const trackedMetric = (await screen.findByText("Tracked time")).closest("article");
    const focusedMetric = screen.getByText("Sustained focus").closest("article");

    expect(trackedMetric).toHaveTextContent("6h 06m");
    expect(trackedMetric).toHaveTextContent("Live foreground activity · idle excluded");
    expect(focusedMetric).toHaveTextContent("4h 39m");
    expect(focusedMetric).toHaveTextContent("76.2% of tracked · sessions 5m+");
  });

  it("lists the active topic names", async () => {
    await renderRoute("#/dashboard");

    const topics = (await screen.findByText("Active topics")).closest("article");
    expect(topics).toHaveTextContent("Software development");
    expect(topics).toHaveTextContent("Planning and notes");
    expect(topics).toHaveTextContent("Web research");
    expect(topics).toHaveTextContent("Inferred from app, title, and domain signals");
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
    expect(screen.getByText("Facts and cautious inferences")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Recommendations" })).toBeInTheDocument();
  });

  it("shows website favicons instead of letter placeholders", async () => {
    await renderRoute("#/dashboard");

    await waitFor(() => {
      const favicon = document.querySelector<HTMLImageElement>(".activity-row .app-token img");
      expect(favicon?.src).toBe("https://v2.tauri.app/favicon.ico");
    });
  });

  it("requests new dashboard data when the range changes", async () => {
    const dashboardSpy = vi.mocked(api.dashboard);
    await renderRoute("#/dashboard");

    fireEvent.click(await screen.findByRole("button", { name: "7 days" }));

    await waitFor(() => expect(dashboardSpy).toHaveBeenCalledWith("7d"));
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

    expect(await screen.findByText("Foreground app, window title, and permitted browser metadata.")).toBeInTheDocument();
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
        "Permanently removes activity, profiles, corrections, recommendations, settings, and provider credentials.",
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
    const chat = vi.spyOn(api, "chat").mockResolvedValue(response);
    await renderRoute("#/assistant");

    fireEvent.change(screen.getByPlaceholderText("What should I focus on next?"), {
      target: { value: "What should I work on?" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Send/ }));

    await waitFor(() => expect(chat).toHaveBeenCalledTimes(1));
    const sentMessages = chat.mock.calls[0][0];
    expect(sentMessages.map(({ role, content }) => ({ role, content }))).toEqual([
      {
        role: "assistant",
        content: "I’m ready. I’ll use your local profile naturally and I’ll distinguish what you told me from what I inferred.",
      },
      { role: "user", content: "What should I work on?" },
    ]);
    expect(await screen.findByText(response.content)).toBeInTheDocument();
  });

  it("sends a message when Enter is pressed", async () => {
    const response: ChatMessage = {
      id: "assistant-response",
      role: "assistant",
      content: "Sent from the keyboard.",
      createdAt: "2026-07-27T17:05:00.000Z",
    };
    const chat = vi.spyOn(api, "chat").mockResolvedValue(response);
    await renderRoute("#/assistant");

    const composer = screen.getByPlaceholderText("What should I focus on next?");
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

    const composer = screen.getByPlaceholderText("What should I focus on next?");
    fireEvent.change(composer, { target: { value: "First line" } });
    fireEvent.keyDown(composer, { key: "Enter", code: "Enter", shiftKey: true });

    expect(chat).not.toHaveBeenCalled();
  });

  it("does not send whitespace-only messages", async () => {
    const chat = vi.spyOn(api, "chat");
    await renderRoute("#/assistant");

    fireEvent.change(screen.getByPlaceholderText("What should I focus on next?"), {
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
    vi.spyOn(api, "chat").mockResolvedValue(response);
    await renderRoute("#/assistant");

    fireEvent.change(screen.getByPlaceholderText("What should I focus on next?"), {
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
