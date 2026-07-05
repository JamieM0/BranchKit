<script lang="ts">
	import { RIGHT_GUTTER, ROW_HEIGHT } from "$lib/graph/geometry";
	import { graphView } from "$lib/stores/graphView.svelte";
	import { commitDraft } from "$lib/stores/commitDraft.svelte";
	import { graphSelection } from "$lib/stores/graphSelection.svelte";
	import { isModEvent } from "$lib/platform";
	import type { WipRow } from "$lib/graph/wip";
	import * as actions from "$lib/actions";

	/** The WIP row — DESIGN_SPEC.md §4.2. Sits at the top of the graph while the working tree differs
	 * from HEAD. Clicking the `// WIP` text opens an inline commit-summary editor two-way synced with
	 * the right-panel composer (both bind {@link commitDraft}); Enter hands focus to the description,
	 * Cmd+Enter commits. Clicking elsewhere on the row selects it → working-directory mode. */
	let {
		row,
		repoId,
		animateIn = false,
		onSelect,
	}: {
		row: WipRow;
		repoId: string | null;
		animateIn?: boolean;
		onSelect: () => void;
	} = $props();

	const counts = $derived(row.counts);

	/** Autofocus + select the inline input the moment it appears. */
	function focusInput(node: HTMLInputElement) {
		node.focus();
		node.select();
	}

	function startEdit(e: MouseEvent) {
		e.stopPropagation();
		// Show the composer (working-directory mode) so the two-way mirror is visible (§4.2), then
		// open the inline editor — the input autofocuses without stealing focus back to the graph.
		graphSelection.clear();
		commitDraft.editingWip = true;
	}

	/** Clicking anywhere on the WIP row — not just the `// WIP` text — selects working-directory
	 * mode AND opens + focuses the inline summary editor. There's nothing else a click on this row
	 * could sensibly mean, so make the whole row the affordance. */
	function handleRowClick() {
		onSelect();
		commitDraft.editingWip = true;
	}

	function onInputKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" && isModEvent(e)) {
			e.preventDefault();
			commitDraft.editingWip = false;
			if (repoId) void actions.commitPrimary(repoId);
		} else if (e.key === "Enter") {
			// Enter → move to the composer's description (§4.2).
			e.preventDefault();
			commitDraft.editingWip = false;
			commitDraft.requestDescriptionFocus();
		} else if (e.key === "Escape") {
			commitDraft.editingWip = false;
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div
	class="row wip"
	class:enter={animateIn}
	role="row"
	tabindex="-1"
	style="height: {ROW_HEIGHT}px;"
	onclick={handleRowClick}
>
	<div class="cell branch" style="width: {graphView.widths.branch}px;"></div>
	<div class="cell graph" style="width: {graphView.graphAuto}px;" aria-hidden="true"></div>

	<div class="cell message grow">
		{#if commitDraft.editingWip}
			<input
				class="wip-input"
				type="text"
				placeholder="Commit summary"
				aria-label="Commit summary"
				bind:value={commitDraft.summary}
				disabled={commitDraft.aiGenerating}
				onclick={(e) => e.stopPropagation()}
				onblur={() => (commitDraft.editingWip = false)}
				onkeydown={onInputKeydown}
				use:focusInput
			/>
			<span
				class="mini-counter"
				class:warn={commitDraft.counter === "warn"}
				class:danger={commitDraft.counter === "danger"}
			>
				{commitDraft.remaining}
			</span>
		{:else}
			<button type="button" class="wip-text" class:placeholder={!commitDraft.summary} onclick={startEdit}>
				{commitDraft.summary || "// WIP"}
			</button>
		{/if}

		<span class="badges" aria-label="Working-tree changes">
			{#if counts.modified > 0}<span class="badge mod" title="{counts.modified} modified">✎{counts.modified}</span>{/if}
			{#if counts.deleted > 0}<span class="badge del" title="{counts.deleted} deleted">−{counts.deleted}</span>{/if}
			{#if counts.added > 0}<span class="badge add" title="{counts.added} added">＋{counts.added}</span>{/if}
		</span>
	</div>

	{#if graphView.author}
		<div class="cell" style="width: {graphView.widths.author}px;"></div>
	{/if}
	{#if graphView.date}
		<div class="cell" style="width: {graphView.widths.date}px;"></div>
	{/if}
	{#if graphView.sha}
		<div class="cell" style="width: {graphView.widths.sha}px;"></div>
	{/if}

	<div class="cell gutter" style="width: {RIGHT_GUTTER}px;" aria-hidden="true"></div>
</div>

<style>
	.row {
		position: relative;
		display: flex;
		align-items: center;
		font-size: var(--font-size-ui);
		color: var(--text);
		cursor: pointer;
		background: transparent;
	}

	.row:hover {
		background: color-mix(in srgb, var(--raised) 45%, transparent);
	}

	/* Slide-in when the working tree first goes dirty (§4.2). Only applied on genuine appearance
	 * (the view gates the `enter` class), so scrolling never replays it. */
	.row.enter {
		animation: wip-slide-in var(--motion-panel);
	}

	@keyframes wip-slide-in {
		from {
			opacity: 0;
			transform: translateY(-6px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.cell {
		box-sizing: border-box;
		display: flex;
		align-items: center;
		height: 100%;
		padding: 0 var(--space-3);
		flex-shrink: 0;
		overflow: hidden;
		white-space: nowrap;
	}

	.cell.branch {
		justify-content: flex-end;
	}

	.cell.graph,
	.cell.gutter {
		padding: 0;
	}

	.cell.grow {
		flex: 1;
		min-width: 60px;
		gap: var(--space-2);
	}

	.wip-text {
		border: none;
		background: transparent;
		font: inherit;
		font-style: italic;
		color: var(--text);
		cursor: text;
		padding: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.wip-text.placeholder {
		color: var(--text-faint);
	}

	.wip-input {
		flex: 1;
		min-width: 0;
		box-sizing: border-box;
		padding: 2px 6px;
		border: 1px solid var(--accent);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font: inherit;
	}

	.wip-input:focus {
		outline: none;
	}

	.mini-counter {
		flex-shrink: 0;
		font-size: 10px;
		font-variant-numeric: tabular-nums;
		color: var(--text-faint);
	}

	.mini-counter.warn {
		color: var(--warn);
	}

	.mini-counter.danger {
		color: var(--danger);
		font-weight: 600;
	}

	.badges {
		display: inline-flex;
		gap: 6px;
		margin-left: auto;
		flex-shrink: 0;
		font-size: 11px;
		font-variant-numeric: tabular-nums;
	}

	.badge.mod {
		color: var(--warn);
	}

	.badge.del {
		color: var(--danger);
	}

	.badge.add {
		color: var(--accent);
	}
</style>
