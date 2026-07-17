import type { GraphTopologyRow } from "$lib/types";

export type GraphEdgeKind = "pass" | "fork" | "merge";

export interface GraphEdgeOp {
	fromLane: number;
	toLane: number;
	kind: GraphEdgeKind;
}

export interface GraphNodeOp {
	lane: number;
	colorIndex: number;
}

/**
 * A drawable connection crossing a single row's vertical band — ARCHITECTURE.md §5.4. Unlike
 * {@link GraphEdgeOp} (which only records the commit's own parent/child connections and is what the
 * lane snapshot tests assert), segments also include the lanes that merely *pass through* a row, so
 * the canvas can draw continuous lane lines without replaying the algorithm. Each end sits on the
 * row's top boundary, its bottom boundary, or the commit node at the row centre; adjacent rows
 * share boundaries, so per-row segments join into unbroken lines.
 */
export type SegmentEnd =
	| { at: "top"; lane: number }
	| { at: "node" }
	| { at: "bottom"; lane: number };

export interface GraphSegment {
	from: SegmentEnd;
	to: SegmentEnd;
	colorIndex: number;
	/** Dashed offshoot — the stash connector (DESIGN_SPEC.md §4.5). */
	dashed?: boolean;
}

export type GraphLaneRow =
	| {
			kind: "commit";
			sha: string;
			parents: string[];
			node: GraphNodeOp;
			edges: GraphEdgeOp[];
			segments: GraphSegment[];
	  }
	| {
			kind: "stash";
			sha: string;
			baseSha: string;
			selector: string;
			subject: string;
			node: GraphNodeOp;
			edges: GraphEdgeOp[];
			segments: GraphSegment[];
	  };

export interface LaneAssignment {
	rows: GraphLaneRow[];
	laneColors: number[];
	maxLane: number;
	passSpansByLane: LanePassSpan[][];
}

export interface LanePassSpan {
	startRow: number;
	endRow: number;
}

export const GRAPH_LANE_PALETTE_SIZE = 8;

export function laneColorIndex(lane: number): number {
	return lane % GRAPH_LANE_PALETTE_SIZE;
}

function firstAvailableLane(lanes: (string | null)[]): number {
	const free = lanes.indexOf(null);
	if (free !== -1) return free;
	lanes.push(null);
	return lanes.length - 1;
}

function compactTrailingNulls(lanes: (string | null)[]) {
	while (lanes.at(-1) === null) lanes.pop();
}

function updatePassSpans(
	lanes: readonly (string | null)[],
	excludedSha: string | undefined,
	rowIndex: number,
	openStarts: (number | null)[],
	spansByLane: LanePassSpan[][],
) {
	const length = Math.max(lanes.length, openStarts.length);
	for (let lane = 0; lane < length; lane += 1) {
		const active = lane < lanes.length && lanes[lane] !== null && lanes[lane] !== excludedSha;
		if (active && openStarts[lane] == null) openStarts[lane] = rowIndex;
		if (!active && openStarts[lane] != null) {
			(spansByLane[lane] ??= []).push({ startRow: openStarts[lane]!, endRow: rowIndex - 1 });
			openStarts[lane] = null;
		}
	}
}

export function assignLanes(topology: readonly GraphTopologyRow[]): LaneAssignment {
	const lanes: (string | null)[] = [];
	const commitLanes = new Map<string, number>();
	const rows: GraphLaneRow[] = [];
	const passSpansByLane: LanePassSpan[][] = [];
	const openPassStarts: (number | null)[] = [];
	let maxLane = 0;

	for (let rowIndex = 0; rowIndex < topology.length; rowIndex += 1) {
		const row = topology[rowIndex];
		if (row.kind === "stash") {
			const lane = commitLanes.get(row.baseSha) ?? 0;
			maxLane = Math.max(maxLane, lane);
			// Every active lane runs straight through the stash pseudo-row; the stash itself hangs off
			// its base commit (directly above) on a short dashed connector.
			updatePassSpans(lanes, undefined, rowIndex, openPassStarts, passSpansByLane);
			const segments: GraphSegment[] = [];
			segments.push({
				from: { at: "top", lane },
				to: { at: "node" },
				colorIndex: laneColorIndex(lane),
				dashed: true,
			});
			rows.push({
				...row,
				node: { lane, colorIndex: laneColorIndex(lane) },
				edges: [],
				segments,
			});
			continue;
		}

		const before = lanes.slice();
		const expecting: number[] = [];
		for (let i = 0; i < lanes.length; i += 1) {
			if (lanes[i] === row.sha) expecting.push(i);
		}

		const edges: GraphEdgeOp[] = [];
		let lane: number;
		if (expecting.length === 0) {
			lane = firstAvailableLane(lanes);
		} else {
			lane = Math.min(...expecting);
			for (const i of expecting) {
				if (i === lane) {
					edges.push({ fromLane: i, toLane: lane, kind: "pass" });
				} else {
					edges.push({ fromLane: i, toLane: lane, kind: "fork" });
					lanes[i] = null;
				}
			}
		}

		commitLanes.set(row.sha, lane);
		maxLane = Math.max(maxLane, lane);

		const [firstParent, ...otherParents] = row.parents;
		lanes[lane] = firstParent ?? null;
		if (firstParent) {
			edges.push({ fromLane: lane, toLane: lane, kind: "pass" });
		}

		const mergeParentLanes: number[] = [];
		for (const parent of otherParents) {
			let parentLane = lanes.indexOf(parent);
			if (parentLane === -1) {
				parentLane = firstAvailableLane(lanes);
				lanes[parentLane] = parent;
			}
			maxLane = Math.max(maxLane, parentLane);
			edges.push({ fromLane: lane, toLane: parentLane, kind: "merge" });
			mergeParentLanes.push(parentLane);
		}

		compactTrailingNulls(lanes);

		// Render segments: incoming lanes fold into the node, unrelated lanes pass straight through
		// at a stable index, and the node's parents fan out toward the bottom boundary.
		const segments: GraphSegment[] = [];
		for (const i of expecting) {
			segments.push({
				from: { at: "top", lane: i },
				to: { at: "node" },
				colorIndex: laneColorIndex(i),
			});
		}
		updatePassSpans(before, row.sha, rowIndex, openPassStarts, passSpansByLane);
		if (firstParent) {
			segments.push({
				from: { at: "node" },
				to: { at: "bottom", lane },
				colorIndex: laneColorIndex(lane),
			});
		}
		for (const parentLane of mergeParentLanes) {
			segments.push({
				from: { at: "node" },
				to: { at: "bottom", lane: parentLane },
				colorIndex: laneColorIndex(parentLane),
			});
		}

		rows.push({
			kind: "commit",
			sha: row.sha,
			parents: row.parents,
			node: { lane, colorIndex: laneColorIndex(lane) },
			edges,
			segments,
		});
	}
	updatePassSpans([], undefined, topology.length, openPassStarts, passSpansByLane);

	const laneColors = Array.from({ length: maxLane + 1 }, (_, lane) => laneColorIndex(lane));
	return { rows, laneColors, maxLane, passSpansByLane };
}
