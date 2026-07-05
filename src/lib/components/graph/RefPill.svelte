<script lang="ts">
	// phosphor-svelte: icon set requested by Jamie (replaces the emoji glyphs the spec's Lucide
	// suggestion originally shipped as) — the only icon dependency in the app.
	import { Check, Cloud, GitPullRequest, Laptop, Tag } from "phosphor-svelte";
	import type { Pill } from "$lib/graph/pills";
	import { dnd } from "$lib/stores/dnd.svelte";
	import { graphNav } from "$lib/stores/graphNav.svelte";
	import { github } from "$lib/stores/github.svelte";
	import BadgeTooltip from "./BadgeTooltip.svelte";

	/** A branch/tag pill in the BRANCH/TAG column — DESIGN_SPEC.md §4.4. Presence icons (💻 local,
	 * ☁ remote), shared-vs-split is decided upstream in `buildPills`; the ahead/behind badge is a
	 * *button* (click → fix-it popover) that also shows a hover tooltip of up to 5 commit summaries,
	 * and diverged badges are warn-tinted. Single-click selects, double-click checks out (remote-only
	 * → track + checkout), right-click opens the branch menu, and the pill is draggable onto other
	 * pills/rows for the merge/rebase/ff drop menu. */
	let {
		pill,
		colorIndex,
		repoId,
		onSelect,
		onCheckout,
		onBadge,
		onMenu,
		onDrop,
	}: {
		pill: Pill;
		colorIndex: number;
		repoId: string | null;
		onSelect: (pill: Pill) => void;
		onCheckout: (pill: Pill) => void;
		onBadge: (pill: Pill, x: number, y: number) => void;
		onMenu: (pill: Pill, x: number, y: number) => void;
		onDrop: (pill: Pill) => void;
	} = $props();

	const hasBadge = $derived(pill.ahead > 0 || pill.behind > 0);
	// A small PR icon in the pill's tooltip area — DESIGN_SPEC.md §12.
	const matchingPr = $derived(
		pill.kind === "branch"
			? (github.pullRequests.find((p) => p.headRef === (pill.localBranch ?? pill.name)) ?? null)
			: null,
	);
	// A pill can be dropped onto if a *different* branch/commit is being dragged.
	const isDropTarget = $derived(
		dnd.dragging && dnd.source !== null && dnd.source.key !== pill.key,
	);
	const glowing = $derived(
		(dnd.overKey === pill.key && isDropTarget) || graphNav.glowSha === pill.sha,
	);

	function badgeLabel(): string {
		const parts: string[] = [];
		if (pill.ahead > 0) parts.push(`${pill.ahead} to push`);
		if (pill.behind > 0) parts.push(`${pill.behind} to pull`);
		return parts.join(", ");
	}

	// Hover tooltip of commit previews — DESIGN_SPEC §4.4, 400ms delay per §2.5.
	let tip = $state<{ x: number; y: number } | null>(null);
	let tipTimer: ReturnType<typeof setTimeout> | undefined;

	function openTip(e: MouseEvent) {
		if (!repoId || !pill.localBranch) return;
		const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
		clearTimeout(tipTimer);
		tipTimer = setTimeout(() => {
			tip = { x: rect.left + rect.width / 2, y: rect.bottom + 6 };
		}, 400);
	}

	function closeTip() {
		clearTimeout(tipTimer);
		tip = null;
	}

	function onDragStart(e: DragEvent) {
		dnd.start(pill);
		// dataTransfer needs *something* or Firefox won't start the drag; the real payload is dnd.source.
		e.dataTransfer?.setData("text/plain", pill.key);
		if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
	}

	function onDragOver(e: DragEvent) {
		if (!isDropTarget) return;
		e.preventDefault();
		dnd.setOver(pill.key, e.clientX, e.clientY);
	}

	function onDragLeave() {
		if (dnd.overKey === pill.key) dnd.setOver(null);
	}

	function handleDrop(e: DragEvent) {
		if (!isDropTarget) return;
		e.preventDefault();
		e.stopPropagation();
		onDrop(pill);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<span
	class="pill"
	class:tag={pill.kind === "tag"}
	class:head={pill.isHead}
	class:glow={glowing}
	class:remote-only={pill.isRemoteOnly}
	style="--pill-lane: var(--lane-{colorIndex});"
	role="button"
	tabindex="-1"
	draggable={pill.kind === "branch"}
	title={pill.kind === "tag" ? `tag ${pill.name}` : pill.name}
	onclick={(e) => {
		e.stopPropagation();
		onSelect(pill);
	}}
	ondblclick={(e) => {
		e.stopPropagation();
		onCheckout(pill);
	}}
	oncontextmenu={(e) => {
		e.preventDefault();
		e.stopPropagation();
		onMenu(pill, e.clientX, e.clientY);
	}}
	ondragstart={onDragStart}
	ondragend={() => dnd.end()}
	ondragover={onDragOver}
	ondragleave={onDragLeave}
	ondrop={handleDrop}
>
	{#if pill.isHead}<span class="check" title="checked out" aria-hidden="true"><Check size={10} weight="bold" /></span>{/if}
	{#if pill.kind === "tag"}<span class="icon" aria-hidden="true"><Tag size={10} /></span>{/if}
	<span class="name">{pill.name}</span>
	{#if pill.kind === "branch"}
		{#if pill.local}<span class="icon presence" title="exists locally" aria-label="local"><Laptop size={11} /></span>{/if}
		{#if pill.remote}<span class="icon presence" title={pill.remoteName ?? "remote"} aria-label="remote"><Cloud size={11} /></span>{/if}
		{#if matchingPr}
			<span class="icon presence" title="PR #{matchingPr.number}: {matchingPr.title}" aria-label="has pull request"><GitPullRequest size={11} /></span>
		{/if}
	{/if}
	{#if hasBadge}
		<button
			type="button"
			class="badge"
			class:diverged={pill.diverged}
			aria-label={badgeLabel()}
			onclick={(e) => {
				e.stopPropagation();
				closeTip();
				onBadge(pill, e.clientX, e.clientY);
			}}
			ondblclick={(e) => e.stopPropagation()}
			onmouseenter={openTip}
			onmouseleave={closeTip}
		>
			{#if pill.ahead > 0}<span class="ab ahead">↑{pill.ahead}</span>{/if}
			{#if pill.behind > 0}<span class="ab behind">↓{pill.behind}</span>{/if}
		</button>
	{/if}
</span>

{#if tip && repoId}
	<BadgeTooltip {pill} {repoId} x={tip.x} y={tip.y} />
{/if}

<style>
	.pill {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		max-width: 100%;
		padding: 1px 6px;
		border: 1px solid var(--pill-lane);
		border-radius: var(--radius-pill);
		background: var(--raised);
		color: var(--text);
		font-size: 11px;
		line-height: 16px;
		white-space: nowrap;
		cursor: pointer;
		transition:
			box-shadow var(--motion-hover),
			background var(--motion-hover);
	}

	.pill.tag {
		border-radius: var(--radius-control);
		border-color: var(--text-muted);
		color: var(--text-muted);
	}

	.pill.head {
		font-weight: 600;
	}

	.pill.remote-only {
		border-style: dashed;
	}

	.pill.glow {
		box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 70%, transparent);
		background: color-mix(in srgb, var(--accent) 14%, var(--raised));
	}

	.check {
		display: inline-flex;
		align-items: center;
		color: var(--accent);
	}

	.icon {
		display: inline-flex;
		align-items: center;
		line-height: 1;
	}

	.presence {
		color: var(--text-muted);
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.badge {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		border: none;
		border-radius: var(--radius-pill);
		background: transparent;
		padding: 0 3px;
		margin: 0 -2px 0 1px;
		font: inherit;
		cursor: pointer;
		line-height: 14px;
	}

	.badge:hover {
		background: color-mix(in srgb, var(--text) 10%, transparent);
	}

	/* Diverged (both ahead and behind) → subtle warn-tinted background (§4.4). */
	.badge.diverged {
		background: color-mix(in srgb, var(--warn) 22%, transparent);
	}

	.badge.diverged:hover {
		background: color-mix(in srgb, var(--warn) 34%, transparent);
	}

	.ab {
		font-variant-numeric: tabular-nums;
		font-size: 10px;
	}

	.ab.ahead {
		color: var(--ahead);
	}

	.ab.behind {
		color: var(--behind);
	}
</style>
