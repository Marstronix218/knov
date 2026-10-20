import { describe, expect, it } from "vitest";
import type { AgentAction } from "../types";
import {
  actionStatus,
  decisionLabel,
  hourLabel,
  isTerminalRun,
  minutesLabel,
  originLabel,
  percent,
  relativeTime,
  riskLabel,
  runStatus,
  triggerLabel,
  weekdayName,
} from "./agentFormat";

function action(overrides: Partial<AgentAction>): AgentAction {
  return {
    id: "action-1",
    runId: "run-1",
    stepId: "step-1",
    position: 0,
    actionType: "open_url",
    title: "Open the report",
    riskClass: "read_only",
    targetLabel: "https://example.com",
    rationale: "",
    scopeKind: "global",
    scopeLabel: "Everywhere",
    decision: "pending",
    decisionReason: "",
    status: "awaiting_approval",
    rollbackAvailable: false,
    canOpen: true,
    createdAt: 0,
    ...overrides,
  };
}

describe("weekdayName", () => {
  it("maps Monday-first indexes to day names", () => {
    expect(weekdayName(0)).toBe("Monday");
    expect(weekdayName(4)).toBe("Friday");
    expect(weekdayName(6)).toBe("Sunday");
  });

  it("clamps out-of-range indexes instead of returning undefined", () => {
    expect(weekdayName(-3)).toBe("Monday");
    expect(weekdayName(42)).toBe("Sunday");
  });
});

describe("hourLabel", () => {
  it("produces distinct, non-empty labels for morning and evening hours", () => {
    expect(hourLabel(9)).toMatch(/9/);
    expect(hourLabel(9)).not.toBe(hourLabel(21));
  });
});

describe("triggerLabel", () => {
  it("describes context and manual triggers", () => {
    expect(triggerLabel({ kind: "context" })).toBe("When you start this workflow");
    expect(triggerLabel({ kind: "manual" })).toBe("Only when you run it");
  });

  it("describes daily schedules with and without an hour", () => {
    expect(triggerLabel({ kind: "schedule" })).toBe("Every day");
    expect(triggerLabel({ kind: "schedule", hour: 9 })).toBe(`Every day at ${hourLabel(9)}`);
  });

  it("describes weekly schedules by weekday", () => {
    expect(triggerLabel({ kind: "schedule", weekday: 0 })).toBe("Mondays");
    expect(triggerLabel({ kind: "schedule", weekday: 4, hour: 17 })).toBe(`Fridays at ${hourLabel(17)}`);
  });
});

describe("originLabel", () => {
  it("names who or what started the run", () => {
    expect(originLabel("schedule")).toBe("Scheduled");
    expect(originLabel("context")).toBe("Started when you began this workflow");
    expect(originLabel("manual")).toBe("Started by you");
  });
});

describe("runStatus", () => {
  it("maps known statuses to labels and tones", () => {
    expect(runStatus("awaiting_approval")).toEqual({ label: "Waiting for you", tone: "info" });
    expect(runStatus("completed")).toEqual({ label: "Done", tone: "ok" });
    expect(runStatus("completed_with_exceptions")).toEqual({ label: "Needs a look", tone: "warn" });
    expect(runStatus("failed")).toEqual({ label: "Failed", tone: "danger" });
    expect(runStatus("blocked").tone).toBe("danger");
    expect(runStatus("cancelled")).toEqual({ label: "Closed", tone: "neutral" });
  });

  it("passes unknown statuses through with a neutral tone", () => {
    expect(runStatus("paused")).toEqual({ label: "paused", tone: "neutral" });
  });
});

describe("actionStatus", () => {
  it("maps known statuses to labels and tones", () => {
    expect(actionStatus("approved")).toEqual({ label: "Approved · queued", tone: "info" });
    expect(actionStatus("succeeded")).toEqual({ label: "Done · verified", tone: "ok" });
    expect(actionStatus("needs_attention").tone).toBe("warn");
    expect(actionStatus("rejected")).toEqual({ label: "Declined", tone: "neutral" });
    expect(actionStatus("rolled_back")).toEqual({ label: "Undone", tone: "neutral" });
    expect(actionStatus("failed").tone).toBe("danger");
  });

  it("passes unknown statuses through with a neutral tone", () => {
    expect(actionStatus("mystery")).toEqual({ label: "mystery", tone: "neutral" });
  });
});

