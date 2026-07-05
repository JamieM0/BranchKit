<script lang="ts">
	import type { Snippet } from "svelte";

	/** One settings control + its required one-line description (DESIGN_SPEC.md §13: "every
	 * control has a one-line description under it"). */
	let { label, description, children }: { label: string; description: string; children: Snippet } =
		$props();
</script>

<div class="field-row">
	<div class="control">{@render children()}</div>
	<div class="text">
		<span class="label">{label}</span>
		<p class="desc">{description}</p>
	</div>
</div>

<style>
	.field-row {
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
		padding: var(--space-2) 0;
	}

	.control {
		flex-shrink: 0;
		padding-top: 1px;
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.label {
		font-size: 13px;
		color: var(--text);
	}

	.desc {
		margin: 0;
		font-size: 11px;
		color: var(--text-faint);
	}
</style>
