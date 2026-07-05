/** Git action orchestration — the layer between UI gestures (pills, menus, drop menu, left panel)
 * and the typed IPC mutations. Each action calls `ipc`, then wires the DESIGN_SPEC §8 toast with
 * its single undo/redo verb, and routes failures through the shared error toast (§11). Kept out of
 * `ipc.ts` (which is only `invoke()` wrappers) and out of components (so the toast catalog lives in
 * one place). */

import * as ipc from "$lib/ipc";
import { toasts, type ToastAction } from "$lib/stores/toasts.svelte";
import { graphNav } from "$lib/stores/graphNav.svelte";
import { graph } from "$lib/stores/graph.svelte";
import { status } from "$lib/stores/status.svelte";
import { commitDraft } from "$lib/stores/commitDraft.svelte";
import { stagedRows } from "$lib/status/sections";
import { repos } from "$lib/stores/repo.svelte";
import { network } from "$lib/stores/network.svelte";

interface AppErrorShape {
	userMessage: string;
	raw: string;
	suggestion: { label: string; actionId: string } | null;
}

function asAppError(e: unknown): AppErrorShape {
	if (e && typeof e === "object" && "userMessage" in e) {
		const o = e as Record<string, unknown>;
		const s = o.suggestion as Record<string, unknown> | null | undefined;
		return {
			userMessage: String(o.userMessage),
			raw: String(o.raw ?? ""),
			suggestion: s ? { label: String(s.label), actionId: String(s.actionId) } : null,
		};
	}
	return { userMessage: e instanceof Error ? e.message : String(e), raw: String(e), suggestion: null };
}

/** Context a caller supplies so a translated error's suggestion (ARCHITECTURE.md §9) can be turned
 * into the toast's one action verb — only the fields relevant to the failing op need be set. */
interface ErrorContext {
	repoId: string;
	/** The branch to act on for the "Pull first" / "Publish" suggestions. */
	branch?: string;
	/** The branch being checked out — for the "would be overwritten" stash-and-checkout retry. */
	checkoutTarget?: string;
	/** Re-runs the operation that failed — for "index.lock" / offline "Retry". */
	retry?: () => void | Promise<void>;
}

/** Turns a translated error's suggestion into a toast action, or `undefined` if this call site
 * doesn't have the context the suggestion needs (in which case the plain message still shows). */
function suggestionAction(err: AppErrorShape, ctx: ErrorContext): ToastAction | undefined {
	const suggestion = err.suggestion;
	if (!suggestion) return undefined;
	const { label, actionId } = suggestion;
	switch (actionId) {
		case "retry":
			return ctx.retry ? { label, run: ctx.retry } : undefined;
		case "retry-offline":
			network.markOffline();
			return ctx.retry ? { label, run: ctx.retry } : undefined;
		case "pull":
			return ctx.branch ? { label, run: () => pull(ctx.repoId, "ff", ctx.branch!) } : undefined;
		case "publish":
			return ctx.branch ? { label, run: () => publish(ctx.repoId, ctx.branch!) } : undefined;
		case "stash-and-checkout":
			return ctx.checkoutTarget
				? { label, run: () => stashAndCheckout(ctx.repoId, ctx.checkoutTarget!) }
				: undefined;
		case "open-credentials-settings":
			// SPEC-DEVIATION: the dynamic Settings window (DESIGN_SPEC.md §13) hasn't landed yet —
			// surface the guidance as a toast instead of a dead link.
			return {
				label,
				run: () => {
					toasts.push({
						message: "Open Settings → Credentials to fix this",
						tone: "info",
						icon: "alert",
					});
				},
			};
		default:
			return undefined;
	}
}

/** Reports a failed operation as an error toast, attaching the suggestion's action verb when `ctx`
 * has what that suggestion needs (§8/§11). */
function reportError(e: unknown, ctx: ErrorContext): void {
	const err = asAppError(e);
	toasts.pushError(err.userMessage, err.raw, suggestionAction(err, ctx));
}

/** Stash uncommitted changes, checkout `name`, then pop — the "would be overwritten" suggestion's
 * compound action (ARCHITECTURE.md §9). */
