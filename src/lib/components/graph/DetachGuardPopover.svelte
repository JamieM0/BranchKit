<script lang="ts">
	import { graphView } from "$lib/stores/graphView.svelte";

	/** The anchored guard shown on double-clicking a commit — DESIGN_SPEC.md §4.6 / §15.2. Never a
	 * modal: a small popover offering detached checkout or "Create branch here…", with a remembered
	 * "don't ask again". The actual checkout/branch git ops are wired in the operations prompt; this
	 * component only emits the intent. */
	let {
		sha,
		x,
		y,
		onCheckout,
		onCreateBranch,
		onDismiss,
	}: {
		sha: string;
		x: number;
		y: number;
		onCheckout: (sha: string) => void;
		onCreateBranch: (sha: string) => void;
		onDismiss: () => void;
	} = $props();

	let dontAsk = $state(graphView.detachDontAsk);

	function commit(action: (sha: string) => void) {
		if (dontAsk !== graphView.detachDontAsk) graphView.setDetachDontAsk(dontAsk);
		action(sha);
		onDismiss();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="scrim" onclick={onDismiss}></div>
<div
	class="popover"
	role="dialog"
	aria-label="Check out commit {sha.slice(0, 7)}"
	style="left: {x}px; top: {y}px;"
>
	<p class="title">Check out this commit?</p>
	<p class="body">You'll be in a detached state at <code>{sha.slice(0, 7)}</code>.</p>
	<div class="actions">
		<button type="button" class="primary" onclick={() => commit(onCheckout)}>Check out</button>
		<button type="button" onclick={() => commit(onCreateBranch)}>Create branch here…</button>
	</div>
	<label class="dont-ask">
		<input type="checkbox" bind:checked={dontAsk} />
		Don't ask again
	</label>
</div>

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 40;
	}

	.popover {
		position: fixed;
		z-index: 41;
		width: 260px;
		padding: var(--space-3);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.title {
		margin: 0;
		font-size: 13px;
		font-weight: 600;
		color: var(--text);
	}

	.body {
		margin: 0;
		font-size: 12px;
		color: var(--text-muted);
	}

	.body code {
		font-family: var(--font-mono);
		color: var(--text);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	button {
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 12px;
		cursor: pointer;
		text-align: left;
		transition: background var(--motion-hover);
	}

	button:hover {
		background: var(--overlay);
	}

	button.primary {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}

	button.primary:hover {
		background: var(--accent-dim);
	}

	.dont-ask {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: 11px;
		color: var(--text-muted);
		cursor: pointer;
	}
</style>
