import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import type {
  BlameRun,
  ChangedFile,
  ChangeKind,
  CommitMeta,
  ConflictState,
  DiscardedEntry,
  Divergence,
  FileConflictRegions,
  FileDiff,
  FileHistoryEntry,
  GitIdentity,
  GraphTopologyRow,
  RecentRepo,
  RefsResponse,
  RepoInfo,
  StatusReport,
  WorktreeInfo,
} from "./types";

/** The ONLY place invoke() is called — ARCHITECTURE.md §1. */

export async function greet(name: string): Promise<string> {
  return invoke("greet", { name });
}

export async function openRepo(path: string): Promise<RepoInfo> {
  return invoke("open_repo", { path });
}

export async function cloneRepo(
  requestId: string,
  url: string,
  destination: string,
): Promise<RepoInfo> {
  return invoke("clone_repo", { requestId, url, destination });
}

export async function closeRepo(id: string): Promise<void> {
  return invoke("close_repo", { id });
}

export async function listRecents(): Promise<RecentRepo[]> {
  return invoke("list_recents");
}

export async function checkGitIdentity(): Promise<GitIdentity> {
  return invoke("check_git_identity");
}

export async function setGitIdentity(name: string, email: string): Promise<void> {
  return invoke("set_git_identity", { name, email });
}

export async function getGraph(repoId: string): Promise<GraphTopologyRow[]> {
  return invoke("get_graph", { repoId });
}

export async function getCommitMeta(repoId: string, shas: string[]): Promise<CommitMeta[]> {
  return invoke("get_commit_meta", { repoId, shas });
}

export async function getRefs(repoId: string): Promise<RefsResponse> {
  return invoke("get_refs", { repoId });
}

export async function getWorktrees(repoId: string): Promise<WorktreeInfo[]> {
  return invoke("get_worktrees", { repoId });
}

/** Creates a linked worktree at `path` checked out to `startRef`; `newBranch` set creates that
 * branch at `startRef` in the same action — the create-worktree dialog (DESIGN_SPEC.md §5). */
export async function createWorktree(
  repoId: string,
  path: string,
  startRef: string,
  newBranch: string | null,
): Promise<void> {
  return invoke("create_worktree", { repoId, path, startRef, newBranch });
}

/** Removes a linked worktree; `force` bypasses git's own dirty-check guard — call once without it,
 * and only retry with `force: true` after the user confirms the armed "Remove anyway". */
export async function removeWorktree(repoId: string, path: string, force: boolean): Promise<void> {
  return invoke("remove_worktree", { repoId, path, force });
}

/** Prunes stale worktree administrative data — the WORKTREES section's "Prune all". */
export async function pruneWorktrees(repoId: string): Promise<void> {
  return invoke("prune_worktrees", { repoId });
}

// --- file history & blame (ARCHITECTURE.md §5.1-style follow, DESIGN_SPEC.md §6.3) ---

export async function getFileHistory(repoId: string, path: string): Promise<FileHistoryEntry[]> {
  return invoke("get_file_history", { repoId, path });
}

export async function getFileHistoryDiff(
  repoId: string,
  path: string,
  sha: string,
): Promise<FileDiff> {
  return invoke("get_file_history_diff", { repoId, path, sha });
}

export async function getBlame(repoId: string, path: string): Promise<BlameRun[]> {
  return invoke("get_blame", { repoId, path });
}

// --- mutations (ARCHITECTURE.md §7.1) — each runs through the repo op queue in Rust and emits its
// own targeted refresh event on success. ---

export async function checkoutBranch(repoId: string, name: string): Promise<void> {
  return invoke("checkout_branch", { repoId, name });
}

/** Create-tracking-branch-and-checkout in one action; resolves to the new local branch name. */
export async function checkoutRemote(repoId: string, remoteRef: string): Promise<string> {
  return invoke("checkout_remote", { repoId, remoteRef });
}

export async function checkoutPrevious(repoId: string): Promise<void> {
  return invoke("checkout_previous", { repoId });
}

export async function checkoutDetached(repoId: string, sha: string): Promise<void> {
  return invoke("checkout_detached", { repoId, sha });
}

export async function createBranch(
  repoId: string,
  name: string,
  sha: string | null,
  checkout: boolean,
): Promise<void> {
  return invoke("create_branch", { repoId, name, sha, checkout });
}

export async function renameBranch(
  repoId: string,
  oldName: string,
  newName: string,
): Promise<void> {
  return invoke("rename_branch", { repoId, oldName, newName });
}

