<script lang="ts">
	/** One conflict region, rendered as the "future file" fragment it will become — DESIGN_SPEC.md
	 * §9.2. Top to bottom: the `same in both` prefix as real numbered lines; the candidate card
	 * (raised inset, no line numbers, one labelled block per side with Keep + per-line hover ✓, or a
	 * deletion ghost); the kept stack as real numbered lines with pins + reorder handles; the `same
	 * in both` suffix. Everything is fluid until the file is confirmed — keeping/unkeeping animates
	 * (180ms) and every downstream line number rolls to match, because the parent recomputes
	 * `startLine` from the reducer's live `regionLineStarts`. */
	import { fly } from "svelte/transition";
	import { highlightLines } from "$lib/diff/highlight";
	import type { KeepPanelStore, ConflictRegionState } from "$lib/stores/keepPanel.svelte";
	import type { Side } from "$lib/types";

	let {
		store,
		regionIndex,
		startLine,
		oursLabel,
		theirsLabel,
		language,
		focused,
		editing,
		onStartEdit,
		onEndEdit,
	}: {
		store: KeepPanelStore;
		regionIndex: number;
		startLine: number;
		oursLabel: string;
		theirsLabel: string;
		language: string | undefined;
		focused: boolean;
		editing: boolean;
		onStartEdit: () => void;
		onEndEdit: () => void;
	} = $props();

	const region = $derived(store.regions[regionIndex] as ConflictRegionState);

	const prefixHtml = $derived(highlightLines(region.sameBothPrefix, language));
	const suffixHtml = $derived(highlightLines(region.sameBothSuffix, language));
	const oursHtml = $derived(highlightLines(region.oursLines, language));
	const theirsHtml = $derived(highlightLines(region.theirsLines, language));

	// Live line numbers — prefix first, then the kept stack (click order), then the suffix. Matches
	// the reducer's `resolvedLines` order exactly so the numbers we print are the file's real ones.
	const prefixStart = $derived(startLine);
	const keptStart = $derived(startLine + region.sameBothPrefix.length);
	const keptLineCount = $derived(region.kept.reduce((sum, k) => sum + k.lines.length, 0));
	const suffixStart = $derived(keptStart + keptLineCount);

	// Absolute first line number for each kept item, so a multi-line run numbers correctly.
	const keptItemStarts = $derived.by(() => {
		const starts: number[] = [];
		let n = keptStart;
		for (const item of region.kept) {
			starts.push(n);
			n += item.lines.length;
		}
		return starts;
	});

	const nothingKept = $derived(region.touched && region.kept.length === 0);

	function keptItemHtml(lines: string[]): string[] {
		return highlightLines(lines, language);
	}

	function pinLabel(source: Side | "edit"): string {
		if (source === "edit") return "hand-edited";
		return source === "ours" ? oursLabel : theirsLabel;
	}

	// --- Edit escape hatch (§9.2) — a local textarea seeded from the current kept result -----------
	let editText = $state("");
	$effect(() => {
		if (editing) {
			// Seed with what's kept so far; if the region is still untouched, offer both sides as raw
			// material to blend ("sometimes the right answer is a mix").
			const kept = region.kept.flatMap((k) => k.lines);
			const seed = kept.length > 0 || region.touched ? kept : [...region.oursLines, ...region.theirsLines];
			editText = seed.join("\n");
		}
	});

	function saveEdit() {
		store.editRegion(regionIndex, editText);
		onEndEdit();
	}
</script>

