import {
  DEFAULT_CONFIG,
  isExcludedUrl,
  normalizeDomains,
  normalizeLoopbackEndpoint
} from "./shared/config";
import type {
  ActivePage,
  ExtensionConfig,
  ExtensionMessage,
  RuntimeStatus
} from "./shared/types";
import { transitionSession } from "./background/tracker";
import {
  loadConfig,
  loadConnection,
  loadSession,
  saveConfig,
  saveConnection,
  saveSession
} from "./background/storage";
import {
  configWithDesktopPolicy,
  sendEvents,
  testConnection
} from "./background/transport";
import type { BrowserActivityEvent } from "./shared/types";

const HEARTBEAT_ALARM = "knov-heartbeat";
const HEARTBEAT_MINUTES = 0.5;
const MAX_EPHEMERAL_EVENTS = 100;
let transitionChain = Promise.resolve();
let flushInFlight: Promise<void> | undefined;
let pendingEvents: BrowserActivityEvent[] = [];

function enqueueEphemeral(event: BrowserActivityEvent): void {
  pendingEvents = [...pendingEvents, event].slice(-MAX_EPHEMERAL_EVENTS);
}

function queueTransition(operation: () => Promise<void>): void {
  transitionChain = transitionChain.then(operation, operation);
}

async function activePageForWindow(windowId?: number): Promise<ActivePage | undefined> {
  const query: chrome.tabs.QueryInfo = { active: true };
  if (windowId !== undefined && windowId !== chrome.windows.WINDOW_ID_NONE) {
    query.windowId = windowId;
  } else {
    query.lastFocusedWindow = true;
  }
  const [tab] = await chrome.tabs.query(query);
  if (
    !tab ||
    tab.id === undefined ||
    tab.windowId === undefined ||
    !tab.url
  ) {
    return undefined;
  }
  return {
    tabId: tab.id,
    windowId: tab.windowId,
    url: tab.url,
    title: tab.title?.trim() || tab.url,
    incognito: tab.incognito
  };
}

async function chromeIsFocused(): Promise<boolean> {
  try {
    const window = await chrome.windows.getLastFocused();
    return window.focused === true && window.id !== chrome.windows.WINDOW_ID_NONE;
  } catch {
    return false;
  }
}

function newEventId(): string {
  return crypto.randomUUID();
}

async function evaluateActivity(): Promise<void> {
  const [config, focused, current] = await Promise.all([
    loadConfig(),
    chromeIsFocused(),
    loadSession()
  ]);
  const page = focused ? await activePageForWindow() : undefined;
  const result = transitionSession(
    current,
    page,
    {
      collectionEnabled: config.collectionEnabled && Boolean(config.browserProfileId),
      chromeFocused: focused,
      excludedDomains: config.excludedDomains,
      browserProfileId: config.browserProfileId
    },
    new Date(),
    newEventId()
  );

  await saveSession(result.session);
  if (result.event) {
    enqueueEphemeral(result.event);
    void attemptFlush();
  }
  await updateAction();
}

async function checkpointActivity(): Promise<void> {
  const session = await loadSession();
  if (!session) {
    await evaluateActivity();
    return;
  }

  const config = await loadConfig();
  const now = new Date();
  const result = transitionSession(
    session,
    undefined,
    {
      collectionEnabled: config.collectionEnabled && Boolean(config.browserProfileId),
      chromeFocused: false,
      excludedDomains: config.excludedDomains,
      browserProfileId: config.browserProfileId
    },
    now,
    newEventId()
  );
  await saveSession(undefined);
  if (result.event) enqueueEphemeral(result.event);
  await evaluateActivity();
  void attemptFlush();
}

async function performFlush(): Promise<void> {
  const config = await loadConfig();
  const events = pendingEvents;
  pendingEvents = [];
  if (!config.pairingToken) {
    await saveConnection({
      state: "unpaired",
      message: "Pair with the Knov Mac app"
    });
    await updateAction();
    return;
  }

  try {
    const response = await testConnection(config);
    const syncedConfig = await applyDesktopPolicy(config, response);
    if (response.collectionEnabled !== false) {
      await sendEvents(
        syncedConfig,
        events.filter(
          (event) => !isExcludedUrl(event.url, syncedConfig.excludedDomains)
        )
      );
    }
  } catch {
    // Completed events are intentionally discarded on delivery failure. Keeping
    // them on disk could resend activity after the user pauses or deletes data.
  }
  await updateAction();
}

async function applyDesktopPolicy(
  config: ExtensionConfig,
  response: Awaited<ReturnType<typeof testConnection>>
): Promise<ExtensionConfig> {
  const next = configWithDesktopPolicy(config, response);
  const changed =
    next.collectionEnabled !== config.collectionEnabled ||
    next.excludedDomains.length !== config.excludedDomains.length ||
    next.excludedDomains.some(
      (domain, index) => domain !== config.excludedDomains[index]
    );
  if (changed) {
    await saveConfig(next);
  }
  if (response.excludedDomains !== undefined || changed) {
    await evaluateActivity();
  }
  return next;
}

