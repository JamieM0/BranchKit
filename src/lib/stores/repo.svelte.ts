import type { UnlistenFn } from "@tauri-apps/api/event";
import { cloneRepo, closeRepo, onCloneProgress, onRepoChanged, openRepo } from "$lib/ipc";
import type { ChangeKind, RepoInfo } from "$lib/types";

/** Fine-grained invalidation flags a repo's tab accumulates from `repo://{id}/changed` events —
 * ARCHITECTURE.md §2. Later prompts' stores (status/refs/graph) read and clear these instead of
 * blindly refetching everything on every change. */
export interface Invalidation {
	status: boolean;
	refs: boolean;
	graph: boolean;
}

function freshInvalidation(): Invalidation {
	return { status: true, refs: true, graph: true };
}

interface PersistedSession {
	paths: string[];
	activePath: string | null;
}

const SESSION_STORAGE_KEY = "branchkit:repo-session";

function loadSession(): PersistedSession {
	if (typeof localStorage === "undefined") return { paths: [], activePath: null };
	try {
		const parsed = JSON.parse(localStorage.getItem(SESSION_STORAGE_KEY) ?? "null") as Partial<PersistedSession> | null;
		return {
			paths: Array.isArray(parsed?.paths) ? parsed.paths.filter((path): path is string => typeof path === "string") : [],
			activePath: typeof parsed?.activePath === "string" ? parsed.activePath : null,
		};
	} catch {
		return { paths: [], activePath: null };
	}
}

export interface RepoTab {
	/** A real backend repo id once open, or `pending:{requestId}` while a clone is in flight. */
	id: string;
	path: string;
	name: string;
	branch: string | null;
	detached: boolean;
	/** True while any long-running operation (currently: cloning) is active for this tab —
	 * DESIGN_SPEC.md §3.1's tab spinner dot. */
	busy: boolean;
	cloneProgress: { phase: string; percent: number | null } | null;
	invalidate: Invalidation;
}

export function nameFromPath(path: string): string {
	const parts = path.split(/[\\/]/).filter(Boolean);
	return parts.at(-1) ?? path;
}

export function applyChangeKind(invalidate: Invalidation, kind: ChangeKind): Invalidation {
	switch (kind.kind) {
		case "workingTree":
			return { ...invalidate, status: true };
		case "index":
			return { ...invalidate, status: true };
		case "refs":
			return { ...invalidate, refs: true, graph: true };
		case "head":
			return { ...invalidate, status: true, refs: true, graph: true };
		case "remote":
			return { ...invalidate, refs: true };
		case "operationProgress":
			return invalidate;
	}
}

class RepoStore {
	tabs: RepoTab[] = $state([]);
	activeId: string | null = $state(null);
	restoringSession = $state(false);
	restoreAttempted = $state(false);

	active: RepoTab | null = $derived(this.tabs.find((t) => t.id === this.activeId) ?? null);

	#unlisten = new Map<string, UnlistenFn>();
	#suspendSessionPersistence = false;

