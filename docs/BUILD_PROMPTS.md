# BUILD_PROMPTS.md — The BranchKit Build Sequence

> **For Jamie.** This file is the build plan: 17 prompts, run in order, one per Claude Code
> session. Each prompt block is copy-paste ready. Above each block is a **📋 Note to Jamie**
> with the recommended model + effort and anything you must do first.

## How to run this

1. **One prompt = one session.** Start each prompt in a fresh session (`/clear` or new session).
   The prompts are self-contained: each tells Claude which spec sections to read first. Don't
   paste two prompts into one session.
2. **Commit between prompts.** Each prompt ends by committing. If a session ends with broken
   code, fix it (or revert) before starting the next prompt — never build on red.
3. **Models & effort.** Default is **Sonnet**. UX-heavy prompts are marked **Opus** — you said
   you'd spend Opus on UI/UX; these are the ones worth it. "Effort" = the thinking/effort dial:
   the prompts are detailed, so **medium is usually enough**; **high** is flagged where the work
   is genuinely hard (graph renderer, Keep Panel, staging edge cases). If a Sonnet result
   disappoints on a UX prompt, rerun the same prompt with Opus — they're written to work for both.
4. **The no-subagent rule** is baked into CLAUDE.md (created in Prompt 1) and repeated in every
   prompt. If Claude proposes launching agents/Tasks, refuse and restate the rule.
5. **Verification is part of every prompt.** Don't accept "done" without the listed checks passing.
6. **Before Prompt 1:** install Rust (rustup), Node 20+, and the Tauri OS prerequisites
   (see tauri.app/start/prerequisites). Before **Prompt 15**: register the GitHub OAuth app
   (github.com → Settings → Developer settings → OAuth Apps → New; enable **Device Flow**;
   no callback needed for device flow — put any URL) and have the Client ID ready.

### Standard header (already embedded at the top of every prompt below)

Every prompt starts with the same preamble so sessions behave consistently. If you ever write
your own follow-up prompts, reuse it.

---

## Phase map

| # | Prompt | Model | Effort |
|---|--------|-------|--------|
| 1 | Scaffold, CLAUDE.md, tokens, CI | Sonnet | medium |
| 2 | Git execution layer + parsers + test harness | Sonnet | **high** |
| 3 | Repo tabs, open/clone, watcher, event pipeline | Sonnet | medium |
| 4 | Graph data pipeline + lane algorithm | Sonnet | **high** |
| 5 | Graph renderer (canvas + DOM virtualization) | **Opus** | high |
| 6 | Ref pills, left panel, checkout interactions | **Opus** | medium |
| 7 | Status, file lists, diff viewer | Sonnet | medium |
| 8 | Hunk/line staging + discard safety net | Sonnet | **high** |
| 9 | Commit composer + WIP inline editing | **Opus** | medium |
| 10 | Sync: fetch/pull/push, badges, error translation | Sonnet | high |
| 11 | Stash, context menus, command palette, shortcuts | Sonnet | medium |
| 12 | Merge machinery + conflict engine (backend) | Sonnet | **high** |
| 13 | The Keep Panel (conflict UI) | **Opus** | **high** |
| 14 | File history, blame, worktrees | Sonnet | medium |
| 15 | Settings, credential manager, GitHub integration | Sonnet | high |
| 16 | AI: local model manager, Ollama, remote APIs | Sonnet | high |
| 17 | Polish pass, a11y, perf audit, release pipeline | **Opus** | medium |

---

## Prompt 1 — Scaffold, CLAUDE.md, design tokens, CI

**📋 Jamie:** Sonnet, medium. First run `git init` in this folder if you haven't. Docs
(these four .md files) get moved into `docs/` by this prompt.

