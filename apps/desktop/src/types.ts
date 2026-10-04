export type RangeKey = "today" | "7d" | "30d";

export type Provider = "openai" | "anthropic" | "bedrock";

export interface UsageSlice {
  name: string;
  seconds: number;
  percentage: number;
  color: string;
  detail?: string;
}

export interface ActivityEvent {
  id: string;
  appName: string;
  windowTitle?: string;
  url?: string;
  pageTitle?: string;
  searchQuery?: string;
  browserProfile?: string;
  startedAt: string;
  durationSeconds: number;
  modifiedFiles?: string[];
  topic?: string;
  source: "collector" | "chrome" | "history" | "editor" | "firefox" | "safari";
}

export interface ActivityPreview {
  kind: "youtube" | "link";
  url: string;
  title?: string;
}

export interface TopicInsight {
  id: string;
  title: string;
  description: string;
  metric: string;
  evidence: string;
}

export type RecommendationKind = "continuity" | "behavioral";

export interface Recommendation {
  id: string;
  kind: RecommendationKind;
  title: string;
  body: string;
  evidence: string;
  createdAt: string;
}

export interface DashboardData {
  range: RangeKey;
  trackedSeconds: number;
  focusedSeconds: number;
  activeTopics: ActiveTopic[];
  appUsage: UsageSlice[];
  siteUsage: UsageSlice[];
  recentActivity: ActivityEvent[];
  insights: TopicInsight[];
  recommendations: Recommendation[];
  generatedAt?: string;
}

export interface ActiveTopic {
  name: string;
  count: number;
}

export interface ProfileItem {
  id: string;
  label: string;
  description?: string;
  confidence?: number;
  provenance: "observed" | "inferred" | "user";
}

export interface ProfileSection {
  id: string;
  title: string;
  items: ProfileItem[];
}

export interface ProfileData {
  summary: string;
  sections: ProfileSection[];
  updatedAt?: string;
}

export interface BrowserProfile {
  id: string;
  browser: "chrome" | "firefox" | "safari";
  name: string;
  path: string;
  selected: boolean;
  support: "required" | "best-effort" | "unavailable";
}

export interface CollectionStatus {
  enabled: boolean;
  accessibilityGranted: boolean;
  dataPath?: string;
  degradedReasons: string[];
}

export interface SettingsData {
  provider: Provider;
  hasProviderKey: boolean;
  behavioralGuidanceEnabled: boolean;
  predictionExperimentEnabled: boolean;
  predictionDisplayThreshold: number;
  agentPaused?: boolean;
  agentMaxActionsPerHour?: number;
  launchAtLogin: boolean;
  selectedBrowserProfileIds: string[];
  excludedApps: string[];
  excludedDomains: string[];
  collectionStatus: CollectionStatus;
}

export type PredictionSource = "heuristic" | "provider" | "workflow";
export type PredictionEvaluationStatus = "pending" | "matched" | "partial" | "missed" | "expired";
export type PredictionFeedback = "correct" | "incorrect" | "dismissed";

export interface PredictionResource {
  type: "thread" | "url" | "domain" | "application" | "document" | "repository" | "unknown";
  label: string;
  safeLocator?: string;
}

export interface WorkPrediction {
  id: string;
  createdAt: number;
  source: PredictionSource;
  intent: string;
  nextAction: string;
  nextResource?: PredictionResource;
  threadId?: string;
  confidence: number;
  horizonMinutes: number;
  reasoningSummary: string;
  evidence: string[];
  evaluationStatus: PredictionEvaluationStatus;
  expiresAt: number;
  observedOutcome?: string;
  matchScore?: number;
  userFeedback?: PredictionFeedback;
  evaluatedAt?: number;
  goal?: string;
  workflowId?: string;
}

export interface PredictionHistoryItem extends WorkPrediction {
  feedbackReason?: string;
}

export interface PredictionStats {
  totalPredictions: number;
  evaluatedPredictions: number;
  matched: number;
  partial: number;
  missed: number;
  providerTop1Accuracy?: number;
  baselineTop1Accuracy?: number;
  workflowTop1Accuracy?: number;
  highConfidenceAccuracy?: number;
  userPositiveFeedbackRate?: number;
  calibration?: CalibrationBin[];
}

export interface CalibrationBin {
  label: string;
  minConfidence: number;
  maxConfidence: number;
  count: number;
  meanConfidence?: number;
  observedAccuracy?: number;
}

export interface PredictionDashboard {
  enabled: boolean;
  predictions: WorkPrediction[];
  stats: PredictionStats;
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  createdAt: string;
}

export interface ThreadContextEvent {
  observedAt: string;
  appName: string;
  source: ActivityEvent["source"];
  title?: string;
  resource?: string;
  searchQuery?: string;
  observedActiveSeconds?: number;
}

export interface ThreadContext {
  version: 1;
  subject: string;
  signalCount: number;
  apps: string[];
  modifiedFiles: string[];
  observedFrom?: string;
  observedThrough?: string;
  events: ThreadContextEvent[];
}

