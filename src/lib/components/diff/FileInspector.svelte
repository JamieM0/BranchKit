<script lang="ts">
	import { graph } from "$lib/stores/graph.svelte";
	import { fileInspector, type FileInspectorMode } from "$lib/stores/fileInspector.svelte";
	import * as ipc from "$lib/ipc";
	import { toasts } from "$lib/stores/toasts.svelte";
	import { relativeTime } from "$lib/format";
	import { authorInitials, discColorIndex } from "$lib/graph/avatars";
	import { languageForPath } from "$lib/diff/language";
	import { escapeHtml, highlightLines } from "$lib/diff/highlight";
	import type { BlameRun, FileDiff, FileHistoryEntry } from "$lib/types";

	/** File History & Blame — DESIGN_SPEC.md §6.3. Replaces the graph center pane (same slot as
	 * `DiffViewer`), entered from a diff view's [File History]/[Blame] buttons or a file row's
	 * context menu. History: left column = `--follow` commit list (same row shape as the graph),
	 * right = that commit's diff for this file. Blame: full file, gutter groups line-runs by
	 * commit with an author disc + relative date, hover for a summary popover, click jumps to
	 * History at that commit. */
	let { path, onBack }: { path: string; onBack: () => void } = $props();

	const repoId = $derived(graph.repoId);
	const mode = $derived(fileInspector.target?.mode ?? "history");
	const language = $derived(languageForPath(path));

	// --- History ---
	let entries = $state<FileHistoryEntry[]>([]);
	let loadingHistory = $state(false);
	let selectedSha = $state<string | null>(null);
	let commitDiff = $state<FileDiff | null>(null);
	let loadingDiff = $state(false);

	async function loadHistory() {
		const id = repoId;
		if (!id) return;
		loadingHistory = true;
		try {
			entries = await ipc.getFileHistory(id, path);
			selectedSha = entries[0]?.sha ?? null;
		} catch (e) {
			toasts.pushError(e instanceof Error ? e.message : String(e));
			entries = [];
		} finally {
			loadingHistory = false;
		}
	}

	async function loadCommitDiff() {
		const id = repoId;
		const sha = selectedSha;
		if (!id || !sha) {
			commitDiff = null;
			return;
		}
		loadingDiff = true;
		try {
			const result = await ipc.getFileHistoryDiff(id, path, sha);
			if (sha === selectedSha) commitDiff = result;
		} catch (e) {
			if (sha === selectedSha) commitDiff = null;
			toasts.pushError(e instanceof Error ? e.message : String(e));
		} finally {
			if (sha === selectedSha) loadingDiff = false;
		}
	}

	function selectCommit(sha: string) {
		selectedSha = sha;
	}

	const preparedDiff = $derived.by(() => {
		if (!commitDiff) return [];
		return commitDiff.hunks.map((hunk) => {
			const html = highlightLines(
				hunk.lines.map((l) => l.text),
				language,
			);
			return { hunk, html };
		});
	});

	// --- Blame ---
	interface BlameLine {
		lineNo: number;
		sha: string;
		authorName: string;
		authorEmail: string;
		authorTime: number;
		summary: string;
		text: string;
		firstOfRun: boolean;
	}

	let blameRuns = $state<BlameRun[]>([]);
	let loadingBlame = $state(false);
	let hoveredRun = $state<{ run: BlameLine; x: number; y: number } | null>(null);

	const blameLines = $derived.by((): BlameLine[] => {
		const out: BlameLine[] = [];
		for (const run of blameRuns) {
			run.lines.forEach((text, i) => {
				out.push({
					lineNo: run.startLine + i,
					sha: run.sha,
					authorName: run.authorName,
					authorEmail: run.authorEmail,
					authorTime: run.authorTime,
					summary: run.summary,
					text,
					firstOfRun: i === 0,
				});
			});
		}
		return out;
	});

	const BLAME_ROW_HEIGHT = 18;
	let blameViewport: HTMLDivElement | undefined = $state();
	let blameScrollTop = $state(0);
	let blameViewportHeight = $state(400);
	const BLAME_OVERSCAN = 30;
	const blameRange = $derived.by(() => {
		const count = blameLines.length;
		if (count === 0) return { start: 0, end: 0 };
		const first = Math.floor(blameScrollTop / BLAME_ROW_HEIGHT) - BLAME_OVERSCAN;
		const last = Math.ceil((blameScrollTop + blameViewportHeight) / BLAME_ROW_HEIGHT) + BLAME_OVERSCAN;
		return { start: Math.max(0, first), end: Math.min(count, Math.max(0, last)) };
	});
	const visibleBlameLines = $derived(blameLines.slice(blameRange.start, blameRange.end));

	function onBlameScroll() {
		if (blameViewport) blameScrollTop = blameViewport.scrollTop;
	}

	async function loadBlame() {
		const id = repoId;
		if (!id) return;
		loadingBlame = true;
		try {
			blameRuns = await ipc.getBlame(id, path);
		} catch (e) {
			toasts.pushError(e instanceof Error ? e.message : String(e));
			blameRuns = [];
		} finally {
			loadingBlame = false;
		}
	}

	function isUncommitted(sha: string): boolean {
		return /^0+$/.test(sha);
	}

	function discColorVar(email: string): string {
		return `var(--lane-${discColorIndex(email)})`;
	}

	function jumpToHistory(sha: string) {
		hoveredRun = null;
		fileInspector.setMode("history");
		selectedSha = sha;
	}

	function setMode(next: FileInspectorMode) {
		fileInspector.setMode(next);
	}

	$effect(() => {
		void path;
		void repoId;
		void loadHistory();
	});

	$effect(() => {
		void selectedSha;
		void loadCommitDiff();
	});

	$effect(() => {
		void path;
		void repoId;
		if (mode === "blame" && blameRuns.length === 0 && !loadingBlame) void loadBlame();
	});
