import { describe, expect, it } from "vitest";
import { fileName, stagedRows, unstagedRows } from "./sections";
import type { StatusEntry } from "$lib/types";

function entry(overrides: Partial<StatusEntry>): StatusEntry {
	return {
		path: "file.txt",
		origPath: null,
		kind: "ordinary",
		indexStatus: "unmodified",
		worktreeStatus: "unmodified",
		isSubmodule: false,
		...overrides,
	};
}

describe("unstagedRows", () => {
	it("includes files with a non-unmodified worktree state", () => {
		const rows = unstagedRows([entry({ worktreeStatus: "modified" })]);
		expect(rows).toEqual([
			{ path: "file.txt", origPath: null, status: "modified", partial: false },
		]);
	});

	it("includes untracked files under the untracked status", () => {
		const rows = unstagedRows([
			entry({ path: "new.txt", kind: "untracked", indexStatus: "untracked", worktreeStatus: "untracked" }),
		]);
		expect(rows[0].status).toBe("untracked");
		expect(rows[0].partial).toBe(false);
	});

	it("marks a file staged+worktree-modified as partial", () => {
		const rows = unstagedRows([
			entry({ indexStatus: "modified", worktreeStatus: "modified" }),
		]);
		expect(rows[0].partial).toBe(true);
	});

	it("shows a rename as a single renamed row, not delete+add", () => {
		const rows = unstagedRows([
			entry({
				path: "new_name.txt",
				origPath: "old_name.txt",
				kind: "renamedOrCopied",
				indexStatus: "unmodified",
				worktreeStatus: "renamed",
			}),
		]);
		expect(rows).toHaveLength(1);
		expect(rows[0].status).toBe("renamed");
		expect(rows[0].origPath).toBe("old_name.txt");
	});

	it("renders unmerged conflicts under the conflicted status", () => {
		const rows = unstagedRows([
			entry({
				path: "conflicted.txt",
				kind: "unmerged",
				indexStatus: "updatedButUnmerged",
				worktreeStatus: "updatedButUnmerged",
			}),
		]);
		expect(rows[0].status).toBe("updatedButUnmerged");
		expect(rows[0].partial).toBe(false);
	});

	it("excludes ignored and unmodified entries", () => {
		const rows = unstagedRows([entry({ kind: "ignored", worktreeStatus: "ignored" })]);
		expect(rows).toHaveLength(0);
	});
});

describe("stagedRows", () => {
	it("includes ordinary entries with a non-unmodified index state", () => {
		const rows = stagedRows([entry({ indexStatus: "added" })]);
		expect(rows[0].status).toBe("added");
	});

	it("excludes untracked and unmerged entries entirely", () => {
		const rows = stagedRows([
			entry({ kind: "untracked", indexStatus: "untracked", worktreeStatus: "untracked" }),
			entry({
				kind: "unmerged",
				indexStatus: "updatedButUnmerged",
				worktreeStatus: "updatedButUnmerged",
			}),
		]);
		expect(rows).toHaveLength(0);
	});

	it("shows a staged rename as a single row", () => {
		const rows = stagedRows([
			entry({
				path: "new_name.txt",
				origPath: "old_name.txt",
				kind: "renamedOrCopied",
				indexStatus: "renamed",
			}),
		]);
		expect(rows).toHaveLength(1);
		expect(rows[0].status).toBe("renamed");
	});

	it("marks a partially-staged file as partial", () => {
		const rows = stagedRows([
			entry({ indexStatus: "modified", worktreeStatus: "modified" }),
		]);
		expect(rows[0].partial).toBe(true);
	});
});

describe("fileName", () => {
	it("returns the last path segment", () => {
		expect(fileName("web/src/app.ts")).toBe("app.ts");
		expect(fileName("root.txt")).toBe("root.txt");
	});
});
