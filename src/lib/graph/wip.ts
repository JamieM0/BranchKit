/** The WIP row model — DESIGN_SPEC.md §4.2. A synthetic top-of-graph row that appears whenever the
 * working tree differs from HEAD: a dashed hollow node in the checked-out branch's lane, hanging
 * off HEAD by a dashed connector, with `✎ − ＋` change badges. It's assembled in the view (not the
 * graph store) so the store stays a pure projection of the real topology; the counts come straight
 * from the status feed. */

import type { GraphSegment } from "./lanes";
import { laneColorIndex } from "./lanes";
import type { StatusEntry } from "$lib/types";

/** Stable synthetic sha for the WIP row — never collides with a real 40-hex commit sha. */
export const WIP_SHA = "__wip__";

export interface WipCounts {
	added: number;
	modified: number;
	deleted: number;
}

export interface WipRow {
	kind: "wip";
	sha: typeof WIP_SHA;
	node: { lane: number; colorIndex: number };
	segments: GraphSegment[];
	counts: WipCounts;
}

/** Roll the working-tree changes up into the three badge buckets (§2.4/§4.2). Each file counts
 * once, preferring its worktree (unstaged) status, falling back to the index (staged) status;
 * untracked and added/copied files are `＋`, deletions are `−`, everything else is `✎`. */
export function wipCounts(entries: readonly StatusEntry[]): WipCounts {
	let added = 0;
	let modified = 0;
	let deleted = 0;
	for (const entry of entries) {
		const code =
			entry.worktreeStatus !== "unmodified" ? entry.worktreeStatus : entry.indexStatus;
		if (entry.kind === "untracked" || code === "added" || code === "copied") added += 1;
		else if (code === "deleted") deleted += 1;
		else modified += 1;
	}
	return { added, modified, deleted };
}

/** Build the WIP row hanging off HEAD's lane. The single dashed segment runs from the node down to
 * the row's bottom boundary so it visually connects into the branch below. */
export function buildWipRow(headLane: number, entries: readonly StatusEntry[]): WipRow {
	const colorIndex = laneColorIndex(headLane);
	return {
		kind: "wip",
		sha: WIP_SHA,
		node: { lane: headLane, colorIndex },
		segments: [
			{ from: { at: "node" }, to: { at: "bottom", lane: headLane }, colorIndex, dashed: true },
		],
		counts: wipCounts(entries),
	};
}
