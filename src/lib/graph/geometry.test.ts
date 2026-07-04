import { describe, expect, it } from "vitest";
import {
	anchoredScrollTop,
	graphWidthForLanes,
	laneCenterX,
	ROW_HEIGHT,
	totalHeight,
	visibleRowRange,
} from "./geometry";

describe("visibleRowRange", () => {
	it("returns an empty window for no rows", () => {
		expect(visibleRowRange(0, 500, 0)).toEqual({ start: 0, end: 0 });
	});

	it("includes overscan above and below the viewport", () => {
		// Viewport of 10 rows starting at row 100, overscan 20 → [80, 130).
		const { start, end } = visibleRowRange(100 * ROW_HEIGHT, 10 * ROW_HEIGHT, 1000, 20);
		expect(start).toBe(80);
		expect(end).toBe(130);
	});

	it("clamps to the available rows", () => {
		expect(visibleRowRange(0, 5 * ROW_HEIGHT, 3, 20)).toEqual({ start: 0, end: 3 });
	});

	it("never returns a negative start when scrolled to the top", () => {
		expect(visibleRowRange(0, 200, 1000, 20).start).toBe(0);
	});
});

describe("lane geometry", () => {
	it("spaces lane centres evenly", () => {
		expect(laneCenterX(1) - laneCenterX(0)).toBe(laneCenterX(2) - laneCenterX(1));
	});

	it("sizes the graph column to fit every lane", () => {
		expect(graphWidthForLanes(3)).toBeGreaterThan(laneCenterX(3));
	});
});

describe("scroll anchoring", () => {
	it("keeps a row at its prior on-screen offset", () => {
		expect(anchoredScrollTop(42, 7)).toBe(42 * ROW_HEIGHT + 7);
	});

	it("never scrolls above the top", () => {
		expect(anchoredScrollTop(0, -50)).toBe(0);
	});
});

describe("totalHeight", () => {
	it("is row count times row height", () => {
		expect(totalHeight(5000)).toBe(5000 * ROW_HEIGHT);
	});
});
