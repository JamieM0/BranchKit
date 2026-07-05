<script lang="ts">
	/** The right-panel **Conflicted (n)** section — DESIGN_SPEC.md §9.1. Lists every conflicted file
	 * with a ‼ glyph and a per-file progress ring; clicking one opens it in the center Keep Panel.
	 * A confirmed file shows a full accent ring + ● (reopenable via the panel's Reset file). Shown
	 * only while an operation is in progress for the panel's repo. */
	import { keepSession } from "$lib/stores/keepSession.svelte";

	const files = $derived(keepSession.allFiles);

	function short(p: string): string {
		const parts = p.split("/");
		return parts[parts.length - 1];
	}

	function dir(p: string): string {
		const i = p.lastIndexOf("/");
		return i === -1 ? "" : p.slice(0, i + 1);
	}

	/** 0–100 fill for the ring: a confirmed file is full; otherwise resolved/total regions. */
	function pct(p: string): number {
		if (keepSession.isConfirmed(p)) return 100;
		const fp = keepSession.entryFor(p)?.store.fileProgress ?? { resolved: 0, total: 0 };
		return fp.total === 0 ? 100 : Math.round((fp.resolved / fp.total) * 100);
	}
</script>

{#if keepSession.conflictActive && files.length > 0}
	<section class="conflicted">
		<div class="section-head">
			<span>CONFLICTED <span class="count">{files.length}</span></span>
		</div>
		{#each files as p (p)}
			{@const confirmed = keepSession.isConfirmed(p)}
			<button
				type="button"
				class="row"
				class:active={p === keepSession.activePath}
				class:done={confirmed}
				onclick={() => keepSession.openFile(p)}
			>
				<span class="glyph" aria-hidden="true">‼</span>
				<span class="ring" style={`--pct:${pct(p)}%`} aria-hidden="true">
					{#if confirmed}<span class="ring-check">✓</span>{/if}
				</span>
				<span class="name">
					{#if dir(p)}<span class="dir">{dir(p)}</span>{/if}{short(p)}
				</span>
			</button>
		{/each}
	</section>
{/if}

<style>
	.conflicted {
		border-bottom: 1px solid var(--border-soft);
		background: color-mix(in srgb, var(--status-conflicted) 6%, var(--surface));
	}

	.section-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-2) var(--space-3);
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.04em;
		color: var(--status-conflicted);
	}

	.count {
		color: var(--text-faint);
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		width: 100%;
		border: none;
		background: none;
		text-align: left;
		padding: 3px var(--space-3);
		font: inherit;
		font-size: 12px;
		color: var(--text);
		cursor: pointer;
	}

	.row:hover {
		background: var(--overlay);
	}

	.row.active {
		background: color-mix(in srgb, var(--accent) 14%, transparent);
	}

	.glyph {
		flex: 0 0 auto;
		color: var(--status-conflicted);
		font-weight: 700;
	}

	.ring {
		flex: 0 0 auto;
		position: relative;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: conic-gradient(var(--accent) var(--pct), var(--border) 0);
	}

	/* punch a hole to make the pie a ring */
	.ring::after {
		content: "";
		position: absolute;
		inset: 3px;
		border-radius: 50%;
		background: var(--surface);
	}

	.row.done .ring {
		background: var(--accent);
	}

	.ring-check {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 9px;
		font-weight: 800;
		color: var(--bg);
		z-index: 1;
	}

	.name {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.dir {
		color: var(--text-faint);
	}

	.row.done .name {
		color: var(--text-muted);
	}
</style>
