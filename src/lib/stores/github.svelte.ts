/** GitHub connection state — DESIGN_SPEC.md §12, ARCHITECTURE.md §11. Everything that touches
 * "are we signed in, and as whom" lives here so every surface (Integrations settings, PULL
 * REQUESTS panel, CI dots, Create-PR toast) reads the same source of truth and degrades invisibly
 * together when not connected. */

import * as ipc from "$lib/ipc";
import type { DeviceCode, GithubUser, PullRequest } from "$lib/types";

class GithubStore {
	user: GithubUser | null = $state(null);
	checked = $state(false);

	// --- device flow sign-in (§12) ---
	deviceCode: DeviceCode | null = $state(null);
	polling = $state(false);
	signInError: string | null = $state(null);

	connected = $derived(this.user !== null);

	async checkConnection() {
		try {
			this.user = await ipc.getGithubConnection();
		} catch {
			this.user = null;
		}
		this.checked = true;
	}

	async beginSignIn() {
		this.signInError = null;
		this.deviceCode = null;
		try {
			this.deviceCode = await ipc.startDeviceFlow();
		} catch (e) {
			this.signInError = e instanceof Error ? e.message : String(e);
			return;
		}
		this.polling = true;
		try {
			const user = await ipc.pollDeviceFlow(
				this.deviceCode.deviceCode,
				this.deviceCode.interval,
				this.deviceCode.expiresIn,
			);
			this.user = user;
			this.deviceCode = null;
		} catch (e) {
			this.signInError = e instanceof Error ? e.message : String(e);
		} finally {
			this.polling = false;
		}
	}

	cancelSignIn() {
		this.deviceCode = null;
		this.polling = false;
		this.signInError = null;
	}

	async signOut() {
		await ipc.githubSignOut();
		this.user = null;
	}

	// --- pull requests (LEFT panel §5, PR panel §12) ---
	pullRequests: PullRequest[] = $state([]);
	#loadedForRepo: string | null = null;

	async loadPullRequests(repoId: string) {
		if (!this.connected) {
			this.pullRequests = [];
			return;
		}
		try {
			this.pullRequests = await ipc.listPullRequests(repoId);
			this.#loadedForRepo = repoId;
		} catch {
			// Non-GitHub origin, rate-limited, or a transient network blip — the panel just shows the
			// quiet "Connect GitHub" / empty state rather than erroring (§12's "degrade invisibly").
			this.pullRequests = [];
		}
	}

	reset() {
		this.pullRequests = [];
		this.#loadedForRepo = null;
	}

	get loadedForRepo() {
		return this.#loadedForRepo;
	}
}

export const github = new GithubStore();
