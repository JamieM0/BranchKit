<script lang="ts">
	/** The one context-menu component used everywhere a right-click surface exists — commit rows,
	 * branch pills/panel rows, stash rows, file rows, and the graph gear (GITKRAKEN_WORKFLOWS.md
	 * §3, DESIGN_SPEC.md §15.30). Overlay + scrim, nested submenus (hover-opened), shortcut labels
	 * right-aligned, and disabled items carry a tooltip explaining why (title attribute — DESIGN_SPEC
	 * §14 "tooltips on everything"). Self-recursive: a submenu is just another `<ContextMenu>`
	 * anchored off its parent item. */
	import ContextMenu from "./ContextMenu.svelte";

	export type MenuItem =
		| {
				type: "action";
				label: string;
				shortcut?: string;
				danger?: boolean;
				disabledReason?: string;
				/** Skip the auto-dismiss so `run` can swap the menu for inline UI (e.g. a follow-up
				 * form) instead of the whole menu unmounting out from under it. */
				keepOpen?: boolean;
				run: () => void | Promise<void>;
		  }
		| { type: "submenu"; label: string; disabledReason?: string; items: MenuItem[] }
		| { type: "separator" };

	let {
		items,
		x,
		y,
		onDismiss,
		ariaLabel = "Context menu",
	}: {
		items: MenuItem[];
		x: number;
		y: number;
		onDismiss: () => void;
		ariaLabel?: string;
	} = $props();

	let openSubmenu = $state<number | null>(null);
	let submenuPos = $state<{ x: number; y: number } | null>(null);
	let hoverTimer: ReturnType<typeof setTimeout> | undefined;

	/** Pixel gutter kept between the menu and the viewport edge so it never touches the screen
	 * border (and never flows off the visible area — root menus and recursive submenus alike). */
	const edgeMargin = 4;
	let menuWidth = $state(0);
	let menuHeight = $state(0);

	const clampedX = $derived(
		menuWidth === 0
			? x
			: Math.max(edgeMargin, Math.min(x, window.innerWidth - menuWidth - edgeMargin)),
	);
	const clampedY = $derived(
		menuHeight === 0
			? y
			: Math.max(edgeMargin, Math.min(y, window.innerHeight - menuHeight - edgeMargin)),
	);

	function enterItem(i: number, item: MenuItem, e: MouseEvent) {
		clearTimeout(hoverTimer);
		if (item.type !== "submenu") {
			openSubmenu = null;
			return;
		}
		const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
		hoverTimer = setTimeout(() => {
			openSubmenu = i;
			submenuPos = { x: rect.right - 2, y: rect.top };
		}, 120);
	}

	function runAction(item: Extract<MenuItem, { type: "action" }>) {
		if (item.disabledReason) return;
		if (!item.keepOpen) onDismiss();
		void item.run();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="scrim" onclick={onDismiss} oncontextmenu={(e) => e.preventDefault()}></div>
<div class="menu" role="menu" aria-label={ariaLabel} bind:clientWidth={menuWidth} bind:clientHeight={menuHeight} style="left: {clampedX}px; top: {clampedY}px;">
	{#each items as item, i (i)}
		{#if item.type === "separator"}
			<div class="sep"></div>
		{:else if item.type === "action"}
			<button
				type="button"
				role="menuitem"
				class:danger={item.danger}
				disabled={!!item.disabledReason}
				title={item.disabledReason}
				onmouseenter={(e) => enterItem(i, item, e)}
				onclick={() => runAction(item)}
			>
				<span class="label">{item.label}</span>
				{#if item.shortcut}<span class="shortcut">{item.shortcut}</span>{/if}
			</button>
		{:else}
			<button
				type="button"
				role="menuitem"
				aria-haspopup="menu"
				aria-expanded={openSubmenu === i}
				disabled={!!item.disabledReason}
				title={item.disabledReason}
				onmouseenter={(e) => enterItem(i, item, e)}
				onclick={(e) => {
					e.stopPropagation();
					if (item.disabledReason) return;
					const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
					openSubmenu = openSubmenu === i ? null : i;
					submenuPos = { x: rect.right - 2, y: rect.top };
				}}
			>
				<span class="label">{item.label}</span>
				<span class="chevron">▸</span>
			</button>
		{/if}
	{/each}
</div>

{#if openSubmenu !== null && submenuPos}
	{@const submenuItem = items[openSubmenu]}
	{#if submenuItem.type === "submenu"}
		<ContextMenu
			items={submenuItem.items}
			x={submenuPos.x}
			y={submenuPos.y}
			onDismiss={onDismiss}
			ariaLabel={submenuItem.label}
		/>
	{/if}
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 90;
	}

	.menu {
		position: fixed;
		z-index: 91;
		min-width: 220px;
		max-width: 320px;
		max-height: calc(100vh - 8px);
		overflow-y: auto;
		padding: var(--space-1);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
		display: flex;
		flex-direction: column;
	}

	.menu button {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		width: 100%;
		padding: var(--space-2) var(--space-2);
		border: none;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text);
		font: inherit;
		font-size: 12px;
		text-align: left;
		cursor: pointer;
	}

	.menu button:hover:not(:disabled) {
		background: var(--raised);
	}

	.menu button:disabled {
		color: var(--text-faint);
		cursor: default;
	}

	.menu button.danger {
		color: var(--danger);
	}

	.label {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.shortcut {
		flex-shrink: 0;
		color: var(--text-faint);
		font-size: 10px;
		letter-spacing: 0.02em;
	}

	.chevron {
		flex-shrink: 0;
		color: var(--text-faint);
		font-size: 9px;
	}

	.sep {
		height: 1px;
		margin: var(--space-1) var(--space-1);
		background: var(--border-soft);
	}
</style>
