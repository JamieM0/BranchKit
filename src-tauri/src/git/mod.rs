//! Git read layer — one module per command family (ARCHITECTURE.md §1). Everything here shells
//! out through `exec::git`; nothing else in the codebase should spawn `git` directly.

pub mod blob;
pub mod diff;
pub mod discard;
pub mod exec;
pub mod identity;
pub mod log;
pub mod ops;
pub mod refs;
pub mod stage;
pub mod status;
pub mod worktree;
