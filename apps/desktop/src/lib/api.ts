import { invoke } from "@tauri-apps/api/core";
import type {
  ActionDecision,
  ActivityEvent,
  ActivityPreview,
  AgentOverview,
  AgentRun,
  AutonomyOverview,
  BootstrapStatus,
  BrowserProfile,
  ChatMode,
  ChatMessage,
  ChatRunResult,
  DashboardData,
  GrantRequest,
  ProfileData,
  Provider,
  PredictionDashboard,
  PredictionFeedback,
  PredictionHistoryItem,
  RangeKey,
  SettingsData,
  Skill,
  SkillUpdate,
  ThreadContext,
  Workflow,
} from "../types";
import {
  mockActivity,
  mockAgentOverview,
  mockAutonomy,
  mockRuns,
  mockSkills,
  mockWorkflows,
  mockBrowsers,
  mockDashboard,
  mockProfile,
  mockPredictionDashboard,
  mockPredictions,
  mockSettings,
} from "./mockData";

const isTauri = () => "__TAURI_INTERNALS__" in window;
export const isDesktopRuntime = isTauri;

function browserPreview(url: string): ActivityPreview {
  try {
    const parsed = new URL(url);
    const host = parsed.hostname.replace(/^www\./, "").toLowerCase();
    let videoId: string | undefined;
    if (["youtube.com", "m.youtube.com"].includes(host)) {
      videoId = parsed.pathname === "/watch"
        ? parsed.searchParams.get("v") ?? undefined
        : parsed.pathname.match(/^\/(?:shorts|embed|live)\/([^/]+)/)?.[1];
    } else if (host === "youtu.be") {
      videoId = parsed.pathname.split("/").filter(Boolean)[0];
    }
    if (videoId && /^[A-Za-z0-9_-]{6,64}$/.test(videoId)) {
      return { kind: "youtube", url };
    }
  } catch {
    // Invalid preview URLs fall back to a metadata-only resource card.
  }
  return { kind: "link", url };
}

async function call<T>(command: string, args?: Record<string, unknown>, fallback?: T): Promise<T> {
  if (!isTauri()) {
    return fallback === undefined ? (undefined as T) : structuredClone(fallback);
  }
  return invoke<T>(command, args);
}

