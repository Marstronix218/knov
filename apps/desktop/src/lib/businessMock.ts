import {
  CERTIFICATION_STATEMENT,
  sanitizeOutboundText,
  type Allocation,
  type BusinessAction,
  type BusinessRecord,
  type BusinessWorkspace,
  type Certification,
  type CertifiedSnapshot,
  type EvidenceItem,
  type ExportArtifact,
  type Project,
  type RecordTemplate,
} from "../businessTypes";

const STORAGE_KEY = "knov.business-workspace.v1";
const VALID_REVIEW = new Set(["accepted", "corrected"]);

const nowSeconds = () => Math.floor(Date.now() / 1000);
const id = (prefix: string) => `${prefix}-${crypto.randomUUID()}`;

function dayAt(daysAgo: number, hour: number, minute = 0): number {
  const date = new Date();
  date.setHours(hour, minute, 0, 0);
  date.setDate(date.getDate() - daysAgo);
  return Math.floor(date.getTime() / 1000);
}

const templates: RecordTemplate[] = [
  {
    id: "generic-allocation",
    name: "Generic Project Allocation",
    description: "A reusable project and category time allocation record.",
    categories: ["Delivery", "Planning", "Internal / Administrative", "Unallocated"],
    dimensions: ["project", "category", "review status"],
    requiredFields: ["period", "project", "hours", "review status"],
    exportColumns: ["project", "client", "category", "hours", "percentage", "review_status"],
    experimental: false,
  },
  {
    id: "rnd-allocation",
    name: "R&D Allocation — Demo",
    description: "Experimental allocation prototype. It is not tax, legal, or accounting advice.",
    categories: ["Direct Research", "Direct Supervision", "Direct Support", "Non-R&D / Unqualified", "Unallocated"],
    dimensions: ["business component", "activity category", "review status"],
    requiredFields: ["period", "business component", "category", "hours"],
    exportColumns: ["project", "category", "hours", "percentage", "review_status"],
    experimental: true,
  },
  {
    id: "professional-services",
    name: "Professional Services Allocation",
    description: "A thin client, matter, and billable/non-billable allocation.",
    categories: ["Billable", "Non-billable", "Business development", "Unallocated"],
    dimensions: ["client", "project or matter", "billing category"],
    requiredFields: ["period", "client", "project", "hours"],
    exportColumns: ["client", "project", "category", "hours", "percentage", "review_status"],
    experimental: false,
  },
];

function seedProjects(): Project[] {
  const createdAt = dayAt(45, 9);
  return [
    {
      id: "project-search-ranking-v2", name: "Search Ranking V2", description: "Ranking quality and relevance improvements.", color: "#b7ff3c", status: "active",
      aliases: ["ranking-v2", "relevance"], keywords: ["ranking", "reranker", "evaluation"], domains: ["github.com", "docs.rs"], repositories: ["knov/search-ranking"], paths: ["/search-ranking/"],
      client: "Northstar Labs", externalIdentifier: "NS-RANK-02", createdAt, updatedAt: createdAt,
    },
    {
      id: "project-authentication", name: "Authentication", description: "Session security and sign-in reliability.", color: "#65cdfd", status: "active",
      aliases: ["auth", "identity"], keywords: ["oauth", "session", "token"], domains: ["auth0.com", "github.com"], repositories: ["knov/auth"], paths: ["/auth/", "/session/"],
      client: "Northstar Labs", externalIdentifier: "NS-AUTH-11", createdAt, updatedAt: createdAt,
    },
    {
      id: "project-internal-operations", name: "Internal Operations", description: "Team coordination, planning, and administration.", color: "#f5b942", status: "active",
      aliases: ["ops", "admin"], keywords: ["planning", "finance", "team"], domains: ["slack.com"], repositories: [], paths: [],
      client: "", externalIdentifier: "INTERNAL", createdAt, updatedAt: createdAt,
    },
  ];
}

