<script lang="ts">
	import { githubChecks } from "$lib/stores/githubChecks.svelte";
	import { focusOnMount } from "$lib/focus";
	import { openInBrowser } from "$lib/ipc";

	/** The CI dot on a commit row — DESIGN_SPEC.md §12/§15.23: "tiny 6px dot right of message:
	 * green/red/amber-pulse pending; hover → checks list popover with per-check status + 'Open in
	 * browser'." Only rendered by the caller once GitHub is connected; requests its own status
	 * lazily on mount so only visible rows (this component only exists for virtualized visible rows)
	 * ever hit the network (ARCHITECTURE.md §11). */
	let { repoId, sha }: { repoId: string; sha: string } = $props();

	let popoverOpen = $state(false);
	let requestTimer: ReturnType<typeof setTimeout> | undefined;

	$effect(() => {
		// Debounced so a fast scroll through many rows doesn't fire a request per row passed
		// through — "batch on scroll-idle" (ARCHITECTURE.md §11).
		clearTimeout(requestTimer);
		requestTimer = setTimeout(() => {
			void githubChecks.request(repoId, sha);
		}, 300);
		return () => clearTimeout(requestTimer);
	});

	const status = $derived(githubChecks.get(sha));

	function stop(e: Event) {
		e.stopPropagation();
	}
</script>

{#if status && status.summary !== "none"}
	<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
	<span
		class="ci-dot {status.summary}"
		role="button"
		tabindex="-1"
		title="CI: {status.summary}"
		aria-label="CI status: {status.summary}"
		onclick={(e) => {
			stop(e);
			popoverOpen = !popoverOpen;
		}}
	></span>

	{#if popoverOpen}
		<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
		<div class="scrim" onclick={() => (popoverOpen = false)}></div>
		<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
		<div class="popover" role="dialog" aria-label="CI checks" tabindex="-1" onclick={stop} use:focusOnMount>
			{#each status.runs as run (run.name)}
				<div class="check-row">
					<span class="dot small {run.conclusion ?? run.status}"></span>
					<span class="name">{run.name}</span>
					<span class="state">{run.conclusion ?? run.status}</span>
					{#if run.htmlUrl}
						<button type="button" class="open" onclick={() => void openInBrowser(run.htmlUrl)}>
							Open
						</button>
					{/if}
				</div>
			{/each}
			{#if status.runs.length === 0}
				<p class="empty">No checks reported</p>
			{/if}
		</div>
	{/if}
{/if}

<style>
	.ci-dot {
		flex-shrink: 0;
		width: 6px;
		height: 6px;
		border-radius: var(--radius-pill);
		cursor: pointer;
		margin-left: var(--space-1);
	}

	.ci-dot.success {
		background: var(--accent);
	}

	.ci-dot.failure {
		background: var(--danger);
	}

	.ci-dot.pending {
		background: var(--warn);
		animation: pulse var(--motion-loop) ease-in-out infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.35;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.ci-dot.pending {
			animation: none;
		}
	}

	.scrim {
		position: fixed;
		inset: 0;
		z-index: 95;
	}

	.popover {
		position: absolute;
		z-index: 96;
		margin-top: var(--space-1);
		min-width: 220px;
		max-width: 320px;
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-2);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 12px 32px rgb(0 0 0 / 35%);
	}

	.check-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: 11px;
		padding: 3px 4px;
	}

	.dot.small {
		width: 6px;
		height: 6px;
		border-radius: var(--radius-pill);
		flex-shrink: 0;
		background: var(--text-faint);
	}

	.dot.small.success {
		background: var(--accent);
	}
	.dot.small.failure,
	.dot.small.timed_out,
	.dot.small.action_required,
	.dot.small.cancelled {
		background: var(--danger);
	}
	.dot.small.in_progress,
	.dot.small.queued {
		background: var(--warn);
	}

	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text);
	}

	.state {
		color: var(--text-muted);
	}

	.open {
		border: none;
		background: none;
		color: var(--info);
		font: inherit;
		font-size: 11px;
		cursor: pointer;
		padding: 0;
	}

	.open:hover {
		text-decoration: underline;
	}

	.empty {
		margin: 0;
		padding: var(--space-1);
		font-size: 11px;
		color: var(--text-faint);
	}
</style>
