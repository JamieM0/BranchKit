/** The backend-persisted Settings window's data — DESIGN_SPEC.md §13. Loaded once at startup and
 * written back instantly on every change (no Save button); the JSON file itself lives in the app
 * config dir and never contains a secret (`settings.rs`). This is the canonical source for every
 * user-facing setting; local stores may project it into render-specific state, but do not keep a
 * second persisted copy. */

import * as ipc from "$lib/ipc";
import type { AppSettings } from "$lib/types";
import { theme } from "$lib/stores/theme.svelte";

const DEFAULTS: AppSettings = {
	general: {
		autoFetchIntervalMinutes: 1,
		openLastReposOnLaunch: true,
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
		let migrated = false;
		if (typeof localStorage !== "undefined") {
			const legacyTheme = localStorage.getItem("branchkit:theme");
			if (legacyTheme === "system" || legacyTheme === "dark" || legacyTheme === "light") {
				this.current.appearance.theme = legacyTheme;
				localStorage.removeItem("branchkit:theme");
				migrated = true;
			}
			try {
				const key = "branchkit:settings";
				const legacy = JSON.parse(localStorage.getItem(key) ?? "null") as Record<string, unknown> | null;
				if (legacy && typeof legacy.combineTrackingBranches === "boolean") {
					this.current.git.combineTrackingBranches = legacy.combineTrackingBranches;
					delete legacy.combineTrackingBranches;
					localStorage.setItem(key, JSON.stringify(legacy));
					migrated = true;
				}
			} catch {
				// A malformed legacy cache is ignored just like the old stores ignored it.
			}
		}
		theme.hydrate(this.current.appearance.theme);
		this.loaded = true;
		if (migrated) await this.#persist();
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
		theme.hydrate(next.appearance.theme);
		void this.#persist();
	}
}

export const appSettings = new AppSettingsStore();