```
You are building BranchKit, a free open-source (AGPL-3.0) cross-platform git client inspired by
GitKraken, in this repository. Four spec documents exist at the repo root:
GITKRAKEN_WORKFLOWS.md, DESIGN_SPEC.md, ARCHITECTURE.md, BUILD_PROMPTS.md.
Read DESIGN_SPEC.md §1–3 and ARCHITECTURE.md §1–2 fully before writing any code.
HARD RULES for this and every session: work only in this session — do NOT use subagents, the
Task/Agent tool, or background agents, ever. Do not add dependencies beyond those named in
ARCHITECTURE.md without a code comment justifying them. Follow the specs exactly; mark any
deviation with a SPEC-DEVIATION comment.

Tasks:
1. Scaffold Tauri 2.x + Svelte 5 + TypeScript + Vite per ARCHITECTURE §1's repository layout.
   App identifier dev.branchkit.app, window title "BranchKit", min size 1024x640.
2. Move the four .md specs into docs/ and create a CLAUDE.md at root containing: a 5-line
   project summary; "Read docs/DESIGN_SPEC.md and docs/ARCHITECTURE.md sections relevant to your
   task before coding"; the HARD RULES above verbatim; the commands to run dev
   (npm run tauri dev), tests (cargo test in src-tauri, npx vitest run), and lint
   (cargo clippy -- -D warnings, npx svelte-check); code conventions (Rust: one module per git
   command family; TS: all IPC through src/lib/ipc.ts; all colors via CSS tokens).
3. Implement the full design token system from DESIGN_SPEC §2 as CSS custom properties with
   data-theme switching (dark default, light, follows OS via matchMedia), a ThemeProvider, and
   a hidden /tokens dev route rendering all tokens/colors/type for visual inspection.
4. Add LICENSE (AGPL-3.0 full text), a short README (name, one-paragraph pitch, screenshot
   placeholder, build instructions, license), .gitignore.
5. CI: .github/workflows/test.yml per ARCHITECTURE §12 (3-OS matrix; include the Ubuntu
   webkit2gtk apt dependencies step from the Tauri docs).
Verify: npm run tauri dev opens a themed empty window; both test commands run green (add one
trivial test each side); clippy and svelte-check pass. Then commit everything as
"chore: scaffold BranchKit (Tauri 2 + Svelte 5), design tokens, CI".
```

---

## Prompt 2 — Git execution layer, parsers, test harness

**📋 Jamie:** Sonnet, **high** effort. This is the foundation everything parses through —
insist on the full fixture list before accepting.

```
BranchKit session. Read CLAUDE.md, then ARCHITECTURE.md §3 (execution rules), §5.1, §5.3, §6.1,
§6.2, §12 in full. No subagents or Task/Agent tools — work only in this session.

Build the entire git read layer in src-tauri/src/git/:
1. exec.rs implementing ARCHITECTURE §3 exactly: args-vector spawning, the mandatory env vars
   and -c config injection, byte stdout, timeouts with process-group kill, GitOutput/GitError,
   Windows CREATE_NO_WINDOW, git version detection (min 2.30).
2. Parsers, each in its own module, each returning serde DTOs (mirror them in src/lib/types.ts):
   - log.rs: topology via rev-list --parents (ARCHITECTURE §5.1 "do it this way" paragraph) and
     batched metadata via git show with %x1f/%x1e separators; stash list injection.
   - refs.rs: for-each-ref with %(upstream:track) ahead/behind parsing, HEAD/detached detection.
   - status.rs: porcelain v2 -z parsing incl. renames, unmerged, partially-staged (both-section)
     files.
   - diff.rs: unified diff → structured Hunks/Lines, binary/image detection, two-commit and
     cached variants.
3. Test harness per ARCHITECTURE §12: a TestRepo builder running real git in tempdirs, then
   integration tests covering EVERY fixture in the §12 list (linear, merge, criss-cross,
   renames, conflict with marker-like content, no-trailing-newline, non-UTF8 filename with
   Windows skip, detached HEAD, worktrees). Aim for the parsers to be boringly bulletproof.
Verify: cargo test green with 25+ meaningful assertions; clippy clean. Commit as
"feat: git execution layer and parsers with real-git test harness".
```

---

## Prompt 3 — Repo management, tabs, watcher, event pipeline

**📋 Jamie:** Sonnet, medium.