type EvidenceSeed = Pick<EvidenceItem, "id" | "application" | "sanitizedContext" | "sourceTypes" | "suggestedProjectId" | "projectConfidence" | "suggestedCategory" | "categoryConfidence" | "explanation" | "reviewStatus" | "projectOverride" | "categoryOverride"> & { daysAgo: number; hour: number; minute?: number; duration: number };

function seedEvidence(): EvidenceItem[] {
  const seeds: EvidenceSeed[] = [
    { id: "ev-ranking-code", daysAgo: 2, hour: 9, duration: 5400, application: "Visual Studio Code", sanitizedContext: "search-ranking / reranker evaluation", sourceTypes: ["foreground_app", "editor", "git"], suggestedProjectId: "project-search-ranking-v2", projectConfidence: "high", suggestedCategory: "Direct Research", categoryConfidence: "high", explanation: "Repository and path match Search Ranking V2; title includes ‘reranker’.", reviewStatus: "accepted", projectOverride: null, categoryOverride: null },
    { id: "ev-ranking-terminal", daysAgo: 2, hour: 10, minute: 35, duration: 2100, application: "Terminal", sanitizedContext: "search-ranking — local evaluation command", sourceTypes: ["foreground_app", "git"], suggestedProjectId: "project-search-ranking-v2", projectConfidence: "high", suggestedCategory: "Direct Research", categoryConfidence: "high", explanation: "Active repository and evaluation command both matched Search Ranking V2.", reviewStatus: "pending", projectOverride: null, categoryOverride: null },
    { id: "ev-ranking-github", daysAgo: 2, hour: 11, minute: 20, duration: 1800, application: "Google Chrome", sanitizedContext: "GitHub · Pull request: ranking evaluation metrics", sourceTypes: ["chrome_history", "foreground_app"], suggestedProjectId: "project-search-ranking-v2", projectConfidence: "high", suggestedCategory: "Direct Supervision", categoryConfidence: "medium", explanation: "Repository name and ranking keyword both matched.", reviewStatus: "pending", projectOverride: null, categoryOverride: null },
    { id: "ev-auth-code", daysAgo: 1, hour: 9, duration: 4200, application: "Visual Studio Code", sanitizedContext: "auth / session renewal", sourceTypes: ["foreground_app", "editor", "git"], suggestedProjectId: "project-authentication", projectConfidence: "high", suggestedCategory: "Direct Support", categoryConfidence: "high", explanation: "Repository and session path matched Authentication.", reviewStatus: "corrected", projectOverride: "project-authentication", categoryOverride: "Direct Research" },
    { id: "ev-auth-docs", daysAgo: 1, hour: 10, minute: 20, duration: 2400, application: "Google Chrome", sanitizedContext: "OAuth token rotation documentation", sourceTypes: ["foreground_app", "chrome_history"], suggestedProjectId: "project-authentication", projectConfidence: "medium", suggestedCategory: "Direct Research", categoryConfidence: "medium", explanation: "OAuth keyword matched; no repository signal was present.", reviewStatus: "pending", projectOverride: null, categoryOverride: null },
    { id: "ev-ambiguous-docs", daysAgo: 1, hour: 11, minute: 10, duration: 1500, application: "Google Chrome", sanitizedContext: "API design guidelines", sourceTypes: ["foreground_app", "chrome_history"], suggestedProjectId: null, projectConfidence: "low", suggestedCategory: "Unallocated", categoryConfidence: "low", explanation: "The page title could apply to multiple active projects.", reviewStatus: "uncertain", projectOverride: null, categoryOverride: null },
    { id: "ev-slack", daysAgo: 0, hour: 9, duration: 1800, application: "Slack", sanitizedContext: "Team planning · weekly priorities", sourceTypes: ["foreground_app"], suggestedProjectId: "project-internal-operations", projectConfidence: "medium", suggestedCategory: "Internal / Administrative", categoryConfidence: "high", explanation: "Planning keyword and Slack domain matched Internal Operations.", reviewStatus: "accepted", projectOverride: null, categoryOverride: null },
    { id: "ev-admin", daysAgo: 0, hour: 9, minute: 35, duration: 1200, application: "Google Chrome", sanitizedContext: "Operations · invoice reconciliation", sourceTypes: ["foreground_app", "chrome_history"], suggestedProjectId: "project-internal-operations", projectConfidence: "medium", suggestedCategory: "Internal / Administrative", categoryConfidence: "high", explanation: "Administrative keyword matched Internal Operations.", reviewStatus: "pending", projectOverride: null, categoryOverride: null },
    { id: "ev-personal", daysAgo: 0, hour: 10, duration: 900, application: "Google Chrome", sanitizedContext: "Personal calendar", sourceTypes: ["foreground_app", "chrome_history"], suggestedProjectId: null, projectConfidence: "low", suggestedCategory: "Unallocated", categoryConfidence: "low", explanation: "No business project signal was found.", reviewStatus: "personal", projectOverride: null, categoryOverride: null },
    { id: "ev-low-confidence", daysAgo: 0, hour: 10, minute: 20, duration: 1500, application: "Notes", sanitizedContext: "Architecture follow-ups", sourceTypes: ["foreground_app"], suggestedProjectId: null, projectConfidence: "low", suggestedCategory: "Unallocated", categoryConfidence: "low", explanation: "The title could apply to multiple active projects.", reviewStatus: "pending", projectOverride: null, categoryOverride: null },
  ];
  const createdAt = nowSeconds();
  return seeds.map(({ daysAgo, hour, minute = 0, duration, ...seed }) => {
    const startedAt = dayAt(daysAgo, hour, minute);
    return { ...seed, startedAt, endedAt: startedAt + duration, durationSeconds: duration, sourceEventIds: [Math.abs(startedAt % 100000)], sourceAvailable: true, inferenceMethod: "local", createdAt, updatedAt: createdAt };
  });
}

