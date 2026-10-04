import type {
  ActivityEvent,
  BrowserProfile,
  DashboardData,
  ProfileData,
  SettingsData,
  PredictionDashboard,
  WorkPrediction,
} from "../types";

const now = Date.now();
const minutesAgo = (minutes: number) => new Date(now - minutes * 60_000).toISOString();
const secondsAgo = (minutes: number) => Math.floor((now - minutes * 60_000) / 1000);

export const mockActivity: ActivityEvent[] = [
  {
    id: "event-1",
    appName: "Google Chrome",
    pageTitle: "Tauri 2 — Security Capabilities",
    windowTitle: "Tauri 2 — Security Capabilities - Google Chrome",
    url: "https://v2.tauri.app/security/capabilities/",
    browserProfile: "Work",
    startedAt: minutesAgo(12),
    durationSeconds: 1_440,
    topic: "Knov implementation",
    source: "chrome",
  },
  {
    id: "event-2-save",
    appName: "Visual Studio Code",
    windowTitle: "Knov — apps/desktop/src/App.tsx",
    pageTitle: "apps/desktop/src/App.tsx",
    startedAt: minutesAgo(28),
    durationSeconds: 0,
    topic: "Knov implementation",
    source: "editor",
  },
  {
    id: "event-2",
    appName: "Visual Studio Code",
    windowTitle: "Knov — App.tsx",
    startedAt: minutesAgo(42),
    durationSeconds: 1_680,
    topic: "Knov implementation",
    source: "collector",
  },
  {
    id: "event-3",
    appName: "Notion",
    pageTitle: "Knov launch notes",
    url: "https://notion.so/example",
    browserProfile: "Work",
    startedAt: minutesAgo(78),
    durationSeconds: 1_260,
    topic: "Product planning",
    source: "chrome",
  },
  {
    id: "event-4",
    appName: "YouTube",
    pageTitle: "Building native macOS apps with Tauri",
    url: "https://youtube.com/watch?v=example",
    browserProfile: "Personal",
    startedAt: minutesAgo(110),
    durationSeconds: 980,
    topic: "Desktop development",
    source: "history",
  },
];

export const mockDashboard: DashboardData = {
  range: "today",
  trackedSeconds: 21_960,
  focusedSeconds: 16_740,
  activeTopics: [
    { name: "Software development", count: 18 },
    { name: "Planning and notes", count: 8 },
    { name: "Web research", count: 7 },
  ],
  appUsage: [
    { name: "Chrome", seconds: 7_900, percentage: 36, color: "#adff2f", detail: "42 pages" },
    { name: "VS Code", seconds: 6_140, percentage: 28, color: "#58c7ff", detail: "3 projects" },
    { name: "Notion", seconds: 3_520, percentage: 16, color: "#c6a8ff", detail: "8 pages" },
    { name: "Terminal", seconds: 2_420, percentage: 11, color: "#ffac66", detail: "7 sessions" },
    { name: "Other", seconds: 1_980, percentage: 9, color: "#78828f" },
  ],
  siteUsage: [
    { name: "github.com", seconds: 4_160, percentage: 31, color: "#adff2f" },
    { name: "tauri.app", seconds: 2_730, percentage: 20, color: "#58c7ff" },
    { name: "notion.so", seconds: 2_190, percentage: 16, color: "#c6a8ff" },
    { name: "youtube.com", seconds: 1_940, percentage: 14, color: "#ffac66" },
    { name: "Other", seconds: 2_620, percentage: 19, color: "#78828f" },
  ],
  recentActivity: mockActivity,
  insights: [
    {
      id: "insight-1",
      title: "Native desktop development",
      description: "Your research clustered around Tauri security, macOS permissions, and browser messaging.",
      metric: "18 resources",
      evidence: "Based on active page titles and URLs from the last 7 days.",
    },
    {
      id: "insight-2",
      title: "Focused implementation block",
      description: "Your longest uninterrupted focused session today was in VS Code.",
      metric: "1h 34m",
      evidence: "Foreground-app focus excluding idle and locked time.",
    },
    {
      id: "insight-3",
      title: "Video research",
      description: "Several active video pages were related to local AI and macOS development.",
      metric: "7 video pages",
      evidence: "This counts active pages, not completed videos.",
    },
  ],
  recommendations: [
    {
      id: "recommendation-1",
      kind: "continuity",
      title: "Continue the native collector",
      body: "Your recent work moved from product requirements into Tauri security research. The next coherent step is validating the macOS permission bridge.",
      evidence: "Tauri documentation, Xcode, and the Knov repository were your strongest recent cluster.",
      createdAt: minutesAgo(5),
    },
    {
      id: "recommendation-2",
      kind: "behavioral",
      title: "A short reset may help",
      body: "You have been active for a sustained block. Consider stepping away before the next implementation pass.",
      evidence: "1h 34m of continuous foreground activity with no idle period longer than five minutes.",
      createdAt: minutesAgo(5),
    },
  ],
};

