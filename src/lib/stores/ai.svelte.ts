/** Runtime (non-persisted) AI state — DESIGN_SPEC.md §13/§7. `appSettings.current.ai` holds the
 * user's *configuration*; this store holds what's actually true right now: whether the local model
 * is downloaded, live download progress, whether a remote API key is stored, and the Ollama
 * model list/connection dot. None of this is written to `settings.json` (ARCHITECTURE.md §8: the
 * key itself lives in the OS keychain; the model's on-disk presence is just a filesystem fact). */

import * as ipc from "$lib/ipc";
import type { LocalDownloadProgress, LocalModelState } from "$lib/types";

class AiStore {
	localModelState: LocalModelState = $state("notDownloaded");
	downloading = $state(false);
	downloadProgress: LocalDownloadProgress | null = $state(null);
	downloadError: string | null = $state(null);

	remoteKeyConfigured = $state(false);

	ollamaModels: string[] = $state([]);
	ollamaConnected = $state(false);

	#unlistenProgress: (() => void) | null = null;

	/** Called once at app startup (`+page.svelte`, alongside `appSettings.load()`). */
	async init() {
		await Promise.all([this.refreshLocalModelState(), this.refreshRemoteKeyConfigured()]);
		if (!this.#unlistenProgress) {
			this.#unlistenProgress = await ipc.onLocalDownloadProgress((progress) => {
				this.downloadProgress = progress;
			});
		}
	}

	async refreshLocalModelState() {
		try {
			this.localModelState = await ipc.getLocalModelState();
		} catch {
			this.localModelState = "notDownloaded";
		}
	}

	async refreshRemoteKeyConfigured() {
		try {
			this.remoteKeyConfigured = await ipc.remoteApiKeyConfigured();
		} catch {
			this.remoteKeyConfigured = false;
		}
	}

	/** The model card's [Download] button (DESIGN_SPEC.md §13). */
	async download() {
		this.downloading = true;
		this.downloadError = null;
		this.downloadProgress = { phase: "Starting…", percent: 0, mbps: 0 };
		try {
			await ipc.downloadLocalModel();
		} catch (e) {
			this.downloadError = e instanceof Error ? e.message : String(e);
		} finally {
			this.downloading = false;
			this.downloadProgress = null;
			await this.refreshLocalModelState();
		}
	}

	/** The model card's Cancel button — the in-flight `download()` resolves shortly after. */
	async cancel() {
		await ipc.cancelLocalDownload();
	}

	/** The model card's Remove button — deletes the GGUF and returns to the Download state. */
	async remove() {
		await ipc.removeLocalModel();
		await this.refreshLocalModelState();
	}

	/** The Ollama URL field's connection dot + model dropdown (DESIGN_SPEC.md §13), refreshed on
	 * every URL edit and by its Refresh button. */
	async refreshOllama(baseUrl: string) {
		try {
			this.ollamaModels = await ipc.listOllamaModels(baseUrl);
			this.ollamaConnected = true;
		} catch {
			this.ollamaModels = [];
			this.ollamaConnected = false;
		}
	}
}

export const ai = new AiStore();
