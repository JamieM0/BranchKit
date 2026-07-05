/** The Keep Panel *session* — DESIGN_SPEC.md §9. One instance coordinates the whole in-progress
 * conflict operation (merge/rebase/cherry-pick/revert/stash-apply) across every conflicted file:
 * it owns the [`ConflictState`], one [`KeepPanelStore`] per file (the per-file reducer from
 * `keepPanel.svelte.ts`), which file is open in the center, and the aggregate progress the banner
 * and file tabs render.
 *
 * Source of truth for "still unmerged" is always the backend's `state.files` (a file drops out of
 * it the instant `confirm_file` stages it, ARCHITECTURE.md §7.5). We snapshot the full file set
 * once (`allFiles`) so a confirmed file keeps its tab (● dot, reopenable) even after it leaves
 * `state.files` — DESIGN_SPEC.md §9.2. `isConfirmed(path)` = in `allFiles` but no longer unmerged. */

import type { UnlistenFn } from "@tauri-apps/api/event";
import {
	getConflictState,
	getConflictRegions,
	confirmFile,
	reopenFile,
	continueConflict,
	abortConflict,
	onRepoChanged,
} from "$lib/ipc";
import type { ChangeKind, ConflictState, FileConflictRegions, Side } from "$lib/types";
import { KeepPanelStore, operationProgress } from "./keepPanel.svelte";

export interface KeepSessionDeps {
	getConflictState(repoId: string): Promise<ConflictState | null>;
	getConflictRegions(repoId: string, path: string): Promise<FileConflictRegions>;
	confirmFile(repoId: string, path: string, resolvedText: string): Promise<void>;
	reopenFile(repoId: string, path: string): Promise<void>;
	continueConflict(repoId: string, message?: string): Promise<void>;
	abortConflict(repoId: string): Promise<void>;
	onRepoChanged(repoId: string, handler: (kind: ChangeKind) => void): Promise<UnlistenFn>;
}

const defaultDeps: KeepSessionDeps = {
	getConflictState,
	getConflictRegions,
	confirmFile,
	reopenFile,
	continueConflict,
	abortConflict,
	onRepoChanged,
};

/** The verbs each conflict kind uses in the banner — DESIGN_SPEC.md §9.1, and §9.3's "rebase-
 * appropriate banner verbs" (the whole point of wiring all four kinds through one banner). */
export const CONFLICT_VERB: Record<ConflictState["kind"], string> = {
	merge: "Merging",
	rebase: "Rebasing",
	cherryPick: "Cherry-picking",
	revert: "Reverting",
	stashApply: "Applying stash",
};

/** The word the banner's Continue button uses per kind — "Continue merge" / "Continue rebase" …
 * (DESIGN_SPEC.md §9.2/§9.3). Stash-apply has no continue of its own (conflict.rs) so it reads
 * "Finish". */
export const CONTINUE_VERB: Record<ConflictState["kind"], string> = {
	merge: "Continue merge",
	rebase: "Continue rebase",
	cherryPick: "Continue cherry-pick",
	revert: "Continue revert",
	stashApply: "Finish",
};

/** The verb for the abort confirm sentence — "Abort merge and return …" (DESIGN_SPEC.md §9.3). */
export const ABORT_VERB: Record<ConflictState["kind"], string> = {
	merge: "Abort merge",
	rebase: "Abort rebase",
	cherryPick: "Abort cherry-pick",
	revert: "Abort revert",
	stashApply: "Discard stash apply",
};

export interface FileEntry {
	path: string;
	store: KeepPanelStore;
	regions: FileConflictRegions;
}

export class KeepSession {
	repoId: string | null = $state(null);
	state: ConflictState | null = $state(null);
	/** The full conflicted-file set, snapshotted when the operation begins and only grown, never
	 * shrunk while the operation runs — a confirmed file stays here (and keeps its tab) even after
	 * git no longer reports it as unmerged. */
	allFiles: string[] = $state([]);
	/** One reducer per file, keyed by path. Loaded eagerly on entry so the banner can show an exact
	 * total conflict count ("2 of 5 conflicts") before any file is opened. */
	entries: Record<string, FileEntry> = $state({});
	/** The file open in the center Keep Panel; `null` ⇒ the panel isn't showing (graph/diff is). */
	activePath: string | null = $state(null);

