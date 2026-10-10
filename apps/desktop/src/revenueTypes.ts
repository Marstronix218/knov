export interface RevenueClient { id: string; name: string; demo: boolean }
export interface RevenueProject { id: string; clientId: string; name: string; description: string; threadIds: string[]; demo: boolean }
export interface RevenueEvidence { id: string; projectId: string; source: string; sourceRef: string; excerpt: string; occurredAt: number; provenance: string; kind: string; demo: boolean }
export interface RevenueOpportunity {
  id: string; projectId: string; clientName: string; projectName: string;
  type: string; title: string; explanation: string; evidenceIds: string[];
  uncertainty: string[]; disproves: string[]; recommendedAction: string;
  status: string; confidence: number; provenance: string;
  amountCents: number | null; currency: string | null;
  clarificationQuestion: string | null; interviewId: string | null;
  createdAt: number; updatedAt: number; demo: boolean;
}
export interface RevenueDraft { id: string; opportunityId: string; kind: string; body: string; status: string; createdAt: number; updatedAt: number; demo: boolean }
export interface RevenueOutcome { id: string; opportunityId: string; kind: string; evidence: string; amountCents: number | null; currency: string | null; verified: boolean; createdAt: number; demo: boolean }
export interface RevenueOverview {
  demo: boolean;
  skillSuggestions?: { projectId: string; title: string; description: string; workflowId: string | null; confirmedOpportunityCount: number }[];
  clients: RevenueClient[]; projects: RevenueProject[];
  agreements: { id: string; projectId: string; title: string; [key: string]: unknown }[];
  evidence: RevenueEvidence[]; commitments: unknown[];
  opportunities: RevenueOpportunity[]; drafts: RevenueDraft[]; outcomes: RevenueOutcome[];
  metrics: { open: number; needsClarification: number; draftsAwaitingReview: number; resolved: number; potentialByCurrency: Record<string, number>; verifiedRecoveredByCurrency: Record<string, number>; reviewed: number; confirmed: number; dismissed: number; draftsApproved: number; actionsRecorded: number; paymentsConfirmed: number };
}
