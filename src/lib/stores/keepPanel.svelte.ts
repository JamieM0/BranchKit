/** The Keep Panel reducer — DESIGN_SPEC.md §9. One instance holds one conflicted file's regions
 * (from `get_conflict_regions`, ARCHITECTURE.md §7.5) and the fluid keep/unkeep state layered on
 * top of them. Nothing here writes to disk — `resolvedText` is handed to `confirmFile` by the
 * caller once every region is decided.
 *
 * Mental model (§9's "choosing what to keep"): a conflict region starts as pure candidates.
 * `keepBlock`/`keepLine` move a candidate into `kept`, in click order — DESIGN_SPEC.md §9.2's
 * "keep both" is just two calls to `keepBlock`, one per side, and click order is preserve order.
 * `sameBothPrefix`/`sameBothSuffix` (the §9.3 "same in both" dedupe) are already resolved from the
 * moment the region is opened — they're never candidates. */

import type { FileRegion, Side } from "$lib/types";

export type KeptSource = Side | "edit";

export interface KeptItem {
	id: string;
	source: KeptSource;
	lines: string[];
}

interface ContextRegionState {
	kind: "context";
	lines: string[];
}

interface AutoResolvedRegionState {
	kind: "autoResolved";
	side: Side;
	lines: string[];
}

interface ConflictRegionState {
	kind: "conflict";
	baseStart: number;
	baseEnd: number;
	sameBothPrefix: string[];
	oursLines: string[];
	theirsLines: string[];
	sameBothSuffix: string[];
	kept: KeptItem[];
	/** Set the moment the user makes any decision for this region — including unkeeping back to
	 * nothing, which is itself a legal, explicit resolution (§9.2 "Nothing kept — lines removed").
	 * Cleared only by `resetFile`/`resetRegion`. Drives the per-file progress count, not whether
	 * `kept` happens to be non-empty. */
	touched: boolean;
}

type RegionState = ContextRegionState | AutoResolvedRegionState | ConflictRegionState;

export interface FileProgress {
	resolved: number;
	total: number;
}

function toRegionState(region: FileRegion): RegionState {
	switch (region.kind) {
		case "context":
			return { kind: "context", lines: region.lines };
		case "autoResolved":
			return { kind: "autoResolved", side: region.side, lines: region.lines };
		case "conflict":
			return {
				kind: "conflict",
				baseStart: region.baseStart,
				baseEnd: region.baseEnd,
				sameBothPrefix: region.sameBothPrefix,
				oursLines: region.oursLines,
				theirsLines: region.theirsLines,
				sameBothSuffix: region.sameBothSuffix,
				kept: [],
				touched: false,
			};
	}
}

export class KeepPanelStore {
	path: string | null = $state(null);
	oursLabel = $state("");
	theirsLabel = $state("");
	regions: RegionState[] = $state([]);

	#nextId = 0;

	/** Loads a freshly-fetched file's regions, discarding any previous fluid state — a new file
	 * (or a reopened/reset one) always starts from the raw region computation. */
	open(path: string, regions: FileRegion[], oursLabel: string, theirsLabel: string) {
		this.path = path;
		this.oursLabel = oursLabel;
		this.theirsLabel = theirsLabel;
		this.regions = regions.map(toRegionState);
	}

