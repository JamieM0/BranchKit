/** Cmd+K overlay open/closed state — DESIGN_SPEC.md §10. Kept as a tiny store (rather than local
 * component state) so any surface — the toolbar's ⌘K button, the global shortcut, a "no results"
 * empty-state link — can open it without prop-drilling. */

class CommandPaletteStore {
	open_ = $state(false);

	get isOpen() {
		return this.open_;
	}

	open() {
		this.open_ = true;
	}

	close() {
		this.open_ = false;
	}

	toggle() {
		this.open_ = !this.open_;
	}
}

export const commandPalette = new CommandPaletteStore();
