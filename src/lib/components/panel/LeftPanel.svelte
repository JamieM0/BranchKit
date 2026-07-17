<script lang="ts">
	// phosphor-svelte: icon set requested by Jamie (replaces emoji glyphs).
	import {
		Archive,
		Cloud,
		Eye,
		EyeSlash,
		GitBranch,
		Link,
		Lock,
		Plus,
		Tag,
	} from "phosphor-svelte";
	import type { RefInfo } from "$lib/types";
	import type { Pill } from "$lib/graph/pills";
	import { buildPanelModel } from "$lib/graph/panel";
	import { fuzzyMatch } from "$lib/fuzzy";
	import { graph } from "$lib/stores/graph.svelte";
	import { settings } from "$lib/stores/settings.svelte";
	import { filter } from "$lib/stores/filter.svelte";
	import { graphNav } from "$lib/stores/graphNav.svelte";
	import { branchEdit } from "$lib/stores/branchEdit.svelte";
	import { repos } from "$lib/stores/repo.svelte";
	import { worktreeDialog } from "$lib/stores/worktreeDialog.svelte";
	import { github } from "$lib/stores/github.svelte";
	import { prPanel } from "$lib/stores/prPanel.svelte";
	import { settingsWindow } from "$lib/stores/settingsWindow.svelte";
	import * as actions from "$lib/actions";
	import BranchMenu from "$lib/components/graph/BranchMenu.svelte";
	import StashMenu from "$lib/components/graph/StashMenu.svelte";
	import ContextMenu, { type MenuItem } from "$lib/components/shell/ContextMenu.svelte";
	import type { WorktreeInfo } from "$lib/types";

	/** The left panel — DESIGN_SPEC.md §5. Sections LOCAL / REMOTES / TAGS / STASHES / WORKTREES with
	 * one universal filter box that fuzzy-filters every section and dims the graph (§15.24), the
	 * combine-tracking-rows behavior (§15.26), hover→pill glow (§15.25), click→scroll-to-tip,
	 * double-click checkout, per-branch hide-eye, and collapse to an icon rail. PRs arrive with the
	 * GitHub prompt. */

	const repoId = $derived(graph.repoId);
	const currentBranch = $derived(graph.head && !graph.head.detached ? graph.head.branch : null);
	const model = $derived(buildPanelModel(graph.refs, settings.combineTrackingBranches));

	let query = $state("");
	// Feed the shared filter (dims the graph) as the box changes.
	$effect(() => {
		filter.set(query);
	});

	const q = $derived(query.trim());
	const filtering = $derived(q.length > 0);

	function matches(text: string): boolean {
		return q === "" || fuzzyMatch(q, text) !== null;
	}

	// Section collapse — auto-expanded while filtering so hits are always visible (§5).
	let collapsed = $state<Record<string, boolean>>({});
	function sectionOpen(id: string): boolean {
		if (filtering) return true;
		return !collapsed[id];
	}
	function toggleSection(id: string) {
		collapsed = { ...collapsed, [id]: !collapsed[id] };
	}

	// --- filtered views ---
	const locals = $derived(model.locals.filter((l) => matches(l.local.shortName)));
	const remoteGroups = $derived(
		model.remotes
			.map((g) => ({ name: g.name, branches: g.branches.filter((b) => matches(b.shortName)) }))
			.filter((g) => g.branches.length > 0),
	);
	const tags = $derived(model.tags.filter((t) => matches(t.shortName)));
	const stashes = $derived(graph.stashes.filter((s) => matches(s.subject)));
	const worktrees = $derived(graph.worktrees.filter((w) => matches(w.path)));
	const prs = $derived(github.pullRequests.filter((p) => matches(p.title) || matches(p.headRef)));

	// PULL REQUESTS section — DESIGN_SPEC.md §5/§12. Degrades to a quiet "Connect GitHub" row when
	// not signed in; loads (and reloads on repo switch) only once connected.
	$effect(() => {
		if (repoId && github.connected) void github.loadPullRequests(repoId);
		else github.reset();
	});

	// --- pill construction for menu/actions ---
	function localPill(ref: RefInfo): Pill {
		return {
			key: `local:${ref.shortName}`,
			kind: "branch",
			name: ref.shortName,
			sha: ref.sha,
			isHead: ref.shortName === currentBranch,
			local: true,
			remote: ref.upstream !== null,
			remoteName: ref.upstream,
			ahead: ref.ahead,
			behind: ref.behind,
			diverged: ref.ahead > 0 && ref.behind > 0,
			localBranch: ref.shortName,
			remoteRef: null,
			isRemoteOnly: false,
			upstream: ref.upstream,
		};
	}

	function remotePill(ref: RefInfo): Pill {
		const tracking = graph.refs.find((r) => r.kind === "branch" && r.upstream === ref.shortName);
		const branchPart = ref.shortName.slice(ref.shortName.indexOf("/") + 1);
		return {
			key: `remote:${ref.shortName}`,
			kind: "branch",
			name: branchPart,
			sha: ref.sha,
			isHead: false,
			local: false,
			remote: true,
			remoteName: ref.shortName,
			ahead: 0,
			behind: 0,
			diverged: false,
			localBranch: tracking ? tracking.shortName : null,
			remoteRef: ref.shortName,
			isRemoteOnly: !tracking,
			upstream: null,
		};
	}

	// --- overlay + interactions ---
	let menu = $state<{ pill: Pill; x: number; y: number } | null>(null);
	let stashMenu = $state<{ selector: string; subject: string; x: number; y: number } | null>(null);
	let tagMenu = $state<{ name: string; x: number; y: number } | null>(null);
	let worktreeMenu = $state<{ wt: WorktreeInfo; x: number; y: number } | null>(null);
	let removeConfirm = $state<{ path: string; armed: boolean } | null>(null);
	let armTimer: ReturnType<typeof setTimeout> | undefined;

	function openWorktreeMenu(wt: WorktreeInfo, e: MouseEvent) {
		e.preventDefault();
		worktreeMenu = { wt, x: e.clientX, y: e.clientY };
	}

	function openWorktreeTab(wt: WorktreeInfo) {
		void repos.open(wt.path);
	}

	async function startRemoveWorktree(path: string) {
		try {
			await actions.removeWorktree(repoId!, path, false);
		} catch {
			// Dirty worktree — escalate to an armed force-confirm, same shape as branch delete (§4.6).
			clearTimeout(armTimer);
			removeConfirm = { path, armed: false };
			armTimer = setTimeout(() => {
				if (removeConfirm) removeConfirm = { ...removeConfirm, armed: true };
			}, 400);
		}
	}

	function cancelRemoveWorktree() {
		clearTimeout(armTimer);
		removeConfirm = null;
	}

	async function confirmRemoveWorktree() {
		if (!removeConfirm?.armed || !repoId) return;
		const path = removeConfirm.path;
		removeConfirm = null;
		await actions.removeWorktree(repoId, path, true);
	}

	const worktreeMenuItems: MenuItem[] = $derived(
		worktreeMenu && repoId
			? [
					{ type: "action", label: "Open", run: () => openWorktreeTab(worktreeMenu!.wt) },
					{
						type: "action",
						label: "Remove…",
						danger: true,
						disabledReason: worktreeMenu.wt.isMain ? "Can't remove the main worktree" : undefined,
						run: () => void startRemoveWorktree(worktreeMenu!.wt.path),
					},
					{ type: "action", label: "Prune all", run: () => void actions.pruneWorktrees(repoId!) },
				]
			: [],
	);

	function openStashMenu(selector: string, subject: string, e: MouseEvent) {
		e.preventDefault();
		stashMenu = { selector, subject, x: e.clientX, y: e.clientY };
	}

	function openTagMenu(name: string, e: MouseEvent) {
		e.preventDefault();
		tagMenu = { name, x: e.clientX, y: e.clientY };
	}

	const tagMenuItems: MenuItem[] = $derived(
		tagMenu && repoId
			? [
					{ type: "action", label: "Copy tag name", run: () => void actions.copyToClipboard(tagMenu!.name, "Copied tag name") },
					{ type: "action", label: "Delete tag", danger: true, run: () => void actions.deleteTag(repoId!, tagMenu!.name) },
				]
			: [],
	);

	function onRowHover(sha: string | null) {
		graphNav.setGlow(sha);
	}

	function onRowClick(sha: string) {
		graphNav.scrollTo(sha);
	}

	function checkoutPill(pill: Pill) {
		if (!repoId) return;
		if (pill.isRemoteOnly && pill.remoteRef) void actions.checkoutRemote(repoId, pill.remoteRef);
		else if (pill.localBranch) void actions.checkoutBranch(repoId, pill.localBranch);
	}

	function openMenu(pill: Pill, e: MouseEvent) {
		e.preventDefault();
		menu = { pill, x: e.clientX, y: e.clientY };
	}