	#conflictRegion(regionIndex: number): ConflictRegionState {
		const region = this.regions[regionIndex];
		if (!region || region.kind !== "conflict") {
			throw new Error(`region ${regionIndex} is not a conflict region`);
		}
		return region;
	}

	/** Keep an entire candidate block (DESIGN_SPEC.md §9.2's block-level Keep button). */
	keepBlock(regionIndex: number, source: Side) {
		const region = this.#conflictRegion(regionIndex);
		const lines = source === "ours" ? region.oursLines : region.theirsLines;
		region.kept.push({ id: String(this.#nextId++), source, lines: [...lines] });
		region.touched = true;
	}

	/** Keep a single candidate line (the gutter's hover ✓, §9.2). */
	keepLine(regionIndex: number, source: Side, lineIndex: number) {
		const region = this.#conflictRegion(regionIndex);
		const lines = source === "ours" ? region.oursLines : region.theirsLines;
		const line = lines[lineIndex];
		if (line === undefined) return;
		region.kept.push({ id: String(this.#nextId++), source, lines: [line] });
		region.touched = true;
	}

	/** Sends a kept block/line back to the candidate pool — clicking its pin (§9.2). */
	unkeep(regionIndex: number, keptId: string) {
		const region = this.#conflictRegion(regionIndex);
		region.kept = region.kept.filter((k) => k.id !== keptId);
		region.touched = true;
	}

	/** Reorders a kept item within its region's click-order stack — the ↑↓ reorder handles (§9.2). */
	reorder(regionIndex: number, keptId: string, direction: "up" | "down") {
		const region = this.#conflictRegion(regionIndex);
		const idx = region.kept.findIndex((k) => k.id === keptId);
		if (idx === -1) return;
		const swapWith = direction === "up" ? idx - 1 : idx + 1;
		if (swapWith < 0 || swapWith >= region.kept.length) return;
		const items = region.kept;
		[items[idx], items[swapWith]] = [items[swapWith], items[idx]];
	}

	/** The hand-edit escape hatch (§9.2) — replaces the region's entire kept stack with one
	 * hand-edited block, marked with `source: "edit"` (the spec's "distinct pin color"). */
	editRegion(regionIndex: number, text: string) {
		const region = this.#conflictRegion(regionIndex);
		region.kept = [{ id: String(this.#nextId++), source: "edit", lines: text.split("\n") }];
		region.touched = true;
	}

	/** Per-file bulk action: "Keep all from `oursLabel`/`theirsLabel`" (§9.2) — keeps that side's
	 * whole block in every still-untouched conflict region (touched regions are left alone, same
	 * as clicking each region's Keep button individually never double-applies). */
	keepAllFrom(source: Side) {
		this.regions.forEach((region, i) => {
			if (region.kind === "conflict" && !region.touched) {
				this.keepBlock(i, source);
			}
		});
	}

	/** "Reset file" (§9.2) — back to all-candidates, as if the regions were just (re)computed. */
	resetFile() {
		for (const region of this.regions) {
			if (region.kind === "conflict") {
				region.kept = [];
				region.touched = false;
			}
		}
	}

	/** The resolved file, line by line, in document order — context + auto-resolved + each
	 * conflict region's `sameBothPrefix` + kept (in click order) + `sameBothSuffix`. Reactive:
	 * recomputes (and so "renumbers" everything downstream) on every keep/unkeep/reorder. */
	get resolvedLines(): string[] {
		const out: string[] = [];
		for (const region of this.regions) {
			if (region.kind === "context" || region.kind === "autoResolved") {
				out.push(...region.lines);
			} else {
				out.push(...region.sameBothPrefix);
				for (const item of region.kept) {
					out.push(...item.lines);
				}
				out.push(...region.sameBothSuffix);
			}
		}
		return out;
	}

	/** The text `confirmFile` writes to disk — always newline-terminated when non-empty, matching
	 * a normal text file (ARCHITECTURE.md doesn't preserve no-trailing-newline through the Keep
	 * Panel; see conflict.rs's `to_lines` for the same simplification on the Rust side). */
	get resolvedText(): string {
		const lines = this.resolvedLines;
		return lines.length > 0 ? lines.join("\n") + "\n" : "";
	}

	/** Conflict regions resolved (touched) vs total — the file tab's progress dots and the
	 * banner's per-file count (DESIGN_SPEC.md §9.1). */
	get fileProgress(): FileProgress {
		let total = 0;
		let resolved = 0;
		for (const region of this.regions) {
			if (region.kind === "conflict") {
				total += 1;
				if (region.touched) resolved += 1;
			}
		}
		return { resolved, total };
	}

	get allResolved(): boolean {
		const { resolved, total } = this.fileProgress;
		return resolved === total;
	}
}

/** Aggregates several files' [`FileProgress`] into the banner's "2 of 5 conflicts resolved · 1 of
 * 2 files done" (DESIGN_SPEC.md §9.1/§15.21) — a pure function so the caller (whatever holds one
 * `KeepPanelStore` per open file) can derive it without a dedicated multi-file store. */
export function operationProgress(perFile: Record<string, FileProgress>): {
	filesDone: number;
	filesTotal: number;
	regionsResolved: number;
	regionsTotal: number;
} {
	const entries = Object.values(perFile);
	return {
		filesDone: entries.filter((p) => p.total === 0 || p.resolved === p.total).length,
		filesTotal: entries.length,
		regionsResolved: entries.reduce((sum, p) => sum + p.resolved, 0),
		regionsTotal: entries.reduce((sum, p) => sum + p.total, 0),
	};
}
