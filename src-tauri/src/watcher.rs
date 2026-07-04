//! Filesystem watching — ARCHITECTURE.md §4. One `notify` watcher covers the repo's actual git
//! directory (resolved via `git rev-parse --absolute-git-dir`, so linked worktrees whose `.git`
//! is a pointer file to elsewhere still work) and a second covers the worktree root, recursively.
//! Raw events are bridged off `notify`'s callback thread, classified, debounced 300ms, folded
//! under event storms, and checked against the repo's self-echo suppression window before being
//! emitted as `repo://{id}/changed`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use crate::events::{ChangeKind, WatchedKind};
use crate::git::exec::{git, GitError, GitOpts};
use crate::state::RepoHandle;

const DEBOUNCE: Duration = Duration::from_millis(300);
/// Above this many raw events in one debounce window (a branch checkout, a build running),
/// stop classifying individually and just assume the worktree changed — ARCHITECTURE.md §4.
const STORM_THRESHOLD: u32 = 500;

#[derive(Debug)]
pub enum WatcherError {
    Notify(notify::Error),
    Git(GitError),
}

impl From<notify::Error> for WatcherError {
    fn from(e: notify::Error) -> Self {
        Self::Notify(e)
    }
}

impl From<GitError> for WatcherError {
    fn from(e: GitError) -> Self {
        Self::Git(e)
    }
}

impl From<WatcherError> for crate::error::AppError {
    fn from(e: WatcherError) -> Self {
        match e {
            WatcherError::Notify(e) => e.into(),
            WatcherError::Git(e) => e.into(),
        }
    }
}