```
BranchKit session. Read CLAUDE.md, ARCHITECTURE.md §2 and §4, DESIGN_SPEC.md §3.1 and §11
(empty states, clone flow, first-launch). No subagents or Task/Agent tools.

Build:
1. RepoHandle registry + per-repo serial op queue + generation counter per ARCHITECTURE §2,
   with tauri commands: open_repo, clone_repo (with §3.1 progress events), close_repo,
   list_recents (persisted JSON in app config dir).
2. watcher.rs per ARCHITECTURE §4: notify on .git + worktree, 300ms debounce, event
   classification, storm folding, self-echo suppression tied to the generation counter.
3. Frontend shell: repo tabs per DESIGN_SPEC §3.1 (Cmd+T fuzzy picker over recents/Open/Clone,
   Cmd+W/Cmd+1..9/middle-click/drag-reorder, per-tab state store, spinner dot during ops);
   empty-state hero (Open/Clone/recents grid); clone dialog with progress phases; first-launch
   flow incl. the git identity check (DESIGN_SPEC §11).
4. ipc.ts typed wrappers + a repo.svelte.ts store that subscribes to repo://{id}/changed events
   and exposes fine-grained invalidation (status vs refs vs graph) for later prompts.
Verify: open two repos in tabs; touch a file in one from a terminal and see its event arrive
(<1s, log it); clone a public repo with visible progress; recents persist across restart.
Commit as "feat: repo tabs, open/clone, filesystem watcher and event pipeline".
```

---

## Prompt 4 — Graph data pipeline + lane algorithm

**📋 Jamie:** Sonnet, **high**. Pure logic, heavy tests — Sonnet is fine, but check the
snapshot tests exist.

```
BranchKit session. Read CLAUDE.md, ARCHITECTURE.md §5.1–§5.3. No subagents or Task/Agent tools.

Build the graph data layer (no rendering yet):
1. Tauri command get_graph(repo_id) returning the full topology (shas+parents, topo order,
   stash pseudo-rows) and get_commit_meta(repo_id, shas[]) for batched metadata, per
   ARCHITECTURE §5.1. get_refs(repo_id) per §5.3.
2. TS lane-assignment module implementing ARCHITECTURE §5.2's algorithm exactly, emitting
   per-row draw ops {node, edges[{fromLane,toLane,kind}]} and lane colors (palette index mod 8).
3. graph.svelte.ts store: composes topology + lazily-fetched metadata (visible-window batches
   of 200) + refs overlay by sha; exposes rows for a virtualized view; invalidates from the
   event pipeline WITHOUT refetching topology unless Refs/Head changed.
4. Vitest snapshot tests for the lane algorithm against fixture topologies: linear, single
   merge, two parallel branches, criss-cross, octopus (3+ parents), stash rows attached to
   their base. Also test that a refs-only update does not recompute lanes.
Verify: vitest + cargo test green; a temporary debug route dumps rows for a real repo and the
lane numbers are visibly sane. Commit as "feat: graph topology pipeline and lane assignment".
```

---

## Prompt 5 — Graph renderer

**📋 Jamie:** **Opus, high** — this is the centerpiece view and the hardest rendering work.
If you must economize, Sonnet-high can do it, but Opus will nail the feel (hover states,
animation timing).

```
BranchKit session. Read CLAUDE.md, ARCHITECTURE.md §5.4 and §13, DESIGN_SPEC.md §4.1–§4.3,
§4.5–§4.7, and §15 items 2,4,5,18,27,32. No subagents or Task/Agent tools.

Build the commit graph view (center area) on top of the prompt-4 store:
1. Hybrid renderer per ARCHITECTURE §5.4: hand-rolled virtualized DOM rows (28px, overscan 20)
   + a single background canvas drawing edges/nodes/avatars for the visible range only, DPR-
   scaled, redrawn via rAF on scroll. No rendering libraries.
2. Row content per DESIGN_SPEC §4.3: avatar-on-node (gravatar with initials-disc fallback +
   ImageBitmap LRU), summary + faint description preview, dimmer merge commits, HEAD accent
   ring. Columns BRANCH/TAG · GRAPH · MESSAGE with draggable widths + gear menu toggling
   Author/Date/SHA columns (persisted).
3. Interactions: click select (right panel placeholder event), Cmd/Ctrl+click compare selection
   (emit event; UI in prompt 7), double-click → detach guard popover per DESIGN_SPEC §4.6 incl.
   "Create branch here…" and don't-ask-again; hover lineage brighten/dim per §4.3 with the
   ghost copy-sha button; j/k/↑/↓ keyboard selection with auto-scroll; loading skeleton rows;
   scroll anchoring on refresh (§4.7 / §15.32).
4. Stash rows per §4.5 (render + selection; actions arrive with menus in prompt 11).
5. Detached-HEAD banner per §4.6.
Verify: load a 5k+ commit repo (clone one if needed): 60fps scroll (measure with the
performance panel), correct lanes vs git log --graph spot-check, hover/selection/keyboard all
work, refresh doesn't move the viewport. Commit as "feat: virtualized canvas commit graph".
```

