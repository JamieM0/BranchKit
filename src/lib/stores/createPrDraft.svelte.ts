/** The Create-PR panel's form state — DESIGN_SPEC.md §12: "base/head pickers (prefilled), title
 * (prefilled from branch name Title-Cased or single-commit summary), body (prefilled with commit
 * list)". */

function titleCase(branchName: string): string {
	const words = branchName
		.replace(/^(feature|feat|fix|bugfix|chore)\//, "")
		.split(/[/_-]+/)
		.filter(Boolean);
	return words.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ");
}

class CreatePrDraftStore {
	base = $state("main");
	head = $state("");
	title = $state("");
	body = $state("");

	/** Called right after a publish — DESIGN_SPEC.md §8/§22's "push new branch → Create pull
	 * request" toast. */
	prefillHead(head: string) {
		this.head = head;
		if (!this.title) this.title = titleCase(head);
	}

	prefillFromCommits(base: string, head: string, commitSubjects: string[]) {
		this.base = base;
		this.head = head;
		if (!this.title) {
			this.title = commitSubjects.length === 1 ? commitSubjects[0] : titleCase(head);
		}
		if (!this.body && commitSubjects.length > 1) {
			this.body = commitSubjects.map((s) => `- ${s}`).join("\n");
		}
	}

	reset() {
		this.base = "main";
		this.head = "";
		this.title = "";
		this.body = "";
	}
}

export const createPrDraft = new CreatePrDraftStore();
