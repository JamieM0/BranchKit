<script lang="ts">
	import GraphColumnHeader from "./GraphColumnHeader.svelte";
	import GraphRow from "./GraphRow.svelte";
	import WipGraphRow from "./WipGraphRow.svelte";
	import DetachGuardPopover from "./DetachGuardPopover.svelte";
	import AheadBehindPopover from "./AheadBehindPopover.svelte";
	import BranchMenu from "./BranchMenu.svelte";
	import DropMenu from "./DropMenu.svelte";
	import CommitMenu from "./CommitMenu.svelte";
	import StashMenu from "./StashMenu.svelte";
	import {
		anchoredScrollTop,
		AVATAR_RADIUS,
		graphWidthForLanes,
		laneCenterX,
		MERGE_NODE_RADIUS,
		ROW_HEIGHT,
		STASH_NODE_RADIUS,
		totalHeight,
		visibleRowRange,
	} from "$lib/graph/geometry";
	import { AvatarCache, authorInitials, discColorIndex } from "$lib/graph/avatars";
	import {
		laneColorIndex,
		type GraphSegment,
		type LanePassSpan,
		type SegmentEnd,
	} from "$lib/graph/lanes";
	import type { Pill } from "$lib/graph/pills";
	import { graph } from "$lib/stores/graph.svelte";
	import { status } from "$lib/stores/status.svelte";
	import { graphSelection } from "$lib/stores/graphSelection.svelte";
	import { commitExplanation } from "$lib/stores/commitExplanation.svelte";
	import { graphView } from "$lib/stores/graphView.svelte";
	import { appSettings } from "$lib/stores/appSettings.svelte";
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
		behind: string;
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
	let commitMenu = $state<{ sha: string; x: number; y: number } | null>(null);
	let stashMenu = $state<{ selector: string; subject: string; x: number; y: number } | null>(null);

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

	// Local-vs-remote commit distinction (§15/QA): commits unreachable from every remote-tracking
	// ref are "unpushed" — they only exist on this machine. Computed by a plain BFS over the
	// topology we already hold client-side; O(commits), no extra git calls.
	const remoteTipShas = $derived(
		graph.refs.filter((r) => r.kind === "remoteBranch").map((r) => r.sha),
	);
	const hasRemoteRefs = $derived(remoteTipShas.length > 0);
	const remoteReachable = $derived.by(() => {
		const parentsBySha = new Map<string, string[]>();
		for (const r of rows) if (r.kind === "commit") parentsBySha.set(r.sha, r.parents);
		const seen = new Set<string>();
		const stack = [...remoteTipShas];
		while (stack.length > 0) {
			const sha = stack.pop()!;
			if (seen.has(sha)) continue;
			seen.add(sha);
			const parents = parentsBySha.get(sha);
			if (parents) for (const p of parents) if (!seen.has(p)) stack.push(p);
		}
		return seen;
	});

	function isUnpushed(row: WipRow | GraphViewRow): boolean {
		return hasRemoteRefs && row.kind === "commit" && !remoteReachable.has(row.sha);
	}

	// Mirror image of the above: commits reachable from a remote-tracking tip but not from any
	// local branch tip are sitting in `origin/*` but haven't been merged into local history yet —
	// they've been fetched (their objects are already in topology/rev-list --all) but not pulled.
	const localTipShas = $derived(graph.refs.filter((r) => r.kind === "branch").map((r) => r.sha));
	const localReachable = $derived.by(() => {
		const parentsBySha = new Map<string, string[]>();
		for (const r of rows) if (r.kind === "commit") parentsBySha.set(r.sha, r.parents);
		const seen = new Set<string>();
		const stack = [...localTipShas];
		while (stack.length > 0) {
			const sha = stack.pop()!;
			if (seen.has(sha)) continue;
			seen.add(sha);
			const parents = parentsBySha.get(sha);
			if (parents) for (const p of parents) if (!seen.has(p)) stack.push(p);
		}
		return seen;
	});

	function isUnpulled(row: WipRow | GraphViewRow): boolean {
		return (
			hasRemoteRefs &&
			row.kind === "commit" &&
			remoteReachable.has(row.sha) &&
			!localReachable.has(row.sha)
		);
	}

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
			behind: v("--behind"),
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

	function firstOverlappingSpan(spans: readonly LanePassSpan[], row: number): number {
		let low = 0;
		let high = spans.length;
		while (low < high) {
			const mid = (low + high) >>> 1;
			if (spans[mid].endRow < row) low = mid + 1;
			else high = mid;
		}
		return low;
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
			// Settings → Appearance "show avatars" (DESIGN_SPEC.md §13) — off falls straight to the
			// plain lane-colored dot below, skipping both the real avatar and the initials fallback.
			const showAvatars = appSettings.current.appearance.showAvatars;
			const bitmap = row.meta && showAvatars ? avatars.get(row.meta.authorEmail) : null;
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
			} else if (row.meta && showAvatars) {
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
		} else if (isUnpushed(row)) {
			// Unpushed (local-only) commits get a dashed lane-colored halo so "not on the remote
			// yet" is visible at a glance right in the graph.
			ctx.beginPath();
			ctx.arc(x, y, AVATAR_RADIUS + 2.5, 0, Math.PI * 2);
			ctx.setLineDash([3, 3]);
			ctx.strokeStyle = c.lanes[row.node.colorIndex] ?? c.muted;
			ctx.lineWidth = 1.5;
			ctx.stroke();
			ctx.setLineDash([]);
		} else if (isUnpulled(row)) {
			// Mirror image: commit exists on a remote tip but isn't in local history yet — a subtle
			// `--behind`-colored dashed halo (same dash language as unpushed, opposite direction, own
			// color so the two are never confused at a glance).
			ctx.beginPath();
			ctx.arc(x, y, AVATAR_RADIUS + 2.5, 0, Math.PI * 2);
			ctx.setLineDash([3, 3]);
			ctx.strokeStyle = c.behind;
			ctx.lineWidth = 1.5;
			ctx.stroke();
			ctx.setLineDash([]);
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
		ctx.rect(graphLeft, 0, graphView.graphAuto, cssH);
		ctx.clip();

		const lit = hoveredSha ? lineageKeys(hoveredSha) : null;

		// Pass 1: straight-through lanes. They are stored as compact ranges and batched into one
		// path per palette color, rather than issuing thousands of individual strokes per frame.
		ctx.lineWidth = 1.6;
		const passPaths = Array.from({ length: 8 }, () => new Path2D());
		const litPassPaths = hoveredSha ? Array.from({ length: 8 }, () => new Path2D()) : null;
		const passRowOffset = wipOffset;
		for (let lane = 0; lane < graph.passSpansByLane.length; lane += 1) {
			const spans = graph.passSpansByLane[lane] ?? [];
			let spanIndex = firstOverlappingSpan(spans, Math.max(0, win.start - passRowOffset));
			for (; spanIndex < spans.length; spanIndex += 1) {
				const span = spans[spanIndex];
				const start = span.startRow + passRowOffset;
				const end = span.endRow + passRowOffset;
				if (start >= win.end) break;
				const clippedStart = Math.max(start, win.start);
				const clippedEnd = Math.min(end, win.end - 1);
				const x = graphLeft + laneCenterX(lane);
				const path = passPaths[laneColorIndex(lane)];
				path.moveTo(x, clippedStart * ROW_HEIGHT - st);
				path.lineTo(x, (clippedEnd + 1) * ROW_HEIGHT - st);
				if (litPassPaths) {
					const litPath = litPassPaths[laneColorIndex(lane)];
					for (let row = clippedStart; row <= clippedEnd; row += 1) {
						if (!lit?.has(`${row}:${lane}`)) continue;
						litPath.moveTo(x, row * ROW_HEIGHT - st);
						litPath.lineTo(x, (row + 1) * ROW_HEIGHT - st);
					}
				}
			}
		}
		ctx.globalAlpha = hoveredSha ? 0.3 : 0.9;
		for (let color = 0; color < passPaths.length; color += 1) {
			ctx.strokeStyle = c.lanes[color] ?? c.muted;
			ctx.stroke(passPaths[color]);
		}
		if (litPassPaths) {
			ctx.globalAlpha = 1;
			for (let color = 0; color < litPassPaths.length; color += 1) {
				ctx.strokeStyle = c.lanes[color] ?? c.muted;
				ctx.stroke(litPassPaths[color]);
			}
		}

		// Pass 2: branch, merge and node transitions. Drawn before nodes so avatars sit on top.
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

		// Pass 3: nodes + avatars.
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
		// Don't hijack keys while typing in a field inside the graph (e.g. the WIP commit input) —
		// otherwise j/k get eaten as vim-style navigation and never reach the input.
		const target = e.target as HTMLElement | null;
		if (target && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) {
			return;
		}
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
		if (row?.kind === "wip") return;
		if (row?.kind === "stash") {
			// Double-click a stash row → Pop with Undo toast (DESIGN_SPEC.md §4.5/§15.18).
			if (repoId) void actions.popStash(repoId, row.selector, row.subject);
			return;
		}
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

	function handleRowMenu(row: GraphViewRow, x: number, y: number) {
		if (row.kind === "commit") commitMenu = { sha: row.sha, x, y };
		else if (row.kind === "stash") stashMenu = { selector: row.selector, subject: row.subject, x, y };
	}

	function explainCommit(sha: string) {
		if (!repoId) return;
		graphSelection.select(sha);
		commitExplanation.open(repoId, sha);
	}

	function compareAgainstWorking(sha: string) {
		graphSelection.compareAgainstWorking(sha);
	}

	function startCreateBranch(sha: string) {
		branchEdit.startCreate(sha);
	}

	function startRename(pill: Pill) {
		if (pill.localBranch) branchEdit.startRename(pill.localBranch, pill.sha);
	}

	// Auto-size the GRAPH column to the lanes actually drawn: widest lane across the topology
	// (nodes and passing segments), clamped by the store. The column collapses to near-nothing on
	// a linear repo and grows as branches fan out — never crowding MESSAGE, never clipping lanes.
	$effect(() => {
		let maxLane = 0;
		for (const row of allRows) {
			if (row.node.lane > maxLane) maxLane = row.node.lane;
			for (const seg of row.segments) {
				if (seg.from.at !== "node" && seg.from.lane > maxLane) maxLane = seg.from.lane;
				if (seg.to.at !== "node" && seg.to.lane > maxLane) maxLane = seg.to.lane;
			}
		}
		maxLane = Math.max(maxLane, graph.laneColors.length - 1);
		graphView.setGraphAuto(graphWidthForLanes(maxLane));
	});

	// Redraw whenever the data, layout, hover or scroll offset changes; rAF coalesces bursts.
	$effect(() => {
		void allRows;
		void graph.metaBySha;
		void graphView.widths.branch;
		void graphView.graphAuto;
		void remoteReachable;
		void localReachable;
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
			commitMenu = null;
			stashMenu = null;
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
									onRowMenu={handleRowMenu}
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

{#if commitMenu && repoId}
	<CommitMenu
		sha={commitMenu.sha}
		{repoId}
		{currentBranch}
		x={commitMenu.x}
		y={commitMenu.y}
		onDismiss={() => (commitMenu = null)}
		onCreateBranch={startCreateBranch}
		onCompareWorking={compareAgainstWorking}
		onExplain={explainCommit}
	/>
{/if}

{#if stashMenu && repoId}
	<StashMenu
		selector={stashMenu.selector}
		subject={stashMenu.subject}
		{repoId}
		x={stashMenu.x}
		y={stashMenu.y}
		onDismiss={() => (stashMenu = null)}
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
		animation: shimmer var(--motion-loop) linear infinite;
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