---

## Prompt 6 — Ref pills, left panel, checkout

**📋 Jamie:** **Opus, medium.** The ahead/behind indicator design you flagged lives here.

```
BranchKit session. Read CLAUDE.md, DESIGN_SPEC.md §4.4 (every word — the ahead/behind rules are
strict), §5, §15 items 1,6,7,8,24,25,26, and GITKRAKEN_WORKFLOWS.md §1.3, §3.2. No subagents or
Task/Agent tools.

Build:
1. Branch/tag pills in the graph per DESIGN_SPEC §4.4: presence icons, shared vs split pills,
   ahead/behind badges EXACTLY per spec (nonzero only, colors, tooltip with commit previews,
   click → fix-it popover with Pull/Push/view actions, warn-tint diverged state), 2+overflow
   pill behavior, tag styling.
2. Checkout interactions: double-click pill → checkout; double-click remote pill → create
   tracking branch + checkout in one action; toast "Switched to X — Back" wired to a toasts
   store implementing DESIGN_SPEC §8's component (stack, timing, one action verb).
3. Drag pill onto pill/row → merge/rebase/ff drop menu with target glow (menu executes via ops
   from ARCHITECTURE §7.1; merge conflicts just surface the raw state for now — full conflict
   UX comes in prompts 12–13).
4. Left panel per DESIGN_SPEC §5: sections LOCAL/REMOTES/TAGS/STASHES/WORKTREES (PRs later),
   universal filter box (fuzzy, auto-expand, also dims graph per §15.24), combine-tracking-rows
   behavior, hover→pill-glow, click→scroll-to-tip, double-click checkout, hide-eye toggle,
   collapse to icon rail.
5. Branch ops with guards: create (inline name editor at HEAD pill), rename, delete with
   unmerged-guard + toast Undo (recreate at recorded sha, §15.13).
Verify: full checkout round-trip on a real repo incl. remote-tracking creation; badges match
git status -sb counts; drag-merge produces a merge commit; every §15 item listed above works.
Commit as "feat: ref pills, left panel, checkout and branch operations".
```

---

## Prompt 7 — Status, file lists, diff viewer

**📋 Jamie:** Sonnet, medium.

```
BranchKit session. Read CLAUDE.md, DESIGN_SPEC.md §6.1–§6.2, §2.4, §15 items 5,10, and
ARCHITECTURE.md §6.1–§6.2. No subagents or Task/Agent tools.

Build:
1. Right panel working-directory mode per DESIGN_SPEC §6.1: header with count + branch,
   Path/Tree toggle (tree with rolled-up folder badges), Unstaged/Staged sections with
   Stage All/Unstage All, per-file hover actions, status glyphs per §2.4, partially-staged
   files in both sections with half-filled glyph, Space-to-stage with auto-advance (§15.10),
   whole-file stage/unstage via git add/reset (ARCHITECTURE §7.1).
2. Right panel commit-detail mode: metadata card (author, relative+absolute date, sha copy,
   parents as clickable links), full message, changed-file list reusing the same row component.
3. Compare mode for Cmd+click pairs (§15.5): "Comparing A ↔ B" header with swap button,
   file list of git diff A B.
4. Diff viewer per DESIGN_SPEC §6.2 replacing the graph center with breadcrumb back: unified +
   split toggle, highlight.js per ARCHITECTURE §6.2, TS word-level intra-line diff (write +
   vitest it), whitespace toggle, hunk collapse >400 lines, binary/image handling
   (side-by-side, checkerboard), History/Blame/Open-file buttons (stubs navigating in prompt 14).
   WIP-row click routes to working-directory mode; commit click to detail mode.
Verify: every status glyph reachable via a scratch repo; rename shows as rename not
delete+add; image diff renders; word-diff highlights single-word changes; Space walks and
stages the whole unstaged list. Commit as "feat: status panels, compare mode and diff viewer".
```

---

## Prompt 8 — Hunk/line staging + discard safety net

**📋 Jamie:** Sonnet, **high**. The edge-case list is the whole game here — hold the line on
the test matrix.

