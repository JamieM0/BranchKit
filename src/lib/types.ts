/** Mirrors Rust DTOs exactly — ARCHITECTURE.md §1. */

// --- repo.rs ---------------------------------------------------------

export interface RepoInfo {
	id: string;
	path: string;
	name: string;
	/** `null` on an unborn branch (no commits yet) as well as on a real detached HEAD. */
	branch: string | null;
	detached: boolean;
}

export interface RecentRepo {
	path: string;
	name: string;
	/** Unix seconds. */
	lastOpenedAt: number;
}

// --- events.rs ---------------------------------------------------------

export type ChangeKind =
	| { kind: 'workingTree' }
	| { kind: 'index' }
	| { kind: 'refs' }
	| { kind: 'head' }
	| { kind: 'remote' }
	| { kind: 'operationProgress'; phase: string; percent: number | null };

// --- git/identity.rs ---------------------------------------------------------

export interface GitIdentity {
	name: string | null;
	email: string | null;
}

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

export type GraphTopologyRow =
	| { kind: "commit"; sha: string; parents: string[] }
	| {
			kind: "stash";
			sha: string;
			baseSha: string;
			selector: string;
			subject: string;
	  };

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

export interface RefsResponse {
	refs: RefInfo[];
	head: HeadInfo;
}

// --- error.rs ---------------------------------------------------------

/** A suggested next action attached to a translated error — DESIGN_SPEC.md §11. `actionId` is an
 * opaque string the frontend's error-handling catalog switches on (see `actions.ts`). */
export interface Suggestion {
	label: string;
	actionId: string;
}

/** The shape every failed `invoke()` rejects with — ARCHITECTURE.md §9. */
export interface AppError {
	userMessage: string;
	suggestion: Suggestion | null;
	raw: string;
}

// --- git/ops.rs ---------------------------------------------------------

export interface CommitLine {
	sha: string;
	subject: string;
}

export interface Divergence {
	/** `↑` commits to push (ahead of upstream). */
	outgoing: CommitLine[];
	/** `↓` commits to pull (behind upstream). */
	incoming: CommitLine[];
}

// --- git/worktree.rs ---------------------------------------------------------

export interface WorktreeInfo {
	path: string;
	/** Short branch name, or `null` when the worktree is detached. */
	branch: string | null;
	head: string;
	detached: boolean;
	/** The main (non-linked) worktree — can't be removed. */
	isMain: boolean;
	locked: boolean;
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
	/** Every change is CRLF↔LF noise (the diff vanishes under `--ignore-cr-at-eol`) — the viewer
	 * shows a quiet "line endings only" note (ARCHITECTURE.md §14). */
	eolOnly: boolean;
}

export type ChangedFileStatus =
	| 'added'
	| 'modified'
	| 'deleted'
	| 'renamed'
	| 'copied'
	| 'typeChanged';

export interface ChangedFile {
	path: string;
	/** Set for renames/copies. */
	origPath: string | null;
	status: ChangedFileStatus;
}

// --- git/log.rs (stash) ---------------------------------------------------------

export interface StashEntry {
	sha: string;
	/** The commit the stash was taken on top of (its first parent). */
	baseSha: string;
	/** Reflog selector, e.g. `stash@{0}`. */
	selector: string;
	subject: string;
}

// --- git/discard.rs ---------------------------------------------------------

/** One entry in the repo menu's "Recently discarded" list — DESIGN_SPEC.md §7.4/§12. */
export interface DiscardedEntry {
	id: string;
	description: string;
	files: string[];
	/** Unix milliseconds. */
	createdAtMs: number;
}

// --- git/conflict.rs ---------------------------------------------------------

export type ConflictKind = "merge" | "rebase" | "cherryPick" | "revert" | "stashApply";

/** The in-progress operation blocking the working tree — ARCHITECTURE.md §7.4, DESIGN_SPEC.md §9.1.
 * Labels are always real branch/commit names, never "ours/theirs" (DESIGN_SPEC.md §4 principle 4). */
export interface ConflictState {
	kind: ConflictKind;
	sourceLabel: string;
	targetLabel: string;
	files: string[];
}

export type Side = "ours" | "theirs";

/** One span of a conflicted file's future content — ARCHITECTURE.md §7.5. Never derived from
 * `<<<<<<<` markers; computed structurally from a 3-way diff against the base. */
export type FileRegion =
	| { kind: "context"; lines: string[] }
	| { kind: "autoResolved"; side: Side; lines: string[] }
	| {
			kind: "conflict";
			baseStart: number;
			baseEnd: number;
			/** Lines identical on both sides, peeled off the region's edges (DESIGN_SPEC.md §9.3
			 * "same in both" dedupe) so only the genuinely divergent middle needs a decision. */
			sameBothPrefix: string[];
			oursLines: string[];
			theirsLines: string[];
			sameBothSuffix: string[];
	  };

