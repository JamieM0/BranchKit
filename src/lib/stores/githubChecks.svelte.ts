/** CI dot cache — DESIGN_SPEC.md §12/§15.23, ARCHITECTURE.md §11: "lazy: only visible commits,
 * cache 60s, batch on scroll-idle... never poll checks more than 1/min/repo." `GraphRow` only
 * exists in the DOM for the virtualized visible range, so a row requesting its own status here
 * already satisfies "visible commits only"; the 60s per-sha cache plus in-flight de-dup covers the
 * rest without a heavier scheduler. */

import * as ipc from "$lib/ipc";
import { github } from "./github.svelte";
import type { CommitCheckStatus } from "$lib/types";

const TTL_MS = 60_000;

interface Entry {
	status: CommitCheckStatus;
	fetchedAt: number;
}

class GithubChecksStore {
	#cache = $state<Record<string, Entry>>({});
	#inflight = new Set<string>();

	get(sha: string): CommitCheckStatus | null {
		return this.#cache[sha]?.status ?? null;
	}

	/** Fire-and-forget: fetches (or refreshes if stale) the check status for `sha`, a no-op when
	 * GitHub isn't connected, already fresh, or already in flight. */
	async request(repoId: string, sha: string) {
		if (!github.connected) return;
		const cached = this.#cache[sha];
		if (cached && Date.now() - cached.fetchedAt < TTL_MS) return;
		if (this.#inflight.has(sha)) return;
		this.#inflight.add(sha);
		try {
			const status = await ipc.getCheckStatus(repoId, sha);
			this.#cache = { ...this.#cache, [sha]: { status, fetchedAt: Date.now() } };
		} catch {
			// Not on GitHub, no checks configured, or rate-limited — the dot simply doesn't render.
		} finally {
			this.#inflight.delete(sha);
		}
	}

	reset() {
		this.#cache = {};
		this.#inflight.clear();
	}
}

export const githubChecks = new GithubChecksStore();