function effectiveProject(item: EvidenceItem): string | null {
  return item.projectOverride ?? item.suggestedProjectId;
}

function effectiveCategory(item: EvidenceItem): string {
  return item.categoryOverride ?? item.suggestedCategory;
}

function relevantEvidence(workspace: BusinessWorkspace, record: Pick<BusinessRecord, "startAt" | "endAt" | "projectIds">): EvidenceItem[] {
  return workspace.evidence.filter((item) => item.startedAt < record.endAt && item.endedAt > record.startAt);
}

function calculateTotals(workspace: BusinessWorkspace, record: Pick<BusinessRecord, "templateId" | "startAt" | "endAt" | "projectIds">) {
  const template = workspace.templates.find((candidate) => candidate.id === record.templateId);
  if (!template) throw new Error("Record template no longer exists.");
  const items = relevantEvidence(workspace, record);
  const durationInRange = (item: EvidenceItem) => Math.max(0, Math.min(item.endedAt, record.endAt) - Math.max(item.startedAt, record.startAt));
  const trackedSeconds = items.reduce((sum, item) => sum + durationInRange(item), 0);
  const excludedSeconds = items.filter((item) => item.reviewStatus === "excluded" || item.reviewStatus === "personal").reduce((sum, item) => sum + durationInRange(item), 0);
  const included = items.filter((item) => item.reviewStatus !== "excluded" && item.reviewStatus !== "personal");
  const reviewedSeconds = included.filter((item) => VALID_REVIEW.has(item.reviewStatus)).reduce((sum, item) => sum + durationInRange(item), 0);
  const unreviewedSeconds = included.filter((item) => !VALID_REVIEW.has(item.reviewStatus)).reduce((sum, item) => sum + durationInRange(item), 0);
  const unallocatedSeconds = included.filter((item) => !effectiveProject(item) || (record.projectIds.length > 0 && !record.projectIds.includes(effectiveProject(item)!)) || !template.categories.includes(effectiveCategory(item)) || effectiveCategory(item) === "Unallocated").reduce((sum, item) => sum + durationInRange(item), 0);
  const grouped = new Map<string, Allocation>();
  for (const item of included) {
    const resolvedId = effectiveProject(item);
    const valid = Boolean(resolvedId) && (record.projectIds.length === 0 || record.projectIds.includes(resolvedId!)) && template.categories.includes(effectiveCategory(item)) && effectiveCategory(item) !== "Unallocated";
    const projectId = valid ? resolvedId : null;
    const project = workspace.projects.find((candidate) => candidate.id === projectId);
    const category = valid ? effectiveCategory(item) : "Unallocated";
    const key = `${projectId ?? "unallocated"}\u0000${category}\u0000${item.reviewStatus}`;
    const allocation = grouped.get(key) ?? { projectId, project: project?.name ?? "Unallocated", client: project?.client ?? "", category, seconds: 0, hours: 0, percentage: 0, reviewStatus: item.reviewStatus };
    allocation.seconds += durationInRange(item);
    grouped.set(key, allocation);
  }
  const denominator = Math.max(1, included.reduce((sum, item) => sum + durationInRange(item), 0));
  const allocations = [...grouped.values()].map((allocation) => ({ ...allocation, hours: Number((allocation.seconds / 3600).toFixed(2)), percentage: Number(((allocation.seconds / denominator) * 100).toFixed(1)) }));
  return { trackedSeconds, reviewedSeconds, unreviewedSeconds, excludedSeconds, unallocatedSeconds, allocations };
}

