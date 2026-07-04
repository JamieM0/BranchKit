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

function deps(overrides: Partial<GraphStoreDeps> = {}): GraphStoreDeps {
	return {
		getGraph: vi.fn(async () => topology),
		getCommitMeta: vi.fn(async () => []),
		getRefs: vi.fn(async () => ({ refs: [mainRef], head })),
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
});
