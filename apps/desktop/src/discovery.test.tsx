import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DiscoveryPage } from "./pages/DiscoveryPage";
import { KnowledgePage } from "./pages/KnowledgePage";
import { WorkflowEditor } from "./pages/WorkflowEditor";
import { api } from "./lib/api";
import { mockDashboard, mockSettings } from "./lib/mockData";
import type { InterviewGraph, InterviewSession, ThreadContext, WorkflowDocument } from "./types";

const workflow: WorkflowDocument = {
  id: "workflow-1", sessionId: "session-1", name: "Invoice reconciliation", description: "Match Friday invoices", businessGoal: "Accurate records", trigger: "Friday", actors: ["Me"], steps: [
    { id: "receive", name: "Receive invoices", description: "Collect incoming invoices", actor: "Me", application: "Mail", inputs: [], outputs: ["Invoices"], dependsOn: [], decision: null, requiresApproval: false, evidence: ["I receive invoices every Friday."], confidence: 0.8 },
    { id: "compare", name: "Compare records", description: "Check accounting totals", actor: "Me", application: "Accounting", inputs: ["Invoices"], outputs: ["Reconciliation"], dependsOn: ["receive"], decision: "Do totals match?", requiresApproval: true, evidence: [], confidence: 0.7 },
  ], applications: ["Mail", "Accounting"], resources: [], inputs: ["Invoices"], outputs: ["Report"], decisions: [], dependencies: [], approvals: [], exceptions: [], bottlenecks: ["Manual comparison"], frequency: "Weekly", estimatedMinutes: 30, desiredOutcome: "Matched balances", automationOpportunities: ["Prepare a comparison draft"], evidence: [{ source: "user_reported", detail: "I reconcile invoices on Friday" }], confidence: 0.8, confirmed: false, updatedAt: 100,
};
const context: ThreadContext = { version: 1, subject: "Finance", signalCount: 2, apps: ["Mail", "Accounting"], modifiedFiles: [], events: [] };
const session: InterviewSession = { id: "session-1", status: "active", threadContext: context, messages: [{ role: "assistant", content: "Which records do you compare?", createdAt: 100 }], workflow, missingInformation: ["Exceptions"], createdAt: 100, updatedAt: 100, revision: 1 };
const graph: InterviewGraph = { sessionId: "session-1", revision: 1, nodes: [{ id: "node-1", kind: "Workflow", label: workflow.name, description: "Invoice process", evidence: [{ sourceType: "interview", sourceRef: "session-1", timestamp: "100", confidence: 0.8, status: "user_reported", userConfirmed: false, detail: "User described invoices" }] }], edges: [] };

