/** Ephemeral right-panel state for a generated commit explanation. It is deliberately not cached:
 * each context-menu invocation asks the configured model once for a fresh explanation. */
class CommitExplanationStore {
	sha: string | null = $state(null);
	repoId: string | null = $state(null);

	open(repoId: string, sha: string) {
		this.repoId = repoId;
		this.sha = sha;
	}

	close() {
		this.sha = null;
		this.repoId = null;
	}
}

export const commitExplanation = new CommitExplanationStore();
