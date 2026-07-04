/** The universal filter + hidden-branch state — DESIGN_SPEC.md §5 / §15.24 / §15.26. The left
 * panel's one filter box feeds `query`; the graph reads it to *dim* (never remove) non-matching
 * rows, and each panel section filters its rows by the same query (fuzzy). The hidden set backs the
 * per-row hide-eye toggle.
 *
 * SPEC-DEVIATION: "hide from graph" here hides the branch's *pill* rather than recomputing topology
 * to drop the branch's exclusive commits — the latter needs a backend ref-exclusion pass that lands
 * with the toolbar/graph-scale prompt. The eye still removes the branch's visual presence in the
 * graph, which is the day-to-day intent. */

import { SvelteSet } from "svelte/reactivity";

class FilterStore {
	query = $state("");
	/** Local branch short names hidden from the graph. */
	hidden = new SvelteSet<string>();

	set(query: string) {
		this.query = query;
	}

	clear() {
		this.query = "";
	}

	get active(): boolean {
		return this.query.trim().length > 0;
	}

	isHidden(name: string): boolean {
		return this.hidden.has(name);
	}

	toggleHidden(name: string) {
		if (this.hidden.has(name)) this.hidden.delete(name);
		else this.hidden.add(name);
	}
}

export const filter = new FilterStore();

/** Case-insensitive substring test used to dim graph rows against the filter query (§15.24). A row
 * matches on its subject, author, sha prefix, or any ref/pill name. */
export function rowMatchesQuery(
	query: string,
	fields: { subject: string; author: string; sha: string; refNames: string[] },
): boolean {
	const q = query.trim().toLowerCase();
	if (q === "") return true;
	if (fields.subject.toLowerCase().includes(q)) return true;
	if (fields.author.toLowerCase().includes(q)) return true;
	if (fields.sha.toLowerCase().startsWith(q)) return true;
	return fields.refNames.some((n) => n.toLowerCase().includes(q));
}