/** Deletes a branch; resolves to its recorded tip sha so a toast can Undo (recreate at it). */
export async function deleteBranch(repoId: string, name: string, force: boolean): Promise<string> {
  return invoke("delete_branch", { repoId, name, force });
}

export async function recreateBranch(repoId: string, name: string, sha: string): Promise<void> {
  return invoke("recreate_branch", { repoId, name, sha });
}

export async function mergeRef(
  repoId: string,
  source: string,
  allowUnrelated = false,
): Promise<void> {
  return invoke("merge_ref", { repoId, source, allowUnrelated });
}

export async function rebaseOnto(repoId: string, onto: string): Promise<void> {
  return invoke("rebase_onto", { repoId, onto });
}

export async function fastForward(
  repoId: string,
  branch: string,
  source: string,
  isCurrent: boolean,
): Promise<void> {
  return invoke("fast_forward", { repoId, branch, source, isCurrent });
}

export async function fetchAll(repoId: string): Promise<void> {
  return invoke("fetch_all", { repoId });
}

export async function pull(repoId: string, mode: "ff" | "rebase" | "merge"): Promise<void> {
  return invoke("pull", { repoId, mode });
}

export async function push(repoId: string, force: boolean): Promise<void> {
  return invoke("push", { repoId, force });
}

/** Push a branch with no upstream yet, setting `origin/<name>` as its tracking ref in one action —
 * the toolbar's Push-becomes-Publish state (DESIGN_SPEC.md §3.2). */
export async function publish(repoId: string, name: string): Promise<void> {
  return invoke("publish", { repoId, name });
}

/** Stash, checkout `name`, then pop — the "would be overwritten by checkout" error's suggested
 * compound action (ARCHITECTURE.md §9). */
export async function checkoutStashAndSwitch(repoId: string, name: string): Promise<void> {
  return invoke("checkout_stash_and_switch", { repoId, name });
}

export async function setUpstream(
  repoId: string,
  branch: string,
  upstream: string,
): Promise<void> {
  return invoke("set_upstream", { repoId, branch, upstream });
}

export async function branchDivergence(repoId: string, branch: string): Promise<Divergence> {
  return invoke("branch_divergence", { repoId, branch });
}

// --- rewriting history / tags (GITKRAKEN_WORKFLOWS.md §2.6/§2.9, §3.1) ---

export async function cherryPick(repoId: string, sha: string): Promise<void> {
  return invoke("cherry_pick", { repoId, sha });
}

export async function revertCommit(repoId: string, sha: string): Promise<void> {
  return invoke("revert_commit", { repoId, sha });
}

export async function resetTo(
  repoId: string,
  sha: string,
  mode: "soft" | "mixed" | "hard",
): Promise<void> {
  return invoke("reset_to", { repoId, sha, mode });
}

export async function createTag(
  repoId: string,
  name: string,
  sha: string,
  message: string | null,
): Promise<void> {
  return invoke("create_tag", { repoId, name, sha, message });
}

export async function deleteTag(repoId: string, name: string): Promise<void> {
  return invoke("delete_tag", { repoId, name });
}

export async function getRemoteUrl(repoId: string, remote: string): Promise<string> {
  return invoke("get_remote_url", { repoId, remote });
}

export async function ignorePath(repoId: string, pattern: string): Promise<void> {
  return invoke("ignore_path", { repoId, pattern });
}

// --- stash (DESIGN_SPEC.md §3.2/§4.5/§15.18) ---

export async function stashPush(
  repoId: string,
  message: string | null,
  includeUntracked: boolean,
): Promise<void> {
  return invoke("stash_push", { repoId, message, includeUntracked });
}

export async function stashPop(repoId: string, selector: string): Promise<void> {
  return invoke("stash_pop", { repoId, selector });
}

export async function stashApply(repoId: string, selector: string): Promise<void> {
  return invoke("stash_apply", { repoId, selector });
}

export async function stashDrop(repoId: string, selector: string): Promise<void> {
  return invoke("stash_drop", { repoId, selector });
}

export async function getStashPatch(repoId: string, selector: string): Promise<string> {
  return invoke("get_stash_patch", { repoId, selector });
}

// --- status & staging (ARCHITECTURE.md §6.1, §7.1) ---

export async function getStatus(repoId: string): Promise<StatusReport> {
  return invoke("get_status", { repoId });
}

export async function stageFile(repoId: string, path: string): Promise<void> {
  return invoke("stage_file", { repoId, path });
}

