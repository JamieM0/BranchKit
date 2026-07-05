<script lang="ts">
	import ContextMenu, { type MenuItem } from "$lib/components/shell/ContextMenu.svelte";
	import * as actions from "$lib/actions";

	/** Stash row right-click menu — GITKRAKEN_WORKFLOWS.md §3.3, DESIGN_SPEC.md §4.5. Apply/Pop run
	 * straight away; Drop gets a lightweight inline confirm (not armed — dropping a stash is
	 * recoverable the same way any git object with a reflog is, so a soft confirm is enough). */
	let {
		selector,
		subject,
		repoId,
		x,
		y,
		onDismiss,
	}: {
		selector: string;
		subject: string;
		repoId: string;
		x: number;
		y: number;
		onDismiss: () => void;
	} = $props();

	let confirmingDrop = $state(false);

	function close() {
		onDismiss();
	}

	function run(fn: () => void | Promise<void>) {
		close();
		void fn();
	}

	const items: MenuItem[] = [
		{ type: "action", label: "Apply stash", run: () => run(() => actions.applyStash(repoId, selector)) },
		{ type: "action", label: "Pop stash", run: () => run(() => actions.popStash(repoId, selector, subject)) },
		{
			type: "action",
			label: "Drop stash…",
			danger: true,
			run: () => {
				confirmingDrop = true;
			},
		},
		{ type: "separator" },
		{ type: "action", label: "Copy patch to clipboard", run: () => run(() => actions.copyStashPatch(repoId, selector)) },
	];
</script>

{#if !confirmingDrop}
	<ContextMenu {items} {x} {y} onDismiss={close} ariaLabel="Stash actions" />
{:else}
	<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
	<div class="scrim" onclick={close}></div>
	<div class="panel" style="left: {x}px; top: {y}px;">
		<p class="text">Drop this stash? It won't be recoverable from the app afterward.</p>
		<div class="actions">
			<button type="button" onclick={close}>Cancel</button>
			<button type="button" class="danger-solid" onclick={() => run(() => actions.dropStash(repoId, selector))}>
				Drop stash
			</button>
		</div>
	</div>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 90;
	}

	.panel {
		position: fixed;
		z-index: 91;
		width: 260px;
		padding: var(--space-2);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.text {
		margin: 0;
		font-size: 12px;
		color: var(--text);
	}

	.actions {
		display: flex;
		gap: var(--space-1);
	}

	.actions button {
		flex: 1;
		justify-content: center;
		padding: var(--space-2);
		border-radius: var(--radius-control);
		border: 1px solid var(--border);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 12px;
		cursor: pointer;
	}

	.actions .danger-solid {
		background: var(--danger);
		border-color: var(--danger);
		color: #fff;
		font-weight: 600;
	}
</style>
