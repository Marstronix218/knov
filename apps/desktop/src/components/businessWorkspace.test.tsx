import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BusinessPage } from "./BusinessWorkspace";
import { mockMutate, mockWorkspace, resetBusinessMock } from "../lib/businessMock";
import { resetBusinessWorkspaceCache } from "../lib/businessWorkspaceStore";
import { businessApi } from "../lib/businessApi";

describe("BusinessPage", () => {
  beforeEach(() => {
    resetBusinessMock();
    resetBusinessWorkspaceCache();
  });

  it("bulk accepts only high-confidence evidence and explains local review", async () => {
    render(<BusinessPage page="evidence" />);
    expect(await screen.findByText(/Nothing is shared externally/)).toBeInTheDocument();
    const button = screen.getByRole("button", { name: /Accept 1 high-confidence/i });
    fireEvent.click(button);
    await waitFor(() => expect(screen.getByRole("button", { name: /Accept 0 high-confidence/i })).toBeDisabled());
    const workspace = await mockWorkspace();
    expect(workspace.evidence.find((item) => item.id === "ev-ranking-terminal")?.reviewStatus).toBe("accepted");
    expect(workspace.evidence.find((item) => item.id === "ev-ranking-github")?.reviewStatus).toBe("pending");
  });

  it("creates a project with all signal fields", async () => {
    render(<BusinessPage page="projects" />);
    await screen.findByText("Search Ranking V2");
    fireEvent.click(screen.getByRole("button", { name: /New project/i }));
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Client Portal" } });
    fireEvent.change(screen.getByLabelText("Client"), { target: { value: "Acme" } });
    fireEvent.change(screen.getByLabelText("Aliases"), { target: { value: "portal, customer app" } });
    fireEvent.change(screen.getByLabelText("Domains"), { target: { value: "acme.example" } });
    fireEvent.click(screen.getByRole("button", { name: "Save project" }));
    expect(await screen.findByText("Client Portal")).toBeInTheDocument();
    const projects = (await mockWorkspace()).projects;
    expect(projects[projects.length - 1]).toMatchObject({ client: "Acme", aliases: ["portal", "customer app"], domains: ["acme.example"] });
  });

  it("shows the full certification snapshot only after explicit attestation", async () => {
    const seed = await mockWorkspace();
    const evidence = seed.evidence.find((item) => item.id === "ev-ranking-code")!;
    let workspace = await mockMutate({ action: "generate_record", name: "Ready record", templateId: "rnd-allocation", startAt: evidence.startedAt, endAt: evidence.endedAt, projectIds: ["project-search-ranking-v2"] });
    expect(workspace.records[workspace.records.length - 1]?.status).toBe("ready");
    render(<BusinessPage page="review" />);
    await screen.findByText("Ready record");
    fireEvent.click(screen.getByRole("button", { name: /Ready record/i }));
    expect(screen.queryByText("Proposed certification fields")).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Attestor name"), { target: { value: "Ada Lovelace" } });
    fireEvent.click(screen.getByText(/I reviewed this record/));
    expect(screen.getByRole("button", { name: /Certify this version/i })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: /Preview certification/i }));
    expect(await screen.findByText("Proposed certification fields")).toBeInTheDocument();
    expect(screen.getByText(/"certifiedBy": "Ada Lovelace"/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Certify this version/i }));
    expect(await screen.findByText(/This version is certified and immutable/)).toBeInTheDocument();
  });

  it("requires destructive confirmation before deleting a certification", async () => {
    const seed = await mockWorkspace();
    const evidence = seed.evidence.find((item) => item.id === "ev-ranking-code")!;
    let workspace = await mockMutate({ action: "generate_record", name: "Disposable certification", templateId: "rnd-allocation", startAt: evidence.startedAt, endAt: evidence.endedAt, projectIds: ["project-search-ranking-v2"] });
    const record = workspace.records[workspace.records.length - 1]!;
    workspace = await mockMutate({ action: "certify", recordId: record.id, certifiedBy: "Ada", statementAccepted: true, expectedVersion: record.version, expectedUpdatedAt: record.updatedAt });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<BusinessPage page="exports" />);
    const card = (await screen.findByText("Disposable certification")).closest("article")!;
    fireEvent.click(within(card).getByRole("button", { name: /Delete certification/i }));
    expect(confirm).toHaveBeenCalled();
    expect((await mockWorkspace()).certifications).toHaveLength(1);
  });
  it("serves cached workspace across tab switches instead of refetching", async () => {
    const fetchWorkspace = vi.spyOn(businessApi, "workspace");
    const first = render(<BusinessPage page="evidence" />);
    await screen.findByText(/Nothing is shared externally/);
    expect(fetchWorkspace).toHaveBeenCalledTimes(1);

    first.unmount();
    render(<BusinessPage page="projects" />);
    // Cached data renders immediately: no loading state, no second backend call.
    expect(screen.getByText("Search Ranking V2")).toBeInTheDocument();
    expect(screen.queryByText(/Loading local workspace/)).not.toBeInTheDocument();
    expect(fetchWorkspace).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /Refresh/i }));
    await waitFor(() => expect(fetchWorkspace).toHaveBeenCalledTimes(2));
    fetchWorkspace.mockRestore();
  });
});