function draftStatus(totals: BusinessRecord["totals"]): BusinessRecord["status"] {
  return totals.unreviewedSeconds === 0 && totals.unallocatedSeconds === 0 && totals.reviewedSeconds > 0 ? "ready" : "needs_review";
}

function seedWorkspace(): BusinessWorkspace {
  const projects = seedProjects();
  const evidence = seedEvidence();
  const workspace: BusinessWorkspace = { projects, evidence, templates, records: [], certifications: [], audit: [], synthetic: true };
  const startAt = dayAt(7, 0);
  const endAt = dayAt(-1, 0);
  const record: BusinessRecord = { id: "record-rnd-draft", name: "Weekly R&D allocation", templateId: "rnd-allocation", startAt, endAt, projectIds: ["project-search-ranking-v2", "project-authentication"], version: 1, status: "draft", totals: { trackedSeconds: 0, reviewedSeconds: 0, unreviewedSeconds: 0, excludedSeconds: 0, unallocatedSeconds: 0, allocations: [] }, createdAt: nowSeconds(), updatedAt: nowSeconds() };
  record.totals = calculateTotals(workspace, record);
  record.status = draftStatus(record.totals);
  workspace.records.push(record);
  return workspace;
}

function load(): BusinessWorkspace {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (!stored) return seedWorkspace();
  try { return JSON.parse(stored) as BusinessWorkspace; } catch { return seedWorkspace(); }
}

function persist(workspace: BusinessWorkspace): BusinessWorkspace {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(workspace));
  return structuredClone(workspace);
}

function refreshDrafts(workspace: BusinessWorkspace) {
  workspace.records = workspace.records.map((record) => {
    if (record.status === "certified" || record.status === "exported") return record;
    const totals = calculateTotals(workspace, record);
    const next = { ...record, totals, status: draftStatus(totals) };
    return JSON.stringify(next.totals) === JSON.stringify(record.totals) && next.status === record.status
      ? record
      : { ...next, updatedAt: Math.max(nowSeconds(), record.updatedAt + 1) };
  });
}

function stableStringify(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  if (value && typeof value === "object") return `{${Object.entries(value as Record<string, unknown>).sort(([a], [b]) => a.localeCompare(b)).map(([key, nested]) => `${JSON.stringify(key)}:${stableStringify(nested)}`).join(",")}}`;
  return JSON.stringify(value);
}

