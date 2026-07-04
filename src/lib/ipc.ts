import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  ChangeKind,
  CommitMeta,
  GitIdentity,
  GraphTopologyRow,
  RecentRepo,
  RefsResponse,
  RepoInfo,
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
