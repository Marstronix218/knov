import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import { RevenuePage, revenueMoney } from "./pages/RevenuePage";
import { revenueApi } from "./lib/revenueApi";
import type { RevenueOverview } from "./revenueTypes";
const fixture: RevenueOverview = {
  demo: false,
  clients: [{ id: "client", name: "Acme", demo: true }], projects: [], agreements: [], commitments: [],
  evidence: [{ id: "evidence", projectId: "project", source: "agreement", sourceRef: "sow:1", excerpt: "Two dashboards included", occurredAt: 100, provenance: "observed", kind: "agreement", demo: true }],
  opportunities: [{ id: "opportunity", projectId: "project", clientName: "Acme", projectName: "Dashboards", type: "scope_change", title: "Possible third dashboard", explanation: "A third dashboard may exceed scope.", evidenceIds: ["evidence"], uncertainty: ["Approval may exist elsewhere"], disproves: ["An approved change order"], recommendedAction: "Check scope", status: "needs_clarification", confidence: 0.8, provenance: "ai_inferred", amountCents: null, currency: null, clarificationQuestion: "Was it already approved?", interviewId: null, createdAt: 100, updatedAt: 100, demo: true }],
  drafts: [], outcomes: [], metrics: { open: 1, needsClarification: 1, draftsAwaitingReview: 0, resolved: 0, potentialByCurrency: {}, verifiedRecoveredByCurrency: {}, reviewed: 0, confirmed: 0, dismissed: 0, draftsApproved: 0, actionsRecorded: 0, paymentsConfirmed: 0 },
};
beforeEach(() => {
  vi.restoreAllMocks();
  vi.mocked(invoke).mockResolvedValue({ connected: false, provider: "gmail", account: null, scope: null, lastSync: null });
  (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
  vi.spyOn(revenueApi, "overview").mockResolvedValue(structuredClone(fixture));
});
describe("Revenue Intelligence", () => {
  it("requires native persistence and never pretends browser preview synchronized", async () => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
    render(<RevenuePage />);
    expect(screen.getByText("Open Revenue in the desktop app")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Analyze local evidence" })).not.toBeInTheDocument();
  });
  it("shows exact source evidence, unknown values, disproof and missing stages", async () => {
    render(<RevenuePage />);
    fireEvent.click(await screen.findByRole("button", { name: /Possible third dashboard/ }));
    expect(screen.getByText("Two dashboards included")).toBeInTheDocument();
    expect(screen.getByText("Approval may exist elsewhere")).toBeInTheDocument();
    expect(screen.getByText("An approved change order")).toBeInTheDocument();
    expect(screen.getByText("No verified payments")).toBeInTheDocument();
    expect(screen.getByText("Unverified · no linked planning record")).toBeInTheDocument();
    expect(screen.getAllByText("Unknown").length).toBeGreaterThan(0);
  });
  it("saves contextual clarification through revenue discovery integration", async () => {
    const review = vi.spyOn(revenueApi, "review").mockResolvedValue(fixture);
    render(<RevenuePage />);
    fireEvent.click(await screen.findByRole("button", { name: /Possible third dashboard/ }));
    fireEvent.click(screen.getByRole("button", { name: "Ask Knov" }));
    fireEvent.change(screen.getByLabelText("Scope decision"), { target: { value: "separately_billable" } });
    fireEvent.click(screen.getByRole("button", { name: "Save clarification" }));
    await waitFor(() => expect(review).toHaveBeenCalledWith("opportunity", "clarify", "separately_billable"));
  });
  it("filters the inbox without changing opportunity evidence", async () => {
    render(<RevenuePage />);
    await screen.findByRole("button", { name: /Possible third dashboard/ });
    fireEvent.change(screen.getByLabelText("Detected on or after"), { target: { value: "2026-01-01" } });
    expect(screen.getByText("No opportunities in this view")).toBeInTheDocument();
  });
  it("requires saving draft edits before approval and approval does not record payment", async () => {
    const data = structuredClone(fixture);
    data.drafts.push({ id: "draft", opportunityId: "opportunity", kind: "change_order", body: "Review additional scope", status: "draft", createdAt: 100, updatedAt: 100, demo: true });
    vi.mocked(revenueApi.overview).mockResolvedValue(data);
    const approve = vi.spyOn(revenueApi, "approveDraft").mockResolvedValue({ ...data, drafts: [{ ...data.drafts[0], status: "approved" }] });
    const record = vi.spyOn(revenueApi, "recordOutcome");
    render(<RevenuePage />);
    fireEvent.click(await screen.findByRole("button", { name: /Possible third dashboard/ }));
    fireEvent.change(screen.getByLabelText("Edit draft"), { target: { value: "Edited scope" } });
    expect(screen.getByRole("button", { name: "Approve draft" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Edit draft"), { target: { value: "Review additional scope" } });
    fireEvent.click(screen.getByRole("button", { name: "Approve draft" }));
    await waitFor(() => expect(approve).toHaveBeenCalledWith("draft"));
    expect(record).not.toHaveBeenCalled();
    expect(screen.getByText("No verified payments")).toBeInTheDocument();
  });
  it("records a self-reported outcome with explicit money units without asserting verification", async () => {
    const record = vi.spyOn(revenueApi, "recordOutcome").mockResolvedValue(fixture);
    render(<RevenuePage />);
    fireEvent.click(await screen.findByRole("button", { name: /Possible third dashboard/ }));
    fireEvent.change(screen.getByLabelText("Outcome"), { target: { value: "paid" } });
    fireEvent.change(screen.getByLabelText("Evidence and source reference"), { target: { value: "Receipt 123 received by email" } });
    fireEvent.change(screen.getByLabelText("Amount (optional; currency units)"), { target: { value: "120.50" } });
    fireEvent.click(screen.getByRole("button", { name: "Record outcome" }));
    await waitFor(() => expect(record).toHaveBeenCalledWith("opportunity", "paid", "Receipt 123 received by email", 12050, "USD", false));
  });
  it("labels isolated demo separately and requests isolated storage", async () => {
    render(<RevenuePage />);
    fireEvent.click(screen.getByRole("button", { name: "Open isolated demo" }));
    expect(screen.getByText(/Isolated demo · Fictional clients/)).toBeInTheDocument();
    await waitFor(() => expect(revenueApi.overview).toHaveBeenCalledWith(true));
    expect(screen.getByRole("button", { name: "Load reproducible demo" })).toBeInTheDocument();
  });
  it("preserves synchronized source identity while asking for reviewed classification", async () => {
    const data = structuredClone(fixture);
    data.projects.push({ id: "project", clientId: "client", name: "Dashboards", description: "", threadIds: [], demo: false });
    data.evidence.push({ id: "message", projectId: "project", source: "gmail", sourceRef: "thread:abc/message:123", excerpt: "Please add a third dashboard", occurredAt: 100, provenance: "observed", kind: "communication", demo: false });
    vi.mocked(revenueApi.overview).mockResolvedValue(data);
    render(<RevenuePage />);
    fireEvent.click(await screen.findByRole("button", { name: "Clients, documents & sources" }));
    fireEvent.change(screen.getByLabelText("Selected client project"), { target: { value: "project" } });
    expect(screen.getByText("Please add a third dashboard")).toBeInTheDocument();
    expect(screen.getByText(/thread:abc\/message:123/)).toBeInTheDocument();
    vi.mocked(invoke).mockResolvedValue(data);
    fireEvent.click(screen.getByRole("button", { name: "Save source classification" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("revenue_classify_evidence", { evidenceId: "message", kind: "request", dueAt: null, amountCents: null, currency: null }));
  });
  it("requires payment and an amount before receipt attestation", async () => {
    render(<RevenuePage />);
    fireEvent.click(await screen.findByRole("button", { name: /Possible third dashboard/ }));
    const checkbox = screen.getByRole("checkbox", { name: /I checked an external payment receipt/ });
    expect(checkbox).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Outcome"), { target: { value: "paid" } });
    expect(checkbox).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Amount (optional; currency units)"), { target: { value: "100" } });
    expect(checkbox).toBeEnabled();
    fireEvent.change(screen.getByLabelText("Outcome"), { target: { value: "invoiced" } });
    expect(checkbox).toBeDisabled();
  });
  it("handles supported and unsupported currency labels honestly", () => {
    expect(revenueMoney(null, "USD")).toBe("Unknown");
    expect(revenueMoney(12050, "USD")).toContain("120.50");
    expect(revenueMoney(100, "invalid")).toBe("1.00 invalid");
  });

  it("never shows actual records under the demo label while loading or after failure", async () => {
    const actual = structuredClone(fixture);
    actual.opportunities[0].title = "Actual confidential candidate";
    let rejectDemo: (reason: Error) => void = () => {};
    vi.mocked(revenueApi.overview).mockImplementation((demo) => demo
      ? new Promise<RevenueOverview>((_resolve, reject) => { rejectDemo = reject; })
      : Promise.resolve(actual));
    render(<RevenuePage />);
    await screen.findByRole("button", { name: /Actual confidential candidate/ });
    fireEvent.click(screen.getByRole("button", { name: "Open isolated demo" }));
    expect(screen.queryByRole("button", { name: /Actual confidential candidate/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Analyze local evidence" })).toBeDisabled();
    await waitFor(() => expect(revenueApi.overview).toHaveBeenCalledWith(true));
    rejectDemo(new Error("Demo load failed"));
    await screen.findByText(/Demo load failed/);
    expect(screen.queryByRole("button", { name: /Actual confidential candidate/ })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Return to actual data" }));
    await screen.findByRole("button", { name: /Actual confidential candidate/ });
  });

  it("shows local disconnection even when remote revocation fails", async () => {
    let connected = true;
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "revenue_connector_disconnect") {
        connected = false;
        throw new Error("Locally disconnected; remote revocation failed");
      }
      return { connected, provider: "gmail", account: "test@example.com", scope: null, lastSync: null };
    });
    render(<RevenuePage />);
    fireEvent.click(await screen.findByRole("button", { name: "Clients, documents & sources" }));
    fireEvent.click(await screen.findByRole("button", { name: "Disconnect & revoke" }));
    await screen.findByText("Locally disconnected; remote revocation failed");
    await screen.findByText("Disconnected · No live source is synchronized");
    expect(screen.queryByRole("button", { name: "Disconnect & revoke" })).not.toBeInTheDocument();
  });

  it("passes an optional desktop OAuth secret without persisting it in browser storage", async () => {
    render(<RevenuePage />);
    fireEvent.click(await screen.findByRole("button", { name: "Clients, documents & sources" }));
    fireEvent.change(await screen.findByLabelText("Google Desktop OAuth client ID (preferred)"), { target: { value: "demo.apps.googleusercontent.com" } });
    fireEvent.change(screen.getByLabelText(/Desktop client secret/), { target: { value: "fixture-client-secret" } });
    fireEvent.change(screen.getByLabelText("Gmail query"), { target: { value: "from:client@example.com" } });
    fireEvent.change(screen.getByLabelText("Read from (UTC date)"), { target: { value: "2026-10-01" } });
    fireEvent.change(screen.getByLabelText("Read before (UTC date)"), { target: { value: "2026-10-09" } });
    fireEvent.click(screen.getByLabelText("I authorize this account and only the source scope above"));
    fireEvent.click(screen.getByRole("button", { name: "Authorize selected scope" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("revenue_gmail_oauth", expect.objectContaining({ clientId: "demo.apps.googleusercontent.com", clientSecret: "fixture-client-secret", authorized: true })));
    expect(JSON.stringify(localStorage)).not.toContain("fixture-client-secret");
    expect(JSON.stringify(sessionStorage)).not.toContain("fixture-client-secret");
  });
});
