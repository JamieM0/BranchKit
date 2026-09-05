import { describe, expect, it, vi } from "vitest";
import { GraphStore, type GraphStoreDeps } from "./graph.svelte";
import { assignLanes } from "$lib/graph/lanes";
import type { ChangeKind, GraphTopologyRow, HeadInfo, RefInfo } from "$lib/types";

const topology: GraphTopologyRow[] = [
	{ kind: "commit", sha: "B", parents: ["A"] },
	{ kind: "commit", sha: "A", parents: [] },
];

const head: HeadInfo = { detached: false, branch: "main", sha: "B" };

const mainRef: RefInfo = {
	name: "refs/heads/main",
	shortName: "main",
	kind: "branch",
	sha: "B",
	upstream: null,
	ahead: 0,
	behind: 0,
	gone: false,
	isHead: true,
};

function remoteMain(sha: string): RefInfo {
	return {
		name: "refs/remotes/origin/main",
		shortName: "origin/main",
		kind: "remoteBranch",
		sha,
		upstream: null,
		ahead: 0,
		behind: 0,
		gone: false,
		isHead: false,
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

function deps(overrides: Partial<GraphStoreDeps> = {}): GraphStoreDeps {
	return {
		getGraph: vi.fn(async () => topology),
		getCommitMeta: vi.fn(async () => []),
		getRefs: vi.fn(async () => ({ refs: [mainRef], head })),
		getWorktrees: vi.fn(async () => []),
		onRepoChanged: vi.fn(async () => () => {}),
		assignLanes: vi.fn(assignLanes),
		...overrides,
	};
}

describe("GraphStore", () => {
	it("refreshes refs-only updates without recomputing lanes", async () => {
		const storeDeps = deps();
		const store = new GraphStore(storeDeps);

		await store.open("repo-1");
		expect(storeDeps.assignLanes).toHaveBeenCalledTimes(1);
		expect(store.laneComputeCount).toBe(1);

		await store.handleChange({ kind: "refs" } as ChangeKind);

		expect(storeDeps.getRefs).toHaveBeenCalledTimes(2);
		expect(storeDeps.getGraph).toHaveBeenCalledTimes(1);
		expect(storeDeps.assignLanes).toHaveBeenCalledTimes(1);
		expect(store.laneComputeCount).toBe(1);
	});

	it("does not let an older refs read restore a stale remote-only pill", async () => {
		const oldRead = deferred<{ refs: RefInfo[]; head: HeadInfo }>();
		const freshRead = deferred<{ refs: RefInfo[]; head: HeadInfo }>();
		const trackingMain = { ...mainRef, upstream: "origin/main" };
		const storeDeps = deps({
			getRefs: vi
				.fn()
				.mockResolvedValueOnce({ refs: [trackingMain, remoteMain("B")], head })
				.mockReturnValueOnce(oldRead.promise)
				.mockReturnValueOnce(freshRead.promise),
		});
		const store = new GraphStore(storeDeps);
		await store.open("repo-1");

		const olderRefresh = store.refreshRefs();
		const newerRefresh = store.refreshRefs();
		freshRead.resolve({ refs: [trackingMain, remoteMain("B")], head });
		await newerRefresh;
		oldRead.resolve({ refs: [trackingMain, remoteMain("A")], head });
		await olderRefresh;

		expect(store.pillsBySha["B"]).toHaveLength(1);
		expect(store.pillsBySha["B"][0]).toMatchObject({ name: "main", local: true, remote: true });
		expect(store.pillsBySha["A"]).toBeUndefined();
	});

	it("reloads topology with the hidden local branch and its upstream excluded", async () => {
		const trackingMain = { ...mainRef, upstream: "origin/main" };
		const storeDeps = deps({
			getRefs: vi.fn(async () => ({ refs: [trackingMain, remoteMain("A")], head })),
		});
		const store = new GraphStore(storeDeps);
		await store.open("repo-1");
		vi.mocked(storeDeps.getGraph).mockClear();

		await store.toggleHiddenBranch("main");

		expect(storeDeps.getGraph).toHaveBeenCalledWith("repo-1", [
			"refs/heads/main",
			"refs/remotes/origin/main",
		]);
	});
});