export type ChatMode = "optimized";

export interface MemoryRecord {
  id: string;
  text: string;
  memoryType: string;
  source: string;
  createdAt: number;
  importance?: number;
  score?: number;
}

export interface ContextEconomics {
  queryId: string;
  mode: ChatMode;
  model: string;
  baselineInputTokens: number;
  optimizedInputTokens: number;
  tokensSaved: number;
  reductionPercent: number;
  actualInputTokens?: number;
  outputTokens?: number;
  latencyMs: number;
  estimatedCostUsd?: number;
  memoryCount: number;
  contextBudgetTokens: number;
  contextEstimatedTokens: number;
  contextUnitsConsidered: number;
  contextUnitsSent: number;
  contextUnitsOmitted: number;
  contextDetailLevel: string;
  providerPreflightInputTokens?: number;
  cacheReadInputTokens?: number;
  cacheWriteInputTokens?: number;
  measurementMethod: string;
  telemetryStatus: string;
  baselineContextPreview: string;
  optimizedContextPreview: string;
}

export interface ChatRunResult {
  message: ChatMessage;
  retrievedMemories: MemoryRecord[];
  economics: ContextEconomics;
}

export interface BootstrapStatus {
  phase: "not-started" | "importing" | "profiling" | "complete" | "error";
  importedEvents: number;
  progress: number;
  message: string;
}

/* Autonomous work agent ------------------------------------------------- */

export type StepCategory =
  | "code" | "terminal" | "code-hosting" | "docs" | "search" | "communication" | "email"
  | "notes" | "spreadsheet" | "calendar" | "video" | "ai" | "design" | "project" | "browser"
  | "sensitive" | "web" | "other";

export interface WorkflowStep {
  key: string;
  kind: "app" | "web" | "search";
  category: StepCategory | string;
  label: string;
  title: string;
  resource?: string;
  averageSeconds: number;
}

export interface WorkflowOccurrence {
  startedAt: number;
  endedAt: number;
  thread?: string;
  details: string[];
}

export interface WorkflowStats {
  occurrences: number;
  distinctDays: number;
  perWeek: number;
  averageDurationSeconds: number;
  completionRate: number;
  typicalWeekday?: string;
  typicalHour?: number;
  thread?: string;
}

export interface OpportunityScore {
  score: number;
  frequency: number;
  timeCost: number;
  stability: number;
  executability: number;
  risk: number;
  preparableSteps: number;
  executableSteps: number;
  estimatedMinutesSavedPerWeek: number;
  surfaced: boolean;
  rationale: string[];
}

export type WorkflowStatus = "discovered" | "confirmed" | "dismissed";

export interface Workflow {
  id: string;
  title: string;
  generatedTitle: string;
  status: WorkflowStatus;
  active: boolean;
  steps: WorkflowStep[];
  stats: WorkflowStats;
  evidence: WorkflowOccurrence[];
  opportunity: OpportunityScore;
  skillId?: string;
  firstSeenAt: number;
  lastSeenAt: number;
  updatedAt: number;
}

export type ActionType = "open_url" | "open_application" | "write_draft" | "run_checks";

export type ActionSpec =
  | { type: "open_url"; url: string; resolveDomain?: string }
  | { type: "open_application"; app: string }
  | { type: "write_draft"; template: string }
  | { type: "run_checks"; workspaceId: string; preset: string };

export interface SkillStep {
  id: string;
  title: string;
  category: string;
  action?: ActionSpec;
  enabled: boolean;
}

export type TriggerKind = "manual" | "context" | "schedule";

export interface SkillTrigger {
  kind: TriggerKind;
  weekday?: number;
  hour?: number;
}

export interface SkillStats {
  runs: number;
  completed: number;
  needsAttention: number;
  failed: number;
  rolledBack: number;
  lastRunAt?: number;
}

export interface Skill {
  id: string;
  name: string;
  description: string;
  workflowId?: string;
  thread?: string;
  trigger: SkillTrigger;
  onException: "stop" | "continue";
  steps: SkillStep[];
  enabled: boolean;
  createdAt: number;
  updatedAt: number;
  lastTriggeredAt?: number;
  stats: SkillStats;
}

export interface SkillStepUpdate {
  id: string;
  enabled: boolean;
  workspaceId?: string;
  checkPreset?: string;
}

export interface SkillUpdate {
  id: string;
  name: string;
  description?: string;
  trigger: SkillTrigger;
  onException: "stop" | "continue";
  enabled: boolean;
  steps: SkillStepUpdate[];
  includeBrief: boolean;
}

export interface Verification {
  passed: boolean;
  checks: string[];
}

export type ActionStatus =
  | "awaiting_approval" | "approved" | "running" | "succeeded" | "needs_attention"
  | "failed" | "rejected" | "blocked" | "skipped" | "rolled_back";

export type ActionDecisionKind = "auto" | "pending" | "approved" | "rejected" | "blocked" | "cancelled";

