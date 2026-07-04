/** The graph's selection state — the "right panel placeholder event" and compare-selection sink for
 * this prompt. DESIGN_SPEC.md §4.3: a single click selects a commit; Cmd/Ctrl+click a second commit
 * enters compare mode (`Comparing A ↔ B`). The right-panel UI that consumes these lands in prompt 7;
 * for now the graph reads `selectedSha`/`compare` back to drive its own highlight. */

export interface CompareSelection {
	a: string;
	b: string;
}

class GraphSelectionStore {
	selectedSha: string | null = $state(null);
	compare: CompareSelection | null = $state(null);

	/** Plain single-select — clears any active comparison. */
	select(sha: string) {
		this.selectedSha = sha;
		this.compare = null;
	}

	/** Cmd/Ctrl+click: pair the current selection with `sha` as compare endpoints. With nothing (or
	 * the same commit) selected it degrades to a plain select. */
	toggleCompare(sha: string) {
		if (!this.selectedSha || this.selectedSha === sha) {
			this.select(sha);
			return;
		}
		this.compare = { a: this.selectedSha, b: sha };
		this.selectedSha = sha;
	}

	/** Swap compare direction (the `Comparing A ↔ B` swap button, DESIGN_SPEC.md §4.3). */
	swapCompare() {
		if (this.compare) this.compare = { a: this.compare.b, b: this.compare.a };
	}

	clear() {
		this.selectedSha = null;
		this.compare = null;
	}
}

export const graphSelection = new GraphSelectionStore();
