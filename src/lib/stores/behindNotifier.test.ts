import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("$lib/actions", () => ({ pull: vi.fn() }));

import { toasts } from "./toasts.svelte";
import { notifyBehindIncrease, resetBehindTracking } from "./behindNotifier";

describe("notifyBehindIncrease", () => {
	beforeEach(() => {
		toasts.clear();
		resetBehindTracking("r1");
		vi.useRealTimers();
	});

	it("does not toast on the first observation", () => {
		notifyBehindIncrease("r1", "main", 3);
		expect(toasts.items).toHaveLength(0);
	});

	it("toasts when behind increases after the first observation", () => {
		notifyBehindIncrease("r1", "main", 1);
		notifyBehindIncrease("r1", "main", 3);
		expect(toasts.items).toHaveLength(1);
		expect(toasts.items[0].message).toContain("3 behind");
	});

	it("does not toast when behind stays the same or decreases", () => {
		notifyBehindIncrease("r1", "main", 3);
		notifyBehindIncrease("r1", "main", 3);
		notifyBehindIncrease("r1", "main", 1);
		expect(toasts.items).toHaveLength(0);
	});

	it("rate-limits to once a minute even if behind keeps increasing", () => {
		// Spy on push rather than inspecting `toasts.items` directly — a real dismiss timer would
		// otherwise fire mid-test once fake time advances past the 6s auto-dismiss.
		vi.useFakeTimers();
		const pushSpy = vi.spyOn(toasts, "push");

		notifyBehindIncrease("r1", "main", 1);
		notifyBehindIncrease("r1", "main", 2);
		expect(pushSpy).toHaveBeenCalledTimes(1);

		vi.advanceTimersByTime(1000);
		notifyBehindIncrease("r1", "main", 3);
		expect(pushSpy).toHaveBeenCalledTimes(1);

		vi.advanceTimersByTime(60_000);
		notifyBehindIncrease("r1", "main", 4);
		expect(pushSpy).toHaveBeenCalledTimes(2);
	});

	it("tracks repos independently", () => {
		notifyBehindIncrease("r1", "main", 1);
		notifyBehindIncrease("r2", "main", 1);
		notifyBehindIncrease("r1", "main", 2);
		expect(toasts.items).toHaveLength(1);
	});
});