export const api = {
  openResource: async (url: string) => {
    if (!isTauri()) {
      const opened = window.open(url, "_blank");
      if (!opened) throw new Error("The browser blocked the new tab.");
      opened.opener = null;
      return;
    }
    await invoke<void>("open_resource", { url });
  },
  openApplication: async (appName: string) => {
    if (!isTauri()) {
      throw new Error("Opening local applications is only available in the desktop app.");
    }
    await invoke<void>("open_application", { appName });
  },
  activityPreview: (url: string) =>
    call<ActivityPreview>("get_activity_preview", { url }, browserPreview(url)),
  activityIcon: (appName: string, url?: string) =>
    call<string | null>("get_activity_icon", { appName, url }, null),
  dashboard: (range: RangeKey) =>
    call<DashboardData>("get_dashboard", { range }, { ...mockDashboard, range }),
  activity: (range: RangeKey, query = "") =>
    call<ActivityEvent[]>("get_activity_history", { range, query }, mockActivity),
  profile: () => call<ProfileData>("get_profile", undefined, mockProfile),
  settings: () => call<SettingsData>("get_settings", undefined, mockSettings),
  predictionsDashboard: () =>
    call<PredictionDashboard>("get_predictions_dashboard", undefined, mockPredictionDashboard),
  predictionHistory: () =>
    call<PredictionHistoryItem[]>("get_prediction_history", undefined, mockPredictions),
  generatePredictions: () =>
    call<PredictionDashboard>("generate_predictions", undefined, {
      ...mockPredictionDashboard,
      enabled: true,
    }),
  recordPredictionFeedback: (predictionId: string, feedback: PredictionFeedback) =>
    call<void>("record_prediction_feedback", { predictionId, feedback }, undefined),
  browserProfiles: () => call<BrowserProfile[]>("get_browser_profiles", undefined, mockBrowsers),
  bootstrapStatus: () =>
    call<BootstrapStatus>(
      "get_bootstrap_status",
      undefined,
      { phase: "not-started", importedEvents: 0, progress: 0, message: "Ready to import browser history." },
    ),
  setCollectionEnabled: (enabled: boolean) =>
    call<SettingsData>("set_collection_enabled", { enabled }, { ...mockSettings, collectionStatus: { ...mockSettings.collectionStatus, enabled } }),
  requestAccessibility: () => call<boolean>("request_accessibility_permission", undefined, false),
  setBrowserProfiles: (profileIds: string[]) =>
    call<void>("set_browser_profiles", { profileIds }, undefined),
  startBootstrap: () => call<BootstrapStatus>("start_bootstrap", undefined, undefined),
  reimportChromeHistory: () =>
    call<ProfileData>("reimport_chrome_history", undefined, mockProfile),
  refreshProfile: () => call<ProfileData>("refresh_profile", undefined, mockProfile),
  saveCorrection: (label: string, description?: string, id?: string) =>
    call<ProfileData>("save_profile_correction", { id, label, description }, mockProfile),
  removeCorrection: (id: string) =>
    call<ProfileData>("remove_profile_correction", { id }, mockProfile),
  dismissInference: (id: string) =>
    call<ProfileData>("dismiss_profile_inference", { id }, mockProfile),
  saveProfileSummary: (summary: string) =>
    call<ProfileData>("save_profile_summary", { summary }, { ...mockProfile, summary }),
  saveProviderKey: (provider: Provider, key: string) =>
    call<void>("save_provider_key", { provider, key }, undefined),
  removeProviderKey: (provider: Provider) =>
    call<void>("remove_provider_key", { provider }, undefined),
  testProvider: (provider: Provider) =>
    call<string>("test_provider", { provider }, "Connection successful."),
  saveSettings: (settings: Partial<SettingsData>) =>
    call<SettingsData>("save_settings", { settings }, { ...mockSettings, ...settings }),
  dismissRecommendation: (id: string, feedback?: string) =>
    call<void>("dismiss_recommendation", { id, feedback }, undefined),
  recordProductEvent: (eventType: string, threadId?: string) =>
    call<void>("record_product_event", { eventType, threadId }, undefined),
  chat: (messages: ChatMessage[], mode: ChatMode = "optimized", threadContext?: ThreadContext) =>
    call<ChatRunResult>(
      "chat",
      { messages, mode, threadContext },
      {
        message: {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            "I’m running in browser preview mode, so this is a sample Knov answer. The native app retrieves relevant profile memories and records aggregate context economics locally.",
          createdAt: new Date().toISOString(),
        },
        retrievedMemories: [
          {
            id: "preview-memory",
            text: "Prefers local-first architecture and explicit privacy boundaries.",
            memoryType: "preference",
            source: "preview",
            createdAt: Math.floor(Date.now() / 1000),
            score: 0.94,
          },
        ],
        economics: {
          queryId: crypto.randomUUID(),
          mode,
          model: "preview-model",
          baselineInputTokens: 3842,
          optimizedInputTokens: 721,
          tokensSaved: 3121,
          reductionPercent: 81.23,
          outputTokens: 84,
          latencyMs: 620,
          memoryCount: 1,
          contextBudgetTokens: 6000,
          contextEstimatedTokens: 721,
          contextUnitsConsidered: 8,
          contextUnitsSent: 8,
          contextUnitsOmitted: 0,
          contextDetailLevel: "selected-event-metadata",
          measurementMethod: "preview_sample",
          telemetryStatus: "preview-only",
          baselineContextPreview: "Sample full profile and summarized activity context.",
          optimizedContextPreview: "Sample local profile memory plus compact query-specific local activity facts.",
        },
      },
    ),
  deleteAllData: () => call<void>("delete_all_data", undefined, undefined),
  startLocalBootstrap: () =>
    call<BootstrapStatus>("start_local_bootstrap", undefined, {
      phase: "complete",
      importedEvents: 0,
      progress: 100,
      message: "Local context is ready.",
    }),

  agentOverview: () => call<AgentOverview>("get_agent_overview", undefined, mockAgentOverview),
  reviewGoal: (goalId: string, status: "confirmed" | "dismissed" | "completed" | "inferred", title?: string) =>
    call<AgentOverview>("review_goal", { goalId, status, title }, previewGoalReview(goalId, status, title)),
  workflows: () => call<Workflow[]>("get_workflows", undefined, mockWorkflows),
  rescanWorkflows: () => call<Workflow[]>("rescan_workflows", undefined, mockWorkflows),
  reviewWorkflow: (workflowId: string, status: Workflow["status"], title?: string) =>
    call<Workflow>("review_workflow", { workflowId, status, title }, previewWorkflowReview(workflowId, status, title)),
  skills: () => call<Skill[]>("get_skills", undefined, mockSkills),
  createSkill: (workflowId: string) =>
    call<Skill>("create_skill", { workflowId }, previewSkillFromWorkflow(workflowId)),
  updateSkill: (skill: SkillUpdate) => call<Skill>("update_skill", { skill }, previewSkillUpdate(skill)),
  deleteSkill: (skillId: string) => call<void>("delete_skill", { skillId }, undefined),
  previewSkillRun: (skillId: string) =>
    call<AgentRun>("preview_skill_run", { skillId }, previewRunFor(skillId)),
  decideRun: (runId: string, decisions: ActionDecision[]) =>
    call<AgentRun>("decide_agent_run", { runId, decisions }, previewDecidedRun(runId, decisions)),
  cancelRun: (runId: string) => call<AgentRun>("cancel_agent_run", { runId }, previewCancelledRun(runId)),
  acknowledgeRun: (runId: string) => call<void>("acknowledge_agent_run", { runId }, undefined),
  agentRuns: (limit = 30) => call<AgentRun[]>("get_agent_runs", { limit }, mockRuns),
  agentRun: (runId: string) => call<AgentRun>("get_agent_run", { runId }, previewRunById(runId)),
  rollbackAction: (actionId: string) =>
    call<AgentRun>("rollback_agent_action", { actionId }, previewRolledBack(actionId)),
  openDraft: async (actionId: string) => {
    if (!isTauri()) throw new Error("Drafts open in the desktop app.");
    await invoke<void>("open_agent_draft", { actionId });
  },
  autonomy: () => call<AutonomyOverview>("get_autonomy", undefined, mockAutonomy),
  setAgentPaused: (paused: boolean) =>
    call<AutonomyOverview>("set_agent_paused", { paused }, { ...mockAutonomy, paused }),
  saveGrant: (grant: GrantRequest) =>
    call<AutonomyOverview>("save_autonomy_grant", { grant }, mockAutonomy),
  revokeGrant: (grantId: string) =>
    call<AutonomyOverview>("revoke_autonomy_grant", { grantId }, {
      ...mockAutonomy,
      grants: mockAutonomy.grants.filter((grant) => grant.id !== grantId),
    }),
  respondProposal: (response: { actionType: string; scopeKind: string; scopeValue?: string; accept: boolean }) =>
    call<AutonomyOverview>("respond_autonomy_proposal", { response }, { ...mockAutonomy, proposals: [] }),
  approveWorkspace: (path: string) =>
    call<AutonomyOverview>("approve_agent_workspace", { path }, mockAutonomy),
  removeWorkspace: (workspaceId: string) =>
    call<AutonomyOverview>("remove_agent_workspace", { workspaceId }, {
      ...mockAutonomy,
      workspaces: mockAutonomy.workspaces.filter((workspace) => workspace.id !== workspaceId),
    }),
};

