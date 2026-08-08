import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";

describe("browser preview API", () => {
  beforeEach(() => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it("returns dashboard preview data for the requested range", async () => {
    const dashboard = await api.dashboard("30d");

    expect(dashboard.range).toBe("30d");
  });

  it("returns independent preview objects across dashboard requests", async () => {
    const first = await api.dashboard("today");
    first.appUsage[0].name = "Changed by test";

    const second = await api.dashboard("today");

    expect(second.appUsage[0].name).toBe("Chrome");
  });

  it("reflects the requested collection state in its preview response", async () => {
    const settings = await api.setCollectionEnabled(false);

    expect(settings.collectionStatus.enabled).toBe(false);
  });

  it("does not synthesize remote favicon requests in browser preview", async () => {
    await expect(api.activityIcon("Chrome", "https://private.example/account")).resolves.toBeNull();
  });

  it("opens resources in a protected browser-preview tab", async () => {
    const openedWindow = { opener: window };
    const open = vi.spyOn(window, "open").mockReturnValue(openedWindow as unknown as Window);

    await api.openResource("https://example.com/work");

    expect(open).toHaveBeenCalledWith("https://example.com/work", "_blank");
    expect(openedWindow.opener).toBeNull();
  });

  it("rejects when the browser blocks a resource tab", async () => {
    vi.spyOn(window, "open").mockReturnValue(null);

    await expect(api.openResource("https://example.com/work")).rejects.toThrow(
      "The browser blocked the new tab.",
    );
  });

  it("returns a clearly disclosed preview response for chat", async () => {
    const response = await api.chat([
      {
        id: "message-1",
        role: "user",
        content: "What am I working on?",
        createdAt: "2026-07-27T17:04:00.000Z",
      },
    ]);

    expect(response.message).toMatchObject({
      role: "assistant",
      content: expect.stringContaining("browser preview mode"),
    });
    expect(response.economics.reductionPercent).toBeGreaterThan(0);
    expect(response.retrievedMemories).not.toHaveLength(0);
  });
});
