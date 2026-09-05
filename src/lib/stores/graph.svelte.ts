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
	getGraph(repoId: string, excludedRefs?: string[]): Promise<GraphTopologyRow[]>;
	getCommitMeta(repoId: string, shas: string[]): Promise<CommitMeta[]>;
	getRefs(repoId: string): Promise<{ refs: RefInfo[]; head: HeadInfo }>;
	getWorktrees(repoId: string): Promise<WorktreeInfo[]>;
	onRepoChanged(repoId: string, handler: (kind: ChangeKind) => void): Promise<UnlistenFn>;
	assignLanes(topology: readonly GraphTopologyRow[]): LaneAssignment;
}

const HIDDEN_BRANCHES_PREFIX = "branchkit:hidden-branches:";

function loadHiddenBranches(key: string): string[] {
	if (typeof localStorage === "undefined") return [];
	try {
		const parsed = JSON.parse(localStorage.getItem(`${HIDDEN_BRANCHES_PREFIX}${key}`) ?? "[]");
		return Array.isArray(parsed)
			? parsed.filter((name): name is string => typeof name === "string")
			: [];
	} catch {
		return [];
	}
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
	hiddenBranches: string[] = $state([]);

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
	/** Monotonic request id: overlapping watcher refreshes must never let an older refs snapshot
	 * replace a newer one. Refs and Remote are emitted back-to-back by network operations, and
	 * reads intentionally do not take the backend mutation lock (ARCHITECTURE §2). */
	#refsRequestId = 0;
	#topologyRequestId = 0;
	#persistenceKey: string | null = null;

	constructor(deps: Partial<GraphStoreDeps> = {}) {
		this.#deps = { ...defaultDeps, ...deps };
	}

	async open(repoId: string, persistenceKey: string = repoId): Promise<void> {
		await this.close();
		this.repoId = repoId;
		this.#persistenceKey = encodeURIComponent(persistenceKey);
		this.hiddenBranches = loadHiddenBranches(this.#persistenceKey);
		this.loading = true;
		this.error = null;
		this.#unlisten = await this.#deps.onRepoChanged(repoId, (kind) => {
			void this.handleChange(kind);
		});
		try {
			await this.refreshRefs();
			await Promise.all([this.reloadTopology(), this.refreshWorktrees()]);
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
		this.hiddenBranches = [];
		this.#metaInFlight.clear();
		this.#refsRequestId += 1;
		this.#topologyRequestId += 1;
		this.#persistenceKey = null;
	}

	async reloadTopology(): Promise<void> {
		const repoId = this.repoId;
		if (!repoId) return;
		const requestId = ++this.#topologyRequestId;
		const excludedRefs = this.hiddenBranches.flatMap((name) => {
			const local = this.refs.find((ref) => ref.kind === "branch" && ref.shortName === name);
			return [
				`refs/heads/${name}`,
				...(local?.upstream ? [`refs/remotes/${local.upstream}`] : []),
			];
		});
		const topology = await this.#deps.getGraph(repoId, excludedRefs);
		if (requestId !== this.#topologyRequestId || this.repoId !== repoId) return;
		const assignment = this.#deps.assignLanes(topology);
		this.laneComputeCount += 1;
		this.laneRows = assignment.rows;
		this.laneColors = assignment.laneColors;
		this.passSpansByLane = assignment.passSpansByLane;
		this.metaBySha = {};
		this.#metaInFlight.clear();
	}

	isBranchHidden(name: string): boolean {
		return this.hiddenBranches.includes(name);
	}

	async toggleHiddenBranch(name: string): Promise<void> {
		this.hiddenBranches = this.isBranchHidden(name)
			? this.hiddenBranches.filter((branch) => branch !== name)
			: [...this.hiddenBranches, name];
		if (this.#persistenceKey && typeof localStorage !== "undefined") {
			localStorage.setItem(
				`${HIDDEN_BRANCHES_PREFIX}${this.#persistenceKey}`,
				JSON.stringify(this.hiddenBranches),
			);
		}
		await this.reloadTopology();
	}

	async refreshRefs(): Promise<void> {
		const repoId = this.repoId;
		if (!repoId) return;
		const requestId = ++this.#refsRequestId;
		const response = await this.#deps.getRefs(repoId);
		// A later refresh (or close/reopen) superseded this read while it was in flight.
		if (requestId !== this.#refsRequestId || this.repoId !== repoId) return;
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
		if (kind.kind === "refs") {
			await Promise.all([this.refreshRefs(), this.refreshWorktrees()]);
			return;
		}
		if (kind.kind === "remote") {
			await this.refreshRefs();
			await Promise.all([this.reloadTopology(), this.refreshWorktrees()]);
			return;
		}
		if (kind.kind === "head") {
			await Promise.all([this.reloadTopology(), this.refreshRefs(), this.refreshWorktrees()]);
		}
	}
}

export const graph = new GraphStore();
