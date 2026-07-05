<script lang="ts">
	import { graph } from "$lib/stores/graph.svelte";
	import { status } from "$lib/stores/status.svelte";
	import { commitDraft } from "$lib/stores/commitDraft.svelte";
	import { toasts } from "$lib/stores/toasts.svelte";
	import { stagedRows, unstagedRows } from "$lib/status/sections";
	import { isModEvent } from "$lib/platform";
	import * as ipc from "$lib/ipc";
	import * as actions from "$lib/actions";

	/** Commit composer — DESIGN_SPEC.md §7. Lives at the bottom of the working-directory panel. The
	 * summary/description bind to the shared {@link commitDraft} store so the graph's WIP-row inline
	 * editor (§4.2) mirrors them live. The AI button is present-but-disabled until a provider is
	 * configured (wired in a later prompt, §7). */

	const repoId = $derived(graph.repoId);
	const branch = $derived(
		graph.head?.detached ? graph.head.sha.slice(0, 7) : (graph.head?.branch ?? "…"),
	);

	const entries = $derived(status.report.entries);
	const stagedCount = $derived(stagedRows(entries).length);
	const totalChanged = $derived(
		new Set([
			...unstagedRows(entries).map((r) => r.path),
			...stagedRows(entries).map((r) => r.path),
		]).size,
	);
	const hasStaged = $derived(stagedCount > 0);
	const hasWip = $derived(totalChanged > 0);

	// Amend-of-a-pushed-commit warning (§15.15): HEAD is on the remote when the branch tracks an
	// upstream and isn't ahead of it (its tip commit already exists on origin).
	const upstream = $derived(status.report.branch.upstream);
	const pushed = $derived(upstream !== null && status.report.branch.ahead === 0);

	let sweeping = $state(false);
	let caretOpen = $state(false);
	let descEl = $state<HTMLTextAreaElement | null>(null);
	let summaryEl = $state<HTMLInputElement | null>(null);

	// --- amend ---------------------------------------------------------------

	async function toggleAmend() {
		if (commitDraft.amend) {
			commitDraft.disableAmend();
			return;
		}
		const headSha = graph.head?.sha;
		if (!repoId || !headSha) return;
		let meta = graph.metaBySha[headSha] ?? null;
		if (!meta) {
			try {
				meta = (await ipc.getCommitMeta(repoId, [headSha]))[0] ?? null;
			} catch {
				/* fall through with empty prefill */
			}
		}
		commitDraft.enableAmend(meta?.subject ?? "", meta?.body ?? "");
	}

	// --- commit --------------------------------------------------------------

	async function runCommit(opts: { stageAllFirst: boolean }) {
		if (!repoId || !commitDraft.canCommit) return;
		caretOpen = false;
		const ok = await actions.commit(repoId, opts);
		if (ok) {
			sweeping = true;
			setTimeout(() => (sweeping = false), 240);
		}
	}

	/** The stateful primary action (§15.16): amend or staged files commit as-is; WIP-only stages
	 * everything first. */
	function runPrimary() {
		if (commitDraft.amend || hasStaged) void runCommit({ stageAllFirst: false });
		else if (hasWip) void runCommit({ stageAllFirst: true });
	}

	function onSummaryKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" && isModEvent(e)) {
			e.preventDefault();
			runPrimary();
		} else if (e.key === "Enter") {
			// Plain Enter in the one-line summary moves to the description (§4.2 parity).
			e.preventDefault();
			descEl?.focus();
		}
	}

	function onDescriptionKeydown(e: KeyboardEvent) {
		if (e.key === "Enter" && isModEvent(e)) {
			e.preventDefault();
			runPrimary();
		}
	}

	function aiTooltip() {
		return "Configure an AI provider in Settings → AI to generate commit messages";
	}

	// Grow the description to 8 lines, then scroll (§7). Re-runs on programmatic changes too
	// (amend prefill, post-commit reset).
	$effect(() => {
		const el = descEl;
		void commitDraft.description;
		if (!el) return;
		el.style.height = "auto";
		const max = 8 * 18 + 12; // ~8 lines at the mono line-height plus padding.
		el.style.height = `${Math.min(el.scrollHeight, max)}px`;
	});

	// Honour the WIP row's Enter → focus-description hand-off (§4.2).
	let lastFocusToken = 0;
	$effect(() => {
		const token = commitDraft.focusDescriptionToken;
		if (token !== lastFocusToken) {
			lastFocusToken = token;
			descEl?.focus();
		}
	});