export async function unstageFile(repoId: string, path: string): Promise<void> {
  return invoke("unstage_file", { repoId, path });
}

export async function stageAll(repoId: string): Promise<void> {
  return invoke("stage_all", { repoId });
}

export async function unstageAll(repoId: string): Promise<void> {
  return invoke("unstage_all", { repoId });
}

/** Stages a subset of one hunk's changed lines — pass every changed index in the hunk to stage
 * the whole hunk (ARCHITECTURE.md §6.3, DESIGN_SPEC.md §6.2/§15.11). `hunkIndex`/`lineIndices`
 * must come from a diff fetched *without* the whitespace-ignore toggle. */
export async function stageLines(
  repoId: string,
  path: string,
  hunkIndex: number,
  lineIndices: number[],
): Promise<void> {
  return invoke("stage_lines", { repoId, path, hunkIndex, lineIndices });
}

/** Same mechanics reversed for the Staged view (DESIGN_SPEC.md §6.2). */
export async function unstageLines(
  repoId: string,
  path: string,
  hunkIndex: number,
  lineIndices: number[],
): Promise<void> {
  return invoke("unstage_lines", { repoId, path, hunkIndex, lineIndices });
}

// --- commit composer (ARCHITECTURE.md §7.1, DESIGN_SPEC.md §7) ---

/** Commit the composer's summary + description. Resolves to the new HEAD sha so the toast can name
 * it (§8). `amend` replaces the tip commit; the message goes over stdin when it exceeds ~8k chars
 * (handled in Rust). */
export async function commit(
  repoId: string,
  summary: string,
  description: string,
  amend: boolean,
): Promise<string> {
  return invoke("commit", { repoId, summary, description, amend });
}

/** The commit toast's **Undo** — a soft reset of the last commit (DESIGN_SPEC.md §8/§15.13). */
export async function undoCommit(repoId: string): Promise<void> {
  return invoke("undo_commit", { repoId });
}

// --- discard safety net (ARCHITECTURE.md §7.3, DESIGN_SPEC.md §7.4) ---

export async function discardFile(repoId: string, path: string): Promise<void> {
  return invoke("discard_file", { repoId, path });
}

export async function discardHunk(repoId: string, path: string, hunkIndex: number): Promise<void> {
  return invoke("discard_hunk", { repoId, path, hunkIndex });
}

export async function discardAll(repoId: string): Promise<void> {
  return invoke("discard_all", { repoId });
}

export async function listDiscarded(repoId: string): Promise<DiscardedEntry[]> {
  return invoke("list_discarded", { repoId });
}

export async function restoreDiscarded(repoId: string, entryId: string): Promise<void> {
  return invoke("restore_discarded", { repoId, entryId });
}

// --- conflicts / Keep Panel (ARCHITECTURE.md §7.4/§7.5, DESIGN_SPEC.md §9) ---

/** `null` when the working tree has no conflict of any kind active. */
export async function getConflictState(repoId: string): Promise<ConflictState | null> {
  return invoke("get_conflict_state", { repoId });
}

/** Continue the in-progress operation — the banner's "Continue merge" (and rebase/cherry-pick/
 * revert equivalents), DESIGN_SPEC.md §9.1/§9.2. `message` edits the merge commit's message (the
 * inline field); it's ignored by the kinds whose `--continue` reuses a stored message. */
export async function continueConflict(repoId: string, message?: string): Promise<void> {
  return invoke("continue_conflict", { repoId, message: message ?? null });
}

/** Abort the in-progress operation — the banner's "Abort…" (DESIGN_SPEC.md §9.1/§9.3). */
export async function abortConflict(repoId: string): Promise<void> {
  return invoke("abort_conflict", { repoId });
}

/** A conflicted file's Keep Panel regions (ARCHITECTURE.md §7.5). */
export async function getConflictRegions(repoId: string, path: string): Promise<FileConflictRegions> {
  return invoke("get_conflict_regions", { repoId, path });
}

/** Writes the Keep Panel's assembled resolved text and stages it — the panel's Confirm button
 * (DESIGN_SPEC.md §9.2). */
export async function confirmFile(repoId: string, path: string, resolvedText: string): Promise<void> {
  return invoke("confirm_file", { repoId, path, resolvedText });
}

/** Regenerates the conflict for a previously-confirmed file — "Reset file" (DESIGN_SPEC.md §9.2). */
export async function reopenFile(repoId: string, path: string): Promise<void> {
  return invoke("reopen_file", { repoId, path });
}

