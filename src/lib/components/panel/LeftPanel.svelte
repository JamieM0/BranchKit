<script lang="ts">
	import type { RefInfo } from "$lib/types";
	import type { Pill } from "$lib/graph/pills";
	import { buildPanelModel } from "$lib/graph/panel";
	import { fuzzyMatch } from "$lib/fuzzy";
	import { graph } from "$lib/stores/graph.svelte";
	import { settings } from "$lib/stores/settings.svelte";
	import { filter } from "$lib/stores/filter.svelte";
	import { graphNav } from "$lib/stores/graphNav.svelte";
	import { branchEdit } from "$lib/stores/branchEdit.svelte";
	import * as actions from "$lib/actions";
	import BranchMenu from "$lib/components/graph/BranchMenu.svelte";

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
			<button type="button" class="rail-btn" title="Local branches" aria-label="Local branches">💻<span class="rail-count">{model.locals.length}</span></button>
			<button type="button" class="rail-btn" title="Remotes" aria-label="Remotes">☁<span class="rail-count">{model.remotes.reduce((n, g) => n + g.branches.length, 0)}</span></button>
			<button type="button" class="rail-btn" title="Tags" aria-label="Tags">🏷<span class="rail-count">{model.tags.length}</span></button>
			<button type="button" class="rail-btn" title="Stashes" aria-label="Stashes">📦<span class="rail-count">{graph.stashes.length}</span></button>
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
							{#if row.tracked}<span class="presence" title={row.tracked.shortName}>☁</span>{/if}
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
								}}>👁</button
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
								<span class="presence" aria-hidden="true">☁</span>
								<span class="name">{pill.name}</span>
							</div>
						{/each}
					{/if}
				</section>
			{/each}

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
							<div class="row" onclick={() => onRowClick(tag.sha)} onmouseenter={() => onRowHover(tag.sha)} onmouseleave={() => onRowHover(null)}>
								<span class="presence" aria-hidden="true">🏷</span>
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
							<div class="row" onclick={() => onRowClick(stash.baseSha)} onmouseenter={() => onRowHover(stash.baseSha)} onmouseleave={() => onRowHover(null)}>
								<span class="presence" aria-hidden="true">📦</span>
								<span class="name stash">{stash.subject || stash.selector}</span>
							</div>
						{/each}
					{/if}
				</section>
			{/if}

			<!-- WORKTREES -->
			{#if worktrees.length > 0}
				<section>
					<button type="button" class="section-head" onclick={() => toggleSection("worktrees")}>
						<span class="chev" class:open={sectionOpen("worktrees")}>▸</span>
						WORKTREES <span class="count">{worktrees.length}</span>
					</button>
					{#if sectionOpen("worktrees")}
						{#each worktrees as wt (wt.path)}
							<div class="row worktree" title={wt.path}>
								<span class="presence" aria-hidden="true">🌿</span>
								<span class="name">{wt.branch ?? wt.head.slice(0, 7)}</span>
								{#if wt.isMain}<span class="tagpill">main</span>{/if}
							</div>
						{/each}
					{/if}
				</section>
			{/if}
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
</style>
