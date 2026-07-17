<script lang="ts">
	import type { Pill } from "$lib/graph/pills";
	import * as actions from "$lib/actions";
	import { filter } from "$lib/stores/filter.svelte";
	import { worktreeDialog } from "$lib/stores/worktreeDialog.svelte";

	/** Branch pill / panel-row right-click menu — GITKRAKEN_WORKFLOWS §3.2, DESIGN_SPEC §4.4.
	 * Actions with backing ops run inline; Rename and "Create branch here" hand off to the graph's
	 * inline editors. Delete uses the armed force-confirm when the branch isn't fully merged
	 * (§4.6/§15.13) rather than a typed-word modal. */
	let {
		pill,
		repoId,
		currentBranch,
		x,
		y,
		onDismiss,
		onRename,
		onCreateBranch,
	}: {
		pill: Pill;
		repoId: string;
		currentBranch: string | null;
		x: number;
		y: number;
		onDismiss: () => void;
		onRename: (pill: Pill) => void;
		onCreateBranch: (sha: string) => void;
	} = $props();

	const sourceRef = $derived(pill.localBranch ?? pill.remoteRef ?? pill.name);
	const isCurrent = $derived(pill.localBranch !== null && pill.localBranch === currentBranch);

	let mode = $state<"menu" | "confirmDelete">("menu");
	let armed = $state(false);
	let armTimer: ReturnType<typeof setTimeout> | undefined;

	function close() {
		clearTimeout(armTimer);
		onDismiss();
	}

	async function run(fn: () => Promise<void> | void) {
		close();
		await fn();
	}

	function checkout() {
		if (pill.isRemoteOnly && pill.remoteRef) {
			void run(() => actions.checkoutRemote(repoId, pill.remoteRef!));
		} else if (pill.localBranch) {
			void run(() => actions.checkoutBranch(repoId, pill.localBranch!));
		}
	}

	async function startDelete() {
		if (!pill.localBranch) return;
		try {
			// Optimistic non-force delete; the guard fires only when unmerged.
			await actions.deleteBranch(repoId, pill.localBranch, false);
			close();
		} catch {
			// Not fully merged → escalate to the armed force-confirm (§4.6).
			mode = "confirmDelete";
			armed = false;
			armTimer = setTimeout(() => (armed = true), 400);
		}
	}

	function copyName() {
		void navigator.clipboard?.writeText(pill.name).catch(() => {});
		close();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="scrim" onclick={close} oncontextmenu={(e) => e.preventDefault()}></div>
<div class="menu" role="menu" aria-label="{pill.name} actions" style="left: {x}px; top: {y}px;">
	{#if mode === "menu"}
		{#if pill.localBranch || pill.isRemoteOnly}
			<button type="button" role="menuitem" onclick={checkout}>
				{pill.isRemoteOnly ? "Checkout (track)" : "Checkout"}
			</button>
		{/if}
		<button type="button" role="menuitem" onclick={() => run(() => onCreateBranch(pill.sha))}>
			Create branch here…
		</button>
		{#if pill.localBranch}
			<button type="button" role="menuitem" onclick={() => run(() => onRename(pill))}>Rename…</button>
			<button type="button" role="menuitem" class="danger" disabled={isCurrent} onclick={startDelete}>
				Delete{isCurrent ? " (checked out)" : ""}
			</button>
		{/if}
		<div class="sep"></div>
		{#if !isCurrent && currentBranch}
			<button
				type="button"
				role="menuitem"
				onclick={() => run(() => actions.mergeInto(repoId, sourceRef, currentBranch))}
			>
				Merge into <code>{currentBranch}</code>
			</button>
			<button
				type="button"
				role="menuitem"
				onclick={() => run(() => actions.rebaseOnto(repoId, currentBranch, sourceRef))}
			>
				Rebase <code>{currentBranch}</code> onto this
			</button>
		{/if}
		<div class="sep"></div>
		<button type="button" role="menuitem" onclick={() => run(() => worktreeDialog.open(sourceRef))}>
			Create worktree from this branch…
		</button>
		<div class="sep"></div>
		<button type="button" role="menuitem" onclick={copyName}>Copy branch name</button>
		{#if pill.localBranch}
			<button
				type="button"
				role="menuitem"
				onclick={() => run(() => filter.toggleHidden(pill.localBranch!))}
			>
				{filter.isHidden(pill.localBranch) ? "Show in graph" : "Hide in graph"}
			</button>
		{/if}
	{:else}
		<p class="confirm-text">
			<code>{pill.name}</code> has commits that aren't merged anywhere. Delete it anyway?
		</p>
		<div class="confirm-actions">
			<button type="button" onclick={close}>Cancel</button>
			<button
				type="button"
				class="danger-solid"
				disabled={!armed}
				onclick={() => run(() => actions.deleteBranch(repoId, pill.localBranch!, true))}
			>
				{armed ? "Delete branch" : "Hold…"}
			</button>
		</div>
	{/if}
</div>

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 42;
	}

	.menu {
		position: fixed;
		z-index: 43;
		min-width: 200px;
		max-width: 280px;
		padding: var(--space-1);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
		display: flex;
		flex-direction: column;
	}

	.menu button {
		display: flex;
		align-items: center;
		gap: 4px;
		width: 100%;
		padding: var(--space-2) var(--space-2);
		border: none;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text);
		font: inherit;
		font-size: 12px;
		text-align: left;
		cursor: pointer;
	}

	.menu button code {
		font-family: var(--font-mono);
		color: var(--text-muted);
	}

	.menu button:hover:not(:disabled) {
		background: var(--raised);
	}

	.menu button:disabled {
		color: var(--text-faint);
		cursor: default;
	}

	.menu button.danger {
		color: var(--danger);
	}

	.sep {
		height: 1px;
		margin: var(--space-1) var(--space-1);
		background: var(--border-soft);
	}

	.confirm-text {
		margin: 0;
		padding: var(--space-2);
		font-size: 12px;
		color: var(--text);
	}

	.confirm-text code {
		font-family: var(--font-mono);
	}

	.confirm-actions {
		display: flex;
		gap: var(--space-1);
		padding: 0 var(--space-1) var(--space-1);
	}

	.confirm-actions button {
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

	.confirm-actions .danger-solid {
		background: var(--danger);
		border-color: var(--danger);
		color: var(--bg);
		font-weight: 600;
	}

	.confirm-actions .danger-solid:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