/* Browser-preview fallbacks: deterministic sample transitions so the
   preview can demonstrate approval, execution, and rollback states. */

function previewGoalReview(goalId: string, status: string, title?: string): AgentOverview {
  const overview = structuredClone(mockAgentOverview);
  const goals = overview.state.goals
    .filter((goal) => !(goal.id === goalId && (status === "dismissed" || status === "completed")))
    .map((goal) => goal.id === goalId
      ? { ...goal, status: status === "confirmed" ? "confirmed" as const : "inferred" as const, title: title || goal.title, confidence: status === "confirmed" ? 1 : goal.confidence }
      : goal);
  overview.state.goals = goals;
  overview.state.goal = goals.find((goal) => goal.id === overview.state.goal?.id) ?? goals[0];
  return overview;
}

function previewWorkflowReview(workflowId: string, status: Workflow["status"], title?: string): Workflow {
  const workflow = structuredClone(mockWorkflows.find((item) => item.id === workflowId) ?? mockWorkflows[0]);
  return { ...workflow, status, title: title?.trim() || workflow.title };
}

function previewSkillFromWorkflow(workflowId: string): Skill {
  const existing = mockSkills.find((skill) => skill.workflowId === workflowId);
  if (existing) return structuredClone(existing);
  const workflow = mockWorkflows.find((item) => item.id === workflowId) ?? mockWorkflows[0];
  return {
    ...structuredClone(mockSkills[0]),
    id: `skill-${workflowId}`,
    name: workflow.title,
    workflowId,
    thread: workflow.stats.thread,
    trigger: { kind: "manual" },
    stats: { runs: 0, completed: 0, needsAttention: 0, failed: 0, rolledBack: 0 },
    steps: workflow.steps.map((step, index) => ({
      id: `step-${index + 1}`,
      title: step.title,
      category: step.category,
      enabled: true,
      action: step.kind === "app"
        ? { type: "open_application" as const, app: step.label }
        : step.resource ? { type: "open_url" as const, url: step.resource } : undefined,
    })),
  };
}

