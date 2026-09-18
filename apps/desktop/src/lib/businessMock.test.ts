import { beforeEach, describe, expect, it } from "vitest";
import { CERTIFICATION_STATEMENT, sanitizeOutboundText } from "../businessTypes";
import { mockMutate, mockPreviewExport, mockSaveExport, mockWorkspace, resetBusinessMock, sha256 } from "./businessMock";

describe("synthetic business workspace", () => {
  beforeEach(() => resetBusinessMock());

  it("seeds realistic local-only projects, templates, and evidence", async () => {
    const workspace = await mockWorkspace();
    expect(workspace.synthetic).toBe(true);
    expect(workspace.projects.map((project) => project.name)).toEqual(["Search Ranking V2", "Authentication", "Internal Operations"]);
    expect(workspace.templates).toHaveLength(3);
    expect(workspace.templates.find((template) => template.id === "rnd-allocation")?.experimental).toBe(true);
    expect(workspace.evidence.some((item) => item.reviewStatus === "corrected" && item.projectOverride)).toBe(true);
    expect(workspace.evidence.some((item) => item.projectConfidence === "low")).toBe(true);
    expect(workspace.evidence.flatMap((item) => item.sourceTypes)).toEqual(expect.arrayContaining(["editor", "git", "chrome_history", "foreground_app"]));
  });

  it("splits evidence without changing its total duration", async () => {
    const before = await mockWorkspace();
    const item = before.evidence.find((candidate) => candidate.id === "ev-ranking-terminal")!;
    const after = await mockMutate({ action: "split", evidenceId: item.id, splitAt: item.startedAt + 600 });
    const pieces = after.evidence.filter((candidate) => candidate.id === item.id || (candidate.startedAt === item.startedAt + 600 && candidate.endedAt === item.endedAt));
    expect(pieces).toHaveLength(2);
    expect(pieces.reduce((sum, candidate) => sum + candidate.durationSeconds, 0)).toBe(item.durationSeconds);
  });

  it("certifies an eligible version, preserves its snapshot, and audits only saved exports", async () => {
    const seed = await mockWorkspace();
    const evidence = seed.evidence.find((item) => item.id === "ev-ranking-code")!;
    let workspace = await mockMutate({ action: "generate_record", name: "Ranking research", templateId: "rnd-allocation", startAt: evidence.startedAt, endAt: evidence.endedAt, projectIds: ["project-search-ranking-v2"] });
    const record = workspace.records[workspace.records.length - 1]!;
    expect(record.status).toBe("ready");

    workspace = await mockMutate({ action: "certify", recordId: record.id, certifiedBy: "Ada Lovelace", statementAccepted: true, expectedVersion: record.version, expectedUpdatedAt: record.updatedAt });
    const certification = workspace.certifications[workspace.certifications.length - 1]!;
    expect(certification.snapshot.certificationStatement).toBe(CERTIFICATION_STATEMENT);
    expect(certification.sha256).toHaveLength(64);
    const snapshotBefore = structuredClone(certification.snapshot);

    const json = await mockPreviewExport(certification.id, "json");
    const csv = await mockPreviewExport(certification.id, "csv");
    expect(JSON.parse(json.content).certifiedBy).toBe("Ada Lovelace");
    expect(csv.content).toContain('"certificationStatement"');
    expect(csv.content).toContain('"integrityHash"');
    expect((await mockWorkspace()).audit.filter((entry) => entry.action === "export_saved")).toHaveLength(0);
    await mockSaveExport(json);
    expect((await mockWorkspace()).audit.filter((entry) => entry.action === "export_saved")).toHaveLength(1);

    await mockMutate({ action: "review", evidenceId: evidence.id, status: "corrected", projectId: "project-authentication", category: "Direct Support" });
    const latest = await mockWorkspace();
    expect(latest.certifications[latest.certifications.length - 1]!.snapshot).toEqual(snapshotBefore);
  });

  it("rejects stale or incomplete certification and verifies exact export bytes", async () => {
    const seed = await mockWorkspace();
    const draft = seed.records[0];
    await expect(mockMutate({ action: "certify", recordId: draft.id, certifiedBy: "Ada", statementAccepted: true, expectedVersion: draft.version, expectedUpdatedAt: draft.updatedAt })).rejects.toThrow(/ready draft/);

    const evidence = seed.evidence.find((item) => item.id === "ev-ranking-code")!;
    let workspace = await mockMutate({ action: "generate_record", name: "Exact bytes", templateId: "rnd-allocation", startAt: evidence.startedAt, endAt: evidence.endedAt, projectIds: ["project-search-ranking-v2"] });
    const record = workspace.records[workspace.records.length - 1]!;
    workspace = await mockMutate({ action: "certify", recordId: record.id, certifiedBy: "Ada", statementAccepted: true, expectedVersion: record.version, expectedUpdatedAt: record.updatedAt });
    const artifact = await mockPreviewExport(workspace.certifications[workspace.certifications.length - 1]!.id, "json");
    await expect(mockSaveExport({ ...artifact, content: `${artifact.content} ` })).rejects.toThrow(/changed after preview/);
  });

  it("sanitizes certified disclosure and resets status when certification is deleted", async () => {
    const seed = await mockWorkspace();
    const evidence = seed.evidence.find((item) => item.id === "ev-ranking-code")!;
    let workspace = await mockMutate({ action: "generate_record", name: "https://private.example password=hunter2", templateId: "rnd-allocation", startAt: evidence.startedAt, endAt: evidence.endedAt, projectIds: ["project-search-ranking-v2"] });
    const record = workspace.records[workspace.records.length - 1]!;
    workspace = await mockMutate({ action: "certify", recordId: record.id, certifiedBy: "sk-private", statementAccepted: true, expectedVersion: record.version, expectedUpdatedAt: record.updatedAt });
    const certification = workspace.certifications[workspace.certifications.length - 1]!;
    const artifact = await mockPreviewExport(certification.id, "json");
    expect(artifact.content).not.toMatch(/private\.example|hunter2|sk-private/);
    workspace = await mockMutate({ action: "delete_certification", certificationId: certification.id });
    expect(workspace.records.find((candidate) => candidate.id === record.id)?.status).toBe("ready");
  });

  it("uses standard SHA-256 output", () => {
    expect(sha256("abc")).toBe("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    expect(sanitizeOutboundText("Non-R&D / Unqualified")).toBe("Non-R&D / Unqualified");
    expect(sanitizeOutboundText("open /Users/ada/private.txt")).toBe("open [redacted]");
  });
});
