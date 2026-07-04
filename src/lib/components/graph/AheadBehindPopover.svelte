<script lang="ts">
	import type { Pill } from "$lib/graph/pills";
	import type { Divergence } from "$lib/types";
	import { divergenceFor } from "$lib/graph/divergenceCache";
	import * as actions from "$lib/actions";
	import { graphNav } from "$lib/stores/graphNav.svelte";

	/** The ahead/behind badge's fix-it popover — DESIGN_SPEC.md §4.4 / §15.7 / §15.8. The badge is
	 * not just an indicator: this offers Pull / Push / view commits, and for a diverged branch the
	 * warn-tinted variant with rebase / merge / force-push-with-lease options, each with a one-line
	 * consequence. The click-to-jump commit list here is the "view commits" affordance; the up-to-5
	 * hover previews live in `BadgeTooltip` (§4.4). Pull/Push act on the checked-out branch, so when
	 * the pill isn't current we offer Checkout instead of a pull that would touch the wrong branch. */
	let {
		pill,
		repoId,
		x,
		y,
		isCurrent,
		onDismiss,
	}: {
		pill: Pill;
		repoId: string;
		x: number;
		y: number;
		isCurrent: boolean;
		onDismiss: () => void;
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

	function jump(sha: string) {
		graphNav.scrollTo(sha);
		onDismiss();
	}

	async function run(fn: () => Promise<void>) {
		onDismiss();
		await fn();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="scrim" onclick={onDismiss}></div>
<div
	class="popover"
	role="dialog"
	aria-label="Sync {pill.name}"
	style="left: {x}px; top: {y}px;"
>
	<p class="title">
		<span class="branch">{pill.name}</span>
		<span class="counts">
			{#if pill.ahead > 0}<span class="ahead">↑{pill.ahead}</span>{/if}
			{#if pill.behind > 0}<span class="behind">↓{pill.behind}</span>{/if}
		</span>
	</p>

	{#if !isCurrent}
		<p class="note">Checkout <code>{pill.name}</code> to pull or push it.</p>
		<div class="actions">
			<button
				type="button"
				class="primary"
				onclick={() => run(() => actions.checkoutBranch(repoId, pill.localBranch!))}
			>
				Checkout {pill.name}
			</button>
		</div>
	{:else if pill.diverged}
		<p class="note warn">
			Diverged — <code>{pill.name}</code> and its upstream both have new commits.
		</p>
		<div class="actions">
			<button type="button" onclick={() => run(() => actions.pull(repoId, "rebase", pill.name))}>
				Pull (rebase)
				<small>Replay your commits on top of theirs.</small>
			</button>
			<button type="button" onclick={() => run(() => actions.pull(repoId, "merge", pill.name))}>
				Pull (merge)
				<small>Create a merge commit joining both.</small>
			</button>
			<button type="button" class="danger" onclick={() => run(() => actions.push(repoId, true, pill.name))}>
				Force push (with lease)…
				<small>Overwrite the remote with your history.</small>
			</button>
		</div>
	{:else}
		<div class="actions">
			{#if pill.behind > 0}
				<button type="button" class="primary" onclick={() => run(() => actions.pull(repoId, "ff", pill.name))}>
					Pull {pill.behind} commit{pill.behind === 1 ? "" : "s"}
				</button>
			{/if}
			{#if pill.ahead > 0}
				<button type="button" class="primary" onclick={() => run(() => actions.push(repoId, false, pill.name))}>
					Push {pill.ahead} commit{pill.ahead === 1 ? "" : "s"}
				</button>
			{/if}
		</div>
	{/if}

	{#if div && (div.outgoing.length > 0 || div.incoming.length > 0)}
		<div class="previews">
			{#if div.outgoing.length > 0}
				<p class="preview-head ahead">↑ {div.outgoing.length} to push</p>
				{#each div.outgoing.slice(0, 5) as c (c.sha)}
					<button type="button" class="commit" onclick={() => jump(c.sha)}>
						<code>{c.sha.slice(0, 7)}</code><span class="subj">{c.subject}</span>
					</button>
				{/each}
			{/if}
			{#if div.incoming.length > 0}
				<p class="preview-head behind">↓ {div.incoming.length} to pull</p>
				{#each div.incoming.slice(0, 5) as c (c.sha)}
					<button type="button" class="commit" onclick={() => jump(c.sha)}>
						<code>{c.sha.slice(0, 7)}</code><span class="subj">{c.subject}</span>
					</button>
				{/each}
			{/if}
		</div>
	{/if}
</div>

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 40;
	}

	.popover {
		position: fixed;
		z-index: 41;
		width: 280px;
		max-height: 70vh;
		overflow-y: auto;
		padding: var(--space-3);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.title {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		margin: 0;
		font-size: 13px;
		font-weight: 600;
	}

	.branch {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.counts {
		flex-shrink: 0;
		font-variant-numeric: tabular-nums;
		font-size: 12px;
	}

	.ahead {
		color: var(--ahead);
	}
	.behind {
		color: var(--behind);
	}

	.note {
		margin: 0;
		font-size: 12px;
		color: var(--text-muted);
	}

	.note.warn {
		color: var(--warn);
	}

	.note code,
	.commit code {
		font-family: var(--font-mono);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.actions button {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 1px;
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 12px;
		cursor: pointer;
		text-align: left;
		transition: background var(--motion-hover);
	}

	.actions button:hover {
		background: var(--overlay);
	}

	.actions button small {
		color: var(--text-muted);
		font-size: 11px;
	}

	.actions button.primary {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--bg);
		font-weight: 600;
		flex-direction: row;
	}

	.actions button.primary:hover {
		background: var(--accent-dim);
	}

	.actions button.danger {
		border-color: color-mix(in srgb, var(--danger) 50%, var(--border));
	}

	.actions button.danger:hover {
		background: color-mix(in srgb, var(--danger) 16%, var(--raised));
	}

	.previews {
		border-top: 1px solid var(--border-soft);
		padding-top: var(--space-2);
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.preview-head {
		margin: var(--space-1) 0 2px;
		font-size: 11px;
		font-weight: 600;
	}

	.preview-head.ahead {
		color: var(--ahead);
	}
	.preview-head.behind {
		color: var(--behind);
	}

	.commit {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		border: none;
		background: none;
		color: var(--text-muted);
		font: inherit;
		font-size: 11px;
		padding: 2px 4px;
		border-radius: var(--radius-control);
		cursor: pointer;
		text-align: left;
		width: 100%;
	}

	.commit:hover {
		background: var(--raised);
		color: var(--text);
	}

	.commit .subj {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
