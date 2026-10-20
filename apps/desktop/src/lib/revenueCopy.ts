import type { RevenueEvidence, RevenueOpportunity } from "../revenueTypes";

export interface IssueCopy {
  /** Short badge, e.g. "Unbilled extra work". */
  label: string;
  /** What is wrong, in one plain sentence. */
  problem: string;
  /** What Knov recommends doing about it. */
  recommendation: string;
  /** Label of the button that prepares the recommended action. */
  actionLabel: string;
}

/** Plain-language framing of a rule-detected opportunity for the Home page. */
export function issueCopy(opportunity: RevenueOpportunity): IssueCopy {
  const client = opportunity.clientName;
  switch (opportunity.type) {
    case "scope_change":
      return {
        label: "Unbilled extra work",
        problem: `${client} asked for work that isn't in the contract, and your team has already started it. Knov found no change order or approval.`,
        recommendation: "Send a change order before more work is done so the extra work gets paid.",
        actionLabel: "Prepare change order",
      };
    case "overdue_commitment":
      return {
        label: "Promise overdue",
        problem: `You promised ${client} a follow-up and the deadline has passed. A late follow-up puts the deal at risk.`,
        recommendation: "Send the promised follow-up today.",
        actionLabel: "Draft follow-up",
      };
    case "unbilled_milestone":
      return {
        label: "Milestone not invoiced",
        problem: `${client} accepted a paid milestone, and your billing records have no invoice for it.`,
        recommendation: "Send the invoice now.",
        actionLabel: "Prepare invoice",
      };
    default:
      return {
        label: "Possibly not invoiced",
        problem: `${client} accepted a milestone, but Knov found no invoice for it. Your billing records may be incomplete.`,
        recommendation: "Check your billing system, and invoice the milestone if it hasn't been. Connecting billing data lets Knov confirm the amount.",
        actionLabel: "Prepare invoice",
      };
  }
}

const SOURCE_LABELS: Record<string, string> = {
  gmail: "Gmail",
  demo_gmail: "Gmail",
  slack: "Slack",
  demo_slack: "Slack",
  demo_thread: "Work activity",
  document: "Contract",
  demo_document: "Contract",
  demo_billing: "Billing",
  user_import: "Added by you",
};

export const sourceLabel = (source: string) => SOURCE_LABELS[source] ?? source.replace(/_/g, " ");

const KIND_LABELS: Record<string, string> = {
  request: "Client request",
  activity: "Work done",
  approval: "Approval",
  commitment: "Your promise",
  milestone_accepted: "Milestone accepted",
  invoice: "Invoice",
  billing_complete: "Billing sync",
  agreement: "Contract",
  communication: "Message",
};

export const kindLabel = (kind: string) => KIND_LABELS[kind] ?? kind.replace(/_/g, " ");

/** Who said it and where, for an evidence quote. */
export function evidenceByline(evidence: RevenueEvidence): string {
  const meta = evidence.meta ?? {};
  const who = meta.author ?? meta.from?.replace(/\s*<.*>$/, "");
  const where = meta.channel ?? meta.subject;
  return [sourceLabel(evidence.source), who, where].filter(Boolean).join(" · ");
}

export const isOpen = (opportunity: RevenueOpportunity) => !["resolved", "dismissed"].includes(opportunity.status);
