import { useCallback, useEffect, useState } from "react";
import type { BusinessAction, BusinessWorkspace } from "../businessTypes";
import { businessApi } from "./businessApi";

// `business_workspace` reclassifies every retained activity event, so refetching it
// on each tab switch made navigation feel slow. The five business pages are views of
// one workspace: cache it here, serve it instantly, and only revalidate when stale.
const STALE_AFTER_MS = 30_000;

let cached: BusinessWorkspace | null = null;
let fetchedAt = 0;
let inFlight: Promise<BusinessWorkspace> | null = null;
const listeners = new Set<(workspace: BusinessWorkspace) => void>();

function publish(workspace: BusinessWorkspace) {
  cached = workspace;
  fetchedAt = Date.now();
  for (const listener of listeners) listener(workspace);
}

// One backend round-trip even when several tab switches land while it is running.
function fetchWorkspace(): Promise<BusinessWorkspace> {
  if (!inFlight) {
    inFlight = businessApi.workspace().finally(() => {
      inFlight = null;
    });
  }
  return inFlight;
}

export function resetBusinessWorkspaceCache() {
  cached = null;
  fetchedAt = 0;
  inFlight = null;
}

export function useBusinessWorkspace() {
  const [workspace, setWorkspace] = useState<BusinessWorkspace | null>(cached);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    listeners.add(setWorkspace);
    return () => {
      listeners.delete(setWorkspace);
    };
  }, []);

  const load = useCallback(async (force = false) => {
    if (!force && cached && Date.now() - fetchedAt < STALE_AFTER_MS) {
      setWorkspace(cached);
      return;
    }
    setBusy(true);
    setError("");
    try {
      publish(await fetchWorkspace());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }, []);

  const refresh = useCallback(() => void load(true), [load]);

  const mutate = useCallback(async (request: BusinessAction) => {
    setBusy(true);
    setError("");
    try {
      const next = await businessApi.mutate(request);
      publish(next);
      return next;
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      return null;
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return { workspace, error, busy, refresh, mutate };
}