function attemptFlush(): Promise<void> {
  if (!flushInFlight) {
    flushInFlight = performFlush().finally(() => {
      flushInFlight = undefined;
    });
  }
  return flushInFlight;
}

async function getStatus(): Promise<RuntimeStatus> {
  const [config, connection, session] = await Promise.all([
    loadConfig(),
    loadConnection(),
    loadSession()
  ]);
  return {
    collectionEnabled: config.collectionEnabled && Boolean(config.browserProfileId),
    connection: connection.state,
    connectionMessage: connection.message,
    lastConnectedAt: connection.lastConnectedAt,
    queueSize: pendingEvents.length,
    currentPage: session ? { title: session.title, url: session.url } : undefined
  };
}

async function updateAction(): Promise<void> {
  const status = await getStatus();
  const badge =
    !status.collectionEnabled
      ? "Ⅱ"
      : status.connection === "connected"
        ? ""
        : "!";
  const color =
    !status.collectionEnabled
      ? "#74717a"
      : status.connection === "connected"
        ? "#30775c"
        : "#b15b35";
  await Promise.all([
    chrome.action.setBadgeText({ text: badge }),
    chrome.action.setBadgeBackgroundColor({ color }),
    chrome.action.setTitle({
      title: status.collectionEnabled
        ? `Knov: ${status.connectionMessage}`
        : "Knov: collection paused"
    })
  ]);
}

async function setCollectionEnabled(enabled: boolean): Promise<RuntimeStatus> {
  const config = await loadConfig();
  await saveConfig({ ...config, collectionEnabled: enabled });
  await evaluateActivity();
  return getStatus();
}

async function updateConfig(
  patch: Partial<ExtensionConfig>
): Promise<ExtensionConfig> {
  const current = await loadConfig();
  const endpoint =
    patch.endpoint === undefined
      ? current.endpoint
      : normalizeLoopbackEndpoint(patch.endpoint);
  const next: ExtensionConfig = {
    ...DEFAULT_CONFIG,
    ...current,
    ...patch,
    transport: patch.transport === "localhost" ? "localhost" : "native",
    endpoint,
    pairingToken: patch.pairingToken?.trim() ?? current.pairingToken,
    excludedDomains: normalizeDomains(
      patch.excludedDomains ?? current.excludedDomains
    )
  };
  await saveConfig(next);
  await evaluateActivity();
  return next;
}

async function handleMessage(message: ExtensionMessage): Promise<unknown> {
  switch (message.type) {
    case "get_status":
      return getStatus();
    case "get_config":
      return loadConfig();
    case "set_collection_enabled":
      return setCollectionEnabled(message.enabled);
    case "save_config": {
      let config = await updateConfig(message.config);
      try {
        config = await applyDesktopPolicy(
          config,
          await testConnection(config)
        );
      } catch {
        // The transport persisted a useful degraded state. Settings still save so
        // pairing can recover automatically when the Mac app becomes available.
      }
      await updateAction();
      return { config, status: await getStatus() };
    }
    case "test_connection": {
      const config = await loadConfig();
      await applyDesktopPolicy(config, await testConnection(config));
      await updateAction();
      return getStatus();
    }
    case "flush":
      await attemptFlush();
      return getStatus();
  }
}

chrome.runtime.onMessage.addListener(
  (message: ExtensionMessage, _sender, sendResponse) => {
    handleMessage(message)
      .then((result) => sendResponse({ ok: true, result }))
      .catch((error: unknown) =>
        sendResponse({
          ok: false,
          error: error instanceof Error ? error.message : "Unexpected extension error"
        })
      );
    return true;
  }
);

chrome.tabs.onActivated.addListener(() => queueTransition(evaluateActivity));
chrome.tabs.onUpdated.addListener((_tabId, changeInfo) => {
  if (changeInfo.url !== undefined || changeInfo.title !== undefined) {
    queueTransition(evaluateActivity);
  }
});
chrome.tabs.onRemoved.addListener(() => queueTransition(evaluateActivity));
chrome.windows.onFocusChanged.addListener(() => queueTransition(evaluateActivity));
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === HEARTBEAT_ALARM) queueTransition(checkpointActivity);
});
chrome.runtime.onInstalled.addListener(() => {
  void chrome.runtime.openOptionsPage();
  queueTransition(initialize);
});
chrome.runtime.onStartup.addListener(() => queueTransition(initialize));

async function initialize(): Promise<void> {
  await chrome.alarms.create(HEARTBEAT_ALARM, {
    delayInMinutes: HEARTBEAT_MINUTES,
    periodInMinutes: HEARTBEAT_MINUTES
  });
  await evaluateActivity();
  void attemptFlush();
}

queueTransition(initialize);
