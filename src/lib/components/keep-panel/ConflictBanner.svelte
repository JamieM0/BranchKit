<script lang="ts">
	/** Conflict banner — DESIGN_SPEC.md §9.1/§9.2/§9.3, §15.20/§15.21. Persistent strip under the
	 * toolbar for the whole in-progress operation: exact progress phrasing, a Continue that is
	 * disabled (with a tooltip that says precisely what's left) until every file is confirmed, an
	 * inline editable merge-message field (never a modal), a ⋯ menu of global bulk keeps, and an
	 * Abort that confirms with the exact consequence. Reads everything from the shared
	 * [`keepSession`] so it stays in lock-step with the Keep Panel and the Conflicted-files list. */
	import { keepSession, CONFLICT_VERB, CONTINUE_VERB, ABORT_VERB } from "$lib/stores/keepSession.svelte";
	import { graph } from "$lib/stores/graph.svelte";
	import { toasts } from "$lib/stores/toasts.svelte";

	// repoId is what the shell keys the banner on; the session is opened for it by the page shell.
	let { repoId }: { repoId: string } = $props();

	const conflict = $derived(keepSession.repoId === repoId ? keepSession.state : null);
	const progress = $derived(keepSession.progress);
	const kind = $derived(conflict?.kind ?? "merge");

	/** The commit the target ref returns to on abort — for a conflicted merge/cherry-pick/revert
	 * HEAD hasn't moved, so the live HEAD sha is exactly it (DESIGN_SPEC.md §9.3's `e4f5a6`). */
	const targetSha = $derived(graph.head?.sha ? graph.head.sha.slice(0, 7) : null);

	/** Files with any resolution that an abort would throw away — "in 2 files" (§9.3). */
	const touchedFiles = $derived(
		keepSession.allFiles.filter(
			(f) => keepSession.isConfirmed(f) || (keepSession.entryFor(f)?.store.fileProgress.resolved ?? 0) > 0,
		).length,
	);

	let showMenu = $state(false);
	let confirmingAbort = $state(false);
	let editingMessage = $state(false);
	let mergeMessage = $state("");
	let busy = $state(false);

	function asError(e: unknown): { userMessage: string; raw: string } {
		if (e && typeof e === "object" && "userMessage" in e) {
			const o = e as Record<string, unknown>;
			return { userMessage: String(o.userMessage), raw: String(o.raw ?? "") };
		}
		return { userMessage: e instanceof Error ? e.message : String(e), raw: String(e) };
	}

	/** Continue: for a merge, first reveal the inline (prefilled, editable) message field; the
	 * second press commits. Other kinds reuse their own stored message, so they continue at once. */
	function onContinue() {
		if (!keepSession.continueEnabled || busy) return;
		if (kind === "merge" && !editingMessage) {
			mergeMessage = `Merge branch '${conflict?.sourceLabel}' into ${conflict?.targetLabel}`;
			editingMessage = true;
			return;
		}
		void runContinue(kind === "merge" ? mergeMessage : undefined);
	}

	async function runContinue(message?: string) {
		busy = true;
		const label = conflict ? `${CONFLICT_VERB[conflict.kind]} ${conflict.sourceLabel}` : "";
		try {
			await keepSession.continue(message);
			editingMessage = false;
			toasts.push({
				message:
					kind === "rebase"
						? `Rebased ${conflict?.sourceLabel ?? "branch"} onto ${conflict?.targetLabel ?? ""}`.trim()
						: kind === "merge"
							? `Merged ${conflict?.sourceLabel ?? ""} into ${conflict?.targetLabel ?? ""}`.trim()
							: `${label} completed`,
				tone: "success",
				icon: "check",
			});
		} catch (e) {
			const { userMessage, raw } = asError(e);
			toasts.pushError(userMessage, raw);
		} finally {
			busy = false;
		}
	}

	function abortConsequence(): string {
		const files = `${touchedFiles} file${touchedFiles === 1 ? "" : "s"}`;
		const loss = touchedFiles > 0 ? ` Your resolved choices in ${files} will be lost.` : "";
		switch (kind) {
			case "merge":
				return `Abort merge and return ${conflict?.targetLabel} to ${targetSha ?? "its previous commit"}?${loss}`;
			case "rebase":
				return `Abort rebase and return ${conflict?.sourceLabel} to where it started?${loss}`;
			case "cherryPick":
				return `Abort cherry-pick of “${conflict?.sourceLabel}”?${loss}`;
			case "revert":
				return `Abort revert of “${conflict?.sourceLabel}”?${loss}`;
			case "stashApply":
				return `Discard this stash apply? The stash stays in your list.${loss}`;
		}
	}

	async function runAbort() {
		busy = true;
		try {
			await keepSession.abort();
			confirmingAbort = false;
			toasts.push({ message: `${ABORT_VERB[kind]}ed — repository restored`, tone: "info" });
		} catch (e) {
			const { userMessage, raw } = asError(e);
			toasts.pushError(userMessage, raw);
		} finally {
			busy = false;
		}
	}

	function keepAllGlobally(side: "ours" | "theirs") {
		keepSession.keepAllGlobally(side);
		showMenu = false;
	}
