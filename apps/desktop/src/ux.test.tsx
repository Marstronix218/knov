import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { CommandPalette, PaletteCommand } from "./components/CommandPalette";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { GlobalNotices } from "./components/GlobalNotices";
import { Modal } from "./components/ui";
import { clearLocalUiState } from "./lib/localState";
import { notifyError } from "./lib/notices";
import { WorkflowEditor } from "./pages/WorkflowEditor";
import type { ThreadContext, WorkflowDocument } from "./types";

beforeEach(() => {
  sessionStorage.clear();
});

describe("error boundary", () => {
  const bomb = { explode: true };
  function Bomb() {
    if (bomb.explode) throw new Error("Kaboom in the view");
    return <p>View is fine</p>;
  }

  it("shows a recoverable panel instead of a blank window, and resets", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const onSendFeedback = vi.fn();
    render(<ErrorBoundary onSendFeedback={onSendFeedback}><Bomb /></ErrorBoundary>);

    expect(screen.getByRole("alert")).toHaveTextContent("This view hit an error");
    expect(screen.getByText("Kaboom in the view")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Send feedback" }));
    expect(onSendFeedback).toHaveBeenCalledOnce();

    bomb.explode = false;
    fireEvent.click(screen.getByRole("button", { name: "Reload view" }));
    expect(screen.getByText("View is fine")).toBeInTheDocument();
    expect(screen.queryByText("This view hit an error")).not.toBeInTheDocument();
  });
});