```
BranchKit session. Read CLAUDE.md, ARCHITECTURE.md §6.3 (the exact patch technique) and §7.3,
DESIGN_SPEC.md §6.2 line-staging mechanics and §7.4, §15 items 11,12. No subagents or
Task/Agent tools.

Build:
1. stage.rs implementing ARCHITECTURE §6.3 exactly: patch construction for hunk and line
   subsets, git apply --cached --recount application, --reverse for unstaging, and explicit
   handling + integration tests for each listed edge case: no-trailing-newline marker,
   untracked (whole-file only), renames (whole-file only), mode-only, binary, staged-side line
   unstaging. These tests are the deliverable — do not skip any.
2. Diff-view gutter UI: hover checkboxes, click to stage line, click+drag for ranges, accent
   ticks on staged lines, per-hunk Stage/Discard buttons in hunk headers, same mechanics
   reversed in staged view.
3. Discard safety net per ARCHITECTURE §7.3 + DESIGN_SPEC §7.4: trash-patch before every
   discard (file/hunk/all + untracked copies), toast Undo, "Recently discarded" list (repo
   menu) with restore, 7-day/200MB purge on startup, arm-delay confirm only for Discard All.
Verify: stage two of five lines in a hunk → git diff --cached shows exactly those; unstage
reverses cleanly; discard→Undo restores byte-identical content incl. an untracked file;
cargo test covers all six edge cases. Commit as
"feat: hunk and line staging with discard safety net".
```

---

## Prompt 9 — Commit composer + WIP inline editing

**📋 Jamie:** **Opus, medium.** Small surface, huge feel — the WIP inline editor and stateful
button are signature moves.

```
BranchKit session. Read CLAUDE.md, DESIGN_SPEC.md §7 (all), §4.2, §15 items 3,13,15,16,17, and
ARCHITECTURE.md §7.1 commit specifics. No subagents or Task/Agent tools.

Build:
1. Commit composer per DESIGN_SPEC §7: summary with countdown-from-72 counter (warn/negative
   states, never blocks), growing description with col-72 soft ruler, amend checkbox with
   HEAD prefill + draft restore on untick + inline pushed-commit warning (§15.15), stateful
   primary button (hidden / "Stage all & commit" split-button / "Commit N files to branch"),
   Cmd+Enter, success sweep animation, ✨ AI button present but disabled with a tooltip
   deep-linking to Settings → AI (wired in prompt 16).
2. WIP row inline editing per §4.2: click //WIP → input, two-way sync with composer via a
   shared store, inline mini-counter, Enter→description focus, Cmd+Enter commits. WIP row
   slide-in/out animation; commit animates WIP → new commit row.
3. Commit toast with Undo (soft reset, only offered until push) per §15.13; commit command
   per ARCHITECTURE §7.1 (-m/-m, -F - fallback >8k).
Verify: type in the graph and watch the composer mirror (and vice versa); amend a pushed
commit shows the warning; Undo after commit restores staged state exactly; counter behavior
matches spec at 62/72/80 chars. Commit as "feat: commit composer with inline WIP editing".
```

---

## Prompt 10 — Sync: fetch/pull/push, badges, error translation

**📋 Jamie:** Sonnet, high.

```
BranchKit session. Read CLAUDE.md, ARCHITECTURE.md §7.1 (network ops), §7.2, §9, §3.1,
DESIGN_SPEC.md §3.2, §8, §11 error examples, §15 items 7,8,9,19. No subagents or
Task/Agent tools.

Build:
1. remote.rs: fetch/pull(ff|rebase|merge)/push/publish/force-with-lease per ARCHITECTURE §7.1
   with §3.1 stderr progress parsing → toolbar/tab progress UI. NEVER plain --force anywhere.
2. Toolbar per DESIGN_SPEC §3.2: Pull with mode dropdown + behind-badge, Push/Publish with
   ahead-badge, both driven by the current branch's upstream track counts; force-with-lease
   only in the dropdown behind a consequence-sentence confirm.
3. Auto-fetch per ARCHITECTURE §7.2 (focused-only interval, collision rules) + the rate-limited
   "N behind — Pull" toast (§15.19) and post-fetch badge refresh.
4. error.rs translation layer per ARCHITECTURE §9 with the full starter catalog, wired so every
   op failure anywhere in the app surfaces as a translated toast/dialog with suggestion button
   + sanitized Details (credential scrubbing test included). Implement the stash-and-checkout
   compound action for the would-be-overwritten case.
5. Divergence popover actions from the §4.4 badge (Pull rebase/merge, force-with-lease path).
Verify: publish a new branch to a scratch GitHub repo; create real divergence and resolve via
the badge popover both ways; kill network and see the offline state + focus-retry; unit test
the error catalog mappings. Commit as "feat: sync operations, indicators and error translation".
```

