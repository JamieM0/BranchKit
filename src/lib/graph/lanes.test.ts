import { describe, expect, it } from "vitest";
import { assignLanes, type GraphLaneRow, type SegmentEnd } from "./lanes";
import type { GraphTopologyRow } from "$lib/types";

function commit(sha: string, parents: string[] = []): GraphTopologyRow {
	return { kind: "commit", sha, parents };
}

function stash(sha: string, baseSha: string, subject = "stash: work in progress"): GraphTopologyRow {
	return { kind: "stash", sha, baseSha, selector: "stash@{0}", subject };
}

function compact(topology: GraphTopologyRow[]) {
	return assignLanes(topology).rows.map((row) => ({
		kind: row.kind,
		sha: row.sha,
		lane: row.node.lane,
		color: row.node.colorIndex,
		edges: row.edges,
	}));
}

describe("assignLanes", () => {
	it("assigns a linear history to one lane", () => {
		expect(compact([commit("C", ["B"]), commit("B", ["A"]), commit("A")])).toMatchInlineSnapshot(`
			[
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "C",
			  },
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "B",
			  },
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "A",
			  },
			]
		`);
	});

	it("opens a merge lane for a single merge commit", () => {
		expect(
			compact([
				commit("M", ["A", "B"]),
				commit("A", ["BASE"]),
				commit("B", ["BASE"]),
				commit("BASE"),
			]),
		).toMatchInlineSnapshot(`
			[
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 0,
			        "kind": "merge",
			        "toLane": 1,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "M",
			  },
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "A",
			  },
			  {
			    "color": 1,
			    "edges": [
			      {
			        "fromLane": 1,
			        "kind": "pass",
			        "toLane": 1,
			      },
			      {
			        "fromLane": 1,
			        "kind": "pass",
			        "toLane": 1,
			      },
			    ],
			    "kind": "commit",
			    "lane": 1,
			    "sha": "B",
			  },
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 1,
			        "kind": "fork",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "BASE",
			  },
			]
		`);
	});

	it("reuses lanes for two parallel branches", () => {
		expect(compact([commit("LEFT", ["BASE"]), commit("RIGHT", ["BASE"]), commit("BASE")]))
			.toMatchInlineSnapshot(`
				[
				  {
				    "color": 0,
				    "edges": [
				      {
				        "fromLane": 0,
				        "kind": "pass",
				        "toLane": 0,
				      },
				    ],
				    "kind": "commit",
				    "lane": 0,
				    "sha": "LEFT",
				  },
				  {
				    "color": 1,
				    "edges": [
				      {
				        "fromLane": 1,
				        "kind": "pass",
				        "toLane": 1,
				      },
				    ],
				    "kind": "commit",
				    "lane": 1,
				    "sha": "RIGHT",
				  },
				  {
				    "color": 0,
				    "edges": [
				      {
				        "fromLane": 0,
				        "kind": "pass",
				        "toLane": 0,
				      },
				      {
				        "fromLane": 1,
				        "kind": "fork",
				        "toLane": 0,
				      },
				    ],
				    "kind": "commit",
				    "lane": 0,
				    "sha": "BASE",
				  },
				]
			`);
	});

	it("handles criss-cross merge topology", () => {
		expect(
			compact([
				commit("LEFT_MERGE", ["LEFT_1", "RIGHT_MERGE"]),
				commit("RIGHT_MERGE", ["RIGHT_1", "LEFT_1"]),
				commit("LEFT_1", ["BASE"]),
				commit("RIGHT_1", ["BASE"]),
				commit("BASE"),
			]),
		).toMatchInlineSnapshot(`
			[
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 0,
			        "kind": "merge",
			        "toLane": 1,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "LEFT_MERGE",
			  },
			  {
			    "color": 1,
			    "edges": [
			      {
			        "fromLane": 1,
			        "kind": "pass",
			        "toLane": 1,
			      },
			      {
			        "fromLane": 1,
			        "kind": "pass",
			        "toLane": 1,
			      },
			      {
			        "fromLane": 1,
			        "kind": "merge",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 1,
			    "sha": "RIGHT_MERGE",
			  },
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "LEFT_1",
			  },
			  {
			    "color": 1,
			    "edges": [
			      {
			        "fromLane": 1,
			        "kind": "pass",
			        "toLane": 1,
			      },
			      {
			        "fromLane": 1,
			        "kind": "pass",
			        "toLane": 1,
			      },
			    ],
			    "kind": "commit",
			    "lane": 1,
			    "sha": "RIGHT_1",
			  },
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 1,
			        "kind": "fork",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "BASE",
			  },
			]
		`);
	});

	it("handles octopus merges with three parents", () => {
		expect(
			compact([
				commit("OCTOPUS", ["A", "B", "C"]),
				commit("A", ["BASE"]),
				commit("B", ["BASE"]),
				commit("C", ["BASE"]),
				commit("BASE"),
			]),
		).toMatchInlineSnapshot(`
			[
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 0,
			        "kind": "merge",
			        "toLane": 1,
			      },
			      {
			        "fromLane": 0,
			        "kind": "merge",
			        "toLane": 2,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "OCTOPUS",
			  },
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "A",
			  },
			  {
			    "color": 1,
			    "edges": [
			      {
			        "fromLane": 1,
			        "kind": "pass",
			        "toLane": 1,
			      },
			      {
			        "fromLane": 1,
			        "kind": "pass",
			        "toLane": 1,
			      },
			    ],
			    "kind": "commit",
			    "lane": 1,
			    "sha": "B",
			  },
			  {
			    "color": 2,
			    "edges": [
			      {
			        "fromLane": 2,
			        "kind": "pass",
			        "toLane": 2,
			      },
			      {
			        "fromLane": 2,
			        "kind": "pass",
			        "toLane": 2,
			      },
			    ],
			    "kind": "commit",
			    "lane": 2,
			    "sha": "C",
			  },
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 1,
			        "kind": "fork",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 2,
			        "kind": "fork",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "BASE",
			  },
			]
		`);
	});

	// The canvas draws per-row segments; for lines to join across rows, the set of lanes leaving a
	// row's bottom boundary must equal the set entering the next row's top boundary.
	function boundaryLanes(
		row: GraphLaneRow,
		rowIndex: number,
		at: "top" | "bottom",
		spansByLane: ReturnType<typeof assignLanes>["passSpansByLane"],
	): Set<number> {
		const lanes = new Set<number>();
		for (let lane = 0; lane < spansByLane.length; lane += 1) {
			if (spansByLane[lane]?.some((span) => span.startRow <= rowIndex && span.endRow >= rowIndex)) {
				lanes.add(lane);
			}
		}
		const collect = (end: SegmentEnd) => {
			if (end.at === at) lanes.add(end.lane);
		};
		for (const seg of row.segments) {
			collect(seg.from);
			collect(seg.to);
		}
		return lanes;
	}

	function expectSetsEqual(a: Set<number>, b: Set<number>) {
		expect([...a].sort()).toEqual([...b].sort());
	}

	function expectContinuous(topology: GraphTopologyRow[]) {
		const { rows, passSpansByLane } = assignLanes(topology);
		expect(boundaryLanes(rows[0], 0, "top", passSpansByLane).size).toBe(0);
		expect(boundaryLanes(rows.at(-1)!, rows.length - 1, "bottom", passSpansByLane).size).toBe(0);
		for (let i = 0; i < rows.length - 1; i += 1) {
			expectSetsEqual(
				boundaryLanes(rows[i], i, "bottom", passSpansByLane),
				boundaryLanes(rows[i + 1], i + 1, "top", passSpansByLane),
			);
		}
	}

	describe("render segments", () => {
		it("keep lane lines continuous across every row boundary", () => {
			expectContinuous([commit("C", ["B"]), commit("B", ["A"]), commit("A")]);
			expectContinuous([
				commit("M", ["A", "B"]),
				commit("A", ["BASE"]),
				commit("B", ["BASE"]),
				commit("BASE"),
			]);
			expectContinuous([
				commit("LEFT_MERGE", ["LEFT_1", "RIGHT_MERGE"]),
				commit("RIGHT_MERGE", ["RIGHT_1", "LEFT_1"]),
				commit("LEFT_1", ["BASE"]),
				commit("RIGHT_1", ["BASE"]),
				commit("BASE"),
			]);
			expectContinuous([
				commit("OCTOPUS", ["A", "B", "C"]),
				commit("A", ["BASE"]),
				commit("B", ["BASE"]),
				commit("C", ["BASE"]),
				commit("BASE"),
			]);
		});

		it("routes a merge commit's parents to distinct bottom lanes", () => {
			const { rows } = assignLanes([
				commit("M", ["A", "B"]),
				commit("A", ["BASE"]),
				commit("B", ["BASE"]),
				commit("BASE"),
			]);
			const bottoms = rows[0].segments
				.filter((s) => s.from.at === "node" && s.to.at === "bottom")
				.map((s) => (s.to.at === "bottom" ? s.to.lane : -1))
				.sort();
			expect(bottoms).toEqual([0, 1]);
		});

		it("passes active lanes straight through a stash row with a dashed connector", () => {
			const { rows, passSpansByLane } = assignLanes([
				commit("C", ["B"]),
				commit("B", ["A"]),
				stash("STASH", "B", "stash: keep changes"),
				commit("A"),
			]);
			const stashRow = rows.find((r) => r.kind === "stash")!;
			expect(stashRow.segments.some((s) => s.dashed && s.to.at === "node")).toBe(true);
			// Lane 0 (B's first parent A) keeps flowing through the stash row.
			expect(passSpansByLane[0].some((span) => span.startRow <= 2 && span.endRow >= 2)).toBe(true);
		});
	});

	it("attaches stash pseudo-rows to their base commit lane", () => {
		expect(
			compact([
				commit("C", ["B"]),
				commit("B", ["A"]),
				stash("STASH", "B", "stash: keep changes"),
				commit("A"),
			]),
		).toMatchInlineSnapshot(`
			[
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "C",
			  },
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "B",
			  },
			  {
			    "color": 0,
			    "edges": [],
			    "kind": "stash",
			    "lane": 0,
			    "sha": "STASH",
			  },
			  {
			    "color": 0,
			    "edges": [
			      {
			        "fromLane": 0,
			        "kind": "pass",
			        "toLane": 0,
			      },
			    ],
			    "kind": "commit",
			    "lane": 0,
			    "sha": "A",
			  },
			]
		`);
	});
});