async function stashAndCheckout(repoId: string, name: string): Promise<void> {
	try {
		await ipc.checkoutStashAndSwitch(repoId, name);
		toasts.push({
			message: `Switched to \`${name}\` — your changes came with you`,
			tone: "success",
			icon: "check",
		});
	} catch (e) {
		reportError(e, { repoId });
	}
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
		reportError(e, { repoId, checkoutTarget: name });
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
		reportError(e, { repoId });
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
export async function mergeInto(
	repoId: string,
	source: string,
	target: string,
	allowUnrelated = false,
): Promise<void> {
	try {
		await ipc.mergeRef(repoId, source, allowUnrelated);
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
		const err = asAppError(e);
		const action =
			err.suggestion?.actionId === "allow-unrelated"
				? { label: err.suggestion.label, run: () => mergeInto(repoId, source, target, true) }
				: suggestionAction(err, { repoId });
		toasts.pushError(err.userMessage, err.raw, action);
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
	repos.setBusy(repoId, true);
	try {
		await ipc.pull(repoId, mode);
		network.markOnline();
		toasts.push({ message: `Pulled \`${branch}\``, tone: "success", icon: "check" });
	} catch (e) {
		reportError(e, { repoId, branch, retry: () => pull(repoId, mode, branch) });
	} finally {
		repos.setBusy(repoId, false);
	}
}

export async function push(repoId: string, force: boolean, branch: string): Promise<void> {
	repos.setBusy(repoId, true);
	try {
		await ipc.push(repoId, force);
		network.markOnline();
		toasts.push({
			message: force ? `Force-pushed \`${branch}\`` : `Pushed \`${branch}\``,
			tone: "success",
			icon: "check",
		});
	} catch (e) {
		reportError(e, { repoId, branch, retry: () => push(repoId, force, branch) });
	} finally {
		repos.setBusy(repoId, false);
	}
}

/** Push a branch with no upstream yet, setting `origin/<name>` as its tracking ref — the
 * toolbar's Push-becomes-**Publish** state (DESIGN_SPEC.md §3.2). Success offers **Create pull
 * request** only once GitHub integration lands (§8/§22) — omitted here (SPEC-DEVIATION: no
 * GitHub integration yet in this build). */
export async function publish(repoId: string, name: string): Promise<void> {
	repos.setBusy(repoId, true);
	try {
		await ipc.publish(repoId, name);
		network.markOnline();
		toasts.push({ message: `Published \`${name}\``, tone: "success", icon: "check" });
	} catch (e) {
		reportError(e, { repoId, branch: name, retry: () => publish(repoId, name) });
	} finally {
		repos.setBusy(repoId, false);
	}
}

/** Fetch every remote — the toolbar Pull dropdown's "Fetch all" (DESIGN_SPEC.md §3.2). Quiet on
 * success (refs/badges refresh from the emitted change event); only failures surface a toast. */
export async function fetchAll(repoId: string): Promise<void> {
	repos.setBusy(repoId, true);
	try {
		await ipc.fetchAll(repoId);
		network.markOnline();
	} catch (e) {
		reportError(e, { repoId, retry: () => fetchAll(repoId) });
	} finally {
		repos.setBusy(repoId, false);
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

// --- stash (DESIGN_SPEC.md §3.2/§4.5/§15.18) ---

/** Stash all WIP — the toolbar Stash button and its dropdown variants. */
export async function stashPush(
	repoId: string,
	opts: { message?: string; includeUntracked?: boolean } = {},
): Promise<void> {
	try {
		await ipc.stashPush(repoId, opts.message ?? null, opts.includeUntracked ?? false);
		toasts.push({ message: "Stashed changes", tone: "success", icon: "check" });
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

/** Pop a stash (double-click a stash row, or the toolbar Pop button — always `stash@{0}`) with a
 * best-effort re-stash **Undo** (DESIGN_SPEC.md §4.5/§8/§15.18): the popped changes are now plain
 * uncommitted changes, so Undo just stashes the working tree again under the same message. */
export async function popStash(repoId: string, selector: string, message: string): Promise<void> {
	try {
		await ipc.stashPop(repoId, selector);
		toasts.push({
			message: "Popped stash",
			tone: "success",
			icon: "undo",
			action: {
				label: "Undo",
				run: () => stashPush(repoId, { message, includeUntracked: true }),
			},
		});
	} catch (e) {
		reportError(e, { repoId });
	}
}

export async function applyStash(repoId: string, selector: string): Promise<void> {
	try {
		await ipc.stashApply(repoId, selector);
		toasts.push({ message: "Applied stash", tone: "success", icon: "check" });
	} catch (e) {
		reportError(e, { repoId });
	}
}

export async function dropStash(repoId: string, selector: string): Promise<void> {
	try {
		await ipc.stashDrop(repoId, selector);
		toasts.push({ message: "Dropped stash", tone: "warn", icon: "trash" });
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function copyStashPatch(repoId: string, selector: string): Promise<void> {
	try {
		const patch = await ipc.getStashPatch(repoId, selector);
		await navigator.clipboard?.writeText(patch);
		toasts.push({ message: "Copied patch to clipboard", tone: "success", icon: "check" });
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

// --- rewriting history (GITKRAKEN_WORKFLOWS.md §2.6/§3.1) ---

export async function cherryPick(repoId: string, sha: string): Promise<void> {
	try {
		await ipc.cherryPick(repoId, sha);
		toasts.push({
			message: `Cherry-picked \`${sha.slice(0, 7)}\``,
			tone: "success",
			icon: "check",
			action: { label: "View", run: () => graphNav.scrollTo(graph.head?.sha ?? sha) },
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function revertCommit(repoId: string, sha: string): Promise<void> {
	try {
		await ipc.revertCommit(repoId, sha);
		toasts.push({
			message: `Reverted \`${sha.slice(0, 7)}\``,
			tone: "success",
			icon: "check",
			action: { label: "View", run: () => graphNav.scrollTo(graph.head?.sha ?? sha) },
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

/** Reset the current branch to `sha`. `mode: "hard"` is guarded by an arm-delayed confirm in the
 * caller (DESIGN_SPEC.md §4.6) before this ever runs. */
export async function resetTo(
	repoId: string,
	sha: string,
	mode: "soft" | "mixed" | "hard",
): Promise<void> {
	try {
		await ipc.resetTo(repoId, sha, mode);
		toasts.push({
			message: `Reset to \`${sha.slice(0, 7)}\` (${mode})`,
			tone: mode === "hard" ? "danger" : "success",
			icon: "undo",
			destructive: mode === "hard",
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function createTag(
	repoId: string,
	name: string,
	sha: string,
	message: string | null,
): Promise<boolean> {
	try {
		await ipc.createTag(repoId, name, sha, message);
		toasts.push({ message: `Created tag \`${name}\``, tone: "success", icon: "check" });
		return true;
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
		return false;
	}
}

export async function deleteTag(repoId: string, name: string): Promise<void> {
	try {
		await ipc.deleteTag(repoId, name);
		toasts.push({ message: `Deleted tag \`${name}\``, tone: "warn", icon: "trash" });
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

/** Builds a web URL for `sha` on a known host (github.com/gitlab.com), or `null` for anything else
 * — "Copy link to this commit on remote" (GITKRAKEN_WORKFLOWS.md §2.9/§3.1). */
export function commitWebUrl(remoteUrl: string, sha: string): string | null {
	const ssh = remoteUrl.match(/^git@([^:]+):(.+?)(?:\.git)?$/);
	const https = remoteUrl.match(/^https?:\/\/([^/]+)\/(.+?)(?:\.git)?\/?$/);
	const match = ssh ?? https;
	if (!match) return null;
	const [, host, ownerRepo] = match;
	if (host.includes("github.com")) return `https://${host}/${ownerRepo}/commit/${sha}`;
	if (host.includes("gitlab.com")) return `https://${host}/${ownerRepo}/-/commit/${sha}`;
	return null;
}

/** Copies a web link to `sha` on `remote`, falling back to the bare sha for unrecognized hosts. */
export async function copyCommitLink(repoId: string, remote: string, sha: string): Promise<void> {
	try {
		const url = await ipc.getRemoteUrl(repoId, remote);
		const link = commitWebUrl(url, sha) ?? sha;
		await navigator.clipboard?.writeText(link);
		toasts.push({
			message: link === sha ? "Copied SHA (unrecognized remote host)" : "Copied commit link",
			tone: "success",
			icon: "check",
		});
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function copyToClipboard(text: string, message: string): Promise<void> {
	try {
		await navigator.clipboard?.writeText(text);
		toasts.push({ message, tone: "success", icon: "check" });
	} catch {
		/* clipboard unavailable — best effort */
	}
}

export async function ignorePath(repoId: string, pattern: string): Promise<void> {
	try {
		await ipc.ignorePath(repoId, pattern);
		toasts.push({ message: `Added \`${pattern}\` to .gitignore`, tone: "success", icon: "check" });
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function createPatchFromCommit(repoId: string, sha: string): Promise<void> {
	try {
		const patch = await ipc.createPatchFromCommit(repoId, sha);
		const saved = await ipc.savePatchAs(`${sha.slice(0, 10)}.patch`, patch);
		if (saved) toasts.push({ message: "Saved patch", tone: "success", icon: "check" });
	} catch (e) {
		const { userMessage, raw } = asAppError(e);
		toasts.pushError(userMessage, raw);
	}
}

export async function createPatchFromFile(
	repoId: string,
	path: string,
	staged: boolean,
): Promise<void> {
	try {
		const patch = await ipc.createPatchFromFile(repoId, path, staged);
		const name = path.split("/").pop() ?? path;
		const saved = await ipc.savePatchAs(`${name}.patch`, patch);
		if (saved) toasts.push({ message: "Saved patch", tone: "success", icon: "check" });
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
