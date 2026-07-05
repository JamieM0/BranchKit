<script lang="ts">
	import { graph } from "$lib/stores/graph.svelte";
	import * as ipc from "$lib/ipc";
	import { toasts } from "$lib/stores/toasts.svelte";
	import type { DiffTarget } from "$lib/stores/diffView.svelte";
	import { fileInspector } from "$lib/stores/fileInspector.svelte";
	import type { DiffLine, FileDiff, Hunk } from "$lib/types";
	import { languageForPath } from "$lib/diff/language";
	import { escapeHtml, highlightLines } from "$lib/diff/highlight";
	import { isLinePair, pairChangedLines, type RenderLine } from "$lib/diff/pairLines";
	import { wordDiff } from "$lib/diff/wordDiff";

	/** The diff viewer — DESIGN_SPEC.md §6.2. Replaces the graph center pane (the parent decides
	 * that; this component only renders once it's mounted) with a breadcrumb back to the graph. */
	let { target, onBack }: { target: DiffTarget; onBack: () => void } = $props();

	const repoId = $derived(graph.repoId);
	const language = $derived(languageForPath(target.path));

	const HUNK_COLLAPSE_THRESHOLD = 400;
	const IMAGE_MIME: Record<string, string> = {
		png: "image/png",
		jpg: "image/jpeg",
		jpeg: "image/jpeg",
		gif: "image/gif",
		bmp: "image/bmp",
		webp: "image/webp",
		ico: "image/x-icon",
		svg: "image/svg+xml",
	};

	let diff = $state<FileDiff | null>(null);
	let loadError = $state<string | null>(null);
	let loading = $state(false);
	let viewMode = $state<"unified" | "split">("unified");
	let ignoreWhitespace = $state(false);
	let expandedHunks = $state<Set<number>>(new Set());

	let beforeImage = $state<string | null>(null);
	let afterImage = $state<string | null>(null);
	let beforeDims = $state<{ w: number; h: number } | null>(null);
	let afterDims = $state<{ w: number; h: number } | null>(null);

	/** Gutter line staging (DESIGN_SPEC.md §6.2/§15.11) — only meaningful for the two live working
	 * copies, not a read-only commit/compare diff. `staged` view stages in reverse (unstage). */
	const isWorkingCopy = $derived(
		target.source.kind === "workingTree" || target.source.kind === "staged",
	);
	const isStagedView = $derived(target.source.kind === "staged");
	/** The backend always diffs without `-w`, so gutter line indices only line up with a diff
	 * fetched the same way — disable the interactive gutter while whitespace is hidden rather than
	 * risk staging the wrong lines. */
	const gutterEnabled = $derived(isWorkingCopy && !ignoreWhitespace);
	let busy = $state(false);
	let dragState = $state<{ hunkIndex: number; hunk: Hunk; anchor: number; current: number } | null>(
		null,
	);

	function lineIndicesInRange(hunk: Hunk, from: number, to: number): number[] {
		const lo = Math.min(from, to);
		const hi = Math.max(from, to);
		const indices: number[] = [];
		for (let i = lo; i <= hi; i += 1) {
			if (hunk.lines[i] && hunk.lines[i].kind !== "context") indices.push(i);
		}
		return indices;
	}

	function allChangeIndices(hunk: Hunk): number[] {
		return hunk.lines.reduce<number[]>((acc, l, i) => {
			if (l.kind !== "context") acc.push(i);
			return acc;
		}, []);
	}

	function asAppError(e: unknown): { userMessage: string; raw: string } {
		if (e && typeof e === "object" && "userMessage" in e) {
			const o = e as Record<string, unknown>;
			return { userMessage: String(o.userMessage), raw: String(o.raw ?? "") };
		}
		return { userMessage: e instanceof Error ? e.message : String(e), raw: String(e) };
	}

	async function applyLineSelection(hunkIndex: number, indices: number[]) {
		const id = repoId;
		if (!id || indices.length === 0 || busy) return;
		busy = true;
		try {
			if (isStagedView) {
				await ipc.unstageLines(id, target.path, hunkIndex, indices);
			} else {
				await ipc.stageLines(id, target.path, hunkIndex, indices);
			}
			await loadDiff();
		} catch (e) {
			const { userMessage, raw } = asAppError(e);
			toasts.pushError(userMessage, raw);
		} finally {
			busy = false;
		}
	}

	async function stageOrUnstageHunk(hunkIndex: number, hunk: Hunk) {
		await applyLineSelection(hunkIndex, allChangeIndices(hunk));
	}

	async function discardHunk(hunkIndex: number) {
		const id = repoId;
		if (!id || busy) return;
		busy = true;
		try {
			await ipc.discardHunk(id, target.path, hunkIndex);
			await loadDiff();
			toasts.push({
				message: "Discarded a change",
				tone: "warn",
				destructive: true,
				action: {
					label: "Undo",
					run: async () => {
						// A fresh call, not the id captured above — discardHunk's own trash write
						// created a new entry each time, and the toast always refers to the latest.
						const entries = await ipc.listDiscarded(id);
						const latest = entries[0];
						if (latest) await ipc.restoreDiscarded(id, latest.id);
						await loadDiff();
					},
				},
			});
		} catch (e) {
			const { userMessage, raw } = asAppError(e);
			toasts.pushError(userMessage, raw);
		} finally {
			busy = false;
		}
	}

	function gutterMouseDown(hunkIndex: number, hunk: Hunk, lineIdx: number, line: DiffLine) {
		if (!gutterEnabled || busy || line.kind === "context") return;
		dragState = { hunkIndex, hunk, anchor: lineIdx, current: lineIdx };
	}

	function gutterMouseEnter(hunkIndex: number, lineIdx: number) {
		if (!dragState || dragState.hunkIndex !== hunkIndex) return;
		dragState = { ...dragState, current: lineIdx };
	}

	function commitDrag() {
		if (!dragState) return;
		const { hunkIndex, hunk, anchor, current } = dragState;
		dragState = null;
		void applyLineSelection(hunkIndex, lineIndicesInRange(hunk, anchor, current));
	}

	function isDragging(hunkIndex: number, lineIdx: number): boolean {
		if (!dragState || dragState.hunkIndex !== hunkIndex) return false;
		const lo = Math.min(dragState.anchor, dragState.current);
		const hi = Math.max(dragState.anchor, dragState.current);
		return lineIdx >= lo && lineIdx <= hi;
	}

	function sourceLabel(): string {
		switch (target.source.kind) {
			case "workingTree":
				return "Working tree";
			case "staged":
				return "Staged";
			case "commit":
				return `${target.source.sha.slice(0, 7)}`;
			case "compare":
				return `${target.source.a.slice(0, 7)} ↔ ${target.source.b.slice(0, 7)}`;
			case "commitVsWorking":
				return `${target.source.sha.slice(0, 7)} ↔ Working directory`;
		}
	}

	/** Before/after revision strings for `ipc.getBlob` — `null` = worktree disk, `":"` = index. */
	function blobRevisions(): { before: string | null; after: string | null } {
		switch (target.source.kind) {
			case "workingTree":
				return { before: ":", after: null };
			case "staged":
				return { before: "HEAD", after: ":" };
			case "commit":
				return { before: `${target.source.sha}^`, after: target.source.sha };
			case "compare":
				return { before: target.source.a, after: target.source.b };
			case "commitVsWorking":
				return { before: target.source.sha, after: null };
		}
	}

	async function loadDiff() {
		const id = repoId;
		if (!id) return;
		loading = true;
		loadError = null;
		diff = null;
		beforeImage = null;
		afterImage = null;
		beforeDims = null;
		afterDims = null;
		expandedHunks = new Set();
		try {
			const path = target.path;
			const iw = ignoreWhitespace;
			let result: FileDiff;
			switch (target.source.kind) {
				case "workingTree":
					result = await ipc.getDiffWorktree(id, path, iw);
					break;
				case "staged":
					result = await ipc.getDiffStaged(id, path, iw);
					break;
				case "commit":
					result = await ipc.getDiffCommit(id, target.source.sha, path, iw);
					break;
				case "compare":
					result = await ipc.getDiffTwoCommits(id, target.source.a, target.source.b, path, iw);
					break;
				case "commitVsWorking":
					result = await ipc.getDiffCommitVsWorking(id, target.source.sha, path, iw);
					break;
			}
			diff = result;
			if (result.isImage) await loadImages(id, result);
		} catch (e) {
			loadError = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	async function loadImages(id: string, result: FileDiff) {
		const { before, after } = blobRevisions();
		const path = target.path;
		if (result.oldPath) {
			try {
				beforeImage = await ipc.getBlob(id, before, target.origPath ?? path);
			} catch {
				beforeImage = null;
			}
		}
		if (result.newPath) {
			try {
				afterImage = await ipc.getBlob(id, after, path);
			} catch {
				afterImage = null;
			}
		}
	}

	function mimeFor(path: string): string {
		const ext = path.split(".").pop()?.toLowerCase() ?? "";
		return IMAGE_MIME[ext] ?? "application/octet-stream";
	}

	$effect(() => {
		void target;
		void ignoreWhitespace;
		void loadDiff();
	});

	function toggleHunk(index: number) {
		const next = new Set(expandedHunks);
		if (next.has(index)) next.delete(index);
		else next.add(index);
		expandedHunks = next;
	}

	interface PreparedHunk {
		hunk: Hunk;
		groups: RenderLine[];
		html: Map<object, string>;
		/** Maps a `DiffLine` (by reference — hunk.lines are stable objects for the diff's
		 * lifetime) back to its index in `hunk.lines`, which is exactly the index the backend's
		 * `stage_lines`/`unstage_lines`/`discard_hunk` expect (ARCHITECTURE.md §6.3). */
		lineIndex: Map<object, number>;
		large: boolean;
	}

	const preparedHunks = $derived.by((): PreparedHunk[] => {
		if (!diff) return [];
		return diff.hunks.map((hunk) => {
			const groups = pairChangedLines(hunk.lines);
			const highlighted = highlightLines(
				hunk.lines.map((l) => l.text),
				language,
			);
			const html = new Map<object, string>();
			const lineIndex = new Map<object, number>();
			hunk.lines.forEach((line, i) => {
				html.set(line, highlighted[i]);
				lineIndex.set(line, i);
			});
			return { hunk, groups, html, lineIndex, large: hunk.lines.length > HUNK_COLLAPSE_THRESHOLD };
		});
	});
</script>

<svelte:window onmouseup={commitDrag} />

<div class="viewer">
	<div class="breadcrumb">
		<button type="button" class="back" onclick={onBack}>← Graph</button>
		<span class="sep">/</span>
		<span class="path">{target.path}</span>
		{#if target.origPath}<span class="rename">renamed from {target.origPath}</span>{/if}
		<span class="source-label">{sourceLabel()}</span>
	</div>

	<div class="toolbar">
		<div class="toggle-group" role="group" aria-label="Diff view mode">
			<button type="button" class:active={viewMode === "unified"} onclick={() => (viewMode = "unified")}>
				Unified
			</button>
			<button type="button" class:active={viewMode === "split"} onclick={() => (viewMode = "split")}>
				Split
			</button>
		</div>
		<label class="whitespace-toggle">
			<input type="checkbox" bind:checked={ignoreWhitespace} />
			Ignore whitespace
		</label>
		{#if ignoreWhitespace}<span class="ws-indicator" title="Whitespace-only changes are hidden">⌗ whitespace hidden</span>{/if}
		<div class="stub-actions">
			<button type="button" onclick={() => fileInspector.open(target.path, "history")}>File History</button>
			<button type="button" onclick={() => fileInspector.open(target.path, "blame")}>Blame</button>
			<button type="button" disabled title="Lands with the external-open integration (prompt 14)">Open file</button>
		</div>
	</div>

	<div class="body">
		{#if loading}
			<p class="empty">Loading diff…</p>
		{:else if loadError}
			<p class="empty error">{loadError}</p>
		{:else if !diff}
			<p class="empty">No diff</p>
		{:else if diff.isBinary && diff.isImage}
			<div class="image-diff">
				<div class="image-side">
					<h4>Before</h4>
					<div class="checkerboard">
						{#if beforeImage}
							<img
								src={`data:${mimeFor(target.origPath ?? target.path)};base64,${beforeImage}`}
								alt="Before"
								onload={(e) => {
									const img = e.currentTarget as HTMLImageElement;
									beforeDims = { w: img.naturalWidth, h: img.naturalHeight };
								}}
							/>
						{:else}
							<span class="none">No previous version</span>
						{/if}
					</div>
					{#if beforeDims}<span class="dims">{beforeDims.w} × {beforeDims.h}</span>{/if}
				</div>
				<div class="image-side">
					<h4>After</h4>
					<div class="checkerboard">
						{#if afterImage}
							<img
								src={`data:${mimeFor(target.path)};base64,${afterImage}`}
								alt="After"
								onload={(e) => {
									const img = e.currentTarget as HTMLImageElement;
									afterDims = { w: img.naturalWidth, h: img.naturalHeight };
								}}
							/>
						{:else}
							<span class="none">Deleted</span>
						{/if}
					</div>
					{#if afterDims}<span class="dims">{afterDims.w} × {afterDims.h}</span>{/if}
				</div>
			</div>
		{:else if diff.isBinary}
			<div class="binary">
				<p>Binary file — no inline diff.</p>
				<button type="button" disabled title="Lands with the external-open integration (prompt 14)">
					Open in external tool
				</button>
			</div>
		{:else if diff.hunks.length === 0}
			<p class="empty">No changes</p>
		{:else if viewMode === "unified"}
			{#each preparedHunks as prepared, hi (hi)}
				<div class="hunk">
					<div class="hunk-header">
						<span class="hunk-header-text">{prepared.hunk.header}</span>
						<div class="hunk-actions">
							{#if isWorkingCopy}
								<button
									type="button"
									disabled={!gutterEnabled || busy}
									title={gutterEnabled ? "" : "Disabled while whitespace is hidden — turn that off to stage/discard by hunk"}
									onclick={() => void stageOrUnstageHunk(hi, prepared.hunk)}
								>
									{isStagedView ? "Unstage hunk" : "Stage hunk"}
								</button>
								{#if !isStagedView}
									<button
										type="button"
										disabled={!gutterEnabled || busy}
										title={gutterEnabled ? "" : "Disabled while whitespace is hidden — turn that off to stage/discard by hunk"}
										onclick={() => void discardHunk(hi)}
									>
										Discard hunk…
									</button>
								{/if}
							{:else}
								<button type="button" disabled title="Only the working tree and staged diffs can be modified">Stage hunk</button>
							{/if}
						</div>
					</div>
					{#if prepared.large && !expandedHunks.has(hi)}
						<button type="button" class="collapse-toggle" onclick={() => toggleHunk(hi)}>
							… {prepared.hunk.lines.length} unchanged lines — expand
						</button>
					{:else}
						<table class="unified">
							<tbody>
								{#each prepared.groups as group, gi (gi)}
									{#if isLinePair(group)}
										{@const wd = group.del && group.add ? wordDiff(group.del.text, group.add.text) : null}
										{#if group.del}
											{@const idx = prepared.lineIndex.get(group.del)}
											<tr class="del" class:staged-pending={idx !== undefined && isDragging(hi, idx)}>
												<td
													class="gutter"
													class:interactive={gutterEnabled}
													onmousedown={() => idx !== undefined && gutterMouseDown(hi, prepared.hunk, idx, group.del!)}
													onmouseenter={() => idx !== undefined && gutterMouseEnter(hi, idx)}
												>
													{#if gutterEnabled}<span class="check" aria-hidden="true"></span>{/if}
												</td>
												<td class="lineno">{group.del.oldNo}</td>
												<td class="lineno"></td>
												<td class="marker">−</td>
												<td class="content">
													{#if wd}
														{#each wd.oldSegments as seg, si (si)}<span class:changed={seg.changed}>{seg.text}</span>{/each}
													{:else}
														{group.del.text}
													{/if}
												</td>
											</tr>
										{/if}
										{#if group.add}
											{@const idx = prepared.lineIndex.get(group.add)}
											<tr class="add" class:staged-pending={idx !== undefined && isDragging(hi, idx)}>
												<td
													class="gutter"
													class:interactive={gutterEnabled}
													onmousedown={() => idx !== undefined && gutterMouseDown(hi, prepared.hunk, idx, group.add!)}
													onmouseenter={() => idx !== undefined && gutterMouseEnter(hi, idx)}
												>
													{#if gutterEnabled}<span class="check" aria-hidden="true"></span>{/if}
												</td>
												<td class="lineno"></td>
												<td class="lineno">{group.add.newNo}</td>
												<td class="marker">＋</td>
												<td class="content">
													{#if wd}
														{#each wd.newSegments as seg, si (si)}<span class:changed={seg.changed}>{seg.text}</span>{/each}
													{:else}
														{group.add.text}
													{/if}
												</td>
											</tr>
										{/if}
									{:else if group.kind === "context"}
										<tr class="context">
											<td class="gutter"></td>
											<td class="lineno">{group.oldNo}</td>
											<td class="lineno">{group.newNo}</td>
											<td class="marker"></td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content">{@html prepared.html.get(group) ?? escapeHtml(group.text)}</td>
										</tr>
									{:else}
										{@const idx = prepared.lineIndex.get(group)}
										<tr class={group.kind} class:staged-pending={idx !== undefined && isDragging(hi, idx)}>
											<td
												class="gutter"
												class:interactive={gutterEnabled}
												onmousedown={() => idx !== undefined && gutterMouseDown(hi, prepared.hunk, idx, group)}
												onmouseenter={() => idx !== undefined && gutterMouseEnter(hi, idx)}
											>
												{#if gutterEnabled}<span class="check" aria-hidden="true"></span>{/if}
											</td>
											<td class="lineno">{group.oldNo ?? ""}</td>
											<td class="lineno">{group.newNo ?? ""}</td>
											<td class="marker">{group.kind === "add" ? "＋" : "−"}</td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content">{@html prepared.html.get(group) ?? escapeHtml(group.text)}</td>
										</tr>
									{/if}
								{/each}
							</tbody>
						</table>
					{/if}
				</div>
			{/each}
		{:else}
			{#each preparedHunks as prepared, hi (hi)}
				<div class="hunk">
					<div class="hunk-header">
						<span class="hunk-header-text">{prepared.hunk.header}</span>
						<div class="hunk-actions">
							{#if isWorkingCopy}
								<button
									type="button"
									disabled={!gutterEnabled || busy}
									title={gutterEnabled ? "" : "Disabled while whitespace is hidden — turn that off to stage/discard by hunk"}
									onclick={() => void stageOrUnstageHunk(hi, prepared.hunk)}
								>
									{isStagedView ? "Unstage hunk" : "Stage hunk"}
								</button>
								{#if !isStagedView}
									<button
										type="button"
										disabled={!gutterEnabled || busy}
										title={gutterEnabled ? "" : "Disabled while whitespace is hidden — turn that off to stage/discard by hunk"}
										onclick={() => void discardHunk(hi)}
									>
										Discard hunk…
									</button>
								{/if}
							{:else}
								<button type="button" disabled title="Only the working tree and staged diffs can be modified">Stage hunk</button>
							{/if}
						</div>
					</div>
					{#if prepared.large && !expandedHunks.has(hi)}
						<button type="button" class="collapse-toggle" onclick={() => toggleHunk(hi)}>
							… {prepared.hunk.lines.length} unchanged lines — expand
						</button>
					{:else}
						<table class="split">
							<tbody>
								{#each prepared.groups as group, gi (gi)}
									{@const wd = isLinePair(group) && group.del && group.add ? wordDiff(group.del.text, group.add.text) : null}
									{@const delIdx = isLinePair(group) ? (group.del ? prepared.lineIndex.get(group.del) : undefined) : (group.kind === "del" ? prepared.lineIndex.get(group) : undefined)}
									{@const addIdx = isLinePair(group) ? (group.add ? prepared.lineIndex.get(group.add) : undefined) : (group.kind === "add" ? prepared.lineIndex.get(group) : undefined)}
									<tr
										class:staged-pending={(delIdx !== undefined && isDragging(hi, delIdx)) ||
											(addIdx !== undefined && isDragging(hi, addIdx))}
									>
										{#if isLinePair(group)}
											<td
												class="gutter"
												class:interactive={gutterEnabled && !!group.del}
												onmousedown={() => group.del && delIdx !== undefined && gutterMouseDown(hi, prepared.hunk, delIdx, group.del)}
												onmouseenter={() => delIdx !== undefined && gutterMouseEnter(hi, delIdx)}
											>
												{#if gutterEnabled && group.del}<span class="check" aria-hidden="true"></span>{/if}
											</td>
											<td class="lineno">{group.del?.oldNo ?? ""}</td>
											<td class="marker del">{group.del ? "−" : ""}</td>
											<td class="content del">
												{#if group.del}
													{#if wd}
														{#each wd.oldSegments as seg, si (si)}<span class:changed={seg.changed}>{seg.text}</span>{/each}
													{:else}
														{group.del.text}
													{/if}
												{/if}
											</td>
											<td
												class="gutter"
												class:interactive={gutterEnabled && !!group.add}
												onmousedown={() => group.add && addIdx !== undefined && gutterMouseDown(hi, prepared.hunk, addIdx, group.add)}
												onmouseenter={() => addIdx !== undefined && gutterMouseEnter(hi, addIdx)}
											>
												{#if gutterEnabled && group.add}<span class="check" aria-hidden="true"></span>{/if}
											</td>
											<td class="lineno">{group.add?.newNo ?? ""}</td>
											<td class="marker add">{group.add ? "＋" : ""}</td>
											<td class="content add">
												{#if group.add}
													{#if wd}
														{#each wd.newSegments as seg, si (si)}<span class:changed={seg.changed}>{seg.text}</span>{/each}
													{:else}
														{group.add.text}
													{/if}
												{/if}
											</td>
										{:else if group.kind === "context"}
											<td class="gutter"></td>
											<td class="lineno">{group.oldNo}</td>
											<td class="marker"></td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content">{@html prepared.html.get(group) ?? escapeHtml(group.text)}</td>
											<td class="gutter"></td>
											<td class="lineno">{group.newNo}</td>
											<td class="marker"></td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content">{@html prepared.html.get(group) ?? escapeHtml(group.text)}</td>
										{:else if group.kind === "del"}
											<td
												class="gutter"
												class:interactive={gutterEnabled}
												onmousedown={() => delIdx !== undefined && gutterMouseDown(hi, prepared.hunk, delIdx, group)}
												onmouseenter={() => delIdx !== undefined && gutterMouseEnter(hi, delIdx)}
											>
												{#if gutterEnabled}<span class="check" aria-hidden="true"></span>{/if}
											</td>
											<td class="lineno">{group.oldNo}</td>
											<td class="marker del">−</td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content del">{@html prepared.html.get(group) ?? escapeHtml(group.text)}</td>
											<td class="gutter"></td>
											<td class="lineno"></td>
											<td class="marker"></td>
											<td class="content"></td>
										{:else}
											<td class="gutter"></td>
											<td class="lineno"></td>
											<td class="marker"></td>
											<td class="content"></td>
											<td
												class="gutter"
												class:interactive={gutterEnabled}
												onmousedown={() => addIdx !== undefined && gutterMouseDown(hi, prepared.hunk, addIdx, group)}
												onmouseenter={() => addIdx !== undefined && gutterMouseEnter(hi, addIdx)}
											>
												{#if gutterEnabled}<span class="check" aria-hidden="true"></span>{/if}
											</td>
											<td class="lineno">{group.newNo}</td>
											<td class="marker add">＋</td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content add">{@html prepared.html.get(group) ?? escapeHtml(group.text)}</td>
										{/if}
									</tr>
								{/each}
							</tbody>
						</table>
					{/if}
				</div>
			{/each}
		{/if}
	</div>
</div>

<style>
	.viewer {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg);
	}

	.breadcrumb {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border);
		font-size: 12px;
		flex-shrink: 0;
	}

	.back {
		border: none;
		background: none;
		color: var(--info);
		font: inherit;
		font-size: 12px;
		cursor: pointer;
		padding: 0;
	}

	.back:hover {
		text-decoration: underline;
	}

	.sep {
		color: var(--text-faint);
	}

	.path {
		font-family: var(--font-mono);
		color: var(--text);
	}

	.rename,
	.source-label {
		color: var(--text-faint);
		font-size: 11px;
	}

	.source-label {
		margin-left: auto;
	}

	.toolbar {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border-soft);
		flex-shrink: 0;
	}

	.toggle-group {
		display: flex;
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		overflow: hidden;
	}

	.toggle-group button {
		border: none;
		background: var(--raised);
		color: var(--text-muted);
		font: inherit;
		font-size: 11px;
		padding: 2px 10px;
		cursor: pointer;
	}

	.toggle-group button.active {
		background: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}

	.whitespace-toggle {
		display: flex;
		align-items: center;
		gap: 4px;
		font-size: 11px;
		color: var(--text-muted);
		cursor: pointer;
	}

	.ws-indicator {
		font-size: 10px;
		color: var(--warn);
	}

	.stub-actions {
		margin-left: auto;
		display: flex;
		gap: var(--space-2);
	}

	.stub-actions button {
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 11px;
		padding: 2px 8px;
		cursor: pointer;
	}

	.stub-actions button:hover:not(:disabled) {
		background: var(--overlay);
	}

	.stub-actions button:disabled {
		color: var(--text-faint);
		cursor: not-allowed;
	}

	.body {
		flex: 1;
		overflow: auto;
	}

	.empty {
		padding: var(--space-4);
		color: var(--text-faint);
		font-size: 12px;
	}

	.empty.error {
		color: var(--danger);
	}

	.hunk {
		border-bottom: 1px solid var(--border-soft);
	}

	.hunk-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 2px var(--space-3);
		background: var(--raised);
		color: var(--info);
		font-family: var(--font-mono);
		font-size: 11px;
	}

	.hunk-actions {
		display: flex;
		gap: 6px;
	}

	.hunk-actions button {
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--surface);
		color: var(--text-faint);
		font: inherit;
		font-size: 10px;
		padding: 0 6px;
		cursor: not-allowed;
	}

	.collapse-toggle {
		display: block;
		width: 100%;
		border: none;
		background: var(--surface);
		color: var(--text-muted);
		font: inherit;
		font-size: 11px;
		padding: var(--space-2);
		cursor: pointer;
		text-align: center;
	}

	.collapse-toggle:hover {
		background: var(--raised);
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-family: var(--font-mono);
		font-size: var(--font-size-mono);
	}

	.lineno {
		width: 40px;
		padding: 0 6px;
		text-align: right;
		color: var(--text-faint);
		user-select: none;
		white-space: nowrap;
	}

	.marker {
		width: 16px;
		text-align: center;
		user-select: none;
	}

	.content {
		white-space: pre;
		padding-right: var(--space-3);
	}

	tr.add,
	td.content.add {
		background: color-mix(in srgb, var(--status-added) 12%, transparent);
	}

	tr.del,
	td.content.del {
		background: color-mix(in srgb, var(--status-deleted) 12%, transparent);
	}

	tr.add .marker,
	.marker.add {
		color: var(--status-added);
	}

	tr.del .marker,
	.marker.del {
		color: var(--status-deleted);
	}

	.content span.changed {
		background: color-mix(in srgb, var(--accent) 35%, transparent);
		border-radius: 2px;
	}

	table.split td:nth-of-type(5) {
		border-left: 1px solid var(--border-soft);
	}

	.gutter {
		width: 16px;
		padding: 0;
		text-align: center;
		user-select: none;
		cursor: default;
	}

	.gutter.interactive {
		cursor: pointer;
	}

	.gutter.interactive .check {
		display: inline-block;
		width: 9px;
		height: 9px;
		border: 1.5px solid var(--text-faint);
		border-radius: 2px;
		opacity: 0;
		transition: opacity var(--motion-hover), border-color var(--motion-hover);
	}

	tr:hover > .gutter.interactive .check {
		opacity: 1;
	}

	.gutter.interactive:hover .check {
		border-color: var(--accent);
		background: color-mix(in srgb, var(--accent) 20%, transparent);
	}

	tr.staged-pending {
		outline: 1px solid var(--accent);
		outline-offset: -1px;
	}

	tr.staged-pending .gutter .check {
		opacity: 1;
		background: var(--accent);
		border-color: var(--accent);
	}

	.binary {
		padding: var(--space-4);
		text-align: center;
		color: var(--text-muted);
	}

	.binary button {
		margin-top: var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text-faint);
		font: inherit;
		padding: var(--space-2) var(--space-3);
		cursor: not-allowed;
	}

	.image-diff {
		display: flex;
		gap: var(--space-4);
		padding: var(--space-4);
	}

	.image-side {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
	}

	.image-side h4 {
		margin: 0;
		font-size: 11px;
		color: var(--text-muted);
	}

	.checkerboard {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		min-height: 160px;
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		background-image: linear-gradient(45deg, var(--raised) 25%, transparent 25%),
			linear-gradient(-45deg, var(--raised) 25%, transparent 25%),
			linear-gradient(45deg, transparent 75%, var(--raised) 75%),
			linear-gradient(-45deg, transparent 75%, var(--raised) 75%);
		background-size: 16px 16px;
		background-position: 0 0, 0 8px, 8px -8px, -8px 0;
		background-color: var(--surface);
	}

	.checkerboard img {
		max-width: 100%;
		max-height: 320px;
	}

	.none {
		color: var(--text-faint);
		font-size: 12px;
	}

	.dims {
		font-size: 11px;
		color: var(--text-faint);
	}
</style>
