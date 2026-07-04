<script lang="ts">
	import { status } from "$lib/stores/status.svelte";
	import { graph } from "$lib/stores/graph.svelte";
	import { settings } from "$lib/stores/settings.svelte";
	import { diffView } from "$lib/stores/diffView.svelte";
	import { toasts } from "$lib/stores/toasts.svelte";
	import * as ipc from "$lib/ipc";
	import { stagedRows, unstagedRows, type FileRow as FileRowModel } from "$lib/status/sections";
	import { buildFileTree } from "$lib/status/tree";
	import FileRow from "./FileRow.svelte";
	import FileTree from "./FileTree.svelte";

	/** Working-directory mode — DESIGN_SPEC.md §6.1. Default right-panel mode: no commit selected
	 * in the graph (a plain click on a commit switches to commit-detail mode instead, handled by
	 * the parent RightPanel). */

	const repoId = $derived(graph.repoId);
	const branch = $derived(graph.head?.detached ? graph.head.sha.slice(0, 7) : (graph.head?.branch ?? "…"));

	const unstagedSorted = $derived(
		unstagedRows(status.report.entries).sort((a, b) => a.path.localeCompare(b.path)),
	);
	const stagedSorted = $derived(
		stagedRows(status.report.entries).sort((a, b) => a.path.localeCompare(b.path)),
	);

	const totalChanged = $derived(
		new Set([...unstagedSorted.map((r) => r.path), ...stagedSorted.map((r) => r.path)]).size,
	);

	type Section = "unstaged" | "staged";
	let selectedPath = $state<string | null>(null);
	let selectedSection = $state<Section | null>(null);

	function asAppError(e: unknown): { userMessage: string; raw: string } {
		if (e && typeof e === "object" && "userMessage" in e) {
			const o = e as Record<string, unknown>;
			return { userMessage: String(o.userMessage), raw: String(o.raw ?? "") };
		}
		return { userMessage: e instanceof Error ? e.message : String(e), raw: String(e) };
	}

	function selectRow(path: string, section: Section) {
		selectedPath = path;
		selectedSection = section;
	}

	function openDiff(row: FileRowModel, section: Section) {
		selectRow(row.path, section);
		diffView.open({
			path: row.path,
			origPath: row.origPath,
			source: section === "staged" ? { kind: "staged" } : { kind: "workingTree" },
		});
	}

	/** Stage one file; auto-advances the selection to the next unstaged row (§15.10) *before*
	 * awaiting the round trip, since the row will disappear from `unstagedSorted` once the status
	 * refresh lands. */
	async function stage(path: string) {
		if (!repoId) return;
		const idx = unstagedSorted.findIndex((r) => r.path === path);
		const next = unstagedSorted[idx + 1] ?? unstagedSorted[idx - 1] ?? null;
		if (next) {
			selectRow(next.path, "unstaged");
		} else if (stagedSorted[0]) {
			selectRow(stagedSorted[0].path, "staged");
		} else {
			selectedPath = null;
			selectedSection = null;
		}
		try {
			await ipc.stageFile(repoId, path);
		} catch (e) {
			const { userMessage, raw } = asAppError(e);
			toasts.pushError(userMessage, raw);
		}
	}

	async function unstage(path: string) {
		if (!repoId) return;
		const idx = stagedSorted.findIndex((r) => r.path === path);
		const next = stagedSorted[idx + 1] ?? stagedSorted[idx - 1] ?? null;
		if (next) selectRow(next.path, "staged");
		else if (unstagedSorted[0]) selectRow(unstagedSorted[0].path, "unstaged");
		else {
			selectedPath = null;
			selectedSection = null;
		}
		try {
			await ipc.unstageFile(repoId, path);
		} catch (e) {
			const { userMessage, raw } = asAppError(e);
			toasts.pushError(userMessage, raw);
		}
	}

	async function stageAll() {
		if (!repoId) return;
		try {
			await ipc.stageAll(repoId);
		} catch (e) {
			const { userMessage, raw } = asAppError(e);
			toasts.pushError(userMessage, raw);
		}
	}

	async function unstageAll() {
		if (!repoId) return;
		try {
			await ipc.unstageAll(repoId);
		} catch (e) {
			const { userMessage, raw } = asAppError(e);
			toasts.pushError(userMessage, raw);
		}
	}

	function actionFor(section: Section): "Stage" | "Unstage" {
		return section === "unstaged" ? "Stage" : "Unstage";
	}

	/** Space stages/unstages the keyboard-selected row (§15.10); ArrowUp/Down move the selection
	 * within its own section. */
	function onKeydown(e: KeyboardEvent) {
		if (e.key === " " || e.key === "Spacebar") {
			if (!selectedPath || !selectedSection) return;
			e.preventDefault();
			if (selectedSection === "unstaged") void stage(selectedPath);
			else void unstage(selectedPath);
			return;
		}
		if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
		e.preventDefault();
		const list = selectedSection === "staged" ? stagedSorted : unstagedSorted;
		if (list.length === 0) return;
		const idx = selectedPath ? list.findIndex((r) => r.path === selectedPath) : -1;
		const delta = e.key === "ArrowDown" ? 1 : -1;
		const nextIdx = Math.max(0, Math.min(list.length - 1, idx === -1 ? 0 : idx + delta));
		selectRow(list[nextIdx].path, selectedSection === "staged" ? "staged" : "unstaged");
	}
