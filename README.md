# BranchKit

BranchKit is a free, open-source, cross-platform git client for macOS, Windows, and Linux,
built with Tauri and Svelte. It's inspired by GitKraken's graph-first workflow — a
click-and-drag commit graph, an in-app conflict resolver (the Keep Panel), safety nets instead
of scary confirm dialogs, and a command palette — without accounts, telemetry, or upsells.

![BranchKit screenshot placeholder](docs/screenshot-placeholder.png)

## Building from source

Requires [Rust](https://www.rust-lang.org/tools/install), [Node.js](https://nodejs.org/) 18+,
and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```sh
npm install
npm run tauri dev    # run in development
npm run tauri build  # produce a release bundle
```

On macOS, `tauri dev` runs an unsigned debug binary, which gets a new identity on every
rebuild — so if you've connected GitHub or a remote AI provider (both store a secret in the
Keychain), macOS will re-prompt for Keychain access on every launch even after "Always Allow."
Run `bash scripts/dev-cert-setup.sh` once to create a stable local signing identity, then use
`npm run tauri:dev:signed` instead of `npm run tauri dev` — it signs the debug binary with that
identity before launching, so the Keychain grant sticks across rebuilds.

## License

[AGPL-3.0](LICENSE)
