import { invoke } from "@tauri-apps/api/core";
import { isDesktopRuntime } from "./api";
import type { InterviewSession } from "../types";
import type { RevenueOverview, SimulationCatalog } from "../revenueTypes";
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
  importUpload: (projectId: string, fileName: string, dataBase64: string) => call("revenue_import_upload", { projectId, fileName, dataBase64 }),
  createProject: (clientName: string, name: string, description: string, threadIds: string[]) => call("revenue_create_project", { clientName, name, description, threadIds }),
  importAgreement: (projectId: string, title: string, text: string) => call("revenue_import_agreement", { projectId, title, text }),
  importFile: (projectId: string, title: string, path: string) => call("revenue_import_file", { projectId, title, path }),
  analyze: (demo: boolean) => call("revenue_analyze", { demo }),
  review: (opportunityId: string, decision: "confirm" | "dismiss" | "correct" | "clarify", answer: string | null = null) => call("revenue_review", { opportunityId, decision, answer }),
  prepare: (opportunityId: string) => call("revenue_prepare_action", { opportunityId }),
  saveDraft: (draftId: string, body: string) => call("revenue_save_draft", { draftId, body }),
  approveDraft: (draftId: string) => call("revenue_approve_draft", { draftId }),
  recordOutcome: (opportunityId: string, kind: string, evidence: string, amountCents: number | null, currency: string | null, verified: boolean) => call("revenue_record_outcome", { opportunityId, kind, evidence, amountCents, currency, verified }),
};

export type SimulationStep = "contracts" | "gmail" | "slack" | "work" | "billing";

/** Reads a picked or dropped file for `importUpload`; the webview never sees file paths. */
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