</script>

<aside class="panel" class:collapsed={settings.leftPanelCollapsed}>
	{#if settings.leftPanelCollapsed}
		<div class="rail">
			<button type="button" class="rail-btn" title="Expand panel" onclick={() => settings.toggleLeftPanel()}>›</button>
			<button type="button" class="rail-btn" title="Local branches" aria-label="Local branches"><GitBranch size={14} /><span class="rail-count">{model.locals.length}</span></button>
			<button type="button" class="rail-btn" title="Remotes" aria-label="Remotes"><Cloud size={14} /><span class="rail-count">{model.remotes.reduce((n, g) => n + g.branches.length, 0)}</span></button>
			<button type="button" class="rail-btn" title="Tags" aria-label="Tags"><Tag size={14} /><span class="rail-count">{model.tags.length}</span></button>
			<button type="button" class="rail-btn" title="Stashes" aria-label="Stashes"><Archive size={14} /><span class="rail-count">{graph.stashes.length}</span></button>
		</div>
	{:else}
		<div class="head">
			<input
				class="filter"
				type="text"
				placeholder="Filter branches, tags…"
				spellcheck="false"
				autocomplete="off"
				bind:value={query}
				aria-label="Filter panel and graph"
			/>
			<button type="button" class="collapse" title="Collapse panel" aria-label="Collapse panel" onclick={() => settings.toggleLeftPanel()}>‹</button>
		</div>

		<div class="sections">
			<!-- LOCAL -->
			<section>
				<button type="button" class="section-head" onclick={() => toggleSection("local")}>
					<span class="chev" class:open={sectionOpen("local")}>▸</span>
					LOCAL <span class="count">{locals.length}</span>
				</button>
				{#if sectionOpen("local")}
					{#each locals as row (row.local.shortName)}
						{@const pill = localPill(row.local)}
						<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
						<div
							class="row"
							class:head-branch={pill.isHead}
							onmouseenter={() => onRowHover(row.local.sha)}
							onmouseleave={() => onRowHover(null)}
							onclick={() => onRowClick(row.local.sha)}
							ondblclick={() => checkoutPill(pill)}
							oncontextmenu={(e) => openMenu(pill, e)}
						>
							{#if pill.isHead}<span class="dot" aria-label="checked out"></span>{/if}
							<span class="name">{row.local.shortName}</span>
							{#if row.tracked}<span class="presence" title={row.tracked.shortName}><Cloud size={11} /></span>{/if}
							<span class="badges">
								{#if row.local.ahead > 0}<span class="ab ahead">↑{row.local.ahead}</span>{/if}
								{#if row.local.behind > 0}<span class="ab behind">↓{row.local.behind}</span>{/if}
							</span>
							<button
								type="button"
								class="eye"
								class:hidden-on={filter.isHidden(row.local.shortName)}
								title={filter.isHidden(row.local.shortName) ? "Show in graph" : "Hide from graph"}
								aria-label="Toggle graph visibility"
								onclick={(e) => {
									e.stopPropagation();
									filter.toggleHidden(row.local.shortName);
								}}>{#if filter.isHidden(row.local.shortName)}<EyeSlash size={12} />{:else}<Eye size={12} />{/if}</button
							>
						</div>
					{/each}
					{#if locals.length === 0}<p class="empty">No local branches</p>{/if}
				{/if}
			</section>

			<!-- REMOTES -->
			{#each remoteGroups as group (group.name)}
				<section>
					<button type="button" class="section-head" onclick={() => toggleSection(`remote:${group.name}`)}>
						<span class="chev" class:open={sectionOpen(`remote:${group.name}`)}>▸</span>
						{group.name.toUpperCase()} <span class="count">{group.branches.length}</span>
					</button>
					{#if sectionOpen(`remote:${group.name}`)}
						{#each group.branches as branch (branch.shortName)}
							{@const pill = remotePill(branch)}
							<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
							<div
								class="row"
								onmouseenter={() => onRowHover(branch.sha)}
								onmouseleave={() => onRowHover(null)}
								onclick={() => onRowClick(branch.sha)}
								ondblclick={() => checkoutPill(pill)}
								oncontextmenu={(e) => openMenu(pill, e)}
							>
								<span class="presence" aria-hidden="true"><Cloud size={11} /></span>
								<span class="name">{pill.name}</span>
							</div>
						{/each}
					{/if}
				</section>
			{/each}

			<!-- PULL REQUESTS -->
			<section>
				<button type="button" class="section-head" onclick={() => toggleSection("prs")}>
					<span class="chev" class:open={sectionOpen("prs")}>▸</span>
					PULL REQUESTS <span class="count">{prs.length}</span>
				</button>
				{#if sectionOpen("prs")}
					{#if !github.connected}
						<button type="button" class="row connect-gh" onclick={() => settingsWindow.show("integrations")}>
							<span class="presence" aria-hidden="true"><Link size={11} /></span>
							<span class="name">Connect GitHub</span>
						</button>
					{:else}
						{#each prs as pr (pr.number)}
							<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
							<div class="row" onclick={() => prPanel.selectPr(pr.number)}>
								<img class="pr-avatar" src={pr.authorAvatarUrl} alt="" />
								<span class="name">#{pr.number} {pr.title}</span>
							</div>
						{/each}
						{#if prs.length === 0}<p class="empty">No open pull requests</p>{/if}
						<button type="button" class="row add-pr" onclick={() => prPanel.openCreate()}>
							<span class="presence" aria-hidden="true"><Plus size={11} /></span>
							<span class="name">New pull request</span>
						</button>
					{/if}
				{/if}
			</section>

			<!-- TAGS -->
			{#if tags.length > 0}
				<section>
					<button type="button" class="section-head" onclick={() => toggleSection("tags")}>
						<span class="chev" class:open={sectionOpen("tags")}>▸</span>
						TAGS <span class="count">{tags.length}</span>
					</button>
					{#if sectionOpen("tags")}
						{#each tags as tag (tag.shortName)}
							<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
							<div
								class="row"
								onclick={() => onRowClick(tag.sha)}
								onmouseenter={() => onRowHover(tag.sha)}
								onmouseleave={() => onRowHover(null)}
								oncontextmenu={(e) => openTagMenu(tag.shortName, e)}
							>
								<span class="presence" aria-hidden="true"><Tag size={11} /></span>
								<span class="name">{tag.shortName}</span>
							</div>
						{/each}
					{/if}
				</section>
			{/if}

			<!-- STASHES -->
			{#if stashes.length > 0}
				<section>
					<button type="button" class="section-head" onclick={() => toggleSection("stashes")}>
						<span class="chev" class:open={sectionOpen("stashes")}>▸</span>
						STASHES <span class="count">{stashes.length}</span>
					</button>
					{#if sectionOpen("stashes")}
						{#each stashes as stash (stash.selector)}
							<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
							<div
								class="row"
								onclick={() => onRowClick(stash.baseSha)}
								ondblclick={() => repoId && actions.popStash(repoId, stash.selector, stash.subject)}
								onmouseenter={() => onRowHover(stash.baseSha)}
								onmouseleave={() => onRowHover(null)}
								oncontextmenu={(e) => openStashMenu(stash.selector, stash.subject, e)}
							>
								<span class="presence" aria-hidden="true"><Archive size={11} /></span>
								<span class="name stash">{stash.subject || stash.selector}</span>
							</div>
						{/each}
					{/if}
				</section>
			{/if}

			<!-- WORKTREES -->
			<section>
				<div class="section-head worktrees-head">
					<button type="button" class="section-head-btn" onclick={() => toggleSection("worktrees")}>
						<span class="chev" class:open={sectionOpen("worktrees")}>▸</span>
						WORKTREES <span class="count">{worktrees.length}</span>
					</button>
					<button
						type="button"
						class="add-worktree"
						title="Create a worktree"
						aria-label="Create a worktree"
						onclick={() => worktreeDialog.open(currentBranch ?? "HEAD")}
					>
						<Plus size={12} />
					</button>
				</div>
				{#if sectionOpen("worktrees")}
					{#each worktrees as wt (wt.path)}
						{@const isOpenTab = repos.tabs.some((t) => t.path === wt.path)}
						<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
						<div
							class="row worktree"
							title={wt.path}
							onclick={() => openWorktreeTab(wt)}
							oncontextmenu={(e) => openWorktreeMenu(wt, e)}
						>
							<span class="status-dot" class:open-tab={isOpenTab} aria-hidden="true"></span>
							<span class="name">{wt.branch ?? wt.head.slice(0, 7)}</span>
							{#if wt.isMain}<span class="tagpill">main</span>{/if}
							{#if wt.locked}<span class="presence" title="Locked"><Lock size={11} /></span>{/if}
						</div>
					{/each}
					{#if worktrees.length === 0}<p class="empty">No linked worktrees</p>{/if}
				{/if}
			</section>
		</div>
	{/if}
</aside>

{#if menu && repoId}
	<BranchMenu
		pill={menu.pill}
		{repoId}
		{currentBranch}
		x={menu.x}
		y={menu.y}
		onDismiss={() => (menu = null)}
		onRename={(pill) => pill.localBranch && branchEdit.startRename(pill.localBranch, pill.sha)}
		onCreateBranch={(sha) => branchEdit.startCreate(sha)}
	/>
{/if}

{#if stashMenu && repoId}
	<StashMenu
		selector={stashMenu.selector}
		subject={stashMenu.subject}
		{repoId}
		x={stashMenu.x}
		y={stashMenu.y}
		onDismiss={() => (stashMenu = null)}
	/>
{/if}

{#if tagMenu}
	<ContextMenu items={tagMenuItems} x={tagMenu.x} y={tagMenu.y} onDismiss={() => (tagMenu = null)} ariaLabel="Tag actions" />
{/if}

{#if worktreeMenu}
	<ContextMenu
		items={worktreeMenuItems}
		x={worktreeMenu.x}
		y={worktreeMenu.y}
		onDismiss={() => (worktreeMenu = null)}
		ariaLabel="Worktree actions"
	/>
{/if}

{#if removeConfirm}
	<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
	<div class="scrim" onclick={cancelRemoveWorktree}></div>
	<div class="remove-confirm" role="dialog" aria-modal="true" aria-label="Remove worktree">
		<p class="confirm-text">
			<code>{removeConfirm.path}</code> has uncommitted or untracked changes. Remove it anyway?
		</p>
		<div class="confirm-actions">
			<button type="button" onclick={cancelRemoveWorktree}>Cancel</button>
			<button type="button" class="danger-solid" disabled={!removeConfirm.armed} onclick={confirmRemoveWorktree}>
				{removeConfirm.armed ? "Remove worktree" : "Hold…"}
			</button>
		</div>
	</div>
{/if}

<style>
	.panel {
		display: flex;
		flex-direction: column;
		width: 240px;
		min-width: 240px;
		height: 100%;
		background: var(--surface);
		border-right: 1px solid var(--border);
		overflow: hidden;
	}

	.panel.collapsed {
		width: 44px;
		min-width: 44px;
	}

	.rail {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) 0;
	}

	.rail-btn {
		position: relative;
		width: 32px;
		height: 32px;
		border: none;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text-muted);
		font-size: 15px;
		cursor: pointer;
	}

	.rail-btn:hover {
		background: var(--raised);
		color: var(--text);
	}

	.rail-count {
		position: absolute;
		bottom: -2px;
		right: -2px;
		font-size: 9px;
		background: var(--raised);
		border-radius: var(--radius-pill);
		padding: 0 3px;
		color: var(--text-muted);
	}

	.head {
		display: flex;
		gap: var(--space-1);
		padding: var(--space-2);
		border-bottom: 1px solid var(--border-soft);
	}

	.filter {
		flex: 1;
		min-width: 0;
		padding: 4px var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		font: inherit;
		font-size: 12px;
		outline: none;
	}

	.filter:focus {
		border-color: var(--accent);
	}

	.collapse {
		width: 26px;
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text-muted);
		cursor: pointer;
	}

	.collapse:hover {
		color: var(--text);
	}

	.sections {
		flex: 1;
		overflow-y: auto;
		padding-bottom: var(--space-3);
	}

	.section-head {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		width: 100%;
		padding: var(--space-2) var(--space-3) 2px;
		border: none;
		background: none;
		color: var(--text-muted);
		font: inherit;
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.04em;
		cursor: pointer;
	}

	.chev {
		display: inline-block;
		font-size: 8px;
		transition: transform var(--motion-hover);
	}

	.chev.open {
		transform: rotate(90deg);
	}

	.count {
		color: var(--text-faint);
		font-weight: 600;
	}

	.worktrees-head {
		display: flex;
		align-items: center;
		padding-right: var(--space-2);
	}

	.section-head-btn {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		flex: 1;
		min-width: 0;
		padding: var(--space-2) 0 2px var(--space-3);
		border: none;
		background: none;
		color: var(--text-muted);
		font: inherit;
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.04em;
		cursor: pointer;
	}

	.add-worktree {
		flex-shrink: 0;
		width: 18px;
		height: 18px;
		border: none;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text-muted);
		font-size: 12px;
		line-height: 1;
		cursor: pointer;
	}

	.add-worktree:hover {
		background: var(--raised);
		color: var(--text);
	}

	.status-dot {
		width: 6px;
		height: 6px;
		flex-shrink: 0;
		border-radius: var(--radius-pill);
		background: var(--text-faint);
	}

	.status-dot.open-tab {
		background: var(--accent);
	}

	.scrim {
		position: fixed;
		inset: 0;
		z-index: 90;
	}

	.remove-confirm {
		position: fixed;
		z-index: 91;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		width: 320px;
		padding: var(--space-3);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 16px 48px rgb(0 0 0 / 35%);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.confirm-text {
		margin: 0;
		font-size: 12px;
		color: var(--text);
	}

	.confirm-text code {
		font-family: var(--font-mono);
		word-break: break-all;
	}

	.confirm-actions {
		display: flex;
		gap: var(--space-1);
	}

	.confirm-actions button {
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

	.confirm-actions .danger-solid {
		background: var(--danger);
		border-color: var(--danger);
		color: var(--bg);
		font-weight: 600;
	}

	.confirm-actions .danger-solid:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.row {
		display: flex;
		align-items: center;
		gap: 5px;
		padding: 3px var(--space-3) 3px var(--space-4);
		font-size: 12px;
		color: var(--text);
		cursor: pointer;
	}

	.row:hover {
		background: var(--raised);
	}

	.row.head-branch .name {
		font-weight: 700;
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: var(--radius-pill);
		background: var(--accent);
		flex-shrink: 0;
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.name.stash {
		font-style: italic;
		color: var(--text-muted);
	}

	.presence {
		font-size: 9px;
		flex-shrink: 0;
	}

	.badges {
		margin-left: auto;
		display: inline-flex;
		gap: 3px;
		font-variant-numeric: tabular-nums;
		font-size: 10px;
	}

	.ab.ahead {
		color: var(--ahead);
	}
	.ab.behind {
		color: var(--behind);
	}

	.eye {
		flex-shrink: 0;
		border: none;
		background: none;
		color: var(--text-faint);
		font-size: 11px;
		cursor: pointer;
		opacity: 0;
		padding: 0 2px;
	}

	.row:hover .eye,
	.eye.hidden-on {
		opacity: 1;
	}

	/* Hidden branch: eye reads "off" — dimmed with a slash. */
	.eye.hidden-on {
		color: var(--text-faint);
		text-decoration: line-through;
	}

	.eye:hover {
		color: var(--text);
	}

	.tagpill {
		margin-left: auto;
		font-size: 9px;
		color: var(--text-muted);
		background: var(--raised);
		border-radius: var(--radius-pill);
		padding: 0 5px;
	}

	.empty {
		margin: 0;
		padding: 2px var(--space-4) var(--space-2);
		font-size: 11px;
		color: var(--text-faint);
	}

	button.row {
		width: 100%;
		border: none;
		background: none;
		text-align: left;
		font: inherit;
	}

	.pr-avatar {
		width: 14px;
		height: 14px;
		border-radius: var(--radius-pill);
		flex-shrink: 0;
	}

	.connect-gh .name,
	.add-pr .name {
		color: var(--text-muted);
	}
</style>
