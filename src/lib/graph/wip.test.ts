import { describe, expect, it } from "vitest";
import { buildWipRow, WIP_SHA, wipCounts } from "./wip";
import type { StatusEntry } from "$lib/types";

function entry(over: Partial<StatusEntry>): StatusEntry {
	return {
		path: "f",
		origPath: null,
		kind: "ordinary",
		indexStatus: "unmodified",
		worktreeStatus: "unmodified",
		isSubmodule: false,
		...over,
	};
}

describe("wipCounts", () => {
	it("buckets untracked/added/copied as added, deletions as deleted, the rest as modified", () => {
		const counts = wipCounts([
			entry({ path: "a", kind: "untracked", worktreeStatus: "untracked" }),
			entry({ path: "b", worktreeStatus: "added" }),
			entry({ path: "c", worktreeStatus: "deleted" }),
			entry({ path: "d", worktreeStatus: "modified" }),
			entry({ path: "e", kind: "renamedOrCopied", worktreeStatus: "renamed" }),
		]);
		expect(counts).toEqual({ added: 2, deleted: 1, modified: 2 });
	});

	it("prefers the worktree status but falls back to the staged (index) status", () => {
		const counts = wipCounts([
			entry({ path: "staged-add", indexStatus: "added" }),
			entry({ path: "staged-del", indexStatus: "deleted" }),
		]);
		expect(counts).toEqual({ added: 1, deleted: 1, modified: 0 });
	});
});

describe("buildWipRow", () => {
	it("hangs a dashed connector off HEAD's lane and carries the change counts", () => {
		const row = buildWipRow(2, [entry({ worktreeStatus: "modified" })]);
		expect(row.kind).toBe("wip");
		expect(row.sha).toBe(WIP_SHA);
		expect(row.node.lane).toBe(2);
		expect(row.segments).toHaveLength(1);
		expect(row.segments[0].dashed).toBe(true);
		expect(row.segments[0].to).toEqual({ at: "bottom", lane: 2 });
		expect(row.counts.modified).toBe(1);
	});
});