</script>

<div class="composer" class:sweeping>
	<div class="summary-field">
		<input
			bind:this={summaryEl}
			class="summary"
			class:warn={commitDraft.counter === "warn"}
			class:danger={commitDraft.counter === "danger"}
			type="text"
			placeholder="Commit summary"
			aria-label="Commit summary"
			bind:value={commitDraft.summary}
			onkeydown={onSummaryKeydown}
		/>
		<span
			class="counter"
			class:warn={commitDraft.counter === "warn"}
			class:danger={commitDraft.counter === "danger"}
			aria-label="{commitDraft.remaining} characters to the {72}-char guide"
			title="Characters remaining before the 72-char summary guide"
		>
			{commitDraft.remaining}
		</span>
		<button
			type="button"
			class="ai"
			disabled
			aria-label="Generate commit message with AI"
			title={aiTooltip()}
		>
			✨
		</button>
	</div>

	<div class="desc-wrap">
		<textarea
			bind:this={descEl}
			class="description"
			placeholder="Description (optional) — Markdown allowed"
			aria-label="Commit description"
			rows="2"
			bind:value={commitDraft.description}
			onkeydown={onDescriptionKeydown}
		></textarea>
		<div class="ruler" aria-hidden="true"></div>
	</div>

	<label class="amend">
		<input type="checkbox" checked={commitDraft.amend} onchange={() => void toggleAmend()} />
		Amend previous commit
	</label>

	{#if commitDraft.amend && pushed}
		<p class="pushed-warning">
			This commit is on <code>{upstream}</code> — amending will require a force push.
		</p>
	{/if}

	{#if commitDraft.amend}
		<button
			type="button"
			class="primary"
			disabled={!commitDraft.canCommit}
			onclick={() => void runCommit({ stageAllFirst: false })}
		>
			Amend commit
		</button>
	{:else if hasStaged}
		<button
			type="button"
			class="primary"
			disabled={!commitDraft.canCommit}
			onclick={() => void runCommit({ stageAllFirst: false })}
		>
			Commit {stagedCount} file{stagedCount === 1 ? "" : "s"} to <code>{branch}</code>
		</button>
	{:else if hasWip}
		<div class="split">
			<button
				type="button"
				class="primary split-main"
				disabled={!commitDraft.canCommit}
				onclick={() => void runCommit({ stageAllFirst: true })}
			>
				Stage all &amp; commit
			</button>
			<button
				type="button"
				class="primary split-caret"
				aria-label="More commit options"
				aria-expanded={caretOpen}
				onclick={() => (caretOpen = !caretOpen)}
			>
				▾
			</button>
			{#if caretOpen}
				<div class="caret-menu" role="menu">
					<button
						type="button"
						role="menuitem"
						disabled={!hasStaged || !commitDraft.canCommit}
						title={hasStaged ? "" : "Nothing staged yet"}
						onclick={() => void runCommit({ stageAllFirst: false })}
					>
						Commit staged only
					</button>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.composer {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3);
		border-top: 1px solid var(--border);
		background: var(--surface);
	}

	.summary-field {
		position: relative;
		display: flex;
		align-items: center;
	}

	.summary {
		flex: 1;
		box-sizing: border-box;
		/* Room for the counter + AI button on the right edge. */
		padding: 6px 56px 6px var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 13px;
	}

	.summary:focus {
		outline: none;
		border-color: var(--accent);
	}

	.summary.warn {
		border-color: var(--warn);
	}

	.summary.danger {
		border-color: var(--danger);
	}

	.counter {
		position: absolute;
		right: 30px;
		font-size: 11px;
		font-variant-numeric: tabular-nums;
		color: var(--text-faint);
		pointer-events: none;
	}

	.counter.warn {
		color: var(--warn);
	}

	.counter.danger {
		color: var(--danger);
		font-weight: 600;
	}

	.ai {
		position: absolute;
		right: 6px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		border: none;
		border-radius: var(--radius-control);
		background: transparent;
		font-size: 13px;
		cursor: not-allowed;
		opacity: 0.5;
	}

	.desc-wrap {
		position: relative;
	}

	.description {
		display: block;
		width: 100%;
		box-sizing: border-box;
		padding: 6px var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font-family: var(--font-mono);
		font-size: var(--font-size-mono);
		line-height: 18px;
		resize: none;
		overflow-y: auto;
	}

	.description:focus {
		outline: none;
		border-color: var(--accent);
	}

	/* Soft ruler at column 72 — DESIGN_SPEC.md §7. `ch` is relative to this element's own mono font,
	 * matched to the textarea, so it lands on the 72nd character column. */
	.ruler {
		position: absolute;
		top: 6px;
		bottom: 6px;
		left: calc(var(--space-2) + 72ch);
		width: 1px;
		font-family: var(--font-mono);
		font-size: var(--font-size-mono);
		background: var(--border);
		pointer-events: none;
	}

	.amend {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 12px;
		color: var(--text-muted);
		cursor: pointer;
	}

	.pushed-warning {
		margin: 0;
		padding: 6px var(--space-2);
		border-radius: var(--radius-control);
		background: color-mix(in srgb, var(--warn) 16%, var(--surface));
		border: 1px solid color-mix(in srgb, var(--warn) 45%, transparent);
		font-size: 11px;
		color: var(--text);
	}

	.pushed-warning code {
		font-family: var(--font-mono);
		color: var(--warn);
	}

	.primary {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 4px;
		padding: 7px var(--space-3);
		border: none;
		border-radius: var(--radius-control);
		background: var(--accent);
		color: var(--bg);
		font: inherit;
		font-size: 13px;
		font-weight: 600;
		cursor: pointer;
	}

	.primary code {
		font-family: var(--font-mono);
		font-weight: 700;
	}

	.primary:hover:not(:disabled) {
		background: var(--accent-dim);
	}

	.primary:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}

	.split {
		position: relative;
		display: flex;
	}

	.split-main {
		flex: 1;
		border-top-right-radius: 0;
		border-bottom-right-radius: 0;
	}

	.split-caret {
		width: 30px;
		padding: 7px 0;
		border-top-left-radius: 0;
		border-bottom-left-radius: 0;
		border-left: 1px solid color-mix(in srgb, var(--bg) 30%, transparent);
	}

	.caret-menu {
		position: absolute;
		right: 0;
		bottom: calc(100% + 4px);
		min-width: 180px;
		padding: 4px;
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		background: var(--overlay);
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
		z-index: 5;
	}

	.caret-menu button {
		display: block;
		width: 100%;
		padding: 6px var(--space-2);
		border: none;
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--text);
		font: inherit;
		font-size: 12px;
		text-align: left;
		cursor: pointer;
	}

	.caret-menu button:hover:not(:disabled) {
		background: var(--raised);
	}

	.caret-menu button:disabled {
		color: var(--text-faint);
		cursor: not-allowed;
	}

	/* Success sweep — a 240ms accent shimmer across the composer after a commit (§7). */
	.composer.sweeping {
		position: relative;
	}

	.composer.sweeping::after {
		content: "";
		position: absolute;
		inset: 0;
		pointer-events: none;
		background: linear-gradient(
			100deg,
			transparent 0%,
			color-mix(in srgb, var(--accent) 30%, transparent) 50%,
			transparent 100%
		);
		background-size: 220% 100%;
		animation: sweep 240ms ease-out;
	}

	@keyframes sweep {
		from {
			background-position: 120% 0;
		}
		to {
			background-position: -120% 0;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.composer.sweeping::after {
			animation: none;
		}
	}
</style>
