<script lang="ts">
	import { createPrDraft } from "$lib/stores/createPrDraft.svelte";
	import { prPanel } from "$lib/stores/prPanel.svelte";
	import { graph } from "$lib/stores/graph.svelte";
	import * as actions from "$lib/actions";

	/** The Create-PR panel — DESIGN_SPEC.md §12: base/head pickers (prefilled), title/body
	 * (prefilled), [Create] → toast with PR number + open action. Never auto-opens the browser. */
	let { repoId }: { repoId: string } = $props();

	const branchNames = $derived(
		graph.refs.filter((r) => r.kind === "branch" && !r.shortName.includes("/")).map((r) => r.shortName),
	);

	$effect(() => {
		if (!createPrDraft.head) {
			const current = graph.head && !graph.head.detached ? graph.head.branch : null;
			if (current) createPrDraft.head = current;
		}
	});

	let creating = $state(false);

	async function submit() {
		if (!createPrDraft.head.trim() || !createPrDraft.base.trim() || !createPrDraft.title.trim()) return;
		creating = true;
		const number = await actions.createPullRequest(
			repoId,
			createPrDraft.base.trim(),
			createPrDraft.head.trim(),
			createPrDraft.title.trim(),
			createPrDraft.body,
		);
		creating = false;
		if (number !== null) {
			createPrDraft.reset();
			prPanel.close();
		}
	}
</script>

<div class="panel">
	<div class="head">
		<button type="button" class="back" onclick={() => prPanel.close()} aria-label="Close">← Back</button>
	</div>

	<div class="body">
		<h2>Create pull request</h2>

		<label class="field">
			<span>Base</span>
			<select bind:value={createPrDraft.base}>
				{#each branchNames as name (name)}
					<option value={name}>{name}</option>
				{/each}
			</select>
		</label>

		<label class="field">
			<span>Compare (head)</span>
			<select bind:value={createPrDraft.head}>
				{#each branchNames as name (name)}
					<option value={name}>{name}</option>
				{/each}
			</select>
		</label>

		<label class="field">
			<span>Title</span>
			<input type="text" bind:value={createPrDraft.title} />
		</label>

		<label class="field">
			<span>Description</span>
			<textarea bind:value={createPrDraft.body} rows="8"></textarea>
		</label>
	</div>

	<div class="actions">
		<button
			type="button"
			class="primary"
			disabled={creating || !createPrDraft.head.trim() || !createPrDraft.base.trim() || !createPrDraft.title.trim() || createPrDraft.head === createPrDraft.base}
			onclick={submit}
		>
			{creating ? "Creating…" : "Create"}
		</button>
	</div>
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.head {
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border-soft);
	}

	.back {
		border: none;
		background: none;
		color: var(--text-muted);
		font: inherit;
		font-size: 12px;
		cursor: pointer;
		padding: 0;
	}

	.back:hover {
		color: var(--text);
	}

	.body {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-3);
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}

	h2 {
		margin: 0;
		font-size: 14px;
		font-weight: 600;
		color: var(--text);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 2px;
		font-size: 12px;
		color: var(--text-muted);
	}

	.field input,
	.field select,
	.field textarea {
		font: inherit;
		font-size: 13px;
		padding: var(--space-2) var(--space-3);
		background: var(--raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		color: var(--text);
		resize: vertical;
	}

	.actions {
		padding: var(--space-3);
		border-top: 1px solid var(--border-soft);
	}

	button {
		width: 100%;
		font: inherit;
		font-size: 13px;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-control);
		border: 1px solid transparent;
		cursor: pointer;
	}

	button.primary {
		background: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}

	button.primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
