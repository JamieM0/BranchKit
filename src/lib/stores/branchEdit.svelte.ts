/** Inline branch name editor state — DESIGN_SPEC.md §5 / GITKRAKEN_WORKFLOWS §2.3. Drives the
 * name input that appears in the graph's BRANCH/TAG column: creating a branch at a commit ("type
 * name inline in the graph's label column; Enter checks it out immediately") and renaming an
 * existing branch in place. Only ever one editor is active. */

export type BranchEditMode = "create" | "rename";

class BranchEditStore {
	mode: BranchEditMode | null = $state(null);
	/** The commit the editor is anchored to (create: start point; rename: the branch tip). */
	sha: string | null = $state(null);
	/** For rename, the existing branch name. */
	oldName: string | null = $state(null);
	/** Initial input value. */
	initial = $state("");

	startCreate(sha: string) {
		this.mode = "create";
		this.sha = sha;
		this.oldName = null;
		this.initial = "";
	}

	startRename(oldName: string, sha: string) {
		this.mode = "rename";
		this.sha = sha;
		this.oldName = oldName;
		this.initial = oldName;
	}

	cancel() {
		this.mode = null;
		this.sha = null;
		this.oldName = null;
		this.initial = "";
	}

	/** Is an editor anchored to this commit row? */
	isEditing(sha: string): boolean {
		return this.mode !== null && this.sha === sha;
	}
}

export const branchEdit = new BranchEditStore();