describe("global notices", () => {
  it("dismisses each notice on its own timer, unaffected by later ones", () => {
    vi.useFakeTimers();
    try {
      render(<GlobalNotices />);
      act(() => notifyError("First failure"));
      act(() => { vi.advanceTimersByTime(5000); });
      act(() => notifyError("Second failure"));
      act(() => { vi.advanceTimersByTime(3500); });

      expect(screen.queryByText("First failure")).not.toBeInTheDocument();
      expect(screen.getByText("Second failure")).toBeInTheDocument();
      act(() => { vi.advanceTimersByTime(5000); });
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows app-wide errors and lets the user dismiss them", () => {
    render(<GlobalNotices />);
    act(() => notifyError("Something went wrong: disk unavailable"));

    expect(screen.getByRole("alert")).toHaveTextContent("disk unavailable");
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("command palette", () => {
  it("shows a failing command's error in the menu instead of failing silently", async () => {
    const onClose = vi.fn();
    const commands: PaletteCommand[] = [{
      id: "rescan",
      label: "Rescan workflows",
      group: "Agent",
      run: () => Promise.reject(new Error("Mining service is offline")),
    }];
    render(<CommandPalette commands={commands} onClose={onClose} />);

    fireEvent.keyDown(screen.getByRole("combobox", { name: "Search commands" }), { key: "Enter" });

    expect(await screen.findByRole("alert")).toHaveTextContent("Rescan workflows failed: Mining service is offline");
    expect(onClose).not.toHaveBeenCalled();
  });

  it("closes once a command succeeds", async () => {
    const onClose = vi.fn();
    const run = vi.fn(async () => undefined);
    render(<CommandPalette commands={[{ id: "pause", label: "Pause collection", group: "Privacy", run }]} onClose={onClose} />);

    fireEvent.keyDown(screen.getByRole("combobox", { name: "Search commands" }), { key: "Enter" });

    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
    expect(run).toHaveBeenCalledOnce();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("modal guards", () => {
  it("asks before discarding a dirty form on Escape or backdrop click", () => {
    const onClose = vi.fn();
    render(<Modal title="Edit" dirty onClose={onClose}><input aria-label="Name" /></Modal>);

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("Discard changes?");

    fireEvent.click(screen.getByRole("button", { name: "Keep editing" }));
    expect(screen.queryByText("Discard changes?")).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Close dialog" })).toHaveFocus();

    // Escape on the prompt cancels it and also returns focus into the dialog.
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByRole("button", { name: "Keep editing" })).toHaveFocus();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByText("Discard changes?")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close dialog" })).toHaveFocus();

    fireEvent.mouseDown(document.querySelector(".modal-backdrop")!);
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("closes a clean form on Escape right away", () => {
    const onClose = vi.fn();
    render(<Modal title="Edit" onClose={onClose}><p>Nothing to lose</p></Modal>);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("guards the workflow editor only once a field changes", () => {
    const onClose = vi.fn();
    render(<WorkflowEditor workflow={workflow} onClose={onClose} onSaved={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("Workflow name"), { target: { value: "Invoice matching" } });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByText("Discard changes?")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Workflow name"), { target: { value: workflow.name } });
    fireEvent.click(screen.getByRole("button", { name: "Keep editing" }));
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("closes only the command menu when Escape is pressed in it over a dialog", () => {
    const closeModal = vi.fn();
    function Stack() {
      const [palette, setPalette] = useState(true);
      return (
        <>
          <Modal title="Underneath" onClose={closeModal}><p>Form</p></Modal>
          {palette && <CommandPalette commands={[]} onClose={() => setPalette(false)} />}
        </>
      );
    }
    render(<Stack />);

    fireEvent.keyDown(screen.getByRole("combobox", { name: "Search commands" }), { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Command menu" })).not.toBeInTheDocument();
    expect(closeModal).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Underneath" })).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(closeModal).toHaveBeenCalledOnce();
  });

  it("ignores window Escape in a lower dialog while another layer is on top", () => {
    const closeLower = vi.fn();
    const closeUpper = vi.fn();
    render(
      <>
        <Modal title="Lower" onClose={closeLower}><p>Lower</p></Modal>
        <Modal title="Upper" onClose={closeUpper}><p>Upper</p></Modal>
      </>,
    );
    fireEvent.keyDown(window, { key: "Escape" });
    expect(closeUpper).toHaveBeenCalledOnce();
    expect(closeLower).not.toHaveBeenCalled();
  });
});

describe("Ask page session state", () => {
  const context: ThreadContext = { version: 1, subject: "Quarterly report", signalCount: 3, apps: ["Excel"], modifiedFiles: [], events: [] };

  async function renderAsk() {
    window.location.hash = "#/assistant";
    render(<App />);
    await screen.findByText("Knov");
  }

  it("shows the selected thread as a removable context chip", async () => {
    sessionStorage.setItem("knov.active-thread-context", JSON.stringify(context));
    await renderAsk();

    expect(await screen.findByText("Context: Quarterly report")).toBeInTheDocument();
    expect(screen.queryByLabelText("Suggested questions")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Clear thread context" }));

    expect(screen.queryByText("Context: Quarterly report")).not.toBeInTheDocument();
    expect(screen.queryByText("Review local candidate evidence")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Suggested questions")).toBeInTheDocument();
    expect(sessionStorage.getItem("knov.active-thread-context")).toBeNull();
  });

  it("keeps the conversation across navigation until a new one is started", async () => {
    // Browser preview answers with a sample reply, so no AI is needed here.
    const reply = { content: /running in browser preview mode/ };
    await renderAsk();

    fireEvent.change(screen.getByPlaceholderText(/Ask about your work/), { target: { value: "What did I do today?" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByText(reply.content)).toBeInTheDocument();

    act(() => { window.location.hash = "#/activity"; });
    await waitFor(() => expect(screen.queryByText(reply.content)).not.toBeInTheDocument());
    act(() => { window.location.hash = "#/assistant"; });
    expect(await screen.findByText(reply.content)).toBeInTheDocument();
    expect(screen.getByText("What did I do today?")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /New conversation/ }));
    expect(screen.queryByText(reply.content)).not.toBeInTheDocument();
    expect(sessionStorage.getItem("knov.ask-transcript")).toBeNull();
    expect(screen.getByLabelText("Suggested questions")).toBeInTheDocument();
  });
});

describe("keyboard shortcuts dialog", () => {
  it("opens with ? outside text fields and keeps ⌘K from stacking the menu on it", async () => {
    window.location.hash = "#/activity";
    render(<App />);
    await screen.findByText("Knov");

    fireEvent.keyDown(screen.getByLabelText("Filter activity"), { key: "?" });
    expect(screen.queryByRole("dialog", { name: "Keyboard shortcuts" })).not.toBeInTheDocument();

    fireEvent.keyDown(document.body, { key: "?" });
    expect(screen.getByRole("dialog", { name: "Keyboard shortcuts" })).toHaveTextContent("⌘K");

    fireEvent.keyDown(window, { key: "k", metaKey: true });
    expect(screen.queryByRole("dialog", { name: "Command menu" })).not.toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Keyboard shortcuts" })).not.toBeInTheDocument();
  });

  it("ignores ⌘-digit navigation while a dialog is open", async () => {
    window.location.hash = "#/activity";
    render(<App />);
    await screen.findByText("Knov");

    fireEvent.keyDown(document.body, { key: "?" });
    expect(screen.getByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "1", metaKey: true });
    expect(window.location.hash).toBe("#/activity");
    expect(screen.getByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    fireEvent.keyDown(window, { key: "1", metaKey: true });
    expect(window.location.hash).toBe("#/home");
  });
});

describe("clearLocalUiState", () => {
  it("removes session state and every knov.* preference, leaving other keys", () => {
    sessionStorage.setItem("knov.ask-transcript", "[]");
    sessionStorage.setItem("knov.active-thread-context", "{}");
    localStorage.setItem("knov.setup-complete", "true");
    localStorage.setItem("knov.open-nav-groups", "[]");
    localStorage.setItem("other-app.theme", "dark");

    clearLocalUiState();

    expect(sessionStorage.length).toBe(0);
    expect(localStorage.getItem("knov.setup-complete")).toBeNull();
    expect(localStorage.getItem("knov.open-nav-groups")).toBeNull();
    expect(localStorage.getItem("other-app.theme")).toBe("dark");
  });
});

const workflow: WorkflowDocument = {
  id: "workflow-1", sessionId: "session-1", name: "Invoice reconciliation", description: "Match Friday invoices", businessGoal: "Accurate records", trigger: "Friday", actors: ["Me"], steps: [
    { id: "receive", name: "Receive invoices", description: "Collect incoming invoices", actor: "Me", application: "Mail", inputs: [], outputs: ["Invoices"], dependsOn: [], decision: null, requiresApproval: false, evidence: [], confidence: 0.8 },
  ], applications: ["Mail"], resources: [], inputs: ["Invoices"], outputs: ["Report"], decisions: [], dependencies: [], approvals: [], exceptions: [], bottlenecks: [], frequency: "Weekly", estimatedMinutes: 30, desiredOutcome: "Matched balances", automationOpportunities: [], evidence: [], confidence: 0.8, confirmed: false, updatedAt: 100,
};
