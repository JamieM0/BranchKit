import { beforeEach, describe, expect, it } from "vitest";
import { commitDraft, COMMIT_GUIDE } from "./commitDraft.svelte";

beforeEach(() => commitDraft.reset());

describe("commitDraft counter (§7/§17)", () => {
	it("counts down from 72 and never blocks", () => {
		commitDraft.summary = "a".repeat(50);
		expect(commitDraft.remaining).toBe(COMMIT_GUIDE - 50);
	});

	it("is normal above 10 remaining, warns at ≤10, goes danger past 72", () => {
		commitDraft.summary = "a".repeat(50); // 22 remaining
		expect(commitDraft.counter).toBe("normal");
		commitDraft.summary = "a".repeat(62); // 10 remaining — warn boundary
		expect(commitDraft.counter).toBe("warn");
		commitDraft.summary = "a".repeat(72); // 0 remaining — still warn
		expect(commitDraft.counter).toBe("warn");
		commitDraft.summary = "a".repeat(80); // -8 remaining — danger
		expect(commitDraft.remaining).toBe(-8);
		expect(commitDraft.counter).toBe("danger");
	});

	it("only allows commit with a non-empty (trimmed) summary", () => {
		expect(commitDraft.canCommit).toBe(false);
		commitDraft.summary = "   ";
		expect(commitDraft.canCommit).toBe(false);
		commitDraft.summary = "real";
		expect(commitDraft.canCommit).toBe(true);
	});
});

describe("commitDraft amend backup (§15.15)", () => {
	it("prefills HEAD's message on enable and restores the draft on disable", () => {
		commitDraft.summary = "my work in progress";
		commitDraft.description = "some notes";
		commitDraft.enableAmend("HEAD summary", "HEAD body");
		expect(commitDraft.amend).toBe(true);
		expect(commitDraft.summary).toBe("HEAD summary");
		expect(commitDraft.description).toBe("HEAD body");
		commitDraft.disableAmend();
		expect(commitDraft.amend).toBe(false);
		expect(commitDraft.summary).toBe("my work in progress");
		expect(commitDraft.description).toBe("some notes");
	});
});