beforeEach(() => {
  vi.restoreAllMocks();
  (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
  vi.spyOn(api, "dashboard").mockResolvedValue(mockDashboard);
  vi.spyOn(api, "settings").mockResolvedValue({ ...mockSettings, hasProviderKey: true });
  vi.spyOn(api, "discoverySessions").mockResolvedValue([]);
  vi.spyOn(api, "discoveredWorkflows").mockResolvedValue([structuredClone(workflow)]);
  vi.spyOn(api, "interviewGraph").mockResolvedValue(graph);
  vi.spyOn(api, "interviewGraphHistory").mockResolvedValue([{ revision: 1, timestamp: "100", nodes: 1, edges: 0 }]);
});

function renderDiscovery() { return render(<DiscoveryPage threadContexts={() => [context]} />); }

describe("Workflow Discovery", () => {
  it("starts from an existing thread, answers adaptively, and persists pause and resume", async () => {
    const start = vi.spyOn(api, "startDiscovery").mockResolvedValue(structuredClone(session));
    const advance = vi.spyOn(api, "advanceDiscovery").mockResolvedValue({ ...session, messages: [...session.messages, { role: "user", content: "Our ledger", createdAt: 101 }, { role: "assistant", content: "What happens when amounts differ?", createdAt: 101 }] });
    const status = vi.spyOn(api, "setDiscoveryStatus").mockImplementation(async (_, value) => ({ ...session, status: value }));
    renderDiscovery();
    await screen.findByRole("option", { name: "Finance · 2 signals" });
    fireEvent.change(screen.getByLabelText("Optional work thread"), { target: { value: "0" } });
    fireEvent.click(screen.getByRole("button", { name: "Start interview" }));
    await screen.findByText("Which records do you compare?");
    expect(start).toHaveBeenCalledWith("", context);
    fireEvent.change(screen.getByLabelText("Your answer"), { target: { value: "Our ledger" } });
    fireEvent.click(screen.getByRole("button", { name: "Send answer" }));
    await screen.findByText("What happens when amounts differ?");
    expect(advance).toHaveBeenCalledWith("session-1", "answer", "Our ledger");
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    await screen.findByRole("button", { name: "Resume interview" });
    expect(status).toHaveBeenCalledWith("session-1", "paused");
    fireEvent.click(screen.getByRole("button", { name: "Resume interview" }));
    await screen.findByLabelText("Your answer");
    expect(status).toHaveBeenCalledWith("session-1", "active");
  });

  it("loads saved sessions and permits local finishing without a provider key", async () => {
    vi.spyOn(api, "settings").mockResolvedValue({ ...mockSettings, hasProviderKey: false });
    vi.spyOn(api, "discoverySessions").mockResolvedValue([session]);
    const advance = vi.spyOn(api, "advanceDiscovery").mockResolvedValue({ ...session, status: "completed" });
    renderDiscovery();
    fireEvent.click(await screen.findByRole("button", { name: /Invoice reconciliation.*active/ }));
    expect(screen.getByRole("button", { name: "Send answer" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Finish & review" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Finish & review" }));
    await screen.findByRole("button", { name: "Review workflow" });
    expect(advance).toHaveBeenCalledWith("session-1", "end", undefined);
  });

  it("keeps an unsent answer after a provider failure and allows retry", async () => {
    vi.spyOn(api, "discoverySessions").mockResolvedValue([session]);
    vi.spyOn(api, "advanceDiscovery").mockRejectedValue(new Error("Provider unavailable"));
    renderDiscovery();
    fireEvent.click(await screen.findByRole("button", { name: /Invoice reconciliation.*active/ }));
    fireEvent.change(screen.getByLabelText("Your answer"), { target: { value: "Our ledger" } });
    fireEvent.click(screen.getByRole("button", { name: "Send answer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Provider unavailable");
    expect(screen.getByLabelText("Your answer")).toHaveValue("Our ledger");
    expect(screen.getByRole("button", { name: "Send answer" })).toBeEnabled();
  });

  it("deletes a saved interview only after the explicit delete control", async () => {
    vi.spyOn(api, "discoverySessions").mockResolvedValue([session]);
    const remove = vi.spyOn(api, "deleteDiscovery").mockResolvedValue(undefined);
    renderDiscovery();
    fireEvent.click(await screen.findByRole("button", { name: /Invoice reconciliation.*active/ }));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Delete interview" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith("session-1"));
    expect(await screen.findByRole("button", { name: "Start interview" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Invoice reconciliation.*active/ })).not.toBeInTheDocument();
  });

  it("discloses browser unavailability without offering fake persistence", async () => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
    renderDiscovery();
    expect(screen.getByText("Open Workflow Discovery in the desktop app")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Start interview" })).not.toBeInTheDocument();
    await expect(api.startDiscovery("invoice work")).rejects.toThrow("requires the desktop app");
  });
});

describe("Workflow knowledge and corrections", () => {
  it("inspects graph sources and requests a historical revision", async () => {
    render(<KnowledgePage />);
    await screen.findByRole("button", { name: /Workflow.*Invoice reconciliation/ });
    fireEvent.click(screen.getByRole("button", { name: /Workflow.*Invoice reconciliation/ }));
    expect(screen.getByText("User described invoices")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Graph revision"), { target: { value: "1" } });
    await waitFor(() => expect(api.interviewGraph).toHaveBeenLastCalledWith("session-1", 1));
    expect(screen.getByText(/Inspecting historical evidence/)).toBeInTheDocument();
  });

  it("saves explicit corrections and confirmation without authorizing automation", async () => {
    const save = vi.spyOn(api, "saveDiscoveredWorkflow").mockImplementation(async (value) => value);
    const onSaved = vi.fn();
    render(<WorkflowEditor workflow={workflow} onClose={vi.fn()} onSaved={onSaved} />);
    fireEvent.change(screen.getByLabelText("Business goal"), { target: { value: "Prevent duplicate payment" } });
    fireEvent.click(screen.getByLabelText("I confirm this workflow describes my work"));
    fireEvent.click(screen.getByRole("button", { name: "Save workflow" }));
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(save.mock.calls[0][0]).toMatchObject({ businessGoal: "Prevent duplicate payment", confirmed: true, updatedAt: 100 });
  });

  it("limits dependencies to earlier steps and removes invalid dependencies after reordering", async () => {
    const save = vi.spyOn(api, "saveDiscoveredWorkflow").mockImplementation(async (value) => value);
    render(<WorkflowEditor workflow={workflow} onClose={vi.fn()} onSaved={vi.fn()} />);
    const firstStep = screen.getByRole("heading", { name: "Step 1" }).closest("section")!;
    expect(within(firstStep).queryByLabelText("Compare records")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Move step 2 up" }));
    fireEvent.click(screen.getByRole("button", { name: "Save workflow" }));
    await waitFor(() => expect(save).toHaveBeenCalled());
    expect(save.mock.calls[0][0].steps[0]).toMatchObject({ id: "compare", dependsOn: [] });
  });
});
