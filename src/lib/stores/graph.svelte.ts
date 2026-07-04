import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCommitMeta, getGraph, getRefs, onRepoChanged } from "$lib/ipc";
import { assignLanes, type GraphLaneRow, type LaneAssignment } from "$lib/graph/lanes";
import type { ChangeKind, CommitMeta, GraphTopologyRow, HeadInfo, RefInfo } from "$lib/types";

const METADATA_BATCH_SIZE = 200;

export type GraphViewRow = GraphLaneRow & {
	index: number;
	meta: CommitMeta | null;
	refs: RefInfo[];
};

export interface GraphStoreDeps {
	getGraph(repoId: string): Promise<GraphTopologyRow[]>;
	getCommitMeta(repoId: string, shas: string[]): Promise<CommitMeta[]>;
	getRefs(repoId: string): Promise<{ refs: RefInfo[]; head: HeadInfo }>;
	onRepoChanged(repoId: string, handler: (kind: ChangeKind) => void): Promise<UnlistenFn>;
	assignLanes(topology: readonly GraphTopologyRow[]): LaneAssignment;
}

const defaultDeps: GraphStoreDeps = {
	getGraph,
	getCommitMeta,
	getRefs,
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
	topology: GraphTopologyRow[] = $state([]);
	laneRows: GraphLaneRow[] = $state([]);
	laneColors: number[] = $state([]);
	metaBySha: Record<string, CommitMeta> = $state({});
	refsBySha: Record<string, RefInfo[]> = $state({});
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
		})),
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
			await Promise.all([this.reloadTopology(), this.refreshRefs()]);
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
		this.topology = [];
		this.laneRows = [];
		this.laneColors = [];
		this.metaBySha = {};
		this.refsBySha = {};
		this.head = null;
		this.#metaInFlight.clear();
	}

	async reloadTopology(): Promise<void> {
		if (!this.repoId) return;
		const topology = await this.#deps.getGraph(this.repoId);
		const assignment = this.#deps.assignLanes(topology);
		this.laneComputeCount += 1;
		this.topology = topology;
		this.laneRows = assignment.rows;
		this.laneColors = assignment.laneColors;
		this.metaBySha = {};
		this.#metaInFlight.clear();
	}

	async refreshRefs(): Promise<void> {
		if (!this.repoId) return;
		const response = await this.#deps.getRefs(this.repoId);
		this.refsBySha = refsBySha(response.refs);
		this.head = response.head;
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
			await this.refreshRefs();
			return;
		}
		if (kind.kind === "head") {
			await Promise.all([this.reloadTopology(), this.refreshRefs()]);
		}
	}
}

export const graph = new GraphStore();