	#persistSession() {
		if (this.#suspendSessionPersistence || typeof localStorage === "undefined") return;
		const realTabs = this.tabs.filter((tab) => !tab.id.startsWith("pending:"));
		const activePath = realTabs.find((tab) => tab.id === this.activeId)?.path ?? null;
		const session: PersistedSession = { paths: realTabs.map((tab) => tab.path), activePath };
		localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session));
	}

	#tabIndex(id: string): number {
		return this.tabs.findIndex((t) => t.id === id);
	}

	async #subscribe(id: string) {
		const unlisten = await onRepoChanged(id, (kind) => {
			const i = this.#tabIndex(id);
			if (i === -1) return;
			this.tabs[i].invalidate = applyChangeKind(this.tabs[i].invalidate, kind);
		});
		this.#unlisten.set(id, unlisten);
	}

	#tabFromInfo(info: RepoInfo): RepoTab {
		return {
			id: info.id,
			path: info.path,
			name: info.name,
			branch: info.branch,
			detached: info.detached,
			busy: false,
			cloneProgress: null,
			invalidate: freshInvalidation(),
		};
	}

	/** Opens `path` as a new tab, or switches to it if it's already open. */
	async open(path: string): Promise<RepoTab> {
		const info = await openRepo(path);
		const existing = this.#tabIndex(info.id);
		if (existing !== -1) {
			this.activeId = info.id;
			this.#persistSession();
			return this.tabs[existing];
		}
		const tab = this.#tabFromInfo(info);
		this.tabs.push(tab);
		this.activeId = tab.id;
		await this.#subscribe(tab.id);
		this.#persistSession();
		return tab;
	}

	/** Clones `url` into `destination` as a pending tab that carries live progress, then swaps
	 * it for the real tab once the clone finishes — DESIGN_SPEC.md §3.1/§11. */
	async clone(url: string, destination: string): Promise<RepoTab> {
		const requestId = crypto.randomUUID();
		const pendingId = `pending:${requestId}`;
		const pending: RepoTab = {
			id: pendingId,
			path: destination,
			name: nameFromPath(destination),
			branch: null,
			detached: false,
			busy: true,
			cloneProgress: { phase: "Starting…", percent: null },
			invalidate: freshInvalidation(),
		};
		this.tabs.push(pending);
		this.activeId = pendingId;

		const unlisten = await onCloneProgress(requestId, (phase, percent) => {
			const i = this.#tabIndex(pendingId);
			if (i === -1) return;
			this.tabs[i].cloneProgress = { phase, percent };
		});

		try {
			const info = await cloneRepo(requestId, url, destination);
			const i = this.#tabIndex(pendingId);
			const tab = this.#tabFromInfo(info);
			if (i !== -1) {
				this.tabs[i] = tab;
			} else {
				this.tabs.push(tab);
			}
			if (this.activeId === pendingId) this.activeId = tab.id;
			await this.#subscribe(tab.id);
			this.#persistSession();
			return tab;
		} catch (e) {
			this.tabs = this.tabs.filter((t) => t.id !== pendingId);
			if (this.activeId === pendingId) this.activeId = this.tabs.at(-1)?.id ?? null;
			this.#persistSession();
			throw e;
		} finally {
			await unlisten();
		}
	}

	async close(id: string): Promise<void> {
		const i = this.#tabIndex(id);
		if (i === -1) return;

		if (!id.startsWith("pending:")) {
			await closeRepo(id);
		}
		this.#unlisten.get(id)?.();
		this.#unlisten.delete(id);

		this.tabs.splice(i, 1);
		if (this.activeId === id) {
			const next = this.tabs[i] ?? this.tabs[i - 1];
			this.activeId = next?.id ?? null;
		}
		this.#persistSession();
	}

	/** Toggles the tab's spinner overlay for a long-running op in that repo — DESIGN_SPEC.md §3.1
	 * ("A tab shows a subtle spinner overlay on its favicon-dot while a long operation runs"). Used
	 * around fetch/pull/push/publish, which otherwise have no other tab-level "in flight" signal. */
	setBusy(id: string, busy: boolean) {
		const i = this.#tabIndex(id);
		if (i === -1) return;
		this.tabs[i].busy = busy;
	}

	switchTo(id: string) {
		if (this.#tabIndex(id) !== -1) {
			this.activeId = id;
			this.#persistSession();
		}
	}

	/** Cmd+1…9 — DESIGN_SPEC.md §3.1. `n` is 1-indexed. */
	switchToIndex(n: number) {
		const tab = this.tabs[n - 1];
		if (tab) {
			this.activeId = tab.id;
			this.#persistSession();
		}
	}

	switchToNextOrPrevious(direction: 1 | -1) {
		if (this.tabs.length === 0) return;
		const i = Math.max(this.#tabIndex(this.activeId ?? ""), 0);
		const next = (i + direction + this.tabs.length) % this.tabs.length;
		this.activeId = this.tabs[next].id;
		this.#persistSession();
	}

	reorder(fromIndex: number, toIndex: number) {
		if (
			fromIndex === toIndex ||
			fromIndex < 0 ||
			toIndex < 0 ||
			fromIndex >= this.tabs.length ||
			toIndex >= this.tabs.length
		) {
			return;
		}
		const tabs = this.tabs.slice();
		const [moved] = tabs.splice(fromIndex, 1);
		tabs.splice(toIndex, 0, moved);
		this.tabs = tabs;
		this.#persistSession();
	}

	/** Reopens the last session in saved tab order, including linked-worktree paths. Missing or
	 * invalid repositories are skipped without preventing the remaining tabs from opening. */
	async restoreLastSession(): Promise<string[]> {
		if (this.restoreAttempted) return [];
		this.restoreAttempted = true;
		this.restoringSession = true;
		const saved = loadSession();
		const failures: string[] = [];
		this.#suspendSessionPersistence = true;
		try {
			for (const path of saved.paths) {
				try {
					await this.open(path);
				} catch {
					failures.push(path);
				}
			}
			const active = this.tabs.find((tab) => tab.path === saved.activePath);
			if (active) this.activeId = active.id;
		} finally {
			this.#suspendSessionPersistence = false;
			this.restoringSession = false;
			this.#persistSession();
		}
		return failures;
	}

	/** Consumers (status/refs/graph stores, added in later prompts) call this once they've
	 * re-queried for the flag(s) they care about. */
	clearInvalidation(id: string, keys: (keyof Invalidation)[]) {
		const i = this.#tabIndex(id);
		if (i === -1) return;
		const invalidate = { ...this.tabs[i].invalidate };
		for (const key of keys) invalidate[key] = false;
		this.tabs[i].invalidate = invalidate;
	}
}

export const repos = new RepoStore();