<div class="region" class:focused data-region={regionIndex}>
	<!-- same-in-both prefix: already real file lines, shown once, auto-kept (§9.3) -->
	{#each prefixHtml as html, i (i)}
		<div class="line same">
			<span class="num">{prefixStart + i}</span>
			<span class="pin same-pin" title="Same on both sides — kept automatically">●</span>
			<span class="text">{@html html}</span>
		</div>
	{/each}

	<!-- the candidate card: floats above the file (shadow), no line numbers -->
	<div class="card">
		{#if editing}
			<div class="edit-pane">
				<div class="edit-head">Editing kept result — a hand-written blend</div>
				<textarea
					class="edit-area"
					bind:value={editText}
					spellcheck="false"
					aria-label="Hand-edit the kept result"
				></textarea>
				<div class="edit-actions">
					<button type="button" class="ghost" onclick={onEndEdit}>Cancel</button>
					<button type="button" class="primary" onclick={saveEdit}>Save edit</button>
				</div>
			</div>
		{:else}
			{#if nothingKept}
				<div class="collapse-note">Nothing kept — these lines are removed. Undoable until you confirm.</div>
			{/if}

			{@render candidate("ours", oursLabel, "yours", region.oursLines, oursHtml)}
			{@render candidate("theirs", theirsLabel, "incoming", region.theirsLines, theirsHtml)}

			<div class="card-foot">
				<span class="hint">Keep one side, or both (they stack in the order you click).</span>
				<div class="foot-actions">
					{#if region.kept.length > 0}
						<button type="button" class="chip" onclick={() => store.unkeepAll(regionIndex)}>
							Unkeep all <kbd>u</kbd>
						</button>
					{/if}
					<button type="button" class="chip" onclick={onStartEdit}>Edit <kbd>e</kbd></button>
				</div>
			</div>
		{/if}
	</div>

	<!-- kept stack: real numbered file lines with pins + reorder handles (§9.2) -->
	{#each region.kept as item, i (item.id)}
		{@const itemHtml = keptItemHtml(item.lines)}
		<div class="kept-run" class:edit={item.source === "edit"} transition:fly={{ y: -6, duration: 180 }}>
			<div class="run-gutter">
				<span
					class="pin"
					class:ours={item.source === "ours"}
					class:theirs={item.source === "theirs"}
					class:hand={item.source === "edit"}
					title={`Kept from ${pinLabel(item.source)} — not yet confirmed`}
				>●</span>
				<div class="reorder">
					<button
						type="button"
						class="handle"
						disabled={i === 0}
						title="Move up"
						aria-label="Move kept block up"
						onclick={() => store.reorder(regionIndex, item.id, "up")}>↑</button>
					<button
						type="button"
						class="handle"
						disabled={i === region.kept.length - 1}
						title="Move down"
						aria-label="Move kept block down"
						onclick={() => store.reorder(regionIndex, item.id, "down")}>↓</button>
				</div>
				<button
					type="button"
					class="unpin"
					title="Unkeep"
					aria-label="Unkeep this block"
					onclick={() => store.unkeep(regionIndex, item.id)}>✕</button>
			</div>
			<div class="run-lines">
				{#if item.lines.length === 0}
					<div class="line deletion-kept">
						<span class="num">—</span>
						<span class="text muted">deletion kept ({pinLabel(item.source)} removes these lines)</span>
					</div>
				{:else}
					{#each itemHtml as html, li (li)}
						<div class="line kept-line">
							<span class="num">{keptItemStarts[i] + li}</span>
							<span class="text">{@html html}</span>
						</div>
					{/each}
				{/if}
			</div>
		</div>
	{/each}

	<!-- same-in-both suffix -->
	{#each suffixHtml as html, i (i)}
		<div class="line same">
			<span class="num">{suffixStart + i}</span>
			<span class="pin same-pin" title="Same on both sides — kept automatically">●</span>
			<span class="text">{@html html}</span>
		</div>
	{/each}
</div>

{#snippet candidate(side: Side, label: string, hint: string, lines: string[], html: string[])}
	<div class="block {side}">
		<div class="block-head">
			<span class="dot"></span>
			<span class="branch">From <strong>{label}</strong></span>
			<span class="side-hint">{hint}</span>
			<span class="spacer"></span>
			<button
				type="button"
				class="keep"
				onclick={() => store.keepBlock(regionIndex, side)}
			>
				{lines.length === 0 ? "Keep deletion" : "Keep"}
				<kbd>{side === "ours" ? "1" : "2"}</kbd>
			</button>
		</div>
		{#if lines.length === 0}
			<div class="ghost">
				<span class="ghost-glyph">⌦</span>
				<code>{label}</code> deletes these lines — keeping this side removes them.
			</div>
		{:else}
			<div class="block-lines">
				{#each html as lineHtml, i (i)}
					<div class="cand-line">
						<button
							type="button"
							class="tick"
							title="Keep just this line"
							aria-label="Keep this line"
							onclick={() => store.keepLine(regionIndex, side, i)}
						>✓</button>
						<span class="text">{@html lineHtml}</span>
					</div>
				{/each}
			</div>
		{/if}
	</div>
{/snippet}

<style>
	.region {
		border-radius: var(--radius-card);
		scroll-margin: 80px;
	}

	/* Focused = the region 1/2/b/u/e act on. A one-shot pulse on arrival (§9.2) plus a persistent
	 * accent rail down the card's left edge so the keyboard target is never ambiguous. */
	.region.focused {
		animation: pulse var(--motion-max);
	}

	.region.focused .card {
		border-left: 2px solid var(--accent);
	}

	@keyframes pulse {
		0% {
			box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 55%, transparent);
		}
		100% {
			box-shadow: 0 0 0 6px transparent;
		}
	}

	/* --- real file lines (prefix / kept / suffix) --- */
	.line {
		display: flex;
		align-items: baseline;
		font-family: var(--font-mono);
		font-size: var(--font-size-mono);
		line-height: 1.55;
	}

	.num {
		flex: 0 0 auto;
		width: 44px;
		padding-right: var(--space-2);
		text-align: right;
		color: var(--text-faint);
		user-select: none;
	}

	.line .text {
		white-space: pre-wrap;
		word-break: break-word;
	}

	.text.muted {
		color: var(--text-faint);
		font-style: italic;
	}

	.same .pin,
	.pin {
		flex: 0 0 auto;
		width: 12px;
		margin-right: 4px;
		text-align: center;
		font-size: 9px;
		line-height: 1;
	}

	.same-pin {
		color: var(--text-faint);
	}

	/* --- the floating candidate card --- */
	.card {
		margin: var(--space-2) 0 var(--space-2) 44px;
		background: var(--raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 6px 18px -8px rgba(0, 0, 0, 0.45);
		overflow: hidden;
	}

	.collapse-note {
		padding: var(--space-2) var(--space-3);
		font-size: 11px;
		color: var(--warn);
		background: color-mix(in srgb, var(--warn) 10%, var(--raised));
		border-bottom: 1px solid var(--border-soft);
	}

	.block {
		border-left: 3px solid transparent;
	}

	.block.ours {
		border-left-color: var(--info);
		background: color-mix(in srgb, var(--info) 12%, var(--raised));
	}

	.block.theirs {
		border-left-color: var(--lane-2);
		background: color-mix(in srgb, var(--lane-2) 12%, var(--raised));
	}

	.block + .block {
		border-top: 1px solid var(--border-soft);
	}

	.block-head {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-3);
		font-size: 11px;
	}

	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		flex: 0 0 auto;
	}

	.ours .dot {
		background: var(--info);
	}

	.theirs .dot {
		background: var(--lane-2);
	}

	.branch {
		color: var(--text);
	}

	.branch strong {
		font-weight: 600;
	}

	.side-hint {
		color: var(--text-faint);
		font-size: 10px;
	}

	.spacer {
		flex: 1;
	}

	.keep {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: 11px;
		font-weight: 600;
		padding: 2px var(--space-2);
		cursor: pointer;
	}

	.keep:hover {
		background: var(--overlay);
	}

	.block-lines {
		padding-bottom: var(--space-1);
	}

	.cand-line {
		display: flex;
		align-items: baseline;
		font-family: var(--font-mono);
		font-size: var(--font-size-mono);
		line-height: 1.55;
		padding-right: var(--space-3);
	}

	.tick {
		flex: 0 0 auto;
		width: 44px;
		padding-right: var(--space-2);
		text-align: right;
		border: none;
		background: none;
		color: transparent;
		font: inherit;
		cursor: pointer;
	}

	.cand-line:hover .tick {
		color: var(--accent);
	}

	.tick:hover {
		color: var(--accent) !important;
		font-weight: 700;
	}

	.ghost {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-3) var(--space-2) 48px;
		font-size: 11px;
		color: var(--text-muted);
	}

	.ghost-glyph {
		color: var(--danger);
	}

	.ghost code {
		font-family: var(--font-mono);
		color: var(--text);
	}

	.card-foot {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-3);
		border-top: 1px solid var(--border-soft);
		background: var(--surface);
	}

	.hint {
		font-size: 10px;
		color: var(--text-faint);
	}

	.foot-actions {
		display: flex;
		gap: var(--space-1);
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		border: 1px solid var(--border);
		border-radius: var(--radius-pill);
		background: var(--raised);
		color: var(--text-muted);
		font: inherit;
		font-size: 10px;
		padding: 1px var(--space-2);
		cursor: pointer;
	}

	.chip:hover {
		background: var(--overlay);
		color: var(--text);
	}

	kbd {
		font-family: var(--font-mono);
		font-size: 9px;
		padding: 0 3px;
		border: 1px solid var(--border);
		border-radius: 3px;
		background: var(--bg);
		color: var(--text-muted);
	}

	/* --- kept runs --- */
	.kept-run {
		display: flex;
		align-items: flex-start;
	}

	.run-gutter {
		flex: 0 0 auto;
		width: 44px;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 2px;
		padding-right: 4px;
	}

	.pin.ours {
		color: var(--info);
	}

	.pin.theirs {
		color: var(--lane-2);
	}

	.pin.hand {
		color: var(--warn);
	}

	.reorder {
		display: flex;
		flex-direction: column;
		line-height: 0.7;
	}

	.handle,
	.unpin {
		border: none;
		background: none;
		color: var(--text-muted);
		font-size: 9px;
		cursor: pointer;
		padding: 0;
	}

	.handle:disabled {
		color: var(--text-faint);
		cursor: default;
	}

	.handle:hover:not(:disabled),
	.unpin:hover {
		color: var(--text);
	}

	.run-lines {
		flex: 1;
		min-width: 0;
	}

	/* The pin always shows (marks "kept, not confirmed"); the reorder/unkeep controls fade in on
	 * hover so a settled kept run stays visually calm. */
	.reorder,
	.unpin {
		opacity: 0;
		transition: opacity var(--motion-hover);
	}

	.kept-run:hover .reorder,
	.kept-run:hover .unpin {
		opacity: 1;
	}

	.deletion-kept .text {
		padding-left: 0;
	}

	/* --- edit pane --- */
	.edit-pane {
		padding: var(--space-2) var(--space-3);
	}

	.edit-head {
		font-size: 10px;
		color: var(--warn);
		margin-bottom: var(--space-1);
	}

	.edit-area {
		width: 100%;
		min-height: 72px;
		resize: vertical;
		font-family: var(--font-mono);
		font-size: var(--font-size-mono);
		line-height: 1.55;
		color: var(--text);
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		padding: var(--space-2);
	}

	.edit-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-top: var(--space-2);
	}

	.edit-actions .ghost {
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--surface);
		color: var(--text-muted);
		font: inherit;
		font-size: 11px;
		padding: 2px var(--space-2);
		cursor: pointer;
	}

	.edit-actions .primary {
		border: 1px solid var(--warn);
		border-radius: var(--radius-control);
		background: var(--warn);
		color: var(--bg);
		font: inherit;
		font-size: 11px;
		font-weight: 600;
		padding: 2px var(--space-3);
		cursor: pointer;
	}
</style>
