import { describe, expect, it } from "vitest";
import { buildPills } from "./pills";
import type { HeadInfo, RefInfo } from "$lib/types";

function localRef(partial: Partial<RefInfo> & { shortName: string; sha: string }): RefInfo {
	return {
		name: `refs/heads/${partial.shortName}`,
		kind: "branch",
		upstream: null,
		ahead: 0,
		behind: 0,
		gone: false,
		isHead: false,
		...partial,
	};
}

function remoteRef(partial: Partial<RefInfo> & { shortName: string; sha: string }): RefInfo {
	return {
		name: `refs/remotes/${partial.shortName}`,
		kind: "remoteBranch",
		upstream: null,
		ahead: 0,
		behind: 0,
		gone: false,
		isHead: false,
		...partial,
	};
}

const head = (branch: string): HeadInfo => ({ detached: false, branch, sha: "" });

describe("buildPills", () => {
	it("shares one pill when local and remote sit on the same commit", () => {
		const refs = [
			localRef({ shortName: "main", sha: "aaa", upstream: "origin/main" }),
			remoteRef({ shortName: "origin/main", sha: "aaa" }),
		];
		const pills = buildPills(refs, head("main"));
		expect(pills["aaa"]).toHaveLength(1);
		const pill = pills["aaa"][0];
		expect(pill.local).toBe(true);
		expect(pill.remote).toBe(true);
		expect(pill.isHead).toBe(true);
		expect(pill.remoteName).toBe("origin/main");
	});

	it("splits into two pills when local and remote diverge", () => {
		const refs = [
			localRef({ shortName: "main", sha: "local1", upstream: "origin/main", ahead: 2, behind: 1 }),
			remoteRef({ shortName: "origin/main", sha: "remote1" }),
		];
		const pills = buildPills(refs, head("main"));
		// Local pill keeps the ahead/behind badge; remote pill is cloud-only at the remote commit.
		const localPill = pills["local1"][0];
		expect(localPill.local).toBe(true);
		expect(localPill.remote).toBe(false);
		expect(localPill.ahead).toBe(2);
		expect(localPill.behind).toBe(1);
		expect(localPill.diverged).toBe(true);

		const remotePill = pills["remote1"][0];
		expect(remotePill.remote).toBe(true);
		expect(remotePill.local).toBe(false);
		expect(remotePill.name).toBe("main");
		expect(remotePill.isRemoteOnly).toBe(false); // a local tracks it → double-click checks out local
		expect(remotePill.localBranch).toBe("main");
	});

	it("renders a remote-only branch as a track+checkout pill", () => {
		const refs = [remoteRef({ shortName: "origin/feature/x", sha: "bbb" })];
		const pills = buildPills(refs, head("main"));
		const pill = pills["bbb"][0];
		expect(pill.remote).toBe(true);
		expect(pill.local).toBe(false);
		expect(pill.isRemoteOnly).toBe(true);
		expect(pill.name).toBe("feature/x");
		expect(pill.remoteRef).toBe("origin/feature/x");
	});

	it("marks tags and sorts the checked-out branch first on a shared commit", () => {
		const refs = [
			localRef({ shortName: "feature", sha: "ccc" }),
			localRef({ shortName: "main", sha: "ccc", isHead: true }),
			{
				name: "refs/tags/v1",
				shortName: "v1",
				kind: "tag" as const,
				sha: "ccc",
				upstream: null,
				ahead: 0,
				behind: 0,
				gone: false,
				isHead: false,
			},
		];
		const pills = buildPills(refs, head("main"));
		expect(pills["ccc"][0].name).toBe("main");
		expect(pills["ccc"][0].isHead).toBe(true);
		expect(pills["ccc"].some((p) => p.kind === "tag" && p.name === "v1")).toBe(true);
	});
});
