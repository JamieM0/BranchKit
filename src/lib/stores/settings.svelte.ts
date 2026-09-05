/** Lightweight local view preferences that are not part of the backend settings schema.
 * Persisted globally (per-machine), not per repo. */

interface Persisted {
	leftPanelCollapsed: boolean;
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
	fileListView = $state<"path" | "tree">("path");

	constructor() {
		const stored = load();
		if (typeof stored.leftPanelCollapsed === "boolean")
			this.leftPanelCollapsed = stored.leftPanelCollapsed;
		if (stored.fileListView === "path" || stored.fileListView === "tree")
			this.fileListView = stored.fileListView;
	}

	#persist() {
		if (typeof localStorage === "undefined") return;
		const snapshot: Persisted = {
			leftPanelCollapsed: this.leftPanelCollapsed,
			fileListView: this.fileListView,
		};
		localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot));
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
