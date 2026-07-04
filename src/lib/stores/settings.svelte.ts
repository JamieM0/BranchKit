/** Persisted app settings — DESIGN_SPEC.md §13. Only the handful this prompt touches are wired:
 * "Combine tracking branches" (§5/§15.26, default ON) and the left-panel collapse state (§3/§5).
 * The full dynamic Settings window lands in a later prompt; this store is the seam it will grow
 * from. Persisted globally (per-machine), not per repo. */

interface Persisted {
	combineTrackingBranches: boolean;
	leftPanelCollapsed: boolean;
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
	combineTrackingBranches = $state(true);
	leftPanelCollapsed = $state(false);

	constructor() {
		const stored = load();
		if (typeof stored.combineTrackingBranches === "boolean")
			this.combineTrackingBranches = stored.combineTrackingBranches;
		if (typeof stored.leftPanelCollapsed === "boolean")
			this.leftPanelCollapsed = stored.leftPanelCollapsed;
	}

	#persist() {
		if (typeof localStorage === "undefined") return;
		const snapshot: Persisted = {
			combineTrackingBranches: this.combineTrackingBranches,
			leftPanelCollapsed: this.leftPanelCollapsed,
		};
		localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot));
	}

	setCombineTracking(value: boolean) {
		this.combineTrackingBranches = value;
		this.#persist();
	}

	toggleLeftPanel() {
		this.leftPanelCollapsed = !this.leftPanelCollapsed;
		this.#persist();
	}
}

export const settings = new SettingsStore();
