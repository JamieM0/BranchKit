/** The universal text filter — DESIGN_SPEC.md §5 / §15.24. The left
 * panel's one filter box feeds `query`; the graph reads it to *dim* (never remove) non-matching
 * rows, and each panel section filters its rows by the same query (fuzzy). Branch visibility lives
 * in the graph store because it changes the backend topology query and lane assignment. */

class FilterStore {
	query = $state("");
	set(query: string) {
		this.query = query;
	}

	clear() {
		this.query = "";
	}

	get active(): boolean {
		return this.query.trim().length > 0;
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
