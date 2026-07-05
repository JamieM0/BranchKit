<script lang="ts">
	import type { Snippet } from "svelte";

	/** Dynamic-visibility reveal — DESIGN_SPEC.md §13: "controls appear/disappear reactively based
	 * on sibling values (180ms height animation)". Pure-CSS `grid-template-rows` 0fr↔1fr trick, so
	 * no JS height measurement is needed and `prefers-reduced-motion` is respected automatically via
	 * `--motion-panel`. */
	let { open, children }: { open: boolean; children: Snippet } = $props();
</script>

<div class="reveal" class:open>
	<div class="inner">
		{@render children()}
	</div>
</div>

<style>
	.reveal {
		display: grid;
		grid-template-rows: 0fr;
		transition: grid-template-rows var(--motion-panel);
	}

	.reveal.open {
		grid-template-rows: 1fr;
	}

	.inner {
		overflow: hidden;
		min-height: 0;
		display: flex;
		flex-direction: column;
	}
</style>