</script>

<div class="inspector">
	<div class="breadcrumb">
		<button type="button" class="back" onclick={onBack}>← Graph</button>
		<span class="sep">/</span>
		<span class="path">{path}</span>
		<div class="toggle-group" role="group" aria-label="History or blame">
			<button type="button" class:active={mode === "history"} onclick={() => setMode("history")}>
				History
			</button>
			<button type="button" class:active={mode === "blame"} onclick={() => setMode("blame")}>
				Blame
			</button>
		</div>
	</div>

	{#if mode === "history"}
		<div class="history">
			<div class="commit-list">
				{#if loadingHistory}
					<p class="empty">Loading history…</p>
				{:else if entries.length === 0}
					<p class="empty">No history for this file</p>
				{:else}
					{#each entries as entry (entry.sha)}
						<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
						<div
							class="commit-row"
							class:selected={entry.sha === selectedSha}
							onclick={() => selectCommit(entry.sha)}
						>
							<span class="subject">{entry.subject}</span>
							<span class="meta">{entry.authorName} · {relativeTime(entry.authorTime)}</span>
						</div>
					{/each}
				{/if}
			</div>
			<div class="commit-diff">
				{#if loadingDiff}
					<p class="empty">Loading diff…</p>
				{:else if !commitDiff}
					<p class="empty">No diff</p>
				{:else if commitDiff.isBinary}
					<p class="empty">Binary file — no inline diff.</p>
				{:else if commitDiff.hunks.length === 0}
					<p class="empty">No changes</p>
				{:else}
					{#each preparedDiff as prepared, hi (hi)}
						<div class="hunk">
							<div class="hunk-header">{prepared.hunk.header}</div>
							<table class="unified">
								<tbody>
									{#each prepared.hunk.lines as line, li (li)}
										<tr class={line.kind}>
											<td class="lineno">{line.oldNo ?? ""}</td>
											<td class="lineno">{line.newNo ?? ""}</td>
											<td class="marker">{line.kind === "add" ? "＋" : line.kind === "del" ? "−" : ""}</td>
											<!-- eslint-disable-next-line svelte/no-at-html-tags -->
											<td class="content">{@html prepared.html[li] ?? escapeHtml(line.text)}</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					{/each}
				{/if}
			</div>
		</div>
	{:else}
		<div
			class="blame"
			bind:this={blameViewport}
			onscroll={onBlameScroll}
			bind:clientHeight={blameViewportHeight}
		>
			{#if loadingBlame}
				<p class="empty">Loading blame…</p>
			{:else if blameLines.length === 0}
				<p class="empty">No blame data</p>
			{:else}
				<div class="blame-spacer" style="height: {blameLines.length * BLAME_ROW_HEIGHT}px;">
					<div
						class="blame-rows"
						style="transform: translateY({blameRange.start * BLAME_ROW_HEIGHT}px);"
					>
						{#each visibleBlameLines as line (line.lineNo)}
							<div class="blame-row" style="height: {BLAME_ROW_HEIGHT}px;">
								<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
								<div
									class="gutter"
									onmouseenter={(e) =>
										line.firstOfRun &&
										(hoveredRun = { run: line, x: e.clientX, y: e.clientY })}
									onmouseleave={() => (hoveredRun = null)}
									onclick={() => !isUncommitted(line.sha) && jumpToHistory(line.sha)}
								>
									{#if line.firstOfRun}
										<span
											class="disc"
											class:uncommitted={isUncommitted(line.sha)}
											style={`background: ${discColorVar(line.authorEmail)};`}
											title={line.authorName}
										>
											{authorInitials(line.authorName, line.authorEmail)}
										</span>
										<span class="when">{relativeTime(line.authorTime)}</span>
									{/if}
								</div>
								<span class="line-no">{line.lineNo}</span>
								<span class="line-text">{line.text}</span>
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>

{#if hoveredRun}
	<div
		class="popover"
		style="left: {hoveredRun.x + 12}px; top: {hoveredRun.y + 12}px;"
		role="tooltip"
	>
		<p class="summary">{hoveredRun.run.summary}</p>
		<p class="author">{hoveredRun.run.authorName} · {relativeTime(hoveredRun.run.authorTime)}</p>
		{#if !isUncommitted(hoveredRun.run.sha)}
			<p class="hint">Click to jump to History</p>
		{/if}
	</div>
{/if}

<style>
	.inspector {
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
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.toggle-group {
		display: flex;
		border: 1px solid var(--border);
		border-radius: var(--radius-pill);
		overflow: hidden;
	}

	.toggle-group button {
		border: none;
		background: var(--raised);
		color: var(--text-muted);
		font: inherit;
		font-size: 11px;
		padding: 2px 12px;
		cursor: pointer;
	}

	.toggle-group button.active {
		background: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}

	.history {
		flex: 1;
		min-height: 0;
		display: flex;
	}

	.commit-list {
		width: 280px;
		min-width: 280px;
		overflow-y: auto;
		border-right: 1px solid var(--border-soft);
	}

	.commit-row {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border-soft);
		cursor: pointer;
	}

	.commit-row:hover {
		background: var(--raised);
	}

	.commit-row.selected {
		background: var(--overlay);
	}

	.commit-row .subject {
		font-size: 12px;
		color: var(--text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.commit-row .meta {
		font-size: 10px;
		color: var(--text-faint);
	}

	.commit-diff {
		flex: 1;
		min-width: 0;
		overflow: auto;
	}

	.hunk {
		border-bottom: 1px solid var(--border-soft);
	}

	.hunk-header {
		padding: 2px var(--space-3);
		background: var(--raised);
		color: var(--info);
		font-family: var(--font-mono);
		font-size: 11px;
	}

	table.unified {
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
	tr.add .marker {
		background: color-mix(in srgb, var(--status-added) 12%, transparent);
		color: var(--status-added);
	}

	tr.del,
	tr.del .marker {
		background: color-mix(in srgb, var(--status-deleted) 12%, transparent);
		color: var(--status-deleted);
	}

	.empty {
		padding: var(--space-4);
		color: var(--text-faint);
		font-size: 12px;
	}

	.blame {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		font-family: var(--font-mono);
		font-size: var(--font-size-mono);
	}

	.blame-spacer {
		position: relative;
	}

	.blame-rows {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
	}

	.blame-row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 var(--space-3);
		white-space: pre;
	}

	.blame-row:hover {
		background: var(--raised);
	}

	.gutter {
		width: 160px;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		gap: 6px;
		height: 100%;
		cursor: pointer;
	}

	.disc {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		border-radius: var(--radius-pill);
		font-size: 8px;
		font-weight: 700;
		color: var(--bg);
		flex-shrink: 0;
	}

	.disc.uncommitted {
		background: var(--text-faint) !important;
	}

	.when {
		font-size: 10px;
		color: var(--text-faint);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.line-no {
		width: 40px;
		flex-shrink: 0;
		text-align: right;
		color: var(--text-faint);
		user-select: none;
	}

	.line-text {
		color: var(--text);
	}

	.popover {
		position: fixed;
		z-index: 95;
		max-width: 320px;
		padding: var(--space-2) var(--space-3);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
		font-size: 11px;
		pointer-events: none;
	}

	.popover .summary {
		margin: 0 0 4px;
		color: var(--text);
		font-weight: 600;
	}

	.popover .author {
		margin: 0;
		color: var(--text-muted);
	}

	.popover .hint {
		margin: 4px 0 0;
		color: var(--text-faint);
	}
</style>
