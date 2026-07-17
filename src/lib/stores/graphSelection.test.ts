import { describe, expect, it, beforeEach } from "vitest";
import { graphSelection } from "./graphSelection.svelte";
import { commitExplanation } from "./commitExplanation.svelte";

describe("graphSelection", () => {
	beforeEach(() => {
		graphSelection.clear();
		commitExplanation.close();
	});

	it("single-selects and clears compare", () => {
		graphSelection.select("A");
		expect(graphSelection.selectedSha).toBe("A");
		expect(graphSelection.compare).toBeNull();
	});

	it("closes a commit explanation when selecting another commit", () => {
		graphSelection.select("A");
		commitExplanation.open("repo", "A");
		graphSelection.select("B");
		graphSelection.select("A");
		expect(commitExplanation.sha).toBeNull();
		expect(commitExplanation.repoId).toBeNull();
	});

	it("cmd-click with a prior selection enters compare mode", () => {
		graphSelection.select("A");
		graphSelection.toggleCompare("B");
		expect(graphSelection.compare).toEqual({ a: "A", b: "B" });
		expect(graphSelection.selectedSha).toBe("B");
	});

	it("cmd-click with nothing selected is a plain select", () => {
		graphSelection.toggleCompare("A");
		expect(graphSelection.compare).toBeNull();
		expect(graphSelection.selectedSha).toBe("A");
	});

	it("cmd-click on the same commit does not compare with itself", () => {
		graphSelection.select("A");
		graphSelection.toggleCompare("A");
		expect(graphSelection.compare).toBeNull();
	});

	it("swaps compare direction", () => {
		graphSelection.select("A");
		graphSelection.toggleCompare("B");
		graphSelection.swapCompare();
		expect(graphSelection.compare).toEqual({ a: "B", b: "A" });
	});
});
