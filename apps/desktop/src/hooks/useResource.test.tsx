import { act, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useResource } from "./useResource";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (cause: Error) => void;
  const promise = new Promise<T>((onResolve, onReject) => { resolve = onResolve; reject = onReject; });
  return { promise, resolve, reject };
}

function Resource({ revision, load }: { revision: number; load: (revision: number) => Promise<string> }) {
  const resource = useResource(() => load(revision), [revision]);
  return <div><p>{resource.data ?? "Empty"}</p><p>{resource.error ?? "No error"}</p><span>{resource.loading ? "Loading" : "Ready"}</span></div>;
}

describe("useResource request ordering", () => {
  it("keeps the latest selected revision when an older request resolves last", async () => {
    const first = deferred<string>();
    const second = deferred<string>();
    const load = (revision: number) => revision === 1 ? first.promise : second.promise;
    const view = render(<Resource revision={1} load={load} />);
    view.rerender(<Resource revision={2} load={load} />);
    await act(async () => second.resolve("Revision 2"));
    expect(screen.getByText("Revision 2")).toBeInTheDocument();
    await act(async () => first.resolve("Revision 1"));
    expect(screen.getByText("Revision 2")).toBeInTheDocument();
    expect(screen.queryByText("Revision 1")).not.toBeInTheDocument();
    expect(screen.getByText("Ready")).toBeInTheDocument();
  });

  it("ignores an obsolete failure while the latest request remains pending", async () => {
    const first = deferred<string>();
    const second = deferred<string>();
    const load = (revision: number) => revision === 1 ? first.promise : second.promise;
    const view = render(<Resource revision={1} load={load} />);
    view.rerender(<Resource revision={2} load={load} />);
    await act(async () => first.reject(new Error("Old revision unavailable")));
    expect(screen.getByText("No error")).toBeInTheDocument();
    expect(screen.getByText("Loading")).toBeInTheDocument();
    await act(async () => second.resolve("Revision 2"));
    expect(screen.getByText("Revision 2")).toBeInTheDocument();
    expect(screen.getByText("Ready")).toBeInTheDocument();
  });
});
