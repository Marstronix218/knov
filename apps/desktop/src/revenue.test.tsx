import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import { RevenuePage, revenueMoney } from "./pages/RevenuePage";
import { HomePage } from "./pages/HomePage";
import { SimulationPage } from "./pages/SimulationPage";
import { SourcesPage } from "./pages/SourcesPage";
import { focusOpportunity, revenueApi } from "./lib/revenueApi";
import { api } from "./lib/api";
import { setSimulationMode } from "./state/simulationMode";
import type { RevenueOverview, SimulationBusiness, SimulationCatalog } from "./revenueTypes";
const fixture: RevenueOverview = {
  demo: false,
  clients: [{ id: "client", name: "Acme", demo: true }], projects: [], agreements: [], commitments: [],
  evidence: [{ id: "evidence", projectId: "project", source: "agreement", sourceRef: "sow:1", excerpt: "Two dashboards included", occurredAt: 100, provenance: "observed", kind: "agreement", demo: true }],
  opportunities: [{ id: "opportunity", projectId: "project", clientName: "Acme", projectName: "Dashboards", type: "scope_change", title: "Possible third dashboard", explanation: "A third dashboard may exceed scope.", evidenceIds: ["evidence"], uncertainty: ["Approval may exist elsewhere"], disproves: ["An approved change order"], recommendedAction: "Check scope", status: "needs_clarification", confidence: 0.8, provenance: "ai_inferred", amountCents: null, currency: null, clarificationQuestion: "Was it already approved?", interviewId: null, createdAt: 100, updatedAt: 100, demo: true }],
  drafts: [], outcomes: [], metrics: { open: 1, needsClarification: 1, draftsAwaitingReview: 0, resolved: 0, potentialByCurrency: {}, verifiedRecoveredByCurrency: {}, reviewed: 0, confirmed: 0, dismissed: 0, draftsApproved: 0, actionsRecorded: 0, paymentsConfirmed: 0 },
};
beforeEach(() => {
  vi.restoreAllMocks();
  setSimulationMode(false);
  window.location.hash = "";
  vi.mocked(invoke).mockResolvedValue({ connected: false, provider: "gmail", account: null, scope: null, lastSync: null });
  (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
  vi.spyOn(revenueApi, "overview").mockResolvedValue(structuredClone(fixture));
});
describe("Revenue Intelligence", () => {
  it("requires native persistence and never pretends browser preview synchronized", async () => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
    render(<RevenuePage />);
    expect(screen.getByText("Open Revenue in the desktop app")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Check sources again" })).not.toBeInTheDocument();
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
  it("opens the problem chosen on Home with a way back", async () => {
    focusOpportunity("opportunity");
    render(<RevenuePage />);
    expect(await screen.findByRole("heading", { name: "Possible third dashboard" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Back to Home/ })).toHaveAttribute("href", "#/home");
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
  it("labels the simulation separately and requests isolated storage", async () => {
    setSimulationMode(true);
    render(<RevenuePage />);
    expect(screen.getByText(/Simulation · Fictional clients/)).toBeInTheDocument();
    await waitFor(() => expect(revenueApi.overview).toHaveBeenCalledWith(true));
  });
  it("preserves synchronized source identity while asking for reviewed classification", async () => {
    const data = structuredClone(fixture);
    data.projects.push({ id: "project", clientId: "client", name: "Dashboards", description: "", threadIds: [], demo: false });
    data.evidence.push({ id: "message", projectId: "project", source: "gmail", sourceRef: "thread:abc/message:123", excerpt: "Please add a third dashboard", occurredAt: 100, provenance: "observed", kind: "communication", demo: false });
    vi.mocked(revenueApi.overview).mockResolvedValue(data);
    render(<SourcesPage />);
    expect(await screen.findByText("Please add a third dashboard")).toBeInTheDocument();
    expect(screen.getByLabelText("Client project")).toHaveValue("project");
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
    act(() => setSimulationMode(true));
    expect(screen.queryByRole("button", { name: /Actual confidential candidate/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check sources again" })).toBeDisabled();
    await waitFor(() => expect(revenueApi.overview).toHaveBeenCalledWith(true));
    rejectDemo(new Error("Demo load failed"));
    await screen.findByText(/Demo load failed/);
    expect(screen.queryByRole("button", { name: /Actual confidential candidate/ })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Show my actual data" }));
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
    render(<SourcesPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Disconnect & revoke" }));
    await screen.findByText("Locally disconnected; remote revocation failed");
    await screen.findByText("Disconnected · No live source is synchronized");
    expect(screen.queryByRole("button", { name: "Disconnect & revoke" })).not.toBeInTheDocument();
  });

  it("passes an optional desktop OAuth secret without persisting it in browser storage", async () => {
    render(<SourcesPage />);
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

describe("Home", () => {
  it("states what is wrong, why, and the recommended action for each open problem", async () => {
    const data = structuredClone(fixture);
    data.evidence.push({ id: "request", projectId: "project", source: "demo_slack", sourceRef: "acme-request", excerpt: "Please build a third dashboard", occurredAt: 200, provenance: "observed", kind: "request", demo: false, meta: { channel: "#acme", author: "Dana Lee (Acme)" } });
    data.opportunities[0].evidenceIds = ["evidence", "request"];
    vi.mocked(revenueApi.overview).mockResolvedValue(data);
    const prepare = vi.spyOn(revenueApi, "prepare").mockResolvedValue(data);
    render(<HomePage />);
    expect(await screen.findByText("Unbilled extra work")).toBeInTheDocument();
    expect(screen.getByText(/Acme asked for work that isn't in the contract/)).toBeInTheDocument();
    expect(screen.getByText("Please build a third dashboard")).toBeInTheDocument();
    expect(screen.getByText(/Slack · Dana Lee \(Acme\) · #acme/)).toBeInTheDocument();
    expect(screen.getByText(/Send a change order/)).toBeInTheDocument();
    expect(screen.getByText("Amount unknown")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Prepare change order/ }));
    await waitFor(() => expect(prepare).toHaveBeenCalledWith("opportunity"));
    await waitFor(() => expect(window.location.hash).toBe("#/revenue"));
  });

  it("lists what is left to set up, linking each step to where it happens", async () => {
    localStorage.removeItem("knov.setup-checklist-hidden");
    const data = structuredClone(fixture);
    data.agreements.push({ id: "sow", projectId: "project", title: "SOW" });
    vi.mocked(revenueApi.overview).mockResolvedValue(data);
    vi.spyOn(api, "discoverySessions").mockResolvedValue([]);
    render(<HomePage />);
    const checklist = await screen.findByRole("region", { name: "Finish setting up" });
    expect(within(checklist).getByText("1 of 4 done. Each step helps Knov find more.")).toBeInTheDocument();
    expect(within(checklist).getByRole("link", { name: /Teach Knov how you bill/ })).toHaveAttribute("href", "#/discovery");
    fireEvent.click(within(checklist).getByRole("button", { name: "Hide" }));
    expect(screen.queryByRole("region", { name: "Finish setting up" })).not.toBeInTheDocument();
    expect(localStorage.getItem("knov.setup-checklist-hidden")).toBe("true");
  });

  it("offers ways to start, including the simulation, when nothing has been added", async () => {
    vi.mocked(revenueApi.overview).mockResolvedValue({ ...structuredClone(fixture), evidence: [], opportunities: [], projects: [] });
    render(<HomePage />);
    fireEvent.click(await screen.findByRole("button", { name: /Try a simulated business/ }));
    expect(localStorage.getItem("knov.simulation-mode")).toBe("true");
    expect(window.location.hash).toBe("#/simulation");
  });
});

describe("Simulation", () => {
  const brightline: SimulationBusiness = { id: "brightline", name: "Brightline Studio", industry: "Web agency", location: "Portland, OR", team: "6 people", currency: "USD", owner: { name: "Jordan Park", email: "you@brightline.studio" }, tools: ["Gmail", "QuickBooks"], profile: "A six-person web agency.", whatToExpect: ["Acme asked for a third dashboard."], expected: [], clientCount: 4, interviewTopic: "change_requests", interviewName: "Change requests and extra work" };
  const lanternwood: SimulationBusiness = { ...brightline, id: "lanternwood", name: "Lanternwood Creative", industry: "Video studio", location: "Austin, TX", team: "3 people", clientCount: 3 };
  const catalog: SimulationCatalog = { active: "brightline", businesses: [brightline, lanternwood], interview: null };

  it("connects one simulated source and explains how the findings changed", async () => {
    const before: RevenueOverview = { ...structuredClone(fixture), demo: true, business: brightline, opportunities: [], evidence: [], projects: [{ id: "project", clientId: "client", name: "Dashboards", description: "", threadIds: [], demo: true }] };
    const after: RevenueOverview = { ...structuredClone(fixture), demo: true, business: brightline, projects: before.projects, evidence: [{ id: "slack", projectId: "project", source: "demo_slack", sourceRef: "acme-request", excerpt: "Please build a third dashboard", occurredAt: 200, provenance: "observed", kind: "request", demo: true, meta: { channel: "#acme", author: "Dana" } }] };
    vi.mocked(revenueApi.overview).mockResolvedValue(before);
    vi.spyOn(revenueApi, "simulationCatalog").mockResolvedValue(catalog);
    const load = vi.spyOn(revenueApi, "simulationLoad").mockResolvedValue(after);
    render(<SimulationPage />);
    expect(await screen.findByRole("heading", { name: "Simulation: Brightline Studio" })).toBeInTheDocument();
    // Contracts are loaded, so the first "Connect" is Gmail.
    fireEvent.click((await screen.findAllByRole("button", { name: "Connect" }))[0]);
    await waitFor(() => expect(load).toHaveBeenCalledWith("gmail"));
    expect(await screen.findByText("New problem: Unbilled extra work at Acme")).toBeInTheDocument();
    expect(localStorage.getItem("knov.simulation-mode")).toBe("true");
  });

  it("lists the simulated businesses and switches only after confirming when data is loaded", async () => {
    const loaded: RevenueOverview = { ...structuredClone(fixture), demo: true, business: brightline, projects: [{ id: "project", clientId: "client", name: "Dashboards", description: "", threadIds: [], demo: true }] };
    vi.mocked(revenueApi.overview).mockResolvedValue(loaded);
    const catalogCall = vi.spyOn(revenueApi, "simulationCatalog").mockResolvedValue(catalog);
    const select = vi.spyOn(revenueApi, "simulationSelect").mockResolvedValue({ ...structuredClone(fixture), demo: true, business: lanternwood, projects: [], evidence: [], opportunities: [] });
    render(<SimulationPage />);
    const businesses = await screen.findByRole("region", { name: "Simulated businesses" });
    expect(within(businesses).getByText("Lanternwood Creative")).toBeInTheDocument();
    expect(within(businesses).getByText("Loaded")).toBeInTheDocument();
    fireEvent.click(within(businesses).getByRole("button", { name: "Choose" }));
    expect(select).not.toHaveBeenCalled();
    expect(within(businesses).getByText(/Switching clears Brightline Studio/)).toBeInTheDocument();
    fireEvent.click(within(businesses).getByRole("button", { name: "Switch" }));
    await waitFor(() => expect(select).toHaveBeenCalledWith("lanternwood"));
    await waitFor(() => expect(catalogCall).toHaveBeenCalledTimes(2));
  });

  it("starts the scripted interview for the loaded business and opens it in simulation mode", async () => {
    vi.mocked(revenueApi.overview).mockResolvedValue({ ...structuredClone(fixture), demo: true, business: brightline, projects: [{ id: "project", clientId: "client", name: "Dashboards", description: "", threadIds: [], demo: true }] });
    vi.spyOn(revenueApi, "simulationCatalog").mockResolvedValue(catalog);
    const interview = vi.spyOn(revenueApi, "simulationInterview").mockResolvedValue({ id: "scripted", status: "active", threadContext: null, messages: [], workflow: { id: "w", sessionId: "scripted", name: "Change requests", description: "", businessGoal: "", trigger: "", actors: [], steps: [], applications: [], resources: [], inputs: [], outputs: [], decisions: [], dependencies: [], approvals: [], exceptions: [], bottlenecks: [], frequency: "", estimatedMinutes: null, desiredOutcome: "", automationOpportunities: [], evidence: [], confidence: 0, confirmed: false, updatedAt: 1, commercialContext: { demo: true, simulation: "brightline", turn: 0 } }, missingInformation: [], createdAt: 1, updatedAt: 1, revision: 0 });
    render(<SimulationPage />);
    const steps = await screen.findByRole("region", { name: "Simulated sources" });
    expect(within(steps).getByText(/Knov interviews Jordan Park/)).toBeInTheDocument();
    fireEvent.click(within(steps).getByRole("button", { name: "Start" }));
    await waitFor(() => expect(interview).toHaveBeenCalled());
    expect(window.location.hash).toBe("#/discovery");
    expect(localStorage.getItem("knov.simulation-mode")).toBe("true");
  });
});

describe("Sources", () => {
  function sourcesWithProject() {
    const data = structuredClone(fixture);
    data.projects.push({ id: "project", clientId: "client", name: "Dashboards", description: "", threadIds: [], demo: false });
    vi.mocked(revenueApi.overview).mockResolvedValue(data);
    return data;
  }

  it("extracts a picked document, lets the text be reviewed, then imports only the reviewed text", async () => {
    const data = sourcesWithProject();
    const extract = vi.spyOn(revenueApi, "extractUpload").mockResolvedValue({ title: "sow.txt", text: "Scope: Build two dashboards." });
    const importAgreement = vi.spyOn(revenueApi, "importAgreement").mockResolvedValue(data);
    const analyze = vi.spyOn(revenueApi, "analyze").mockResolvedValue(data);
    render(<SourcesPage />);
    await screen.findByText(/Drop files here for Acme · Dashboards/);
    const file = new File(["Scope: Build two dashboards."], "sow.txt", { type: "text/plain" });
    fireEvent.change(screen.getByLabelText("Upload documents"), { target: { files: [file] } });
    await waitFor(() => expect(extract).toHaveBeenCalledWith("sow.txt", btoa("Scope: Build two dashboards.")));

    const review = await screen.findByRole("region", { name: "Review extracted text" });
    const text = within(review).getByLabelText("Extracted text");
    expect(text).toHaveValue("Scope: Build two dashboards.");
    expect(within(review).getByText("28 characters")).toBeInTheDocument();
    expect(importAgreement).not.toHaveBeenCalled();

    fireEvent.change(text, { target: { value: "Scope: Build three dashboards." } });
    expect(within(review).getByText("30 characters")).toBeInTheDocument();
    fireEvent.click(within(review).getByRole("button", { name: "Import this text" }));
    await waitFor(() => expect(importAgreement).toHaveBeenCalledWith("project", "sow.txt", "Scope: Build three dashboards."));
    await waitFor(() => expect(analyze).toHaveBeenCalledWith(false));
    expect(await screen.findByText(/Imported 1 document for Acme · Dashboards/)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Review extracted text" })).not.toBeInTheDocument();
  });

  it("discards the extracted text on Cancel without importing", async () => {
    sourcesWithProject();
    vi.spyOn(revenueApi, "extractUpload").mockResolvedValue({ title: "sow.txt", text: "Scope" });
    const importAgreement = vi.spyOn(revenueApi, "importAgreement");
    const analyze = vi.spyOn(revenueApi, "analyze");
    render(<SourcesPage />);
    await screen.findByText(/Drop files here for Acme · Dashboards/);
    fireEvent.change(screen.getByLabelText("Upload documents"), { target: { files: [new File(["Scope"], "sow.txt")] } });
    const review = await screen.findByRole("region", { name: "Review extracted text" });
    fireEvent.click(within(review).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("region", { name: "Review extracted text" })).not.toBeInTheDocument();
    expect(importAgreement).not.toHaveBeenCalled();
    expect(analyze).not.toHaveBeenCalled();
  });

  it("clears the reading state and reports an extraction failure", async () => {
    sourcesWithProject();
    vi.spyOn(revenueApi, "extractUpload").mockRejectedValue(new Error("No text extracted; scanned PDFs require OCR, which is unsupported"));
    render(<SourcesPage />);
    await screen.findByText(/Drop files here for Acme · Dashboards/);
    fireEvent.change(screen.getByLabelText("Upload documents"), { target: { files: [new File(["%PDF"], "scan.pdf")] } });
    expect(await screen.findByText("scan.pdf: No text extracted; scanned PDFs require OCR, which is unsupported")).toBeInTheDocument();
    expect(screen.queryByText(/Reading scan.pdf/)).not.toBeInTheDocument();
    expect(screen.getByText(/Drop files here for Acme · Dashboards/)).toBeInTheDocument();
  });

  it("shows a loading state until the overview arrives", async () => {
    vi.mocked(revenueApi.overview).mockReturnValue(new Promise(() => undefined));
    render(<SourcesPage />);
    expect(screen.getByRole("status")).toHaveTextContent("Loading…");
  });
});