// --- diffs (ARCHITECTURE.md §6.2) ---

export async function getDiffWorktree(
  repoId: string,
  path: string,
  ignoreWhitespace: boolean,
): Promise<FileDiff> {
  return invoke("get_diff_worktree", { repoId, path, ignoreWhitespace });
}

export async function getDiffStaged(
  repoId: string,
  path: string,
  ignoreWhitespace: boolean,
): Promise<FileDiff> {
  return invoke("get_diff_staged", { repoId, path, ignoreWhitespace });
}

export async function getDiffCommit(
  repoId: string,
  sha: string,
  path: string,
  ignoreWhitespace: boolean,
): Promise<FileDiff> {
  return invoke("get_diff_commit", { repoId, sha, path, ignoreWhitespace });
}

export async function getDiffTwoCommits(
  repoId: string,
  a: string,
  b: string,
  path: string,
  ignoreWhitespace: boolean,
): Promise<FileDiff> {
  return invoke("get_diff_two_commits", { repoId, a, b, path, ignoreWhitespace });
}

export async function getCommitFiles(repoId: string, sha: string): Promise<ChangedFile[]> {
  return invoke("get_commit_files", { repoId, sha });
}

export async function getDiffFiles(repoId: string, a: string, b: string): Promise<ChangedFile[]> {
  return invoke("get_diff_files", { repoId, a, b });
}

/** A commit vs the current worktree — "Compare commit against working directory" (§2.6/§3.1). */
export async function getDiffCommitVsWorking(
  repoId: string,
  sha: string,
  path: string,
  ignoreWhitespace: boolean,
): Promise<FileDiff> {
  return invoke("get_diff_commit_vs_working", { repoId, sha, path, ignoreWhitespace });
}

export async function getCommitFilesVsWorking(repoId: string, sha: string): Promise<ChangedFile[]> {
  return invoke("get_commit_files_vs_working", { repoId, sha });
}

/** A whole commit as a mailbox-format patch — "Create patch from commit" (§2.9/§3.1). */
export async function createPatchFromCommit(repoId: string, sha: string): Promise<string> {
  return invoke("create_patch_from_commit", { repoId, sha });
}

/** One file's current changes as a unified diff — the file row menu's "Create patch from file
 * changes" (§3.4). */
export async function createPatchFromFile(
  repoId: string,
  path: string,
  staged: boolean,
): Promise<string> {
  return invoke("create_patch_from_file", { repoId, path, staged });
}

/** `revision: null` reads the worktree file off disk; `":"` reads the staged/index blob; any
 * other string is a commit sha — the diff viewer's image-diff before/after (§6.2). */
export async function getBlob(
  repoId: string,
  revision: string | null,
  path: string,
): Promise<string> {
  const blob = await invoke<{ base64: string }>("get_blob", { repoId, revision, path });
  return blob.base64;
}

/** Subscribes to `repo://{id}/changed` — ARCHITECTURE.md §2. Returns the unlisten function. */
export async function onRepoChanged(
  id: string,
  handler: (kind: ChangeKind) => void,
): Promise<UnlistenFn> {
  return listen<ChangeKind>(`repo://${id}/changed`, (event) => handler(event.payload));
}

/** Subscribes to a clone's progress stream — see the SPEC-DEVIATION note on `clone_repo` in
 * repo.rs: there's no repo id yet during a clone, so progress uses a request-scoped channel. */
/** Native "Open repo" folder picker — not an `invoke()` call itself, but the dialog plugin is
 * the only sanctioned way to reach the OS filesystem picker, so it lives here alongside ipc. */
export async function pickFolder(title: string): Promise<string | null> {
  const result = await openDialog({ title, directory: true, multiple: false });
  return typeof result === "string" ? result : null;
}

/** Native "Save patch as…" dialog + write — "Create patch from commit/file" (§2.9/§3.1/§3.4). */
export async function savePatchAs(defaultName: string, contents: string): Promise<boolean> {
  const path = await saveDialog({ defaultPath: defaultName, filters: [{ name: "Patch", extensions: ["patch"] }] });
  if (!path) return false;
  await invoke("save_text_file", { path, contents });
  return true;
}

export async function onCloneProgress(
  requestId: string,
  handler: (phase: string, percent: number | null) => void,
): Promise<UnlistenFn> {
  return listen<ChangeKind>(`clone://${requestId}/progress`, (event) => {
    if (event.payload.kind === "operationProgress") {
      handler(event.payload.phase, event.payload.percent);
    }
  });
}
