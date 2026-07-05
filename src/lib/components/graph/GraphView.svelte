<script lang="ts">
	import GraphColumnHeader from "./GraphColumnHeader.svelte";
	import GraphRow from "./GraphRow.svelte";
	import WipGraphRow from "./WipGraphRow.svelte";
	import DetachGuardPopover from "./DetachGuardPopover.svelte";
	import AheadBehindPopover from "./AheadBehindPopover.svelte";
	import BranchMenu from "./BranchMenu.svelte";
	import DropMenu from "./DropMenu.svelte";
	import {
		anchoredScrollTop,
		AVATAR_RADIUS,
		laneCenterX,
		MERGE_NODE_RADIUS,
		ROW_HEIGHT,
		STASH_NODE_RADIUS,
		totalHeight,
		visibleRowRange,
	} from "$lib/graph/geometry";
	import { AvatarCache, authorInitials, discColorIndex } from "$lib/graph/avatars";
	import type { GraphSegment, SegmentEnd } from "$lib/graph/lanes";
	import type { Pill } from "$lib/graph/pills";
	import { graph } from "$lib/stores/graph.svelte";
	import { status } from "$lib/stores/status.svelte";
	import { graphSelection } from "$lib/stores/graphSelection.svelte";
	import { graphView } from "$lib/stores/graphView.svelte";
	import { buildWipRow, WIP_SHA, type WipRow } from "$lib/graph/wip";
	import type { GraphViewRow } from "$lib/stores/graph.svelte";
	import { branchEdit } from "$lib/stores/branchEdit.svelte";
	import { dnd } from "$lib/stores/dnd.svelte";
	import { graphNav } from "$lib/stores/graphNav.svelte";
	import { theme } from "$lib/stores/theme.svelte";
	import { isModEvent } from "$lib/platform";
	import * as actions from "$lib/actions";

	/** The commit graph — DESIGN_SPEC.md §4, ARCHITECTURE.md §5.4. Hand-rolled virtualized DOM rows
	 * over a single background canvas that draws edges/nodes/avatars for the visible range only,
	 * redrawn via rAF on scroll. Git mutations (checkout, create branch, back-to-branch, open diff)
	 * are emitted as intents; they get wired to the op queue in later prompts. */
	let {
		onSelectCommit,
		onCompare,
		onOpenCommit,
	}: {
		onSelectCommit?: (sha: string) => void;
		onCompare?: (a: string, b: string) => void;
		onOpenCommit?: (sha: string) => void;
	} = $props();

	const CANVAS_FONT_FAMILY = '-apple-system, "Segoe UI", Ubuntu, sans-serif';

	interface Colors {
		lanes: string[];
		accent: string;
		bg: string;
		surface: string;
		muted: string;
	}

	let scrollEl: HTMLDivElement | null = $state(null);
	let bodyEl: HTMLDivElement | null = $state(null);
	let canvasEl: HTMLCanvasElement | null = $state(null);

	let scrollTop = $state(0);
	let viewportHeight = $state(0);
	let hoveredSha = $state<string | null>(null);
	let popover = $state<{ sha: string; x: number; y: number } | null>(null);
	// Pill overlays — the badge fix-it popover, the branch context menu and the drag drop menu.
	let badgePopover = $state<{ pill: Pill; x: number; y: number } | null>(null);
	let branchMenu = $state<{ pill: Pill; x: number; y: number } | null>(null);
	let dropMenu = $state<{
		source: Pill;
		targetPill: Pill | null;
		targetSha: string;
		x: number;
		y: number;
	} | null>(null);

	const repoId = $derived(graph.repoId);
	const currentBranch = $derived(graph.head && !graph.head.detached ? graph.head.branch : null);

	// Non-reactive scratch state used only inside the canvas draw / anchoring.
	let shaIndex = new Map<string, number>();
	let anchor: { sha: string | null; offset: number } = { sha: null, offset: 0 };
	let lastCompute = -1;
	let lastRepo: string | null = null;
	let rafId = 0;
	let colorsCache: Colors | null = null;
	let colorsTheme = "";

	const avatars = new AvatarCache(() => requestDraw());

	const rows = $derived(graph.rows);
	const headSha = $derived(graph.head?.sha ?? null);
	const detached = $derived(graph.head?.detached ?? false);

	// The WIP row (§4.2) is synthesized here, not in the store, so the store stays a pure topology
	// projection. It hangs off HEAD's lane; `allRows` is what the canvas, virtualization and DOM all
	// index against, so the +1 offset it introduces stays consistent everywhere.
	const headLane = $derived(rows.find((r) => r.sha === headSha)?.node.lane ?? 0);
	const wipRow = $derived<WipRow | null>(
		status.report.entries.length > 0 && headSha
			? buildWipRow(headLane, status.report.entries)
			: null,
	);
	const allRows = $derived<(WipRow | GraphViewRow)[]>(wipRow ? [wipRow, ...rows] : rows);
	const wipOffset = $derived(wipRow ? 1 : 0);

	const range = $derived(visibleRowRange(scrollTop, viewportHeight, allRows.length));
	const visibleRows = $derived(allRows.slice(range.start, range.end));

	// Slide-in only on genuine appearance (working tree goes dirty), never on scroll (§4.2).
	let wipEnter = $state(false);
	let wipWasVisible = false;
	let wipEnterTimer: ReturnType<typeof setTimeout> | undefined;
	const showSkeleton = $derived(graph.loading && rows.length === 0);
	const showEmpty = $derived(!graph.loading && rows.length === 0 && graph.repoId !== null);

	function requestDraw() {
		if (rafId) return;
		rafId = requestAnimationFrame(() => {
			rafId = 0;
			draw();
		});
	}

	function readColors(): Colors {
		const s = getComputedStyle(document.documentElement);
		const v = (name: string) => s.getPropertyValue(name).trim();
		return {
			lanes: Array.from({ length: 8 }, (_, i) => v(`--lane-${i}`)),
			accent: v("--accent"),
			bg: v("--bg"),
			surface: v("--surface"),
			muted: v("--text-muted"),
		};
	}

	function colors(): Colors {
		const t = theme.resolved;
		if (!colorsCache || colorsTheme !== t) {
			colorsCache = readColors();
			colorsTheme = t;
		}
		return colorsCache;
	}

	function endpoint(end: SegmentEnd, graphLeft: number, yTop: number, yMid: number, yBot: number, nodeLane: number) {
		if (end.at === "node") return { x: graphLeft + laneCenterX(nodeLane), y: yMid };
		const y = end.at === "top" ? yTop : yBot;
		return { x: graphLeft + laneCenterX(end.lane), y };
	}

	/** `${rowIndex}:${lane}` keys covering the first-parent chain below `sha` — the lit lineage on
	 * hover (DESIGN_SPEC.md §4.3). Runs of a lane between a commit and its first parent are lit so the
	 * ancestry line brightens while everything else dims. */
	function lineageKeys(sha: string): Set<string> {
		const lit = new Set<string>();
		const laneRows = allRows;
		let idx = shaIndex.get(sha);
		let guard = 0;
		while (idx !== undefined && guard++ < laneRows.length + 1) {
			const row = laneRows[idx];
			if (!row) break;
			lit.add(`${idx}:${row.node.lane}`);
			const parent = row.kind === "commit" ? row.parents[0] : undefined;
			const pIdx = parent !== undefined ? shaIndex.get(parent) : undefined;
			if (pIdx === undefined) break;
			for (let r = idx + 1; r < pIdx; r += 1) lit.add(`${r}:${row.node.lane}`);
			idx = pIdx;
		}
		return lit;
	}

	function segmentLane(seg: GraphSegment, nodeLane: number): number {
		if (seg.to.at === "bottom") return seg.to.lane;
		if (seg.from.at === "top") return seg.from.lane;
		return nodeLane;
	}

	function drawInitialsDisc(
		ctx: CanvasRenderingContext2D,
		x: number,
		y: number,
		r: number,
		colorIndex: number,
		text: string,
		c: Colors,
	) {
		ctx.beginPath();
		ctx.arc(x, y, r, 0, Math.PI * 2);
		ctx.fillStyle = c.lanes[colorIndex] ?? c.accent;
		ctx.fill();
		ctx.fillStyle = c.bg;
		ctx.font = `600 ${Math.round(r * 0.85)}px ${CANVAS_FONT_FAMILY}`;
		ctx.textAlign = "center";
		ctx.textBaseline = "middle";
		ctx.fillText(text, x, y + 0.5);
	}

	function drawNode(ctx: CanvasRenderingContext2D, row: WipRow | GraphViewRow, x: number, y: number, c: Colors) {
		// A background halo so lane lines don't cut across the node.
		const haloR = row.kind === "stash" ? STASH_NODE_RADIUS + 2 : AVATAR_RADIUS + 1;
		ctx.beginPath();
		ctx.arc(x, y, haloR, 0, Math.PI * 2);
		ctx.fillStyle = c.bg;
		ctx.fill();

		if (row.kind === "wip") {
			// Dashed hollow node — DESIGN_SPEC.md §4.2.
			ctx.beginPath();
			ctx.arc(x, y, AVATAR_RADIUS - 2, 0, Math.PI * 2);
			ctx.fillStyle = c.bg;
			ctx.fill();
			ctx.setLineDash([2, 2]);
			ctx.strokeStyle = c.muted;
			ctx.lineWidth = 1.5;
			ctx.stroke();
			ctx.setLineDash([]);
			return;
		}

		if (row.kind === "stash") {
			ctx.beginPath();
			ctx.rect(x - STASH_NODE_RADIUS, y - STASH_NODE_RADIUS, STASH_NODE_RADIUS * 2, STASH_NODE_RADIUS * 2);
			ctx.fillStyle = c.surface;
			ctx.fill();
			ctx.strokeStyle = c.muted;
			ctx.lineWidth = 1.5;
			ctx.stroke();
			return;
		}

		const isMerge = row.parents.length > 1;
		if (isMerge) {
			// Smaller plain node for merges — DESIGN_SPEC.md §4.3.
			ctx.beginPath();
			ctx.arc(x, y, MERGE_NODE_RADIUS, 0, Math.PI * 2);
			ctx.fillStyle = c.lanes[row.node.colorIndex] ?? c.accent;
			ctx.fill();
		} else {
			const bitmap = row.meta ? avatars.get(row.meta.authorEmail) : null;
			if (bitmap) {
				ctx.save();
				ctx.beginPath();
				ctx.arc(x, y, AVATAR_RADIUS, 0, Math.PI * 2);
				ctx.clip();
				ctx.drawImage(bitmap, x - AVATAR_RADIUS, y - AVATAR_RADIUS, AVATAR_RADIUS * 2, AVATAR_RADIUS * 2);
				ctx.restore();
				ctx.beginPath();
				ctx.arc(x, y, AVATAR_RADIUS, 0, Math.PI * 2);
				ctx.strokeStyle = c.surface;
				ctx.lineWidth = 1;
				ctx.stroke();
			} else if (row.meta) {
				drawInitialsDisc(
					ctx,
					x,
					y,
					AVATAR_RADIUS,
					discColorIndex(row.meta.authorEmail),
					authorInitials(row.meta.authorName, row.meta.authorEmail),
					c,
				);
			} else {
				ctx.beginPath();
				ctx.arc(x, y, AVATAR_RADIUS - 3, 0, Math.PI * 2);
				ctx.fillStyle = c.lanes[row.node.colorIndex] ?? c.accent;
				ctx.fill();
			}
		}

		if (row.sha === headSha) {
			ctx.beginPath();
			ctx.arc(x, y, AVATAR_RADIUS + 2.5, 0, Math.PI * 2);
			ctx.strokeStyle = c.accent;
			ctx.lineWidth = 2;
			ctx.stroke();
		}
	}

	function draw() {
		const canvas = canvasEl;
		if (!canvas) return;
		const ctx = canvas.getContext("2d");
		if (!ctx) return;
		const dpr = window.devicePixelRatio || 1;
		const cssW = canvas.clientWidth;
		const cssH = canvas.clientHeight;
		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		ctx.clearRect(0, 0, cssW, cssH);

		const st = scrollEl ? scrollEl.scrollTop : scrollTop;
		const win = visibleRowRange(st, cssH, allRows.length);
		const c = colors();
		const graphLeft = graphView.widths.branch;

		ctx.save();
		ctx.beginPath();
		ctx.rect(graphLeft, 0, graphView.widths.graph, cssH);
		ctx.clip();

		const lit = hoveredSha ? lineageKeys(hoveredSha) : null;

		// Pass 1: lane lines. Drawn before nodes so avatars sit on top.
		ctx.lineWidth = 1.6;
		for (let i = win.start; i < win.end; i += 1) {
			const row = allRows[i];
			const yTop = i * ROW_HEIGHT - st;
			const yMid = yTop + ROW_HEIGHT / 2;
			const yBot = yTop + ROW_HEIGHT;
			for (const seg of row.segments) {
				const p1 = endpoint(seg.from, graphLeft, yTop, yMid, yBot, row.node.lane);
				const p2 = endpoint(seg.to, graphLeft, yTop, yMid, yBot, row.node.lane);
				let alpha = hoveredSha ? 0.3 : 0.9;
				if (lit && lit.has(`${i}:${segmentLane(seg, row.node.lane)}`)) alpha = 1;
				ctx.globalAlpha = alpha;
				ctx.strokeStyle = c.lanes[seg.colorIndex] ?? c.muted;
				ctx.setLineDash(seg.dashed ? [2, 3] : []);
				ctx.beginPath();
				ctx.moveTo(p1.x, p1.y);
				if (Math.abs(p1.x - p2.x) < 0.5) {
					ctx.lineTo(p2.x, p2.y);
				} else {
					const midY = (p1.y + p2.y) / 2;
					ctx.bezierCurveTo(p1.x, midY, p2.x, midY, p2.x, p2.y);
				}
				ctx.stroke();
			}
		}
		ctx.setLineDash([]);
		ctx.globalAlpha = 1;

		// Pass 2: nodes + avatars.
		for (let i = win.start; i < win.end; i += 1) {
			const row = allRows[i];
			const yMid = i * ROW_HEIGHT - st + ROW_HEIGHT / 2;
			drawNode(ctx, row, graphLeft + laneCenterX(row.node.lane), yMid, c);
		}

		ctx.restore();
	}

	function resizeCanvas() {
		if (!canvasEl || !bodyEl) return;
		const rect = bodyEl.getBoundingClientRect();
		viewportHeight = rect.height;
		const dpr = window.devicePixelRatio || 1;
		canvasEl.width = Math.max(1, Math.round(rect.width * dpr));
		canvasEl.height = Math.max(1, Math.round(rect.height * dpr));
		canvasEl.style.width = `${rect.width}px`;
		canvasEl.style.height = `${rect.height}px`;
		requestDraw();
	}

	function onScroll() {
		if (!scrollEl) return;
		const st = scrollEl.scrollTop;
		scrollTop = st;
		const topIndex = Math.floor(st / ROW_HEIGHT);
		const row = allRows[topIndex];
		anchor = { sha: row?.sha ?? null, offset: st - topIndex * ROW_HEIGHT };
		requestDraw();
	}

	function scrollRowIntoView(idx: number) {
		if (!scrollEl) return;
		const top = idx * ROW_HEIGHT;
		const bottom = top + ROW_HEIGHT;
		if (top < scrollEl.scrollTop) {
			scrollEl.scrollTop = top;
		} else if (bottom > scrollEl.scrollTop + viewportHeight) {
			scrollEl.scrollTop = bottom - viewportHeight;
		}
		scrollTop = scrollEl.scrollTop;
	}

	function moveSelection(delta: number) {
		if (allRows.length === 0) return;
		const current = graphSelection.selectedSha ? shaIndex.get(graphSelection.selectedSha) : undefined;
		let idx = current === undefined ? (delta > 0 ? 0 : allRows.length - 1) : current + delta;
		idx = Math.max(0, Math.min(allRows.length - 1, idx));
		const sha = allRows[idx].sha;
		if (sha === WIP_SHA) {
			// Landing on the WIP row selects working-directory mode, not a commit (§4.2).
			graphSelection.clear();
		} else {
			graphSelection.select(sha);
			onSelectCommit?.(sha);
		}
		scrollRowIntoView(idx);
	}

	function onKeydown(e: KeyboardEvent) {
		const key = e.key;
		if (key === "ArrowDown" || key === "j") {
			e.preventDefault();
			moveSelection(1);
		} else if (key === "ArrowUp" || key === "k") {
			e.preventDefault();
			moveSelection(-1);
		} else if (key === "Enter") {
			if (graphSelection.selectedSha) {
				e.preventDefault();
				onOpenCommit?.(graphSelection.selectedSha);
			}
		}
	}

	function handleSelect(sha: string, e: MouseEvent) {
		scrollEl?.focus();
		if (isModEvent(e)) {
			const previous = graphSelection.selectedSha;
			graphSelection.toggleCompare(sha);
			if (graphSelection.compare && previous) onCompare?.(previous, sha);
		} else {
			graphSelection.select(sha);
			onSelectCommit?.(sha);
		}
	}

	/** Clicking the WIP row body (not the `// WIP` text) → working-directory mode (§4.2). */
	function handleWipSelect() {
		scrollEl?.focus();
		graphSelection.clear();
	}

	function handleActivate(sha: string, e: MouseEvent) {
		const row = allRows[shaIndex.get(sha) ?? -1];
		if (row?.kind === "stash" || row?.kind === "wip") return; // stash pop / WIP have no detach.
		// Double-click a commit → detached checkout, guarded (§4.6). "Don't ask again" skips the popover.
		if (graphView.detachDontAsk) {
			if (repoId) void actions.checkoutDetached(repoId, sha);
			return;
		}
		popover = { sha, x: e.clientX, y: e.clientY };
	}

	async function handleCopySha(sha: string) {
		try {
			await navigator.clipboard?.writeText(sha);
		} catch {
			/* clipboard unavailable — best effort */
		}
	}

	function scrollShaIntoView(sha: string) {
		const idx = shaIndex.get(sha);
		if (idx !== undefined) scrollRowIntoView(idx);
	}

	// --- pill interactions (DESIGN_SPEC §4.4) ---

	function handlePillSelect(pill: Pill) {
		graphSelection.select(pill.sha);
		onSelectCommit?.(pill.sha);
		scrollShaIntoView(pill.sha);
	}

	function handlePillCheckout(pill: Pill) {
		if (!repoId) return;
		if (pill.kind === "tag") return; // tags aren't checked out from a double-click (v1).
		if (pill.isRemoteOnly && pill.remoteRef) {
			void actions.checkoutRemote(repoId, pill.remoteRef);
		} else if (pill.localBranch) {
			void actions.checkoutBranch(repoId, pill.localBranch);
		}
	}

	function handlePillBadge(pill: Pill, x: number, y: number) {
		badgePopover = { pill, x, y };
	}

	function handlePillMenu(pill: Pill, x: number, y: number) {
		branchMenu = { pill, x, y };
	}

	/** A dragged pill was dropped onto pill `target`. */
	function handlePillDrop(target: Pill) {
		const source = dnd.source;
		if (!source || source.key === target.key) return;
		dropMenu = { source, targetPill: target, targetSha: target.sha, x: dnd.x, y: dnd.y };
	}

	/** A dragged pill was dropped onto a bare commit row `sha`. */
	function handleRowDrop(sha: string) {
		const source = dnd.source;
		if (!source || source.sha === sha) return;
		dropMenu = { source, targetPill: null, targetSha: sha, x: dnd.x, y: dnd.y };
	}

	function startCreateBranch(sha: string) {
		branchEdit.startCreate(sha);
	}

	function startRename(pill: Pill) {
		if (pill.localBranch) branchEdit.startRename(pill.localBranch, pill.sha);
	}

	// Redraw whenever the data, layout, hover or scroll offset changes; rAF coalesces bursts.
	$effect(() => {
		void allRows;
		void graph.metaBySha;
		void graphView.widths.branch;
		void graphView.widths.graph;
		void hoveredSha;
		void scrollTop;
		void theme.resolved;
		requestDraw();
	});

	// Lazy-load metadata for the visible window (+ overscan) — ARCHITECTURE.md §5.1. The window is
	// in `allRows` space; shift it back by the WIP row so it indexes the store's commit rows.
	$effect(() => {
		const { start, end } = range;
		if (graph.repoId)
			void graph.ensureMetadataForWindow(Math.max(0, start - wipOffset), Math.max(0, end - wipOffset));
	});

	// Fire the WIP row's slide-in only when the working tree genuinely goes dirty (§4.2).
	// SPEC-DEVIATION (§4.2 "commit animates WIP → new commit row"): the literal morph of the WIP
	// row into the freshly-created commit row would need a FLIP across the canvas-drawn node layer,
	// which the virtualized canvas graph can't animate. It's approximated by the WIP row's slide-in
	// on reappearance plus the composer's 240ms success sweep; the row itself just swaps on refresh.
	$effect(() => {
		const visible = wipRow !== null;
		if (visible && !wipWasVisible) {
			wipEnter = true;
			clearTimeout(wipEnterTimer);
			wipEnterTimer = setTimeout(() => (wipEnter = false), 240);
		}
		wipWasVisible = visible;
	});

	// Flash-scroll to a commit on request (panel click-to-tip, badge "view commits", merge "View").
	$effect(() => {
		void graphNav.scrollToken;
		const sha = graphNav.scrollSha;
		if (sha) scrollShaIntoView(sha);
	});

	// Rebuild the sha→index map on topology reloads and restore the viewport to the anchored sha so
	// refreshes never move what you're looking at — DESIGN_SPEC.md §4.7 / §15.32.
	$effect(() => {
		const rowsForIndex = allRows;
		const cc = graph.laneComputeCount;
		const map = new Map<string, number>();
		for (let i = 0; i < rowsForIndex.length; i += 1) map.set(rowsForIndex[i].sha, i);
		shaIndex = map;
		if (cc !== lastCompute) {
			const previous = lastCompute;
			lastCompute = cc;
			if (previous !== -1 && anchor.sha && scrollEl) {
				const idx = map.get(anchor.sha);
				if (idx !== undefined) {
					const top = anchoredScrollTop(idx, anchor.offset);
					scrollEl.scrollTop = top;
					scrollTop = top;
				}
			}
		}
		requestDraw();
	});

	// Reset transient view state when the repo changes under us.
	$effect(() => {
		const id = graph.repoId;
		if (id !== lastRepo) {
			lastRepo = id;
			lastCompute = -1;
			anchor = { sha: null, offset: 0 };
			hoveredSha = null;
			popover = null;
			badgePopover = null;
			branchMenu = null;
			dropMenu = null;
			branchEdit.cancel();
			dnd.end();
			if (scrollEl) scrollEl.scrollTop = 0;
			scrollTop = 0;
			graphSelection.clear();
		}
	});

	// Size the canvas to the body and keep it sized.
	$effect(() => {
		if (!bodyEl) return;
		resizeCanvas();
		const observer = new ResizeObserver(() => resizeCanvas());
		observer.observe(bodyEl);
		return () => observer.disconnect();
	});

	// Release decoded avatars on unmount.
	$effect(() => () => avatars.dispose());
