<script lang="ts">
	import RefPill from "./RefPill.svelte";
	import { RIGHT_GUTTER, ROW_HEIGHT } from "$lib/graph/geometry";
	import { graphView } from "$lib/stores/graphView.svelte";
	import { relativeTime } from "$lib/format";
	import type { GraphViewRow } from "$lib/stores/graph.svelte";

	/** One DOM row overlaying the canvas — DESIGN_SPEC.md §4.3 / §4.5. The GRAPH cell is a
	 * transparent spacer; the canvas behind draws that row's lanes, node and avatar. Everything else
	 * (pills, message, metadata) is real, focusable, right-clickable DOM. */
	let {
		row,
		selected = false,
		head = false,
		onSelect,
		onActivate,
		onHover,
		onCopySha,
	}: {
		row: GraphViewRow;
		selected?: boolean;
		head?: boolean;
		onSelect: (sha: string, event: MouseEvent) => void;
		onActivate: (sha: string, event: MouseEvent) => void;
		onHover: (sha: string | null) => void;
		onCopySha: (sha: string) => void;
	} = $props();

	const isMerge = $derived(row.kind === "commit" && row.parents.length > 1);
	const descriptionPreview = $derived(
		row.kind === "commit" && row.meta?.body ? row.meta.body.split("\n")[0].trim() : "",
	);
	const visiblePills = $derived(row.refs.slice(0, 2));
	const overflowCount = $derived(Math.max(0, row.refs.length - 2));

	function copy(e: MouseEvent) {
		e.stopPropagation();
		onCopySha(row.sha);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div
	class="row"
	class:selected
	class:head
	class:merge={isMerge}
	class:stash={row.kind === "stash"}
	role="row"
	tabindex="-1"
	aria-selected={selected}
	style="height: {ROW_HEIGHT}px;"
	onclick={(e) => onSelect(row.sha, e)}
	ondblclick={(e) => onActivate(row.sha, e)}
	onmouseenter={() => onHover(row.sha)}
	onmouseleave={() => onHover(null)}
>
	<div class="cell branch" style="width: {graphView.widths.branch}px;">
		{#each visiblePills as ref (ref.name)}
			<RefPill {ref} colorIndex={row.node.colorIndex} />
		{/each}
		{#if overflowCount > 0}
			<span class="overflow" title="{overflowCount} more refs">+{overflowCount}</span>
		{/if}
	</div>

	<div class="cell graph" style="width: {graphView.widths.graph}px;" aria-hidden="true"></div>

	<div class="cell message grow">
		{#if row.kind === "stash"}
			<span class="stash-label">{row.subject || "stash"}</span>
		{:else if row.meta}
			<span class="subject">{row.meta.subject}</span>
			{#if descriptionPreview}
				<span class="description"> — {descriptionPreview}</span>
			{/if}
		{:else}
			<span class="pending">Loading…</span>
		{/if}
		<button type="button" class="copy-sha" aria-label="Copy commit SHA" title="Copy SHA" onclick={copy}>
			⧉ {row.sha.slice(0, 7)}
		</button>
	</div>

	{#if graphView.author}
		<div class="cell author" style="width: {graphView.widths.author}px;">
			{row.kind === "commit" ? (row.meta?.authorName ?? "") : ""}
		</div>
	{/if}
	{#if graphView.date}
		<div class="cell date" style="width: {graphView.widths.date}px;">
			{row.kind === "commit" && row.meta ? relativeTime(row.meta.authorTime) : ""}
		</div>
	{/if}
	{#if graphView.sha}
		<div class="cell sha" style="width: {graphView.widths.sha}px;">
			{row.kind === "commit" ? row.sha.slice(0, 7) : ""}
		</div>
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
		/* Translucent so the canvas lanes behind stay visible through the highlight. */
		background: transparent;
		transition: background var(--motion-hover);
	}

	/* HEAD gets a left accent bar in addition to the canvas node ring, so the current commit is not
	   signalled by color/ring alone — DESIGN_SPEC.md §14. */
	.row.head::before {
		content: "";
		position: absolute;
		left: 0;
		top: 0;
		bottom: 0;
		width: 2px;
		background: var(--accent);
	}

	.row:hover {
		background: color-mix(in srgb, var(--raised) 45%, transparent);
	}

	.row.selected {
		background: color-mix(in srgb, var(--accent) 16%, transparent);
	}

	.row.merge {
		color: var(--text-muted);
	}

	.cell {
		/* border-box so a cell's configured px width is its real width — the canvas positions lane
		   nodes at exactly `branchWidth + laneCenterX`, which only lines up if padding is included. */
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
		gap: 4px;
		justify-content: flex-end;
	}

	.cell.graph,
	.cell.gutter {
		padding: 0;
	}

	.cell.grow {
		flex: 1;
		min-width: 60px;
	}

	.overflow {
		font-size: 10px;
		color: var(--text-muted);
		background: var(--raised);
		border-radius: var(--radius-pill);
		padding: 0 5px;
		line-height: 16px;
	}

	.message {
		gap: 0;
		position: relative;
	}

	.subject {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.description {
		color: var(--text-faint);
		overflow: hidden;
		text-overflow: ellipsis;
		flex-shrink: 1;
		min-width: 0;
	}

	.stash-label {
		font-style: italic;
		color: var(--text-muted);
	}

	.pending {
		color: var(--text-faint);
		font-style: italic;
	}

	.copy-sha {
		margin-left: auto;
		flex-shrink: 0;
		display: inline-flex;
		align-items: center;
		gap: 3px;
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text-muted);
		font-family: var(--font-mono);
		font-size: 10px;
		padding: 1px 5px;
		cursor: pointer;
		opacity: 0;
		transition: opacity var(--motion-hover), color var(--motion-hover);
	}

	.row:hover .copy-sha {
		opacity: 1;
	}

	.copy-sha:hover {
		color: var(--text);
		border-color: var(--accent);
	}

	.author,
	.date,
	.sha {
		color: var(--text-muted);
		font-size: 12px;
	}

	.sha {
		font-family: var(--font-mono);
	}

	.date {
		font-variant-numeric: tabular-nums;
	}
</style>