	#deps: KeepSessionDeps;
	#unlisten: UnlistenFn | null = null;
	#advanceTimer: ReturnType<typeof setTimeout> | undefined;

	constructor(deps: Partial<KeepSessionDeps> = {}) {
		this.#deps = { ...defaultDeps, ...deps };
	}

	/** True while any conflict operation is in progress — drives the banner's existence. */
	get conflictActive(): boolean {
		return this.state !== null;
	}

	/** True when the Keep Panel should replace the graph/diff in the center. */
	get panelOpen(): boolean {
		return this.state !== null && this.activePath !== null;
	}

	/** A file is confirmed (staged, DESIGN_SPEC.md §9.2's ● dot) once git no longer lists it as
	 * unmerged, yet we still remember it from `allFiles`. */
	isConfirmed(path: string): boolean {
		return (
			this.state !== null && this.allFiles.includes(path) && !this.state.files.includes(path)
		);
	}

	entryFor(path: string): FileEntry | undefined {
		return this.entries[path];
	}

	async open(repoId: string): Promise<void> {
		if (this.repoId === repoId) return;
		await this.close();
		this.repoId = repoId;
		this.#unlisten = await this.#deps.onRepoChanged(repoId, (kind) => {
			if (kind.kind === "workingTree" || kind.kind === "index" || kind.kind === "head") {
				void this.refresh();
			}
		});
		await this.refresh();
	}

	async close(): Promise<void> {
		clearTimeout(this.#advanceTimer);
		if (this.#unlisten) {
			await this.#unlisten();
			this.#unlisten = null;
		}
		this.repoId = null;
		this.state = null;
		this.allFiles = [];
		this.entries = {};
		this.activePath = null;
	}

	/** Re-reads the conflict state and reconciles the per-file stores. New unmerged files get a
	 * freshly-loaded store; a store that already exists is left alone (its fluid keep/unkeep state
	 * must survive a status refresh). When the state goes `null` the operation finished — tear the
	 * whole session down so the banner and panel disappear. */
	async refresh(): Promise<void> {
		if (!this.repoId) return;
		const next = await this.#deps.getConflictState(this.repoId);
		if (!next) {
			this.state = null;
			this.allFiles = [];
			this.entries = {};
			this.activePath = null;
			return;
		}

		const wasActive = this.state !== null;
		this.state = next;

		if (!wasActive) {
			// Fresh conflict — snapshot the file set and drop any stale stores.
			this.allFiles = [...next.files];
			this.entries = {};
		} else {
			// Ongoing — a genuinely new conflicted path can only be added, never subtracted (a
			// subtraction means "confirmed", which we track via `isConfirmed`, not by forgetting).
			for (const f of next.files) {
				if (!this.allFiles.includes(f)) this.allFiles = [...this.allFiles, f];
			}
		}

		for (const f of next.files) {
			if (!this.entries[f]) await this.#loadEntry(f);
		}
	}

	async #loadEntry(path: string): Promise<void> {
		if (!this.repoId || !this.state) return;
		const regions = await this.#deps.getConflictRegions(this.repoId, path);
		const store = new KeepPanelStore();
		// ours = the branch we're on / rebasing onto (stage :2:, `target_label`); theirs = the
		// incoming side (stage :3:, `source_label`) — DESIGN_SPEC.md §9.2's blue "yours" / purple
		// "incoming". conflict.rs already resolves both to real names for every conflict kind.
		store.open(path, regions.regions, this.state.targetLabel, this.state.sourceLabel);
		this.entries = { ...this.entries, [path]: { path, store, regions } };
	}

	/** Open a file in the center panel (a click on its Conflicted-section row or file tab). */
	openFile(path: string) {
		if (this.entries[path]) this.activePath = path;
	}

	// --- per-file confirm / reset (DESIGN_SPEC.md §9.2) -----------------------------------------

	/** Confirm the active file: write its assembled text, `git add`, then auto-advance to the next
	 * still-unresolved file after a 400ms beat (§9.1 "never feels yanked"). */
	async confirmActive(): Promise<void> {
		const path = this.activePath;
		if (!path || !this.repoId) return;
		const entry = this.entries[path];
		if (!entry || !entry.store.allResolved) return;
		await this.#deps.confirmFile(this.repoId, path, entry.store.resolvedText);
		await this.refresh();
		clearTimeout(this.#advanceTimer);
		this.#advanceTimer = setTimeout(() => {
			const nextFile = this.allFiles.find((f) => !this.isConfirmed(f));
			if (nextFile && nextFile !== path) this.activePath = nextFile;
		}, 400);
	}

	/** "Reset file" (§9.2). A confirmed file is reopened at the git level (`checkout -m`, which
	 * regenerates the conflict) and its store reloaded from scratch; an un-confirmed file just has
	 * its reducer reset back to all-candidates. */
	async resetActiveFile(): Promise<void> {
		const path = this.activePath;
		if (!path || !this.repoId) return;
		if (this.isConfirmed(path)) {
			await this.#deps.reopenFile(this.repoId, path);
			await this.refresh();
			await this.#loadEntry(path);
		} else {
			this.entries[path]?.store.resetFile();
		}
	}

	// --- global bulk actions (banner ⋯ menu, DESIGN_SPEC.md §9.2) --------------------------------

	/** "Keep all from `<side>`" across every not-yet-confirmed file. */
	keepAllGlobally(source: Side) {
		for (const path of this.allFiles) {
			if (!this.isConfirmed(path)) this.entries[path]?.store.keepAllFrom(source);
		}
	}

	// --- finish / abort (DESIGN_SPEC.md §9.1/§9.2) ----------------------------------------------

	/** Continue enabled only when nothing is still unmerged (every file confirmed) — the banner's
	 * disabled-Continue state (§9.1/§15.21). */
	get continueEnabled(): boolean {
		return this.state !== null && this.state.files.length === 0;
	}

	async continue(message?: string): Promise<void> {
		if (!this.repoId) return;
		await this.#deps.continueConflict(this.repoId, message);
		await this.refresh();
	}

	async abort(): Promise<void> {
		if (!this.repoId) return;
		await this.#deps.abortConflict(this.repoId);
		await this.refresh();
	}

	// --- aggregate progress (banner text + file tabs, DESIGN_SPEC.md §9.1/§15.21) ---------------

	get progress(): {
		filesDone: number;
		filesTotal: number;
		regionsResolved: number;
		regionsTotal: number;
	} {
		const perFile: Record<string, { resolved: number; total: number }> = {};
		for (const path of this.allFiles) {
			const entry = this.entries[path];
			const fp = entry ? entry.store.fileProgress : { resolved: 0, total: 0 };
			// A confirmed file is fully resolved by definition (its choices are on disk & staged).
			perFile[path] = this.isConfirmed(path) ? { resolved: fp.total, total: fp.total } : fp;
		}
		const agg = operationProgress(perFile);
		return {
			// "files done" means *staged*, not merely "all regions touched in the panel" — a file
			// you've decided but not yet Confirmed is not done (its result isn't on disk).
			filesDone: this.allFiles.filter((f) => this.isConfirmed(f)).length,
			filesTotal: this.allFiles.length,
			regionsResolved: agg.regionsResolved,
			regionsTotal: agg.regionsTotal,
		};
	}

	/** The precise "what's left" the disabled-Continue tooltip must state (DESIGN_SPEC.md §15.21). */
	get remainingSummary(): string {
		const unresolvedFiles = this.allFiles.filter((f) => !this.isConfirmed(f));
		if (unresolvedFiles.length === 0) return "";
		const parts = unresolvedFiles.map((f) => {
			const entry = this.entries[f];
			const fp = entry ? entry.store.fileProgress : { resolved: 0, total: 0 };
			const left = fp.total - fp.resolved;
			if (left > 0) {
				return `${left} conflict${left === 1 ? "" : "s"} left in ${f}`;
			}
			return `${f} is resolved but not yet confirmed`;
		});
		return parts.join("; ");
	}
}

export const keepSession = new KeepSession();
