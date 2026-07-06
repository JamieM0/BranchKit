<script lang="ts">
	import ContextMenu, { type MenuItem } from "$lib/components/shell/ContextMenu.svelte";
	import * as actions from "$lib/actions";
	import { worktreeDialog } from "$lib/stores/worktreeDialog.svelte";

	/** Commit row right-click menu — GITKRAKEN_WORKFLOWS.md §3.1, DESIGN_SPEC.md §15.30. Everything
	 * that doesn't need a follow-up prompt runs straight off the menu; Reset Hard gets the
	 * arm-delayed confirm (§4.6), and Create tag gets a tiny inline name/message form — both shown
	 * in place of the menu rather than a modal. */
	let {
		sha,
		repoId,
		currentBranch,
		x,
		y,
		onDismiss,
		onCreateBranch,
		onCompareWorking,
	}: {
		sha: string;
		repoId: string;
		currentBranch: string | null;
		x: number;
		y: number;
		onDismiss: () => void;
		onCreateBranch: (sha: string) => void;
		onCompareWorking: (sha: string) => void;
	} = $props();

	let mode = $state<"menu" | "confirmResetHard" | "tag" | "annotatedTag">("menu");
	let resetArmed = $state(false);
	let armTimer: ReturnType<typeof setTimeout> | undefined;
	let tagName = $state("");
	let tagMessage = $state("");

	function close() {
		clearTimeout(armTimer);
		onDismiss();
	}

	function run(fn: () => void | Promise<void>) {
		close();
		void fn();
	}

	function startResetHard() {
		mode = "confirmResetHard";
		resetArmed = false;
		clearTimeout(armTimer);
		armTimer = setTimeout(() => (resetArmed = true), 400);
	}

	async function confirmResetHard() {
		if (!resetArmed || !currentBranch) return;
		const target = sha;
		close();
		await actions.resetTo(repoId, target, "hard");
	}

	function startTag(annotated: boolean) {
		tagName = "";
		tagMessage = "";
		mode = annotated ? "annotatedTag" : "tag";
	}

	async function submitTag() {
		const name = tagName.trim();
		if (!name) return;
		const message = mode === "annotatedTag" ? tagMessage.trim() || null : null;
		const target = sha;
		close();
		await actions.createTag(repoId, name, target, message);
	}

	// Daily-driver actions stay top-level; history-rewriting and power tools live in one
	// "Advanced" flyout so the menu reads at a glance (Jamie's request — the old flat menu had
	// 13 entries and 5 separators).
	const items: MenuItem[] = $derived([
		{ type: "action", label: "Checkout this commit", run: () => run(() => actions.checkoutDetached(repoId, sha)) },
		{ type: "action", label: "Create branch here…", run: () => run(() => onCreateBranch(sha)) },
		{ type: "separator" },
		...(currentBranch
			? ([
					{
						type: "submenu",
						label: `Reset ${currentBranch} to this commit`,
						items: [
							{ type: "action", label: "Soft (keep changes staged)", run: () => run(() => actions.resetTo(repoId, sha, "soft")) },
							{ type: "action", label: "Mixed (keep changes unstaged)", run: () => run(() => actions.resetTo(repoId, sha, "mixed")) },
							{ type: "action", label: "Hard (discard changes)…", danger: true, keepOpen: true, run: startResetHard },
						],
					},
				] satisfies MenuItem[])
			: []),
		{ type: "action", label: "Copy commit SHA", shortcut: "", run: () => run(() => actions.copyToClipboard(sha, "Copied SHA")) },
		{ type: "separator" },
		{
			type: "action",
			label: "Create tag here…",
			keepOpen: true,
			run: () => {
				mode = "tag";
			},
		},
		{
			type: "action",
			label: "Create annotated tag here…",
			keepOpen: true,
			run: () => {
				mode = "annotatedTag";
			},
		},
		{ type: "separator" },
		{
			type: "submenu",
			label: "Advanced",
			items: [
				{ type: "action", label: "Cherry-pick commit", run: () => run(() => actions.cherryPick(repoId, sha)) },
				...(currentBranch
					? ([
							{
								type: "action",
								label: `Rebase ${currentBranch} onto this commit`,
								run: () => run(() => actions.rebaseOnto(repoId, currentBranch, sha)),
							},
						] satisfies MenuItem[])
					: []),
				{ type: "action", label: "Revert commit", run: () => run(() => actions.revertCommit(repoId, sha)) },
				{ type: "separator" },
				{
					type: "action",
					label: "Copy link to this commit on remote: origin",
					run: () => run(() => actions.copyCommitLink(repoId, "origin", sha)),
				},
				{ type: "action", label: "Create patch from commit", run: () => run(() => actions.createPatchFromCommit(repoId, sha)) },
				{ type: "action", label: "Compare commit against working directory", run: () => run(() => onCompareWorking(sha)) },
				{ type: "action", label: "Create worktree from this commit…", run: () => run(() => worktreeDialog.open(sha)) },
			],
		},
	]);
</script>

{#if mode === "menu"}
	<ContextMenu {items} {x} {y} onDismiss={close} ariaLabel="Commit actions" />
{:else if mode === "confirmResetHard"}
	<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
	<div class="scrim" onclick={close}></div>
	<div class="panel" style="left: {x}px; top: {y}px;">
		<p class="text">
			Reset <code>{currentBranch}</code> to <code>{sha.slice(0, 7)}</code>? Uncommitted changes and
			commits after this point that aren't merged elsewhere will be discarded from the branch tip.
		</p>
		<div class="actions">
			<button type="button" onclick={close}>Cancel</button>
			<button type="button" class="danger-solid" disabled={!resetArmed} onclick={confirmResetHard}>
				{resetArmed ? "Reset (hard)" : "Hold…"}
			</button>
		</div>
	</div>
{:else}
	<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
	<div class="scrim" onclick={close}></div>
	<div class="panel" style="left: {x}px; top: {y}px;">
		<label class="field">
			Tag name
			<input type="text" bind:value={tagName} placeholder="v1.0.0" autofocus />
		</label>
		{#if mode === "annotatedTag"}
			<label class="field">
				Message
				<input type="text" bind:value={tagMessage} placeholder="Release notes…" />
			</label>
		{/if}
		<div class="actions">
			<button type="button" onclick={close}>Cancel</button>
			<button type="button" class="primary" disabled={!tagName.trim()} onclick={submitTag}>Create tag</button>
		</div>
	</div>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 90;
	}

	.panel {
		position: fixed;
		z-index: 91;
		width: 260px;
		padding: var(--space-2);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.text {
		margin: 0;
		font-size: 12px;
		color: var(--text);
	}

	.text code {
		font-family: var(--font-mono);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 3px;
		font-size: 11px;
		color: var(--text-muted);
	}

	.field input {
		padding: 4px var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 12px;
	}

	.field input:focus {
		outline: none;
		border-color: var(--accent);
	}

	.actions {
		display: flex;
		gap: var(--space-1);
	}

	.actions button {
		flex: 1;
		justify-content: center;
		padding: var(--space-2);
		border-radius: var(--radius-control);
		border: 1px solid var(--border);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 12px;
		cursor: pointer;
	}

	.actions .primary {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}

	.actions .primary:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.actions .danger-solid {
		background: var(--danger);
		border-color: var(--danger);
		color: #fff;
		font-weight: 600;
	}

	.actions .danger-solid:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