</script>

{#if detached && headSha}
	<div class="banner" role="status">
		<span class="banner-text">
			Detached at <code>{headSha.slice(0, 7)}</code> — changes here can be lost.
		</span>
		<div class="banner-actions">
			<button type="button" onclick={() => startCreateBranch(headSha)}>Create branch here</button>
			<button type="button" onclick={() => repoId && actions.backToPrevious(repoId)}>
				Back to branch
			</button>
		</div>
	</div>
{/if}

<div class="graph-view">
	<GraphColumnHeader />

	<div class="graph-body" bind:this={bodyEl}>
		<canvas class="graph-canvas" bind:this={canvasEl} aria-hidden="true"></canvas>

		{#if showSkeleton}
			<div class="skeleton" aria-hidden="true">
				{#each Array(24) as _, i (i)}
					<div class="skeleton-row" style="height: {ROW_HEIGHT}px;">
						<span class="shimmer node"></span>
						<span class="shimmer bar" style="width: {40 + ((i * 37) % 45)}%;"></span>
					</div>
				{/each}
			</div>
		{:else if showEmpty}
			<div class="empty">
				<p>No commits yet — stage files and make the first one.</p>
			</div>
		{:else}
			<div
				class="graph-scroll"
				bind:this={scrollEl}
				tabindex="0"
				role="grid"
				aria-label="Commit graph"
				aria-rowcount={allRows.length}
				onscroll={onScroll}
				onkeydown={onKeydown}
			>
				<div class="graph-spacer" style="height: {totalHeight(allRows.length)}px;">
					<div class="graph-rows" style="transform: translateY({range.start * ROW_HEIGHT}px);">
						{#each visibleRows as row (row.sha)}
							{#if row.kind === "wip"}
								<WipGraphRow {row} {repoId} animateIn={wipEnter} onSelect={handleWipSelect} />
							{:else}
								<GraphRow
									{row}
									{repoId}
									selected={graphSelection.selectedSha === row.sha}
									head={row.sha === headSha}
									onSelect={handleSelect}
									onActivate={handleActivate}
									onHover={(sha) => (hoveredSha = sha)}
									onCopySha={handleCopySha}
									onPillSelect={handlePillSelect}
									onPillCheckout={handlePillCheckout}
									onPillBadge={handlePillBadge}
									onPillMenu={handlePillMenu}
									onPillDrop={handlePillDrop}
									onRowDrop={handleRowDrop}
								/>
							{/if}
						{/each}
					</div>
				</div>
			</div>
		{/if}
	</div>
</div>

{#if popover}
	<DetachGuardPopover
		sha={popover.sha}
		x={popover.x}
		y={popover.y}
		onCheckout={(sha) => repoId && actions.checkoutDetached(repoId, sha)}
		onCreateBranch={(sha) => startCreateBranch(sha)}
		onDismiss={() => (popover = null)}
	/>
{/if}

{#if badgePopover && repoId}
	<AheadBehindPopover
		pill={badgePopover.pill}
		{repoId}
		x={badgePopover.x}
		y={badgePopover.y}
		isCurrent={badgePopover.pill.localBranch !== null &&
			badgePopover.pill.localBranch === currentBranch}
		onDismiss={() => (badgePopover = null)}
	/>
{/if}

{#if branchMenu && repoId}
	<BranchMenu
		pill={branchMenu.pill}
		{repoId}
		{currentBranch}
		x={branchMenu.x}
		y={branchMenu.y}
		onDismiss={() => (branchMenu = null)}
		onRename={startRename}
		onCreateBranch={startCreateBranch}
	/>
{/if}

{#if dropMenu && repoId}
	<DropMenu
		source={dropMenu.source}
		targetPill={dropMenu.targetPill}
		targetSha={dropMenu.targetSha}
		{repoId}
		{currentBranch}
		x={dropMenu.x}
		y={dropMenu.y}
		onDismiss={() => (dropMenu = null)}
	/>
{/if}

<style>
	.graph-view {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		background: var(--bg);
	}

	.banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		background: color-mix(in srgb, var(--warn) 16%, var(--surface));
		border-bottom: 1px solid var(--warn);
		font-size: 12px;
		color: var(--text);
	}

	.banner code {
		font-family: var(--font-mono);
		color: var(--warn);
	}

	.banner-actions {
		display: flex;
		gap: var(--space-2);
		flex-shrink: 0;
	}

	.banner-actions button {
		padding: 3px var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 11px;
		cursor: pointer;
	}

	.banner-actions button:hover {
		background: var(--overlay);
	}

	.graph-body {
		position: relative;
		flex: 1;
		min-height: 0;
		overflow: hidden;
	}

	.graph-canvas {
		position: absolute;
		inset: 0;
		z-index: 0;
		pointer-events: none;
	}

	.graph-scroll {
		position: absolute;
		inset: 0;
		z-index: 1;
		overflow-y: auto;
		overflow-x: hidden;
		outline: none;
	}

	.graph-scroll:focus-visible {
		box-shadow: inset 0 0 0 2px var(--accent);
	}

	.graph-spacer {
		position: relative;
		width: 100%;
	}

	.graph-rows {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		will-change: transform;
	}

	.skeleton {
		position: absolute;
		inset: 0;
		padding-top: var(--space-1);
	}

	.skeleton-row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: 0 var(--space-4);
	}

	.shimmer {
		background: linear-gradient(90deg, var(--surface) 25%, var(--raised) 50%, var(--surface) 75%);
		background-size: 200% 100%;
		animation: shimmer 1.4s linear infinite;
		border-radius: var(--radius-control);
	}

	.shimmer.node {
		width: 14px;
		height: 14px;
		border-radius: var(--radius-pill);
		margin-left: 200px;
		flex-shrink: 0;
	}

	.shimmer.bar {
		height: 8px;
	}

	@keyframes shimmer {
		to {
			background-position: -200% 0;
		}
	}

	.empty {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-muted);
		font-size: 13px;
	}

	@media (prefers-reduced-motion: reduce) {
		.shimmer {
			animation: none;
		}
	}
</style>
