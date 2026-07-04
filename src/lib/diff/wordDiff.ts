/** Intra-line word-level diff — ARCHITECTURE.md §6.2 "compute in TS between paired add/del lines
 * with a small LCS on word tokens". Tokenizes into word / non-word runs so whitespace and
 * punctuation are their own tokens, then runs the standard LCS backtrack to mark which tokens on
 * each side changed. */

export interface WordSegment {
	text: string;
	changed: boolean;
}

export function tokenize(line: string): string[] {
	return line.match(/\w+|[^\w]+/g) ?? [];
}

function coalesce(segments: WordSegment[]): WordSegment[] {
	const out: WordSegment[] = [];
	for (const seg of segments) {
		const last = out[out.length - 1];
		if (last && last.changed === seg.changed) {
			last.text += seg.text;
		} else {
			out.push({ ...seg });
		}
	}
	return out;
}

export interface WordDiffResult {
	oldSegments: WordSegment[];
	newSegments: WordSegment[];
}

/** LCS on word tokens between a paired del/add line. `O(n*m)` table, fine for single lines. */
export function wordDiff(oldLine: string, newLine: string): WordDiffResult {
	const a = tokenize(oldLine);
	const b = tokenize(newLine);
	const n = a.length;
	const m = b.length;

	const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
	for (let i = n - 1; i >= 0; i -= 1) {
		for (let j = m - 1; j >= 0; j -= 1) {
			dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
		}
	}

	const oldSegments: WordSegment[] = [];
	const newSegments: WordSegment[] = [];
	let i = 0;
	let j = 0;
	while (i < n && j < m) {
		if (a[i] === b[j]) {
			oldSegments.push({ text: a[i], changed: false });
			newSegments.push({ text: b[j], changed: false });
			i += 1;
			j += 1;
		} else if (dp[i + 1][j] >= dp[i][j + 1]) {
			oldSegments.push({ text: a[i], changed: true });
			i += 1;
		} else {
			newSegments.push({ text: b[j], changed: true });
			j += 1;
		}
	}
	while (i < n) {
		oldSegments.push({ text: a[i], changed: true });
		i += 1;
	}
	while (j < m) {
		newSegments.push({ text: b[j], changed: true });
		j += 1;
	}

	return { oldSegments: coalesce(oldSegments), newSegments: coalesce(newSegments) };
}
