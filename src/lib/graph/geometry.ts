/** Pure layout maths for the commit graph — kept separate from the Svelte component so the row/lane
 * geometry and virtualization window can be unit-tested. ARCHITECTURE.md §5.4, DESIGN_SPEC.md §4.1. */

/** Fixed row height — DESIGN_SPEC.md §4.1 ("Comfortable" density; §13's Compact 24px is a later
 * prompt). */
export const ROW_HEIGHT = 28;
/** Rows rendered above and below the viewport — ARCHITECTURE.md §5.4. */
export const OVERSCAN = 20;

/** Horizontal distance between adjacent lanes, in CSS px. */
export const LANE_WIDTH = 16;
/** Left padding inside the GRAPH column before lane 0's centre. */
export const GRAPH_PAD_X = 12;
/** Radius of a normal commit node disc / avatar. */
export const AVATAR_RADIUS = 9;
/** Radius of the plain disc drawn for merge commits — smaller per DESIGN_SPEC.md §4.3. */
export const MERGE_NODE_RADIUS = 4;
/** Radius of the square-ish stash node. */
export const STASH_NODE_RADIUS = 5;

/** Width of the trailing gutter reserved for the header's column-gear button, mirrored by an empty
 * cell on each row so the optional AUTHOR/DATE/SHA columns line up with their headers. */
export const RIGHT_GUTTER = 32;

/** Centre X of a lane within the GRAPH column (relative to the column's left edge). */
export function laneCenterX(lane: number): number {
	return GRAPH_PAD_X + lane * LANE_WIDTH;
}

/** Minimum GRAPH column width that shows `maxLane + 1` lanes plus a little breathing room. */
export function graphWidthForLanes(maxLane: number): number {
	return laneCenterX(maxLane) + GRAPH_PAD_X + AVATAR_RADIUS;
}

export interface RowWindow {
	start: number;
	end: number;
}

/** The slice of rows to render for a given scroll position — the virtualization window. `end` is
 * exclusive. Clamped to `[0, rowCount]`. */
export function visibleRowRange(
	scrollTop: number,
	viewportHeight: number,
	rowCount: number,
	overscan: number = OVERSCAN,
): RowWindow {
	if (rowCount === 0) return { start: 0, end: 0 };
	const first = Math.floor(scrollTop / ROW_HEIGHT) - overscan;
	const last = Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + overscan;
	return {
		start: Math.max(0, first),
		end: Math.min(rowCount, Math.max(0, last)),
	};
}

/** Total scrollable height for `rowCount` rows. */
export function totalHeight(rowCount: number): number {
	return rowCount * ROW_HEIGHT;
}

/** New `scrollTop` that keeps `rowIndex` at the same on-screen offset it currently occupies —
 * the basis of refresh scroll-anchoring (DESIGN_SPEC.md §4.7 / §15.32). */
export function anchoredScrollTop(rowIndex: number, offsetWithinList: number): number {
	return Math.max(0, rowIndex * ROW_HEIGHT + offsetWithinList);
}
