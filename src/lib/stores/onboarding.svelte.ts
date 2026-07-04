/** First-launch flow state — DESIGN_SPEC.md §11: theme choice, then a git identity check, then
 * open/clone. "Skippable" and remembered locally so it never runs twice on the same machine. */

const STORAGE_KEY = "branchkit:onboarded";

class OnboardingStore {
	done: boolean = $state(
		typeof localStorage !== "undefined" && localStorage.getItem(STORAGE_KEY) === "1",
	);

	finish() {
		this.done = true;
		if (typeof localStorage !== "undefined") localStorage.setItem(STORAGE_KEY, "1");
	}
}

export const onboarding = new OnboardingStore();
