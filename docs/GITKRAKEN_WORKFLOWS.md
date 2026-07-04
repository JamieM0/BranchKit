# GITKRAKEN_WORKFLOWS.md — The GitKraken Reference for BranchKit

> **Purpose.** BranchKit is a free, open-source (AGPL-3.0) git client heavily inspired by GitKraken's
> workflows and visual clarity. This document is the definitive reference for *how GitKraken works* —
> screen by screen, workflow by workflow — so any Claude session building BranchKit understands exactly
> which interactions we are preserving, and which pain points we are fixing. It is based on GitKraken
> Desktop ~12.x on macOS and annotated screenshots of a real working session.
>
> **How to read this.** "GK" = GitKraken behavior we are replicating in spirit. "BK" notes = where
> BranchKit deliberately deviates (full details live in DESIGN_SPEC.md).

---

## 1. Interface anatomy

GitKraken's window is organized into fixed regions. BranchKit adopts the same skeleton.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ ① Repo tabs:  [Launchpad] [Release Notes] [repoA] [repoB] [uaw ×] [+ New Tab] │
├──────────────────────────────────────────────────────────────────────────────┤
│ ② Toolbar: repo▾ branch▾ | Undo Redo | Pull▾ Push Branch Stash Pop | Terminal │
├───────┬────────────────────────────────────────────────┬─────────────────────┤
│ ③Left │ ④ Commit graph (center)                        │ ⑤ Right panel       │
│ rail+ │   BRANCH/TAG | GRAPH | COMMIT MESSAGE columns  │  - file changes     │
│ panel │   WIP row at top, then commit rows             │  - commit details   │
│       │                                                │  - commit compose   │
├───────┴────────────────────────────────────────────────┴─────────────────────┤
│ ⑥ Optional embedded terminal panel                                           │
├──────────────────────────────────────────────────────────────────────────────┤
│ ⑦ Status bar: notifications | trial/upsell | zoom | support | version        │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 1.1 Repo tabs (①)
- Multiple repositories open simultaneously, each in a tab with the repo name. Tabs are closable
  (×) and reorderable. `+` opens a new tab showing open/clone/recent options.
- GK also puts non-repo surfaces in tabs (Launchpad, Release Notes). **BK:** repo tabs only in v1.
- Each tab preserves its own full state (selection, scroll position, panels).

### 1.2 Toolbar (②)
- **Repo & branch selectors** (top-left): dropdown showing current repo and current branch; the
  branch dropdown is a quick way to checkout.
- **Undo / Redo:** GK's famous safety buttons — undo the last commit/checkout/branch delete etc.
  **BK:** deferred (per scope decision); we instead put targeted "undo" actions in toasts
  (see DESIGN_SPEC §8).
- **Pull** (with dropdown: fast-forward only / merge / rebase; the dropdown remembers the default),
  **Push**, **Branch** (create branch at HEAD), **Stash** (stash all WIP), **Pop** (pop latest stash).
- **Terminal** toggle. **BK:** deferred to v2.
- Buttons show spinners while their operation runs; the whole toolbar never blocks the UI.

### 1.3 Left rail + left panel (③)
- A thin icon rail with counters: local machine (workspaces), cloud (remotes), notifications,
  Pull Requests (e.g. "4"), Issues, Focus view, To-dos, Team members. Clicking expands a panel.
- The expanded left panel lists, in collapsible sections with a filter box at the top:
  **LOCAL** branches, **REMOTE** (per remote, e.g. `origin`, with nested branches), **PULL REQUESTS**
  (from GitHub integration), **TAGS**, **STASHES**, **SUBMODULES**.
- The checked-out branch has a checkmark. Hovering any branch reveals a "hide" (eye) toggle to
  filter it out of the graph. Double-click a branch → checkout. Right-click → full branch menu.
- **BK:** same section layout (minus submodules, plus WORKTREES), same filter-first design.

### 1.4 The commit graph (④) — the heart of the app
Three columns: `BRANCH / TAG` (ref labels), `GRAPH` (lanes/edges/nodes), `COMMIT MESSAGE`
(summary text, plus extended description preview in dimmer text after the summary).

**Rows, top to bottom:**
- **WIP row (always first when there are uncommitted changes):** shows a dashed/hollow node,
  the text `// WIP` in italics, and change badges (e.g. `✎ 4  − 2` = 4 modified, 2 deleted).
  - Clicking the WIP row focuses the right panel on the working-directory changes.
  - **The WIP row is directly editable:** click into the `// WIP` text and type — you are typing
    the commit summary *inline in the graph*, and it live-syncs with the Commit Summary field in
    the right panel (screenshot 3 shows "Writing a commit message right here!" typed in the graph
    while the right panel shows the same text and its live character count "36"). This is a
    beloved micro-feature. **BK keeps it.**
