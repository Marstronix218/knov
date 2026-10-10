import { invoke } from "@tauri-apps/api/core";
import { isDesktopRuntime } from "./api";
import type { RevenueOverview } from "../revenueTypes";
async function call(command: string, args?: Record<string, unknown>): Promise<RevenueOverview> {
  if (!isDesktopRuntime()) throw new Error("Revenue Intelligence requires the desktop app. Browser preview does not save, analyze, or synchronize commercial data.");
  return invoke<RevenueOverview>(command, args);
}
export const revenueApi = {
  overview: (demo: boolean) => call("revenue_overview", { demo }),
  seedDemo: () => call("revenue_seed_demo"),
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
