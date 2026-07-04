/** Toasts & undo affordances — DESIGN_SPEC.md §8. Bottom-left stack, max 3, 6s timeout (10s for
 * destructive ones), hover pauses the countdown. Every toast has an icon, one sentence, and at
 * most ONE action verb. This replaces GitKraken's global Undo: the reversal lives in the toast
 * (Back / Undo / Resolve / View …). */

export type ToastTone = "info" | "success" | "warn" | "danger";

export interface ToastAction {
	/** A single verb — "Back", "Undo", "View", "Pull". */
	label: string;
	run: () => void | Promise<void>;
}

export interface ToastInput {
	message: string;
	tone?: ToastTone;
	/** Lucide-ish glyph; kept as a short string so the component can map/render it. */
	icon?: string;
	action?: ToastAction;
	/** Destructive toasts linger 10s instead of 6s (§8). */
	destructive?: boolean;
	/** Raw git output for the failure "Details" expander (§8/§11). */
	details?: string;
}

export interface Toast extends ToastInput {
	id: number;
	createdAt: number;
}

const MAX_TOASTS = 3;
const DEFAULT_TIMEOUT = 6000;
const DESTRUCTIVE_TIMEOUT = 10000;

class ToastStore {
	items: Toast[] = $state([]);

	#nextId = 1;
	#timers = new Map<number, ReturnType<typeof setTimeout>>();

	timeoutFor(toast: Toast): number {
		return toast.destructive ? DESTRUCTIVE_TIMEOUT : DEFAULT_TIMEOUT;
	}

	push(input: ToastInput): number {
		const id = this.#nextId++;
		const toast: Toast = { id, createdAt: Date.now(), tone: "info", ...input };
		// Newest at the top of the stack; cap at 3 (evict the oldest).
		this.items = [toast, ...this.items].slice(0, MAX_TOASTS);
		this.#arm(id);
		return id;
	}

	/** A failure toast (§8/§11) — danger tone, Details expander, 10s. */
	pushError(message: string, details?: string, action?: ToastAction): number {
		return this.push({ message, tone: "danger", icon: "alert", details, action, destructive: true });
	}

	#arm(id: number) {
		this.#clearTimer(id);
		const toast = this.items.find((t) => t.id === id);
		if (!toast) return;
		this.#timers.set(
			id,
			setTimeout(() => this.dismiss(id), this.timeoutFor(toast)),
		);
	}

	#clearTimer(id: number) {
		const timer = this.#timers.get(id);
		if (timer !== undefined) {
			clearTimeout(timer);
			this.#timers.delete(id);
		}
	}

	/** Hover pauses the auto-dismiss countdown (§8). */
	pause(id: number) {
		this.#clearTimer(id);
	}

	/** Pointer left — restart the full countdown. */
	resume(id: number) {
		this.#arm(id);
	}

	dismiss(id: number) {
		this.#clearTimer(id);
		this.items = this.items.filter((t) => t.id !== id);
	}

	/** Fire a toast's action, then dismiss it — an action verb is a one-shot. */
	async runAction(id: number) {
		const toast = this.items.find((t) => t.id === id);
		if (!toast?.action) return;
		this.dismiss(id);
		await toast.action.run();
	}

	clear() {
		for (const id of this.#timers.keys()) this.#clearTimer(id);
		this.items = [];
	}
}

export const toasts = new ToastStore();
