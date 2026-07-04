import { describe, expect, it } from "vitest";
import { buildPanelModel } from "./panel";
import type { RefInfo } from "$lib/types";

function ref(partial: Partial<RefInfo> & { shortName: string; kind: RefInfo["kind"] }): RefInfo {
	return {
		name: partial.shortName,
		sha: "x",
		upstream: null,
		ahead: 0,
		behind: 0,
		gone: false,
		isHead: false,
		...partial,
	};
}

describe("buildPanelModel", () => {
	const refs = [
		ref({ shortName: "main", kind: "branch", upstream: "origin/main" }),
		ref({ shortName: "feature", kind: "branch" }),
		ref({ shortName: "origin/main", kind: "remoteBranch" }),
		ref({ shortName: "origin/other", kind: "remoteBranch" }),
		ref({ shortName: "v1", kind: "tag" }),
	];

	it("merges tracked remotes into their local row when combining", () => {
		const model = buildPanelModel(refs, true);
		const main = model.locals.find((l) => l.local.shortName === "main")!;
		expect(main.tracked?.shortName).toBe("origin/main");
		// origin/main is combined away; only origin/other remains under REMOTES.
		const origin = model.remotes.find((g) => g.name === "origin")!;
		expect(origin.branches.map((b) => b.shortName)).toEqual(["origin/other"]);
		expect(model.tags.map((t) => t.shortName)).toEqual(["v1"]);
	});

	it("duplicates tracked remotes under REMOTES when not combining", () => {
		const model = buildPanelModel(refs, false);
		const origin = model.remotes.find((g) => g.name === "origin")!;
		expect(origin.branches.map((b) => b.shortName)).toEqual(["origin/main", "origin/other"]);
	});
});
