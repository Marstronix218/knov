import "./ui.css";
import type { ExtensionConfig, RuntimeStatus } from "./shared/types";

interface MessageResponse<T> {
  ok: boolean;
  result?: T;
  error?: string;
}

interface SavedConfigResult {
  config: ExtensionConfig;
  status: RuntimeStatus;
}

const form = document.querySelector<HTMLFormElement>("#settings-form")!;
const endpointInput = document.querySelector<HTMLInputElement>("#endpoint")!;
const endpointField = document.querySelector<HTMLElement>("#endpoint-field")!;
const transportInput = document.querySelector<HTMLSelectElement>("#transport")!;
const tokenInput = document.querySelector<HTMLInputElement>("#pairing-token")!;
const profileInput =
  document.querySelector<HTMLInputElement>("#browser-profile-id")!;
const exclusionsInput =
  document.querySelector<HTMLTextAreaElement>("#excluded-domains")!;
const result = document.querySelector<HTMLElement>("#connection-result")!;
const testButton =
  document.querySelector<HTMLButtonElement>("#test-connection")!;

async function send<T>(message: object): Promise<T> {
  const response = (await chrome.runtime.sendMessage(message)) as MessageResponse<T>;
  if (!response.ok || response.result === undefined) {
    throw new Error(response.error ?? "The extension background service is unavailable.");
  }
  return response.result;
}

function showResult(message: string, isError = false): void {
  result.textContent = message;
  result.classList.toggle("error", isError);
}

async function load(): Promise<void> {
  try {
    const config = await send<ExtensionConfig>({ type: "get_config" });
    endpointInput.value = config.endpoint;
    transportInput.value = config.transport;
    endpointField.hidden = config.transport !== "localhost";
    tokenInput.value = config.pairingToken;
    profileInput.value = config.browserProfileId;
    exclusionsInput.value = config.excludedDomains.join("\n");
  } catch (error) {
    showResult(error instanceof Error ? error.message : "Could not load settings.", true);
  }
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const submit = form.querySelector<HTMLButtonElement>('button[type="submit"]')!;
  submit.disabled = true;
  showResult("Saving and checking the local app…");
  try {
    const saved = await send<SavedConfigResult>({
      type: "save_config",
      config: {
        endpoint: endpointInput.value,
        transport: transportInput.value,
        pairingToken: tokenInput.value,
        browserProfileId: profileInput.value.trim(),
        excludedDomains: exclusionsInput.value.split(/\r?\n/)
      }
    });
    showResult(
      saved.status.connection === "connected"
        ? "Connected. Settings saved."
        : `Settings saved. ${saved.status.connectionMessage}`,
      saved.status.connection !== "connected"
    );
  } catch (error) {
    showResult(
      error instanceof Error ? error.message : "Could not save settings.",
      true
    );
  } finally {
    submit.disabled = false;
  }
});

transportInput.addEventListener("change", () => {
  endpointField.hidden = transportInput.value !== "localhost";
});

testButton.addEventListener("click", async () => {
  testButton.disabled = true;
  showResult("Checking the local app…");
  try {
    const status = await send<RuntimeStatus>({ type: "test_connection" });
    showResult(status.connectionMessage, status.connection !== "connected");
  } catch (error) {
    showResult(
      error instanceof Error ? error.message : "Connection test failed.",
      true
    );
  } finally {
    testButton.disabled = false;
  }
});

void load();
