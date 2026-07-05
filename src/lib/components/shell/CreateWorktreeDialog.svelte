<script lang="ts">
	import { pickFolder } from "$lib/ipc";
	import { repos } from "$lib/stores/repo.svelte";
	import { graph } from "$lib/stores/graph.svelte";
	import * as actions from "$lib/actions";

	/** Create-worktree dialog — DESIGN_SPEC.md §5 WORKTREES. Path picker defaults to a sibling of
	 * the repo root, suggested from the start ref; branch selector lists local branches, with a
	 * free-text field doubling as "create a new branch here" when its name doesn't match one. */
	let { startRef, onDismiss }: { startRef: string; onDismiss: () => void } = $props();

	const repoId = $derived(graph.repoId);
	const repoRoot = $derived(repos.tabs.find((t) => t.id === repoId)?.path ?? null);
	const repoName = $derived(repoRoot ? (repoRoot.split(/[\\/]/).filter(Boolean).at(-1) ?? "repo") : "repo");
	const localBranches = $derived(graph.refs.filter((r) => r.kind === "branch").map((r) => r.shortName));

	function siblingSuggestion(root: string, ref: string): string {
		const parent = root.replace(/[\\/][^\\/]+[\\/]?$/, "");
		const safeRef = ref.replace(/[^a-zA-Z0-9._-]+/g, "-");
		return `${parent}/${repoName}-${safeRef}`;
	}

	let path = $state("");
	let selectedRef = $state("");
	let createBranch = $state(false);
	let newBranchName = $state("");
	let submitting = $state(false);
	let initialized = false;

	// One-time defaults from props/store, applied as soon as `repoRoot` resolves — guarded so a
	// later unrelated repo-root/ref change doesn't clobber what the user already typed.
	$effect(() => {
		if (initialized || !repoRoot) return;
		initialized = true;
		selectedRef = startRef;
		path = siblingSuggestion(repoRoot, startRef);
	});

	async function choosePath() {
		const dir = await pickFolder("Create worktree in…");
		if (dir) path = `${dir}/${repoName}-${selectedRef.replace(/[^a-zA-Z0-9._-]+/g, "-")}`;
	}

	async function submit() {
		const id = repoId;
		if (!id || !path.trim() || !selectedRef.trim() || submitting) return;
		submitting = true;
		const ok = await actions.createWorktree(
			id,
			path.trim(),
			selectedRef.trim(),
			createBranch && newBranchName.trim() ? newBranchName.trim() : null,
		);
		submitting = false;
		if (ok) onDismiss();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Escape") {
			e.preventDefault();
			onDismiss();
		}
	}
</script>

<div class="scrim" onkeydown={handleKeydown} role="presentation">
	<div class="dialog" role="dialog" aria-modal="true" aria-label="Create a worktree">
		<h2>Create a worktree</h2>

		<label class="field">
			<span>Start from</span>
			<input list="worktree-branches" bind:value={selectedRef} type="text" placeholder="branch or commit" />
			<datalist id="worktree-branches">
				{#each localBranches as b (b)}<option value={b}></option>{/each}
			</datalist>
		</label>

		<label class="checkbox">
			<input type="checkbox" bind:checked={createBranch} />
			Create a new branch here
		</label>
		{#if createBranch}
			<label class="field">
				<span>New branch name</span>
				<input bind:value={newBranchName} type="text" placeholder="feature/x" />
			</label>
		{/if}

		<label class="field">
			<span>Path</span>
			<div class="row">
				<button type="button" class="secondary" onclick={choosePath}>Choose Folder…</button>
				<input bind:value={path} type="text" placeholder="/path/to/worktree" />
			</div>
		</label>

		<div class="actions">
			<button type="button" class="secondary" onclick={onDismiss}>Cancel</button>
			<button
				type="button"
				class="primary"
				disabled={!path.trim() || !selectedRef.trim() || submitting}
				onclick={submit}
			>
				Create worktree
			</button>
		</div>
	</div>
</div>

<style>
	.scrim {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 40%);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 100;
	}

	.dialog {
		width: min(480px, 90vw);
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 16px 48px rgb(0 0 0 / 35%);
		padding: var(--space-5);
	}

	h2 {
		font-size: 15px;
		font-weight: 600;
		color: var(--text);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: 12px;
		color: var(--text-muted);
	}

	.field input {
		font: inherit;
		font-size: 13px;
		padding: var(--space-2) var(--space-3);
		background: var(--raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		color: var(--text);
	}

	.field input:focus {
		outline: 2px solid var(--accent);
		outline-offset: -1px;
	}

	.checkbox {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: 12px;
		color: var(--text-muted);
		cursor: pointer;
	}

	.row {
		display: flex;
		gap: var(--space-2);
	}

	.row input {
		flex: 1;
		min-width: 0;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	button {
		font: inherit;
		font-size: 13px;
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-control);
		border: 1px solid transparent;
		cursor: pointer;
		transition: background var(--motion-hover);
	}

	button.secondary {
		background: var(--raised);
		color: var(--text);
		border-color: var(--border);
	}

	button.secondary:hover {
		background: var(--overlay);
	}

	button.primary {
		background: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}

	button.primary:hover {
		background: var(--accent-dim);
	}

	button.primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
