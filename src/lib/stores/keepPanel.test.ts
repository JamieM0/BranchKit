import { beforeEach, describe, expect, it } from "vitest";
import type { FileRegion } from "$lib/types";
import { KeepPanelStore, operationProgress } from "./keepPanel.svelte";

function regions(): FileRegion[] {
	return [
		{ kind: "context", lines: ["line1"] },
		{
			kind: "conflict",
			baseStart: 1,
			baseEnd: 2,
			sameBothPrefix: [],
			oursLines: ["OURS"],
			theirsLines: ["THEIRS"],
			sameBothSuffix: [],
		},
		{ kind: "context", lines: ["line3"] },
	];
}

let store: KeepPanelStore;

beforeEach(() => {
	store = new KeepPanelStore();
	store.open("f.txt", regions(), "main", "feature/x");
});

describe("keep/unkeep", () => {
	it("keeping a block moves it into resolvedText and marks the region touched", () => {
		expect(store.fileProgress).toEqual({ resolved: 0, total: 1 });
		store.keepBlock(1, "ours");
		expect(store.resolvedText).toBe("line1\nOURS\nline3\n");
		expect(store.fileProgress).toEqual({ resolved: 1, total: 1 });
		expect(store.allResolved).toBe(true);
	});

	it("keeping a single line only carries that line", () => {
		const withMultiline = regions();
		(withMultiline[1] as Extract<FileRegion, { kind: "conflict" }>).oursLines = ["a", "b"];
		store.open("f.txt", withMultiline, "main", "feature/x");
		store.keepLine(1, "ours", 1);
		expect(store.resolvedText).toBe("line1\nb\nline3\n");
	});

	it("unkeep sends a kept block back to the candidate pool", () => {
		store.keepBlock(1, "ours");
		const keptId = (store.regions[1] as { kept: { id: string }[] }).kept[0].id;
		store.unkeep(1, keptId);
		expect(store.resolvedText).toBe("line1\nline3\n");
		// still touched — unkeeping is itself a decision, not a reset.
		expect(store.fileProgress).toEqual({ resolved: 1, total: 1 });
	});
});

describe("keep-both click ordering", () => {
	it("stacks both sides in the order they were clicked", () => {
		store.keepBlock(1, "theirs");
		store.keepBlock(1, "ours");
		expect(store.resolvedText).toBe("line1\nTHEIRS\nOURS\nline3\n");
	});

	it("the reverse click order produces the reverse stack", () => {
		store.keepBlock(1, "ours");
		store.keepBlock(1, "theirs");
		expect(store.resolvedText).toBe("line1\nOURS\nTHEIRS\nline3\n");
	});
});

describe("reorder", () => {
	it("moves a kept item up or down within its region's stack, renumbering the assembled file", () => {
		store.keepBlock(1, "ours");
		store.keepBlock(1, "theirs");
		const [first, second] = (store.regions[1] as { kept: { id: string }[] }).kept;
		expect(store.resolvedLines).toEqual(["line1", "OURS", "THEIRS", "line3"]);

		store.reorder(1, second.id, "up");
		expect(store.resolvedLines).toEqual(["line1", "THEIRS", "OURS", "line3"]);

		// moving the (now-first) item further up is a no-op at the boundary.
		store.reorder(1, second.id, "up");
		expect(store.resolvedLines).toEqual(["line1", "THEIRS", "OURS", "line3"]);

		store.reorder(1, second.id, "down");
		expect(store.resolvedLines).toEqual(["line1", "OURS", "THEIRS", "line3"]);
		expect(first.id).not.toBe(second.id);
	});
});

describe("edit region", () => {
	it("replaces the whole kept stack with the hand-edited text", () => {
		store.keepBlock(1, "ours");
		store.keepBlock(1, "theirs");
		store.editRegion(1, "hand written\nfix");
		expect(store.resolvedLines).toEqual(["line1", "hand written", "fix", "line3"]);
		const kept = (store.regions[1] as { kept: { source: string }[] }).kept;
		expect(kept).toHaveLength(1);
		expect(kept[0].source).toBe("edit");
	});
});

describe("nothing kept — explicit deletion", () => {
	it("touching a region then unkeeping everything removes its lines but still counts as resolved", () => {
		store.keepBlock(1, "ours");
		const keptId = (store.regions[1] as { kept: { id: string }[] }).kept[0].id;
		store.unkeep(1, keptId);
		expect(store.resolvedText).toBe("line1\nline3\n");
		expect(store.fileProgress).toEqual({ resolved: 1, total: 1 });
		expect(store.allResolved).toBe(true);
	});

	it("an untouched region (nothing decided yet) is not counted as resolved", () => {
		expect(store.fileProgress).toEqual({ resolved: 0, total: 1 });
		expect(store.allResolved).toBe(false);
	});
});

describe("same-in-both dedupe is always present in resolvedText", () => {
	it("prefix/suffix lines appear even before the middle candidate is decided", () => {
		const withDedupe: FileRegion[] = [
			{
				kind: "conflict",
				baseStart: 0,
				baseEnd: 2,
				sameBothPrefix: ["shared new line"],
				oursLines: ["line1 edited"],
				theirsLines: ["line1"],
				sameBothSuffix: ["tail"],
			},
		];
		store.open("f.txt", withDedupe, "main", "feature/x");
		expect(store.resolvedText).toBe("shared new line\ntail\n");
		store.keepBlock(0, "ours");
		expect(store.resolvedText).toBe("shared new line\nline1 edited\ntail\n");
	});
});

describe("reset file", () => {
	it("clears every region back to untouched, all-candidates state", () => {
		store.keepBlock(1, "ours");
		store.resetFile();
		expect(store.resolvedText).toBe("line1\nline3\n");
		expect(store.fileProgress).toEqual({ resolved: 0, total: 1 });
	});
});

describe("keepAllFrom", () => {
	it("keeps one side's whole block in every untouched region, leaving touched ones alone", () => {
		const multi: FileRegion[] = [
			{
				kind: "conflict",
				baseStart: 0,
				baseEnd: 1,
				sameBothPrefix: [],
				oursLines: ["A-ours"],
				theirsLines: ["A-theirs"],
				sameBothSuffix: [],
			},
			{
				kind: "conflict",
				baseStart: 1,
				baseEnd: 2,
				sameBothPrefix: [],
				oursLines: ["B-ours"],
				theirsLines: ["B-theirs"],
				sameBothSuffix: [],
			},
		];
		store.open("f.txt", multi, "main", "feature/x");
		store.editRegion(1, "already decided by hand");

		store.keepAllFrom("theirs");

		expect(store.resolvedLines).toEqual(["A-theirs", "already decided by hand"]);
	});
});

describe("operationProgress", () => {
	it("aggregates several files' progress into the banner's counts", () => {
		const progress = operationProgress({
			"a.txt": { resolved: 2, total: 2 },
			"b.txt": { resolved: 0, total: 3 },
		});
		expect(progress).toEqual({
			filesDone: 1,
			filesTotal: 2,
			regionsResolved: 2,
			regionsTotal: 5,
		});
	});

	it("a file with zero conflict regions counts as done", () => {
		const progress = operationProgress({ "clean.txt": { resolved: 0, total: 0 } });
		expect(progress.filesDone).toBe(1);
	});
});
