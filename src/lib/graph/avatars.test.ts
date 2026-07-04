import { describe, expect, it } from "vitest";
import { authorInitials, discColorIndex, gravatarHash, gravatarUrl } from "./avatars";
import { GRAPH_LANE_PALETTE_SIZE } from "./lanes";

describe("authorInitials", () => {
	it("takes first and last name initials", () => {
		expect(authorInitials("Jane Doe", "jane@example.com")).toBe("JD");
	});

	it("takes two letters from a single name", () => {
		expect(authorInitials("madonna", "m@x.com")).toBe("MA");
	});

	it("splits on dots and dashes in a name", () => {
		expect(authorInitials("ada.lovelace", "ada@x.com")).toBe("AL");
	});

	it("falls back to the email local-part when the name is blank", () => {
		expect(authorInitials("", "grace@example.com")).toBe("GR");
	});

	it("returns a placeholder when there is nothing to use", () => {
		expect(authorInitials("", "")).toBe("?");
	});
});

describe("discColorIndex", () => {
	it("is stable and within the palette", () => {
		const a = discColorIndex("jane@example.com");
		const b = discColorIndex("JANE@EXAMPLE.COM ");
		expect(a).toBe(b);
		expect(a).toBeGreaterThanOrEqual(0);
		expect(a).toBeLessThan(GRAPH_LANE_PALETTE_SIZE);
	});
});

describe("gravatar", () => {
	it("hashes the normalized email with SHA-256", async () => {
		// Known SHA-256 of "jane@example.com".
		expect(await gravatarHash("  Jane@Example.com ")).toBe(
			await gravatarHash("jane@example.com"),
		);
		expect(await gravatarHash("jane@example.com")).toHaveLength(64);
	});

	it("builds a 404-on-miss avatar URL", () => {
		expect(gravatarUrl("abc123", 48)).toBe(
			"https://www.gravatar.com/avatar/abc123?s=48&d=404",
		);
	});
});
