/** Global keyboard map lives in DESIGN_SPEC.md §10 — Cmd on macOS, Ctrl elsewhere. */

export function isMac(): boolean {
	if (typeof navigator === "undefined") return false;
	return navigator.platform.toUpperCase().includes("MAC");
}

/** The primary modifier for global shortcuts: Cmd on macOS, Ctrl elsewhere. Accepts keyboard and
 * mouse events (Cmd/Ctrl+click compare selection — DESIGN_SPEC.md §4.3). */
export function isModEvent(e: KeyboardEvent | MouseEvent): boolean {
	return isMac() ? e.metaKey : e.ctrlKey;
}
