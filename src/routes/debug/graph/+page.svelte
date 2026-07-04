<script lang="ts">
	import { page } from "$app/state";
	import { graph } from "$lib/stores/graph.svelte";
	import { repos } from "$lib/stores/repo.svelte";

	const WINDOW_SIZE = 200;

	let loading = $state(false);
	let error = $state<string | null>(null);
	const path = $derived(page.url.searchParams.get("path"));

	const rows = $derived(
		graph.rows.slice(0, WINDOW_SIZE).map((row) => ({
			index: row.index,
			kind: row.kind,
			sha: row.sha.slice(0, 12),
			lane: row.node.lane,
			color: row.node.colorIndex,
			refs: row.refs.map((ref) => ref.shortName).join(", "),
			subject:
				row.kind === "commit"
					? (row.meta?.subject ?? "(metadata pending)")
					: row.subject,
			edges: row.edges.map((edge) => `${edge.kind}:${edge.fromLane}->${edge.toLane}`).join(" "),
		})),
	);

	async function load() {
		if (path && repos.active?.path !== path) {
			await repos.open(path);
		}
		const repoId = repos.activeId;
		if (!repoId || repoId.startsWith("pending:")) return;
		loading = true;
		error = null;
		try {
			await graph.open(repoId);
			await graph.ensureMetadataForWindow(0, WINDOW_SIZE);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Graph Debug</title>
</svelte:head>

<main class="debug-page">
	<header>
		<div>
			<h1>Graph Debug</h1>
			<p>{repos.active?.name ?? "No active repo"}</p>
		</div>
		<button type="button" onclick={load} disabled={!repos.activeId || loading}>
			{loading ? "Loading" : "Load active repo"}
		</button>
	</header>

	{#if error}
		<p class="error">{error}</p>
	{/if}

	<table>
		<thead>
			<tr>
				<th>#</th>
				<th>Kind</th>
				<th>SHA</th>
				<th>Lane</th>
				<th>Refs</th>
				<th>Subject</th>
				<th>Edges</th>
			</tr>
		</thead>
		<tbody>
			{#each rows as row}
				<tr>
					<td>{row.index}</td>
					<td>{row.kind}</td>
					<td><code>{row.sha}</code></td>
					<td>{row.lane} / {row.color}</td>
					<td>{row.refs}</td>
					<td>{row.subject}</td>
					<td><code>{row.edges}</code></td>
				</tr>
			{/each}
		</tbody>
	</table>
</main>

<style>
	.debug-page {
		min-height: 100vh;
		background: var(--bg);
		color: var(--text);
		padding: var(--space-4);
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		margin-bottom: var(--space-4);
	}

	h1 {
		font-size: 18px;
		margin: 0;
	}

	p {
		margin: var(--space-1) 0 0;
		color: var(--text-muted);
	}

	button {
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		background: var(--raised);
		color: var(--text);
		padding: var(--space-2) var(--space-3);
	}

	button:disabled {
		color: var(--text-faint);
	}

	.error {
		color: var(--danger);
		margin-bottom: var(--space-3);
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 12px;
	}

	th,
	td {
		border-bottom: 1px solid var(--border-soft);
		padding: var(--space-2);
		text-align: left;
		vertical-align: top;
	}

	th {
		color: var(--text-muted);
		font-weight: 600;
	}

	code {
		font-family: var(--font-mono);
	}
</style>
