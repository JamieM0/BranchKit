export type ThemeSetting = "dark" | "light" | "system";
export type ResolvedTheme = "dark" | "light";

const STORAGE_KEY = "branchkit:theme";

function systemTheme(): ResolvedTheme {
  if (typeof matchMedia === "undefined") return "dark";
  return matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

class ThemeStore {
  setting: ThemeSetting = $state("dark");
  system: ResolvedTheme = $state(systemTheme());

  resolved: ResolvedTheme = $derived(this.setting === "system" ? this.system : this.setting);

  constructor() {
    if (typeof localStorage !== "undefined") {
      const stored = localStorage.getItem(STORAGE_KEY) as ThemeSetting | null;
      if (stored === "dark" || stored === "light" || stored === "system") {
        this.setting = stored;
      }
    }
    if (typeof matchMedia !== "undefined") {
      matchMedia("(prefers-color-scheme: light)").addEventListener("change", (e) => {
        this.system = e.matches ? "light" : "dark";
      });
    }
  }

  set(setting: ThemeSetting) {
    this.setting = setting;
    if (typeof localStorage !== "undefined") localStorage.setItem(STORAGE_KEY, setting);
  }
}

export const theme = new ThemeStore();
