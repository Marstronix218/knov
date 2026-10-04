import type { AgentAction, AgentRun, SkillTrigger } from "../types";

export type Tone = "ok" | "warn" | "danger" | "info" | "neutral";

const WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

export function weekdayName(index: number): string {
  return WEEKDAYS[Math.min(Math.max(index, 0), 6)];
}

export function hourLabel(hour: number): string {
  return new Intl.DateTimeFormat(undefined, { hour: "numeric" }).format(new Date(2026, 0, 5, hour));
}

export function triggerLabel(trigger: SkillTrigger): string {
  if (trigger.kind === "context") return "When you start this workflow";
  if (trigger.kind === "schedule") {
    const at = trigger.hour === undefined ? "" : ` at ${hourLabel(trigger.hour)}`;
    return trigger.weekday === undefined ? `Every day${at}` : `${weekdayName(trigger.weekday)}s${at}`;
  }
  return "Only when you run it";
}

export function originLabel(origin: AgentRun["origin"]): string {
  if (origin === "schedule") return "Scheduled";
  if (origin === "context") return "Started when you began this workflow";
  return "Started by you";
}

export function runStatus(status: string): { label: string; tone: Tone } {
  switch (status) {
    case "awaiting_approval": return { label: "Waiting for you", tone: "info" };
    case "ready": return { label: "Ready to run", tone: "info" };
    case "running": return { label: "Running", tone: "info" };
    case "completed": return { label: "Done", tone: "ok" };
    case "completed_with_exceptions": return { label: "Needs a look", tone: "warn" };
    case "failed": return { label: "Failed", tone: "danger" };
    case "blocked": return { label: "Blocked", tone: "danger" };
    case "cancelled": return { label: "Closed", tone: "neutral" };
    default: return { label: status, tone: "neutral" };
  }
}

export function actionStatus(status: string): { label: string; tone: Tone } {
  switch (status) {
    case "awaiting_approval": return { label: "Waiting for approval", tone: "info" };
    case "approved": return { label: "Approved · queued", tone: "info" };
    case "running": return { label: "Running", tone: "info" };
    case "succeeded": return { label: "Done · verified", tone: "ok" };
    case "needs_attention": return { label: "Needs a look", tone: "warn" };
    case "failed": return { label: "Failed", tone: "danger" };
    case "rejected": return { label: "Declined", tone: "neutral" };
    case "blocked": return { label: "Blocked", tone: "danger" };
    case "skipped": return { label: "Skipped", tone: "neutral" };
    case "rolled_back": return { label: "Undone", tone: "neutral" };
    default: return { label: status, tone: "neutral" };
  }
}

export function riskLabel(riskClass: string): string {
  switch (riskClass) {
    case "read_only": return "Read-only";
    case "ephemeral": return "Local · temporary";
    case "draft": return "Draft for review";
    case "persistent_reversible": return "Persistent · reversible";
    case "external_communication": return "Sends externally";
    case "destructive": return "Destructive";
    default: return riskClass;
  }
}

export function decisionLabel(action: AgentAction): string {
  switch (action.decision) {
    case "auto": return action.grantId ? "Allowed by your permission" : "Allowed automatically";
    case "approved": return "You approved this";
    case "rejected": return "You declined this";
    case "blocked": return "Blocked by policy";
    case "cancelled": return "Closed without a decision";
    default: return "Needs your approval";
  }
}

export function isTerminalRun(status: string): boolean {
  return !["awaiting_approval", "ready", "running"].includes(status);
}

export function relativeTime(seconds: number, now = Date.now()): string {
  const delta = Math.round(now / 1000 - seconds);
  if (delta < 45) return "just now";
  if (delta < 3_600) return `${Math.round(delta / 60)} min ago`;
  if (delta < 86_400) return `${Math.round(delta / 3_600)} h ago`;
  const days = Math.round(delta / 86_400);
  return days === 1 ? "yesterday" : `${days} days ago`;
}

export function percent(value?: number, digits = 0): string {
  return value === undefined || value === null ? "—" : `${(value * 100).toFixed(digits)}%`;
}

export function minutesLabel(seconds: number): string {
  const minutes = Math.round(seconds / 60);
  if (minutes < 1) return "under a minute";
  if (minutes < 90) return `${minutes} min`;
  return `${(minutes / 60).toFixed(1)} h`;
}
