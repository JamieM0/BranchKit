/** The shared commit-composer draft — DESIGN_SPEC.md §7 / §4.2 / §15.3,15,16,17.
 *
 * Single source of truth for BOTH the right-panel commit composer (§7) AND the graph's WIP-row
 * inline editor (§4.2), so typing in one mirrors instantly in the other — components bind directly
 * to `summary`/`description`. It also owns the 72-char countdown state (§17), the amend draft
 * backup that's restored on untick (§15.15) and the one-shot "move focus to the description"
 * signal (Enter from a summary field). */

import { appSettings } from "./appSettings.svelte";

/** Commit-summary guide length — DESIGN_SPEC.md §7/§13 (default 72, configurable in Settings → Git). */
export const COMMIT_GUIDE = 72;

/** Counter tint: `--text-faint` normally, `--warn` when close, `--danger` once past the guide. */
export type CounterState = "normal" | "warn" | "danger";

class CommitDraftStore {
	summary = $state("");
	description = $state("");
	amend = $state(false);
	/** The WIP-row `// WIP` text is being edited inline right now (§4.2). */
	editingWip = $state(false);
	/** AI generation is streaming into the draft — every bound editor (composer summary +
	 * description, WIP inline input) locks itself while this is true so the user's keystrokes and
	 * the stream can't fight over the fields. */
	aiGenerating = $state(false);
	/** Bumped to ask the composer to focus its description textarea (Enter from a summary field). */
	focusDescriptionToken = $state(0);

	// The draft backed up while amend is on, so unticking restores it verbatim (§15.15).
	#savedSummary = "";
	#savedDescription = "";

	/** Chars remaining before the guide length (Settings → Git, default 72); goes negative past it —
	 * shown, never blocks (§7/§17). */
	get remaining(): number {
		return appSettings.current.git.commitSummaryGuideLength - this.summary.length;
	}

	/** `--warn` at ≤10 remaining, `--danger` once negative, `--text-faint` otherwise (§7). */
	get counter(): CounterState {
		if (this.remaining < 0) return "danger";
		if (this.remaining <= 10) return "warn";
		return "normal";
	}

	/** A commit needs a non-empty summary; amend can also just rewrite the message. */
	get canCommit(): boolean {
		return this.summary.trim().length > 0;
	}

	requestDescriptionFocus() {
		this.focusDescriptionToken += 1;
	}

	/** Amend ticked → back up the current draft and prefill HEAD's message (§15.15). */
	enableAmend(headSummary: string, headBody: string) {
		this.#savedSummary = this.summary;
		this.#savedDescription = this.description;
		this.summary = headSummary;
		this.description = headBody;
		this.amend = true;
	}

	/** Amend unticked → restore the draft we backed up; don't lose the user's message (§15.15). */
	disableAmend() {
		this.summary = this.#savedSummary;
		this.description = this.#savedDescription;
		this.amend = false;
	}

	/** Clear everything after a successful commit (fields clear, §7). */
	reset() {
		this.summary = "";
		this.description = "";
		this.amend = false;
		this.editingWip = false;
		this.#savedSummary = "";
		this.#savedDescription = "";
	}
}

export const commitDraft = new CommitDraftStore();
