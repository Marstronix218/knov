import "./ui.css";
import type { RuntimeStatus } from "./shared/types";

interface MessageResponse<T> {
  ok: boolean;
  result?: T;
  error?: string;
}

const collectionLabel = document.querySelector<HTMLElement>("#collection-label")!;
const connectionLabel = document.querySelector<HTMLElement>("#connection-label")!;
const statusDot = document.querySelector<HTMLElement>("#status-dot")!;
const toggleButton =
  document.querySelector<HTMLButtonElement>("#toggle-collection")!;
const currentTitle = document.querySelector<HTMLElement>("#current-title")!;
const currentHost = document.querySelector<HTMLElement>("#current-host")!;
const queueLabel = document.querySelector<HTMLElement>("#queue-label")!;
const degradedNotice =
  document.querySelector<HTMLElement>("#degraded-notice")!;

async function send<T>(message: object): Promise<T> {
  const response = (await chrome.runtime.sendMessage(message)) as MessageResponse<T>;
  if (!response.ok || response.result === undefined) {
    throw new Error(response.error ?? "The extension background service is unavailable.");
  }
  return response.result;
}

function render(status: RuntimeStatus): void {
  const connected = status.connection === "connected";
  collectionLabel.textContent = status.collectionEnabled
    ? "Collection is on"
    : "Collection is paused";
  connectionLabel.textContent = status.connectionMessage;
  toggleButton.textContent = status.collectionEnabled
    ? "Pause collection"
    : "Resume collection";
  toggleButton.disabled = false;
  statusDot.className = `status-dot ${
    !status.collectionEnabled ? "paused" : connected ? "connected" : ""
  }`;
  degradedNotice.hidden = connected;
  queueLabel.textContent = `${status.queueSize} ${
    status.queueSize === 1 ? "event" : "events"
  } waiting`;

  if (status.currentPage && status.collectionEnabled) {
    currentTitle.textContent = status.currentPage.title;
    try {
      currentHost.textContent = new URL(status.currentPage.url).hostname;
    } catch {
      currentHost.textContent = "";
    }
  } else {
    currentTitle.textContent = status.collectionEnabled
      ? "Chrome is not currently focused"
      : "Collection is paused";
    currentHost.textContent = "";
  }
}

async function refresh(): Promise<void> {
  try {
    render(await send<RuntimeStatus>({ type: "get_status" }));
  } catch (error) {
    collectionLabel.textContent = "Status unavailable";
    connectionLabel.textContent =
      error instanceof Error ? error.message : "Could not load status";
    degradedNotice.hidden = false;
  }
}

toggleButton.addEventListener("click", async () => {
  toggleButton.disabled = true;
  const shouldEnable = toggleButton.textContent === "Resume collection";
  try {
    render(
      await send<RuntimeStatus>({
        type: "set_collection_enabled",
        enabled: shouldEnable
      })
    );
  } finally {
    toggleButton.disabled = false;
  }
});

document.querySelector("#open-options")?.addEventListener("click", () => {
  void chrome.runtime.openOptionsPage();
});

void refresh();
