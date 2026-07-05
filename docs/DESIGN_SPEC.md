# DESIGN_SPEC.md — BranchKit UI/UX Specification

> **This is the source of truth for how BranchKit looks, feels, and behaves.** Every build prompt
> references sections of this file. When implementing, match this spec exactly unless it is
> physically impossible; if you must deviate, leave a `// SPEC-DEVIATION:` comment explaining why.
> Companion docs: GITKRAKEN_WORKFLOWS.md (what we're inspired by), ARCHITECTURE.md (how to build it).

---

## 1. Product identity & design principles

**Name:** BranchKit. **License:** AGPL-3.0. **Platforms:** macOS, Windows, Linux (day one).

**Principles — in priority order:**
1. **Truth, instantly.** The UI always reflects the real repo state within ~1s, without any
   refresh action. If an operation is in flight, show it in flight.
2. **The graph is the workbench.** Every object in the graph (commit, label, stash, WIP) is
   interactive: clickable, double-clickable, right-clickable, draggable where meaningful.
3. **No fear.** Every destructive action is either guarded, reversible, or both. Prefer
   reversible (safety nets, toast-undo) over guarded (confirm dialogs). Confirm dialogs are a
   last resort and must say exactly what will happen in plain words.
4. **Plain words, real names.** Never "ours/theirs", never raw git jargon as the primary label,
   never an icon without a tooltip. Branch names, file names, and verbs.
5. **Simple surface, deep power.** The default view shows only what daily work needs. Advanced
   power lives one right-click or one Cmd+K away — never deleted, just not shouting.

**Non-goals:** accounts, cloud services, telemetry, upsells, Gitflow ceremony, integrated terminal (v1).

---

## 2. Design tokens

Implement as CSS custom properties on `:root[data-theme="dark"|"light"]`. All components consume
tokens only — no hard-coded colors anywhere.

### 2.1 Color — dark theme (default)
```
--bg:          #12141c;   /* window background */
--surface:     #1a1d28;   /* panels */
--raised:      #222635;   /* cards, inputs, hover surfaces */
--overlay:     #2a2f42;   /* menus, popovers */
--border:      #2c3040;
--border-soft: #232736;
--text:        #e6e9f2;
--text-muted:  #8b91a7;
--text-faint:  #5c6274;
--accent:      #3ddc97;   /* BranchKit green — primary actions, checked-out branch */
--accent-dim:  #2aa876;
--info:        #6ea8ff;
--warn:        #ffb454;
--danger:      #ff5c5c;
--ahead:       #3ddc97;   /* ↑ outgoing */
--behind:      #ffb454;   /* ↓ incoming */
```
### 2.2 Color — light theme
Mirror the scale: `--bg:#f7f8fb; --surface:#ffffff; --raised:#eef0f6; --border:#d8dce8;
--text:#1c1f2b; --text-muted:#5c6274;` same accent/danger/warn hues darkened ~10% for contrast.
Both themes must pass WCAG AA for text on its surface.

### 2.3 Graph lane palette (8, cycled by lane index; same in both themes)
`#3ddc97 #6ea8ff #c084fc #f472b6 #ffb454 #22d3ee #a3e635 #fb7185`

### 2.4 File-status colors
added `#3ddc97` (＋) · modified `#ffb454` (✎) · deleted `#ff5c5c` (−) · renamed `#c084fc` (→) ·
conflicted `#fb7185` (‼) · untracked `#6ea8ff` (＋ hollow)

### 2.5 Type, spacing, shape, motion
- UI font: system stack (`-apple-system, Segoe UI, Ubuntu, sans-serif`); mono: `ui-monospace,
  SF Mono, Cascadia Code, JetBrains Mono, monospace`. Base 13px UI / 12px mono in diffs.
- Spacing scale 4/8/12/16/24. Radius: 6px controls, 10px cards/menus, pills fully rounded.
- Motion: 120ms ease-out for hovers, 180ms for panel/row transitions, 240ms max for anything.
  Respect `prefers-reduced-motion` (disable all non-essential animation).
- Icons: **Phosphor** (phosphor-svelte) only, 10–16px, always with tooltips (400ms delay). (Changed from Lucide at Jamie's request during the v0.1 polish pass.)

---

## 3. Window layout

Same skeleton as GK (see GITKRAKEN_WORKFLOWS §1 diagram): repo tabs → toolbar → [left panel |
graph | right panel] → status bar. Left and right panels are resizable (drag handle, min 220px,
double-click handle resets to default) and collapsible (chevron; collapsed left panel becomes a
thin rail of section icons). Panel sizes persist per… globally (not per repo).

### 3.1 Repo tabs
- Tab = repo name + current branch in muted small text below/beside (e.g. **uaw** `main`).
- Cmd+T → repo picker overlay (fuzzy search over recents + Open… + Clone…). Cmd+W closes tab,
  Cmd+1…9 jump, middle-click closes, drag to reorder. `+` button = same picker.
- A tab shows a subtle spinner overlay on its favicon-dot while a long operation runs in that repo.

### 3.2 Toolbar
`[repo ▾] [branch ▾]  ···  [Pull ▾(badge ↓m)] [Push (badge ↑n)] [Branch] [Stash ▾] [Pop]  ···  [🔍 filter] [⌘K]`
- **Pull** primary action = configured mode (default: fast-forward, else prompt). Dropdown:
  Pull (fast-forward if possible) / Pull (rebase) / Pull (merge) / Fetch all. Badge shows behind-count
  of current branch; button subtly emphasized when >0, normal (not disabled) when 0 — fetch is
  always allowed from the dropdown.
- **Push** badge shows ahead-count. When branch has no upstream, button reads **Publish** and
  pushing prompts (prefilled `origin/<name>`). Force-with-lease only via dropdown, never default.
- **Stash** stashes all WIP immediately; dropdown: "Stash with message…", "Stash including
  untracked". **Pop** pops latest; disabled with tooltip when no stashes.
- **Filter** (Cmd+F): filters graph rows by message/author/sha/branch; matches highlighted,
  non-matches dimmed (not removed — context preserved); Esc clears.

---

## 4. The commit graph

### 4.1 Rendering & structure
Canvas edges/nodes + DOM row overlay (see ARCHITECTURE §5). Fixed row height 28px. Columns:
BRANCH/TAG · GRAPH · MESSAGE · (toggleable: AUTHOR · DATE · SHA) via gear menu, widths draggable.

### 4.2 The WIP row
- Appears (animated 180ms slide-in) whenever working tree ≠ HEAD. Dashed hollow node, italic
  `// WIP` placeholder, change badges `✎4 −2 ＋1` colored per §2.4.
- **Inline commit summary editing:** clicking the `// WIP` text turns it into an input,
  live-synced (shared store) with the right panel's Commit Summary field, including the live
  72-counter rendered inline as small text. Enter moves focus to the right-panel description;
  Cmd+Enter commits.
- Clicking elsewhere on the WIP row selects it → right panel shows working-directory mode.

### 4.3 Commit rows
- Node with author avatar (gravatar-hash fallback to colored-initials disc; cache locally).
  Merge commits: smaller plain node, dimmer text.
- Summary in `--text`; if a description exists, ` — first line of description` appended in
  `--text-faint` (like GK). HEAD commit's row node has an accent ring.
- **Hover:** row highlight + the commit's first-parent lineage edges brighten ~20% while other
  lanes dim ~30% (120ms). Hovering also reveals a copy-sha ghost button at the row end.
- **Selection:** click. **Range/compare:** Cmd/Ctrl+click a second commit → right panel shows
  "Comparing A ↔ B" diff mode with a swap-direction button. Shift+click selects a range (v1:
  treated as compare endpoints; range actions like squash are v2).
- **Double-click a commit → checkout** (detached HEAD) after a lightweight inline confirm
  (see §4.6 — double-click must never silently detach).
- **Keyboard:** ↑/↓ or j/k move selection (graph auto-scrolls); Enter opens the selected
  commit's first file diff; Space toggles right-panel focus; type-to-filter jumps to Cmd+F.

### 4.4 Ref labels (branch pills) — *handle with care (Jamie flagged this)*
- Pill in the BRANCH/TAG column, colored border matching lane, background `--raised`.
  Contents, left→right: `[✓ if checked out] name [💻][☁] [↑n ↓m]`.
- **Presence icons:** 💻 laptop = local exists; ☁ cloud = remote exists (tooltip: "origin/name").
  Same-commit local+remote share one pill. **Diverged:** pills split; the *local* pill keeps the
  ahead/behind badge; a dotted short connector is NOT drawn (visual noise) — divergence is
  communicated by the badge alone.
- **Ahead/behind badge rules (the "careful" part):**
  - Only rendered when nonzero. `↑n` in `--ahead`, `↓m` in `--behind`. Never percentages, never
    combined into one number.
  - Tooltip on hover lists up to 5 incoming/outgoing commit summaries ("↓ 3 to pull: …").
  - Clicking the badge opens a mini-popover with actions: Pull / Push / view commits — so the
    badge is not just an indicator, it's the fix.
  - Diverged (both ↑ and ↓): badge gets a subtle warn-tinted background; popover explains and
    offers "Pull (rebase)" / "Pull (merge)" / "Force push (with lease)…" with one-line
    consequences under each.
  - Counts come from the local refs (upstream tracking), refreshed after every fetch — they must
    never be stale relative to the graph shown.
- Multiple branches on one commit: show first 2 pills + `+3` overflow pill; click overflow →
  popover listing all (each with full pill behavior). The checked-out branch always wins a slot.
- **Interactions:** single-click pill → select commit & flash-scroll if off-screen (from panel);
  **double-click → checkout** (remote pill: create tracking branch + checkout, one action —
  the sacred GK workflow); right-click → branch menu (GITKRAKEN_WORKFLOWS §3.2);
  **drag pill onto another pill or row** → drop menu: "Merge X into Y" / "Rebase X onto Y" /
  "Fast-forward Y to X" (only when valid, listed first when possible). While dragging, valid
  drop targets glow; invalid rows dim. Esc cancels.
- Tags: square-cornered label, `--text-muted` border, 🏷 prefix.

### 4.5 Stash rows
Inline at their base commit: box icon node, dashed connector, label "stash: {message|WIP on
branch}" in italic muted text. Single-click → right panel shows stash contents (diff list).
Double-click → **Pop** with toast-undo ("Popped stash — Undo" = re-stash same contents).
Right-click: Apply / Pop / Drop… (confirm) / Copy patch to clipboard.

### 4.6 Detached HEAD & destructive guards in the graph
- Double-click commit → small anchored popover (not a modal): "Check out this commit? You'll be
  in a detached state." [Check out] [Create branch here…]. Remembers "don't ask again" checkbox.
- While detached: a slim amber banner under the toolbar: "Detached at `a1b2c3d` — changes here
  can be lost." Buttons: **[Create branch here] [Back to `main`]** (back = checkout previous ref).
- Reset Hard, force push, branch delete with unmerged commits, discard-all: typed-word confirms
  are NOT used (annoying); instead a red confirm button with a 400ms arm delay + explicit
  consequence sentence, and where possible a safety net (§7.4, §8).

### 4.7 Loading & scale behavior
Initial paint: first 2,000 commits; skeleton shimmer rows while loading; background-load the
rest in chunks (ARCHITECTURE §5.4). Viewport never jumps on refresh — anchor to the topmost
visible commit sha. Target: buttery at 20k commits.

---

## 5. Left panel

Sections (collapsible, counts in headers): **LOCAL · REMOTES (per remote) · PULL REQUESTS ·
TAGS · STASHES · WORKTREES**, with one filter box at top that filters every section live
(fuzzy, highlights matches, auto-expands sections with hits).
- Rows: name + (for locals) ahead/behind badge + hidden-eye toggle on hover (hide from graph).
- Checked-out branch: accent dot + bold. Hovering a branch row **highlights its pill + tip
  commit in the graph** (subtle glow); clicking scrolls the graph to its tip; double-click
  checks out. Right-click menus per GITKRAKEN_WORKFLOWS §3.
- Remote branch rows show ☁ only; if a local tracks it, shown nested/merged under LOCAL instead
  of duplicated (setting: "Combine tracking branches" default ON — this de-noises the panel;
  GK duplicates and it's clutter).
- PULL REQUESTS section (when GitHub connected): open PRs for this repo — `#83 Fix memory leak`
  + CI dot + author avatar; click → PR panel (§12); branches with PRs also get a small PR icon
  in their pill tooltip area.
- WORKTREES: path + branch pill; click → open as tab; right-click: Open / Remove… / Prune all.
- Drag a local branch row onto another → same merge/rebase drop menu as pills.

---

## 6. Diff & staging views

### 6.1 File list (right panel, working-directory mode)
- Header: "N changes on `branch`" + Discard All (trash, guarded §7.4) + **Path/Tree toggle**
  (persisted). Tree mode: folders collapsible with rolled-up badges (`✎4 −1` on `web/`).
- Sections: **Unstaged (n)** [Stage All] / **Staged (n)** [Unstage All]. File row: status
  glyph (§2.4) + name + dimmed parent path; hover reveals [Stage/Unstage] button and overflow
  ⋯ menu (= right-click menu). **Space** stages/unstages the keyboard-selected row. Click opens
  the diff. After staging a file, keyboard selection moves to the next unstaged file —
  so Space-Space-Space walks the list.
- A file that is both partially staged shows in both sections with a half-filled glyph.

### 6.2 Diff viewer (replaces graph center; breadcrumb "← Graph / path/to/file")
- Unified view default; toggle to split. Syntax highlighting; intra-line word-diff highlights;
  whitespace-changes toggle; hidden-whitespace indicator when active.
- **Hunk header bar:** `@@ context @@` + [Stage hunk] [Discard hunk…] buttons.
- **Line staging:** hovering the gutter shows a checkbox per changed line; click = stage line;
  click+drag down the gutter = stage a run of lines. Staged lines get an accent gutter tick.
  Same mechanics in reverse in the Staged view (unstage).
- Large files: hunks beyond 400 visible lines collapse ("… 213 unchanged lines — expand");
  files >1MB or binary: no inline diff, offer "Open in external tool". Images: side-by-side
  before/after with checkerboard background and dimensions.
- Every diff view shows [File History] [Blame] [Open file] buttons top-right.

### 6.3 File history & blame
History: left column = commit list (filtered to file, same row styling as graph), right = diff
of selected commit for that file; follow-renames ON. Blame: full file, gutter shows
author-initials disc + relative date per line-run, hover → popover with commit summary,
click → jumps to that commit in History mode. Toggle pill [History | Blame] top center.

---

## 7. Commit composer (right panel, bottom)

- **Summary input** with live counter: shows remaining-to-72 (e.g. `36`) in `--text-faint`,
  turns `--warn` at ≤10 remaining, `--danger` and counts negative past 72 (never blocks).
  Synced two-way with WIP row inline editor (§4.2).
- **Description** textarea grows to 8 lines; soft ruler line at col 72; Markdown allowed, not
  rendered.
- **✨ AI button** inside the summary field edge: generates summary+description from the
  *staged* diff (or all WIP if nothing staged — with a hint "Generated from unstaged changes"),
  streams tokens into the fields, [↻ regenerate] [✕ dismiss] chips appear after. Disabled with
  tooltip when no AI provider is configured, and the tooltip deep-links to Settings → AI.
- **Amend checkbox:** prefills HEAD message on tick (restores your draft on untick — don't lose
  it). If HEAD is already on the remote: inline amber note "This commit is on `origin/main` —
  amending will require a force push."
- **Primary button states:** nothing staged & no WIP → hidden; WIP but nothing staged →
  `Stage all & commit` (split-button: caret offers "Commit staged only"); staged files →
  `Commit N files to main`. Cmd+Enter always triggers it. After commit: button plays a 240ms
  success sweep, fields clear, WIP row animates into the new commit row.
- **7.4 Discard safety net:** every Discard (file/hunk/all) writes a patch (+ copies of
  untracked files) to the app's trash before touching the tree (ARCHITECTURE §7.3). Toast:
  "Discarded 3 files — **Undo**". A "Recently discarded" item lives in the repo menu; entries
  auto-purge after 7 days. Discard confirms are therefore soft (single click, arm-delay only
  for Discard All).

---

## 8. Toasts & undo affordances (replaces GK's global Undo)

Bottom-left stack, max 3, 6s timeout (destructive ones: 10s), hover pauses. Every toast: icon,
one sentence, and **at most one action verb**. Canonical catalog:

| After | Toast + action |
|---|---|
| Checkout | "Switched to `feature/x` — **Back**" (checkout previous ref) |
| Branch delete | "Deleted `old-branch` — **Undo**" (recreate at recorded sha) |
| Stash pop/apply conflict | "Stash conflicts with working tree — **Resolve**" |
| Discard | "Discarded N files — **Undo**" (apply trash patch) |
| Commit | "Committed `a1b2c3d` to `main` — **Undo**" (soft reset, only until pushed) |
| Push new branch | "Published `feature/x` — **Create pull request**" (when GitHub connected) |
| Fetch found new commits | "`main` is 3 behind — **Pull**" (only for current branch, max 1/minute) |
| Merge/rebase/cherry-pick done | "Merged `feature/x` into `main` — **View**" (scroll to commit) |

Failures use the same shape with a **Details** expander revealing raw git output (§11).

---

## 9. Conflict resolution — the **Keep Panel** (flagship feature)

**Mental model (Jamie's design):** during a conflict, the disputed lines are *not in the file*.
The user is not "editing a conflict"; they are **choosing what to keep**. Kept lines become real
file lines (they gain line numbers and lose their tint); everything still tinted is merely a
*candidate*.

### 9.1 Entry & framing
- Conflict state (merge/rebase/cherry-pick/revert/stash-apply) → persistent banner under
  toolbar: "Merging `feature/x` into `main` — 2 of 5 conflicts resolved · 1 of 2 files done"
  with [Continue merge] (disabled until all resolved, tooltip says what's left) and [Abort…].
- Conflicted files listed in right panel under **Conflicted (n)** with ‼ glyph and a per-file
  progress ring. Clicking opens the Keep Panel in the center area.
- File tabs across the Keep Panel top when multiple files conflict, each with progress dots
  (● resolved ○ pending); auto-advance to next file when one completes (with a beat — 400ms —
  so it never feels yanked).

### 9.2 The panel — ONE panel, the future file
- The file rendered top-to-bottom exactly as it will exist, syntax highlighted, real line
  numbers on resolved/normal lines.
- A **conflict region** renders as a card-inset in the flow: soft raised background, no line
  numbers, containing two labeled candidate blocks:
  - `⬤ From main (yours)` — blue-tinted block (`--info` at 12% alpha, 3px left bar)
  - `⬤ From feature/x (incoming)` — purple-tinted block (`#c084fc` at 12% alpha)
  - **Always actual branch names.** "yours/incoming" only as small suffix hints.
  - If one side is empty (deletion), render an explicit ghost block: "`main` deletes these
    lines" with a keep-the-deletion affordance — absence must be selectable, never implicit.
- **Keeping:** each block has a `Keep` button on its label bar, and every candidate line has a
  hover ✓ in its gutter (keep single line). Click Keep → the block/lines animate (180ms) out of
  the card into the document flow: tint fades, real line numbers roll in, and a subtle **pin**
  (📌-style dot) remains in the gutter marking "kept, not yet confirmed".
- **Unkeeping:** click a pin (or the line, then `Unkeep` chip) → line animates back into the
  candidate card. Until the file is confirmed, everything is fluid.
- **Keep both:** keep blocks in any order — kept lines stack in **click order** at the region
  site. Small ↑↓ handles on kept runs allow reorder while pins are present. (Click-order is the
  creative bet: it reads as "I'm building the file"; document-order fallback is one setting away.)
- **Nothing kept + region confirmed** = both sides deleted. Legal, explicit: region collapses
  with message "Nothing kept — lines removed", undoable via pin history until confirm.
- **Hand-edit escape hatch:** an `Edit` chip on each region converts the *kept result* into a
  small editable text area (monospace, same highlight) for manual tweaks — because sometimes the
  right answer is a mix. Editing marks the region resolved-by-hand (distinct pin color).
- **Per-file bulk actions** top bar: `Keep all from main` / `Keep all from feature/x` /
  `Reset file` (back to all-candidates). Global bulk equivalents live in the banner's ⋯ menu.
- **Navigation:** `n`/`p` (and toolbar chevrons) jump between unresolved regions with a smooth
  scroll + brief card pulse. Progress "3/5" between the chevrons.
- **Confirm file** button (accent, bottom-right, floats): enabled when no unresolved regions.
  On confirm: pins fade, file is staged (git add), tab dot goes ●, auto-advance. A confirmed
  file can be reopened (`Reset file`) until the merge is finalized.
- **Continue merge** (banner): commits with prefilled message ("Merge branch 'feature/x'…"),
  editable in a compact inline field — not a modal. Then a success toast (§8).

### 9.3 Details that make it feel right
- Region cards cast a slight shadow = "floating above the file, not yet part of it."
- The document's line numbers are *live*: keeping a 3-line block immediately renumbers
  subsequent lines — the file visibly "heals" as you work. This is the core feedback loop;
  implement numbering reactively, never on-confirm.
- Identical-on-both-sides lines within a region (common in marker noise) are pre-deduplicated:
  shown once with a merged label "same in both" and auto-kept (pin, undoable) — reduces silly
  choices. (Parser detail: ARCHITECTURE §7.5.)
- Keyboard: `1` keep-current-region-from-first-side, `2` second side, `b` both, `u` unkeep-all
  in region, `e` edit, `n/p` navigate, `Cmd+Enter` confirm file. A `?` chip shows this map.
- Never show `<<<<<<<`/`=======` markers anywhere in the UI.
- Abort is always one click away but confirms with the exact consequence: "Abort merge and
  return `main` to `e4f5a6`? Your resolved choices in 2 files will be lost."

---

## 10. Command palette & keyboard

Cmd+K overlay. Sections in order: **Actions** (context-aware: "Commit staged", "Pull",
"Create branch…", "Resolve conflicts"), **Branches** (`↵` checkout, `Cmd↵` merge into current),
**Changed files** (open diff), **Repos** (recents). Fuzzy match with highlighted characters;
every action row shows its shortcut, teaching the keymap passively.

Global map (also shown in menus): Cmd+K palette · Cmd+T repo picker · Cmd+F filter graph ·
Cmd+Enter commit · Cmd+P pull · Cmd+Shift+P push · Cmd+B new branch · Cmd+S stash · Cmd+Shift+S
pop · j/k rows · Space stage · n/p conflicts · Cmd+, settings · Cmd+1…9 tabs.

---

## 11. Errors, empty states, onboarding

- **Error translation layer** (ARCHITECTURE §9): every common git failure maps to a human
  sentence + suggested action button; raw stderr always available behind "Details". Examples:
  non-fast-forward push → "`main` has new commits on origin — **Pull first**"; auth 403 →
  "GitHub rejected your credentials — **Open credential settings**"; `index.lock` → "Another
  git process is running (editor?). **Retry**".
- **Empty states:** no repo → hero card: Open / Clone / recents grid (with last-opened times).
  Empty graph (fresh repo) → "No commits yet — stage files and make the first one" with arrow
  toward the right panel. No changes → right panel shows a quiet "Working tree clean ✓" +
  last commit card (with Amend shortcut).
- **First launch:** theme choice (respects OS, offers both), then a git identity check — if
  `user.name`/`user.email` unset, inline form (writes global config) — this is the #1 new-user
  papercut in every git client. Then open/clone. Three cards, ~20 seconds, skippable.
- **Clone flow:** URL paste auto-detects provider & suggests folder name; progress bar with
  phase text (counting/compressing/receiving); on done, "Open now" primary.

---

## 12. GitHub integration (v1 scope)

- **Sign-in:** Settings → Integrations → GitHub → device flow: show the 8-char code huge, [Copy]
  [Open github.com/login/device], poll quietly, success state with avatar + username. Token in
  OS keychain. Sign out = revoke locally.
- **Surfaces once connected:** PULL REQUESTS panel section (§5); PR badges in branch pill
  tooltips; **CI dots** on commit rows (tiny 6px dot right of message: green/red/amber-pulse
  pending; hover → checks list popover with per-check status + "Open in browser"); after
  pushing a new branch → toast "**Create pull request**".
- **PR side panel** (replaces right panel when a PR is selected): title, state chip, branches
  `feature/x → main`, CI checks list, description (rendered markdown), comment count, reviewers;
  buttons: **Open in browser** (primary), **Merge…** (when green & permitted; choose
  merge/squash/rebase per repo settings), **Checkout branch** (fetches & checks out PR head —
  works for fork PRs via `pull/<n>/head`).
- **Create PR panel:** base/head pickers (prefilled), title (prefilled from branch name
  Title-Cased or single-commit summary), body (prefilled with commit list), [Create] →
  toast with PR number + open action. Never auto-opens the browser.
- All GitHub features degrade invisibly when not connected — no nags, one quiet "Connect
  GitHub" row in the PULL REQUESTS section.

---

## 13. Settings (dynamic — Jamie's requirement)

Single window (Cmd+,), left nav: General · Appearance · Git · Credentials · AI · Integrations.
**Dynamic visibility rule:** controls appear/disappear *reactively* based on sibling values
(180ms height animation). Never show a disabled irrelevant field. Persist instantly (no
Save button); every control has a one-line description under it.

- **General:** auto-fetch interval (off/1/5/15 min), open-last-repos-on-launch, default clone dir.
- **Appearance:** theme (System/Dark/Light), graph density (Comfortable 28px / Compact 24px),
  date style (relative/absolute), show avatars.
- **Git:** default pull mode, push tags with commits, prune on fetch, combine tracking branches
  (§5), commit-summary guide length (72 default).
- **Credentials:** list of stored HTTPS credentials (host + username + last used) with
  remove buttons; SSH section: detected keys/agent status read-only + "Generate new key…"
  helper (never displays private material; ARCHITECTURE §8).
- **AI:** master switch "Enable AI commit messages" (off → nothing else visible). Provider
  radio: **Local · Ollama · Remote API** — each reveals only its own block:
  - **Local:** model card "Gemma 3 1B (Q4, ~800 MB)" with state machine: [Download] →
    progress bar (%/MBps, Cancel) → [✓ Ready · Remove]. Disk path shown. "Runs entirely on
    this machine" note.
  - **Ollama:** URL field (default `http://localhost:11434`), live connection dot, model
    dropdown auto-populated from `/api/tags`, Refresh.
  - **Remote API:** format radio (OpenAI-compatible / Anthropic) → base URL + API key
    (masked, stored in OS keychain, never in config files) + model name + [Test] button with
    inline ✓/✗ result.
  - Shared: "Style" select (Plain / Conventional Commits) + max diff size slider.
- **Integrations:** GitHub connect/disconnect (§12).

---

## 14. Accessibility & quality bar

Full keyboard operability (graph included — DOM row overlay makes rows focusable); visible focus
rings (2px accent); aria-labels on all icon buttons; the canvas layer is decoration —
semantics live in the DOM rows; AA contrast both themes; hit targets ≥24px; tooltips on
everything iconic; no information conveyed by color alone (glyphs accompany status colors).

**Definition of "feels right":** startup → interactive graph <1.5s on a 5k-commit repo;
watcher-to-UI latency <1s; 60fps scroll at 20k commits; zero layout shift after initial paint.

---

## 15. The little things — consolidated checklist

The micro-decisions above, in one place (build prompts cite these by number):

1. Double-click branch pill / panel row → checkout; remote pill → track + checkout in one action.
2. Double-click commit → detach, with anchored popover guard + "Create branch here…" alternative.
3. WIP row inline commit-message editing, live-synced with composer + inline 72-counter.
4. Hover commit → lineage brightens, other lanes dim; ghost copy-sha button.
5. Cmd+click two commits → instant compare mode with swap button.
6. Drag branch pill onto branch → merge/rebase/ff drop menu; targets glow while dragging.
7. Ahead/behind badges are **buttons**: click opens fix-it popover (Pull/Push/view commits).
8. Diverged branch = warn-tinted badge + explained options, never a scary modal.
9. Pull/Push toolbar badges mirror current branch counts; Push becomes **Publish** when no upstream.
10. Space stages selected file; selection auto-advances → Space-Space-Space to stage a list.
11. Gutter click+drag stages line ranges; staged lines get accent ticks.
12. Discard anything → recoverable for 7 days ("Recently discarded"), toast Undo.
13. Commit toast Undo = soft reset (until pushed). Branch-delete toast Undo = recreate at sha.
14. Checkout toast → **Back** (checkout previous ref).
15. Amend warns inline when HEAD is already pushed; unticking amend restores your draft message.
16. Commit button is stateful: hidden / "Stage all & commit" / "Commit N files to `branch`".
17. 72-counter counts down, warns, never blocks.
18. Stash double-click = Pop with Undo toast; stash naming at creation.
19. Fetch-found-new-commits toast (current branch only, rate-limited) with **Pull** action.
20. Keep Panel: single-panel merge; branch names not ours/theirs; live renumbering; pins;
    keep-both stacks in click order with reorder handles; explicit deletion ghost blocks;
    dedupe "same in both" lines; keyboard 1/2/b/u/e/n/p; per-file confirm + auto-advance.
21. Conflict banner shows progress ("2 of 5 conflicts · 1 of 2 files") and disabled-Continue
    tooltip says exactly what's left.
22. Push of new branch → "Create pull request" toast (GitHub connected).
23. CI dots on commit rows with checks popover.
24. Branch filter box filters graph *and* all panel sections; graph filter dims, never removes.
25. Hover branch in panel → its pill glows in graph; click scrolls to tip.
26. Combine local+remote tracking rows in panel (setting, default ON).
27. Column gear menu (Author/Date/SHA toggles) like GK; widths persist.
28. First-launch git identity check with inline fix.
29. index.lock and friends translated to human errors with Retry.
30. Every context-menu item shows its shortcut; palette teaches the keymap.
31. Settings reveal only relevant fields (AI provider blocks, master switches).
32. Viewport anchoring: refreshes never move what you're looking at.