// Small dependency-free SHA-256 implementation keeps browser preview and tests byte-identical.
export function sha256(input: string): string {
  const bytes = new TextEncoder().encode(input);
  const words: number[] = [];
  const bitLength = bytes.length * 8;
  for (const byte of bytes) words.push(byte);
  words.push(0x80);
  while ((words.length % 64) !== 56) words.push(0);
  for (let index = 7; index >= 0; index--) words.push(index < 4 ? (bitLength >>> (index * 8)) & 0xff : 0);
  const h = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
  const k = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ];
  const rotr = (value: number, amount: number) => (value >>> amount) | (value << (32 - amount));
  for (let offset = 0; offset < words.length; offset += 64) {
    const w = new Array<number>(64);
    for (let index = 0; index < 16; index++) w[index] = ((words[offset + index * 4] << 24) | (words[offset + index * 4 + 1] << 16) | (words[offset + index * 4 + 2] << 8) | words[offset + index * 4 + 3]) >>> 0;
    for (let index = 16; index < 64; index++) {
      const s0 = rotr(w[index - 15], 7) ^ rotr(w[index - 15], 18) ^ (w[index - 15] >>> 3);
      const s1 = rotr(w[index - 2], 17) ^ rotr(w[index - 2], 19) ^ (w[index - 2] >>> 10);
      w[index] = (w[index - 16] + s0 + w[index - 7] + s1) >>> 0;
    }
    let [a, b, c, d, e, f, g, hh] = h;
    for (let index = 0; index < 64; index++) {
      const s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
      const choice = (e & f) ^ (~e & g);
      const temp1 = (hh + s1 + choice + k[index] + w[index]) >>> 0;
      const s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
      const majority = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = (s0 + majority) >>> 0;
      hh = g; g = f; f = e; e = (d + temp1) >>> 0; d = c; c = b; b = a; a = (temp1 + temp2) >>> 0;
    }
    [a, b, c, d, e, f, g, hh].forEach((value, index) => { h[index] = (h[index] + value) >>> 0; });
  }
  return h.map((value) => value.toString(16).padStart(8, "0")).join("");
}

function certificationSnapshot(workspace: BusinessWorkspace, record: BusinessRecord, certifiedBy: string, certifiedAt: number): CertifiedSnapshot {
  const template = workspace.templates.find((candidate) => candidate.id === record.templateId)!;
  const totals = structuredClone(record.totals);
  totals.allocations.forEach((allocation) => { allocation.project = sanitizeOutboundText(allocation.project); allocation.client = sanitizeOutboundText(allocation.client); allocation.category = sanitizeOutboundText(allocation.category); allocation.reviewStatus = sanitizeOutboundText(allocation.reviewStatus); });
  return { schemaVersion: 1, recordId: record.id, recordVersion: record.version, recordName: sanitizeOutboundText(record.name), certifiedBy: sanitizeOutboundText(certifiedBy), periodStart: record.startAt, periodEnd: record.endAt, template: sanitizeOutboundText(template.name), certifiedAt, certificationStatement: CERTIFICATION_STATEMENT, totals };
}

export async function mockWorkspace(): Promise<BusinessWorkspace> { return structuredClone(load()); }

