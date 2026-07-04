/** Status glyph → color/char mapping — DESIGN_SPEC.md §2.4. `copied` and `typeChanged` aren't
 * named in §2.4's table; they fold into the closest visual sibling (renamed / modified) since
 * they're rare in practice and git already reports copies as a rename-shaped record. */

import type { FileStatusCode } from "$lib/types";

export interface StatusGlyph {
	char: string;
	colorVar: string;
	/** Untracked files render the `+` hollow rather than filled (§2.4). */
	hollow: boolean;
	label: string;
}

const GLYPHS: Record<FileStatusCode, StatusGlyph> = {
	added: { char: "＋", colorVar: "--status-added", hollow: false, label: "Added" },
	modified: { char: "✎", colorVar: "--status-modified", hollow: false, label: "Modified" },
	deleted: { char: "−", colorVar: "--status-deleted", hollow: false, label: "Deleted" },
	renamed: { char: "→", colorVar: "--status-renamed", hollow: false, label: "Renamed" },
	copied: { char: "→", colorVar: "--status-renamed", hollow: false, label: "Copied" },
	typeChanged: { char: "✎", colorVar: "--status-modified", hollow: false, label: "Type changed" },
	updatedButUnmerged: {
		char: "‼",
		colorVar: "--status-conflicted",
		hollow: false,
		label: "Conflicted",
	},
	untracked: { char: "＋", colorVar: "--status-untracked", hollow: true, label: "Untracked" },
	unmodified: { char: "", colorVar: "--text-faint", hollow: false, label: "" },
	ignored: { char: "", colorVar: "--text-faint", hollow: false, label: "Ignored" },
};

export function statusGlyph(status: FileStatusCode): StatusGlyph {
	return GLYPHS[status];
}
