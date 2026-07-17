<script lang="ts">
	import { github } from "$lib/stores/github.svelte";
	import { githubChecks } from "$lib/stores/githubChecks.svelte";
	import { prPanel } from "$lib/stores/prPanel.svelte";
	import * as ipc from "$lib/ipc";
	import * as actions from "$lib/actions";

	/** The PR side panel — DESIGN_SPEC.md §12: replaces the right panel when a PR is selected. */
	let { repoId }: { repoId: string } = $props();

	const pr = $derived(github.pullRequests.find((p) => p.number === prPanel.selectedNumber) ?? null);

	let mergeMethod: "merge" | "squash" | "rebase" = $state("merge");
	let merging = $state(false);
	let checkingOut = $state(false);

	$effect(() => {
		if (pr) void githubChecks.request(repoId, pr.headSha);
	});

	const checks = $derived(pr ? githubChecks.get(pr.headSha) : null);

	async function doMerge() {
		if (!pr) return;
		merging = true;
		const ok = await actions.mergePullRequest(repoId, pr.number, mergeMethod);
		merging = false;
		if (ok) await github.loadPullRequests(repoId);
	}

	async function doCheckout() {
		if (!pr) return;
		checkingOut = true;
		await actions.checkoutPrHead(repoId, pr.number);
		checkingOut = false;
	}
</script>

{#if pr}
	<div class="panel">
		<div class="head">
			<button type="button" class="back" onclick={() => prPanel.close()} aria-label="Close">← Back</button>
		</div>

		<div class="body">
			<h2>{pr.title} <span class="number">#{pr.number}</span></h2>
			<div class="chips">
				<span class="chip" class:draft={pr.draft}>{pr.draft ? "Draft" : pr.state}</span>
				{#if checks}<span class="chip ci {checks.summary}">{checks.summary}</span>{/if}
			</div>

			<p class="branches"><code>{pr.headRef}</code> → <code>{pr.baseRef}</code></p>

			{#if checks && checks.runs.length > 0}
				<div class="checks">
					{#each checks.runs as run (run.name)}
						<div class="check-row">
							<span class="dot {run.conclusion ?? run.status}"></span>
							<span class="name">{run.name}</span>
							<span class="state">{run.conclusion ?? run.status}</span>
						</div>
					{/each}
				</div>
			{/if}

			{#if pr.body}
				<pre class="description">{pr.body}</pre>
			{/if}

			<p class="meta">
				{pr.commentCount} {pr.commentCount === 1 ? "comment" : "comments"}
				{#if pr.reviewers.length > 0}
					· reviewers: {pr.reviewers.join(", ")}
				{/if}
			</p>
		</div>

		<div class="actions">
			<button type="button" class="primary" onclick={() => void ipc.openInBrowser(pr.htmlUrl)}>
				Open in browser
			</button>
			<button type="button" class="secondary" disabled={checkingOut} onclick={doCheckout}>
				{checkingOut ? "Checking out…" : "Checkout branch"}
			</button>
			{#if !pr.draft}
				<div class="merge-row">
					<select bind:value={mergeMethod}>
						<option value="merge">Merge commit</option>
						<option value="squash">Squash and merge</option>
						<option value="rebase">Rebase and merge</option>
					</select>
					<button type="button" class="danger-solid" disabled={merging} onclick={doMerge}>
						{merging ? "Merging…" : "Merge…"}
					</button>
				</div>
			{/if}
		</div>
	</div>
{/if}

<style>
	.panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.head {
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border-soft);
	}

	.back {
		border: none;
		background: none;
		color: var(--text-muted);
		font: inherit;
		font-size: 12px;
		cursor: pointer;
		padding: 0;
	}

	.back:hover {
		color: var(--text);
	}

	.body {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-3);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	h2 {
		margin: 0;
		font-size: 14px;
		font-weight: 600;
		color: var(--text);
	}

	.number {
		color: var(--text-faint);
		font-weight: 400;
	}

	.chips {
		display: flex;
		gap: var(--space-1);
	}

	.chip {
		font-size: 10px;
		padding: 2px 6px;
		border-radius: var(--radius-pill);
		background: var(--raised);
		color: var(--text-muted);
		text-transform: capitalize;
	}

	.chip.draft {
		color: var(--warn);
	}

	.chip.ci.success {
		color: var(--accent);
	}
	.chip.ci.failure {
		color: var(--danger);
	}
	.chip.ci.pending {
		color: var(--warn);
	}

	.branches {
		margin: 0;
		font-size: 12px;
		color: var(--text-muted);
	}

	.branches code {
		font-family: var(--font-mono);
		color: var(--text);
	}

	.checks {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-2);
		background: var(--raised);
		border-radius: var(--radius-control);
	}

	.check-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: 11px;
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: var(--radius-pill);
		background: var(--text-faint);
		flex-shrink: 0;
	}

	.dot.success {
		background: var(--accent);
	}
	.dot.failure,
	.dot.timed_out,
	.dot.action_required,
	.dot.cancelled {
		background: var(--danger);
	}
	.dot.in_progress,
	.dot.queued {
		background: var(--warn);
	}

	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.state {
		color: var(--text-muted);
	}

	.description {
		margin: 0;
		white-space: pre-wrap;
		font-family: inherit;
		font-size: 12px;
		color: var(--text-muted);
		max-height: 200px;
		overflow-y: auto;
	}

	.meta {
		margin: 0;
		font-size: 11px;
		color: var(--text-faint);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3);
		border-top: 1px solid var(--border-soft);
	}

	.merge-row {
		display: flex;
		gap: var(--space-2);
	}

	.merge-row select {
		flex: 1;
		min-width: 0;
	}

	button,
	select {
		font: inherit;
		font-size: 12px;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-control);
		border: 1px solid var(--border);
		background: var(--raised);
		color: var(--text);
		cursor: pointer;
	}

	button.primary {
		background: var(--accent);
		color: var(--bg);
		border-color: transparent;
		font-weight: 600;
	}

	button.danger-solid {
		background: var(--danger);
		border-color: transparent;
		color: var(--bg);
		font-weight: 600;
	}

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
