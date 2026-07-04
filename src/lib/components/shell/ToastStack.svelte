<script lang="ts">
	import { toasts } from "$lib/stores/toasts.svelte";

	/** Bottom-left toast stack — DESIGN_SPEC.md §8. Renders the shared toasts store; hover pauses,
	 * one action verb per toast, failures reveal raw git output behind Details. */
	const ICONS: Record<string, string> = {
		check: "✓",
		back: "↩",
		undo: "↺",
		view: "→",
		alert: "‼",
		branch: "⑂",
		merge: "⑃",
	};
</script>

<div class="toast-stack" role="region" aria-label="Notifications">
	{#each toasts.items as toast (toast.id)}
		<div
			class="toast tone-{toast.tone}"
			role="status"
			onmouseenter={() => toasts.pause(toast.id)}
			onmouseleave={() => toasts.resume(toast.id)}
		>
			<span class="icon" aria-hidden="true">{ICONS[toast.icon ?? ""] ?? "•"}</span>
			<div class="body">
				<p class="message">{toast.message}</p>
				{#if toast.details}
					<details class="details">
						<summary>Details</summary>
						<pre>{toast.details}</pre>
					</details>
				{/if}
			</div>
			{#if toast.action}
				<button type="button" class="action" onclick={() => toasts.runAction(toast.id)}>
					{toast.action.label}
				</button>
			{/if}
			<button
				type="button"
				class="close"
				aria-label="Dismiss"
				title="Dismiss"
				onclick={() => toasts.dismiss(toast.id)}>✕</button
			>
		</div>
	{/each}
</div>

<style>
	.toast-stack {
		position: fixed;
		left: var(--space-4);
		bottom: var(--space-4);
		z-index: 60;
		display: flex;
		flex-direction: column-reverse;
		gap: var(--space-2);
		pointer-events: none;
	}

	.toast {
		pointer-events: auto;
		display: flex;
		align-items: flex-start;
		gap: var(--space-2);
		width: 340px;
		max-width: calc(100vw - 2 * var(--space-4));
		padding: var(--space-2) var(--space-3);
		background: var(--overlay);
		border: 1px solid var(--border);
		border-left: 3px solid var(--text-muted);
		border-radius: var(--radius-card);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
		font-size: 12px;
		color: var(--text);
		animation: toast-in var(--motion-panel);
	}

	.tone-success {
		border-left-color: var(--accent);
	}
	.tone-warn {
		border-left-color: var(--warn);
	}
	.tone-danger {
		border-left-color: var(--danger);
	}
	.tone-info {
		border-left-color: var(--info);
	}

	.icon {
		flex-shrink: 0;
		line-height: 18px;
		font-size: 13px;
		color: var(--text-muted);
	}

	.tone-success .icon {
		color: var(--accent);
	}
	.tone-warn .icon {
		color: var(--warn);
	}
	.tone-danger .icon {
		color: var(--danger);
	}

	.body {
		flex: 1;
		min-width: 0;
	}

	.message {
		margin: 0;
		line-height: 18px;
	}

	.details {
		margin-top: var(--space-1);
	}

	.details summary {
		cursor: pointer;
		color: var(--text-muted);
		font-size: 11px;
	}

	.details pre {
		margin: var(--space-1) 0 0;
		max-height: 140px;
		overflow: auto;
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--text-muted);
		white-space: pre-wrap;
		word-break: break-word;
	}

	.action {
		flex-shrink: 0;
		align-self: center;
		border: 1px solid var(--accent);
		border-radius: var(--radius-control);
		background: color-mix(in srgb, var(--accent) 16%, transparent);
		color: var(--accent);
		font: inherit;
		font-weight: 600;
		font-size: 12px;
		padding: 2px var(--space-2);
		cursor: pointer;
		transition: background var(--motion-hover);
	}

	.action:hover {
		background: color-mix(in srgb, var(--accent) 28%, transparent);
	}

	.close {
		flex-shrink: 0;
		border: none;
		background: none;
		color: var(--text-faint);
		font-size: 12px;
		cursor: pointer;
		padding: 0 2px;
		line-height: 18px;
	}

	.close:hover {
		color: var(--text);
	}

	@keyframes toast-in {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.toast {
			animation: none;
		}
	}
</style>
