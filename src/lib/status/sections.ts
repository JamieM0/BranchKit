/** Derives the Unstaged/Staged row lists from a `StatusReport` — DESIGN_SPEC.md §6.1. A file with
 * both a non-`.` index and worktree state is partially staged and appears in *both* lists (the
 * caller renders a half-filled glyph for it, §6.1 "A file that is both partially staged..."). */

import type { FileStatusCode, StatusEntry } from "$lib/types";

export interface FileRow {
	path: string;
	/** Set for renames/copies. */
	origPath: string | null;
	/** The status code that drives this row's glyph in *this* section. */
	status: FileStatusCode;
	/** True when the same path also appears in the other section. */
	partial: boolean;
}

function parentPath(path: string): string | null {
	const idx = path.lastIndexOf("/");
	return idx === -1 ? null : path.slice(0, idx);
}

export function fileName(path: string): string {
	const idx = path.lastIndexOf("/");
	return idx === -1 ? path : path.slice(idx + 1);
}

export { parentPath };

/** Unstaged section: every entry whose worktree state is non-`unmodified` — untracked files
 * included (their worktree state is `untracked`) — rendered under a Conflicted glyph when the
 * entry is unmerged. */
export function unstagedRows(entries: readonly StatusEntry[]): FileRow[] {
	return entries
		.filter((e) => e.kind !== "ignored" && e.worktreeStatus !== "unmodified")
		.map((e) => ({
			path: e.path,
			origPath: e.origPath,
			status: e.kind === "unmerged" ? "updatedButUnmerged" : e.worktreeStatus,
			partial:
				e.kind !== "untracked" && e.kind !== "unmerged" && e.indexStatus !== "unmodified",
		}));
}

/** Staged section: ordinary/renamed entries with a non-`unmodified` index state. Untracked and
 * unmerged (conflicted) entries never appear here — an untracked file becomes an ordinary `added`
 * entry the moment it's staged, and a conflict isn't "staged" in the normal sense. */
export function stagedRows(entries: readonly StatusEntry[]): FileRow[] {
	return entries
		.filter((e) => e.kind === "ordinary" || e.kind === "renamedOrCopied")
		.filter((e) => e.indexStatus !== "unmodified")
		.map((e) => ({
			path: e.path,
			origPath: e.origPath,
			status: e.indexStatus,
			partial: e.worktreeStatus !== "unmodified",
		}));
}
