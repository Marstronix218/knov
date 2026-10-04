import { createContext, ReactNode, useCallback, useContext, useEffect, useMemo } from "react";
import { useResource } from "../hooks/useResource";
import { api } from "../lib/api";
import type { AgentOverview, SettingsData } from "../types";

const AGENT_REFRESH_MS = 30_000;

interface AppStatus {
  settings: ReturnType<typeof useResource<SettingsData>>;
  agent: ReturnType<typeof useResource<AgentOverview>>;
  setCollectionEnabled: (enabled: boolean) => Promise<void>;
  setAgentPaused: (paused: boolean) => Promise<void>;
  refreshAgent: () => Promise<void>;
}

const AppStatusContext = createContext<AppStatus | undefined>(undefined);

/**
 * One source of truth for collection and agent state, shared by the sidebar,
 * Now, Settings, and the Agent page so a change in one place shows everywhere.
 */
export function AppStatusProvider({ children }: { children: ReactNode }) {
  const settings = useResource(() => api.settings(), []);
  const agent = useResource(() => api.agentOverview(), []);
  const { setData: setSettings } = settings;
  const { setData: setAgent, reload: reloadAgent } = agent;

  const refreshAgent = useCallback(async () => {
    try {
      setAgent(await api.agentOverview());
    } catch {
      // Keep the last known state; the next refresh will retry.
    }
  }, [setAgent]);

  useEffect(() => {
    const interval = window.setInterval(() => void refreshAgent(), AGENT_REFRESH_MS);
    const onFocus = () => void refreshAgent();
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearInterval(interval);
      window.removeEventListener("focus", onFocus);
    };
  }, [refreshAgent]);

  const setCollectionEnabled = useCallback(async (enabled: boolean) => {
    setSettings(await api.setCollectionEnabled(enabled));
  }, [setSettings]);

  const setAgentPaused = useCallback(async (paused: boolean) => {
    await api.setAgentPaused(paused);
    setSettings((current) => current ? { ...current, agentPaused: paused } : current);
    await reloadAgent();
  }, [setSettings, reloadAgent]);

  const value = useMemo(
    () => ({ settings, agent, setCollectionEnabled, setAgentPaused, refreshAgent }),
    [settings, agent, setCollectionEnabled, setAgentPaused, refreshAgent],
  );
  return <AppStatusContext.Provider value={value}>{children}</AppStatusContext.Provider>;
}

export function useAppStatus(): AppStatus {
  const value = useContext(AppStatusContext);
  if (!value) throw new Error("useAppStatus must be used inside AppStatusProvider");
  return value;
}
