<script lang="ts">
	// phosphor-svelte: icon set requested by Jamie (replaces emoji/text glyphs on action buttons).
	import {
		ArrowLineDown,
		ArrowLineUp,
		CaretDown,
		Gear,
		GitBranch,
		TrayArrowDown,
		TrayArrowUp,
	} from "phosphor-svelte";
	import * as actions from "$lib/actions";
	import * as ipc from "$lib/ipc";
	import { graph } from "$lib/stores/graph.svelte";
	import { branchEdit } from "$lib/stores/branchEdit.svelte";
	import { graphNav } from "$lib/stores/graphNav.svelte";
	import { commandPalette } from "$lib/stores/commandPalette.svelte";
	import { appSettings } from "$lib/stores/appSettings.svelte";
	import { settingsWindow } from "$lib/stores/settingsWindow.svelte";
	import { isMac } from "$lib/platform";

	/** The full toolbar sync/action cluster — DESIGN_SPEC.md §3.2:
	 * `[Pull ▾(badge ↓m)] [Push/Publish (badge ↑n)] [Branch] [Stash ▾] [Pop] ··· [⌘K]`. Repo/branch
	 * pickers and the graph filter box are separate surfaces (RepoTabs, left panel).
	 *
	 * Pull/Push/Branch/Stash/Pop act on the checked-out branch; while detached there's no branch to
	 * sync so the sync cluster hides (the detached banner, §4.6, already covers that state's
	 * affordances) — Stash/Pop/⌘K still work while detached, so they render unconditionally. */
	let { repoId }: { repoId: string } = $props();

	const currentRef = $derived(graph.refs.find((r) => r.kind === "branch" && r.isHead) ?? null);
	const branch = $derived(graph.head && !graph.head.detached ? graph.head.branch : null);
	const behind = $derived(currentRef?.behind ?? 0);
	const ahead = $derived(currentRef?.ahead ?? 0);
	const hasUpstream = $derived(currentRef?.upstream !== null && currentRef?.upstream !== undefined);
	const hasStashes = $derived(graph.stashes.length > 0);
	/** Settings → Git's "default pull mode" (DESIGN_SPEC.md §3.2/§13) — the primary Pull button's
	 * one-click action; the dropdown still offers all three explicitly. */
	const defaultPullMode = $derived(appSettings.current.git.defaultPullMode);
	const defaultPullLabel = $derived(
		defaultPullMode === "rebase"
			? "Pull (rebase)"
			: defaultPullMode === "merge"
				? "Pull (merge)"
				: "Pull (fast-forward if possible)",
	);

	// No configured remote → Publish can't work; disable it with an explanation instead of letting
	// the push die on git's raw "'origin' does not appear to be a git repository" (Jamie's report).
	let remotes = $state<string[]>([]);
	$effect(() => {
		const id = repoId;
		void graph.refs; // refresh the remote list whenever refs change (remote add/remove).
		ipc.listRemotes(id).then(
			(r) => (remotes = r),
			() => (remotes = []),
		);
	});
	const hasRemote = $derived(remotes.length > 0);

	let pullMenuOpen = $state(false);
	let pushMenuOpen = $state(false);
	let stashMenuOpen = $state(false);
	let stashMessageOpen = $state(false);
	let stashMessage = $state("");
	let forceArmed = $state(false);
	let forceArmTimer: ReturnType<typeof setTimeout> | undefined;

	function closeMenus() {
		pullMenuOpen = false;
		pushMenuOpen = false;
		stashMenuOpen = false;
		stashMessageOpen = false;
		clearTimeout(forceArmTimer);
		forceArmed = false;
	}

	function createBranch() {
		const sha = graph.head?.sha;
		if (!sha) return;
		graphNav.scrollTo(sha);
		branchEdit.startCreate(sha);
	}

	function stashAll() {
		closeMenus();
		void actions.stashPush(repoId, {});
	}

	function stashWithUntracked() {
		closeMenus();
		void actions.stashPush(repoId, { includeUntracked: true });
	}

	function openStashMessagePrompt() {
		pullMenuOpen = false;
		pushMenuOpen = false;
		stashMenuOpen = false;
		stashMessage = "";
		stashMessageOpen = true;
	}

	function submitStashMessage() {
		const message = stashMessage.trim();
		closeMenus();
		void actions.stashPush(repoId, { message: message || undefined });
	}

	/** Always `stash@{0}` — git's own notion of "latest", independent of the stash row's position
	 * in the graph (which is ordered by commit topology, not stash recency). */
	function popLatest() {
		if (!hasStashes) return;
		void actions.popStash(repoId, "stash@{0}", "");
	}

	function openPushMenu() {
		pullMenuOpen = false;
		pushMenuOpen = true;
		forceArmed = false;
		clearTimeout(forceArmTimer);
		// Armed after a beat rather than a typed-word confirm — DESIGN_SPEC §4.6.
		forceArmTimer = setTimeout(() => (forceArmed = true), 400);
	}

	async function run(fn: () => Promise<void> | void) {
		closeMenus();
		await fn();
	}

	function doPull(mode: "ff" | "rebase" | "merge") {
		if (!branch) return;
		void run(() => actions.pull(repoId, mode, branch));
	}

	function doFetch() {
		void run(() => actions.fetchAll(repoId));
	}

	function doPushOrPublish() {
		if (!branch) return;
		if (hasUpstream) void run(() => actions.push(repoId, false, branch));
		else void run(() => actions.publish(repoId, branch));
	}

	function doForcePush() {
		if (!branch) return;
		void run(() => actions.push(repoId, true, branch));
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
{#if pullMenuOpen || pushMenuOpen || stashMenuOpen || stashMessageOpen}
	<div class="scrim" onclick={closeMenus}></div>
{/if}

<div class="toolbar">
	{#if branch}
		<div class="split">
			<button type="button" class="primary" onclick={() => doPull(defaultPullMode)} title={defaultPullLabel}>
				<ArrowLineDown size={13} />
				Pull
				{#if behind > 0}<span class="badge behind">↓{behind}</span>{/if}
			</button>
			<button
				type="button"
				class="caret"
				aria-label="Pull options"
				onclick={() => {
					pushMenuOpen = false;
					pullMenuOpen = !pullMenuOpen;
				}}
			>
				<CaretDown size={10} />
			</button>
			{#if pullMenuOpen}
				<div class="menu" role="menu">
					<button type="button" role="menuitem" onclick={() => doPull("ff")}>Pull (fast-forward if possible)</button>
					<button type="button" role="menuitem" onclick={() => doPull("rebase")}>Pull (rebase)</button>
					<button type="button" role="menuitem" onclick={() => doPull("merge")}>Pull (merge)</button>
					<div class="sep"></div>
					<button type="button" role="menuitem" onclick={doFetch}>Fetch all</button>
				</div>
			{/if}
		</div>

		<div class="split">
			<button
				type="button"
				class="primary"
				disabled={!hasUpstream && !hasRemote}
				title={!hasUpstream && !hasRemote
					? "No remote configured — add one first (e.g. git remote add origin <url>)"
					: hasUpstream
						? "Push to upstream"
						: "Publish this branch to the remote"}
				onclick={doPushOrPublish}
			>
				<ArrowLineUp size={13} />
				{hasUpstream ? "Push" : "Publish"}
				{#if hasUpstream && ahead > 0}<span class="badge ahead">↑{ahead}</span>{/if}
			</button>
			{#if hasUpstream}
				<button type="button" class="caret" aria-label="Push options" onclick={openPushMenu}><CaretDown size={10} /></button>
				{#if pushMenuOpen}
					<div class="menu" role="menu">
						<p class="consequence">
							Force push will overwrite <code>origin/{branch}</code> if it has commits
							you don't — <code>--force-with-lease</code> refuses if someone else pushed since
							your last fetch.
						</p>
						<button type="button" class="danger" disabled={!forceArmed} onclick={doForcePush}>
							{forceArmed ? "Force push (with lease)…" : "Hold…"}
						</button>
					</div>
				{/if}
			{/if}
		</div>

		<button type="button" class="plain" onclick={createBranch} title="Create branch at HEAD"><GitBranch size={13} /> Branch</button>
	{/if}

	<div class="split">
		<button type="button" class="primary" onclick={stashAll} title="Stash all uncommitted changes"><TrayArrowDown size={13} /> Stash</button>
		<button
			type="button"
			class="caret"
			aria-label="Stash options"
			onclick={() => {
				pullMenuOpen = false;
				pushMenuOpen = false;
				stashMenuOpen = !stashMenuOpen;
			}}
		>
			<CaretDown size={10} />
		</button>
		{#if stashMenuOpen}
			<div class="menu" role="menu">
				<button type="button" role="menuitem" onclick={openStashMessagePrompt}>Stash with message…</button>
				<button type="button" role="menuitem" onclick={stashWithUntracked}>Stash including untracked</button>
			</div>
		{/if}
		{#if stashMessageOpen}
			<div class="menu message-menu" role="menu">
				<input
					type="text"
					placeholder="Stash message…"
					bind:value={stashMessage}
					autofocus
					onkeydown={(e) => e.key === "Enter" && submitStashMessage()}
				/>
				<div class="message-actions">
					<button type="button" onclick={closeMenus}>Cancel</button>
					<button type="button" class="primary-small" onclick={submitStashMessage}>Stash</button>
				</div>
			</div>
		{/if}
	</div>

	<button type="button" class="plain" disabled={!hasStashes} title={hasStashes ? "Pop the latest stash" : "No stashes"} onclick={popLatest}>
		<TrayArrowUp size={13} /> Pop
	</button>

	<button type="button" class="palette-trigger" onclick={() => commandPalette.open()} title="Command palette">
		{isMac() ? "⌘K" : "Ctrl+K"}
	</button>

	<button
		type="button"
		class="settings-trigger"
		onclick={() => settingsWindow.show()}
		title={isMac() ? "Settings (⌘,)" : "Settings (Ctrl+,)"}
		aria-label="Settings"
	>
		<Gear size={14} />
	</button>
</div>

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 40;
	}

	.toolbar {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border);
		background: var(--surface);
	}

	.split {
		position: relative;
		display: flex;
		align-items: stretch;
	}

	.split button.primary {
		display: flex;
		align-items: center;
		gap: 5px;
		padding: var(--space-1) var(--space-2);
		border: 1px solid var(--border);
		border-right: none;
		border-radius: var(--radius-control) 0 0 var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 12px;
		font-weight: 600;
		cursor: pointer;
		transition: background var(--motion-hover), border-color var(--motion-hover);
	}

	.split button.primary:hover:not(:disabled) {
		background: var(--overlay);
		border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
	}

	.split button.primary:disabled {
		color: var(--text-faint);
		cursor: default;
	}

	/* No caret sibling (e.g. Publish before there's an upstream to offer push options for) —
	   the primary button is the whole control, so give it its right border/radius back instead
	   of leaving the edge open as if a caret were about to close it off. */
	.split button.primary:last-child {
		border-right: 1px solid var(--border);
		border-radius: var(--radius-control);
	}

	.split button.caret {
		padding: var(--space-1) var(--space-1);
		border: 1px solid var(--border);
		border-radius: 0 var(--radius-control) var(--radius-control) 0;
		background: var(--raised);
		color: var(--text-muted);
		font: inherit;
		font-size: 10px;
		cursor: pointer;
		transition: background var(--motion-hover);
	}

	.split button.caret:hover {
		background: var(--overlay);
	}

	.badge {
		font-variant-numeric: tabular-nums;
		font-size: 11px;
		font-weight: 700;
	}

	.badge.ahead {
		color: var(--ahead);
	}
	.badge.behind {
		color: var(--behind);
	}

	.menu {
		position: absolute;
		z-index: 41;
		top: calc(100% + 4px);
		left: 0;
		min-width: 240px;
		padding: var(--space-1);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.menu button {
		display: block;
		width: 100%;
		text-align: left;
		padding: var(--space-2) var(--space-2);
		border: none;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text);
		font: inherit;
		font-size: 12px;
		cursor: pointer;
	}

	.menu button:hover {
		background: var(--raised);
	}

	.menu .sep {
		height: 1px;
		margin: var(--space-1) 0;
		background: var(--border-soft);
	}

	.consequence {
		margin: 0 0 var(--space-1);
		padding: 0 var(--space-2);
		font-size: 11px;
		color: var(--text-muted);
	}

	.consequence code {
		font-family: var(--font-mono);
	}

	.menu button.danger {
		color: var(--danger);
		font-weight: 600;
	}

	.menu button.danger:disabled {
		color: var(--text-faint);
		cursor: default;
	}

	.menu button.danger:not(:disabled):hover {
		background: color-mix(in srgb, var(--danger) 16%, var(--raised));
	}

	button.plain {
		display: flex;
		align-items: center;
		gap: 5px;
		padding: var(--space-1) var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 12px;
		font-weight: 600;
		cursor: pointer;
		transition: background var(--motion-hover), border-color var(--motion-hover);
	}

	button.plain:hover:not(:disabled) {
		background: var(--overlay);
		border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
	}

	button.plain:disabled {
		color: var(--text-faint);
		cursor: default;
	}

	.palette-trigger {
		margin-left: auto;
		padding: var(--space-1) var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text-muted);
		font: inherit;
		font-size: 11px;
		cursor: pointer;
	}

	.palette-trigger:hover {
		background: var(--overlay);
		color: var(--text);
	}

	.settings-trigger {
		padding: var(--space-1) var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text-muted);
		font-size: 13px;
		line-height: 1;
		cursor: pointer;
	}

	.settings-trigger:hover {
		background: var(--overlay);
		color: var(--text);
	}

	.message-menu {
		gap: var(--space-2);
		padding: var(--space-2);
	}

	.message-menu input {
		padding: 4px var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: 12px;
	}

	.message-menu input:focus {
		outline: none;
		border-color: var(--accent);
	}

	.message-actions {
		display: flex;
		gap: var(--space-1);
	}

	.message-actions button {
		flex: 1;
		padding: 3px var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: 11px;
		cursor: pointer;
	}

	.message-actions .primary-small {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}
</style>
