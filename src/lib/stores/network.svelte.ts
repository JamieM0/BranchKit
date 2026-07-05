/** Global offline indicator — ARCHITECTURE.md §9 "Could not resolve host → offline (also flip a
 * global offline indicator; retry fetches on focus)". Flipped on by `actions.ts` whenever a
 * network op's translated error is the "retry-offline" suggestion; flipped off by any op that
 * subsequently succeeds. `retryOnFocus` re-fetches the given repo once when the window regains
 * focus while offline — ARCHITECTURE.md §14 macOS app-nap note pairs with this same focus signal. */

class NetworkStore {
	offline = $state(false);

	markOffline() {
		this.offline = true;
	}

	markOnline() {
		this.offline = false;
	}
}

export const network = new NetworkStore();

/** Wires a single `window` focus listener that retries `onRetry` once per regained focus while
 * `network.offline` is true. Returns the cleanup function. Kept as a plain function (rather than
 * baked into the store) since it needs a live repo id to retry against, which only the shell
 * component knows. */
export function retryOnFocus(onRetry: () => void): () => void {
	function handleFocus() {
		if (network.offline) onRetry();
	}
	window.addEventListener("focus", handleFocus);
	return () => window.removeEventListener("focus", handleFocus);
}
