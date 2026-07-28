import type { ExtensionConfig } from "./types";

export const DEFAULT_ENDPOINT = "http://127.0.0.1:48321";

export const DEFAULT_CONFIG: ExtensionConfig = {
  collectionEnabled: false,
  browserProfileId: "",
  transport: "native",
  endpoint: DEFAULT_ENDPOINT,
  pairingToken: "",
  excludedDomains: []
};

export function normalizeDomain(value: string): string | null {
  const trimmed = value.trim().toLowerCase().replace(/^\.+|\.+$/g, "");
  if (!trimmed) return null;

  try {
    const candidate = trimmed.includes("://") ? trimmed : `https://${trimmed}`;
    const hostname = new URL(candidate).hostname.toLowerCase().replace(/\.$/, "");
    return hostname || null;
  } catch {
    return null;
  }
}

export function normalizeDomains(values: Iterable<string>): string[] {
  return [
    ...new Set(
      [...values]
        .map(normalizeDomain)
        .filter((domain): domain is string => domain !== null)
    )
  ].sort();
}

export function isExcludedUrl(url: string, excludedDomains: string[]): boolean {
  try {
    const hostname = new URL(url).hostname.toLowerCase().replace(/\.$/, "");
    return excludedDomains.some(
      (domain) => hostname === domain || hostname.endsWith(`.${domain}`)
    );
  } catch {
    return true;
  }
}

export function normalizeLoopbackEndpoint(value: string): string {
  const url = new URL(value.trim());
  const loopbackHosts = new Set(["127.0.0.1", "localhost", "[::1]"]);
  if (url.protocol !== "http:" || !loopbackHosts.has(url.hostname)) {
    throw new Error("The app address must be an http:// loopback address.");
  }

  url.username = "";
  url.password = "";
  url.hash = "";
  url.search = "";
  return url.toString().replace(/\/$/, "");
}
