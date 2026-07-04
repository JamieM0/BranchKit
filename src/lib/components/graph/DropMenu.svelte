<script lang="ts">
	import type { Pill } from "$lib/graph/pills";
	import * as actions from "$lib/actions";

	/** The drop menu shown after dragging a pill onto another pill or a commit row — DESIGN_SPEC
	 * §4.4 / §15.6: "Merge X into Y" / "Rebase X onto Y" / "Fast-forward Y to X". Only actions that
	 * map onto the current checkout are offered (merge/rebase run against HEAD), so exactly one of
	 * source/target must be the current branch for the full set; otherwise only a ref-moving
	 * fast-forward is possible. */
	let {
		source,
		targetPill,
		targetSha,
		repoId,
		currentBranch,
		x,
		y,
		onDismiss,
	}: {
		source: Pill;
		targetPill: Pill | null;
		targetSha: string;
		repoId: string;
		currentBranch: string | null;
		x: number;
		y: number;
		onDismiss: () => void;
	} = $props();

	interface Option {
		label: string;
		run: () => Promise<void>;
	}

	const sourceRef = $derived(source.localBranch ?? source.remoteRef ?? source.name);
	const sourceLabel = $derived(source.name);
	const sourceIsCurrent = $derived(source.localBranch !== null && source.localBranch === currentBranch);

	const targetRef = $derived(
		targetPill ? (targetPill.localBranch ?? targetPill.remoteRef ?? targetPill.name) : targetSha,
	);
	const targetLabel = $derived(targetPill ? targetPill.name : targetSha.slice(0, 7));
	const targetBranch = $derived(targetPill?.localBranch ?? null);
	const targetIsCurrent = $derived(targetBranch !== null && targetBranch === currentBranch);

	function buildOptions(): Option[] {
		if (sourceIsCurrent) {
			return [
				{
					label: `Fast-forward ${sourceLabel} to ${targetLabel}`,
					run: () => actions.fastForward(repoId, sourceLabel, targetRef, true),
				},
				{
					label: `Rebase ${sourceLabel} onto ${targetLabel}`,
					run: () => actions.rebaseOnto(repoId, sourceLabel, targetRef),
				},
				{
					label: `Merge ${targetLabel} into ${sourceLabel}`,
					run: () => actions.mergeInto(repoId, targetRef, sourceLabel),
				},
			];
		}
		if (targetIsCurrent) {
			return [
				{
					label: `Fast-forward ${targetLabel} to ${sourceLabel}`,
					run: () => actions.fastForward(repoId, targetLabel, sourceRef, true),
				},
				{
					label: `Merge ${sourceLabel} into ${targetLabel}`,
					run: () => actions.mergeInto(repoId, sourceRef, targetLabel),
				},
				{
					label: `Rebase ${targetLabel} onto ${sourceLabel}`,
					run: () => actions.rebaseOnto(repoId, targetLabel, sourceRef),
				},
			];
		}
		if (targetBranch) {
			return [
				{
					label: `Fast-forward ${targetLabel} to ${sourceLabel}`,
					run: () => actions.fastForward(repoId, targetBranch, sourceRef, false),
				},
			];
		}
		return [];
	}

	const options = $derived(buildOptions());

	async function pick(option: Option) {
		onDismiss();
		await option.run();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="scrim" onclick={onDismiss}></div>
<div class="menu" role="menu" aria-label="Drop actions" style="left: {x}px; top: {y}px;">
	{#if options.length === 0}
		<p class="empty">Check out {targetLabel} or {sourceLabel} to merge or rebase them.</p>
	{:else}
		{#each options as option (option.label)}
			<button type="button" role="menuitem" onclick={() => pick(option)}>{option.label}</button>
		{/each}
	{/if}
</div>

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 42;
	}

	.menu {
		position: fixed;
		z-index: 43;
		min-width: 220px;
		max-width: 300px;
		padding: var(--space-1);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
		display: flex;
		flex-direction: column;
	}

	.menu button {
		width: 100%;
		padding: var(--space-2);
		border: none;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text);
		font: inherit;
		font-size: 12px;
		text-align: left;
		cursor: pointer;
	}

	.menu button:hover {
		background: var(--raised);
	}

	.empty {
		margin: 0;
		padding: var(--space-2);
		font-size: 12px;
		color: var(--text-muted);
	}
</style>
