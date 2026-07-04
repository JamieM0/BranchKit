import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ChangeKind, RepoInfo } from "$lib/types";

const mockIpc = vi.hoisted(() => ({
  openRepo: vi.fn(),
  cloneRepo: vi.fn(),
  closeRepo: vi.fn(),
  onRepoChanged: vi.fn(async () => () => {}),
  onCloneProgress: vi.fn(async () => () => {}),
}));

vi.mock("$lib/ipc", () => mockIpc);

const { applyChangeKind, nameFromPath, repos } = await import("./repo.svelte");

function repoInfo(overrides: Partial<RepoInfo> = {}): RepoInfo {
  return {
    id: "repo-1",
    path: "/Users/jamie/dev/BranchKit",
    name: "BranchKit",
    branch: "main",
    detached: false,
    ...overrides,
  };
}

describe("nameFromPath", () => {
  it("takes the final path component on either separator", () => {
    expect(nameFromPath("/Users/jamie/dev/BranchKit")).toBe("BranchKit");
    expect(nameFromPath("C:\\Users\\jamie\\BranchKit")).toBe("BranchKit");
  });

  it("falls back to the whole string with no separators", () => {
    expect(nameFromPath("BranchKit")).toBe("BranchKit");
  });
});

describe("applyChangeKind", () => {
  const fresh = { status: false, refs: false, graph: false };

  it("workingTree and index only invalidate status", () => {
    expect(applyChangeKind(fresh, { kind: "workingTree" } as ChangeKind)).toEqual({
      status: true,
      refs: false,
      graph: false,
    });
    expect(applyChangeKind(fresh, { kind: "index" } as ChangeKind)).toEqual({
      status: true,
      refs: false,
      graph: false,
    });
  });

  it("refs invalidates refs and graph but not status", () => {
    expect(applyChangeKind(fresh, { kind: "refs" } as ChangeKind)).toEqual({
      status: false,
      refs: true,
      graph: true,
    });
  });

  it("head invalidates everything", () => {
    expect(applyChangeKind(fresh, { kind: "head" } as ChangeKind)).toEqual({
      status: true,
      refs: true,
      graph: true,
    });
  });

  it("remote invalidates only refs", () => {
    expect(applyChangeKind(fresh, { kind: "remote" } as ChangeKind)).toEqual({
      status: false,
      refs: true,
      graph: false,
    });
  });

  it("operationProgress leaves invalidation untouched", () => {
    const kind: ChangeKind = { kind: "operationProgress", phase: "Receiving objects", percent: 50 };
    expect(applyChangeKind(fresh, kind)).toEqual(fresh);
  });

  it("never clears a flag that was already set", () => {
    const allSet = { status: true, refs: true, graph: true };
    expect(applyChangeKind(allSet, { kind: "workingTree" } as ChangeKind)).toEqual(allSet);
  });
});

describe("RepoStore", () => {
  beforeEach(() => {
    repos.tabs = [];
    repos.activeId = null;
    vi.clearAllMocks();
    mockIpc.onRepoChanged.mockImplementation(async () => () => {});
    mockIpc.onCloneProgress.mockImplementation(async () => () => {});
  });

  it("open() pushes a tab and makes it active", async () => {
    mockIpc.openRepo.mockResolvedValue(repoInfo());
    const tab = await repos.open("/Users/jamie/dev/BranchKit");
    expect(repos.tabs).toHaveLength(1);
    expect(repos.activeId).toBe(tab.id);
    expect(tab.name).toBe("BranchKit");
    expect(tab.invalidate).toEqual({ status: true, refs: true, graph: true });
  });

  it("open() on an already-open repo switches to it instead of duplicating", async () => {
    mockIpc.openRepo.mockResolvedValue(repoInfo());
    await repos.open("/Users/jamie/dev/BranchKit");
    repos.activeId = null;
    await repos.open("/Users/jamie/dev/BranchKit");
    expect(repos.tabs).toHaveLength(1);
    expect(repos.activeId).toBe("repo-1");
  });

  it("switchToIndex jumps to the Nth tab (1-indexed)", async () => {
    mockIpc.openRepo
      .mockResolvedValueOnce(repoInfo({ id: "repo-1" }))
      .mockResolvedValueOnce(repoInfo({ id: "repo-2" }));
    await repos.open("/a");
    await repos.open("/b");
    repos.switchToIndex(1);
    expect(repos.activeId).toBe("repo-1");
    repos.switchToIndex(2);
    expect(repos.activeId).toBe("repo-2");
    repos.switchToIndex(9);
    expect(repos.activeId).toBe("repo-2"); // out of range: no-op
  });

  it("reorder moves a tab within the array", async () => {
    mockIpc.openRepo
      .mockResolvedValueOnce(repoInfo({ id: "repo-1" }))
      .mockResolvedValueOnce(repoInfo({ id: "repo-2" }))
      .mockResolvedValueOnce(repoInfo({ id: "repo-3" }));
    await repos.open("/a");
    await repos.open("/b");
    await repos.open("/c");
    repos.reorder(0, 2);
    expect(repos.tabs.map((t) => t.id)).toEqual(["repo-2", "repo-3", "repo-1"]);
  });

  it("close() removes the tab and calls closeRepo for a real repo", async () => {
    mockIpc.openRepo.mockResolvedValue(repoInfo());
    await repos.open("/a");
    await repos.close("repo-1");
    expect(mockIpc.closeRepo).toHaveBeenCalledWith("repo-1");
    expect(repos.tabs).toHaveLength(0);
    expect(repos.activeId).toBeNull();
  });

  it("close() activates a neighboring tab when closing the active one", async () => {
    mockIpc.openRepo
      .mockResolvedValueOnce(repoInfo({ id: "repo-1" }))
      .mockResolvedValueOnce(repoInfo({ id: "repo-2" }));
    await repos.open("/a");
    await repos.open("/b");
    repos.activeId = "repo-1";
    await repos.close("repo-1");
    expect(repos.activeId).toBe("repo-2");
  });

  it("clone() creates a pending tab immediately, then swaps it for the real one", async () => {
    let resolveClone!: (info: RepoInfo) => void;
    mockIpc.cloneRepo.mockReturnValue(
      new Promise<RepoInfo>((resolve) => {
        resolveClone = resolve;
      }),
    );

    const clonePromise = repos.clone("https://example.com/repo.git", "/dest/repo");
    await Promise.resolve();
    await Promise.resolve();

    expect(repos.tabs).toHaveLength(1);
    expect(repos.tabs[0].id.startsWith("pending:")).toBe(true);
    expect(repos.tabs[0].busy).toBe(true);
    expect(repos.activeId).toBe(repos.tabs[0].id);

    resolveClone(repoInfo({ id: "repo-9", path: "/dest/repo", name: "repo" }));
    const tab = await clonePromise;

    expect(tab.id).toBe("repo-9");
    expect(repos.tabs).toHaveLength(1);
    expect(repos.tabs[0].id).toBe("repo-9");
    expect(repos.activeId).toBe("repo-9");
  });

  it("clone() removes the pending tab if the clone fails", async () => {
    mockIpc.cloneRepo.mockRejectedValue(new Error("network error"));
    await expect(repos.clone("https://example.com/repo.git", "/dest/repo")).rejects.toThrow(
      "network error",
    );
    expect(repos.tabs).toHaveLength(0);
    expect(repos.activeId).toBeNull();
  });
});
