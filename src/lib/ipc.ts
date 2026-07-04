import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  ChangedFile,
  ChangeKind,
  CommitMeta,
  Divergence,
  FileDiff,
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

export async function mergeRef(repoId: string, source: string): Promise<void> {
  return invoke("merge_ref", { repoId, source });
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

export async function pull(repoId: string, mode: "ff" | "rebase" | "merge"): Promise<void> {
  return invoke("pull", { repoId, mode });
}

export async function push(repoId: string, force: boolean): Promise<void> {
  return invoke("push", { repoId, force });
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

// --- diffs (ARCHITECTURE.md §6.2) ---

export async function getDiffWorktree(repoId: string, path: string, ignoreWhitespace: boolean): Promise<FileDiff> {
  return invoke("get_diff_worktree", { repoId, path, ignoreWhitespace });
}

export async function getDiffStaged(repoId: string, path: string, ignoreWhitespace: boolean): Promise<FileDiff> {
  return invoke("get_diff_staged", { repoId, path, ignoreWhitespace });
}

export async function getDiffCommit(repoId: string, sha: string, path: string, ignoreWhitespace: boolean): Promise<FileDiff> {
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
