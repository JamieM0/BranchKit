export type ThemeSetting = "dark" | "light" | "system";
export type ResolvedTheme = "dark" | "light";

function systemTheme(): ResolvedTheme {
  if (typeof matchMedia === "undefined") return "dark";
  return matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

class ThemeStore {
  setting: ThemeSetting = $state("system");
  system: ResolvedTheme = $state(systemTheme());

  resolved: ResolvedTheme = $derived(this.setting === "system" ? this.system : this.setting);

  constructor() {
    if (typeof matchMedia !== "undefined") {
      matchMedia("(prefers-color-scheme: light)").addEventListener("change", (e) => {
        this.system = e.matches ? "light" : "dark";
      });
    }
  }

  /** Project the canonical backend setting into reactive render state. */
  hydrate(setting: ThemeSetting) {
    this.setting = setting;
  }
}

export const theme = new ThemeStore();
