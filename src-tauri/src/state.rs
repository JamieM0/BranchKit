//! Per-repo actor registry — ARCHITECTURE.md §2. Each open repo gets a `RepoHandle` in a
//! `DashMap` in tauri `State`. All mutating git commands (added in later prompts) must take
//! `op_queue` before running; reads may run concurrently. `generation` + `suppress` implement
//! the watcher self-echo suppression described in §4.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::events::WatchedKind;
use crate::watcher::RepoWatcher;

/// How long a self-initiated op's kinds are suppressed from the watcher — ARCHITECTURE.md §4.
const SUPPRESSION_WINDOW: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RepoId(pub String);

impl std::fmt::Display for RepoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

struct Suppression {
    until: Instant,
    kinds: HashSet<WatchedKind>,
}

pub struct RepoHandle {
    pub id: RepoId,
    /// Canonicalized worktree root (`git rev-parse --show-toplevel`).
    pub path: PathBuf,
    /// All mutating commands serialize through this — ARCHITECTURE.md §2. Reads never take it.
    pub op_queue: tokio::sync::Mutex<()>,
    /// Bumped by every self-initiated mutating op so the watcher can tell its own echo apart
    /// from an external change — ARCHITECTURE.md §2, §4.
    pub generation: AtomicU64,
    suppress: Mutex<Option<Suppression>>,
    pub watcher: Mutex<Option<RepoWatcher>>,
    /// When the last fetch (auto or manual) completed — ARCHITECTURE.md §7.2's "skipped if a
    /// fetch ran <30s ago (manual counts)" rule reads this before every auto-fetch tick.
    pub last_fetch: Mutex<Option<Instant>>,
    /// The running auto-fetch interval task, if any, so it can be cancelled on `close_repo`.
    pub auto_fetch_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl RepoHandle {
    pub fn new(id: RepoId, path: PathBuf) -> Self {
        Self {
            id,
            path,
            op_queue: tokio::sync::Mutex::new(()),
            generation: AtomicU64::new(0),
            suppress: Mutex::new(None),
            watcher: Mutex::new(None),
            last_fetch: Mutex::new(None),
            auto_fetch_task: Mutex::new(None),
        }
    }

    /// Records that a fetch (auto or manual) just completed now.
    pub fn record_fetch(&self) {
        *self.last_fetch.lock().expect("last_fetch mutex poisoned") = Some(Instant::now());
    }

    /// Whether a fetch completed within the last `window` — the auto-fetch "ran <30s ago" guard.
    pub fn fetched_within(&self, window: Duration) -> bool {
        match *self.last_fetch.lock().expect("last_fetch mutex poisoned") {
            Some(at) => at.elapsed() < window,
            None => false,
        }
    }

    /// Call before running a self-initiated mutating op that will cause filesystem events of
    /// `kinds`. Bumps the generation counter and opens a 1.5s window in which the watcher drops
    /// events of exactly those kinds (the op's own completion handler triggers the refresh
    /// instead, so the UI updates exactly once, immediately) — ARCHITECTURE.md §4.
    pub fn begin_self_op(&self, kinds: &[WatchedKind]) -> u64 {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut guard = self.suppress.lock().expect("suppress mutex poisoned");
        *guard = Some(Suppression {
            until: Instant::now() + SUPPRESSION_WINDOW,
            kinds: kinds.iter().copied().collect(),
        });
        generation
    }

    /// Whether the watcher should drop an event of `kind` right now.
    pub fn is_suppressed(&self, kind: WatchedKind) -> bool {
        let guard = self.suppress.lock().expect("suppress mutex poisoned");
        match guard.as_ref() {
            Some(s) if s.until > Instant::now() => s.kinds.contains(&kind),
            _ => false,
        }
    }
}

pub struct AppState {
    pub repos: DashMap<RepoId, std::sync::Arc<RepoHandle>>,
    next_id: AtomicU64,
    /// Whether the app window currently has focus — ARCHITECTURE.md §7.2's "only while window
    /// focused" auto-fetch gate. Updated from tauri `WindowEvent::Focused` in `lib.rs`; starts
    /// `true` so a repo opened before the first focus event still auto-fetches. `Arc`-wrapped so
    /// each repo's auto-fetch task can hold its own cheap, independent handle to it.
    pub focused: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            repos: DashMap::new(),
            next_id: AtomicU64::new(0),
            focused: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl AppState {
    /// A cloned handle to the focus flag for a repo's auto-fetch task to poll independently of
    /// any single tauri command's `State` borrow.
    pub fn focused_handle(&self) -> Arc<AtomicBool> {
        self.focused.clone()
    }

    pub fn next_repo_id(&self) -> RepoId {
        let n = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        RepoId(format!("repo-{n}"))
    }

    /// An already-open handle for this canonical path, if any — `open_repo` is idempotent.
    pub fn find_by_path(&self, path: &std::path::Path) -> Option<std::sync::Arc<RepoHandle>> {
        self.repos
            .iter()
            .find(|entry| entry.value().path == path)
            .map(|entry| entry.value().clone())
    }

    pub fn get_repo(&self, id: &str) -> Option<std::sync::Arc<RepoHandle>> {
        self.repos
            .get(&RepoId(id.to_string()))
            .map(|entry| entry.value().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppresses_only_the_kinds_and_window_given() {
        let handle = RepoHandle::new(RepoId("r1".into()), PathBuf::from("/tmp/repo"));
        assert!(!handle.is_suppressed(WatchedKind::Refs));

        handle.begin_self_op(&[WatchedKind::Refs, WatchedKind::Head]);
        assert!(handle.is_suppressed(WatchedKind::Refs));
        assert!(handle.is_suppressed(WatchedKind::Head));
        assert!(!handle.is_suppressed(WatchedKind::WorkingTree));
    }

    #[test]
    fn generation_increments_each_call() {
        let handle = RepoHandle::new(RepoId("r1".into()), PathBuf::from("/tmp/repo"));
        let g1 = handle.begin_self_op(&[WatchedKind::Index]);
        let g2 = handle.begin_self_op(&[WatchedKind::Index]);
        assert_eq!(g2, g1 + 1);
    }

    #[test]
    fn next_repo_id_is_unique_and_ordered() {
        let state = AppState::default();
        let a = state.next_repo_id();
        let b = state.next_repo_id();
        assert_ne!(a, b);
        assert_eq!(a.0, "repo-1");
        assert_eq!(b.0, "repo-2");
    }
}