export async function mockMutate(request: BusinessAction): Promise<BusinessWorkspace> {
  const workspace = load();
  const occurredAt = nowSeconds();
  if (request.action === "refresh") return persist(workspace);
  if (request.action === "save_project") {
    const project = { ...request.project, name: request.project.name.trim(), updatedAt: occurredAt, createdAt: request.project.createdAt || occurredAt };
    if (!project.name) throw new Error("Project name is required.");
    const index = workspace.projects.findIndex((candidate) => candidate.id === project.id);
    if (index >= 0) workspace.projects[index] = project; else workspace.projects.push(project);
    workspace.audit.push({ id: id("audit"), action: index >= 0 ? "project_updated" : "project_created", subjectId: project.id, occurredAt });
  } else if (request.action === "delete_project") {
    if (workspace.evidence.some((item) => effectiveProject(item) === request.projectId) || workspace.records.some((record) => record.projectIds.includes(request.projectId))) throw new Error("Archive this project instead: it is referenced by evidence or records.");
    workspace.projects = workspace.projects.filter((project) => project.id !== request.projectId);
    workspace.audit.push({ id: id("audit"), action: "project_deleted", subjectId: request.projectId, occurredAt });
  } else if (request.action === "review") {
    const item = workspace.evidence.find((candidate) => candidate.id === request.evidenceId);
    if (!item) throw new Error("Evidence item no longer exists.");
    item.reviewStatus = request.status; item.projectOverride = request.projectId; item.categoryOverride = request.category; item.updatedAt = occurredAt;
    workspace.audit.push({ id: id("audit"), action: "evidence_reviewed", subjectId: item.id, occurredAt });
    refreshDrafts(workspace);
  } else if (request.action === "split") {
    const index = workspace.evidence.findIndex((candidate) => candidate.id === request.evidenceId);
    const item = workspace.evidence[index];
    if (!item || request.splitAt <= item.startedAt || request.splitAt >= item.endedAt) throw new Error("Split point must be inside the evidence block.");
    const second = { ...item, id: id("evidence"), startedAt: request.splitAt, durationSeconds: item.endedAt - request.splitAt, createdAt: occurredAt, updatedAt: occurredAt };
    item.endedAt = request.splitAt; item.durationSeconds = item.endedAt - item.startedAt; item.updatedAt = occurredAt;
    workspace.evidence.splice(index + 1, 0, second);
    workspace.audit.push({ id: id("audit"), action: "evidence_split", subjectId: item.id, occurredAt });
    refreshDrafts(workspace);
  } else if (request.action === "bulk_accept") {
    for (const item of workspace.evidence) if (request.evidenceIds.includes(item.id) && item.projectConfidence === "high" && item.categoryConfidence === "high" && item.suggestedProjectId) item.reviewStatus = "accepted";
    workspace.audit.push({ id: id("audit"), action: "evidence_bulk_accepted", subjectId: request.evidenceIds.join(","), occurredAt });
    refreshDrafts(workspace);
  } else if (request.action === "generate_record") {
    if (request.endAt <= request.startAt) throw new Error("End date must be after start date.");
    if (!workspace.templates.some((template) => template.id === request.templateId)) throw new Error("Select a valid template.");
    const record: BusinessRecord = { id: id("record"), name: request.name.trim() || "Untitled business record", templateId: request.templateId, startAt: request.startAt, endAt: request.endAt, projectIds: request.projectIds, version: 1, status: "draft", totals: { trackedSeconds: 0, reviewedSeconds: 0, unreviewedSeconds: 0, excludedSeconds: 0, unallocatedSeconds: 0, allocations: [] }, createdAt: occurredAt, updatedAt: occurredAt };
    record.totals = calculateTotals(workspace, record); record.status = draftStatus(record.totals); workspace.records.push(record);
    workspace.audit.push({ id: id("audit"), action: "record_generated", subjectId: record.id, occurredAt });
  } else if (request.action === "revise_record") {
    const index = workspace.records.findIndex((record) => record.id === request.recordId);
    const record = workspace.records[index];
    if (!record) throw new Error("Record no longer exists.");
    if (!workspace.certifications.some((certification) => certification.recordId === record.id && certification.recordVersion === record.version)) throw new Error("Only a certified record needs a new draft version.");
    const next: BusinessRecord = { ...record, version: record.version + 1, status: "draft", updatedAt: occurredAt };
    next.totals = calculateTotals(workspace, next); next.status = draftStatus(next.totals);
    workspace.records[index] = next;
    workspace.audit.push({ id: id("audit"), action: "record_revised", subjectId: record.id, occurredAt });
  } else if (request.action === "certify") {
    const record = workspace.records.find((candidate) => candidate.id === request.recordId);
    if (!record) throw new Error("Record no longer exists.");
    if (!request.statementAccepted || !request.certifiedBy.trim()) throw new Error("Enter the attestor name and accept the certification statement.");
    if (record.version !== request.expectedVersion || record.updatedAt !== request.expectedUpdatedAt) throw new Error("This draft changed. Review the latest version before certifying.");
    if (record.status !== "ready") throw new Error("Only the current ready draft can be certified.");
    if (workspace.certifications.some((candidate) => candidate.recordId === record.id && candidate.recordVersion === record.version)) throw new Error("This record version is already certified.");
    const totals = calculateTotals(workspace, record);
    if (totals.reviewedSeconds <= 0 || totals.unreviewedSeconds > 0 || totals.unallocatedSeconds > 0) throw new Error("Review and allocate every included evidence block before certification.");
    record.totals = totals; record.status = "certified"; record.updatedAt = occurredAt;
    const snapshot = certificationSnapshot(workspace, record, request.certifiedBy.trim(), occurredAt);
    const certification: Certification = { id: id("certification"), recordId: record.id, recordVersion: record.version, certifiedAt: occurredAt, certificationStatement: CERTIFICATION_STATEMENT, snapshot, sha256: sha256(stableStringify(snapshot)) };
    workspace.certifications.push(certification);
    workspace.audit.push({ id: id("audit"), action: "record_certified", subjectId: certification.id, occurredAt });
  } else if (request.action === "delete_certification") {
    const certification = workspace.certifications.find((candidate) => candidate.id === request.certificationId);
    if (!certification) throw new Error("Certification no longer exists.");
    workspace.certifications = workspace.certifications.filter((candidate) => candidate.id !== request.certificationId);
    const record = workspace.records.find((candidate) => candidate.id === certification.recordId && candidate.version === certification.recordVersion);
    if (record && (record.status === "certified" || record.status === "exported")) {
      record.status = draftStatus(calculateTotals(workspace, record));
      record.updatedAt = Math.max(occurredAt, record.updatedAt + 1);
    }
    workspace.audit.push({ id: id("audit"), action: "certification_deleted", subjectId: request.certificationId, occurredAt });
  }
  return persist(workspace);
}

