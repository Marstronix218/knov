import { invoke } from "@tauri-apps/api/core";
import { isDesktopRuntime } from "./api";
import type { InterviewSession } from "../types";
import type { ConnectorConfig, ConnectorProvider, ConnectorScope, ConnectorStatus, ConnectorSyncResult, ExtractedDocument, RevenueOverview, SimulationCatalog, SlackChannel, SlackOAuthStart } from "../revenueTypes";
async function call(command: string, args?: Record<string, unknown>): Promise<RevenueOverview> {
  if (!isDesktopRuntime()) throw new Error("Revenue Intelligence requires the desktop app. Browser preview does not save, analyze, or synchronize commercial data.");
  return invoke<RevenueOverview>(command, args);
}
export const revenueApi = {
  overview: (demo: boolean) => call("revenue_overview", { demo }),
  seedDemo: () => call("revenue_seed_demo"),
  simulationLoad: (step: SimulationStep | "all", business?: string) => call("revenue_simulation_load", business ? { step, business } : { step }),
  simulationReset: () => call("revenue_simulation_reset"),
  /** The ten fictional businesses, which one is loaded, and its interview. */
  simulationCatalog: async (): Promise<SimulationCatalog> => {
    if (!isDesktopRuntime()) return { active: null, businesses: [], interview: null };
    return invoke<SimulationCatalog>("revenue_simulation_catalog");
  },
  /** Switches the sandbox to another business, clearing the previous one. */
  simulationSelect: (business: string) => call("revenue_simulation_select", { business }),
  /** Starts (or resumes) the scripted interview for the loaded business; no AI key needed. */
  simulationInterview: async (): Promise<InterviewSession> => {
    if (!isDesktopRuntime()) throw new Error("The simulated interview requires the desktop app.");
    return invoke<InterviewSession>("revenue_simulation_interview");
  },
  /** Extracts an uploaded document's text without storing it, so it can be reviewed before `importAgreement`. */
  extractUpload: async (fileName: string, dataBase64: string): Promise<ExtractedDocument> => {
    if (!isDesktopRuntime()) throw new Error("Reading documents requires the desktop app.");
    return invoke<ExtractedDocument>("revenue_extract_upload", { fileName, dataBase64 });
  },
  createProject: (clientName: string, name: string, description: string, threadIds: string[]) => call("revenue_create_project", { clientName, name, description, threadIds }),
  importAgreement: (projectId: string, title: string, text: string) => call("revenue_import_agreement", { projectId, title, text }),
  analyze: (demo: boolean) => call("revenue_analyze", { demo }),
  review: (opportunityId: string, decision: "confirm" | "dismiss" | "correct" | "clarify", answer: string | null = null) => call("revenue_review", { opportunityId, decision, answer }),
  prepare: (opportunityId: string) => call("revenue_prepare_action", { opportunityId }),
  saveDraft: (draftId: string, body: string) => call("revenue_save_draft", { draftId, body }),
  approveDraft: (draftId: string) => call("revenue_approve_draft", { draftId }),
  recordOutcome: (opportunityId: string, kind: string, evidence: string, amountCents: number | null, currency: string | null, verified: boolean) => call("revenue_record_outcome", { opportunityId, kind, evidence, amountCents, currency, verified }),
};

/** Gmail and Slack connections. Tokens and client secrets go to Keychain and never come back. */
export const connectorApi = {
  config: () => invoke<ConnectorConfig>("revenue_connector_config"),
  status: (provider: ConnectorProvider) => invoke<ConnectorStatus>("revenue_connector_status", { provider }),
  /** Opens Google consent in the browser; resolves after approval (up to 180 s). */
  gmailSignIn: (clientId: string, clientSecret: string | null, scope: ConnectorScope, authorized: boolean) => invoke<ConnectorStatus>("revenue_gmail_oauth", { clientId, clientSecret, scope, authorized }),
  connectWithToken: (provider: ConnectorProvider, token: string, scope: ConnectorScope, authorized: boolean) => invoke<ConnectorStatus>("revenue_connector_connect", { provider, token, scope, authorized }),
  slackStart: (clientId: string, clientSecret: string, scope: ConnectorScope, authorized: boolean) => invoke<SlackOAuthStart>("revenue_slack_oauth_start", { clientId, clientSecret, scope, authorized }),
  slackComplete: (code: string, state: string) => invoke<ConnectorStatus>("revenue_slack_oauth_complete", { code, state, authorized: true }),
  slackChannels: () => invoke<SlackChannel[]>("revenue_slack_list_channels"),
  updateScope: (provider: ConnectorProvider, scope: ConnectorScope) => invoke<ConnectorStatus>("revenue_connector_update_scope", { provider, scope, authorized: true }),
  sync: (provider: ConnectorProvider, projectId: string) => invoke<ConnectorSyncResult>("revenue_connector_sync", { provider, projectId }),
  disconnect: (provider: ConnectorProvider) => invoke<void>("revenue_connector_disconnect", { provider, revoke: true }),
  deleteImported: (provider: ConnectorProvider) => invoke<unknown>("revenue_delete_source", { source: provider }),
};

export type SimulationStep = "contracts" | "gmail" | "slack" | "work" | "billing";

/** Reads a picked or dropped file for `extractUpload`; the webview never sees file paths. */
export function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(",", 2)[1] ?? "");
    reader.onerror = () => reject(reader.error ?? new Error(`${file.name} could not be read.`));
    reader.readAsDataURL(file);
  });
}

let focusedOpportunity = "";
/** Asks the Opportunities page to open this opportunity the next time it mounts. */
export function focusOpportunity(id: string) { focusedOpportunity = id; }
export const pendingFocusedOpportunity = () => focusedOpportunity;
export function clearFocusedOpportunity() { focusedOpportunity = ""; }
