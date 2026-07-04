<script lang="ts">
	import { graph } from "$lib/stores/graph.svelte";
	import * as ipc from "$lib/ipc";
	import type { DiffTarget } from "$lib/stores/diffView.svelte";
	import type { FileDiff, Hunk } from "$lib/types";
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
			hunk.lines.forEach((line, i) => html.set(line, highlighted[i]));
			return { hunk, groups, html, large: hunk.lines.length > HUNK_COLLAPSE_THRESHOLD };
		});
	});
</script>

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
			<button type="button" disabled title="Lands with file history (prompt 14)">File History</button>
			<button type="button" disabled title="Lands with blame (prompt 14)">Blame</button>
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
							<button type="button" disabled title="Hunk staging lands with the Keep Panel work (ARCHITECTURE §6.3)">Stage hunk</button>
							<button type="button" disabled title="Hunk staging lands with the Keep Panel work (ARCHITECTURE §6.3)">Discard hunk…</button>
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
											<tr class="del">
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
											<tr class="add">
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
											<td class="lineno">{group.oldNo}</td>
											<td class="lineno">{group.newNo}</td>
											<td class="marker"></td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content">{@html prepared.html.get(group) ?? escapeHtml(group.text)}</td>
										</tr>
									{:else}
										<tr class={group.kind}>
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
							<button type="button" disabled title="Hunk staging lands with the Keep Panel work (ARCHITECTURE §6.3)">Stage hunk</button>
							<button type="button" disabled title="Hunk staging lands with the Keep Panel work (ARCHITECTURE §6.3)">Discard hunk…</button>
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
									<tr>
										{#if isLinePair(group)}
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
											<td class="lineno">{group.oldNo}</td>
											<td class="marker"></td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content">{@html prepared.html.get(group) ?? escapeHtml(group.text)}</td>
											<td class="lineno">{group.newNo}</td>
											<td class="marker"></td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content">{@html prepared.html.get(group) ?? escapeHtml(group.text)}</td>
										{:else if group.kind === "del"}
											<td class="lineno">{group.oldNo}</td>
											<td class="marker del">−</td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content del">{@html prepared.html.get(group) ?? escapeHtml(group.text)}</td>
											<td class="lineno"></td>
											<td class="marker"></td>
											<td class="content"></td>
										{:else}
											<td class="lineno"></td>
											<td class="marker"></td>
											<td class="content"></td>
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
		color: var(--text-faint);
		font: inherit;
		font-size: 11px;
		padding: 2px 8px;
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

	table.split td.lineno:nth-of-type(4) {
		border-left: 1px solid var(--border-soft);
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