export const mockProfile: ProfileData = {
  summary:
    "You are building Knov, a memory-efficient personal AI agent. Your recent work is concentrated on local-first architecture, selective memory retrieval, and measurable context reduction.",
  sections: [
    {
      id: "projects",
      title: "Active projects",
      items: [
        {
          id: "project-knov",
          label: "Knov",
          description: "Apple Silicon macOS alpha using Tauri, React, Rust, SQLite, and local browser-history import.",
          confidence: 0.99,
          provenance: "inferred",
        },
      ],
    },
    {
      id: "skills",
      title: "Skills and tools",
      items: [
        { id: "skill-ts", label: "TypeScript & React", confidence: 0.95, provenance: "inferred" },
        { id: "skill-ai", label: "AI-assisted product development", confidence: 0.88, provenance: "inferred" },
        { id: "skill-product", label: "Product specification", confidence: 0.84, provenance: "observed" },
      ],
    },
    {
      id: "truth",
      title: "Your corrections",
      items: [
        {
          id: "truth-local",
          label: "Knov is local-first",
          description: "Raw behavioral history must stay on this Mac.",
          provenance: "user",
        },
      ],
    },
  ],
  updatedAt: minutesAgo(5),
};

export const mockBrowsers: BrowserProfile[] = [
  {
    id: "chrome-default",
    browser: "chrome",
    name: "Default",
    path: "~/Library/Application Support/Google/Chrome/Default",
    selected: true,
    support: "required",
  },
  {
    id: "chrome-profile-1",
    browser: "chrome",
    name: "Work",
    path: "~/Library/Application Support/Google/Chrome/Profile 1",
    selected: true,
    support: "required",
  },
  {
    id: "safari-default",
    browser: "safari",
    name: "Safari",
    path: "~/Library/Safari",
    selected: false,
    support: "best-effort",
  },
];

export const mockSettings: SettingsData = {
  provider: "openai",
  hasProviderKey: false,
  behavioralGuidanceEnabled: true,
  predictionExperimentEnabled: false,
  predictionDisplayThreshold: 0.65,
  launchAtLogin: false,
  selectedBrowserProfileIds: ["chrome-default", "chrome-profile-1"],
  excludedApps: ["1Password"],
  excludedDomains: ["bank.example"],
  collectionStatus: {
    enabled: true,
    accessibilityGranted: false,
    degradedReasons: ["Accessibility permission is not granted."],
  },
};

export const mockPredictions: WorkPrediction[] = [
  {
    id: "prediction-provider-1",
    createdAt: secondsAgo(4),
    source: "provider",
    intent: "Continue validating the Knov permission bridge",
    nextAction: "Review the Tauri capability settings and continue the desktop implementation.",
    nextResource: {
      type: "url",
      label: "Tauri security capabilities",
      safeLocator: "https://v2.tauri.app/security/capabilities/",
    },
    threadId: "knov-implementation",
    confidence: 0.78,
    horizonMinutes: 20,
    reasoningSummary: "Your recent work stayed in the Knov implementation thread and moved between code and Tauri security guidance.",
    evidence: [
      "28 min in Visual Studio Code",
      "Recent Tauri security reference",
      "Same work thread across recent activity",
    ],
    evaluationStatus: "pending",
    expiresAt: secondsAgo(-16),
  },
  {
    id: "prediction-baseline-1",
    createdAt: secondsAgo(4),
    source: "heuristic",
    intent: "Return to Knov implementation",
    nextAction: "Resume the most recently active work thread.",
    threadId: "knov-implementation",
    confidence: 0.61,
    horizonMinutes: 20,
    reasoningSummary: "This was the most recently active thread.",
    evidence: ["Most recently active thread"],
    evaluationStatus: "pending",
    expiresAt: secondsAgo(-16),
  },
];

export const mockPredictionDashboard: PredictionDashboard = {
  enabled: false,
  predictions: mockPredictions,
  stats: {
    totalPredictions: 18,
    evaluatedPredictions: 14,
    matched: 7,
    partial: 4,
    missed: 3,
    providerTop1Accuracy: 0.64,
    baselineTop1Accuracy: 0.43,
    highConfidenceAccuracy: 0.71,
    userPositiveFeedbackRate: 0.75,
  },
};