export interface FileConflictRegions {
	/** The file doesn't exist in `ours` (stage `:2:`) — a modify/delete conflict, this side deleted it. */
	oursDeleted: boolean;
	/** Same, for `theirs` (stage `:3:`). */
	theirsDeleted: boolean;
	regions: FileRegion[];
}

// --- git/history.rs ---------------------------------------------------------

/** One row of a file's `--follow` history — DESIGN_SPEC.md §6.3. */
export interface FileHistoryEntry {
	sha: string;
	authorName: string;
	authorEmail: string;
	/** Unix seconds (author date). */
	authorTime: number;
	subject: string;
}

// --- git/blame.rs ---------------------------------------------------------

/** A contiguous run of lines attributed to the same commit — the blame gutter's unit
 * (DESIGN_SPEC.md §6.3). Uncommitted lines carry git's all-zero sha and author "Not Committed
 * Yet". */
export interface BlameRun {
	sha: string;
	authorName: string;
	authorEmail: string;
	/** Unix seconds (author date). */
	authorTime: number;
	summary: string;
	/** 1-based line number of the run's first line in the current file. */
	startLine: number;
	lines: string[];
}

// --- settings.rs ---------------------------------------------------------

export type PullModeSetting = "ff" | "rebase" | "merge";

export interface GeneralSettings {
	autoFetchIntervalMinutes: number;
	openLastReposOnLaunch: boolean;
	defaultCloneDir: string | null;
}

export interface AppearanceSettings {
	theme: "system" | "dark" | "light";
	graphDensity: "comfortable" | "compact";
	dateStyle: "relative" | "absolute";
	showAvatars: boolean;
}

export interface GitSettings {
	defaultPullMode: PullModeSetting;
	pushTagsWithCommits: boolean;
	pruneOnFetch: boolean;
	combineTrackingBranches: boolean;
	commitSummaryGuideLength: number;
}

export type AiProviderKind = "local" | "ollama" | "remote";
export type RemoteApiFormat = "openAi" | "anthropic";
export type CommitStyle = "plain" | "conventional";

export interface AiSettings {
	enabled: boolean;
	provider: AiProviderKind;
	ollamaBaseUrl: string;
	ollamaModel: string | null;
	remoteFormat: RemoteApiFormat;
	remoteBaseUrl: string;
	remoteModel: string;
	style: CommitStyle;
	maxDiffSizeKb: number;
}

export interface AppSettings {
	general: GeneralSettings;
	appearance: AppearanceSettings;
	git: GitSettings;
	ai: AiSettings;
}

// --- ai/ ---------------------------------------------------------

export type LocalModelState = "notDownloaded" | "ready";

export interface LocalDownloadProgress {
	phase: string;
	percent: number | null;
	mbps: number;
}

export interface GeneratedCommitMessage {
	summary: string;
	description: string;
}

export interface AiTestResult {
	ok: boolean;
	message: string;
}

// --- credentials.rs ---------------------------------------------------------

export interface CredentialInfo {
	host: string;
	username: string;
	/** Unix seconds. */
	lastUsedAt: number;
}

export interface SshAgentStatus {
	agentRunning: boolean;
	identities: string[];
}

export interface SshKeyInfo {
	publicKey: string;
	path: string;
}

// --- github/mod.rs, github/api.rs ---------------------------------------------------------

export interface DeviceCode {
	deviceCode: string;
	userCode: string;
	verificationUri: string;
	expiresIn: number;
	interval: number;
}

export interface GithubUser {
	login: string;
	avatarUrl: string;
}

export interface PullRequest {
	number: number;
	title: string;
	body: string;
	state: string;
	draft: boolean;
	headRef: string;
	baseRef: string;
	headSha: string;
	authorLogin: string;
	authorAvatarUrl: string;
	htmlUrl: string;
	commentCount: number;
	reviewers: string[];
}

export interface CheckRun {
	name: string;
	status: string;
	conclusion: string | null;
	htmlUrl: string;
}

export type CheckSummary = "success" | "failure" | "pending" | "none";

export interface CommitCheckStatus {
	summary: CheckSummary;
	runs: CheckRun[];
}

export interface CreatedPr {
	number: number;
	htmlUrl: string;
}

/** SPEC-DEVIATION (ARCHITECTURE.md §11 / DESIGN_SPEC.md §12): repo-creation types, not in the
 * documented v1 GitHub scope — see github/api.rs's header comment. */
export interface GithubOrg {
	login: string;
	avatarUrl: string;
}

export interface CreatedGithubRepo {
	htmlUrl: string;
	cloneUrl: string;
}
