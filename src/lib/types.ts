/** Mirrors Rust DTOs exactly — ARCHITECTURE.md §1. */

// --- git/log.rs ---------------------------------------------------------

export interface CommitTopology {
	sha: string;
	parents: string[];
}

export interface CommitMeta {
	sha: string;
	parents: string[];
	authorName: string;
	authorEmail: string;
	/** Unix seconds (author date). */
	authorTime: number;
	subject: string;
	/**
	 * Full commit body. May be empty. May contain embedded newlines — show only the first line
	 * in the graph preview; the rest is available without a re-fetch.
	 */
	body: string;
}

export interface StashEntry {
	sha: string;
	/** The commit the stash was taken on top of (its first parent). */
	baseSha: string;
	/** Reflog selector, e.g. `stash@{0}`. */
	selector: string;
	subject: string;
}

// --- git/refs.rs ---------------------------------------------------------

export type RefKind = 'branch' | 'remoteBranch' | 'tag';

export interface RefInfo {
	/** Full ref name, e.g. `refs/heads/main`. */
	name: string;
	/** `refs/heads/`, `refs/remotes/`, `refs/tags/` stripped. */
	shortName: string;
	kind: RefKind;
	sha: string;
	upstream: string | null;
	ahead: number;
	behind: number;
	/** Upstream existed but was deleted. */
	gone: boolean;
	isHead: boolean;
}

export interface HeadInfo {
	detached: boolean;
	/** Short branch name, e.g. `main`. `null` when detached. */
	branch: string | null;
	sha: string;
}

// --- git/status.rs ---------------------------------------------------------

export type FileStatusCode =
	| 'unmodified'
	| 'modified'
	| 'added'
	| 'deleted'
	| 'renamed'
	| 'copied'
	| 'updatedButUnmerged'
	| 'typeChanged'
	| 'untracked'
	| 'ignored';

export type StatusEntryKind = 'ordinary' | 'renamedOrCopied' | 'unmerged' | 'untracked' | 'ignored';

export interface StatusEntry {
	path: string;
	/** Set for renames/copies. */
	origPath: string | null;
	kind: StatusEntryKind;
	/** Index/staged state. `unmodified` maps from `.`. */
	indexStatus: FileStatusCode;
	/**
	 * Worktree/unstaged state. `unmodified` maps from `.`.
	 *
	 * A single entry can have both non-`unmodified` — that's a partially-staged file; render it
	 * into both the staged and unstaged lists from this one entry.
	 */
	worktreeStatus: FileStatusCode;
	isSubmodule: boolean;
}

export interface BranchStatus {
	oid: string | null;
	/** `null` when detached. */
	head: string | null;
	upstream: string | null;
	ahead: number;
	behind: number;
}

export interface StatusReport {
	branch: BranchStatus;
	entries: StatusEntry[];
}

// --- git/diff.rs ---------------------------------------------------------

export type DiffLineKind = 'context' | 'add' | 'del';

export interface DiffLine {
	kind: DiffLineKind;
	oldNo: number | null;
	newNo: number | null;
	text: string;
	noNewlineAtEof: boolean;
}

export interface Hunk {
	header: string;
	lines: DiffLine[];
}

export interface FileDiff {
	isBinary: boolean;
	isImage: boolean;
	oldPath: string | null;
	newPath: string | null;
	hunks: Hunk[];
}
