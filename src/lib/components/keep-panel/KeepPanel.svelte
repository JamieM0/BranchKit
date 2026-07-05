<script lang="ts">
	/** The Keep Panel — DESIGN_SPEC.md §9's flagship. ONE panel that renders a conflicted file as
	 * the future file it will become: real line numbers on real lines, conflict regions as floating
	 * candidate cards, live renumbering as you keep. File tabs across the top (progress dots +
	 * 400ms-beat auto-advance driven by the session), a per-file bulk bar, n/p region navigation, a
	 * floating Confirm, and the full keyboard map (1/2/b/u/e/n/p/Cmd+Enter). Never renders a single
	 * git conflict marker — the reducer only ever sees structured regions (ARCHITECTURE.md §7.5). */
	import { keepSession } from "$lib/stores/keepSession.svelte";
	import { languageForPath } from "$lib/diff/language";
	import { highlightLines } from "$lib/diff/highlight";
	import { isModEvent } from "$lib/platform";
	import ConflictRegionCard from "./ConflictRegionCard.svelte";

	const path = $derived(keepSession.activePath);
	const entry = $derived(path ? keepSession.entryFor(path) : undefined);
	const store = $derived(entry?.store);
	const language = $derived(path ? languageForPath(path) : undefined);
	const confirmed = $derived(path ? keepSession.isConfirmed(path) : false);
	const oursLabel = $derived(store?.oursLabel ?? "");
	const theirsLabel = $derived(store?.theirsLabel ?? "");
	const starts = $derived(store?.regionLineStarts ?? []);
	const progress = $derived(store?.fileProgress ?? { resolved: 0, total: 0 });

	let focusedRegion = $state<number | null>(null);
	let editingRegion = $state<number | null>(null);
	let showCheat = $state(false);
	let scroller = $state<HTMLElement | null>(null);

	/** When the open file changes, drop stale per-region UI state and park focus on its first
	 * unresolved region so 1/2/b act somewhere sensible with no click needed. */
	let lastPath: string | null = null;
	$effect(() => {
		if (path !== lastPath) {
			lastPath = path;
			editingRegion = null;
			focusedRegion = store ? (store.unresolvedRegionIndices[0] ?? null) : null;
		}
	});

	function shortPath(p: string): string {
		const parts = p.split("/");
		return parts[parts.length - 1];
	}

	/** ● for each resolved conflict region, ○ for each pending one — the tab progress dots (§9.1),
	 * capped at 8 so a very conflicted file doesn't overflow the tab. A confirmed file collapses to
	 * a single ✓ instead of dots. */
	function tabDots(p: string): { dots: boolean[]; done: boolean } {
		const e = keepSession.entryFor(p);
		const fp = e ? e.store.fileProgress : { resolved: 0, total: 0 };
		const done = keepSession.isConfirmed(p) || (fp.total > 0 && fp.resolved === fp.total);
		const filled = keepSession.isConfirmed(p) ? fp.total : fp.resolved;
		const shown = Math.min(fp.total, 8);
		return { dots: Array.from({ length: shown }, (_, i) => i < filled), done };
	}

	function scrollToRegion(i: number) {
		const el = scroller?.querySelector(`[data-region="${i}"]`);
		el?.scrollIntoView({ behavior: "smooth", block: "center" });
	}

	/** n/p across the still-unresolved regions, with a card pulse (§9.2). Wraps around; if the
	 * currently-focused region is itself resolved we jump to the nearest unresolved in `dir`. */
	function navigate(dir: 1 | -1) {
		if (!store) return;
		const unresolved = store.unresolvedRegionIndices;
		if (unresolved.length === 0) return;
		let target: number;
		if (focusedRegion === null) {
			target = dir === 1 ? unresolved[0] : unresolved[unresolved.length - 1];
		} else if (dir === 1) {
			target = unresolved.find((i) => i > focusedRegion!) ?? unresolved[0];
		} else {
			target = [...unresolved].reverse().find((i) => i < focusedRegion!) ?? unresolved[unresolved.length - 1];
		}
		focusedRegion = target;
		scrollToRegion(target);
	}

	function keepSide(side: "ours" | "theirs") {
		if (store && focusedRegion !== null) store.keepBlock(focusedRegion, side);
	}

	async function confirm() {
		await keepSession.confirmActive();
	}

	async function resetFile() {
		await keepSession.resetActiveFile();
		focusedRegion = store ? (store.unresolvedRegionIndices[0] ?? null) : null;
	}

	function onKeydown(e: KeyboardEvent) {
		const target = e.target as HTMLElement | null;
		if (target && (target.tagName === "TEXTAREA" || target.tagName === "INPUT")) return;
		if (!store) return;

		if (isModEvent(e) && e.key === "Enter") {
			e.preventDefault();
			if (store.allResolved && !confirmed) void confirm();
			return;
		}
		if (isModEvent(e)) return;

		switch (e.key) {
			case "1":
				e.preventDefault();
				keepSide("ours");
				break;
			case "2":
				e.preventDefault();
				keepSide("theirs");
				break;
			case "b":
				e.preventDefault();
				if (focusedRegion !== null) {
					store.keepBlock(focusedRegion, "ours");
					store.keepBlock(focusedRegion, "theirs");
				}
				break;
			case "u":
				e.preventDefault();
				if (focusedRegion !== null) store.unkeepAll(focusedRegion);
				break;
			case "e":
				e.preventDefault();
				if (focusedRegion !== null) editingRegion = focusedRegion;
				break;
			case "n":
				e.preventDefault();
				navigate(1);
				break;
			case "p":
				e.preventDefault();
				navigate(-1);
				break;
			case "?":
				e.preventDefault();
				showCheat = !showCheat;
				break;
			case "Escape":
				if (showCheat) {
					e.preventDefault();
					showCheat = false;
				}
				break;
		}
	}

	const CHEATS: [string, string][] = [
		["1", "keep this region from the first branch"],
		["2", "keep from the second branch"],
		["b", "keep both (stacked in click order)"],
		["u", "unkeep everything in this region"],
		["e", "hand-edit the kept result"],
		["n / p", "next / previous unresolved region"],
		["⌘↵", "confirm this file"],
	];
