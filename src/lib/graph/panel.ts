/** Left-panel section model — DESIGN_SPEC.md §5 / §15.26. Turns the flat ref list into LOCAL /
 * REMOTES (per remote) / TAGS rows, honoring "Combine tracking branches": with it ON a remote that
 * a local already tracks is merged into that local's row (☁ + ahead/behind) rather than duplicated
 * under REMOTES; with it OFF, GitKraken-style, the remote also appears in its remote section. Pure
 * so the combine behavior is unit-tested. */

import type { RefInfo } from "$lib/types";

export interface LocalRow {
	local: RefInfo;
	/** The remote-tracking ref this local tracks, when it exists (drives the ☁ presence). */
	tracked: RefInfo | null;
}

export interface RemoteGroup {
	/** Remote name, e.g. `origin`. */
	name: string;
	branches: RefInfo[];
}

export interface PanelModel {
	locals: LocalRow[];
	remotes: RemoteGroup[];
	tags: RefInfo[];
}

function remoteName(shortName: string): string {
	const slash = shortName.indexOf("/");
	return slash === -1 ? shortName : shortName.slice(0, slash);
}

export function buildPanelModel(refs: readonly RefInfo[], combineTracking: boolean): PanelModel {
	const localRefs = refs.filter((r) => r.kind === "branch");
	const remoteRefs = refs.filter((r) => r.kind === "remoteBranch");
	const tags = refs.filter((r) => r.kind === "tag").slice().sort((a, b) => a.shortName.localeCompare(b.shortName));

	const remoteByShort = new Map<string, RefInfo>();
	for (const r of remoteRefs) remoteByShort.set(r.shortName, r);

	const locals: LocalRow[] = localRefs
		.slice()
		.sort((a, b) => a.shortName.localeCompare(b.shortName))
		.map((local) => ({
			local,
			tracked: local.upstream ? (remoteByShort.get(local.upstream) ?? null) : null,
		}));

	// When combining, remotes that a local tracks are represented by that local's row.
	const trackedShortNames = new Set(
		combineTracking
			? locals.filter((l) => l.tracked).map((l) => l.tracked!.shortName)
			: [],
	);

	const groups = new Map<string, RefInfo[]>();
	for (const r of remoteRefs) {
		if (trackedShortNames.has(r.shortName)) continue;
		const name = remoteName(r.shortName);
		(groups.get(name) ?? groups.set(name, []).get(name)!).push(r);
	}
	const remotes: RemoteGroup[] = [...groups.entries()]
		.sort((a, b) => a[0].localeCompare(b[0]))
		.map(([name, branches]) => ({
			name,
			branches: branches.slice().sort((a, b) => a.shortName.localeCompare(b.shortName)),
		}));

	return { locals, remotes, tags };
}
