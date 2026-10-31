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
	error: unknown = $state(null);

	#deps: StatusStoreDeps;
	#unlisten: UnlistenFn | null = null;
	#session = 0;
	#request = 0;

	constructor(deps: Partial<StatusStoreDeps> = {}) {
		this.#deps = { ...defaultDeps, ...deps };
	}

	async open(repoId: string): Promise<void> {
		void this.close();
		const session = this.#session;
		this.repoId = repoId;
		this.loading = true;
		const unlisten = await this.#deps.onRepoChanged(repoId, (kind) => {
			if (session !== this.#session) return;
			if (["workingTree", "index", "head", "refs", "remote"].includes(kind.kind)) {
				void this.refresh();
			}
		});
		if (session !== this.#session) { unlisten(); return; }
		this.#unlisten = unlisten;
		await this.refresh();
	}

	async close(): Promise<void> {
		this.#session += 1;
		this.#request += 1;
		this.#unlisten?.();
		this.#unlisten = null;
		this.repoId = null;
		this.report = EMPTY_REPORT;
		this.loading = false;
		this.error = null;
	}

	async refresh(): Promise<void> {
		const repoId = this.repoId;
		if (!repoId) return;
		const request = ++this.#request;
		try {
			const report = await this.#deps.getStatus(repoId);
			if (request !== this.#request) return;
			this.report = report;
			this.error = null;
		} catch (error) {
			if (request === this.#request) this.error = error;
		} finally {
			if (request === this.#request) this.loading = false;
		}
	}

}

export const status = new StatusStore();
