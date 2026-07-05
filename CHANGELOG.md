# Changelog

## 0.1.0 — first release

The first usable BranchKit: a graph-first git client for macOS, Windows and Linux.

### Highlights

- **Commit graph as the workbench** — virtualized canvas graph smooth at 20k+ commits, with
  local + remote branch pills inline, one-gesture remote checkout (double-click), drag a
  branch onto another to merge/rebase/fast-forward, inline stash rows, an editable WIP row,
  and a dashed halo on commits that haven't reached any remote yet.
- **The Keep Panel** — a single-panel conflict resolver: conflicted lines are candidates you
  *keep* (or don't), with live line renumbering, real branch names instead of ours/theirs,
  keep-both in click order, per-line keeps, a hand-edit escape hatch, and full keyboard
  control (1/2/b/u/e/n/p).
- **Staging done right** — per-file, per-hunk, and per-line staging (click or drag the
  gutter), Space-walks the file list, and a 7-day discard safety net so nothing is ever lost
  to a misclick.
- **Sync without fear** — ahead/behind badges are clickable fix-it buttons, force push is
  always `--force-with-lease`, amending a pushed commit warns first, and common git failures
  are translated into plain sentences with a suggested action.
- **GitHub, optional** — device-flow sign-in, PR list and detail panel, create/merge/checkout
  PRs, CI dots on commit rows. Everything degrades invisibly when not connected.
- **AI commit messages, local-first** — a fully in-app-managed local model (Gemma 4 E2B via
  llama.cpp, downloaded with progress + sha256 verification), or your own Ollama instance, or
  any OpenAI-/Anthropic-format API. Optional, off by default.
- Multi-repo tabs, worktrees, file history + blame, command palette (⌘K), dark + light
  themes, and no accounts, telemetry, or upsells — ever.