</script>

<div class="panel" tabindex="0" role="listbox" aria-label="Working directory changes" onkeydown={onKeydown}>
	<div class="header">
		<div class="title">
			<strong>{totalChanged}</strong> changes on <code>{branch}</code>
		</div>
		<div class="header-actions">
			<button type="button" class="discard-all" disabled title="Discard All lands with the discard safety net (ARCHITECTURE §7.3)">
				🗑
			</button>
			<div class="view-toggle" role="group" aria-label="File list view">
				<button
					type="button"
					class:active={settings.fileListView === "path"}
					onclick={() => settings.setFileListView("path")}
				>
					Path
				</button>
				<button
					type="button"
					class:active={settings.fileListView === "tree"}
					onclick={() => settings.setFileListView("tree")}
				>
					Tree
				</button>
			</div>
		</div>
	</div>

	<div class="sections">
		<section>
			<div class="section-head">
				<span>UNSTAGED <span class="count">{unstagedSorted.length}</span></span>
				{#if unstagedSorted.length > 0}
					<button type="button" class="bulk" onclick={stageAll}>Stage All</button>
				{/if}
			</div>
			{#if unstagedSorted.length === 0}
				<p class="empty">No unstaged changes</p>
			{:else if settings.fileListView === "tree"}
				<FileTree
					nodes={buildFileTree(unstagedSorted)}
					selectedPath={selectedSection === "unstaged" ? selectedPath : null}
					actionFor={() => "Stage"}
					onFileClick={(p) => {
						const row = unstagedSorted.find((r) => r.path === p);
						if (row) openDiff(row, "unstaged");
					}}
					onFileAction={(p) => void stage(p)}
				/>
			{:else}
				{#each unstagedSorted as row (row.path)}
					<FileRow
						path={row.path}
						origPath={row.origPath}
						status={row.status}
						partial={row.partial}
						selected={selectedSection === "unstaged" && selectedPath === row.path}
						actionLabel="Stage"
						onClick={() => openDiff(row, "unstaged")}
						onAction={() => void stage(row.path)}
					/>
				{/each}
			{/if}
		</section>

		<section>
			<div class="section-head">
				<span>STAGED <span class="count">{stagedSorted.length}</span></span>
				{#if stagedSorted.length > 0}
					<button type="button" class="bulk" onclick={unstageAll}>Unstage All</button>
				{/if}
			</div>
			{#if stagedSorted.length === 0}
				<p class="empty">No staged changes</p>
			{:else if settings.fileListView === "tree"}
				<FileTree
					nodes={buildFileTree(stagedSorted)}
					selectedPath={selectedSection === "staged" ? selectedPath : null}
					actionFor={() => "Unstage"}
					onFileClick={(p) => {
						const row = stagedSorted.find((r) => r.path === p);
						if (row) openDiff(row, "staged");
					}}
					onFileAction={(p) => void unstage(p)}
				/>
			{:else}
				{#each stagedSorted as row (row.path)}
					<FileRow
						path={row.path}
						origPath={row.origPath}
						status={row.status}
						partial={row.partial}
						selected={selectedSection === "staged" && selectedPath === row.path}
						actionLabel="Unstage"
						onClick={() => openDiff(row, "staged")}
						onAction={() => void unstage(row.path)}
					/>
				{/each}
			{/if}
		</section>
	</div>
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		outline: none;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		padding: var(--space-3);
		border-bottom: 1px solid var(--border-soft);
	}

	.title {
		font-size: 12px;
		color: var(--text-muted);
	}

	.title code {
		font-family: var(--font-mono);
		color: var(--text);
	}

	.header-actions {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex-shrink: 0;
	}

	.discard-all {
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text-faint);
		font-size: 11px;
		padding: 2px 6px;
		cursor: not-allowed;
	}

	.view-toggle {
		display: flex;
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		overflow: hidden;
	}

	.view-toggle button {
		border: none;
		background: var(--raised);
		color: var(--text-muted);
		font: inherit;
		font-size: 11px;
		padding: 2px 8px;
		cursor: pointer;
	}

	.view-toggle button.active {
		background: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}

	.sections {
		flex: 1;
		overflow-y: auto;
	}

	.section-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-2) var(--space-3);
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.04em;
		color: var(--text-muted);
	}

	.count {
		color: var(--text-faint);
		font-weight: 600;
	}

	.bulk {
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: 10px;
		padding: 1px var(--space-2);
		cursor: pointer;
	}

	.bulk:hover {
		background: var(--overlay);
	}

	.empty {
		margin: 0;
		padding: 2px var(--space-4) var(--space-3);
		font-size: 11px;
		color: var(--text-faint);
	}
</style>
