# BranchKit

BranchKit is a free, open-source, cross-platform git client for macOS, Windows, and Linux,
built with Tauri and Svelte. It's inspired by GitKraken's graph-first workflow — a
click-and-drag commit graph, an in-app conflict resolver (the Keep Panel), safety nets instead
of scary confirm dialogs, and a command palette — without accounts, telemetry, or upsells.

![BranchKit screenshot placeholder](docs/screenshot-placeholder.png)

## Features

- **Commit graph as the workbench.** Virtualized canvas graph, smooth at 20k+ commits. Local
  and remote branches inline with presence icons; double-click a remote branch to create a
  tracking branch and check it out in one gesture; drag a branch onto another to
  merge/rebase/fast-forward; stashes live in the graph; the WIP row is your commit message
  editor; unpushed commits wear a dashed halo.
- **The Keep Panel.** Conflicts as choices, not marker soup: one panel showing the future
  file, candidate blocks labeled with real branch names, keep/unkeep with live line
  renumbering, keep-both in click order, per-line keeps, a hand-edit escape hatch, full
  keyboard control. Works for merge, rebase, cherry-pick, revert and stash conflicts.
- **Staging done right.** Per-file, per-hunk, per-line staging; Space walks the list; every
  discard is recoverable for 7 days.
- **Sync without fear.** Clickable ahead/behind badges, `--force-with-lease` only,
  pushed-amend warnings, and human-language errors with suggested fixes.
- **GitHub, optional.** Device-flow sign-in, PR panel, create/merge/checkout PRs, CI dots.
- **AI commit messages, local-first and optional.** In-app-managed local model (llama.cpp),
  or your own Ollama, or any OpenAI-/Anthropic-format endpoint. Keys live in the OS keychain.
- Multi-repo tabs, worktrees, file history + blame, ⌘K palette, dark + light themes.

## Install

Download the latest release for your OS from
[Releases](../../releases): `.dmg` (macOS), `.msi` (Windows), `.AppImage` / `.deb` (Linux).
Requires git ≥ 2.30 on your PATH. On Linux, secure credential storage uses
libsecret/gnome-keyring; without it, secrets are kept in memory only for the session.

## Building from source

Requires [Rust](https://www.rust-lang.org/tools/install), [Node.js](https://nodejs.org/) 18+,
and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```sh
npm install
npm run tauri dev    # run in development
npm run tauri build  # produce a release bundle
```

Tests and lints: `cargo test` and `cargo clippy -- -D warnings` in `src-tauri`,
`npx vitest run` and `npm run check` at the root.

On macOS, `tauri dev` runs an unsigned debug binary, which gets a new identity on every
rebuild — so if you've connected GitHub or a remote AI provider (both store a secret in the
Keychain), macOS will re-prompt for Keychain access on every launch even after "Always Allow."
Run `bash scripts/dev-cert-setup.sh` once to create a stable local signing identity, then use
`npm run tauri:dev:signed` instead of `npm run tauri dev` — it signs the debug binary with that
identity before launching, so the Keychain grant sticks across rebuilds.

## Contributing

Issues and pull requests are welcome. Before opening a PR: read `docs/DESIGN_SPEC.md` and
`docs/ARCHITECTURE.md` (they are the source of truth for UX and technical decisions), keep
all colors on design tokens, route all IPC through `src/lib/ipc.ts`, and make sure the test
and lint commands above pass on your change.

## License

[AGPL-3.0](LICENSE). BranchKit is an independent project, not affiliated with, endorsed by,
or connected to GitKraken or Axosoft, LLC.
