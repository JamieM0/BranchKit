import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ConflictState, FileConflictRegions } from "$lib/types";
import { KeepSession } from "./keepSession.svelte";

/** A minimal in-memory stand-in for the Rust conflict backend — enough to drive the whole
 * multi-file session the way the Keep Panel UI does: `state.files` shrinks the instant a file is
 * confirmed (staged), and the operation ends only once nothing is unmerged and `continue` is
 * called. Mirrors the manual-verification script from the build prompt (3-file merge). */
class FakeBackend {
	unmerged: string[];
	finished = false;
	constructor(
		files: string[],
		private regions: Record<string, FileConflictRegions>,
	) {
		this.unmerged = [...files];
	}

	getConflictState = async (): Promise<ConflictState | null> =>
		this.finished
			? null
			: { kind: "merge", sourceLabel: "feature/x", targetLabel: "main", files: [...this.unmerged] };

	getConflictRegions = async (_repoId: string, path: string): Promise<FileConflictRegions> =>
		this.regions[path];

	confirmFile = async (_repoId: string, path: string): Promise<void> => {
		this.unmerged = this.unmerged.filter((f) => f !== path);
	};

	reopenFile = async (_repoId: string, path: string): Promise<void> => {
		if (!this.unmerged.includes(path)) this.unmerged.push(path);
	};

	continueConflict = async (): Promise<void> => {
		if (this.unmerged.length === 0) this.finished = true;
	};

	abortConflict = async (): Promise<void> => {
		this.finished = true;
	};

	onRepoChanged = async () => () => {};
}

/** One conflict region: pick `ours` or `theirs`. */
function oneConflict(ours: string, theirs: string): FileConflictRegions {
	return {
		oursDeleted: false,
		theirsDeleted: false,
		regions: [
			{
				kind: "conflict",
				baseStart: 0,
				baseEnd: 1,
				sameBothPrefix: [],
				oursLines: [ours],
				theirsLines: [theirs],
				sameBothSuffix: [],
			},
		],
	};
}

function newSession(backend: FakeBackend): KeepSession {
	return new KeepSession(backend);
}

describe("KeepSession — 3-file merge", () => {
	let backend: FakeBackend;
	let session: KeepSession;

	beforeEach(async () => {
		vi.useFakeTimers();
		backend = new FakeBackend(["a.ts", "b.ts", "c.ts"], {
			"a.ts": oneConflict("A-ours", "A-theirs"),
			"b.ts": oneConflict("B-ours", "B-theirs"),
			"c.ts": oneConflict("C-ours", "C-theirs"),
		});
		session = newSession(backend);
		await session.open("repo1");
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it("loads every file's regions up front and starts with the panel closed", () => {
		expect(session.conflictActive).toBe(true);
		expect(session.panelOpen).toBe(false);
		expect(session.allFiles).toEqual(["a.ts", "b.ts", "c.ts"]);
		expect(session.progress).toEqual({
			filesDone: 0,
			filesTotal: 3,
			regionsResolved: 0,
			regionsTotal: 3,
		});
	});

	it("Continue is gated until every file is confirmed, then finishes the operation", async () => {
		expect(session.continueEnabled).toBe(false);

		// Resolve + confirm all three, advancing past each 400ms auto-advance beat.
		for (const path of ["a.ts", "b.ts", "c.ts"]) {
			session.openFile(path);
			session.entryFor(path)!.store.keepBlock(0, "ours");
			await session.confirmActive();
			await vi.advanceTimersByTimeAsync(400);
		}

		expect(session.progress.filesDone).toBe(3);
		expect(session.continueEnabled).toBe(true);

		await session.continue("Merge branch 'feature/x' into main");
		expect(session.conflictActive).toBe(false);
		expect(session.panelOpen).toBe(false);
	});

	it("confirming a file auto-advances to the next unresolved one after the 400ms beat", async () => {
		session.openFile("a.ts");
		session.entryFor("a.ts")!.store.keepBlock(0, "theirs");
		await session.confirmActive();

		expect(session.isConfirmed("a.ts")).toBe(true);
		// Not yanked immediately — only after the beat.
		expect(session.activePath).toBe("a.ts");
		await vi.advanceTimersByTimeAsync(400);
		expect(session.activePath).toBe("b.ts");
	});

	it("a confirmed file can be reopened (Reset file), returning it to unmerged", async () => {
		session.openFile("a.ts");
		session.entryFor("a.ts")!.store.keepBlock(0, "ours");
		await session.confirmActive();
		await vi.advanceTimersByTimeAsync(400);
		expect(session.isConfirmed("a.ts")).toBe(true);

		session.openFile("a.ts");
		await session.resetActiveFile();
		expect(session.isConfirmed("a.ts")).toBe(false);
		expect(session.entryFor("a.ts")!.store.fileProgress).toEqual({ resolved: 0, total: 1 });
	});

	it("keep-both resolves with click order, and nothing-kept is a legal explicit deletion", async () => {
		const store = session.entryFor("a.ts")!.store;
		store.keepBlock(0, "theirs");
		store.keepBlock(0, "ours");
		expect(store.resolvedText).toBe("A-theirs\nA-ours\n");

		store.unkeepAll(0);
		expect(store.allResolved).toBe(true); // touched, even though nothing is kept
		expect(store.resolvedText).toBe("");
	});

	it("global 'keep all from main' resolves every un-confirmed file at once", () => {
		session.keepAllGlobally("ours");
		expect(session.progress.regionsResolved).toBe(3);
		expect(session.entryFor("b.ts")!.store.resolvedText).toBe("B-ours\n");
	});

	it("abort tears the whole session down", async () => {
		await session.abort();
		expect(session.conflictActive).toBe(false);
		expect(session.allFiles).toEqual([]);
	});

	it("the disabled-Continue tooltip names exactly what's left", () => {
		session.entryFor("a.ts")!.store.keepBlock(0, "ours");
		// a.ts is resolved-in-panel but not confirmed; b/c still have a live conflict.
		const summary = session.remainingSummary;
		expect(summary).toContain("a.ts is resolved but not yet confirmed");
		expect(summary).toContain("1 conflict left in b.ts");
		expect(summary).toContain("1 conflict left in c.ts");
	});
});