- **Commit rows:** author avatar on the node, commit summary, optional appended description
  preview. Merge commits render smaller/dimmer nodes. The current HEAD's row is connected to WIP.
- **Stash rows:** stashes appear *inline in the graph* at the commit they were created from, with
  a distinct box icon and dashed connector. Right-clicking gives the stash menu (§5.3).
- **Branch labels** in the BRANCH/TAG column: pill-shaped, colored by lane. A label shows the
  branch name plus **presence icons**: a laptop icon (exists locally) and/or a cloud/avatar icon
  (exists on remote). When local and remote point at the same commit they share one pill; when
  they diverge, the pills split to their respective commits. The checked-out branch pill has a
  checkmark. Tags get a tag-shaped label.
- **Lane edges** are colored bezier curves; merges curve into the parent lane.

**Graph interactions (GK):**
- Single-click commit → selects it; right panel switches to commit detail (changed files, stats).
- Click WIP row → right panel switches back to working-directory view.
- **Double-click a branch label → checkout** that branch. Double-click a *remote* branch label →
  creates a local tracking branch and checks it out (one action). This is the "view remote
  branches inline and quickly check them out" workflow that must be preserved.
- **Drag a branch label onto another branch label** → contextual menu appears offering:
  *Merge X into Y*, *Rebase X onto Y*, *Fast-forward*, *Create pull request*. This drag-to-merge
  is a GK signature. **BK keeps drag-to-merge/rebase.**
- Right-click anywhere → context menus (§5).
- Hovering a commit highlights its row; the graph subtly emphasizes related history.
- Ctrl/Cmd-click selects a second commit → right panel shows the diff *between* the two commits.
- Scrolling is virtualized and smooth even for large repos. A soft "ghost" row at the very top
  (dotted circle) represents where a new commit will land.

### 1.5 Right panel (⑤)
Two modes:

**A. Working-directory mode** (WIP selected or files changed):
- Header: "N file changes on `branch`" + a trash icon (discard all, guarded).
- **Path / Tree toggle** for the file list (flat paths vs. collapsible folder tree with per-folder
  change badges like `✎ 4 − 1`).
- **Unstaged Files (n)** section with **Stage All Changes** button and per-file hover actions;
  **Staged Files (n)** section with Unstage All. Each file row: status icon (color-coded:
  green added, yellow/pencil modified, red minus deleted, purple renamed), filename, and on
  hover a Stage/Unstage button. Clicking the row opens the diff view in the center area.
- **Commit composer** (bottom): tabs for Commit; an **"Amend previous commit"** checkbox;
  **Commit Summary** input with a live character counter counting down from 72 (turns
  amber/red as you exceed); **Description** textarea; **"Compose commits with AI"** button
  (✨ icons also appear next to the summary field); collapsed **Commit options**; and the big
  primary button **"Stage Changes to Commit"** which contextually becomes
  **"Commit changes to N files"** once files are staged.

**B. Commit-detail mode** (a commit is selected):
- Commit metadata (author, date, sha with copy button, parents), full message, list of changed
  files with the same status icons; clicking a file shows its diff for that commit.

### 1.6 Diff / file view (center overlay)
Clicking a file replaces the graph with a diff view: unified or split (inline/hunk view toggle),
syntax highlighted, with per-hunk **Stage hunk** buttons and per-line staging by clicking the
`+`/gutter, "Discard hunk", and for the whole file: Stage/Discard/History/Blame buttons.
A back arrow / breadcrumb returns to the graph. **BK keeps all of this** (see DESIGN_SPEC §6).

### 1.7 Status bar (⑦)
Notifications ("JamieM0/Perspectives#83 needs your review"), zoom %, support, version.
**BK:** minimal status bar — background task indicator (fetch spinner), current operation, zoom.

---

## 2. Core daily workflows (step-by-step, as GK does them)

### 2.1 Open / clone a repository
1. `+` tab → Open repo (file browser), Clone (URL + destination + auth), or pick from recents.
2. Clone shows progress; on completion offers "Open now". Auth failures prompt for credentials
   (GK integrates OAuth with GitHub/GitLab/etc; BK v1: PAT/SSH via credential manager + GitHub OAuth).

### 2.2 The commit loop (the workflow used 50 times a day)
1. Edit files in your editor. GK's WIP row appears/updates automatically within ~1s (filesystem
   watcher — no manual refresh, ever).
2. Click the WIP row. Review changed files in right panel; click files to inspect diffs.
3. Stage: per file (hover → Stage), all (Stage All), per hunk / per line (in diff view).
4. Type summary (counter guides ≤72) + optional description; or type directly on the WIP graph row.
5. Press the commit button (or Cmd+Enter). The WIP row becomes a real commit; graph animates.
6. **Amend flow:** tick "Amend previous commit" → summary/description prefill from HEAD; commit
   button becomes "Amend Previous Commit". GK does not warn if the commit was already pushed. 
   **BK improves:** warn when amending a pushed commit (see DESIGN_SPEC §7).