/// Owns the live `notify` watcher and its debounce task. Dropping this stops watching: the
/// `notify::Watcher` unwatches on drop, which closes the bridging thread's channel, which ends
/// the debounce task's loop — `abort()` here just makes that immediate instead of eventual.
pub struct RepoWatcher {
    _watcher: notify::RecommendedWatcher,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for RepoWatcher {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn resolve_git_dir(repo_path: &Path) -> Result<PathBuf, GitError> {
    let output = git(
        repo_path,
        &["rev-parse", "--absolute-git-dir"],
        GitOpts::default(),
    )
    .await?;
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

/// Classifies one changed path, or `None` if it's noise we never want to refresh for (loose
/// object writes, reflogs, hooks, `index.lock` and friends — ARCHITECTURE.md §4).
fn classify(git_dir: &Path, worktree_root: &Path, path: &Path) -> Option<WatchedKind> {
    if let Ok(rel) = path.strip_prefix(git_dir) {
        let rel = rel.to_string_lossy();
        if rel.is_empty()
            || rel.starts_with("objects")
            || rel.starts_with("logs")
            || rel.starts_with("hooks")
        {
            return None;
        }
        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        return match file_name {
            "HEAD" | "MERGE_HEAD" | "REBASE_HEAD" | "CHERRY_PICK_HEAD" | "REVERT_HEAD" => {
                Some(WatchedKind::Head)
            }
            "FETCH_HEAD" => Some(WatchedKind::Remote),
            "index" => Some(WatchedKind::Index),
            "packed-refs" => Some(WatchedKind::Refs),
            _ if rel.starts_with("refs")
                || rel.starts_with("rebase-merge")
                || rel.starts_with("rebase-apply") =>
            {
                Some(WatchedKind::Refs)
            }
            _ => None,
        };
    }
    if path.strip_prefix(worktree_root).is_ok() {
        return Some(WatchedKind::WorkingTree);
    }
    None
}

pub async fn start(app: AppHandle, handle: Arc<RepoHandle>) -> Result<RepoWatcher, WatcherError> {
    let git_dir = resolve_git_dir(&handle.path).await?;
    let worktree_root = handle.path.clone();

    let (raw_tx, raw_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = raw_tx.send(res);
    })?;
    watcher.watch(&git_dir, RecursiveMode::Recursive)?;
    watcher.watch(&worktree_root, RecursiveMode::Recursive)?;

    // notify's callback fires on its own thread and is sync; bridge it into a tokio channel with
    // a plain OS thread (its loop ends the moment `raw_tx` above is dropped, i.e. when `watcher`
    // is dropped).
    let (path_tx, mut path_rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();
    std::thread::spawn(move || {
        while let Ok(res) = raw_rx.recv() {
            if let Ok(event) = res {
                for path in event.paths {
                    if path_tx.send(path).is_err() {
                        return;
                    }
                }
            }
        }
    });

    let repo_id = handle.id.clone();
    let task = tokio::spawn(async move {
        let mut pending: HashSet<WatchedKind> = HashSet::new();
        let mut raw_count: u32 = 0;
        let mut deadline: Option<tokio::time::Instant> = None;

        loop {
            tokio::select! {
                maybe_path = path_rx.recv() => {
                    let Some(path) = maybe_path else { break };
                    raw_count += 1;
                    if raw_count <= STORM_THRESHOLD {
                        if let Some(kind) = classify(&git_dir, &worktree_root, &path) {
                            pending.insert(kind);
                        }
                    } else {
                        pending.insert(WatchedKind::WorkingTree);
                    }
                    if deadline.is_none() {
                        deadline = Some(tokio::time::Instant::now() + DEBOUNCE);
                    }
                }
                _ = tokio::time::sleep_until(deadline.unwrap_or_else(tokio::time::Instant::now)), if deadline.is_some() => {
                    for kind in pending.drain() {
                        if !handle.is_suppressed(kind) {
                            let _ = app.emit(&format!("repo://{repo_id}/changed"), ChangeKind::from(kind));
                        }
                    }
                    raw_count = 0;
                    deadline = None;
                }
            }
        }
    });

    Ok(RepoWatcher {
        _watcher: watcher,
        task,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_head_and_merge_state_files_as_head() {
        let git_dir = Path::new("/repo/.git");
        let root = Path::new("/repo");
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/.git/HEAD")),
            Some(WatchedKind::Head)
        );
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/.git/MERGE_HEAD")),
            Some(WatchedKind::Head)
        );
    }

    #[test]
    fn classifies_refs_and_packed_refs() {
        let git_dir = Path::new("/repo/.git");
        let root = Path::new("/repo");
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/.git/refs/heads/main")),
            Some(WatchedKind::Refs)
        );
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/.git/packed-refs")),
            Some(WatchedKind::Refs)
        );
    }

    #[test]
    fn classifies_index_and_fetch_head() {
        let git_dir = Path::new("/repo/.git");
        let root = Path::new("/repo");
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/.git/index")),
            Some(WatchedKind::Index)
        );
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/.git/FETCH_HEAD")),
            Some(WatchedKind::Remote)
        );
    }

    #[test]
    fn ignores_lockfiles_objects_logs_and_hooks() {
        let git_dir = Path::new("/repo/.git");
        let root = Path::new("/repo");
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/.git/index.lock")),
            None
        );
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/.git/objects/ab/cdef")),
            None
        );
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/.git/logs/HEAD")),
            None
        );
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/.git/hooks/pre-commit")),
            None
        );
    }

    #[test]
    fn worktree_paths_outside_git_dir_are_working_tree() {
        let git_dir = Path::new("/repo/.git");
        let root = Path::new("/repo");
        assert_eq!(
            classify(git_dir, root, Path::new("/repo/src/main.rs")),
            Some(WatchedKind::WorkingTree)
        );
    }

    #[test]
    fn linked_worktree_git_dir_outside_worktree_root_still_classifies() {
        // `git worktree add` gives the linked worktree a `.git` *file* pointing elsewhere; the
        // real git dir can live completely outside the worktree root.
        let git_dir = Path::new("/main-repo/.git/worktrees/feature");
        let root = Path::new("/somewhere/else/feature-worktree");
        assert_eq!(
            classify(
                git_dir,
                root,
                Path::new("/main-repo/.git/worktrees/feature/HEAD")
            ),
            Some(WatchedKind::Head)
        );
        assert_eq!(
            classify(
                git_dir,
                root,
                Path::new("/somewhere/else/feature-worktree/README.md")
            ),
            Some(WatchedKind::WorkingTree)
        );
    }

    #[test]
    fn unrelated_paths_are_ignored() {
        let git_dir = Path::new("/repo/.git");
        let root = Path::new("/repo");
        assert_eq!(
            classify(git_dir, root, Path::new("/somewhere/else.txt")),
            None
        );
    }
}
