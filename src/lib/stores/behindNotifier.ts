/** The rate-limited "found new commits" toast — DESIGN_SPEC.md §8/§15.19: "`main` is 3 behind —
 * **Pull**", current branch only, at most once a minute. Fires only when the behind-count
 * *increases* from what was last seen (a fetch just found new commits) — not on every refs
 * refresh, which would otherwise fire on every unrelated ref change too. Auto-fetch runs every
 * minute (ARCHITECTURE.md §7.2), so the rate limit is what keeps this from nagging on every tick
 * once behind is already nonzero. */

import { toasts } from "./toasts.svelte";
import { pull } from "$lib/actions";

const RATE_LIMIT_MS = 60_000;

interface Tracked {
	behind: number;
	lastToastAt: number;
}

const tracked = new Map<string, Tracked>();

/** Call whenever the current branch's behind-count is known (after a refs refresh). */
export function notifyBehindIncrease(repoId: string, branch: string, behind: number): void {
	const prev = tracked.get(repoId);
	const now = Date.now();
	if (prev !== undefined && behind > prev.behind && now - prev.lastToastAt >= RATE_LIMIT_MS) {
		toasts.push({
			message: `\`${branch}\` is ${behind} behind`,
			tone: "info",
			icon: "download",
			action: { label: "Pull", run: () => pull(repoId, "ff", branch) },
		});
		tracked.set(repoId, { behind, lastToastAt: now });
		return;
	}
	tracked.set(repoId, { behind, lastToastAt: prev?.lastToastAt ?? 0 });
}

/** Drops tracking for a repo — call when it's closed so a later reopen doesn't compare against a
 * stale behind-count. */
export function resetBehindTracking(repoId: string): void {
	tracked.delete(repoId);
}