### 2.3 Branching
- **Create:** toolbar Branch button or right-click a commit → "Create branch here"; type name
  inline in the graph's label column; Enter checks it out immediately.
- **Checkout:** double-click branch label (graph) or branch name (left panel); or branch dropdown
  in toolbar.
- **Checkout a remote branch:** double-click the remote branch label → GK creates a local branch
  with the same name tracking it and checks out. If a local with that name exists, it just
  checks out and sets upstream if unset.
- **Rename / Delete:** right-click branch → rename (inline edit) / delete (guarded if unmerged;
  deleting a branch with an open remote counterpart offers to delete the remote branch too).

### 2.4 Sync: fetch / pull / push
- GK auto-fetches on an interval (default 1 min) and on window focus; branch pills update with
  **ahead/behind badges** (`↑n` commits to push, `↓m` to pull) next to the branch name.
- **Pull:** toolbar button uses the configured mode (ff-only / merge / rebase). If diverged and
  ff-only fails, GK surfaces a dialog explaining and offering merge/rebase.
- **Push:** pushes current branch to its upstream; if no upstream, prompts to create
  `origin/<name>` (prefilled, editable). Force push is behind the dropdown and shows a strong
  warning dialog. GK uses `--force` here; **BK always uses `--force-with-lease`.**
- Failures (auth, non-fast-forward, no network) appear as toasts with a "View details" expander.

### 2.5 Stash
- **Stash** button stashes all WIP (GK stashes tracked changes; untracked handling is a setting).
  The stash appears inline in the graph immediately.
- **Pop** button pops the latest stash. Right-click a stash row → Apply / Pop / Delete /
  Edit stash message / Share as Cloud Patch / Hide (screenshot 4). Apply keeps the stash; Pop
  deletes on success; conflicts during apply put the repo in conflict state (§2.7).
- **BK:** name-on-create option, Apply/Pop/Drop, no cloud patches. "Edit stash message" is not
  native git; BK offers naming at creation instead.

### 2.6 Rewriting history (commit context menu, §5.1)
- **Cherry-pick commit:** right-click any commit (e.g. on another branch) → applies onto HEAD.
  Conflicts → conflict state.
- **Revert commit:** creates the inverse commit; GK asks whether to commit immediately or leave
  staged.
- **Reset `main` to this commit:** submenu Soft (keep changes staged) / Mixed (keep changes
  unstaged) / Hard (discard — strong warning).
- **Rebase `main` onto this commit**, **Interactive rebase** (BK: v2), **Squash** children, etc.
- **Create tag / annotated tag here.**
- **Copy commit sha**, **Copy link to this commit on remote** (builds the GitHub URL),
  **Create patch from commit**, **Compare commit against working directory**,
  **Checkout this commit** (detached HEAD — GK shows the detached state as a temporary label),
  **Create worktree from this commit**.

### 2.7 Merging & conflicts
1. Drag branch A onto branch B (or right-click branch → "Merge A into B", with B checked out).
2. Fast-forward happens silently when possible.
3. On conflict, GK: banner across the top ("Merge in progress — resolve conflicts"), conflicted
   files listed in the right panel in a **Conflicted Files** section (warning icons), and the
   WIP area shows both branches' state. Buttons: **Abort merge** / (after resolving) **Commit
   and Merge** with a prefilled merge message.
4. Clicking a conflicted file opens GK's **merge tool**: three panes — "ours" (top-left),
   "theirs" (top-right), output (bottom); checkboxes on each conflict block on each side; the
   output pane assembles the result; per-block checkboxes and an editable output buffer.
   - **Known pain points (why BK redesigns this — see DESIGN_SPEC §9):** three panes force
     eye-jumping; checkbox semantics are unclear ("checked" = included, but ordering rules are
     invisible); the icons are cryptic; line numbers in the output don't match the file until
     saved; it's unclear when you're "done".
5. Same conflict machinery handles cherry-pick, revert, rebase and stash-apply conflicts, with
   Continue / Abort actions appropriate to the operation (GK shows these in the banner).

### 2.8 File history & blame
- Right-click file (anywhere a file appears) → **File History**: commit list filtered to that
  file with per-commit diffs; **File Blame**: annotated source, hover a line → commit info,
  click → jump to commit. Toggle between History/Blame inside the view.

### 2.9 Tags, patches, misc
- Create tag at commit (lightweight/annotated), push tags on push (setting or per-push prompt),
  delete tag. Tags render as labels in the graph and a section in the left panel.
- Create patch from commit / from file changes; apply patch from file.
- "Copy file path", "Open in external diff tool", "Ignore" (adds to .gitignore, submenu: this
  file / by extension / folder) on file rows (screenshot 2).

