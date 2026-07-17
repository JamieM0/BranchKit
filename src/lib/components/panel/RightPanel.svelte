<script lang="ts">
	import { graphSelection } from "$lib/stores/graphSelection.svelte";
	import { prPanel } from "$lib/stores/prPanel.svelte";
	import { graph } from "$lib/stores/graph.svelte";
	import { commitExplanation } from "$lib/stores/commitExplanation.svelte";
	import { status } from "$lib/stores/status.svelte";
	import WorkingDirectoryPanel from "./WorkingDirectoryPanel.svelte";
	import CommitDetailPanel from "./CommitDetailPanel.svelte";
	import ComparePanel from "./ComparePanel.svelte";
	import PrDetailPanel from "./PrDetailPanel.svelte";
	import CreatePrPanel from "./CreatePrPanel.svelte";
	import CommitExplanationPanel from "./CommitExplanationPanel.svelte";

	/** The right panel — DESIGN_SPEC.md §3/§6.1. Mode follows the graph's selection: a Cmd+click
	 * pair → compare mode; a single commit selected → commit-detail mode; a PR selected (or the
	 * Create-PR form open, §12) takes the panel too; nothing selected (the default) →
	 * working-directory mode. A fresh graph selection always displaces the PR panel; selecting a PR
	 * (LeftPanel) clears the graph selection the same way. */
	const compare = $derived(graphSelection.compare);
	const selectedSha = $derived(graphSelection.selectedSha);
	const repoId = $derived(graph.repoId);
	const explaining = $derived(
		commitExplanation.repoId === repoId &&
			commitExplanation.sha !== null &&
			commitExplanation.sha === selectedSha,
	);

	let lastGraphKey: string | null = $state(null);
	$effect(() => {
		const key = compare ? `${compare.a}:${compare.b}` : selectedSha;
		if (key !== lastGraphKey) {
			lastGraphKey = key;
			if (key !== null) prPanel.close();
		}
	});

	// The panel only exists when it has something to say: a selection/compare/PR, or a dirty
	// working tree to stage & commit. A clean tree with nothing selected → no panel at all,
	// giving the graph the full width instead of showing "0 changes / no changes" boilerplate.
	const hasWip = $derived(status.report.entries.length > 0);
	const visible = $derived(
		compare !== null ||
			selectedSha !== null ||
			prPanel.creating ||
			prPanel.selectedNumber !== null ||
			hasWip,
	);
</script>

{#if !visible}
	<!-- nothing selected, nothing to commit — the graph gets the space -->
{:else}
<aside class="panel">
	{#if compare}
		<ComparePanel a={compare.a} b={compare.b} />
	{:else if prPanel.creating && repoId}
		<CreatePrPanel {repoId} />
	{:else if prPanel.selectedNumber !== null && repoId}
		<PrDetailPanel {repoId} />
	{:else if selectedSha}
		{#if explaining}
			<CommitExplanationPanel sha={selectedSha} />
		{:else}
			<CommitDetailPanel sha={selectedSha} />
		{/if}
	{:else}
		<WorkingDirectoryPanel />
	{/if}
</aside>
{/if}

<style>
	.panel {
		display: flex;
		flex-direction: column;
		width: 320px;
		min-width: 260px;
		height: 100%;
		background: var(--surface);
		border-left: 1px solid var(--border);
		overflow: hidden;
	}
</style>