</script>

<svelte:window onkeydown={onKeydown} />

<section class="keep-panel">
	<!-- file tabs -->
	{#if keepSession.allFiles.length > 1}
		<div class="tabs" role="tablist">
			{#each keepSession.allFiles as p (p)}
				{@const d = tabDots(p)}
				<button
					type="button"
					role="tab"
					class="tab"
					class:active={p === path}
					aria-selected={p === path}
					onclick={() => keepSession.openFile(p)}
				>
					<span class="dots" aria-hidden="true">
						{#if d.done}
							<span class="tab-check">✓</span>
						{:else}
							{#each d.dots as filled, i (i)}
								<span class="dot" class:filled></span>
							{/each}
						{/if}
					</span>
					<span class="tab-name">{shortPath(p)}</span>
				</button>
			{/each}
		</div>
	{/if}

	{#if path && store}
		<!-- per-file bulk bar + region navigation -->
		<div class="bulk-bar">
			<div class="file-id">
				<span class="conflict-glyph" title="Conflicted file">‼</span>
				<code>{path}</code>
				{#if confirmed}<span class="confirmed-tag">confirmed</span>{/if}
			</div>
			<div class="bulk-actions">
				<button type="button" class="bulk" onclick={() => store.keepAllFrom("ours")}>
					Keep all from {oursLabel}
				</button>
				<button type="button" class="bulk" onclick={() => store.keepAllFrom("theirs")}>
					Keep all from {theirsLabel}
				</button>
				<button type="button" class="bulk" onclick={() => void resetFile()}>Reset file</button>
				<div class="nav">
					<button type="button" class="chev" title="Previous unresolved (p)" onclick={() => navigate(-1)}>‹</button>
					<span class="nav-count">{progress.resolved}/{progress.total}</span>
					<button type="button" class="chev" title="Next unresolved (n)" onclick={() => navigate(1)}>›</button>
				</div>
			</div>
		</div>

		<!-- the single document -->
		<div class="document" bind:this={scroller} class:done={store.allResolved}>
			{#each store.regions as region, i (i)}
				{#if region.kind === "conflict"}
					<ConflictRegionCard
						{store}
						regionIndex={i}
						startLine={starts[i]}
						{oursLabel}
						{theirsLabel}
						{language}
						focused={focusedRegion === i}
						editing={editingRegion === i}
						onStartEdit={() => (editingRegion = i)}
						onEndEdit={() => (editingRegion = null)}
					/>
				{:else}
					{@const html = highlightLines(region.lines, language)}
					{#each html as lineHtml, li (li)}
						<div class="doc-line" class:auto={region.kind === "autoResolved"}>
							<span class="num">{starts[i] + li}</span>
							<span class="text">{@html lineHtml}</span>
						</div>
					{/each}
				{/if}
			{/each}

			{#if store.regions.length === 0}
				<p class="empty">No differences to resolve in this file.</p>
			{/if}
		</div>

		<!-- floating confirm -->
		<div class="float">
			<button
				type="button"
				class="cheat-chip"
				title="Keyboard shortcuts"
				onclick={() => (showCheat = !showCheat)}
			>?</button>
			{#if confirmed}
				<button type="button" class="confirm reopen" onclick={() => void resetFile()}>Reset file</button>
			{:else}
				<button
					type="button"
					class="confirm"
					disabled={!store.allResolved}
					title={store.allResolved ? "Confirm this file (⌘↵)" : "Resolve every region first"}
					onclick={() => void confirm()}
				>
					Confirm file
				</button>
			{/if}
		</div>

		{#if showCheat}
			<div class="cheat" role="dialog" aria-label="Keyboard shortcuts">
				<div class="cheat-head">Keyboard</div>
				{#each CHEATS as [key, desc] ([key, desc])}
					<div class="cheat-row"><kbd>{key}</kbd><span>{desc}</span></div>
				{/each}
			</div>
		{/if}
	{/if}
</section>

<style>
	.keep-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		position: relative;
		background: var(--bg);
	}

	/* --- tabs --- */
	.tabs {
		display: flex;
		gap: 2px;
		padding: var(--space-2) var(--space-3) 0;
		border-bottom: 1px solid var(--border);
		overflow-x: auto;
	}

	.tab {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		border: 1px solid var(--border);
		border-bottom: none;
		border-radius: var(--radius-control) var(--radius-control) 0 0;
		background: var(--surface);
		color: var(--text-muted);
		font: inherit;
		font-size: 12px;
		padding: var(--space-1) var(--space-3);
		cursor: pointer;
		white-space: nowrap;
	}

	.tab.active {
		background: var(--bg);
		color: var(--text);
		font-weight: 600;
	}

	.dots {
		display: inline-flex;
		align-items: center;
		gap: 3px;
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		border: 1px solid var(--text-faint);
	}

	.dot.filled {
		background: var(--accent);
		border-color: var(--accent);
	}

	.tab-check {
		color: var(--accent);
		font-size: 11px;
		font-weight: 700;
	}

	/* --- bulk bar --- */
	.bulk-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border-soft);
	}

	.file-id {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		font-size: 12px;
	}

	.conflict-glyph {
		color: var(--status-conflicted);
		font-weight: 700;
	}

	.file-id code {
		font-family: var(--font-mono);
		color: var(--text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.confirmed-tag {
		font-size: 10px;
		color: var(--accent);
		border: 1px solid var(--accent-dim);
		border-radius: var(--radius-pill);
		padding: 0 var(--space-2);
	}

	.bulk-actions {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex-shrink: 0;
	}

	.bulk {
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: 11px;
		padding: 2px var(--space-2);
		cursor: pointer;
	}

	.bulk:hover {
		background: var(--overlay);
	}

	.nav {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		margin-left: var(--space-1);
	}

	.chev {
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--surface);
		color: var(--text-muted);
		font-size: 14px;
		line-height: 1;
		padding: 0 6px;
		cursor: pointer;
	}

	.chev:hover {
		background: var(--overlay);
		color: var(--text);
	}

	.nav-count {
		font-size: 11px;
		font-variant-numeric: tabular-nums;
		color: var(--text-muted);
		min-width: 30px;
		text-align: center;
	}

	/* --- document --- */
	.document {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-3) var(--space-3) 96px;
	}

	.doc-line {
		display: flex;
		align-items: baseline;
		font-family: var(--font-mono);
		font-size: var(--font-size-mono);
		line-height: 1.55;
	}

	.doc-line .num {
		flex: 0 0 auto;
		width: 44px;
		padding-right: var(--space-2);
		text-align: right;
		color: var(--text-faint);
		user-select: none;
	}

	.doc-line.auto .num {
		color: var(--accent-dim);
	}

	.doc-line .text {
		white-space: pre-wrap;
		word-break: break-word;
	}

	.empty {
		padding: var(--space-4);
		color: var(--text-faint);
		font-size: 12px;
	}

	/* --- floating controls --- */
	.float {
		position: absolute;
		right: var(--space-4);
		bottom: var(--space-4);
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.cheat-chip {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		border: 1px solid var(--border);
		background: var(--raised);
		color: var(--text-muted);
		font-size: 14px;
		font-weight: 700;
		cursor: pointer;
	}

	.cheat-chip:hover {
		background: var(--overlay);
		color: var(--text);
	}

	.confirm {
		border: 1px solid var(--accent);
		border-radius: var(--radius-control);
		background: var(--accent);
		color: var(--bg);
		font: inherit;
		font-size: 13px;
		font-weight: 700;
		padding: var(--space-2) var(--space-4);
		cursor: pointer;
		box-shadow: 0 8px 20px -8px color-mix(in srgb, var(--accent) 60%, transparent);
	}

	.confirm:disabled {
		background: var(--raised);
		color: var(--text-faint);
		border-color: var(--border);
		box-shadow: none;
		cursor: not-allowed;
	}

	.confirm.reopen {
		background: var(--surface);
		color: var(--text);
		border-color: var(--border);
		box-shadow: none;
	}

	.cheat {
		position: absolute;
		right: var(--space-4);
		bottom: 64px;
		width: 260px;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 10px 30px -10px rgba(0, 0, 0, 0.5);
		padding: var(--space-2);
	}

	.cheat-head {
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.04em;
		color: var(--text-muted);
		padding: var(--space-1) var(--space-2);
	}

	.cheat-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: 2px var(--space-2);
		font-size: 11px;
		color: var(--text);
	}

	.cheat-row kbd {
		flex: 0 0 auto;
		min-width: 32px;
		text-align: center;
		font-family: var(--font-mono);
		font-size: 10px;
		padding: 1px 4px;
		border: 1px solid var(--border);
		border-radius: 3px;
		background: var(--bg);
		color: var(--text-muted);
	}
</style>
