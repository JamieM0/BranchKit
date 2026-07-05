/** The create-worktree dialog's open/closed state — DESIGN_SPEC.md §5 WORKTREES. Triggered from the
 * branch pill/panel-row menu ("Create worktree from this branch…") and the commit row menu
 * ("Create worktree from this commit…"), both of which just supply the ref to start from; the
 * dialog itself (rendered once, at the shell level, like `CloneDialog`) owns the path picker and
 * optional new-branch name. */

class WorktreeDialogStore {
	startRef: string | null = $state(null);

	open(startRef: string) {
		this.startRef = startRef;
	}

	close() {
		this.startRef = null;
	}
}

export const worktreeDialog = new WorktreeDialogStore();
