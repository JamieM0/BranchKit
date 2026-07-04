/**
 * Small subsequence fuzzy matcher — DESIGN_SPEC.md §3.1 ("fuzzy search over recents + Open… +
 * Clone…") and §10 (command palette). Contiguous runs and word-boundary starts score higher so
 * "bk" beats a scattered match inside a long path.
 */

export interface FuzzyMatch {
	score: number;
	/** Indices into `target` that matched, for highlighting. */
	indices: number[];
}

export function fuzzyMatch(query: string, target: string): FuzzyMatch | null {
	if (query.length === 0) return { score: 0, indices: [] };

	const q = query.toLowerCase();
	const t = target.toLowerCase();
	const indices: number[] = [];
	let qi = 0;
	let score = 0;
	let prevMatchIndex = -2;

	for (let ti = 0; ti < t.length && qi < q.length; ti++) {
		if (t[ti] !== q[qi]) continue;
		let charScore = 1;
		if (ti === prevMatchIndex + 1) charScore += 3;
		if (ti === 0 || /[\s/_-]/.test(t[ti - 1])) charScore += 2;
		score += charScore;
		indices.push(ti);
		prevMatchIndex = ti;
		qi++;
	}

	if (qi < q.length) return null;
	// Tiny length penalty so ties favor the shorter (more specific) target.
	return { score: score - target.length * 0.01, indices };
}

export interface FuzzyResult<T> {
	item: T;
	indices: number[];
}

/** Filters and ranks `items` by `fuzzyMatch` against `getText(item)`. An empty query returns
 * every item, unranked, in its original order. */
export function fuzzyFilter<T>(
	query: string,
	items: T[],
	getText: (item: T) => string,
): FuzzyResult<T>[] {
	if (!query.trim()) return items.map((item) => ({ item, indices: [] }));

	const scored: (FuzzyResult<T> & { score: number })[] = [];
	for (const item of items) {
		const match = fuzzyMatch(query, getText(item));
		if (match) scored.push({ item, indices: match.indices, score: match.score });
	}
	scored.sort((a, b) => b.score - a.score);
	return scored.map(({ item, indices }) => ({ item, indices }));
}