function csvCell(value: unknown): string {
  const raw = String(value);
  const safe = /^[=+\-@\t\r]/.test(raw) ? `'${raw}` : raw;
  return `"${safe.replace(/"/g, '""')}"`;
}

export async function mockPreviewExport(certificationId: string, format: "csv" | "json"): Promise<ExportArtifact> {
  const certification = load().certifications.find((candidate) => candidate.id === certificationId);
  if (!certification) throw new Error("Certification no longer exists.");
  const snapshot = certification.snapshot;
  if (sha256(stableStringify(snapshot)) !== certification.sha256) throw new Error("The certified snapshot failed its integrity check.");
  let content: string;
  if (format === "json") content = `${JSON.stringify({ ...snapshot, certificationId: certification.id, integrityHash: certification.sha256 }, null, 2)}\n`;
  else {
    const header = ["certificationId", "recordId", "recordVersion", "recordName", "certifiedBy", "periodStart", "periodEnd", "template", "certifiedAt", "certificationStatement", "project", "client", "category", "hours", "percentage", "reviewStatus", "integrityHash"];
    const rows = snapshot.totals.allocations.map((allocation) => [certification.id, snapshot.recordId, snapshot.recordVersion, snapshot.recordName, snapshot.certifiedBy, snapshot.periodStart, snapshot.periodEnd, snapshot.template, snapshot.certifiedAt, snapshot.certificationStatement, allocation.project, allocation.client, allocation.category, allocation.hours, allocation.percentage, allocation.reviewStatus, certification.sha256]);
    content = `${[header, ...rows].map((row) => row.map(csvCell).join(",")).join("\n")}\n`;
  }
  return { certificationId, format, fileName: `${snapshot.recordName.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}-v${snapshot.recordVersion}.${format}`, content, sha256: sha256(content) };
}

export async function mockSaveExport(artifact: ExportArtifact): Promise<string | null> {
  const expected = await mockPreviewExport(artifact.certificationId, artifact.format);
  if (expected.content !== artifact.content || expected.sha256 !== artifact.sha256) throw new Error("Export changed after preview. Preview it again before saving.");
  const workspace = load();
  const certification = workspace.certifications.find((candidate) => candidate.id === artifact.certificationId)!;
  const record = workspace.records.find((candidate) => candidate.id === certification.recordId && candidate.version === certification.recordVersion);
  if (record) record.status = "exported";
  workspace.audit.push({ id: id("audit"), action: "export_saved", subjectId: artifact.certificationId, occurredAt: nowSeconds() });
  persist(workspace);
  return null;
}

export function resetBusinessMock() { localStorage.removeItem(STORAGE_KEY); }
