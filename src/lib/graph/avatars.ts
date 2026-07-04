/** Author avatars for graph nodes — DESIGN_SPEC.md §4.3 / ARCHITECTURE.md §5.4: a Gravatar with a
 * colored-initials-disc fallback, decoded to an `ImageBitmap` and held in a small LRU cache so the
 * canvas can `drawImage` clipped circles without re-decoding on every scroll frame. */

import { GRAPH_LANE_PALETTE_SIZE } from "./lanes";

/** Up-to-two initials for the fallback disc, derived from the author's display name (falling back
 * to the local-part of the email). */
export function authorInitials(name: string, email: string): string {
	const source = name.trim() || email.split("@")[0]?.trim() || "";
	const parts = source.split(/[\s._\-]+/u).filter(Boolean);
	if (parts.length === 0) return "?";
	if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
	return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}

/** Deterministic lane-palette index for the fallback disc's background, hashed off the email so a
 * given author always gets the same color. */
export function discColorIndex(email: string): number {
	let hash = 0;
	const key = email.trim().toLowerCase();
	for (let i = 0; i < key.length; i += 1) {
		hash = (hash * 31 + key.charCodeAt(i)) | 0;
	}
	return Math.abs(hash) % GRAPH_LANE_PALETTE_SIZE;
}

/** SHA-256 hex of the normalized email — Gravatar's current hashing scheme (it accepts SHA-256 in
 * place of the legacy MD5, which the Web Crypto API can't produce). */
export async function gravatarHash(email: string): Promise<string> {
	const normalized = email.trim().toLowerCase();
	const bytes = new TextEncoder().encode(normalized);
	const digest = await crypto.subtle.digest("SHA-256", bytes);
	return Array.from(new Uint8Array(digest))
		.map((b) => b.toString(16).padStart(2, "0"))
		.join("");
}

/** Gravatar URL that 404s (rather than returning a generic image) when the author has no avatar, so
 * a failed load cleanly signals "use the initials disc". */
export function gravatarUrl(hash: string, size: number): string {
	return `https://www.gravatar.com/avatar/${hash}?s=${size}&d=404`;
}

type CacheState = ImageBitmap | "pending" | "failed";

/**
 * LRU cache of decoded avatar bitmaps keyed by email. `get` is synchronous and non-blocking: it
 * returns a ready bitmap or `null`, kicking off a background fetch+decode on first sight and
 * invoking `onLoad` (a canvas redraw request) when one arrives. Capacity per ARCHITECTURE.md §13.
 */
export class AvatarCache {
	#capacity: number;
	#onLoad: () => void;
	#bitmaps = new Map<string, CacheState>();
	#pixelSize: number;

	constructor(onLoad: () => void, capacity = 200, pixelSize = 48) {
		this.#onLoad = onLoad;
		this.#capacity = capacity;
		this.#pixelSize = pixelSize;
	}

	/** A ready bitmap for `email`, or `null` (draw the fallback disc). Triggers a fetch on first use. */
	get(email: string): ImageBitmap | null {
		const key = email.trim().toLowerCase();
		if (!key) return null;
		const existing = this.#bitmaps.get(key);
		if (existing instanceof ImageBitmap) {
			// Refresh recency.
			this.#bitmaps.delete(key);
			this.#bitmaps.set(key, existing);
			return existing;
		}
		if (existing === undefined) {
			this.#bitmaps.set(key, "pending");
			void this.#load(key);
		}
		return null;
	}

	async #load(key: string): Promise<void> {
		try {
			if (typeof fetch !== "function" || typeof createImageBitmap !== "function") {
				this.#bitmaps.set(key, "failed");
				return;
			}
			const hash = await gravatarHash(key);
			const response = await fetch(gravatarUrl(hash, this.#pixelSize));
			if (!response.ok) {
				this.#bitmaps.set(key, "failed");
				return;
			}
			const blob = await response.blob();
			const bitmap = await createImageBitmap(blob);
			this.#bitmaps.set(key, bitmap);
			this.#evict();
			this.#onLoad();
		} catch {
			this.#bitmaps.set(key, "failed");
		}
	}

	#evict(): void {
		while (this.#bitmaps.size > this.#capacity) {
			const oldest = this.#bitmaps.keys().next().value;
			if (oldest === undefined) break;
			const state = this.#bitmaps.get(oldest);
			if (state instanceof ImageBitmap) state.close();
			this.#bitmaps.delete(oldest);
		}
	}

	/** Release every decoded bitmap — called when the graph unmounts. */
	dispose(): void {
		for (const state of this.#bitmaps.values()) {
			if (state instanceof ImageBitmap) state.close();
		}
		this.#bitmaps.clear();
	}
}
