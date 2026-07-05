/** Git action orchestration — the layer between UI gestures (pills, menus, drop menu, left panel)
 * and the typed IPC mutations. Each action calls `ipc`, then wires the DESIGN_SPEC §8 toast with
 * its single undo/redo verb, and routes failures through the shared error toast (§11). Kept out of
 * `ipc.ts` (which is only `invoke()` wrappers) and out of components (so the toast catalog lives in
 * one place). */

import * as ipc from "$lib/ipc";
import { toasts } from "$lib/stores/toasts.svelte";
import { graphNav } from "$lib/stores/graphNav.svelte";
import { graph } from "$lib/stores/graph.svelte";
import { status } from "$lib/stores/status.svelte";
import { commitDraft } from "$lib/stores/commitDraft.svelte";
import { stagedRows } from "$lib/status/sections";

interface AppErrorShape {
	userMessage: string;
	raw: string;
}

function asAppError(e: unknown): AppErrorShape {
	if (e && typeof e === "object" && "userMessage" in e) {
		const o = e as Record<string, unknown>;
		return { userMessage: String(o.userMessage), raw: String(o.raw ?? "") };
	}
	return { userMessage: e instanceof Error ? e.message : String(e), raw: String(e) };
}

/** True when a `git branch -d` failed because the branch isn't fully merged — the guard that
 * gates the armed force-delete confirm (DESIGN_SPEC §4.6/§15.13). */
export function isUnmergedError(e: unknown): boolean {
	const { userMessage, raw } = asAppError(e);
	return /not fully merged/i.test(`${userMessage} ${raw}`);
}

