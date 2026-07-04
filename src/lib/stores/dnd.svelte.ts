/** Pill drag-to-merge state — DESIGN_SPEC.md §4.4 / §15.6. Native HTML drag-and-drop can only
 * carry strings on the dataTransfer, so the dragged pill itself lives here; rows and pills read
 * `source` to decide whether they're a valid drop target and to glow while dragging. */

import type { Pill } from "$lib/graph/pills";

class DragStore {
	/** The pill currently being dragged, or `null`. */
	source: Pill | null = $state(null);
	/** Key of the drop target under the pointer (a sha or a pill key) — for the target glow. */
	overKey: string | null = $state(null);
	/** Last pointer position during the drag, so the drop menu can anchor where you let go. */
	x = 0;
	y = 0;

	start(pill: Pill) {
		this.source = pill;
		this.overKey = null;
	}

	setOver(key: string | null, x?: number, y?: number) {
		this.overKey = key;
		if (x !== undefined) this.x = x;
		if (y !== undefined) this.y = y;
	}

	end() {
		this.source = null;
		this.overKey = null;
	}

	get dragging(): boolean {
		return this.source !== null;
	}
}

export const dnd = new DragStore();
