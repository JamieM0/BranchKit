<script lang="ts">
	import { graphView, type SizableColumn } from "$lib/stores/graphView.svelte";
	import { RIGHT_GUTTER } from "$lib/graph/geometry";

	/** The graph's column header — titles, draggable width handles, and the gear menu that toggles
	 * the Author/Date/SHA columns. DESIGN_SPEC.md §4.1 / §15.27; widths + visibility persist via the
	 * graphView store. Row cells read the same store so they stay aligned. */

	let gearOpen = $state(false);
	let drag: { column: SizableColumn; startX: number; startWidth: number } | null = $state(null);

	function startResize(e: PointerEvent, column: SizableColumn) {
		e.preventDefault();
		(e.target as HTMLElement).setPointerCapture(e.pointerId);
		drag = { column, startX: e.clientX, startWidth: graphView.widths[column] };
	}

	function onResizeMove(e: PointerEvent) {
		if (!drag) return;
		graphView.setWidth(drag.column, drag.startWidth + (e.clientX - drag.startX));
	}

	function endResize(e: PointerEvent) {
		if (!drag) return;
		(e.target as HTMLElement).releasePointerCapture(e.pointerId);
		drag = null;
	}

	function resetWidth(column: SizableColumn) {
		graphView.resetWidth(column);
	}
</script>

<svelte:window onclick={() => (gearOpen = false)} />

<div class="header" role="row">
	<div class="cell" role="columnheader" style="width: {graphView.widths.branch}px;">
		<span>BRANCH / TAG</span>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="handle"
			class:active={drag?.column === "branch"}
			onpointerdown={(e) => startResize(e, "branch")}
			onpointermove={onResizeMove}
			onpointerup={endResize}
			ondblclick={() => resetWidth("branch")}
		></div>
	</div>

	<!-- The GRAPH column is unlabeled and auto-sizes to the lanes it actually draws (no manual
	     resize handle) — the lanes are self-explanatory and the column should never crowd MESSAGE. -->
	<div class="cell" role="columnheader" aria-label="Graph" style="width: {graphView.graphAuto}px;"></div>

	<div class="cell grow" role="columnheader"><span>MESSAGE</span></div>

	{#if graphView.author}
		<div class="cell" role="columnheader" style="width: {graphView.widths.author}px;">
			<span>AUTHOR</span>
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="handle"
				class:active={drag?.column === "author"}
				onpointerdown={(e) => startResize(e, "author")}
				onpointermove={onResizeMove}
				onpointerup={endResize}
				ondblclick={() => resetWidth("author")}
			></div>
		</div>
	{/if}

	{#if graphView.date}
		<div class="cell" role="columnheader" style="width: {graphView.widths.date}px;">
			<span>DATE</span>
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="handle"
				class:active={drag?.column === "date"}
				onpointerdown={(e) => startResize(e, "date")}
				onpointermove={onResizeMove}
				onpointerup={endResize}
				ondblclick={() => resetWidth("date")}
			></div>
		</div>
	{/if}

	{#if graphView.sha}
		<div class="cell" role="columnheader" style="width: {graphView.widths.sha}px;">
			<span>SHA</span>
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="handle"
				class:active={drag?.column === "sha"}
				onpointerdown={(e) => startResize(e, "sha")}
				onpointermove={onResizeMove}
				onpointerup={endResize}
				ondblclick={() => resetWidth("sha")}
			></div>
		</div>
	{/if}

	<div class="gear-cell" style="width: {RIGHT_GUTTER}px;">
		<button
			type="button"
			class="gear"
			aria-label="Choose columns"
			aria-expanded={gearOpen}
			onclick={(e) => {
				e.stopPropagation();
				gearOpen = !gearOpen;
			}}
		>
			⚙
		</button>
		{#if gearOpen}
			<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
			<div class="menu" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()}>
				<label><input type="checkbox" checked={graphView.author} onchange={() => graphView.toggle("author")} /> Author</label>
				<label><input type="checkbox" checked={graphView.date} onchange={() => graphView.toggle("date")} /> Date</label>
				<label><input type="checkbox" checked={graphView.sha} onchange={() => graphView.toggle("sha")} /> SHA</label>
			</div>
		{/if}
	</div>
</div>

<style>
	.header {
		display: flex;
		align-items: stretch;
		height: 26px;
		background: var(--surface);
		border-bottom: 1px solid var(--border);
		color: var(--text-faint);
		font-size: 10px;
		letter-spacing: 0.04em;
		user-select: none;
		flex-shrink: 0;
	}

	.cell {
		box-sizing: border-box;
		position: relative;
		display: flex;
		align-items: center;
		padding: 0 var(--space-3);
		flex-shrink: 0;
		overflow: hidden;
	}

	.cell.grow {
		flex: 1;
		min-width: 60px;
	}

	.handle {
		position: absolute;
		top: 0;
		right: -3px;
		width: 7px;
		height: 100%;
		cursor: col-resize;
		z-index: 1;
	}

	.handle::after {
		content: "";
		position: absolute;
		top: 4px;
		bottom: 4px;
		left: 3px;
		width: 1px;
		background: var(--border);
		transition: background var(--motion-hover);
	}

	.handle:hover::after,
	.handle.active::after {
		background: var(--accent);
	}

	.gear-cell {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.gear {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		border: none;
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 12px;
	}

	.gear:hover {
		background: var(--raised);
		color: var(--text);
	}

	.menu {
		position: absolute;
		top: 24px;
		right: 0;
		z-index: 30;
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 130px;
		padding: var(--space-2);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.3);
	}

	.menu label {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-1);
		font-size: 12px;
		letter-spacing: normal;
		color: var(--text);
		cursor: pointer;
		border-radius: var(--radius-control);
		text-transform: none;
	}

	.menu label:hover {
		background: var(--raised);
	}
</style>
