import { describe, expect, it } from "vitest";
import { fuzzyFilter, fuzzyMatch } from "./fuzzy";

describe("fuzzyMatch", () => {
  it("matches a subsequence and records indices", () => {
    const result = fuzzyMatch("bk", "BranchKit");
    expect(result).not.toBeNull();
    expect(result?.indices).toEqual([0, 6]);
  });

  it("returns null when the query isn't a subsequence", () => {
    expect(fuzzyMatch("xyz", "BranchKit")).toBeNull();
  });

  it("empty query matches everything with zero score", () => {
    expect(fuzzyMatch("", "anything")).toEqual({ score: 0, indices: [] });
  });

  it("scores a contiguous match higher than a scattered one", () => {
    const contiguous = fuzzyMatch("git", "gitkraken");
    const scattered = fuzzyMatch("git", "great-init-tool");
    expect(contiguous!.score).toBeGreaterThan(scattered!.score);
  });

  it("scores a word-boundary match higher than a mid-word match", () => {
    const boundary = fuzzyMatch("kit", "branch-kit");
    const midword = fuzzyMatch("kit", "brankithing");
    expect(boundary!.score).toBeGreaterThan(midword!.score);
  });
});

describe("fuzzyFilter", () => {
  const items = ["BranchKit", "GitKraken", "gitui"];

  it("filters out non-matches and ranks the rest", () => {
    const results = fuzzyFilter("git", items, (s) => s);
    expect(results.map((r) => r.item)).toEqual(["gitui", "GitKraken"]);
  });

  it("returns every item unranked for an empty query", () => {
    const results = fuzzyFilter("  ", items, (s) => s);
    expect(results.map((r) => r.item)).toEqual(items);
  });
});
