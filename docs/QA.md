# QA.md — v0.1.0 polish-pass audit (BUILD_PROMPTS.md Prompt 17)

Audit of the running codebase against DESIGN_SPEC.md §15 (all 32 items), the §14
accessibility bar, and ARCHITECTURE.md §13/§14. Statuses: **pass** (verified in code, no
change needed), **fixed** (found broken during this pass, now fixed), **manual** (needs a
running app to confirm — this pass was executed in an environment without a Rust toolchain
or display, see "Not verified here" at the end).

## DESIGN_SPEC §15 — the 32-item checklist

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Double-click pill / remote pill checkout | pass | `RefPill` dblclick → `checkoutBranch` / `checkoutRemote` (track+checkout) |
| 2 | Double-click commit → guarded detach | pass | `DetachGuardPopover` + don't-ask-again + Create-branch alternative |
| 3 | WIP inline editing, synced, counter | pass | shared `commitDraft` store; **improved**: clicking anywhere on the WIP row now opens+focuses the editor |
| 4 | Hover lineage brighten/dim + ghost copy-sha | pass | `lineageKeys` in GraphView; copy-sha button on row hover |
| 5 | Cmd+click compare + swap | pass | `graphSelection.toggleCompare` → `ComparePanel` |
| 6 | Drag pill → merge/rebase/ff drop menu with glow | pass | `dnd` store + `DropMenu`, targets glow, invalid rows unaffected |
| 7 | Ahead/behind badges are buttons → fix-it popover | pass | `AheadBehindPopover` with Pull/Push/view commits |
| 8 | Diverged = warn tint + explained options | pass | `.badge.diverged` + popover copy |
| 9 | Toolbar badges mirror branch; Push→Publish | pass | **improved**: Publish now disables with an explanatory tooltip when the repo has no remote at all (new `list_remotes`) |
| 10 | Space stages + auto-advance | pass | `WorkingDirectoryPanel` keydown; advance before refresh |
| 11 | Gutter click+drag line staging + accent ticks | pass | `gutterMouseDown/Enter` drag mechanics in DiffViewer |
| 12 | Discard safety net, 7 days, toast Undo | pass | `discard.rs` trash patches + untracked copies + purge |
| 13 | Commit Undo until pushed; branch-delete Undo | pass | `actions.ts` (`undoCommit` soft reset; recreate-at-sha) |
| 14 | Checkout toast → Back | pass | two call sites in `actions.ts` |
| 15 | Amend pushed warning; draft restore on untick | pass | `pushed` derivation + `enableAmend`/`disableAmend` backup |
| 16 | Stateful commit button | pass | hidden / "Stage all & commit" split / "Commit N file(s) to `branch`" |
| 17 | 72-counter counts down, warns, never blocks | pass | `commitDraft.remaining/counter`; guide length configurable |
| 18 | Stash dblclick Pop + Undo; naming at creation | pass | graph + panel dblclick; "Stash with message…" |
| 19 | Behind toast, current branch, rate-limited | pass | `behindNotifier` (tested) wired in `+page.svelte` |
| 20 | Keep Panel (all sub-items) | **fixed** | was crashing on every conflict: tagged serde enums need `rename_all_fields` — `sameBothPrefix` et al. arrived in snake_case. Same bug fixed for stash rows' `baseSha` (stashes were all pinned to lane 0) |
| 21 | Banner progress + disabled-Continue tooltip | pass | "N of M conflicts · X of Y files done" in `ConflictBanner` |
| 22 | Push new branch → Create-PR toast | pass | in `actions.publish` when GitHub connected |
| 23 | CI dots + checks popover | pass | `CiDot` + `githubChecks` (lazy, cached, visible rows only) |
| 24 | Filter dims, never removes; filters panel too | pass | `filter` store shared by graph rows + all panel sections |
| 25 | Panel hover → pill glow; click → scroll to tip | pass | `graphNav.glowSha` / `scrollTo` |
| 26 | Combine tracking rows (setting, default on) | pass | `buildPanelModel` + `settings.combineTrackingBranches` |
| 27 | Column gear (Author/Date/SHA) + persisted widths | pass | **changed by design**: the GRAPH column lost its label and manual resize — it now auto-sizes to the widest visible lane (Jamie's request) |
| 28 | First-launch identity check | pass | `FirstLaunch` inline `user.name`/`user.email` form |
| 29 | index.lock → human error + Retry | pass | `error.rs` catalog + tests; **added** this pass: "no reachable remote" translation for pushes to missing remotes |
| 30 | Menu items show shortcuts; palette teaches keymap | pass | `ContextMenu` shortcut column; palette hints |
| 31 | Settings reveal only relevant fields | pass | `RevealSection` grid-rows animation, AI provider blocks |
| 32 | Viewport anchoring on refresh | pass | `anchoredScrollTop` + sha re-anchor effect |

## Accessibility (DESIGN_SPEC §14)

- **Focus rings — fixed.** Only the graph scroller had a visible focus style; added a global
  `:focus-visible { outline: 2px solid var(--accent) }` in `tokens.css` (keyboard-only, no
  mouse-click rings).
- **aria-labels — pass.** Audited every icon-only button (toolbar carets/gear, copy-sha, eye
  toggle, rail buttons, AI button, toast/settings close): all labeled.
- **Reduced motion — pass.** Global motion tokens zero out under `prefers-reduced-motion`;
  8 files carry additional explicit guards (shimmer, AI pulse/spin, animations).
- **No color-only information — pass.** Status colors ship with glyphs; ahead/behind ship
  with ↑/↓ characters; the unpushed halo is a shape (dashed ring), not just a color.
- **Contrast — fixed.** Scripted WCAG check over the token pairs (script below):
  - Dark: all body-text pairs ≥ 4.5:1 after nudging `--text-muted` `#8b91a7 → #959bb0` and
    `--danger` `#ff5c5c → #ff6b6b` (both were AA-large-only on `--overlay` menus).
  - Light: `--accent`/`--warn`/`--danger`/`--info` sat at 2.2–4.1:1 as text; darkened to
    `#117a55` / `#8f5c0f` / `#c0332d` / `#2b66c2` — all now ≥ 4.5:1 on every light surface
    including `--raised`. (`--text-faint` is intentionally sub-AA: decorative only.)

<details><summary>Contrast script (node)</summary>

```js
const lum = h => { const c=[1,3,5].map(i=>parseInt(h.slice(i,i+2),16)/255)
  .map(v=>v<=0.03928?v/12.92:((v+0.055)/1.055)**2.4);
  return 0.2126*c[0]+0.7152*c[1]+0.0722*c[2]; };
const ratio=(a,b)=>{const[x,y]=[lum(a),lum(b)].sort((p,q)=>q-p);return(x+0.05)/(y+0.05);};
// run each --text* / status hue against --bg, --surface, --raised, --overlay per theme
```
</details>

## Cross-platform (ARCHITECTURE §14)

- **CREATE_NO_WINDOW — pass.** Applied on every spawn (git exec ×2, llama-server sidecar).
- **Path handling — pass.** No string-concatenated filesystem paths found; `std::path` +
  tauri path APIs throughout.
- **EOL-only diffs — implemented this pass.** `run_diff` re-checks non-empty diffs with
  `--ignore-cr-at-eol`; if the diff vanishes, `FileDiff.eolOnly` is set and the viewer shows
  "Only line endings changed (CRLF ↔ LF)" instead of a wall of fake changes.
- **libsecret degradation — partial.** The keyring layer tolerates a missing secret service
  (documented in `credentials.rs`), but the explicit warning banner is not yet surfaced in
  the UI. **Deviation, tracked for 0.1.1.**

## Performance (ARCHITECTURE §13) — manual

Budgets (startup→graph <1.5s @5k, scroll 60fps @20k, watcher→UI <1s, keypress→paint <50ms)
could not be measured in this environment (no Rust toolchain/display). The architecture-level
guarantees are in place: topology-once + windowed metadata, single-rAF canvas redraws, avatar
ImageBitmap LRU, virtualized lists, visible-row-only CI checks. **To measure:** clone a large
OSS repo, `npm run tauri dev`, and use the WebView performance panel while scrolling; record
numbers here.

## Not verified here (run before tagging v0.1.0)

- `cargo test` + `cargo clippy -- -D warnings` in `src-tauri` — the Rust-side changes of this
  pass (serde `rename_all_fields`, `origin/HEAD` ref filter, `list_remotes`, error-catalog
  entry, `eol_only` diff probe, model pin constants) compile-checked only by inspection.
- Frontend verified: `svelte-check` 0 errors, `vitest` 127/127 green.
- The v0.1.0 release flow itself (`release.yml`) runs on the first pushed tag; artifacts land
  in a draft release for manual inspection.
- Known pre-existing wart: the repo's initial commit message contains a Co-Authored-By
  trailer; rewriting pushed history is the maintainer's call.