---

## Prompt 11 — Stash, context menus, command palette, shortcuts

**📋 Jamie:** Sonnet, medium.

```
BranchKit session. Read CLAUDE.md, DESIGN_SPEC.md §4.5, §10, §15 items 14,18,30, and
GITKRAKEN_WORKFLOWS.md §3 (full menu inventory), ARCHITECTURE.md §7.1 stash/history ops.
No subagents or Task/Agent tools.

Build:
1. Stash: toolbar Stash (dropdown: with message / include untracked) + Pop; inline stash row
   actions per DESIGN_SPEC §4.5 (double-click Pop with Undo-toast = re-stash, click → contents
   in right panel).
2. A single ContextMenu component (overlay, submenu, shortcut labels right-aligned, disabled
   states with tooltip reasons) and wire EVERY menu from GITKRAKEN_WORKFLOWS §3.1–§3.5 with
   the BK omissions noted there: commit row, branch pill/panel row, stash row, file row, graph
   gear. Actions all exist from prior prompts except: cherry-pick, revert, reset soft/mixed/hard
   (guarded per DESIGN_SPEC §4.6), rebase-onto, tag create/annotated/delete, copy sha/link/path,
   create-patch-to-file, compare-against-working-directory, ignore submenu — implement these in
   ops modules now (ARCHITECTURE §7.1). Conflicted outcomes surface state; UI lands in 12–13.
3. Command palette per DESIGN_SPEC §10 (context-aware actions, branches with ↵/Cmd↵ semantics,
   changed files, repos) and the full global shortcut map, with shortcuts displayed in menus.
Verify: cherry-pick and revert round-trip on a scratch repo; reset hard is guarded + arm-delayed;
every menu item either works or is visibly disabled-with-reason; palette fuzzy-checkouts a
branch in <5 keystrokes. Commit as "feat: stash actions, context menus and command palette".
```

---

## Prompt 12 — Merge machinery + conflict engine (backend)

**📋 Jamie:** Sonnet, **high**. Pure logic; the Keep Panel's correctness depends on it.

```
BranchKit session. Read CLAUDE.md, ARCHITECTURE.md §7.4–§7.5 in full, DESIGN_SPEC.md §9.1 for
the states it must serve. No subagents or Task/Agent tools.

Build conflict.rs + the conflict store, no UI beyond a banner stub:
1. Conflict-state detection per ARCHITECTURE §7.4: kind (merge/rebase/cherry-pick/revert/
   stash-apply), source/target labels with real branch names, file list, continue/abort
   commands with GIT_EDITOR=true.
2. The region computer per ARCHITECTURE §7.5: read :1/:2/:3 stages, similar-crate diffs against
   base, overlap→conflict-region / one-side→auto-resolved / else context. Include the
   edge dedupe of identical ours/theirs lines. NO conflict-marker parsing anywhere.
3. Region DTOs to the frontend + a keep-panel reducer store in TS: keep(block|line, source),
   unkeep, keep-both click-ordering, reorder, edit-region, reset-file, resolved-text assembly
   with reactive renumbering, per-file and per-operation progress derivations.
4. confirm_file (write + git add) and reopen_file (git checkout -m --) commands.
5. Tests, both layers: rust fixtures — both-modified, modify/delete each direction, both-added,
   identical-edges dedupe, marker-like content in code, three files at once; vitest — the
   reducer: keep/unkeep/both-ordering/reorder/renumber/nothing-kept-deletion.
Verify: cargo test + vitest green across all fixtures; a scripted merge conflict shows correct
region data in a debug dump; continue/abort work for merge AND cherry-pick kinds. Commit as
"feat: conflict engine — state detection, region computation, keep reducer".
```

---

## Prompt 13 — The Keep Panel

**📋 Jamie:** **Opus, high — the flagship.** This is the prompt most worth your Opus budget.
Test it yourself with a real conflict before accepting; the *feel* (animations, renumbering,
pins) is the product.

