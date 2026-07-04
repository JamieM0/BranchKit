# BranchKit

BranchKit is a free, open-source (AGPL-3.0) cross-platform git client inspired by GitKraken.
It's built with Tauri 2.x (Rust backend) and Svelte 5 + TypeScript (frontend). The Rust side
owns all git and secret operations; the frontend only talks to it through typed IPC. The flagship
feature is the Keep Panel, an in-app conflict resolver that treats conflict resolution as
"choosing what to keep" rather than editing diff markers.

Read docs/DESIGN_SPEC.md and docs/ARCHITECTURE.md sections relevant to your task before coding.

## HARD RULES for this and every session

Work only in this session — do NOT use subagents, the Task/Agent tool, or background agents,
ever. Do not add dependencies beyond those named in ARCHITECTURE.md without a code comment
justifying them. Follow the specs exactly; mark any deviation with a SPEC-DEVIATION comment.

## Commands

- Dev: `npm run tauri dev`
- Tests: `cargo test` (in `src-tauri`), `npx vitest run`
- Lint: `cargo clippy -- -D warnings` (in `src-tauri`), `npx svelte-check`

## Code conventions

- Rust: one module per git command family (`src-tauri/src/git/log.rs`, `status.rs`, etc.).
- TypeScript: all IPC calls go through `src/lib/ipc.ts` — it is the only place `invoke()` is called.
- All colors via CSS design tokens (`src/lib/tokens.css`) — no hard-coded colors anywhere.
