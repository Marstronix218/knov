/** Native evidence compiler contract. Times are Unix seconds; ranges are [start, end). */
export type Confidence = "high" | "medium" | "low";
export type ReviewStatus = "pending" | "accepted" | "corrected" | "excluded" | "personal" | "uncertain";
export interface Project {
  id: string; name: string; description: string; color: string; status: "active" | "archived";
  aliases: string[]; keywords: string[]; domains: string[]; repositories: string[]; paths: string[];
  client: string; externalIdentifier: string; createdAt: number; updatedAt: number;
}
export interface EvidenceItem {
  id: string; startedAt: number; endedAt: number; durationSeconds: number;
  sourceTypes: string[]; application: string; sanitizedContext: string;
  suggestedProjectId: string | null; projectConfidence: Confidence;
  suggestedCategory: string; categoryConfidence: Confidence; explanation: string;
  sourceEventIds: number[]; sourceAvailable: boolean; inferenceMethod: "local";
  reviewStatus: ReviewStatus; projectOverride: string | null; categoryOverride: string | null;
  createdAt: number; updatedAt: number;
}
export interface RecordTemplate {
  id: string; name: string; description: string; categories: string[];
  dimensions: string[]; requiredFields: string[]; exportColumns: string[]; experimental: boolean;
}
export interface Allocation {
  projectId: string | null; project: string; client: string; category: string;
  seconds: number; hours: number; percentage: number; reviewStatus: string;
}
export interface RecordTotals {
  trackedSeconds: number; reviewedSeconds: number; unreviewedSeconds: number;
  excludedSeconds: number; unallocatedSeconds: number; allocations: Allocation[];
}
export interface BusinessRecord {
  id: string; name: string; templateId: string; startAt: number; endAt: number; projectIds: string[];
  version: number; status: "draft" | "needs_review" | "ready" | "certified" | "exported";
  totals: RecordTotals; createdAt: number; updatedAt: number;
}
/** Allowlisted disclosure. Never add evidence or source metadata to this type. */
export interface CertifiedSnapshot {
  schemaVersion: number; recordId: string; recordVersion: number; recordName: string;
  certifiedBy: string; periodStart: number; periodEnd: number; template: string;
  certifiedAt: number; certificationStatement: string; totals: RecordTotals;
}
export interface Certification {
  id: string; recordId: string; recordVersion: number; certifiedAt: number;
  certificationStatement: string; snapshot: CertifiedSnapshot; sha256: string;
}
export interface AuditEntry { id: string; action: string; subjectId: string; occurredAt: number }
export interface BusinessWorkspace {
  projects: Project[]; evidence: EvidenceItem[]; templates: RecordTemplate[];
  records: BusinessRecord[]; certifications: Certification[]; audit: AuditEntry[]; synthetic: boolean;
}
export type BusinessAction =
  | { action: "refresh" }
  | { action: "save_project"; project: Project }
  | { action: "delete_project"; projectId: string }
  | { action: "review"; evidenceId: string; status: ReviewStatus; projectId: string | null; category: string | null }
  | { action: "split"; evidenceId: string; splitAt: number }
  | { action: "bulk_accept"; evidenceIds: string[] }
  | { action: "generate_record"; name: string; templateId: string; startAt: number; endAt: number; projectIds: string[] }
  | { action: "revise_record"; recordId: string }
  | { action: "certify"; recordId: string; certifiedBy: string; statementAccepted: boolean; expectedVersion: number; expectedUpdatedAt: number }
  | { action: "delete_certification"; certificationId: string };
export interface ExportArtifact { certificationId: string; format: "csv" | "json"; fileName: string; content: string; sha256: string }
export const CERTIFICATION_STATEMENT = "I reviewed this record and confirm that, to the best of my knowledge, it reasonably represents my work during this period.";

/** Mirrors the native certification allowlist sanitizer for preview parity. */
export function sanitizeOutboundText(value: string): string {
  return value
    .split(/\s+/)
    .map((token) => ((token !== "/" && /^(?:\/|[a-z]:\\)/i.test(token)) || /\\|https?:\/\/|(?:token|password|api_?key|secret)=|^sk-/i.test(token)) ? "[redacted]" : token)
    .join(" ")
    .slice(0, 160);
}
