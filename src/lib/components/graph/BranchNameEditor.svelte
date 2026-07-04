<script lang="ts">
	import { branchEdit } from "$lib/stores/branchEdit.svelte";
	import * as actions from "$lib/actions";

	/** Inline branch-name input rendered in the BRANCH/TAG column — DESIGN_SPEC.md §5. Enter commits
	 * (create checks out immediately; rename applies), Esc / blur cancels. Git rejects invalid names
	 * itself, surfaced as an error toast by the actions layer. */
	let { repoId }: { repoId: string } = $props();

	let value = $state(branchEdit.initial);
	let el = $state<HTMLInputElement | null>(null);
	let submitting = $state(false);

	$effect(() => {
		el?.focus();
		el?.select();
	});

	async function submit() {
		const name = value.trim();
		if (submitting) return;
		if (name === "" || (branchEdit.mode === "rename" && name === branchEdit.oldName)) {
			branchEdit.cancel();
			return;
		}
		submitting = true;
		let ok = false;
		if (branchEdit.mode === "create") {
			ok = await actions.createBranch(repoId, name, branchEdit.sha, true);
		} else if (branchEdit.mode === "rename" && branchEdit.oldName) {
			ok = await actions.renameBranch(repoId, branchEdit.oldName, name);
		}
		submitting = false;
		// On failure leave the editor open so the user can fix the name.
		if (ok) branchEdit.cancel();
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === "Enter") {
			e.preventDefault();
			void submit();
		} else if (e.key === "Escape") {
			e.preventDefault();
			branchEdit.cancel();
		}
		e.stopPropagation();
	}
</script>

<input
	bind:this={el}
	class="editor"
	type="text"
	spellcheck="false"
	autocomplete="off"
	placeholder={branchEdit.mode === "create" ? "branch name" : "new name"}
	bind:value
	onkeydown={onKeydown}
	onblur={() => branchEdit.cancel()}
	onclick={(e) => e.stopPropagation()}
	ondblclick={(e) => e.stopPropagation()}
/>

<style>
	.editor {
		width: 130px;
		max-width: 100%;
		padding: 1px 6px;
		border: 1px solid var(--accent);
		border-radius: var(--radius-pill);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 11px;
		line-height: 16px;
		outline: none;
	}
</style>
