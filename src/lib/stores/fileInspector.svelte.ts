/** Which file's History/Blame view currently replaces the graph center pane — DESIGN_SPEC.md §6.3.
 * Entered from the diff viewer's [File History]/[Blame] buttons and file row context menus;
 * mutually exclusive with `diffView` (opening one closes the other, mirroring how `diffView`
 * itself displaces the graph). */

export type FileInspectorMode = "history" | "blame";

export interface FileInspectorTarget {
	path: string;
	mode: FileInspectorMode;
}

class FileInspectorStore {
	target: FileInspectorTarget | null = $state(null);

	open(path: string, mode: FileInspectorMode = "history") {
		this.target = { path, mode };
	}

	setMode(mode: FileInspectorMode) {
		if (this.target) this.target = { ...this.target, mode };
	}

	close() {
		this.target = null;
	}
}

export const fileInspector = new FileInspectorStore();
