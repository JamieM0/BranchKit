/** Pairs consecutive del/add runs within a hunk so the diff viewer can word-diff matched pairs —
 * ARCHITECTURE.md §6.2. A run of `n` dels immediately followed by a run of `m` adds pairs up the
 * first `min(n, m)` index-wise (the common "line replaced" shape); any excess dels or adds render
 * as plain (unpaired) lines. */

import type { DiffLine } from "$lib/types";

export interface LinePair {
	del: DiffLine | null;
	add: DiffLine | null;
}

export type RenderLine = DiffLine | LinePair;

export function isLinePair(line: RenderLine): line is LinePair {
	return !("kind" in line);
}

export function pairChangedLines(lines: readonly DiffLine[]): RenderLine[] {
	const out: RenderLine[] = [];
	let i = 0;
	while (i < lines.length) {
		const line = lines[i];
		if (line.kind === "context") {
			out.push(line);
			i += 1;
			continue;
		}

		const delRun: DiffLine[] = [];
		while (i < lines.length && lines[i].kind === "del") {
			delRun.push(lines[i]);
			i += 1;
		}
		const addRun: DiffLine[] = [];
		while (i < lines.length && lines[i].kind === "add") {
			addRun.push(lines[i]);
			i += 1;
		}

		const pairCount = Math.min(delRun.length, addRun.length);
		for (let k = 0; k < pairCount; k += 1) {
			out.push({ del: delRun[k], add: addRun[k] });
		}
		for (let k = pairCount; k < delRun.length; k += 1) out.push(delRun[k]);
		for (let k = pairCount; k < addRun.length; k += 1) out.push(addRun[k]);
	}
	return out;
}
