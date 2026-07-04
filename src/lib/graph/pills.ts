/** Branch/tag pill grouping — DESIGN_SPEC.md §4.4. Raw refs (one `RefInfo` per underlying git ref)
 * are grouped across the whole repo into the pills the graph draws: a local+remote pair at the
 * *same* commit shares ONE pill carrying both presence icons; when they diverge the pills split to
 * their respective commits (the local pill keeps the ahead/behind badge, the remote pill is cloud-
 * only). Remote-tracking refs with no local counterpart become remote-only pills whose double-click
 * creates a tracking branch. This grouping is pure so it can be unit-tested. */

import type { HeadInfo, RefInfo } from "$lib/types";

export interface Pill {
	/** Stable key for `{#each}`. */
	key: string;
	kind: "branch" | "tag";
	/** Display name (branch short name, or branch part of a remote ref). */
	name: string;
	/** Commit sha this pill sits on. */
	sha: string;
	/** ✓ — this branch is the checked-out HEAD. */
	isHead: boolean;
	/** 💻 — a local branch exists at this pill's commit. */
	local: boolean;
	/** ☁ — a remote-tracking branch is represented by this pill. */
	remote: boolean;
	/** Full remote short name (`origin/main`) for the ☁ tooltip / ops. */
	remoteName: string | null;
	ahead: number;
	behind: number;
	/** Both ahead and behind — the warn-tinted badge state (§4.4). */
	diverged: boolean;
	/** Local branch name for checkout/rename/delete/merge ops (`null` for remote-only pills). */
	localBranch: string | null;
	/** Remote-tracking short name for track+checkout (`null` when purely local). */
	remoteRef: string | null;
	/** No local counterpart — double-click creates a tracking branch + checks out (§15.1). */
	isRemoteOnly: boolean;
	/** The local branch's configured upstream, if any (drives the badge divergence query). */
	upstream: string | null;
}

function branchPart(remoteShort: string): string {
	const slash = remoteShort.indexOf("/");
	return slash === -1 ? remoteShort : remoteShort.slice(slash + 1);
}

/** Groups `refs` into pills keyed by the commit sha each sits on. `head` marks the checked-out
 * branch so it can win a pill slot in the overflow rules (§4.4). */
export function buildPills(refs: readonly RefInfo[], head: HeadInfo | null): Record<string, Pill[]> {
	const locals = refs.filter((r) => r.kind === "branch");
	const remotes = refs.filter((r) => r.kind === "remoteBranch");
	const tags = refs.filter((r) => r.kind === "tag");

	const remoteByShort = new Map<string, RefInfo>();
	for (const r of remotes) remoteByShort.set(r.shortName, r);
	const consumed = new Set<string>();

	const bySha: Record<string, Pill[]> = {};
	const push = (pill: Pill) => {
		(bySha[pill.sha] ??= []).push(pill);
	};

	const headBranch = head && !head.detached ? head.branch : null;

	for (const l of locals) {
		const pill: Pill = {
			key: `local:${l.shortName}`,
			kind: "branch",
			name: l.shortName,
			sha: l.sha,
			isHead: l.shortName === headBranch,
			local: true,
			remote: false,
			remoteName: l.upstream ?? null,
			ahead: l.ahead,
			behind: l.behind,
			diverged: l.ahead > 0 && l.behind > 0,
			localBranch: l.shortName,
			remoteRef: null,
			isRemoteOnly: false,
			upstream: l.upstream ?? null,
		};
		if (l.upstream) {
			const r = remoteByShort.get(l.upstream);
			if (r && r.sha === l.sha) {
				// Same commit → one shared pill carrying both 💻 and ☁.
				pill.remote = true;
				pill.remoteRef = r.shortName;
				consumed.add(r.shortName);
			}
			// Diverged (r at a different sha): leave the remote unconsumed so it renders as its own
			// cloud-only pill below; the local pill keeps the ahead/behind badge.
		}
		push(pill);
	}

	for (const r of remotes) {
		if (consumed.has(r.shortName)) continue;
		const tracking = locals.find((l) => l.upstream === r.shortName);
		push({
			key: `remote:${r.shortName}`,
			kind: "branch",
			name: branchPart(r.shortName),
			sha: r.sha,
			isHead: false,
			local: false,
			remote: true,
			remoteName: r.shortName,
			ahead: 0,
			behind: 0,
			diverged: false,
			// A diverged remote whose local exists: double-click checks out that local. A truly
			// remote-only branch: double-click tracks + checks out.
			localBranch: tracking ? tracking.shortName : null,
			remoteRef: r.shortName,
			isRemoteOnly: !tracking,
			upstream: null,
		});
	}

	for (const t of tags) {
		push({
			key: `tag:${t.shortName}`,
			kind: "tag",
			name: t.shortName,
			sha: t.sha,
			isHead: false,
			local: false,
			remote: false,
			remoteName: null,
			ahead: 0,
			behind: 0,
			diverged: false,
			localBranch: null,
			remoteRef: null,
			isRemoteOnly: false,
			upstream: null,
		});
	}

	// The checked-out branch always wins a slot — sort it first within its commit (§4.4).
	for (const sha of Object.keys(bySha)) {
		bySha[sha].sort((a, b) => Number(b.isHead) - Number(a.isHead));
	}
	return bySha;
}
