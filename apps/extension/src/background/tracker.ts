import { isExcludedUrl } from "../shared/config";
import type {
  ActivePage,
  ActiveSession,
  BrowserActivityEvent
} from "../shared/types";

const MIN_EVENT_DURATION_MS = 250;

export interface TrackerContext {
  collectionEnabled: boolean;
  chromeFocused: boolean;
  excludedDomains: string[];
  browserProfileId: string;
}

export function isTrackablePage(
  page: ActivePage | undefined,
  context: TrackerContext
): page is ActivePage {
  if (!page || !context.collectionEnabled || !context.chromeFocused) return false;
  if (page.incognito) return false;
  if (!page.url.startsWith("http://") && !page.url.startsWith("https://")) {
    return false;
  }
  return !isExcludedUrl(page.url, context.excludedDomains);
}

export function startSession(page: ActivePage, now: Date): ActiveSession {
  const observedAt = now.toISOString();
  return { ...page, startedAt: observedAt, lastObservedAt: observedAt };
}

export function sessionMatchesPage(
  session: ActiveSession,
  page: ActivePage
): boolean {
  return (
    session.tabId === page.tabId &&
    session.windowId === page.windowId &&
    session.url === page.url &&
    session.title === page.title
  );
}

export function closeSession(
  session: ActiveSession,
  now: Date,
  browserProfileId: string,
  id: string
): BrowserActivityEvent | null {
  const startedAt = Date.parse(session.startedAt);
  const endedAt = Math.max(startedAt, now.getTime());
  const durationMs = endedAt - startedAt;
  if (durationMs < MIN_EVENT_DURATION_MS) return null;

  return {
    id,
    kind: "browser_focus",
    source: "chrome_extension",
    browser: "chrome",
    browserProfileId,
    url: session.url,
    title: session.title,
    startedAt: session.startedAt,
    endedAt: new Date(endedAt).toISOString(),
    durationMs,
    incognito: session.incognito
  };
}

export interface TransitionResult {
  session?: ActiveSession;
  event: BrowserActivityEvent | null;
}

export function transitionSession(
  current: ActiveSession | undefined,
  page: ActivePage | undefined,
  context: TrackerContext,
  now: Date,
  eventId: string
): TransitionResult {
  const trackable = isTrackablePage(page, context);
  if (current && trackable && sessionMatchesPage(current, page)) {
    return {
      session: { ...current, lastObservedAt: now.toISOString() },
      event: null
    };
  }

  const event = current
    ? closeSession(current, now, context.browserProfileId, eventId)
    : null;
  return {
    session: trackable ? startSession(page, now) : undefined,
    event
  };
}