export interface AgentAction {
  id: string;
  runId: string;
  stepId: string;
  position: number;
  actionType: ActionType | string;
  title: string;
  riskClass: string;
  targetLabel: string;
  rationale: string;
  scopeKind: "global" | "skill" | "workspace" | string;
  scopeValue?: string;
  scopeLabel: string;
  decision: ActionDecisionKind | string;
  decisionReason: string;
  grantId?: string;
  status: ActionStatus | string;
  resultSummary?: string;
  outputExcerpt?: string;
  preview?: string;
  verification?: Verification;
  rollbackAvailable: boolean;
  canOpen: boolean;
  createdAt: number;
  startedAt?: number;
  finishedAt?: number;
}

export type RunStatus =
  | "awaiting_approval" | "ready" | "running" | "completed" | "completed_with_exceptions"
  | "failed" | "blocked" | "cancelled";

export interface RunState {
  thread?: string;
  goal?: string;
  workflow?: string;
  recentSteps: string[];
}

export interface AgentRun {
  id: string;
  skillId?: string;
  title: string;
  origin: "manual" | "schedule" | "context";
  status: RunStatus | string;
  summary: string;
  state: RunState;
  manualSteps: string[];
  onException: string;
  actions: AgentAction[];
  createdAt: number;
  finishedAt?: number;
}

export interface ActionDecision {
  actionId: string;
  approved: boolean;
  remember: boolean;
}

export interface AutonomyGrant {
  id: string;
  actionType: string;
  scopeKind: "global" | "skill" | "workspace";
  scopeValue?: string;
  scopeLabel: string;
  mode: "auto" | "ask" | "never";
  source: string;
  createdAt: number;
  expiresAt?: number;
}

export interface GrantRequest {
  actionType: string;
  scopeKind: "global" | "skill" | "workspace";
  scopeValue?: string;
  mode: "auto" | "ask" | "never";
  durationDays?: number;
}

export interface AutonomyProposal {
  actionType: string;
  actionTitle: string;
  scopeKind: string;
  scopeValue?: string;
  scopeLabel: string;
  approvals: number;
  message: string;
}

export interface CheckPreset {
  id: string;
  label: string;
}

export interface ApprovedWorkspace {
  id: string;
  label: string;
  path: string;
  checkPresets: CheckPreset[];
  createdAt: number;
}

export interface DetectedWorkspace {
  label: string;
  path: string;
}

export interface ActionKind {
  actionType: string;
  title: string;
  description: string;
  riskClass: string;
  riskLabel: string;
  defaultPolicy: string;
  interruptsUser: boolean;
  rollback: string;
  available: boolean;
}

export interface PolicyPreference {
  actionType: string;
  actionTitle: string;
  scopeLabel: string;
  approvals: number;
  rejections: number;
  automatic: number;
  rolledBack: number;
  tendency: string;
}

export interface AgentMetrics {
  runs: number;
  finishedRuns: number;
  completedRuns: number;
  taskCompletionRate?: number;
  actionsExecuted: number;
  verifiedActions: number;
  verificationRate?: number;
  approvalAcceptanceRate?: number;
  rollbackRate?: number;
  estimatedMinutesSaved: number;
  activeAutoGrants: number;
  highRiskActions: number;
  actionsLast7Days: number;
}

export interface AutonomyOverview {
  paused: boolean;
  maxActionsPerHour: number;
  autoActionsLastHour: number;
  catalog: ActionKind[];
  grants: AutonomyGrant[];
  proposals: AutonomyProposal[];
  workspaces: ApprovedWorkspace[];
  detectedWorkspaces: DetectedWorkspace[];
  policy: PolicyPreference[];
  metrics: AgentMetrics;
}

export interface Goal {
  id: string;
  title: string;
  topic: string;
  status: "inferred" | "confirmed";
  confidence: number;
  evidence: string[];
  activeDays: number;
  observedSeconds: number;
  lastActiveAt: number;
}

export interface StateStep {
  label: string;
  title: string;
  category: string;
  at: number;
  seconds: number;
}

export interface StateResource {
  label: string;
  kind: string;
  locator?: string;
  at: number;
}

export interface WorkflowProgress {
  workflowId: string;
  title: string;
  matchedSteps: number;
  totalSteps: number;
  nextStep: WorkflowStep;
  confidence: number;
  skillId?: string;
}

export interface UnresolvedItem {
  kind: "awaiting_approval" | "needs_attention" | "workflow_in_progress" | string;
  title: string;
  detail: string;
  runId?: string;
  at: number;
}

export interface WorkState {
  generatedAt: number;
  activeThread?: string;
  goal?: Goal;
  goals: Goal[];
  recentSteps: StateStep[];
  openResources: StateResource[];
  workflowProgress?: WorkflowProgress;
  unresolved: UnresolvedItem[];
}

export interface AgentOverview {
  paused: boolean;
  state: WorkState;
  attention: AgentRun[];
  awaitingCount: number;
  proposals: AutonomyProposal[];
  opportunityCount: number;
  workflowCount: number;
}
