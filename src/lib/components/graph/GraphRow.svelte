<script lang="ts">
	import RefPill from "./RefPill.svelte";
	import BranchNameEditor from "./BranchNameEditor.svelte";
	import CiDot from "./CiDot.svelte";
	import { RIGHT_GUTTER, ROW_HEIGHT } from "$lib/graph/geometry";
	import { graphView } from "$lib/stores/graphView.svelte";
	import { branchEdit } from "$lib/stores/branchEdit.svelte";
	import { filter, rowMatchesQuery } from "$lib/stores/filter.svelte";
	import { dnd } from "$lib/stores/dnd.svelte";
	import { graphNav } from "$lib/stores/graphNav.svelte";
	import { formatCommitDate } from "$lib/format";
	import { appSettings } from "$lib/stores/appSettings.svelte";
	import { github } from "$lib/stores/github.svelte";
	import { githubChecks } from "$lib/stores/githubChecks.svelte";
	import type { GraphViewRow } from "$lib/stores/graph.svelte";
	import type { Pill } from "$lib/graph/pills";

	/** One DOM row overlaying the canvas — DESIGN_SPEC.md §4.3 / §4.4 / §4.5. The GRAPH cell is a
	 * transparent spacer; the canvas behind draws that row's lanes/node/avatar. Pills, message and
	 * metadata are real, focusable, right-clickable DOM. The row is also a drop target for a dragged
	 * pill (drag pill onto row → merge/rebase/ff, §4.4). */
	let {
		row,
		selected = false,
		head = false,
		repoId,
		onSelect,
		onActivate,
		onHover,
		onCopySha,
		onPillSelect,
		onPillCheckout,
		onPillBadge,
		onPillMenu,
		onPillDrop,
		onRowDrop,
		onRowMenu,
	}: {
		row: GraphViewRow;
		selected?: boolean;
		head?: boolean;
		repoId: string | null;
		onSelect: (sha: string, event: MouseEvent) => void;
		onActivate: (sha: string, event: MouseEvent) => void;
		onHover: (sha: string | null) => void;
		onCopySha: (sha: string) => void;
		onPillSelect: (pill: Pill) => void;
		onPillCheckout: (pill: Pill) => void;
		onPillBadge: (pill: Pill, x: number, y: number) => void;
		onPillMenu: (pill: Pill, x: number, y: number) => void;
		onPillDrop: (pill: Pill) => void;
		onRowDrop: (sha: string) => void;
		onRowMenu?: (row: GraphViewRow, x: number, y: number) => void;
	} = $props();

	let expanded = $state(false);

	const isMerge = $derived(row.kind === "commit" && row.parents.length > 1);
	const descriptionPreview = $derived(
		row.kind === "commit" && row.meta?.body ? row.meta.body.split("\n")[0].trim() : "",
	);

	// Hide-eye: drop pills whose local branch is hidden from the graph (§5/§15.26).
	const shownPills = $derived(
		row.pills.filter((p) => !(p.localBranch && filter.isHidden(p.localBranch))),
	);
	const visiblePills = $derived(expanded ? shownPills : shownPills.slice(0, 2));
	const overflowCount = $derived(Math.max(0, shownPills.length - 2));
	const editingHere = $derived(branchEdit.isEditing(row.sha));

	// Dim (never remove) rows that don't match the universal filter — §15.24.
	const dimmed = $derived(
		filter.active &&
			!rowMatchesQuery(filter.query, {
				subject: row.kind === "commit" ? (row.meta?.subject ?? "") : row.subject,
				author: row.kind === "commit" ? (row.meta?.authorName ?? "") : "",
				sha: row.sha,
				refNames: row.pills.map((p) => p.name),
			}),
	);

	const dropTarget = $derived(dnd.dragging && dnd.overKey === row.sha);
	const glowing = $derived(graphNav.glowSha === row.sha);

	function copy(e: MouseEvent) {
		e.stopPropagation();
		onCopySha(row.sha);
	}

	function onRowDragOver(e: DragEvent) {
		if (!dnd.dragging) return;
		// Don't offer the whole-row target for the pill you started dragging from this very row.
		if (dnd.source && dnd.source.sha === row.sha) return;
		e.preventDefault();
		dnd.setOver(row.sha, e.clientX, e.clientY);
	}

	function onRowDragLeave() {
		if (dnd.overKey === row.sha) dnd.setOver(null);
	}

	function handleRowDrop(e: DragEvent) {
		if (!dnd.dragging) return;
		e.preventDefault();
		onRowDrop(row.sha);
	}

	function handleContextMenu(e: MouseEvent) {
		if (!onRowMenu) return;
		e.preventDefault();
		e.stopPropagation();
		onRowMenu(row, e.clientX, e.clientY);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div
	class="row"
	class:selected
	class:head
	class:merge={isMerge}
	class:stash={row.kind === "stash"}
	class:dimmed
	class:drop-target={dropTarget}
	class:glow={glowing}
	role="row"
	tabindex="-1"
	aria-selected={selected}
	style="height: {ROW_HEIGHT}px;"
	onclick={(e) => onSelect(row.sha, e)}
	ondblclick={(e) => onActivate(row.sha, e)}
	onmouseenter={() => onHover(row.sha)}
	onmouseleave={() => onHover(null)}
	ondragover={onRowDragOver}
	ondragleave={onRowDragLeave}
	ondrop={handleRowDrop}
	oncontextmenu={handleContextMenu}
>
	<div class="cell branch" style="width: {graphView.widths.branch}px;">
		{#if editingHere && branchEdit.mode === "create" && repoId}
			<BranchNameEditor {repoId} />
		{:else}
			{#each visiblePills as pill (pill.key)}
				{#if editingHere && branchEdit.mode === "rename" && branchEdit.oldName === pill.localBranch && repoId}
					<BranchNameEditor {repoId} />
				{:else}
					<RefPill
						{pill}
						colorIndex={row.node.colorIndex}
						{repoId}
						onSelect={onPillSelect}
						onCheckout={onPillCheckout}
						onBadge={onPillBadge}
						onMenu={onPillMenu}
						onDrop={onPillDrop}
					/>
				{/if}
			{/each}
			{#if overflowCount > 0 && !expanded}
				<button
					type="button"
					class="overflow"
					title="Show {overflowCount} more"
					onclick={(e) => {
						e.stopPropagation();
						expanded = true;
					}}>+{overflowCount}</button
				>
			{/if}
		{/if}
	</div>

	<div class="cell graph" style="width: {graphView.graphAuto}px;" aria-hidden="true"></div>

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
		{#if row.kind === "commit" && repoId && github.connected}
			<CiDot {repoId} sha={row.sha} />
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
			{row.kind === "commit" && row.meta
				? formatCommitDate(row.meta.authorTime, appSettings.current.appearance.dateStyle)
				: ""}
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
		background: transparent;
		transition: background var(--motion-hover), opacity var(--motion-hover);
	}

	/* The checked-out commit: accent edge bar + a faint accent wash across the whole row, so HEAD
	 * is findable at a glance even when its pill is scrolled out of the BRANCH/TAG column. */
	.row.head {
		background: color-mix(in srgb, var(--accent) 7%, transparent);
	}

	.row.head::before {
		content: "";
		position: absolute;
		left: 0;
		top: 0;
		bottom: 0;
		width: 3px;
		background: var(--accent);
	}

	.row:hover {
		background: color-mix(in srgb, var(--raised) 45%, transparent);
	}

	/* Selection reads unmistakably: stronger wash + a full inset ring (§4.3). */
	.row.selected {
		background: color-mix(in srgb, var(--accent) 18%, transparent);
		box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 55%, transparent);
	}

	/* Universal-filter dim — context preserved, never removed (§15.24). */
	.row.dimmed {
		opacity: 0.3;
	}

	/* Valid drop target while dragging a pill (§4.4). */
	.row.drop-target {
		background: color-mix(in srgb, var(--accent) 12%, transparent);
		box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 60%, transparent);
	}

	/* Panel-hover glow: this row is a branch tip being hovered in the left panel (§15.25). */
	.row.glow {
		background: color-mix(in srgb, var(--info) 14%, transparent);
	}

	.row.merge {
		color: var(--text-muted);
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
		border: none;
		border-radius: var(--radius-pill);
		padding: 0 5px;
		line-height: 16px;
		cursor: pointer;
	}

	.overflow:hover {
		color: var(--text);
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
