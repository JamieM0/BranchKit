/** Which PR-related view the right panel shows — DESIGN_SPEC.md §12. Mirrors the priority the
 * right panel already gives compare/commit-detail/working-directory: a selected PR (or the
 * Create-PR form) takes over the panel until dismissed or another selection (commit, compare)
 * displaces it. */

class PrPanelStore {
	selectedNumber: number | null = $state(null);
	creating = $state(false);

	selectPr(number: number) {
		this.creating = false;
		this.selectedNumber = number;
	}

	openCreate() {
		this.selectedNumber = null;
		this.creating = true;
	}

	close() {
		this.selectedNumber = null;
		this.creating = false;
	}
}

export const prPanel = new PrPanelStore();