### 2.10 Pull requests (GitHub integration)
- After pushing a new branch, GK shows a "Create pull request" affordance; a PR panel/section in
  the left rail lists open PRs with CI status; clicking opens a detail view or the browser.
- Branch labels of branches with open PRs show a PR icon; CI status shows as a dot/check/cross.
- **BK v1:** GitHub sign-in (device flow), PR badges on branches, PR detail side panel
  (read-only + merge button + open-in-browser), create PR from pushed branch, CI check dots.

### 2.11 Worktrees
- Right-click commit/branch → "Create worktree from this commit" (screenshot 5): choose folder +
  branch; the worktree opens as another repo tab. Worktrees listed in left panel; right-click →
  open in tab / prune / remove.

---

## 3. Context-menu inventory (transcribed from the real app)

These are the exact menus BK should match in coverage (wording may improve).

### 3.1 Commit row (right-click)
Checkout this commit · Create worktree from this commit ─ Create branch here · Cherry-pick
commit · Rebase `<current>` onto this commit · Reset `<current>` to this commit ▸ (Soft/Mixed/
Hard) · Revert commit ─ Copy commit sha · Copy link to this commit on remote: origin · Create
patch from commit · Share commit as Cloud Patch *(BK: omit)* ─ Compare commit against working
directory ─ Create tag here · Create annotated tag here

### 3.2 Branch label / branch in panel (right-click)
Checkout · Create branch here · Rename · Delete · Merge into current · Rebase current onto ·
Set upstream · Push · Pull (ff/merge/rebase) · Copy branch name · Hide in graph ·
Create pull request *(BK adds)* · Compare against current branch

### 3.3 Stash row (right-click)
Apply Stash · Pop Stash · Delete Stash ─ Edit stash message *(BK: name at creation instead)* ─
Share stash as Cloud Patch *(BK: omit)* ─ Hide

### 3.4 File row in Unstaged/Staged (right-click)
Stage / Unstage · Discard changes · Ignore ▸ · Stash file *(BK: v2)* ─ File History · File
Blame ─ Open in external diff tool · Open file · Show in Finder/Explorer ─ Copy file path ·
Create patch from file changes

### 3.5 Graph background / column headers
Column show/hide (Branch/Tag, Graph, Commit message, Author, Date/SHA columns are toggleable
via the gear icon top-right of the graph header). **BK keeps the gear + column toggles.**

---

## 4. What GitKraken gets *right* (preserve these)

1. **Zero-refresh model.** The graph and WIP always reflect reality within ~1s. No refresh button
   in daily use. This single property is most of why GK feels good.
2. **The graph is the workbench**, not a picture: labels are checkoutable, draggable; WIP lives in
   it; stashes live in it; the commit message is editable in it.
3. **Remote branches visible inline** with local/remote presence icons, and one-gesture checkout.
4. **Everything is right-clickable** and the menu shows everything you can do to that object.
5. **Undo-ability / low fear.** Destructive actions guarded; the Undo button (BK: toast-undo)
   means beginners explore without terror.
6. **The 72-char counter** and amend checkbox — tiny, correct nudges.
7. **Visual status language:** consistent color-coded file states, ahead/behind arrows,
   laptop/cloud presence icons.
8. **Multi-repo tabs** with full per-tab state.

## 5. Where GitKraken falls short (BK's opportunities)

1. **The merge tool** (see §2.7) — BK's single-panel "Keep" model is the flagship improvement
   (DESIGN_SPEC §9).
2. **Performance**: Electron heft, slow startup, sluggish on big repos. BK: Tauri + canvas +
   virtualization; startup target <1.5s to interactive graph.
3. **Upsells and account walls** everywhere (trial banners, cloud patches, paywalled features).
   BK: none, ever. No account. AGPL.
4. **Error surfacing**: raw git errors in modals. BK: humanized errors with suggested actions
   (DESIGN_SPEC §11).
5. **Force push uses `--force`.** BK: `--force-with-lease` always, with a clear explanation.
6. **Amending pushed commits** without warning. BK warns.
7. **Discard is irreversible.** BK: discard safety net — every discard is recoverable for 7 days
   (DESIGN_SPEC §7.4).
8. **Cluttered left rail** (workspaces/cloud/teams/focus). BK: only git-relevant sections.

---

## 6. Glossary mapping (GK term → BK term)

| GitKraken | BranchKit |
|---|---|
| WIP node | WIP row (same behavior) |
| Undo/Redo toolbar buttons | Action-scoped undo in toasts |
| Merge conflict output editor | **Keep Panel** (single-panel resolver) |
| Cloud Patch | (omitted) |
| Launchpad | (omitted) |
| Gitflow | (omitted) |
| Ours / Theirs | Actual branch names, always (e.g. "from `main`" / "from `feature/login`") |
