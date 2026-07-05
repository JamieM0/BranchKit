/** The backend-persisted Settings window's data — DESIGN_SPEC.md §13. Loaded once at startup and
 * written back instantly on every change (no Save button); the JSON file itself lives in the app
 * config dir and never contains a secret (`settings.rs`). This is distinct from the smaller
 * localStorage-backed `settings.svelte.ts` (combine-tracking-branches, file list view, left panel
 * collapse) and `theme.svelte.ts`, which already had working instant persistence before this
 * window existed — the Settings window's Appearance section simply also reads/writes those. */

import * as ipc from "$lib/ipc";
import type { AppSettings } from "$lib/types";

const DEFAULTS: AppSettings = {
	general: {
		autoFetchIntervalMinutes: 1,
		openLastReposOnLaunch: false,
		defaultCloneDir: null,
	},
	appearance: {
		theme: "system",
		graphDensity: "comfortable",
		dateStyle: "relative",
		showAvatars: true,
	},
	git: {
		defaultPullMode: "ff",
		pushTagsWithCommits: false,
		pruneOnFetch: true,
		combineTrackingBranches: true,
		commitSummaryGuideLength: 72,
	},
	ai: {
		enabled: false,
		provider: "local",
		ollamaBaseUrl: "http://localhost:11434",
		ollamaModel: null,
		remoteFormat: "openAi",
		remoteBaseUrl: "",
		remoteModel: "",
		style: "plain",
		maxDiffSizeKb: 8,
	},
};

class AppSettingsStore {
	current: AppSettings = $state(structuredClone(DEFAULTS));
	loaded = $state(false);

	async load() {
		try {
			this.current = await ipc.getSettings();
		} catch {
			this.current = structuredClone(DEFAULTS);
		}
		this.loaded = true;
	}

	async #persist() {
		try {
			// `$state.snapshot` — `this.current` is a Svelte reactive proxy, which the IPC bridge
			// (and `structuredClone` below) can't serialize directly.
			await ipc.updateSettings($state.snapshot(this.current));
		} catch {
			// Best-effort — a failed write shouldn't block the UI from reflecting the change locally.
		}
	}

	update(patch: (draft: AppSettings) => void) {
		// `structuredClone` throws `DataCloneError` on a Svelte reactive proxy — snapshot first.
		const next = structuredClone($state.snapshot(this.current));
		patch(next);
		this.current = next;
		void this.#persist();
	}
}

export const appSettings = new AppSettingsStore();
