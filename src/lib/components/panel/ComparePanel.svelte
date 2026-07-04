<script lang="ts">
	import { graph } from "$lib/stores/graph.svelte";
	import { graphSelection } from "$lib/stores/graphSelection.svelte";
	import { diffView } from "$lib/stores/diffView.svelte";
	import * as ipc from "$lib/ipc";
	import type { ChangedFile } from "$lib/types";
	import FileRow from "./FileRow.svelte";

	/** Compare mode — DESIGN_SPEC.md §4.3/§15.5. Cmd+click two commits in the graph to pair them;
	 * this shows "Comparing A ↔ B" with a swap button and the file list of `git diff A B`. */
	let { a, b }: { a: string; b: string } = $props();

	const repoId = $derived(graph.repoId);

	let files = $state<ChangedFile[]>([]);
	let loading = $state(false);

	$effect(() => {
		const id = repoId;
		const [x, y] = [a, b];
		if (!id) return;
		loading = true;
		files = [];
		void ipc
			.getDiffFiles(id, x, y)
			.then((result) => {
				if (a === x && b === y) files = result;
			})
			.finally(() => {
				if (a === x && b === y) loading = false;
			});
	});

	function openFile(file: ChangedFile) {
		diffView.open({
			path: file.path,
			origPath: file.origPath,
			source: { kind: "compare", a, b },
		});
	}
</script>

<div class="panel">
	<div class="header">
		<span class="title">
			Comparing <code>{a.slice(0, 7)}</code> ↔ <code>{b.slice(0, 7)}</code>
		</span>
		<button type="button" class="swap" title="Swap comparison direction" onclick={() => graphSelection.swapCompare()}>
			⇄
		</button>
	</div>

	<div class="files">
		{#if loading}
			<p class="empty">Loading…</p>
		{:else if files.length === 0}
			<p class="empty">No differences</p>
		{:else}
			{#each files as file (file.path)}
				<FileRow
					path={file.path}
					origPath={file.origPath}
					status={file.status}
					onClick={() => openFile(file)}
				/>
			{/each}
		{/if}
	</div>
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		padding: var(--space-3);
		border-bottom: 1px solid var(--border-soft);
		font-size: 12px;
		color: var(--text-muted);
	}

	.title code {
		font-family: var(--font-mono);
		color: var(--text);
	}

	.swap {
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font-size: 13px;
		padding: 2px 8px;
		cursor: pointer;
		flex-shrink: 0;
	}

	.swap:hover {
		background: var(--overlay);
	}

	.files {
		flex: 1;
		overflow-y: auto;
	}

	.empty {
		margin: 0;
		padding: var(--space-3) var(--space-4);
		font-size: 11px;
		color: var(--text-faint);
	}
</style>
