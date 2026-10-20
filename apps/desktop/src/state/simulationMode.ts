import { useSyncExternalStore } from "react";

const KEY = "knov.simulation-mode";
const listeners = new Set<() => void>();

function read(): boolean {
  try { return localStorage.getItem(KEY) === "true"; } catch { return false; }
}

/**
 * Whether Home, Opportunities, and Sources show the simulated business instead
 * of actual data. The two never mix: the backend stores them in separate scopes.
 */
export function setSimulationMode(on: boolean) {
  try { localStorage.setItem(KEY, String(on)); } catch { /* the mode simply won't persist */ }
  listeners.forEach((listener) => listener());
}

export function useSimulationMode(): boolean {
  return useSyncExternalStore((listener) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  }, read);
}