```
BranchKit session. Read CLAUDE.md, then DESIGN_SPEC.md §9 twice — every sentence is a
requirement — plus §15 items 20,21 and GITKRAKEN_WORKFLOWS.md §2.7 (the GK pain points this
replaces). The backend and reducer exist from prompt 12. No subagents or Task/Agent tools.

Build the Keep Panel UI exactly per DESIGN_SPEC §9:
1. Conflict banner (progress phrasing, disabled-Continue tooltip, Abort with exact-consequence
   confirm), Conflicted (n) section with progress rings, file tabs with dots + 400ms-beat
   auto-advance.
2. The single-panel document: real line numbers on real lines, region cards (inset, shadow,
   no line numbers, branch-name labels with yours/incoming small hints, color per spec,
   explicit deletion ghost blocks), Keep on blocks and hover-✓ on lines, the 180ms
   candidate→document animation with tint fade + rolling line numbers, pins with unkeep,
   keep-both click-order stacking with reorder handles, Edit escape hatch (distinct pin color),
   per-file bulk bar, n/p navigation with card pulse, floating Confirm file, live renumbering
   throughout (already reactive from the reducer — make it VISIBLE and smooth).
3. Keyboard map 1/2/b/u/e/n/p/Cmd+Enter + the ? cheat-chip. Syntax highlighting incl. tinted
   candidates. Never render git conflict markers.
4. Continue-merge inline message field + success toast; wire all four conflict kinds' banners.
Verify manually (scripted repo from prompt 12 fixtures): resolve a 3-file merge using only the
mouse, then only the keyboard; keep-both ordering follows click order and reorders; nothing-
kept collapses with the removal notice; abort restores pre-merge state; a rebase conflict shows
rebase-appropriate banner verbs. Commit as "feat: Keep Panel conflict resolution".
```

---

## Prompt 14 — File history, blame, worktrees

**📋 Jamie:** Sonnet, medium.

```
BranchKit session. Read CLAUDE.md, DESIGN_SPEC.md §6.3 and §5 (worktrees), ARCHITECTURE.md §7.1
worktree commands. No subagents or Task/Agent tools.

Build:
1. File History per DESIGN_SPEC §6.3: git log --follow -- <file> list (reuse graph row
   styling) + per-commit file diff; entered from diff-view buttons and file context menus.
2. Blame per §6.3: git blame --porcelain parsing (group line-runs by commit), gutter
   author-discs + relative dates, hover popover, click→History jump, History|Blame toggle pill.
   Virtualize for big files.
3. Worktrees: create-from-commit/branch dialog (path picker with suggested sibling folder,
   branch selector/creator), WORKTREES panel section (path, branch pill, status dot), open-as-
   tab, remove with dirty-check guard, prune. git worktree list --porcelain parsing + tests.
Verify: blame a 2k-line file smoothly; --follow survives a rename fixture; create a worktree,
commit in it in a second tab, remove it guarded. Commit as
"feat: file history, blame and worktrees".
```

---

## Prompt 15 — Settings, credential manager, GitHub integration

**📋 Jamie:** Sonnet, high. **Have your OAuth Client ID ready** (see "Before Prompt 15" at top).
Review the credential code personally — ARCHITECTURE §8's rules are the security contract.

```
BranchKit session. Read CLAUDE.md, DESIGN_SPEC.md §13 (dynamic-visibility rule is absolute),
§12, §15 items 22,23,31, ARCHITECTURE.md §8 (security contract — follow to the letter), §11.
No subagents or Task/Agent tools. My GitHub OAuth client_id is: <PASTE_CLIENT_ID>.

Build:
1. Settings window per DESIGN_SPEC §13: all six sections, reactive reveal animations, instant
   persistence (JSON in app config dir — NO secrets in it ever), every control with its
   one-line description. AI section fields render but providers activate in prompt 16.
2. Credential manager per ARCHITECTURE §8: keyring storage, the credential-helper subcommand
   speaking git's get/store/erase protocol, per-call helper injection (clearing system
   helpers), auth-failure → credential dialog → retry-once flow, SSH section (agent status,
   consent-gated ed25519 keygen, pubkey copy), Credentials settings UI (metadata only, masked,
   remove). Include the CI grep-for-secrets-in-logs test and URL userinfo scrubbing test.
3. GitHub per ARCHITECTURE §11 + DESIGN_SPEC §12: device-flow sign-in UI (huge code, copy,
   poll, avatar success), PULL REQUESTS panel section, CI dots on visible commit rows with
   checks popover (lazy, cached, rate-limit rules), PR side panel (incl. Merge… and
   checkout-PR-head via pull/{n}/head), Create-PR panel with prefills, the push-new-branch →
   Create-PR toast, silent degradation when disconnected.
Verify: full device-flow round trip; an HTTPS-PAT push with no system helper interference;
secrets absent from config/logs (run the greps); PR create/checkout/merge cycle on a scratch
repo; every settings field appears only when relevant. Commit as
"feat: settings, secure credential manager and GitHub integration".
```

