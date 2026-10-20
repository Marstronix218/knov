export interface RevenueClient { id: string; name: string; demo: boolean }
export interface RevenueProject { id: string; clientId: string; name: string; description: string; threadIds: string[]; demo: boolean }
export interface RevenueEvidenceMeta { from?: string; to?: string; subject?: string; channel?: string; author?: string; apps?: string; hours?: number; thread?: string; number?: string; status?: string; customer?: string }
export interface RevenueEvidence { id: string; projectId: string; source: string; sourceRef: string; excerpt: string; occurredAt: number; dueAt?: number | null; amountCents?: number | null; currency?: string | null; provenance: string; kind: string; demo: boolean; meta?: RevenueEvidenceMeta }
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
/** One of the fictional businesses the sandbox can load. */
export interface SimulationBusiness {
  id: string; name: string; industry: string; location: string; team: string; currency: string;
  owner: { name: string; email: string }; tools: string[]; profile: string;
  /** Plain-language checklist of what Knov should find, for testers. */
  whatToExpect: string[];
  /** The same checklist as data: client, finding type, final status, and amount. */
  expected: { client: string; type: string; status: string; amountCents: number | null; currency: string | null }[];
  clientCount: number; interviewTopic: string; interviewName: string;
}
export interface SimulationCatalog {
  active: string | null;
  businesses: SimulationBusiness[];
  /** The scripted interview started for the active business, if any. */
  interview: { sessionId: string; status: string } | null;
}
export interface RevenueOverview {
  demo: boolean;
  /** The sandbox business these records belong to; null for actual data. */
  business?: SimulationBusiness | null;
  skillSuggestions?: { projectId: string; title: string; description: string; workflowId: string | null; confirmedOpportunityCount: number }[];
  clients: RevenueClient[]; projects: RevenueProject[];
  agreements: { id: string; projectId: string; title: string; text?: string; createdAt?: number; [key: string]: unknown }[];
  evidence: RevenueEvidence[]; commitments: unknown[];
  opportunities: RevenueOpportunity[]; drafts: RevenueDraft[]; outcomes: RevenueOutcome[];
  metrics: { open: number; needsClarification: number; draftsAwaitingReview: number; resolved: number; potentialByCurrency: Record<string, number>; verifiedRecoveredByCurrency: Record<string, number>; reviewed: number; confirmed: number; dismissed: number; draftsApproved: number; actionsRecorded: number; paymentsConfirmed: number };
}

/** Text extracted from an uploaded document, for review before import. */
export interface ExtractedDocument { title: string; text: string }
