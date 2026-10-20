import { describe, expect, it } from "vitest";
import type { RevenueEvidence, RevenueOpportunity } from "../revenueTypes";
import { evidenceByline, isOpen, issueCopy, kindLabel, sourceLabel } from "./revenueCopy";

function opportunity(overrides: Partial<RevenueOpportunity> = {}): RevenueOpportunity {
  return {
    id: "opp-1",
    projectId: "project-1",
    clientName: "Acme Studio",
    projectName: "Website refresh",
    type: "scope_change",
    title: "Extra pages requested",
    explanation: "",
    evidenceIds: [],
    uncertainty: [],
    disproves: [],
    recommendedAction: "",
    status: "open",
    confidence: 0.8,
    provenance: "rule",
    amountCents: null,
    currency: null,
    clarificationQuestion: null,
    interviewId: null,
    createdAt: 0,
    updatedAt: 0,
    demo: true,
    ...overrides,
  };
}

function evidence(overrides: Partial<RevenueEvidence> = {}): RevenueEvidence {
  return {
    id: "ev-1",
    projectId: "project-1",
    source: "gmail",
    sourceRef: "msg-1",
    excerpt: "Can you also add a blog?",
    occurredAt: 0,
    provenance: "import",
    kind: "request",
    demo: true,
    ...overrides,
  };
}

describe("issueCopy", () => {
  it("frames scope changes as unbilled extra work for the named client", () => {
    const copy = issueCopy(opportunity({ type: "scope_change" }));
    expect(copy.label).toBe("Unbilled extra work");
    expect(copy.problem).toContain("Acme Studio asked for work that isn't in the contract");
    expect(copy.actionLabel).toBe("Prepare change order");
  });

  it("frames overdue commitments as a follow-up to send today", () => {
    const copy = issueCopy(opportunity({ type: "overdue_commitment", clientName: "Birch Co" }));
    expect(copy.label).toBe("Promise overdue");
    expect(copy.problem).toContain("You promised Birch Co a follow-up");
    expect(copy.recommendation).toBe("Send the promised follow-up today.");
    expect(copy.actionLabel).toBe("Draft follow-up");
  });

  it("frames unbilled milestones as an invoice to send", () => {
    const copy = issueCopy(opportunity({ type: "unbilled_milestone" }));
    expect(copy.label).toBe("Milestone not invoiced");
    expect(copy.problem).toContain("your billing records have no invoice");
    expect(copy.actionLabel).toBe("Prepare invoice");
  });

  it("hedges for any other type because billing data may be incomplete", () => {
    const copy = issueCopy(opportunity({ type: "possibly_unbilled_milestone" }));
    expect(copy.label).toBe("Possibly not invoiced");
    expect(copy.problem).toContain("may be incomplete");
    expect(copy.recommendation).toContain("Check your billing system");
    expect(copy.actionLabel).toBe("Prepare invoice");
  });
});

describe("sourceLabel", () => {
  it("maps real and demo sources to the same friendly name", () => {
    expect(sourceLabel("gmail")).toBe("Gmail");
    expect(sourceLabel("demo_gmail")).toBe("Gmail");
    expect(sourceLabel("demo_slack")).toBe("Slack");
    expect(sourceLabel("demo_document")).toBe("Contract");
    expect(sourceLabel("user_import")).toBe("Added by you");
  });

  it("falls back to the raw source with underscores replaced", () => {
    expect(sourceLabel("google_drive_file")).toBe("google drive file");
  });
});

describe("kindLabel", () => {
  it("maps evidence kinds to plain-language names", () => {
    expect(kindLabel("request")).toBe("Client request");
    expect(kindLabel("commitment")).toBe("Your promise");
    expect(kindLabel("milestone_accepted")).toBe("Milestone accepted");
    expect(kindLabel("billing_complete")).toBe("Billing sync");
  });

  it("falls back to the raw kind with underscores replaced", () => {
    expect(kindLabel("purchase_order")).toBe("purchase order");
  });
});

describe("evidenceByline", () => {
  it("shows only the source when there is no metadata", () => {
    expect(evidenceByline(evidence())).toBe("Gmail");
  });

  it("strips the email address from a sender and includes the subject", () => {
    const byline = evidenceByline(evidence({ meta: { from: "Dana Lee <dana@acme.test>", subject: "Blog pages" } }));
    expect(byline).toBe("Gmail · Dana Lee · Blog pages");
  });

  it("prefers the author over the sender and the channel over the subject", () => {
    const byline = evidenceByline(evidence({
      source: "demo_slack",
      meta: { author: "Sam", from: "Other <o@x.test>", channel: "#acme", subject: "ignored" },
    }));
    expect(byline).toBe("Slack · Sam · #acme");
  });
});

describe("isOpen", () => {
  it("treats resolved and dismissed opportunities as closed", () => {
    expect(isOpen(opportunity({ status: "resolved" }))).toBe(false);
    expect(isOpen(opportunity({ status: "dismissed" }))).toBe(false);
  });

  it("treats every other status as open", () => {
    expect(isOpen(opportunity({ status: "open" }))).toBe(true);
    expect(isOpen(opportunity({ status: "clarifying" }))).toBe(true);
  });
});
