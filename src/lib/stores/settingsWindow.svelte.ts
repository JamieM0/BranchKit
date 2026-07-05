/** Settings window open/close + active section — DESIGN_SPEC.md §13 (Cmd+,). */

export type SettingsSection = "general" | "appearance" | "git" | "credentials" | "ai" | "integrations";

class SettingsWindowStore {
	open = $state(false);
	section: SettingsSection = $state("general");

	show(section?: SettingsSection) {
		if (section) this.section = section;
		this.open = true;
	}

	dismiss() {
		this.open = false;
	}
}

export const settingsWindow = new SettingsWindowStore();
