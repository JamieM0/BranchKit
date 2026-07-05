/** Shared relative-time formatting — DESIGN_SPEC.md §5/§13 "date style (relative/absolute)". */

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function relativeTime(unixSeconds: number, now: number = Date.now() / 1000): string {
	const diff = Math.max(0, Math.round(now - unixSeconds));
	if (diff < MINUTE) return "just now";
	if (diff < HOUR) return `${Math.round(diff / MINUTE)}m ago`;
	if (diff < DAY) return `${Math.round(diff / HOUR)}h ago`;
	return `${Math.round(diff / DAY)}d ago`;
}

/** The Settings → Appearance "date style: absolute" alternative (DESIGN_SPEC.md §5/§13). */
export function absoluteTime(unixSeconds: number): string {
	return new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
		year: "numeric",
		month: "short",
		day: "numeric",
	});
}

/** Picks between `relativeTime`/`absoluteTime` per the Settings → Appearance date-style setting —
 * the one call site every commit-date display should go through. */
export function formatCommitDate(unixSeconds: number, style: "relative" | "absolute"): string {
	return style === "absolute" ? absoluteTime(unixSeconds) : relativeTime(unixSeconds);
}
