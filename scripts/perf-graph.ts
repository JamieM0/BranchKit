import { execFileSync } from "node:child_process";
import { performance } from "node:perf_hooks";
import { assignLanes } from "../src/lib/graph/lanes.ts";

const repo = process.argv[2];
if (!repo) {
	console.error("Usage: npx vite-node scripts/perf-graph.ts <repo-path>");
	process.exit(2);
}

const started = performance.now();
const output = execFileSync(
	"git",
	["-C", repo, "rev-list", "--exclude=refs/stash", "--all", "--topo-order", "--parents"],
	{ encoding: "utf8", maxBuffer: 128 * 1024 * 1024 },
);
const topologyLoaded = performance.now();
const topology = output
	.trim()
	.split("\n")
	.filter(Boolean)
	.map((line) => {
		const [sha, ...parents] = line.split(" ");
		return { kind: "commit" as const, sha, parents };
	});
const assignment = assignLanes(topology);
const lanesAssigned = performance.now();

console.log(
	JSON.stringify({
		commits: topology.length,
		gitMs: Number((topologyLoaded - started).toFixed(1)),
		lanesMs: Number((lanesAssigned - topologyLoaded).toFixed(1)),
		totalMs: Number((lanesAssigned - started).toFixed(1)),
		maxLane: assignment.maxLane,
		segmentObjects: assignment.rows.reduce((count, row) => count + row.segments.length, 0),
		passSpanObjects: assignment.passSpansByLane.reduce((count, spans) => count + spans.length, 0),
		passingLanes: assignment.passSpansByLane.reduce(
			(count, spans) => count + spans.reduce((n, span) => n + span.endRow - span.startRow + 1, 0),
			0,
		),
	}),
);
