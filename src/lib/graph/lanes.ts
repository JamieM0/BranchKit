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

export type GraphLaneRow =
	| {
			kind: "commit";
			sha: string;
			parents: string[];
			node: GraphNodeOp;
			edges: GraphEdgeOp[];
	  }
	| {
			kind: "stash";
			sha: string;
			baseSha: string;
			selector: string;
			subject: string;
			node: GraphNodeOp;
			edges: GraphEdgeOp[];
	  };

export interface LaneAssignment {
	rows: GraphLaneRow[];
	laneColors: number[];
	maxLane: number;
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

export function assignLanes(topology: readonly GraphTopologyRow[]): LaneAssignment {
	const lanes: (string | null)[] = [];
	const commitLanes = new Map<string, number>();
	const rows: GraphLaneRow[] = [];
	let maxLane = 0;

	for (const row of topology) {
		if (row.kind === "stash") {
			const lane = commitLanes.get(row.baseSha) ?? 0;
			maxLane = Math.max(maxLane, lane);
			rows.push({
				...row,
				node: { lane, colorIndex: laneColorIndex(lane) },
				edges: [],
			});
			continue;
		}

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

		for (const parent of otherParents) {
			let parentLane = lanes.indexOf(parent);
			if (parentLane === -1) {
				parentLane = firstAvailableLane(lanes);
				lanes[parentLane] = parent;
			}
			maxLane = Math.max(maxLane, parentLane);
			edges.push({ fromLane: lane, toLane: parentLane, kind: "merge" });
		}

		compactTrailingNulls(lanes);
		rows.push({
			kind: "commit",
			sha: row.sha,
			parents: row.parents,
			node: { lane, colorIndex: laneColorIndex(lane) },
			edges,
		});
	}

	const laneColors = Array.from({ length: maxLane + 1 }, (_, lane) => laneColorIndex(lane));
	return { rows, laneColors, maxLane };
}
