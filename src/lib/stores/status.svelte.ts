/** Working-tree/index status feed for the right panel's working-directory mode —
 * DESIGN_SPEC.md §6.1. Refetches on `workingTree`/`index`/`head` refresh events (a checkout
 * changes both the file list and the header's branch name). */

import type { UnlistenFn } from "@tauri-apps/api/event";
import { getStatus, onRepoChanged } from "$lib/ipc";
import type { ChangeKind, StatusReport } from "$lib/types";

export interface StatusStoreDeps {
	getStatus(repoId: string): Promise<StatusReport>;
	onRepoChanged(repoId: string, handler: (kind: ChangeKind) => void): Promise<UnlistenFn>;
}

const defaultDeps: StatusStoreDeps = { getStatus, onRepoChanged };

const EMPTY_REPORT: StatusReport = {
	branch: { oid: null, head: null, upstream: null, ahead: 0, behind: 0 },
	entries: [],
};

export class StatusStore {
	repoId: string | null = $state(null);
	report: StatusReport = $state(EMPTY_REPORT);
	loading = $state(false);

	#deps: StatusStoreDeps;
	#unlisten: UnlistenFn | null = null;

	constructor(deps: Partial<StatusStoreDeps> = {}) {
		this.#deps = { ...defaultDeps, ...deps };
	}

	async open(repoId: string): Promise<void> {
		await this.close();
		this.repoId = repoId;
		this.loading = true;
		this.#unlisten = await this.#deps.onRepoChanged(repoId, (kind) => {
			if (kind.kind === "workingTree" || kind.kind === "index" || kind.kind === "head") {
				void this.refresh();
			}
		});
		try {
			await this.refresh();
		} finally {
			this.loading = false;
		}
	}

	async close(): Promise<void> {
		if (this.#unlisten) {
			await this.#unlisten();
			this.#unlisten = null;
		}
		this.repoId = null;
		this.report = EMPTY_REPORT;
	}

	async refresh(): Promise<void> {
		if (!this.repoId) return;
		this.report = await this.#deps.getStatus(this.repoId);
	}
}

export const status = new StatusStore();
