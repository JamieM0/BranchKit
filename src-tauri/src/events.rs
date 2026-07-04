//! Event payloads emitted Rust → frontend — ARCHITECTURE.md §2. Every open repo emits
//! `repo://{id}/changed` carrying a [`ChangeKind`]; the frontend re-queries only what changed.

use serde::Serialize;

/// The five plain kinds a filesystem watcher can classify a change into (ARCHITECTURE.md §4),
/// plus `OperationProgress` for long network ops (§3.1) once a repo has an id to emit against.
///
/// SPEC-DEVIATION: `clone_repo` runs before a repo (and therefore an id) exists, so its progress
/// is emitted on a request-scoped `clone://{request_id}/progress` channel instead of this one —
/// see `repo.rs`. Every other op that can reach this enum already has a real repo id.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChangeKind {
    WorkingTree,
    Index,
    Refs,
    Head,
    Remote,
    OperationProgress { phase: String, percent: Option<u32> },
}

/// The subset of [`ChangeKind`] the watcher classifies raw filesystem events into. Kept separate
/// from `ChangeKind` because this one needs to be a plain `Copy + Eq + Hash` key for the
/// generation-tied suppression set (ARCHITECTURE.md §4) and the per-window dedup set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchedKind {
    WorkingTree,
    Index,
    Refs,
    Head,
    Remote,
}

impl From<WatchedKind> for ChangeKind {
    fn from(kind: WatchedKind) -> Self {
        match kind {
            WatchedKind::WorkingTree => ChangeKind::WorkingTree,
            WatchedKind::Index => ChangeKind::Index,
            WatchedKind::Refs => ChangeKind::Refs,
            WatchedKind::Head => ChangeKind::Head,
            WatchedKind::Remote => ChangeKind::Remote,
        }
    }
}
