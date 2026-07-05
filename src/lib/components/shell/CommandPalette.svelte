<script lang="ts">
	import { fuzzyFilter } from "$lib/fuzzy";
	import { commandPalette } from "$lib/stores/commandPalette.svelte";
	import { graph } from "$lib/stores/graph.svelte";
	import { status } from "$lib/stores/status.svelte";
	import { repos } from "$lib/stores/repo.svelte";
	import { graphSelection } from "$lib/stores/graphSelection.svelte";
	import { graphNav } from "$lib/stores/graphNav.svelte";
	import { diffView } from "$lib/stores/diffView.svelte";
	import { unstagedRows, stagedRows } from "$lib/status/sections";
	import { isModEvent } from "$lib/platform";
	import * as actions from "$lib/actions";

	/** Cmd+K command palette — DESIGN_SPEC.md §10. Sections in order: Actions (context-aware),
	 * Branches (↵ checkout, Cmd↵ merge into current), Changed files (open diff), Repos (recents).
	 * Fuzzy match across all rows at once so a query can jump straight to whichever section has the
	 * best hit — the section headers just group the results, they don't gate the search. */

	const repoId = $derived(graph.repoId);
	const currentBranch = $derived(graph.head && !graph.head.detached ? graph.head.branch : null);

	interface Row {
		section: "Actions" | "Branches" | "Changed files" | "Repos";
		label: string;
		hint?: string;
		run: (mergeIntoCurrent?: boolean) => void;
	}

	let query = $state("");
	let activeIndex = $state(0);
	let inputEl = $state<HTMLInputElement | null>(null);

	function actionRows(): Row[] {
		if (!repoId) return [];
		const hasStaged = stagedRows(status.report.entries).length > 0;
		const hasAny = status.report.entries.length > 0;
		const rows: Row[] = [];
		if (hasAny) {
			rows.push({
				section: "Actions",
				label: hasStaged ? "Commit staged" : "Stage all & commit",
				hint: "⌘↵",
				run: () => void actions.commitPrimary(repoId),
			});
		}
		if (currentBranch) {
			rows.push({ section: "Actions", label: "Pull", hint: "⌘P", run: () => void actions.pull(repoId, "ff", currentBranch) });
			rows.push({ section: "Actions", label: "Push", hint: "⌘⇧P", run: () => void actions.push(repoId, false, currentBranch) });
		}
		rows.push({ section: "Actions", label: "Fetch all", run: () => void actions.fetchAll(repoId) });
		rows.push({ section: "Actions", label: "Create branch…", hint: "⌘B", run: () => {
			const sha = graph.head?.sha;
			if (sha) graphNav.scrollTo(sha);
		} });
		rows.push({ section: "Actions", label: "Stash all", hint: "⌘S", run: () => void actions.stashPush(repoId, {}) });
		if (graph.stashes.length > 0) {
			rows.push({ section: "Actions", label: "Pop latest stash", hint: "⌘⇧S", run: () => void actions.popStash(repoId, "stash@{0}", "") });
		}
		return rows;
	}

	function branchRows(): Row[] {
		if (!repoId) return [];
		return graph.refs
			.filter((r) => r.kind === "branch" || r.kind === "remoteBranch")
			.map((r): Row => ({
				section: "Branches",
				label: r.shortName,
				hint: r.kind === "remoteBranch" ? "remote" : undefined,
				run: (mergeIntoCurrent) => {
					if (mergeIntoCurrent && currentBranch && r.shortName !== currentBranch) {
						void actions.mergeInto(repoId, r.shortName, currentBranch);
					} else if (r.kind === "remoteBranch") {
						void actions.checkoutRemote(repoId, r.shortName);
					} else {
						void actions.checkoutBranch(repoId, r.shortName);
					}
				},
			}));
	}

	function fileRows(): Row[] {
		if (!repoId) return [];
		const paths = new Set<string>();
		for (const r of unstagedRows(status.report.entries)) paths.add(r.path);
		for (const r of stagedRows(status.report.entries)) paths.add(r.path);
		return Array.from(paths).map((path): Row => ({
			section: "Changed files",
			label: path,
			run: () => {
				const staged = stagedRows(status.report.entries).some((r) => r.path === path);
				diffView.open({ path, origPath: null, source: staged ? { kind: "staged" } : { kind: "workingTree" } });
			},
		}));
	}

	function repoRows(): Row[] {
		return repos.tabs
			.filter((t) => !t.id.startsWith("pending:"))
			.map((t): Row => ({
				section: "Repos",
				label: t.name,
				hint: t.branch ?? undefined,
				run: () => repos.switchTo(t.id),
			}));
	}

	const allRows = $derived<Row[]>([...actionRows(), ...branchRows(), ...fileRows(), ...repoRows()]);
	const filtered = $derived(
		fuzzyFilter(query, allRows, (r) => r.label).map((f) => f.item),
	);

	$effect(() => {
		void filtered;
		activeIndex = 0;
	});

	$effect(() => {
		if (commandPalette.isOpen) {
			query = "";
			activeIndex = 0;
			queueMicrotask(() => inputEl?.focus());
		}
	});

	function close() {
		commandPalette.close();
	}

	function pick(row: Row, e?: KeyboardEvent | MouseEvent) {
		close();
		row.run(e ? isModEvent(e) : false);
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === "Escape") {
			e.preventDefault();
			close();
		} else if (e.key === "ArrowDown") {
			e.preventDefault();
			activeIndex = Math.min(filtered.length - 1, activeIndex + 1);
		} else if (e.key === "ArrowUp") {
			e.preventDefault();
			activeIndex = Math.max(0, activeIndex - 1);
		} else if (e.key === "Enter") {
			e.preventDefault();
			const row = filtered[activeIndex];
			if (row) pick(row, e);
		}
	}

	let sections: { name: string; rows: Row[] }[] = $derived.by(() => {
		const grouped = new Map<string, Row[]>();
		for (const row of filtered) {
			(grouped.get(row.section) ?? grouped.set(row.section, []).get(row.section)!).push(row);
		}
		return Array.from(grouped, ([name, rows]) => ({ name, rows }));
	});

	function flatIndexOf(row: Row): number {
		return filtered.indexOf(row);
	}
