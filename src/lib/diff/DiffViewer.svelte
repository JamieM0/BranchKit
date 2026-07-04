<script lang="ts">
	import { diffView } from "$lib/stores/diffView.svelte";
	import { graph } from "$lib/stores/graph.svelte";
	import * as ipc from "$lib/ipc";
	import type { FileDiff, DiffLine, Hunk } from "$lib/types";
	import { pairChangedLines, isLinePair } from "./pairLines";
	import { wordDiff } from "./wordDiff";
	import { languageForPath } from "./language";
	import hljs from "highlight.js";
	import "highlight.js/styles/github-dark.css";

	const repoId = $derived(graph.repoId);
	const target = $derived(diffView.target);

	let diff = $state<FileDiff | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	
	let splitView = $state(false);
	let ignoreWhitespace = $state(false);
	let collapsedHunks = $state<Set<number>>(new Set());

	$effect(() => {
		if (!target || !repoId) return;
		loading = true;
		error = null;
		diff = null;
		collapsedHunks = new Set();
		
		const fetchDiff = async () => {
			if (target.source.kind === "workingTree") {
				return await ipc.getDiffWorktree(repoId, target.path, ignoreWhitespace);
			} else if (target.source.kind === "staged") {
				return await ipc.getDiffStaged(repoId, target.path, ignoreWhitespace);
			} else if (target.source.kind === "commit") {
				return await ipc.getDiffCommit(repoId, target.source.sha, target.path, ignoreWhitespace);
			} else if (target.source.kind === "compare") {
				return await ipc.getDiffTwoCommits(repoId, target.source.a, target.source.b, target.path, ignoreWhitespace);
			}
			throw new Error("Unknown source kind");
		};
		
		fetchDiff()
			.then((res) => {
				if (diffView.target !== target) return;
				diff = res;
				// Collapse hunks > 400 lines
				res.hunks.forEach((h, i) => {
					if (h.lines.length > 400) collapsedHunks.add(i);
				});
			})
			.catch((err) => {
				if (diffView.target !== target) return;
				error = err.message || String(err);
			})
			.finally(() => {
				if (diffView.target === target) loading = false;
			});
	});

	function toggleCollapse(index: number) {
		const newSet = new Set(collapsedHunks);
		if (newSet.has(index)) newSet.delete(index);
		else newSet.add(index);
		collapsedHunks = newSet;
	}

	function escapeHtml(text: string): string {
		return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
	}

	function highlightHunk(hunk: Hunk, path: string): string[] {
		const lang = languageForPath(path);
		const fullText = hunk.lines.map((l) => l.text).join("\n");
		if (lang && hljs.getLanguage(lang)) {
			try {
				const html = hljs.highlight(fullText, { language: lang, ignoreIllegals: true }).value;
				return html.split("\n");
			} catch {}
		}
		return hunk.lines.map((l) => escapeHtml(l.text));
	}
</script>