describe("riskLabel", () => {
  it("labels each risk class in plain language", () => {
    expect(riskLabel("read_only")).toBe("Read-only");
    expect(riskLabel("ephemeral")).toBe("Local · temporary");
    expect(riskLabel("draft")).toBe("Draft for review");
    expect(riskLabel("persistent_reversible")).toBe("Persistent · reversible");
    expect(riskLabel("external_communication")).toBe("Sends externally");
    expect(riskLabel("destructive")).toBe("Destructive");
  });

  it("returns unknown risk classes unchanged", () => {
    expect(riskLabel("novel_risk")).toBe("novel_risk");
  });
});

describe("decisionLabel", () => {
  it("distinguishes automatic decisions backed by a permission grant", () => {
    expect(decisionLabel(action({ decision: "auto", grantId: "grant-1" }))).toBe("Allowed by your permission");
    expect(decisionLabel(action({ decision: "auto" }))).toBe("Allowed automatically");
  });

  it("labels explicit and policy decisions", () => {
    expect(decisionLabel(action({ decision: "approved" }))).toBe("You approved this");
    expect(decisionLabel(action({ decision: "rejected" }))).toBe("You declined this");
    expect(decisionLabel(action({ decision: "blocked" }))).toBe("Blocked by policy");
    expect(decisionLabel(action({ decision: "cancelled" }))).toBe("Closed without a decision");
  });

  it("asks for approval when no decision has been made", () => {
    expect(decisionLabel(action({ decision: "pending" }))).toBe("Needs your approval");
  });
});

describe("isTerminalRun", () => {
  it("treats waiting, ready, and running runs as still active", () => {
    expect(isTerminalRun("awaiting_approval")).toBe(false);
    expect(isTerminalRun("ready")).toBe(false);
    expect(isTerminalRun("running")).toBe(false);
  });

  it("treats every other status as terminal", () => {
    expect(isTerminalRun("completed")).toBe(true);
    expect(isTerminalRun("failed")).toBe(true);
    expect(isTerminalRun("cancelled")).toBe(true);
  });
});

describe("relativeTime", () => {
  const now = 1_800_000_000_000;
  const secondsAgo = (delta: number) => now / 1000 - delta;

  it("uses 'just now' for the first 44 seconds", () => {
    expect(relativeTime(secondsAgo(0), now)).toBe("just now");
    expect(relativeTime(secondsAgo(44), now)).toBe("just now");
  });

  it("switches to minutes, hours, and days at their boundaries", () => {
    expect(relativeTime(secondsAgo(45), now)).toBe("1 min ago");
    expect(relativeTime(secondsAgo(30 * 60), now)).toBe("30 min ago");
    expect(relativeTime(secondsAgo(3_600), now)).toBe("1 h ago");
    expect(relativeTime(secondsAgo(5 * 3_600), now)).toBe("5 h ago");
  });

  it("says 'yesterday' for one day and counts days after that", () => {
    expect(relativeTime(secondsAgo(86_400), now)).toBe("yesterday");
    expect(relativeTime(secondsAgo(3 * 86_400), now)).toBe("3 days ago");
  });
});

describe("percent", () => {
  it("formats fractions as percentages with optional decimals", () => {
    expect(percent(0.256)).toBe("26%");
    expect(percent(0.256, 1)).toBe("25.6%");
    expect(percent(1)).toBe("100%");
    expect(percent(0)).toBe("0%");
  });

  it("shows a dash when there is no value", () => {
    expect(percent(undefined)).toBe("—");
    expect(percent(null as unknown as number)).toBe("—");
  });
});

describe("minutesLabel", () => {
  it("reports sub-minute durations in words", () => {
    expect(minutesLabel(0)).toBe("under a minute");
    expect(minutesLabel(29)).toBe("under a minute");
  });

  it("uses whole minutes below 90 minutes", () => {
    expect(minutesLabel(30)).toBe("1 min");
    expect(minutesLabel(89 * 60)).toBe("89 min");
  });

  it("switches to fractional hours at 90 minutes", () => {
    expect(minutesLabel(90 * 60)).toBe("1.5 h");
    expect(minutesLabel(4 * 3_600)).toBe("4.0 h");
  });
});
