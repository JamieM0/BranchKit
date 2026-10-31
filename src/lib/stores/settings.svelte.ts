/** Lightweight local view preferences that are not part of the backend settings schema.
 * Persisted globally (per-machine), not per repo. */

interface Persisted {
	leftPanelCollapsed: boolean;
	leftPanelWidth: number;
	rightPanelWidth: number;
	/** Working-directory file list display — DESIGN_SPEC.md §6.1 "Path/Tree toggle (persisted)". */
	fileListView: "path" | "tree";
}

const STORAGE_KEY = "branchkit:settings";

function load(): Partial<Persisted> {
	if (typeof localStorage === "undefined") return {};
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		return raw ? (JSON.parse(raw) as Partial<Persisted>) : {};
	} catch {
		return {};
	}
}

class SettingsStore {
	leftPanelCollapsed = $state(false);
	leftPanelWidth = $state(240);
	rightPanelWidth = $state(320);
	fileListView = $state<"path" | "tree">("path");

	constructor() {
		const stored = load();
		for (const side of ["left", "right"] as const) {
			const value = stored[`${side}PanelWidth`];
			if (typeof value === "number" && Number.isFinite(value)) this[`${side}PanelWidth`] = Math.max(220, Math.min(480, value));
		}
		if (typeof stored.leftPanelCollapsed === "boolean")
			this.leftPanelCollapsed = stored.leftPanelCollapsed;
		if (stored.fileListView === "path" || stored.fileListView === "tree")
			this.fileListView = stored.fileListView;
	}

	#persist() {
		if (typeof localStorage === "undefined") return;
		const snapshot: Persisted = {
			leftPanelCollapsed: this.leftPanelCollapsed,
			leftPanelWidth: this.leftPanelWidth,
			rightPanelWidth: this.rightPanelWidth,
			fileListView: this.fileListView,
		};
		localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot));
	}

	setPanelWidth(side: "left" | "right", value: number) {
		const other = side === "left" ? this.rightPanelWidth : this.leftPanelCollapsed ? 44 : this.leftPanelWidth;
		const available = typeof window === "undefined" ? 480 : window.innerWidth - other - 320;
		this[`${side}PanelWidth`] = Math.round(Math.max(220, Math.min(480, available, value)));
		this.#persist();
	}

	resetPanelWidth(side: "left" | "right") {
		this.setPanelWidth(side, side === "left" ? 240 : 320);
	}

	toggleLeftPanel() {
		this.leftPanelCollapsed = !this.leftPanelCollapsed;
		this.#persist();
	}

	setFileListView(value: "path" | "tree") {
		this.fileListView = value;
		this.#persist();
	}
}

export const settings = new SettingsStore();
