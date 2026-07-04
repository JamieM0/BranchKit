<script lang="ts">
	import type { RefInfo } from "$lib/types";

	/** A branch/tag label in the BRANCH/TAG column — DESIGN_SPEC.md §4.4. This is the read-only
	 * rendering; the rich interactions (drag-to-merge, ahead/behind fix-it popover, presence icons)
	 * arrive with the branch menus in prompt 7. The border tint matches the commit's lane color. */
	let { ref, colorIndex }: { ref: RefInfo; colorIndex: number } = $props();

	const isTag = $derived(ref.kind === "tag");
</script>

<span
	class="pill"
	class:tag={isTag}
	class:head={ref.isHead}
	style="--pill-lane: var(--lane-{colorIndex});"
	title={ref.upstream ? `tracks ${ref.upstream}` : ref.shortName}
>
	{#if ref.isHead}<span class="check" aria-hidden="true">✓</span>{/if}
	{#if isTag}<span class="tag-glyph" aria-hidden="true">⌗</span>{/if}
	<span class="name">{ref.shortName}</span>
	{#if ref.ahead > 0}<span class="ab ahead">↑{ref.ahead}</span>{/if}
	{#if ref.behind > 0}<span class="ab behind">↓{ref.behind}</span>{/if}
</span>

<style>
	.pill {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		max-width: 100%;
		padding: 1px 7px;
		border: 1px solid var(--pill-lane);
		border-radius: var(--radius-pill);
		background: var(--raised);
		color: var(--text);
		font-size: 11px;
		line-height: 16px;
		white-space: nowrap;
	}

	.pill.tag {
		border-radius: var(--radius-control);
		border-color: var(--text-muted);
		color: var(--text-muted);
	}

	.pill.head {
		font-weight: 600;
	}

	.check {
		color: var(--accent);
		font-size: 10px;
	}

	.tag-glyph {
		color: var(--text-muted);
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.ab {
		font-variant-numeric: tabular-nums;
		font-size: 10px;
	}

	.ab.ahead {
		color: var(--ahead);
	}

	.ab.behind {
		color: var(--behind);
	}
</style>
