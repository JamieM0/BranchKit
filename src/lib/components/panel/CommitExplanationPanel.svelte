<script lang="ts">
	import { ArrowLeft, CircleNotch, Sparkle } from "phosphor-svelte";
	import * as ipc from "$lib/ipc";
	import { graph } from "$lib/stores/graph.svelte";
	import { commitExplanation } from "$lib/stores/commitExplanation.svelte";
	import { renderMarkdown } from "$lib/markdown";

	let { sha }: { sha: string } = $props();
	const repoId = $derived(graph.repoId);
	let markdown = $state("");
	let loading = $state(true);
	let error = $state<string | null>(null);
	let startedKey: string | null = null;
	const html = $derived(markdown ? renderMarkdown(markdown) : "");

	$effect(() => {
		const id = repoId;
		const currentSha = sha;
		const key = id ? `${id}:${currentSha}` : null;
		if (!id || startedKey === key) return;
		startedKey = key;
		markdown = "";
		loading = true;
		error = null;
		let unlisten: (() => void) | undefined;
		let alive = true;
		void (async () => {
			try {
				unlisten = await ipc.onAiExplanationToken((token) => {
					if (alive && sha === currentSha) markdown += token;
				});
				const result = await ipc.explainCommit(id, currentSha);
				if (alive && sha === currentSha) markdown = result.markdown;
			} catch (reason) {
				if (alive && sha === currentSha) error = reason instanceof Error ? reason.message : String(reason);
			} finally {
				if (alive && sha === currentSha) loading = false;
				unlisten?.();
			}
		})();
		return () => {
			alive = false;
			unlisten?.();
		};
	});
</script>

<section class="explanation" aria-busy={loading}>
	<header>
		<div>
			<p class="eyebrow"><Sparkle size={12} weight="fill" /> AI</p>
			<h2>Explanation</h2>
			<p class="sha">{sha.slice(0, 10)}</p>
		</div>
		<button type="button" class="back" onclick={() => commitExplanation.close()} title="Back to commit info" aria-label="Back to commit info">
			<ArrowLeft size={15} />
		</button>
	</header>

	{#if error}
		<div class="message-state error"><strong>Couldn’t explain this commit.</strong><span>{error}</span></div>
	{:else if !markdown && loading}
		<div class="message-state"><CircleNotch class="spin" size={16} /> Reading the entire commit…</div>
	{:else}
		<div class="markdown" class:streaming={loading}>{@html html}</div>
		{#if loading}<p class="generating"><CircleNotch class="spin" size={12} /> Explaining…</p>{/if}
	{/if}
</section>

<style>
	.explanation { height: 100%; overflow-y: auto; }
	header { display: flex; justify-content: space-between; gap: var(--space-2); padding: var(--space-3); border-bottom: 1px solid var(--border-soft); }
	.eyebrow { display: flex; align-items: center; gap: 4px; margin: 0 0 2px; color: var(--accent); font-size: 10px; font-weight: 700; letter-spacing: .08em; }
	h2 { margin: 0; font-size: 16px; color: var(--text); }
	.sha { margin: 3px 0 0; font: 11px var(--font-mono); color: var(--text-faint); }
	.back { width: 26px; height: 26px; display: grid; place-items: center; border: 1px solid var(--border); border-radius: var(--radius-control); color: var(--text-muted); background: transparent; cursor: pointer; }
	.back:hover { color: var(--text); background: var(--raised); }
	.message-state { display: flex; align-items: center; gap: var(--space-2); padding: var(--space-4); color: var(--text-muted); font-size: 12px; }
	.message-state.error { align-items: flex-start; flex-direction: column; color: var(--danger); }
	.markdown { padding: var(--space-3); color: var(--text); font-size: 13px; line-height: 1.55; }
	.markdown :global(h1), .markdown :global(h2), .markdown :global(h3), .markdown :global(h4) { margin: 18px 0 7px; line-height: 1.22; }
	.markdown :global(h1) { font-size: 18px; } .markdown :global(h2) { font-size: 15px; } .markdown :global(h3), .markdown :global(h4) { font-size: 13px; }
	.markdown :global(p), .markdown :global(ul), .markdown :global(ol), .markdown :global(blockquote) { margin: 0 0 var(--space-3); }
	.markdown :global(ul), .markdown :global(ol) { padding-left: 20px; }
	.markdown :global(li + li) { margin-top: 3px; }
	.markdown :global(code) { padding: 1px 3px; border-radius: 3px; font: 11px var(--font-mono); background: var(--raised); }
	.markdown :global(pre) { overflow-x: auto; padding: var(--space-2); border: 1px solid var(--border-soft); border-radius: var(--radius-control); background: var(--raised); }
	.markdown :global(pre code) { padding: 0; background: none; }
	.markdown :global(blockquote) { padding-left: var(--space-2); border-left: 2px solid var(--accent-dim); color: var(--text-muted); }
	.markdown :global(a) { color: var(--info); } .markdown :global(table) { width: 100%; border-collapse: collapse; font-size: 12px; } .markdown :global(th), .markdown :global(td) { padding: 5px; border: 1px solid var(--border); text-align: left; }
	.generating { display: flex; align-items: center; gap: 5px; margin: 0 var(--space-3) var(--space-3); color: var(--text-faint); font-size: 11px; }
	:global(.spin) { animation: spin var(--motion-loop) linear infinite; } @keyframes spin { to { transform: rotate(360deg); } } @media (prefers-reduced-motion: reduce) { :global(.spin) { animation: none; } }
</style>