</script>

{#if conflict}
	<div class="banner" role="status">
		<div class="lead">
			<span class="kind">{CONFLICT_VERB[kind]}</span>
			<strong>{conflict.sourceLabel}</strong>
			<span class="into">into</span>
			<strong>{conflict.targetLabel}</strong>
			<span class="sep">—</span>
			<span class="counts">
				{progress.regionsResolved} of {progress.regionsTotal} conflict{progress.regionsTotal === 1 ? "" : "s"} resolved
				· {progress.filesDone} of {progress.filesTotal} file{progress.filesTotal === 1 ? "" : "s"} done
			</span>
		</div>

		<div class="actions">
			{#if editingMessage}
				<input
					class="msg"
					type="text"
					bind:value={mergeMessage}
					aria-label="Merge commit message"
					placeholder="Merge commit message"
					onkeydown={(e) => {
						if (e.key === "Enter") void runContinue(mergeMessage);
						if (e.key === "Escape") editingMessage = false;
					}}
				/>
				<button type="button" class="primary" disabled={busy} onclick={() => void runContinue(mergeMessage)}>
					Commit merge
				</button>
				<button type="button" class="ghost" onclick={() => (editingMessage = false)}>Cancel</button>
			{:else}
				<button
					type="button"
					class="primary"
					disabled={!keepSession.continueEnabled || busy}
					title={keepSession.continueEnabled
						? "Finish the operation"
						: `Still to do: ${keepSession.remainingSummary}`}
					onclick={onContinue}
				>
					{CONTINUE_VERB[kind]}
				</button>

				<div class="menu-wrap">
					<button
						type="button"
						class="ghost more"
						aria-label="More actions"
						aria-haspopup="menu"
						aria-expanded={showMenu}
						onclick={() => (showMenu = !showMenu)}
					>⋯</button>
					{#if showMenu}
						<div class="menu" role="menu">
							<button type="button" role="menuitem" onclick={() => keepAllGlobally("ours")}>
								Keep all from {conflict.targetLabel} (every file)
							</button>
							<button type="button" role="menuitem" onclick={() => keepAllGlobally("theirs")}>
								Keep all from {conflict.sourceLabel} (every file)
							</button>
						</div>
					{/if}
				</div>

				<button type="button" class="abort" onclick={() => (confirmingAbort = true)}>Abort…</button>
			{/if}
		</div>
	</div>

	{#if confirmingAbort}
		<div class="abort-confirm" role="alertdialog" aria-label="Confirm abort">
			<span>{abortConsequence()}</span>
			<div class="confirm-actions">
				<button type="button" class="ghost" onclick={() => (confirmingAbort = false)}>Keep going</button>
				<button type="button" class="danger" disabled={busy} onclick={() => void runAbort()}>
					{ABORT_VERB[kind]}
				</button>
			</div>
		</div>
	{/if}
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

	.lead {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
		flex-wrap: wrap;
	}

	.kind {
		font-weight: 600;
	}

	.lead strong {
		font-weight: 600;
		color: var(--text);
	}

	.into,
	.sep {
		color: var(--text-muted);
	}

	.counts {
		color: var(--text-muted);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex-shrink: 0;
	}

	button {
		font: inherit;
		font-size: 12px;
		padding: 4px 10px;
		border-radius: var(--radius-control);
		border: 1px solid var(--border);
		background: var(--raised);
		color: var(--text);
		cursor: pointer;
	}

	button:hover:not(:disabled) {
		background: var(--overlay);
	}

	.primary {
		border-color: var(--accent);
		background: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}

	.primary:disabled {
		border-color: var(--border);
		background: var(--raised);
		color: var(--text-faint);
		cursor: not-allowed;
	}

	.abort {
		color: var(--danger);
	}

	.more {
		padding: 4px 8px;
		font-weight: 700;
	}

	.menu-wrap {
		position: relative;
	}

	.menu {
		position: absolute;
		right: 0;
		top: calc(100% + 4px);
		z-index: 20;
		width: max-content;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 10px 30px -10px rgba(0, 0, 0, 0.5);
		padding: var(--space-1);
		display: flex;
		flex-direction: column;
	}

	.menu button {
		border: none;
		background: none;
		text-align: left;
		font-size: 12px;
		padding: var(--space-2);
		border-radius: var(--radius-control);
	}

	.menu button:hover {
		background: var(--overlay);
	}

	.msg {
		width: 320px;
		max-width: 40vw;
		font: inherit;
		font-size: 12px;
		padding: 4px var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--bg);
		color: var(--text);
	}

	.abort-confirm {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		font-size: 12px;
		color: var(--text);
		background: color-mix(in srgb, var(--danger) 10%, var(--surface));
		border-bottom: 1px solid var(--border);
	}

	.confirm-actions {
		display: flex;
		gap: var(--space-2);
		flex-shrink: 0;
	}

	.danger {
		border-color: var(--danger);
		background: var(--danger);
		color: white;
		font-weight: 600;
	}

	.danger:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}
</style>
