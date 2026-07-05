/** Persisted, per-machine view preferences for the commit graph — DESIGN_SPEC.md §4.1 (column
 * visibility + draggable widths, gear menu, §15.27) and §4.6 (the detached-checkout "don't ask
 * again" choice). Panel/graph chrome persists globally, not per repo (DESIGN_SPEC.md §3). */

export type OptionalColumn = "author" | "date" | "sha";
export type SizableColumn = "branch" | "graph" | "author" | "date" | "sha";

interface Persisted {
	author: boolean;
	date: boolean;
	sha: boolean;
	widths: Record<SizableColumn, number>;
	detachDontAsk: boolean;
}

const STORAGE_KEY = "branchkit:graph-view";

const DEFAULT_WIDTHS: Record<SizableColumn, number> = {
	branch: 200,
	graph: 140,
	author: 130,
	date: 96,
	sha: 72,
};

const MIN_WIDTH: Record<SizableColumn, number> = {
	branch: 80,
	graph: 48,
	author: 72,
	date: 72,
	sha: 56,
};

const MAX_WIDTH = 480;

function clampWidth(column: SizableColumn, px: number): number {
	return Math.max(MIN_WIDTH[column], Math.min(MAX_WIDTH, Math.round(px)));
}

function load(): Partial<Persisted> {
	if (typeof localStorage === "undefined") return {};
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		return raw ? (JSON.parse(raw) as Partial<Persisted>) : {};
	} catch {
		return {};
	}
}

class GraphViewStore {
	author = $state(true);
	date = $state(true);
	sha = $state(false);
	widths: Record<SizableColumn, number> = $state({ ...DEFAULT_WIDTHS });
	detachDontAsk = $state(false);
	/** Auto-computed GRAPH column width — sized to the widest visible lane layout by the view
	 * (not persisted, not user-resizable): the column collapses/expands with the graph itself. */
	graphAuto = $state(DEFAULT_WIDTHS.graph);

	constructor() {
		const stored = load();
		if (typeof stored.author === "boolean") this.author = stored.author;
		if (typeof stored.date === "boolean") this.date = stored.date;
		if (typeof stored.sha === "boolean") this.sha = stored.sha;
		if (typeof stored.detachDontAsk === "boolean") this.detachDontAsk = stored.detachDontAsk;
		if (stored.widths) {
			for (const key of Object.keys(DEFAULT_WIDTHS) as SizableColumn[]) {
				const value = stored.widths[key];
				if (typeof value === "number") this.widths[key] = clampWidth(key, value);
			}
		}
	}

	#persist() {
		if (typeof localStorage === "undefined") return;
		const snapshot: Persisted = {
			author: this.author,
			date: this.date,
			sha: this.sha,
			widths: { ...this.widths },
			detachDontAsk: this.detachDontAsk,
		};
		localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot));
	}

	isVisible(column: OptionalColumn): boolean {
		return this[column];
	}

	toggle(column: OptionalColumn) {
		this[column] = !this[column];
		this.#persist();
	}

	setWidth(column: SizableColumn, px: number) {
		this.widths = { ...this.widths, [column]: clampWidth(column, px) };
		this.#persist();
	}

	resetWidth(column: SizableColumn) {
		this.setWidth(column, DEFAULT_WIDTHS[column]);
	}

	setGraphAuto(px: number) {
		const clamped = Math.max(MIN_WIDTH.graph, Math.min(MAX_WIDTH, Math.round(px)));
		if (clamped !== this.graphAuto) this.graphAuto = clamped;
	}

	setDetachDontAsk(value: boolean) {
		this.detachDontAsk = value;
		this.#persist();
	}
}

export const graphView = new GraphViewStore();
