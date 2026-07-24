<div align="center">

# BranchKit

**A git client that makes the hard parts feel easy.**

Merge conflicts you resolve by pointing and clicking, a commit graph you
manipulate directly, and safe action buttons rather than memorising commands.

Free · open source · cross-platform — with no accounts, telemetry, or upsells.

<br />

![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-2ea44f?style=flat-square)
![Platforms](https://img.shields.io/badge/macOS%20·%20Windows%20·%20Linux-1f2328?style=flat-square)
![Built with Tauri + Svelte](https://img.shields.io/badge/Tauri%202%20·%20Svelte%205-1f2328?style=flat-square)

</div>

<br />

<div align="center">

![The Keep Panel showing two candidate blocks labelled by branch](docs/screenshots/keep-panel.jpg)


<em>Resolving a merge conflict by choosing what to keep — no diff markers, no hand-editing.</em>

</div>

## Why BranchKit

Most git tools put git's complexity in front of you and expect you to manage it.
BranchKit manages the hard part for you. The operations people usually dread — resolving conflicts,
rebasing, recovering from a mistake — become effortless by pointing, clicking,
and dragging, with the tool watching your back.

It's built for people who want to focus on building and get git out of the way, not for power users who enjoy tinkering with git.

## Conflicts as choices

Conflict resolution is the most common "complex" part of git, so it's the part
BranchKit works hardest to simplify. The **Keep Panel** replaces the usual "theirs" and "ours" with a single, readable view of the file you're building.

Each side of a conflict is a labelled block with its real branch name — *yours* and
*incoming*. You **keep** a side with one click, or keep both and they stack in the order you click them. Lines renumber live as
you go, you can keep individual lines, and there's a hand-edit escape hatch when you
need it — all fully keyboard-drivable. The same panel handles merge, rebase,
cherry-pick, revert, and stash conflicts, so there's one thing to learn instead of five.

## The graph is the workbench

The commit graph is how you *do* things. Drag one branch onto another to merge, rebase, or fast-forward. Double-click a remote branch to
create a tracking branch and check it out in a single gesture. Stashes live right in the
graph, unpushed commits wear a dashed halo so you can see what's local at a glance, and
the top row doubles as your commit message editor.

<div align="center">

![BranchKit's commit graph with branches, tags, stashes and worktrees](docs/screenshots/commit-graph.jpg)

</div>

## Nothing you do is scary

BranchKit assumes you'll make mistakes and makes them cheap to undo. Every discard is
recoverable for 7 days. Force-pushes use `--force-with-lease` only. Amending a pushed
commit warns you first. When something does go wrong, errors are written in plain
language with a suggested fix — not raw git output you have to interpret.

Basically, BranchKit's got your back every step of the way.

## Everything else, kept simple

- **Staging that stays out of your way.** Stage by file, hunk, or single line; walk the list using 'Space'; discards stay recoverable.
- **AI commit messages, local-first and optional.** Use the in-app managed local model
  (llama.cpp), your own Ollama, or any OpenAI-/Anthropic-format endpoint. Keys live in
  the OS keychain, never in a config file.
- **GitHub, only if you want it.** Device-flow sign-in, a PR panel, create/merge/checkout
  PRs, and CI status dots — entirely optional.
- **The rest.** Multi-repo tabs, worktrees, file history and blame, a `⌘K` command
  palette, and dark + light themes.

## Install

Download the latest release from [Releases](../../releases). BranchKit requires git ≥ 2.30
on your `PATH`.

- **macOS:** download the build for your Mac (`aarch64` for Apple silicon, `x86_64` for
  Intel), open the `.dmg`, and drag BranchKit to Applications. Release signing and
  notarisation are not yet configured for the release candidate, so macOS may require
  **Open Anyway** in Privacy & Security.
- **Windows:** download and run the `.msi` installer. Windows 10 and 11 are supported.
- **Linux:** use the portable `.AppImage` or install the `.deb` on Debian/Ubuntu. Secure
  credential storage uses libsecret/gnome-keyring; when it is unavailable, BranchKit shows
  a warning and keeps secrets in memory for that session only.

## Screenshots

The images above show the v0.1.0 Keep Panel and commit graph running against real repositories.
More workflow captures are welcome with documentation contributions.

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

[AGPL-3.0](LICENSE). If you distribute a modified version or offer it over a network, the
AGPL requires you to make the corresponding source available under the same licence.

BranchKit is an independent project and is not affiliated with GitKraken or its developers.
