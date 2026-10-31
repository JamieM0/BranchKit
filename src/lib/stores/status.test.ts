import { describe, expect, it, vi } from "vitest";
import { StatusStore } from "./status.svelte";
import type { ChangeKind, StatusReport } from "$lib/types";

function report(head: string): StatusReport {
  return { branch: { head, oid: null, upstream: null, ahead: 0, behind: 0 }, entries: [] };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

describe("StatusStore", () => {
  it("ignores an old repo's status after switching tabs", async () => {
    const old = deferred<StatusReport>();
    const store = new StatusStore({
      getStatus: vi.fn().mockReturnValueOnce(old.promise).mockResolvedValue(report("second")),
      onRepoChanged: async () => () => {},
    });
    const firstOpen = store.open("first");
    await Promise.resolve();
    await store.open("second");
    old.resolve(report("first"));
    await firstOpen;
    expect(store.repoId).toBe("second");
    expect(store.report.branch.head).toBe("second");
  });

  it("keeps the latest refresh when reads complete out of order", async () => {
    const old = deferred<StatusReport>();
    const getStatus = vi.fn().mockResolvedValueOnce(report("main"))
      .mockReturnValueOnce(old.promise).mockResolvedValueOnce(report("new"));
    const store = new StatusStore({ getStatus, onRepoChanged: async () => () => {} });
    await store.open("repo");
    const pending = store.refresh();
    await store.refresh();
    old.resolve(report("old"));
    await pending;
    expect(store.report.branch.head).toBe("new");
  });

  it("refreshes ahead/behind after fetch and exposes read failures", async () => {
    let handler!: (kind: ChangeKind) => void;
    const getStatus = vi.fn().mockResolvedValue(report("main"));
    const store = new StatusStore({ getStatus, onRepoChanged: async (_, fn) => {
      handler = fn; return () => {};
    } });
    await store.open("repo");
    handler({ kind: "remote" });
    await Promise.resolve();
    expect(getStatus).toHaveBeenCalledTimes(2);
    getStatus.mockRejectedValueOnce(new Error("missing repo"));
    await store.refresh();
    expect(store.error).toBeInstanceOf(Error);
    await store.refresh();
    expect(store.error).toBeNull();
  });
});
