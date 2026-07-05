<script lang="ts">
	import { graph } from "$lib/stores/graph.svelte";
	import { graphSelection } from "$lib/stores/graphSelection.svelte";
	import { graphNav } from "$lib/stores/graphNav.svelte";
	import { diffView } from "$lib/stores/diffView.svelte";
	import { repos } from "$lib/stores/repo.svelte";
	import * as ipc from "$lib/ipc";
	import type { ChangedFile, CommitMeta } from "$lib/types";
	import FileRow from "./FileRow.svelte";
	import { relativeTime } from "$lib/format";

	/** Commit-detail mode — DESIGN_SPEC.md §6.1. Metadata card + full message + changed-file list,
	 * reusing the same row component as the working-directory sections. */
	let { sha }: { sha: string } = $props();

	const repoId = $derived(graph.repoId);
	const repoRoot = $derived(repos.tabs.find((t) => t.id === repoId)?.path ?? null);

	let meta = $state<CommitMeta | null>(null);
	let files = $state<ChangedFile[]>([]);
	let loading = $state(false);

	$effect(() => {
		const currentSha = sha;
		const cached = graph.metaBySha[currentSha];
		if (cached) {
			meta = cached;
		} else if (repoId) {
			void ipc.getCommitMeta(repoId, [currentSha]).then((metas) => {
				if (sha === currentSha) meta = metas[0] ?? null;
			});
		}
	});

	$effect(() => {
		const currentSha = sha;
		const id = repoId;
		if (!id) return;
		loading = true;
		files = [];
		void ipc
			.getCommitFiles(id, currentSha)
			.then((result) => {
				if (sha === currentSha) files = result;
			})
			.finally(() => {
				if (sha === currentSha) loading = false;
			});
	});

	function openParent(parentSha: string) {
		graphSelection.select(parentSha);
		graphNav.scrollTo(parentSha);
	}

	function openFile(file: ChangedFile) {
		diffView.open({
			path: file.path,
			origPath: file.origPath,
			source: { kind: "commit", sha },
		});
	}

	async function copySha() {
		try {
			await navigator.clipboard?.writeText(sha);
		} catch {
			/* clipboard unavailable — best effort */
		}
	}
</script>

<div class="panel">
	{#if meta}
		<div class="card">
			<div class="row">
				<span class="label">Author</span>
				<span class="value">{meta.authorName} <span class="muted">&lt;{meta.authorEmail}&gt;</span></span>
			</div>
			<div class="row">
				<span class="label">Date</span>
				<span class="value" title={new Date(meta.authorTime * 1000).toLocaleString()}>
					{relativeTime(meta.authorTime)} · {new Date(meta.authorTime * 1000).toLocaleString()}
				</span>
			</div>
			<div class="row">
				<span class="label">SHA</span>
				<button type="button" class="sha" onclick={copySha} title="Copy full SHA">
					{sha.slice(0, 10)} ⧉
				</button>
			</div>
			{#if meta.parents.length > 0}
				<div class="row">
					<span class="label">Parents</span>
					<span class="value parents">
						{#each meta.parents as parentSha, i (parentSha)}
							{#if i > 0}<span class="sep">·</span>{/if}
							<button type="button" class="parent-link" onclick={() => openParent(parentSha)}>
								{parentSha.slice(0, 7)}
							</button>
						{/each}
					</span>
				</div>
			{/if}
		</div>

		<div class="message selectable">
			<p class="subject">{meta.subject}</p>
			{#if meta.body}<pre class="body">{meta.body}</pre>{/if}
		</div>
	{:else}
		<p class="empty">Loading commit…</p>
	{/if}

	<div class="files">
		<div class="section-head">
			CHANGED FILES <span class="count">{files.length}</span>
		</div>
		{#if loading}
			<p class="empty">Loading…</p>
		{:else if files.length === 0}
			<p class="empty">No file changes</p>
		{:else}
			{#each files as file (file.path)}
				<FileRow
					path={file.path}
					origPath={file.origPath}
					status={file.status}
					{repoRoot}
					onClick={() => openFile(file)}
				/>
			{/each}
		{/if}
	</div>
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow-y: auto;
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: var(--space-3);
		border-bottom: 1px solid var(--border-soft);
		font-size: 12px;
	}

	.row {
		display: flex;
		gap: var(--space-2);
	}

	.label {
		flex-shrink: 0;
		width: 56px;
		color: var(--text-faint);
	}

	.value {
		color: var(--text);
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.muted {
		color: var(--text-faint);
	}

	.sha {
		border: none;
		background: none;
		color: var(--text);
		font-family: var(--font-mono);
		font-size: 12px;
		cursor: pointer;
		padding: 0;
	}

	.sha:hover {
		color: var(--accent);
	}

	.parents {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
	}

	.parent-link {
		border: none;
		background: none;
		color: var(--info);
		font-family: var(--font-mono);
		font-size: 12px;
		cursor: pointer;
		padding: 0;
	}

	.parent-link:hover {
		text-decoration: underline;
	}

	.sep {
		color: var(--text-faint);
	}

	.message {
		padding: var(--space-3);
		border-bottom: 1px solid var(--border-soft);
	}

	.subject {
		margin: 0 0 var(--space-2);
		font-size: 13px;
		font-weight: 600;
		color: var(--text);
	}

	.body {
		margin: 0;
		font-family: var(--font-ui);
		font-size: 12px;
		color: var(--text-muted);
		white-space: pre-wrap;
	}

	.files {
		flex: 1;
	}

	.section-head {
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

	.empty {
		margin: 0;
		padding: 2px var(--space-4) var(--space-3);
		font-size: 11px;
		color: var(--text-faint);
	}
</style>
