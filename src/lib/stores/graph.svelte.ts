import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCommitMeta, getGraph, getRefs, getWorktrees, onRepoChanged } from "$lib/ipc";
import {
	assignLanes,
	type GraphLaneRow,
	type LaneAssignment,
	type LanePassSpan,
} from "$lib/graph/lanes";
import { buildPills, type Pill } from "$lib/graph/pills";
import type {
	ChangeKind,
	CommitMeta,
	GraphTopologyRow,
	HeadInfo,
	RefInfo,
	WorktreeInfo,
} from "$lib/types";

const METADATA_BATCH_SIZE = 200;

export type GraphViewRow = GraphLaneRow & {
	index: number;
	meta: CommitMeta | null;
	refs: RefInfo[];
	pills: Pill[];
};

export interface GraphStoreDeps {
	getGraph(repoId: string): Promise<GraphTopologyRow[]>;
	getCommitMeta(repoId: string, shas: string[]): Promise<CommitMeta[]>;
	getRefs(repoId: string): Promise<{ refs: RefInfo[]; head: HeadInfo }>;
	getWorktrees(repoId: string): Promise<WorktreeInfo[]>;
	onRepoChanged(repoId: string, handler: (kind: ChangeKind) => void): Promise<UnlistenFn>;
	assignLanes(topology: readonly GraphTopologyRow[]): LaneAssignment;
}

const defaultDeps: GraphStoreDeps = {
	getGraph,
	getCommitMeta,
	getRefs,
	getWorktrees,
	onRepoChanged,
	assignLanes,
};

function refsBySha(refs: readonly RefInfo[]): Record<string, RefInfo[]> {
	const bySha: Record<string, RefInfo[]> = {};
	for (const ref of refs) {
		(bySha[ref.sha] ??= []).push(ref);
	}
	return bySha;
}

function uniqueMissingCommitShas(
	rows: readonly GraphLaneRow[],
	metaBySha: Record<string, CommitMeta>,
	inFlight: Set<string>,
	start: number,
	end: number,
): string[] {
	const shas: string[] = [];
	const seen = new Set<string>();
	for (const row of rows.slice(Math.max(0, start), Math.max(0, end))) {
		if (row.kind !== "commit") continue;
		if (metaBySha[row.sha] || inFlight.has(row.sha) || seen.has(row.sha)) continue;
		seen.add(row.sha);
		shas.push(row.sha);
	}
	return shas;
}

export class GraphStore {
	repoId: string | null = $state(null);
	// These payloads are immutable snapshots replaced as a unit. Deep-proxying a 20k+ topology
	// recursively wraps millions of canvas segment objects and can turn a sub-second load into a
	// multi-minute stall. Raw state preserves replacement reactivity without touching every edge.
	laneRows: GraphLaneRow[] = $state.raw([]);
	laneColors: number[] = $state.raw([]);
	passSpansByLane: LanePassSpan[][] = $state.raw([]);
	metaBySha: Record<string, CommitMeta> = $state({});
	refsBySha: Record<string, RefInfo[]> = $state({});
	/** Flat ref list (LOCAL/REMOTES/TAGS in the left panel) — DESIGN_SPEC §5. */
	refs: RefInfo[] = $state([]);
	/** Grouped branch/tag pills (shared/split, presence, ahead/behind) — DESIGN_SPEC §4.4. */
	pillsBySha: Record<string, Pill[]> = $state({});
	/** Linked worktrees for the left-panel WORKTREES section (§5). */
	worktrees: WorktreeInfo[] = $state([]);
	head: HeadInfo | null = $state(null);
	loading = $state(false);
	error: unknown = $state(null);
	laneComputeCount = $state(0);

	rows: GraphViewRow[] = $derived(
		this.laneRows.map((row, index) => ({
			...row,
			index,
			meta: row.kind === "commit" ? (this.metaBySha[row.sha] ?? null) : null,
			refs: this.refsBySha[row.sha] ?? [],
			pills: this.pillsBySha[row.sha] ?? [],
		})),
	);

	/** Stash pseudo-rows (left-panel STASHES section, §5) — they live inline in the topology. */
	stashes = $derived(
		this.laneRows.filter((r): r is Extract<GraphLaneRow, { kind: "stash" }> => r.kind === "stash"),
	);

	#deps: GraphStoreDeps;
	#unlisten: UnlistenFn | null = null;
	#metaInFlight = new Set<string>();

	constructor(deps: Partial<GraphStoreDeps> = {}) {
		this.#deps = { ...defaultDeps, ...deps };
	}

	async open(repoId: string): Promise<void> {
		await this.close();
		this.repoId = repoId;
		this.loading = true;
		this.error = null;
		this.#unlisten = await this.#deps.onRepoChanged(repoId, (kind) => {
			void this.handleChange(kind);
		});
		try {
			await Promise.all([this.reloadTopology(), this.refreshRefs(), this.refreshWorktrees()]);
		} catch (e) {
			this.error = e;
			throw e;
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
		this.laneRows = [];
		this.laneColors = [];
		this.passSpansByLane = [];
		this.metaBySha = {};
		this.refsBySha = {};
		this.refs = [];
		this.pillsBySha = {};
		this.worktrees = [];
		this.head = null;
		this.#metaInFlight.clear();
	}

	async reloadTopology(): Promise<void> {
		if (!this.repoId) return;
		const topology = await this.#deps.getGraph(this.repoId);
		const assignment = this.#deps.assignLanes(topology);
		this.laneComputeCount += 1;
		this.laneRows = assignment.rows;
		this.laneColors = assignment.laneColors;
		this.passSpansByLane = assignment.passSpansByLane;
		this.metaBySha = {};
		this.#metaInFlight.clear();
	}

	async refreshRefs(): Promise<void> {
		if (!this.repoId) return;
		const response = await this.#deps.getRefs(this.repoId);
		this.refs = response.refs;
		this.refsBySha = refsBySha(response.refs);
		this.head = response.head;
		this.pillsBySha = buildPills(response.refs, response.head);
	}

	async refreshWorktrees(): Promise<void> {
		if (!this.repoId) return;
		this.worktrees = await this.#deps.getWorktrees(this.repoId);
	}

	async ensureMetadataForWindow(start: number, end: number): Promise<void> {
		if (!this.repoId) return;
		const missing = uniqueMissingCommitShas(
			this.laneRows,
			this.metaBySha,
			this.#metaInFlight,
			start,
			end,
		);
		if (missing.length === 0) return;

		for (let i = 0; i < missing.length; i += METADATA_BATCH_SIZE) {
			const batch = missing.slice(i, i + METADATA_BATCH_SIZE);
			for (const sha of batch) this.#metaInFlight.add(sha);
			try {
				const metas = await this.#deps.getCommitMeta(this.repoId, batch);
				const next = { ...this.metaBySha };
				for (const meta of metas) next[meta.sha] = meta;
				this.metaBySha = next;
			} finally {
				for (const sha of batch) this.#metaInFlight.delete(sha);
			}
		}
	}

	async handleChange(kind: ChangeKind): Promise<void> {
		if (kind.kind === "operationProgress" || kind.kind === "workingTree" || kind.kind === "index") {
			return;
		}
		if (kind.kind === "refs" || kind.kind === "remote") {
			await Promise.all([this.refreshRefs(), this.refreshWorktrees()]);
			return;
		}
		if (kind.kind === "head") {
			await Promise.all([this.reloadTopology(), this.refreshRefs(), this.refreshWorktrees()]);
		}
	}
}

export const graph = new GraphStore();
