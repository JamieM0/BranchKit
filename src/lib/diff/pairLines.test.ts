import { describe, expect, it } from "vitest";
import { isLinePair, pairChangedLines } from "./pairLines";
import type { DiffLine } from "$lib/types";

function ctx(text: string): DiffLine {
	return { kind: "context", oldNo: 1, newNo: 1, text, noNewlineAtEof: false };
}
function del(text: string): DiffLine {
	return { kind: "del", oldNo: 1, newNo: null, text, noNewlineAtEof: false };
}
function add(text: string): DiffLine {
	return { kind: "add", oldNo: null, newNo: 1, text, noNewlineAtEof: false };
}

describe("pairChangedLines", () => {
	it("passes context lines through unpaired", () => {
		const result = pairChangedLines([ctx("a")]);
		expect(result).toEqual([ctx("a")]);
	});

	it("pairs a single del followed by a single add", () => {
		const result = pairChangedLines([del("old"), add("new")]);
		expect(result).toHaveLength(1);
		expect(isLinePair(result[0])).toBe(true);
		if (isLinePair(result[0])) {
			expect(result[0].del?.text).toBe("old");
			expect(result[0].add?.text).toBe("new");
		}
	});

	it("pairs index-wise when del/add runs are equal length", () => {
		const result = pairChangedLines([del("d1"), del("d2"), add("a1"), add("a2")]);
		expect(result).toHaveLength(2);
		expect(isLinePair(result[0]) && result[0].del?.text).toBe("d1");
		expect(isLinePair(result[0]) && result[0].add?.text).toBe("a1");
		expect(isLinePair(result[1]) && result[1].del?.text).toBe("d2");
		expect(isLinePair(result[1]) && result[1].add?.text).toBe("a2");
	});

	it("leaves excess dels or adds unpaired", () => {
		const result = pairChangedLines([del("d1"), del("d2"), add("a1")]);
		expect(result).toHaveLength(2);
		expect(isLinePair(result[0])).toBe(true);
		expect(isLinePair(result[1])).toBe(false);
		expect(result[1]).toEqual(del("d2"));
	});

	it("handles a pure addition with no preceding del", () => {
		const result = pairChangedLines([add("new")]);
		expect(result).toHaveLength(1);
		expect(isLinePair(result[0])).toBe(false);
	});
});
