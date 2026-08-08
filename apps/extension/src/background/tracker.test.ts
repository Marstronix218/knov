import { describe, expect, it } from "vitest";
import type { ActivePage, ActiveSession } from "../shared/types";
import {
  closeSession,
  isTrackablePage,
  transitionSession,
  type TrackerContext
} from "./tracker";

const page: ActivePage = {
  tabId: 4,
  windowId: 2,
  url: "https://docs.example.com/project",
  title: "Project notes",
  incognito: false
};

const context: TrackerContext = {
  collectionEnabled: true,
  chromeFocused: true,
  excludedDomains: [],
  browserProfileId: "extension-profile"
};

describe("isTrackablePage", () => {
  it("tracks http metadata only while enabled and focused", () => {
    expect(isTrackablePage(page, context)).toBe(true);
    expect(isTrackablePage(page, { ...context, collectionEnabled: false })).toBe(
      false
    );
    expect(isTrackablePage(page, { ...context, chromeFocused: false })).toBe(false);
    expect(
      isTrackablePage({ ...page, url: "chrome://settings" }, context)
    ).toBe(false);
    expect(isTrackablePage({ ...page, incognito: true }, context)).toBe(false);
  });

  it("applies exact and subdomain exclusions", () => {
    expect(
      isTrackablePage(page, { ...context, excludedDomains: ["example.com"] })
    ).toBe(false);
    expect(
      isTrackablePage(page, {
        ...context,
        excludedDomains: ["notexample.com"]
      })
    ).toBe(true);
  });
});

describe("session transitions", () => {
  it("starts a session without emitting an event", () => {
    const result = transitionSession(
      undefined,
      page,
      context,
      new Date("2026-01-01T10:00:00.000Z"),
      "event-1"
    );
    expect(result.event).toBeNull();
    expect(result.session?.url).toBe(page.url);
  });

  it("closes the previous page when the active tab changes", () => {
    const session: ActiveSession = {
      ...page,
      startedAt: "2026-01-01T10:00:00.000Z",
      lastObservedAt: "2026-01-01T10:00:00.000Z"
    };
    const result = transitionSession(
      session,
      { ...page, tabId: 5, url: "https://other.example/page" },
      context,
      new Date("2026-01-01T10:00:12.000Z"),
      "event-2"
    );
    expect(result.event).toMatchObject({
      id: "event-2",
      durationMs: 12_000,
      url: page.url
    });
    expect(result.session?.tabId).toBe(5);
  });

  it("stops immediately when collection is paused", () => {
    const session: ActiveSession = {
      ...page,
      startedAt: "2026-01-01T10:00:00.000Z",
      lastObservedAt: "2026-01-01T10:00:00.000Z"
    };
    const result = transitionSession(
      session,
      page,
      { ...context, collectionEnabled: false },
      new Date("2026-01-01T10:00:03.000Z"),
      "event-3"
    );
    expect(result.session).toBeUndefined();
    expect(result.event?.durationMs).toBe(3_000);
  });

  it("stops the current session when its domain becomes excluded", () => {
    const session: ActiveSession = {
      ...page,
      startedAt: "2026-01-01T10:00:00.000Z",
      lastObservedAt: "2026-01-01T10:00:00.000Z"
    };
    const result = transitionSession(
      session,
      page,
      { ...context, excludedDomains: ["example.com"] },
      new Date("2026-01-01T10:00:03.000Z"),
      "event-excluded"
    );

    expect(result.session).toBeUndefined();
    expect(result.event?.durationMs).toBe(3_000);
  });

  it("drops accidental sub-quarter-second sessions", () => {
    const session: ActiveSession = {
      ...page,
      startedAt: "2026-01-01T10:00:00.000Z",
      lastObservedAt: "2026-01-01T10:00:00.000Z"
    };
    expect(
      closeSession(
        session,
        new Date("2026-01-01T10:00:00.100Z"),
        "extension-profile",
        "event-4"
      )
    ).toBeNull();
  });
});
