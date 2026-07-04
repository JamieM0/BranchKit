import { describe, expect, it } from "vitest";
import { tokenize, wordDiff } from "./wordDiff";

describe("tokenize", () => {
	it("splits into word and non-word runs", () => {
		expect(tokenize("the quick, brown-fox")).toEqual([
			"the",
			" ",
			"quick",
			", ",
			"brown",
			"-",
			"fox",
		]);
	});
});

describe("wordDiff", () => {
	it("marks nothing changed for identical lines", () => {
		const { oldSegments, newSegments } = wordDiff("const x = 1;", "const x = 1;");
		expect(oldSegments.every((s) => !s.changed)).toBe(true);
		expect(newSegments.every((s) => !s.changed)).toBe(true);
	});

	it("highlights only the single changed word", () => {
		const { oldSegments, newSegments } = wordDiff("the quick brown fox", "the quick red fox");
		const changedOld = oldSegments.filter((s) => s.changed).map((s) => s.text);
		const changedNew = newSegments.filter((s) => s.changed).map((s) => s.text);
		expect(changedOld).toEqual(["brown"]);
		expect(changedNew).toEqual(["red"]);
		// Unchanged context on both sides is preserved verbatim (the space tokens flanking the
		// changed word each survive independently, so they appear back-to-back once joined).
		expect(oldSegments.filter((s) => !s.changed).map((s) => s.text).join("")).toBe(
			"the quick  fox",
		);
	});

	it("marks everything changed when lines share no tokens", () => {
		const { oldSegments, newSegments } = wordDiff("aaa", "zzz");
		expect(oldSegments).toEqual([{ text: "aaa", changed: true }]);
		expect(newSegments).toEqual([{ text: "zzz", changed: true }]);
	});

	it("handles a pure insertion", () => {
		const { oldSegments, newSegments } = wordDiff("foo bar", "foo baz bar");
		expect(oldSegments.every((s) => !s.changed)).toBe(true);
		const changedNew = newSegments.filter((s) => s.changed).map((s) => s.text);
		expect(changedNew.join("")).toContain("baz");
	});

	it("coalesces adjacent same-state tokens into one segment", () => {
		const { oldSegments } = wordDiff("completely different text here", "x");
		expect(oldSegments).toHaveLength(1);
		expect(oldSegments[0].text).toBe("completely different text here");
	});
});