</script>

{#if commandPalette.isOpen}
	<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
	<div class="scrim" onclick={close}></div>
	<div class="palette" role="dialog" aria-label="Command palette">
		<input
			bind:this={inputEl}
			bind:value={query}
			type="text"
			class="input"
			placeholder="Search actions, branches, files, repos…"
			spellcheck="false"
			autocomplete="off"
			onkeydown={onKeydown}
		/>
		<div class="results">
			{#if filtered.length === 0}
				<p class="empty">No matches</p>
			{:else}
				{#each sections as section (section.name)}
					<div class="section-head">{section.name}</div>
					{#each section.rows as row (row.section + row.label)}
						{@const idx = flatIndexOf(row)}
						<button
							type="button"
							class="row"
							class:active={idx === activeIndex}
							onmouseenter={() => (activeIndex = idx)}
							onclick={(e) => pick(row, e)}
						>
							<span class="label">{row.label}</span>
							{#if row.hint}<span class="hint">{row.hint}</span>{/if}
						</button>
					{/each}
				{/each}
			{/if}
		</div>
	</div>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 100;
		background: rgb(0 0 0 / 0.35);
	}

	.palette {
		position: fixed;
		z-index: 101;
		top: 15vh;
		left: 50%;
		transform: translateX(-50%);
		width: min(560px, 90vw);
		max-height: 60vh;
		display: flex;
		flex-direction: column;
		background: var(--overlay);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 16px 48px rgb(0 0 0 / 0.45);
		overflow: hidden;
	}

	.input {
		flex-shrink: 0;
		padding: var(--space-3);
		border: none;
		border-bottom: 1px solid var(--border);
		background: transparent;
		color: var(--text);
		font: inherit;
		font-size: 14px;
		outline: none;
	}

	.results {
		overflow-y: auto;
		padding: var(--space-1);
	}

	.section-head {
		padding: var(--space-2) var(--space-2) 2px;
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.04em;
		color: var(--text-faint);
	}

	.row {
		display: flex;
		align-items: center;
		width: 100%;
		padding: var(--space-2);
		border: none;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text);
		font: inherit;
		font-size: 13px;
		text-align: left;
		cursor: pointer;
	}

	.row.active {
		background: var(--raised);
	}

	.label {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.hint {
		flex-shrink: 0;
		color: var(--text-faint);
		font-size: 11px;
	}

	.empty {
		margin: 0;
		padding: var(--space-4);
		text-align: center;
		color: var(--text-faint);
		font-size: 12px;
	}
</style>
