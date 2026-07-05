<script lang="ts">
	/** Conflict-state banner stub — DESIGN_SPEC.md §9.1. Just enough UI to prove `conflict.rs`'s
	 * state detection + continue/abort commands work end to end; the real Keep Panel (progress
	 * dots, exact-consequence abort confirm, per-file tabs) is a later prompt. */
	import { onDestroy } from "svelte";
	import { abortConflict, continueConflict, getConflictState, onRepoChanged } from "$lib/ipc";
	import type { ConflictState } from "$lib/types";

	let { repoId }: { repoId: string } = $props();

	let state: ConflictState | null = $state(null);
	let unlisten: (() => void) | null = null;

	async function refresh() {
		state = await getConflictState(repoId);
	}

	$effect(() => {
		void refresh();
		let cancelled = false;
		onRepoChanged(repoId, (kind) => {
			if (cancelled) return;
			if (kind.kind === "workingTree" || kind.kind === "index" || kind.kind === "head") {
				void refresh();
			}
		}).then((fn) => {
			if (cancelled) fn();
			else unlisten = fn;
		});
		return () => {
			cancelled = true;
			unlisten?.();
			unlisten = null;
		};
	});

	onDestroy(() => unlisten?.());

	const verb: Record<ConflictState["kind"], string> = {
		merge: "Merging",
		rebase: "Rebasing",
		cherryPick: "Cherry-picking",
		revert: "Reverting",
		stashApply: "Applying stash",
	};

	async function handleContinue() {
		await continueConflict(repoId);
		await refresh();
	}

	async function handleAbort() {
		await abortConflict(repoId);
		await refresh();
	}
</script>

{#if state}
	<div class="banner" role="status">
		<span class="text">
			{verb[state.kind]} <strong>{state.sourceLabel}</strong> into <strong>{state.targetLabel}</strong>
			— {state.files.length} conflicted {state.files.length === 1 ? "file" : "files"}
		</span>
		<div class="actions">
			<button type="button" onclick={handleContinue}>Continue</button>
			<button type="button" class="abort" onclick={handleAbort}>Abort…</button>
		</div>
	</div>
{/if}

<style>
	.banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		font-size: 12px;
		color: var(--text);
		background: color-mix(in srgb, var(--warn) 12%, var(--surface));
		border-bottom: 1px solid var(--border);
	}

	.text strong {
		font-weight: 600;
	}

	.actions {
		display: flex;
		gap: var(--space-2);
		flex-shrink: 0;
	}

	button {
		font-size: 12px;
		padding: 4px 10px;
		border-radius: 6px;
		border: 1px solid var(--border);
		background: var(--raised);
		color: var(--text);
		cursor: pointer;
	}

	button:hover {
		background: var(--overlay);
	}

	button.abort {
		color: var(--danger);
	}
</style>