function previewSkillUpdate(update: SkillUpdate): Skill {
  const skill = structuredClone(mockSkills.find((item) => item.id === update.id) ?? mockSkills[0]);
  return {
    ...skill,
    name: update.name.trim() || skill.name,
    trigger: update.trigger,
    onException: update.onException,
    enabled: update.enabled,
    steps: skill.steps.map((step) => ({
      ...step,
      enabled: update.steps.find((candidate) => candidate.id === step.id)?.enabled ?? step.enabled,
    })),
  };
}

function previewRunFor(skillId: string): AgentRun {
  const run = structuredClone(mockRuns[0]);
  return {
    ...run,
    id: `run-preview-${skillId}`,
    skillId,
    origin: "manual",
    status: "awaiting_approval",
    actions: run.actions.map((action) => ({
      ...action,
      runId: `run-preview-${skillId}`,
      status: "awaiting_approval",
      decision: action.decision === "auto" ? "auto" : "pending",
      resultSummary: undefined,
      outputExcerpt: undefined,
      verification: undefined,
      rollbackAvailable: false,
      canOpen: false,
    })),
  };
}

function previewRunById(runId: string): AgentRun {
  return structuredClone(mockRuns.find((run) => run.id === runId) ?? mockRuns[0]);
}

function previewDecidedRun(runId: string, decisions: ActionDecision[]): AgentRun {
  const run = previewRunById(runId);
  run.id = runId;
  run.actions = run.actions.map((action) => {
    const choice = decisions.find((decision) => decision.actionId === action.id);
    if (!choice || action.status !== "awaiting_approval") return action;
    return choice.approved
      ? {
        ...action,
        status: "succeeded",
        decision: action.decision === "auto" ? "auto" : "approved",
        resultSummary: `Preview: ${action.title}`,
        verification: { passed: true, checks: ["Browser preview: no action was taken on your Mac"] },
      }
      : { ...action, status: "rejected", decision: "rejected" };
  });
  run.status = run.actions.some((action) => action.status === "needs_attention") ? "completed_with_exceptions" : "completed";
  run.summary = "Preview run finished";
  return run;
}

function previewCancelledRun(runId: string): AgentRun {
  const run = previewRunById(runId);
  run.actions = run.actions.map((action) => action.status === "awaiting_approval" ? { ...action, status: "skipped", decision: "cancelled" } : action);
  run.status = "cancelled";
  return run;
}

function previewRolledBack(actionId: string): AgentRun {
  const run = structuredClone(mockRuns.find((candidate) => candidate.actions.some((action) => action.id === actionId)) ?? mockRuns[0]);
  run.actions = run.actions.map((action) => action.id === actionId
    ? { ...action, status: "rolled_back", rollbackAvailable: false, canOpen: false, resultSummary: "Draft deleted." }
    : action);
  return run;
}
