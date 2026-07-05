<script lang="ts">
	import type { TreeNode } from "$lib/status/tree";
	import FileRow from "./FileRow.svelte";
	import { statusGlyph } from "$lib/status/glyphs";
	import type { FileStatusCode } from "$lib/types";

	/** Tree-mode rendering — DESIGN_SPEC.md §6.1 "folders collapsible with rolled-up badges". Each
	 * folder shows an aggregate badge like `✎4 −1` and can be collapsed; collapse state is local
	 * (not persisted) and keyed by folder path. */
	let {
		nodes,
		selectedPath = null,
		actionFor,
		repoId = null,
		repoRoot = null,
		onFileClick,
		onFileAction,
		onFileDiscard,
	}: {
		nodes: TreeNode[];
		selectedPath?: string | null;
		actionFor: (path: string) => "Stage" | "Unstage";
		repoId?: string | null;
		repoRoot?: string | null;
		onFileClick: (path: string) => void;
		onFileAction: (path: string) => void;
		onFileDiscard?: (path: string) => void;
	} = $props();

	let collapsed = $state<Record<string, boolean>>({});
	function toggle(path: string) {
		collapsed = { ...collapsed, [path]: !collapsed[path] };
	}

	const STATUS_ORDER: FileStatusCode[] = [
		"added",
		"modified",
		"renamed",
		"deleted",
		"updatedButUnmerged",
		"untracked",
	];
</script>

{#snippet folderBadge(counts: Partial<Record<FileStatusCode, number>>)}
	{#each STATUS_ORDER as code (code)}
		{#if counts[code]}
			{@const g = statusGlyph(code)}
			<span class="badge-item" style={`color: var(${g.colorVar})`}>{g.char}{counts[code]}</span>
		{/if}
	{/each}
{/snippet}

{#snippet tree(items: TreeNode[], depth: number)}
	{#each items as node (node.kind === "folder" ? `d:${node.path}` : `f:${node.row.path}`)}
		{#if node.kind === "folder"}
			<button
				type="button"
				class="folder"
				style={`padding-left: ${depth * 14 + 8}px`}
				onclick={() => toggle(node.path)}
			>
				<span class="chev" class:open={!collapsed[node.path]}>▸</span>
				<span class="folder-name">{node.name}</span>
				<span class="badge">{@render folderBadge(node.counts)}</span>
			</button>
			{#if !collapsed[node.path]}
				{@render tree(node.children, depth + 1)}
			{/if}
		{:else}
			<div style={`padding-left: ${depth * 14}px`}>
				<FileRow
					path={node.row.path}
					origPath={node.row.origPath}
					status={node.row.status}
					partial={node.row.partial}
					selected={selectedPath === node.row.path}
					actionLabel={actionFor(node.row.path)}
					{repoId}
					{repoRoot}
					onClick={() => onFileClick(node.row.path)}
					onAction={() => onFileAction(node.row.path)}
					onDiscard={onFileDiscard ? () => onFileDiscard(node.row.path) : undefined}
				/>
			</div>
		{/if}
	{/each}
{/snippet}

{@render tree(nodes, 0)}

<style>
	.folder {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		border: none;
		background: none;
		color: var(--text-muted);
		font: inherit;
		font-size: 11px;
		font-weight: 600;
		cursor: pointer;
		padding-top: 3px;
		padding-bottom: 3px;
		padding-right: var(--space-3);
	}

	.folder:hover {
		background: var(--raised);
	}

	.chev {
		display: inline-block;
		font-size: 8px;
		flex-shrink: 0;
		transition: transform var(--motion-hover);
	}

	.chev.open {
		transform: rotate(90deg);
	}

	.folder-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.badge {
		margin-left: auto;
		display: inline-flex;
		gap: 5px;
		font-variant-numeric: tabular-nums;
	}

	.badge-item {
		font-size: 10px;
		font-weight: 700;
	}
</style>
