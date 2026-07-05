/** Which file diff currently replaces the graph center pane — DESIGN_SPEC.md §6.2 "Diff viewer
 * (replaces graph center; breadcrumb '← Graph / path/to/file')". Clicking a file row anywhere
 * (working-directory sections, commit-detail's changed-file list, compare mode's file list)
 * opens it here; the breadcrumb's back arrow (or the WIP-row/commit-row click that already
 * happens in the graph) clears it back to the graph. */

export type DiffSource =
	| { kind: "workingTree" }
	| { kind: "staged" }
	| { kind: "commit"; sha: string }
	| { kind: "compare"; a: string; b: string }
	| { kind: "commitVsWorking"; sha: string };

export interface DiffTarget {
	path: string;
	origPath: string | null;
	source: DiffSource;
}

class DiffViewStore {
	target: DiffTarget | null = $state(null);

	open(target: DiffTarget) {
		this.target = target;
	}

	close() {
		this.target = null;
	}
}

export const diffView = new DiffViewStore();