---

## Prompt 16 — AI: local model manager, Ollama, remote APIs

**📋 Jamie:** Sonnet, high.

```
BranchKit session. Read CLAUDE.md, ARCHITECTURE.md §10 in full, DESIGN_SPEC.md §13 AI section
and §7 (✨ button behavior). No subagents or Task/Agent tools.

Build:
1. The AiProvider interface + three implementations per ARCHITECTURE §10: llama-server sidecar
   (pinned-version downloads with resume + sha256 + progress events, free-port spawn, /health
   poll, 5-min idle kill, on-exit + orphan-pidfile reaping), Ollama (tags listing, connection
   dot), Remote (OpenAI adapter + Anthropic adapter to the same interface), all streaming.
2. Settings → AI activation: the model card state machine (Download→progress w/ MBps+Cancel→
   Ready·Remove), Ollama block with auto-populated model dropdown, Remote block with format
   radio/URL/keyring-stored key/model/Test button with inline result. Dynamic visibility per
   DESIGN_SPEC §13 exactly.
3. The ✨ commit-message flow per DESIGN_SPEC §7: staged-diff prompt per ARCHITECTURE §10's
   template + truncation rules, token streaming into summary/description, regenerate/dismiss
   chips, unstaged-fallback hint, disabled-state tooltip now deep-linking correctly.
4. Tests: prompt truncation rules; Anthropic + OpenAI response parsing fixtures; download
   resume logic (mock server).
Verify: download the real model in-app (progress visible, cancel+resume works), generate a
message from a real staged diff end-to-end locally; point at a fake OpenAI endpoint fixture and
see the same UX; Remove deletes the GGUF and returns the card to Download state. Commit as
"feat: AI commit messages — local llama.cpp, Ollama and remote APIs".
```

---

## Prompt 17 — Polish pass, a11y, perf audit, release

**📋 Jamie:** **Opus, medium.** A sweep prompt: fresh eyes over the whole app against the spec.

```
BranchKit session. Read CLAUDE.md, then DESIGN_SPEC.md §14, §15 (the full 32-item checklist),
§11, §2.5 motion rules, ARCHITECTURE.md §13 budgets and §14 gotchas, §12 release pipeline.
No subagents or Task/Agent tools.

This is a verification-and-polish pass, not a feature pass:
1. Walk ALL 32 items of DESIGN_SPEC §15 against the running app. Produce docs/QA.md listing
   each item as pass/fixed/deviation-with-reason, fixing what you find as you go.
2. A11y sweep per §14: keyboard-only full workflow (open→branch→stage→commit→push), focus
   rings, aria-labels on every icon button, reduced-motion audit, AA contrast check on both
   themes (script it against the token values).
3. Perf audit per ARCHITECTURE §13 on a 20k-commit repo (clone a big OSS repo): measure
   startup-to-graph, scroll fps, watcher latency; fix the worst offender if any budget is
   missed and record numbers in docs/QA.md.
4. Cross-platform gotchas §14: verify the Windows/Linux items that are checkable from code
   (CREATE_NO_WINDOW, path handling, EOL-only diff note, libsecret degradation banner).
5. Release: release.yml with tauri-action (dmg/msi/AppImage+deb on tags), version 0.1.0,
   README expanded (real feature list, install per-OS, screenshots section, contributing +
   AGPL notes, "not affiliated with GitKraken" line), CHANGELOG.md.
Verify: CI green on all three OSes; tag v0.1.0-rc1 on a branch and confirm artifacts build.
Commit as "chore: v0.1.0 polish, accessibility and release pipeline".
```

---

## After v0.1.0 — the v2 shortlist (write prompts the same way)

Interactive rebase (drag-to-reorder/squash — design first in DESIGN_SPEC style), submodules,
integrated terminal, image diff modes (swipe/onion), commit signing (GPG/SSH) UI, GitLab
integration via the same provider seam, stash-single-file, light branch-cleanup assistant
("gone" upstreams). Keep the same rules: spec first, one session per prompt, no subagents.
