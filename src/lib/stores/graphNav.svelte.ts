/** Cross-component graph navigation signals — DESIGN_SPEC.md §4.4 / §5 / §15.25. Lets the left
 * panel and pills ask the graph to scroll to a commit (click → scroll-to-tip) and to glow a
 * branch's tip + pill on hover, without those components reaching into the graph's private scroll
 * state. The graph subscribes and reacts. */

class GraphNav {
	/** Bumped on every scroll request so the graph's `$effect` re-fires even for the same sha. */
	scrollToken = $state(0);
	scrollSha: string | null = $state(null);
	/** Tip commit sha to glow (hover a branch row) — `null` clears. */
	glowSha: string | null = $state(null);

	/** Flash-scroll the graph so `sha` is in view (§4.4 "flash-scroll if off-screen"). */
	scrollTo(sha: string) {
		this.scrollSha = sha;
		this.scrollToken += 1;
	}

	setGlow(sha: string | null) {
		this.glowSha = sha;
	}
}

export const graphNav = new GraphNav();
