# v0.1.0 QA report

Audit date: 17 July 2026. Native verification was performed on macOS with the Tauri dev build,
plus source-level cross-platform checks and the repository's Rust/TypeScript test suites.

## DESIGN_SPEC §15 — all 32 items

1. **Pass** — Local pill and panel-row double-click check out; remote pills create a tracking
   branch and check it out in one action (`RefPill`, `LeftPanel`, checkout actions).
2. **Pass** — Commit double-click opens the anchored detach guard with the create-branch
   alternative; the opt-out preference is respected.
3. **Pass** — Native QA confirmed the WIP summary, composer summary and 72 counter update from
   the same draft while typing.
4. **Pass** — Hover lineage dim/highlight and the copy-SHA affordance are implemented in the
   canvas/DOM row pair.
5. **Pass** — Cmd-click compare selection opens compare mode; swap is available in the panel.
6. **Pass** — Pill drag targets glow and the drop menu exposes merge/rebase/fast-forward choices.
7. **Pass** — Ahead/behind badges are buttons with Pull, Push and view-commits actions.
8. **Pass** — Divergence uses the warning treatment and inline explained choices.
9. **Pass** — Toolbar counts follow the checked-out branch and the native QA repo showed Publish
   before an upstream existed.
10. **Pass** — Keyboard QA staged three files with Space; selection advanced after each action.
11. **Pass** — Diff gutters support click-drag line ranges and staged accent ticks.
12. **Pass** — File/hunk/line/all discards use the seven-day recovery store and Undo toast.
13. **Pass** — Commit Undo soft-resets while eligible; branch-delete Undo recreates the saved SHA.
14. **Pass** — Checkout completion offers Back to the previous ref.
15. **Pass** — Amend warns for an upstream-reachable HEAD and preserves/restores the draft.
16. **Pass** — Commit action has hidden, Stage all & commit, and Commit N files states.
17. **Pass** — The live counter counts down, warns past 72, and does not block commit.
18. **Pass** — Stash names are accepted at creation; double-click pops with Undo.
19. **Pass** — Behind-count increases on the current branch produce a rate-limited Pull toast.
20. **Pass** — Native conflict QA verified real branch labels, keep-side/per-line controls,
   progress and keyboard names; reducer tests cover keep-both order, reorder, deletion ghosts,
   dedupe, renumbering and file advance.
21. **Pass** — The live merge banner showed “0 of 1 conflict resolved · 0 of 1 file done”; Continue
   was disabled with the exact remaining file in its help text.
22. **Pass** — A newly pushed branch offers Create pull request only when GitHub is connected.
23. **Pass** — Commit rows expose CI dots and the checks popover, lazily for visible rows.
24. **Pass** — The shared filter affects panel sections and dims graph rows instead of removing
   topology.
25. **Pass** — Panel branch hover highlights its graph pill; click navigates to its tip.
26. **Pass** — Combine tracking branches is present and defaults on.
27. **Pass** — The column gear controls Author/Date/SHA and persisted widths.
28. **Pass** — First launch checks Git identity and provides inline name/email repair.
29. **Pass** — Lock/auth/non-fast-forward failures are translated into human actions including
   Retry; translator tests cover index.lock.
30. **Pass** — Context menus show shortcut hints and the command palette exposes the keymap.
31. **Pass** — Settings conditionally reveal provider-specific AI and master-switch fields.
32. **Pass** — SHA anchoring restores the same row/offset after topology refresh; unit coverage
   exercises the anchoring calculation.

## Accessibility (§14)

- **Keyboard workflow: Pass.** With the mouse unused: Cmd-T opened a repo, Cmd-B created
  `qa-keyboard`, Space staged three files with auto-advance, the summary was entered through Tab,
  Cmd-Return created commit `9cd6b61`, and Cmd-Shift-P published it to a local bare remote.
- **Focus: Fixed.** Dialog/popover entry now uses `focusOnMount`; `svelte-check` has no autofocus
  warnings. Browser-computed focus style is a 2px accent outline with 1px offset.
- **Icon names: Pass.** `npm run check:a11y-icons` scans all 49 Svelte components and passes.
  Native accessibility trees also exposed names for every icon-only control encountered.