/** Checkout a local branch → "Switched to `x` — Back" (§8, §15.14). */
export async function checkoutBranch(repoId: string, name: string): Promise<void> {
	try {
		await ipc.checkoutBranch(repoId, name);
		toasts.push({
			message: `Switched to \`${name}\``,
			tone: "success",
			icon: "check",
			action: { label: "Back", run: () => backToPrevious(repoId) },
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

/** Track a remote branch + check it out in one action, then the same Back toast (§4.4/§15.1). */
export async function checkoutRemote(repoId: string, remoteRef: string): Promise<void> {
	try {
		const local = await ipc.checkoutRemote(repoId, remoteRef);
		toasts.push({
			message: `Switched to \`${local}\` (tracking \`${remoteRef}\`)`,
			tone: "success",
			icon: "check",
			action: { label: "Back", run: () => backToPrevious(repoId) },
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function backToPrevious(repoId: string): Promise<void> {
	try {
		await ipc.checkoutPrevious(repoId);
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function checkoutDetached(repoId: string, sha: string): Promise<void> {
	try {
		await ipc.checkoutDetached(repoId, sha);
		toasts.push({
			message: `Detached at \`${sha.slice(0, 7)}\``,
			tone: "warn",
			icon: "check",
			action: { label: "Back", run: () => backToPrevious(repoId) },
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function createBranch(
	repoId: string,
	name: string,
	sha: string | null,
	checkout: boolean,
): Promise<boolean> {
	try {
		await ipc.createBranch(repoId, name, sha, checkout);
		toasts.push({
			message: checkout ? `Created and switched to \`${name}\`` : `Created \`${name}\``,
			tone: "success",
			icon: "branch",
		});
		return true;
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
		return false;
	}
}

export async function renameBranch(
	repoId: string,
	oldName: string,
	newName: string,
): Promise<boolean> {
	try {
		await ipc.renameBranch(repoId, oldName, newName);
		toasts.push({ message: `Renamed to \`${newName}\``, tone: "success", icon: "branch" });
		return true;
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
		return false;
	}
}

/** Delete a branch with the Undo toast (recreate at recorded sha, §15.13). Rethrows the unmerged
 * guard error so the caller can show the armed force-delete confirm; other failures toast here. */
export async function deleteBranch(repoId: string, name: string, force: boolean): Promise<void> {
	try {
		const sha = await ipc.deleteBranch(repoId, name, force);
		toasts.push({
			message: `Deleted \`${name}\``,
			tone: "danger",
			icon: "undo",
			destructive: true,
			action: {
				label: "Undo",
				run: async () => {
					try {
						await ipc.recreateBranch(repoId, name, sha);
						toasts.push({ message: `Restored \`${name}\``, tone: "success", icon: "branch" });
					} catch (e) {
						const { userMessage, raw } = asAppError(e);
						toasts.pushError(userMessage, raw);
					}
				},
			},
		});
	} catch (e) {
		if (!force && isUnmergedError(e)) throw e;
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

/** Merge `source` into the current branch (`target`). Success → "Merged … — View" (scroll to the
 * merge commit). Conflict/other failure → error toast; the repo is left in its conflicted state
 * for the graph to surface (§4.4). */
export async function mergeInto(repoId: string, source: string, target: string): Promise<void> {
	try {
		await ipc.mergeRef(repoId, source);
		toasts.push({
			message: `Merged \`${source}\` into \`${target}\``,
			tone: "success",
			icon: "merge",
			// By the time the user clicks View, the Head refresh has landed and `graph.head` is the
			// new merge commit — scroll to it (§8 "scroll to commit").
			action: {
				label: "View",
				run: () => {
					const sha = graph.head?.sha;
					if (sha) graphNav.scrollTo(sha);
				},
			},
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function rebaseOnto(repoId: string, source: string, onto: string): Promise<void> {
	try {
		await ipc.rebaseOnto(repoId, onto);
		toasts.push({
			message: `Rebased \`${source}\` onto \`${onto}\``,
			tone: "success",
			icon: "merge",
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function pull(
	repoId: string,
	mode: "ff" | "rebase" | "merge",
	branch: string,
): Promise<void> {
	try {
		await ipc.pull(repoId, mode);
		toasts.push({ message: `Pulled \`${branch}\``, tone: "success", icon: "check" });
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function push(repoId: string, force: boolean, branch: string): Promise<void> {
	try {
		await ipc.push(repoId, force);
		toasts.push({
			message: force ? `Force-pushed \`${branch}\`` : `Pushed \`${branch}\``,
			tone: "success",
			icon: "check",
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

/** Commit the current draft (DESIGN_SPEC.md §7). When `stageAllFirst` is set — the "Stage all &
 * commit" button state (§15.16) — everything is staged before committing. On success the draft is
 * cleared and, for a plain (non-amend) commit, the toast offers **Undo** (soft reset, §8/§15.13);
 * amend has no clean soft-reset undo so it's omitted. Returns true so the caller can play its
 * success sweep. */
export async function commit(
	repoId: string,
	opts: { stageAllFirst: boolean },
): Promise<boolean> {
	const { summary, description, amend } = commitDraft;
	if (!commitDraft.canCommit) return false;
	try {
		if (opts.stageAllFirst) await ipc.stageAll(repoId);
		const sha = await ipc.commit(repoId, summary, description, amend);
		const branch = graph.head && !graph.head.detached ? graph.head.branch : null;
		commitDraft.reset();
		toasts.push({
			message: branch
				? `Committed \`${sha.slice(0, 7)}\` to \`${branch}\``
				: `Committed \`${sha.slice(0, 7)}\``,
			tone: "success",
			icon: "check",
			action: amend ? undefined : { label: "Undo", run: () => undoCommit(repoId) },
		});
		return true;
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
		return false;
	}
}

/** The composer/WIP-row "primary" commit decision (§15.16): with staged files, commit them as-is;
 * with only unstaged WIP, stage everything first. Used by the WIP row's Cmd+Enter and the
 * composer's split-button default. */
export async function commitPrimary(repoId: string): Promise<boolean> {
	const hasStaged = stagedRows(status.report.entries).length > 0;
	return commit(repoId, { stageAllFirst: !hasStaged });
}

/** The commit toast's **Undo** — soft-reset the last commit; its changes return to the index
 * exactly as staged (§8/§15.13). Only ever wired up before the commit is pushed. */
async function undoCommit(repoId: string): Promise<void> {
	try {
		await ipc.undoCommit(repoId);
		toasts.push({
			message: "Commit undone — changes are staged again",
			tone: "info",
			icon: "undo",
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function fastForward(
	repoId: string,
	branch: string,
	source: string,
	isCurrent: boolean,
): Promise<void> {
	try {
		await ipc.fastForward(repoId, branch, source, isCurrent);
		toasts.push({
			message: `Fast-forwarded \`${branch}\` to \`${source}\``,
			tone: "success",
			icon: "merge",
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}
