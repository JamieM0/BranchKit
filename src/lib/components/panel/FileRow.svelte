<script lang="ts">
	import type { FileStatusCode } from "$lib/types";
	import { statusGlyph } from "$lib/status/glyphs";
	import { fileName, parentPath } from "$lib/status/sections";

	/** Shared file row — DESIGN_SPEC.md §6.1. Reused by the working-directory sections, the
	 * commit-detail changed-file list and compare mode's file list, so status glyphs, rename
	 * display and hover actions stay identical everywhere a file can be listed. */
	let {
		path,
		origPath = null,
		status,
		partial = false,
		selected = false,
		actionLabel = null,
		onClick,
		onAction,
		onDiscard,
	}: {
		path: string;
		origPath?: string | null;
		status: FileStatusCode;
		partial?: boolean;
		selected?: boolean;
		actionLabel?: "Stage" | "Unstage" | null;
		onClick?: () => void;
		onAction?: () => void;
		/** Discard this file's unstaged changes — only offered from the Unstaged section
		 * (DESIGN_SPEC.md §6.1/§7.4); omit the prop to hide the button entirely. */
		onDiscard?: () => void;
	} = $props();

	const glyph = $derived(statusGlyph(status));
	const name = $derived(fileName(path));
	const parent = $derived(parentPath(path));

	async function copyPath() {
		try {
			await navigator.clipboard?.writeText(path);
		} catch {
			/* clipboard unavailable — best effort */
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="row" class:selected onclick={() => onClick?.()}>
	<span
		class="glyph"
		class:hollow={glyph.hollow}
		class:partial
		style={`color: var(${glyph.colorVar}); --glyph-color: var(${glyph.colorVar});`}
		title={glyph.label}
		aria-hidden="true"
	>
		{glyph.char}
	</span>
	<span class="name">
		{name}
		{#if origPath}
			<span class="rename-from" title={`Renamed from ${origPath}`}>← {origPath}</span>
		{/if}
	</span>
	{#if parent}<span class="parent" title={parent}>{parent}</span>{/if}
	<span class="actions">
		{#if actionLabel}
			<button
				type="button"
				class="action"
				onclick={(e) => {
					e.stopPropagation();
					onAction?.();
				}}
			>
				{actionLabel}
			</button>
		{/if}
		{#if onDiscard}
			<button
				type="button"
				class="action discard"
				title="Discard changes to this file — recoverable from Recently Discarded"
				aria-label="Discard {name}"
				onclick={(e) => {
					e.stopPropagation();
					onDiscard?.();
				}}
			>
				🗑
			</button>
		{/if}
		<button
			type="button"
			class="overflow"
			title="Copy path"
			aria-label="Copy path"
			onclick={(e) => {
				e.stopPropagation();
				void copyPath();
			}}
		>
			⋯
		</button>
	</span>
</div>

<style>
	.row {
		position: relative;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 3px var(--space-3) 3px var(--space-4);
		font-size: 12px;
		color: var(--text);
		cursor: pointer;
	}

	.row:hover,
	.row.selected {
		background: var(--raised);
	}

	.glyph {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 14px;
		flex-shrink: 0;
		font-weight: 700;
		font-size: 12px;
	}

	.glyph.hollow {
		color: transparent !important;
		-webkit-text-stroke: 1px var(--glyph-color);
	}

	/* Partially-staged: a half-opacity swatch behind the left half of the glyph signals "this
	   file also has changes in the other section" — DESIGN_SPEC.md §6.1. */
	.glyph.partial::before {
		content: "";
		position: absolute;
		inset: 1px 7px 1px 1px;
		background: var(--glyph-color);
		opacity: 0.3;
		border-radius: 2px;
		z-index: -1;
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex-shrink: 1;
		min-width: 40px;
	}

	.rename-from {
		margin-left: 6px;
		color: var(--text-faint);
		font-size: 11px;
	}

	.parent {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		direction: rtl;
		text-align: left;
		color: var(--text-faint);
		font-size: 11px;
	}

	.actions {
		display: flex;
		align-items: center;
		gap: 2px;
		margin-left: auto;
		flex-shrink: 0;
		opacity: 0;
	}

	.row:hover .actions {
		opacity: 1;
	}

	.action {
		padding: 1px var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: 10px;
		cursor: pointer;
	}

	.action:hover {
		background: var(--overlay);
	}

	.action.discard {
		color: var(--danger);
	}

	.overflow {
		border: none;
		background: none;
		color: var(--text-faint);
		font-size: 12px;
		cursor: pointer;
		padding: 0 3px;
		line-height: 1;
	}

	.overflow:hover {
		color: var(--text);
	}
</style>