- **Reduced motion: Fixed.** Motion tokens collapse under `prefers-reduced-motion`; CSS animations,
  transitions and smooth scrolling are suppressed. Loading loops use the documented loop token and
  AI ellipsis stops cycling.
- **Contrast: Fixed.** `npm run check:contrast` checks semantic/status tokens against all four
  surfaces in both themes. Minimums: dark 4.62:1 (`text-faint`/overlay), light 4.54:1
  (`text-faint`/raised); all pairs meet WCAG AA.
- **Semantics/hit targets: Pass.** Canvas is `aria-hidden`; the virtualized DOM grid owns graph
  semantics. Icon controls include help text and status colors retain glyphs/text.

## Performance (§13)

Repository: `git/git` cloned from GitHub (81,540 commits), then copied into a standalone working
tree with a shallow boundary at exactly 20,000 reachable commits for repeatable measurements.

| Measurement | Budget | Result |
| --- | ---: | ---: |
| Git topology + lane assignment, 20k | supporting metric | 0.35s (0.23s Git, 0.12s lanes) |
| Watcher event → visible UI | <1s | 519ms |
| Startup → graph, 5k budget | <1.5s | **Deviation:** native debug/CUA measurement was not stable enough for a defensible release number; core 20k pipeline is 0.35s |
| Scroll at 20k | 60fps | **Deviation:** native CUA full-tree capture perturbed WebKit during the 20k run; canvas remains one rAF draw with virtual rows, but a release-build frame trace is still required |

The worst measured offender was fixed: straight lanes previously materialised 2,155,977 repeated
per-row pass entries (and 2,205,360 total segment objects in the initial representation). They now
use 9,774 merged spans plus 49,383 real transition objects, grouped by lane and batch-stroked by the
eight palette colors. Immutable snapshots use raw Svelte state, and topology crosses IPC as a
compact line payload rather than a nested bridge object graph. Re-run with:

```sh
npm run audit:graph -- /path/to/20k/repo
```

## Cross-platform gotchas (§14)

- **Windows: Pass (code inspection).** Every Git/AI child process uses `CREATE_NO_WINDOW` under
  `cfg(windows)`; paths use `Path`/`PathBuf` and UI normalisation supports both separators.
- **EOL-only diffs: Pass.** The backend retries with `--ignore-cr-at-eol`; the diff viewer shows the
  quiet line-ending-only note instead of a full-file change.
- **Linux secrets: Fixed.** When libsecret/keyring is unavailable, secrets fall back to zeroized
  process memory and the app displays a non-persistent-storage warning banner.
- **All platforms: Pass (code inspection).** Tauri path APIs provide app directories; no UI path
  assumes `~`. The only home-directory lookup is for conventional SSH key discovery.

## Release and verification

- Version is `0.1.0` in npm, Cargo and Tauri configuration.
- `release.yml` uses `tauri-apps/tauri-action` on `v*` tags and explicitly requests DMG (arm64 and
  x64), MSI, AppImage and DEB bundles. Hyphenated tags are prereleases.
- README includes features, per-OS install notes, real screenshots, contributing/AGPL notes and the
  GitKraken non-affiliation statement. CHANGELOG contains the 0.1.0 release.
- Local required checks are recorded after the final run below. Remote three-OS CI and RC artifact
  confirmation require the branch/tag push and are updated when GitHub accepts it.

Final local run:

- `npm run build` — pass (bundle-size advisory only).
- `npx vitest run` — 127/127 pass.
- `npx svelte-check` — 0 errors, 0 warnings.
- `cargo test` — 145 unit tests plus all integration suites pass.
- `cargo clippy --all-targets -- -D warnings` — pass.
- `npm run check:a11y-icons` and `npm run check:contrast` — pass.

## Spec deviations

- **Motion loop token:** indeterminate loading indicators retain a controlled loop in normal mode;
  reduced-motion mode suppresses them. This is documented with `SPEC-DEVIATION` at the token.
- **Local model pin:** the shipped managed model is Gemma 4 E2B rather than the older Gemma 3 1B
  wording in the design spec; the existing product/model pin is retained and documented in code.
- **WIP commit morph:** canvas virtualization cannot FLIP the DOM WIP row into a canvas commit node;
  the existing slide/success-sweep approximation remains documented in `GraphView`.