<div class="diff-viewer">
	<div class="toolbar">
		<button type="button" class="back-btn" onclick={() => diffView.close()}>
			← Graph <span class="sep">/</span> {target?.path}
		</button>
		<div class="actions">
			<label class="toggle">
				<input type="checkbox" bind:checked={ignoreWhitespace} />
				Ignore Whitespace
			</label>
			<div class="tabs">
				<button type="button" class:active={!splitView} onclick={() => splitView = false}>Unified</button>
				<button type="button" class:active={splitView} onclick={() => splitView = true}>Split</button>
			</div>
			<div class="btn-group">
				<button type="button">History</button>
				<button type="button">Blame</button>
				<button type="button">Open file</button>
			</div>
		</div>
	</div>

	<div class="content" class:split={splitView}>
		{#if loading}
			<div class="message">Loading diff...</div>
		{:else if error}
			<div class="message error">{error}</div>
		{:else if diff}
			{#if diff.isBinary}
				<div class="message binary">
					{#if diff.isImage}
						<div class="image-diff">
							<div class="checkerboard old-image">Image diff placeholder (old)</div>
							<div class="checkerboard new-image">Image diff placeholder (new)</div>
						</div>
					{:else}
						Binary file — Open in external tool
					{/if}
				</div>
			{:else if diff.hunks.length === 0}
				<div class="message">No differences</div>
			{:else}
				{#each diff.hunks as hunk, hIndex}
					{@const collapsed = collapsedHunks.has(hIndex)}
					<div class="hunk">
						<div class="hunk-header">
							<span class="hunk-text">{hunk.header}</span>
							<button type="button" class="collapse-btn" onclick={() => toggleCollapse(hIndex)}>
								{collapsed ? "Expand" : "Collapse"}
							</button>
							<div class="hunk-actions">
								<button type="button">Stage hunk</button>
								<button type="button">Discard hunk...</button>
							</div>
						</div>
						
						{#if collapsed}
							<div class="hunk-collapsed">
								<button type="button" class="expand-hunk-btn" onclick={() => toggleCollapse(hIndex)}>
									... {hunk.lines.length} lines — expand
								</button>
							</div>
						{:else}
							{@const htmlLines = highlightHunk(hunk, target?.path || "")}
							{@const paired = pairChangedLines(hunk.lines)}
							<!-- In unified mode we just render the paired lines in sequence -->
							<div class="hunk-lines" class:split-mode={splitView}>
								{#each paired as item, itemIdx}
									{#if splitView}
										<div class="split-row">
											{#if isLinePair(item)}
												{@const wordRes = wordDiff(item.del?.text || "", item.add?.text || "")}
												<div class="split-half del-half">
													{#if item.del}
														<div class="gutter">{item.del.oldNo ?? ""}</div>
														<div class="code hljs">
															{#each wordRes.oldSegments as seg}
																<span class:word-changed={seg.changed}>{escapeHtml(seg.text)}</span>
															{/each}
														</div>
													{:else}
														<div class="gutter"></div><div class="code"></div>
													{/if}
												</div>
												<div class="split-half add-half">
													{#if item.add}
														<div class="gutter">{item.add.newNo ?? ""}</div>
														<div class="code hljs">
															{#each wordRes.newSegments as seg}
																<span class:word-changed={seg.changed}>{escapeHtml(seg.text)}</span>
															{/each}
														</div>
													{:else}
														<div class="gutter"></div><div class="code"></div>
													{/if}
												</div>
											{:else}
												{@const lineIndex = hunk.lines.indexOf(item)}
												{@const rawHtml = htmlLines[lineIndex] || escapeHtml(item.text)}
												{#if item.kind === "context"}
													<div class="split-half context-half">
														<div class="gutter">{item.oldNo ?? ""}</div>
														<div class="code hljs">{@html rawHtml}</div>
													</div>
													<div class="split-half context-half">
														<div class="gutter">{item.newNo ?? ""}</div>
														<div class="code hljs">{@html rawHtml}</div>
													</div>
												{:else if item.kind === "add"}
													<div class="split-half empty-half">
														<div class="gutter"></div><div class="code"></div>
													</div>
													<div class="split-half add-half">
														<div class="gutter">{item.newNo ?? ""}</div>
														<div class="code hljs">{@html rawHtml}</div>
													</div>
												{:else if item.kind === "del"}
													<div class="split-half del-half">
														<div class="gutter">{item.oldNo ?? ""}</div>
														<div class="code hljs">{@html rawHtml}</div>
													</div>
													<div class="split-half empty-half">
														<div class="gutter"></div><div class="code"></div>
													</div>
												{/if}
											{/if}
										</div>
									{:else}
										<!-- Unified View -->
										{#if isLinePair(item)}
											{@const wordRes = wordDiff(item.del?.text || "", item.add?.text || "")}
											{#if item.del}
												<div class="line del-line">
													<div class="gutter">{item.del.oldNo ?? ""}</div>
													<div class="gutter">{item.del.newNo ?? ""}</div>
													<div class="code hljs">
														{#each wordRes.oldSegments as seg}
															<span class:word-changed={seg.changed}>{escapeHtml(seg.text)}</span>
														{/each}
													</div>
												</div>
											{/if}
											{#if item.add}
												<div class="line add-line">
													<div class="gutter">{item.add.oldNo ?? ""}</div>
													<div class="gutter">{item.add.newNo ?? ""}</div>
													<div class="code hljs">
														{#each wordRes.newSegments as seg}
															<span class:word-changed={seg.changed}>{escapeHtml(seg.text)}</span>
														{/each}
													</div>
												</div>
											{/if}
										{:else}
											<!-- Single unpaired line -->
											{@const lineIndex = hunk.lines.indexOf(item)}
											{@const rawHtml = htmlLines[lineIndex] || escapeHtml(item.text)}
											<div class="line {item.kind}-line">
												<div class="gutter">{item.oldNo ?? ""}</div>
												<div class="gutter">{item.newNo ?? ""}</div>
												<div class="code hljs">
													<!-- eslint-disable-next-line svelte/no-at-html-tags -->
													{@html rawHtml}
												</div>
											</div>
										{/if}
									{/if}
								{/each}
							</div>
						{/if}
					</div>
				{/each}
			{/if}
		{/if}
	</div>
</div>

<style>
	.diff-viewer {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg);
	}

	.toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border-soft);
		font-size: 12px;
	}

	.back-btn {
		background: none;
		border: none;
		color: var(--text);
		font-family: inherit;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: 4px 8px;
		border-radius: var(--radius-control);
	}

	.back-btn:hover {
		background: var(--surface);
	}

	.sep {
		color: var(--text-muted);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}

	.toggle {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--text-muted);
		cursor: pointer;
	}

	.tabs {
		display: flex;
		background: var(--surface);
		border-radius: var(--radius-control);
		padding: 2px;
		border: 1px solid var(--border);
	}

	.tabs button {
		background: none;
		border: none;
		padding: 2px 8px;
		font-size: 11px;
		color: var(--text-muted);
		border-radius: 4px;
		cursor: pointer;
	}

	.tabs button.active {
		background: var(--raised);
		color: var(--text);
		box-shadow: 0 1px 2px rgba(0,0,0,0.1);
	}

	.btn-group {
		display: flex;
		gap: 4px;
	}

	.btn-group button {
		background: var(--surface);
		border: 1px solid var(--border);
		color: var(--text);
		padding: 3px 8px;
		border-radius: var(--radius-control);
		font-size: 11px;
		cursor: pointer;
	}

	.btn-group button:hover {
		background: var(--raised);
	}

	.content {
		flex: 1;
		overflow-y: auto;
		padding-bottom: var(--space-4);
	}

	.message {
		padding: var(--space-4);
		text-align: center;
		color: var(--text-muted);
		font-size: 13px;
	}

	.error {
		color: var(--danger);
	}

	.hunk {
		margin: var(--space-3);
		border: 1px solid var(--border-soft);
		border-radius: var(--radius-control);
		overflow: hidden;
		background: var(--surface);
	}

	.hunk-header {
		display: flex;
		align-items: center;
		padding: 4px 8px;
		background: var(--overlay);
		border-bottom: 1px solid var(--border-soft);
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--text-muted);
	}

	.hunk-text {
		flex: 1;
	}

	.collapse-btn, .hunk-actions button {
		background: none;
		border: 1px solid var(--border);
		color: var(--text);
		font-size: 10px;
		padding: 2px 6px;
		border-radius: 4px;
		cursor: pointer;
		margin-left: var(--space-2);
	}

	.collapse-btn:hover, .hunk-actions button:hover {
		background: var(--raised);
	}

	.hunk-collapsed {
		padding: var(--space-3);
		text-align: center;
	}

	.expand-hunk-btn {
		background: none;
		border: none;
		color: var(--info);
		font-size: 12px;
		cursor: pointer;
	}

	.expand-hunk-btn:hover {
		text-decoration: underline;
	}

	.hunk-lines {
		display: flex;
		flex-direction: column;
		font-family: var(--font-mono);
		font-size: 12px;
	}

	.line {
		display: flex;
		min-width: fit-content;
	}

	.gutter {
		width: 40px;
		flex-shrink: 0;
		text-align: right;
		padding: 0 8px;
		color: var(--text-faint);
		user-select: none;
		border-right: 1px solid var(--border-soft);
	}

	.code {
		flex: 1;
		padding: 0 8px;
		white-space: pre;
	}

	.context-line:hover, .add-line:hover, .del-line:hover {
		background: color-mix(in srgb, var(--text) 4%, transparent);
	}

	.add-line {
		background: color-mix(in srgb, var(--ahead) 12%, transparent);
	}
	.add-line .gutter {
		color: color-mix(in srgb, var(--ahead) 60%, transparent);
	}
	.del-line {
		background: color-mix(in srgb, var(--danger) 12%, transparent);
	}
	.del-line .gutter {
		color: color-mix(in srgb, var(--danger) 60%, transparent);
	}

	.word-changed {
		background: color-mix(in srgb, var(--text) 20%, transparent);
		border-radius: 2px;
	}

	.add-line .word-changed, .add-half .word-changed {
		background: color-mix(in srgb, var(--ahead) 35%, transparent);
	}
	
	.del-line .word-changed, .del-half .word-changed {
		background: color-mix(in srgb, var(--danger) 35%, transparent);
	}

	.split-row {
		display: flex;
		width: 100%;
	}

	.split-half {
		display: flex;
		flex: 1;
		width: 50%;
		min-width: 0;
	}
	
	.split-half .gutter {
		width: 40px;
	}

	.split-half .code {
		flex: 1;
		min-width: 0;
		overflow-x: auto;
	}

	.context-half:hover, .add-half:hover, .del-half:hover {
		background: color-mix(in srgb, var(--text) 4%, transparent);
	}

	.add-half {
		background: color-mix(in srgb, var(--ahead) 12%, transparent);
	}
	.add-half .gutter {
		color: color-mix(in srgb, var(--ahead) 60%, transparent);
	}

	.del-half {
		background: color-mix(in srgb, var(--danger) 12%, transparent);
	}
	.del-half .gutter {
		color: color-mix(in srgb, var(--danger) 60%, transparent);
	}

	.empty-half {
		background: color-mix(in srgb, var(--text) 2%, transparent);
	}

	.image-diff {
		display: flex;
		gap: var(--space-4);
		justify-content: center;
	}
	
	.checkerboard {
		width: 200px;
		height: 200px;
		display: flex;
		align-items: center;
		justify-content: center;
		background-image: linear-gradient(45deg, var(--border) 25%, transparent 25%),
			linear-gradient(-45deg, var(--border) 25%, transparent 25%),
			linear-gradient(45deg, transparent 75%, var(--border) 75%),
			linear-gradient(-45deg, transparent 75%, var(--border) 75%);
		background-size: 20px 20px;
		background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		color: var(--text-muted);
	}
</style>
