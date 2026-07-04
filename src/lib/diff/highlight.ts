/** highlight.js integration — ARCHITECTURE.md §6.2: "run highlight.js per full line was
 * inaccurate across multi-line constructs; use `highlight(text, {language})` on the joined hunk
 * then split — acceptable v1 compromise". A construct that spans a line boundary loses its
 * highlighting on the far side of the split (the tag just doesn't reopen); this is exactly the
 * documented trade-off, not a bug. */

import hljs from "highlight.js";

export function escapeHtml(s: string): string {
	return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** Returns one highlighted HTML string per input line, same order/length as `lines`. */
export function highlightLines(lines: readonly string[], language: string | undefined): string[] {
	if (!language || lines.length === 0) return lines.map(escapeHtml);
	try {
		const joined = lines.join("\n");
		const result = hljs.highlight(joined, { language, ignoreIllegals: true });
		const split = result.value.split("\n");
		// Defensive: a highlighter quirk could in principle change line count — fall back to
		// plain escaping rather than mis-align lines with the wrong HTML.
		return split.length === lines.length ? split : lines.map(escapeHtml);
	} catch {
		return lines.map(escapeHtml);
	}
}
