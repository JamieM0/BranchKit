<script lang="ts">
	import { graphSelection } from "$lib/stores/graphSelection.svelte";
	import WorkingDirectoryPanel from "./WorkingDirectoryPanel.svelte";
	import CommitDetailPanel from "./CommitDetailPanel.svelte";
	import ComparePanel from "./ComparePanel.svelte";

	/** The right panel — DESIGN_SPEC.md §3/§6.1. Mode follows the graph's selection: a Cmd+click
	 * pair → compare mode; a single commit selected → commit-detail mode; nothing selected (the
	 * default, and where a WIP-row click would land once that row exists) → working-directory
	 * mode. */
	const compare = $derived(graphSelection.compare);
	const selectedSha = $derived(graphSelection.selectedSha);
</script>

<aside class="panel">
	{#if compare}
		<ComparePanel a={compare.a} b={compare.b} />
	{:else if selectedSha}
		<CommitDetailPanel sha={selectedSha} />
	{:else}
		<WorkingDirectoryPanel />
	{/if}
</aside>

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
