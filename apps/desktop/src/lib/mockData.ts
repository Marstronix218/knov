import type {
  ActivityEvent,
  BrowserProfile,
  DashboardData,
  ProfileData,
  SettingsData,
} from "../types";

const now = Date.now();
const minutesAgo = (minutes: number) => new Date(now - minutes * 60_000).toISOString();

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
    topic: "Knoveyla implementation",
    source: "chrome",
  },
  {
    id: "event-2",
    appName: "Visual Studio Code",
    windowTitle: "knoveyla — App.tsx",
    startedAt: minutesAgo(42),
    durationSeconds: 1_680,
    topic: "Knoveyla implementation",
    source: "collector",
  },
  {
    id: "event-3",
    appName: "Notion",
    pageTitle: "Knoveyla launch notes",
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
      evidence: "Tauri documentation, Xcode, and the Knoveyla repository were your strongest recent cluster.",
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
    "You are building Knoveyla, a local-first behavioral context layer for personal AI. Your recent work is concentrated on macOS desktop architecture, privacy boundaries, and turning a detailed product specification into an alpha.",
  sections: [
    {
      id: "projects",
      title: "Active projects",
      items: [
        {
          id: "project-knoveyla",
          label: "Knoveyla",
          description: "Apple Silicon macOS alpha using Tauri, React, Rust, SQLite, and a Chrome extension.",
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
          label: "Knoveyla is local-first",
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
  launchAtLogin: false,
  selectedBrowserProfileIds: ["chrome-default", "chrome-profile-1"],
  excludedApps: ["1Password"],
  excludedDomains: ["bank.example"],
  collectionStatus: {
    enabled: true,
    accessibilityGranted: false,
    browserConnected: false,
    degradedReasons: ["Accessibility permission is not granted.", "Chrome extension is not connected."],
  },
};
