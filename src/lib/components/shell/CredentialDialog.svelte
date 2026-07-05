<script lang="ts">
	import { credentialDialog } from "$lib/stores/credentialDialog.svelte";
	import { toasts } from "$lib/stores/toasts.svelte";
	import * as ipc from "$lib/ipc";

	/** The auth-failure → credential dialog → retry-once flow (ARCHITECTURE.md §8). Opened by
	 * `actions.ts` when a git error translates to the `open-credentials-settings` suggestion;
	 * saving here stores the credential in the OS keychain (never in this component's own state
	 * once submitted) and retries the operation that failed exactly once. */

	let username = $state("");
	let password = $state("");
	let busy = $state(false);
	let error = $state("");

	$effect(() => {
		if (credentialDialog.state) {
			username = "";
			password = "";
			error = "";
		}
	});

	async function submit() {
		const dialog = credentialDialog.state;
		if (!dialog || !username.trim() || !password) return;
		busy = true;
		error = "";
		try {
			await ipc.saveCredential(dialog.host, username.trim(), password);
			credentialDialog.dismiss();
			if (dialog.retry) await dialog.retry();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Escape") {
			e.preventDefault();
			credentialDialog.dismiss();
		}
	}
</script>

{#if credentialDialog.state}
	<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
	<div class="scrim" onclick={() => credentialDialog.dismiss()} onkeydown={handleKeydown} role="presentation">
		<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
		<div class="dialog" role="dialog" aria-modal="true" aria-label="Sign in to {credentialDialog.state.host}" onclick={(e) => e.stopPropagation()}>
			<h2>Sign in to {credentialDialog.state.host}</h2>
			<p class="hint">Saved to your OS keychain — BranchKit never stores this in a plain file.</p>

			<label class="field">
				<span>Host</span>
				<input type="text" bind:value={credentialDialog.state.host} />
			</label>
			<label class="field">
				<span>Username</span>
				<input type="text" bind:value={username} autocomplete="username" />
			</label>
			<label class="field">
				<span>Password / token</span>
				<input type="password" bind:value={password} autocomplete="current-password" />
			</label>

			{#if error}<p class="error">{error}</p>{/if}

			<div class="actions">
				<button type="button" class="secondary" onclick={() => credentialDialog.dismiss()}>Cancel</button>
				<button type="button" class="primary" disabled={busy || !username.trim() || !password} onclick={submit}>
					{busy ? "Saving…" : "Save & Retry"}
				</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 40%);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 150;
	}

	.dialog {
		width: min(400px, 90vw);
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 16px 48px rgb(0 0 0 / 35%);
		padding: var(--space-5);
	}

	h2 {
		margin: 0;
		font-size: 15px;
		font-weight: 600;
		color: var(--text);
	}

	.hint {
		margin: 0;
		font-size: 11px;
		color: var(--text-faint);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 2px;
		font-size: 12px;
		color: var(--text-muted);
	}

	.field input {
		font: inherit;
		font-size: 13px;
		padding: var(--space-2) var(--space-3);
		background: var(--raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		color: var(--text);
	}

	.error {
		margin: 0;
		font-size: 12px;
		color: var(--danger);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	button {
		font: inherit;
		font-size: 13px;
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-control);
		border: 1px solid transparent;
		cursor: pointer;
	}

	button.secondary {
		background: var(--raised);
		color: var(--text);
		border-color: var(--border);
	}

	button.primary {
		background: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}

	button.primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
