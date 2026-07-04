<script lang="ts">
	import type { Pill } from "$lib/graph/pills";
	import type { Divergence } from "$lib/types";
	import { divergenceFor } from "$lib/graph/divergenceCache";

	/** The ahead/behind badge's hover tooltip — DESIGN_SPEC.md §4.4: lists up to 5 outgoing/incoming
	 * commit summaries ("↑ n to push" / "↓ m to pull"). Read-only; the click popover carries the
	 * fix-it actions. Data comes through the counts-keyed cache so hovering never re-runs `git log`
	 * while the counts are unchanged. */
	let {
		pill,
		repoId,
		x,
		y,
	}: {
		pill: Pill;
		repoId: string;
		x: number;
		y: number;
	} = $props();

	let div = $state<Divergence | null>(null);

	$effect(() => {
		if (!pill.localBranch) return;
		let cancelled = false;
		divergenceFor(repoId, pill.localBranch, pill.ahead, pill.behind)
			.then((d) => {
				if (!cancelled) div = d;
			})
			.catch(() => {});
		return () => {
			cancelled = true;
		};
	});
</script>

<div class="tip" role="tooltip" style="left: {x}px; top: {y}px;">
	{#if div === null}
		<p class="loading">Loading…</p>
	{:else}
		{#if pill.ahead > 0}
			<p class="head ahead">↑ {pill.ahead} to push</p>
			{#each div.outgoing.slice(0, 5) as c (c.sha)}
				<p class="line"><code>{c.sha.slice(0, 7)}</code> {c.subject}</p>
			{/each}
			{#if pill.ahead > 5}<p class="more">+{pill.ahead - 5} more</p>{/if}
		{/if}
		{#if pill.behind > 0}
			<p class="head behind">↓ {pill.behind} to pull</p>
			{#each div.incoming.slice(0, 5) as c (c.sha)}
				<p class="line"><code>{c.sha.slice(0, 7)}</code> {c.subject}</p>
			{/each}
			{#if pill.behind > 5}<p class="more">+{pill.behind - 5} more</p>{/if}
		{/if}
	{/if}
</div>

<style>
	.tip {
		position: fixed;
		z-index: 50;
		transform: translateX(-50%);
		width: 260px;
		max-width: 260px;
		padding: var(--space-2);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 6px 20px rgb(0 0 0 / 0.35);
		pointer-events: none;
	}

	.loading {
		margin: 0;
		font-size: 11px;
		color: var(--text-muted);
	}

	.head {
		margin: 0 0 2px;
		font-size: 11px;
		font-weight: 700;
	}

	.head.ahead {
		color: var(--ahead);
	}

	.head.behind {
		color: var(--behind);
		margin-top: var(--space-2);
	}

	.line {
		margin: 0;
		font-size: 11px;
		color: var(--text-muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.line code {
		font-family: var(--font-mono);
		color: var(--text-faint);
	}

	.more {
		margin: 2px 0 0;
		font-size: 10px;
		color: var(--text-faint);
	}
</style>
