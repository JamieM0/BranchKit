/** The auth-failure → credential dialog → retry-once flow — ARCHITECTURE.md §8's "Simpler v1
 * fallback": on a 401/"Authentication failed" (translated by `error.rs` to the
 * `open-credentials-settings` suggestion), `actions.ts` opens this dialog instead of a dead
 * settings link; saving here calls `save_credential` (keychain, never this store) and then retries
 * the operation that failed exactly once. */

interface DialogState {
	host: string;
	retry?: () => void | Promise<void>;
}

class CredentialDialogStore {
	state: DialogState | null = $state(null);

	open(host: string, retry?: () => void | Promise<void>) {
		this.state = { host, retry };
	}

	dismiss() {
		this.state = null;
	}
}

export const credentialDialog = new CredentialDialogStore();
