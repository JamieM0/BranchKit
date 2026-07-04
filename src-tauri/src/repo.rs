//! Repo lifecycle commands — ARCHITECTURE.md §2, DESIGN_SPEC.md §3.1/§11. `open_repo` and
//! `clone_repo` are the only two ways a `RepoHandle` enters the registry; `close_repo` is the
//! only way one leaves it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::AppError;
use crate::events::ChangeKind;
use crate::git::exec::{git, git_with_progress, GitErrorKind, GitOpts};
use crate::git::identity::{self, GitIdentity};
use crate::state::{AppState, RepoHandle, RepoId};
use crate::watcher;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub id: String,
    pub path: String,
    pub name: String,
    /// `None` on an unborn branch (no commits yet) as well as on a real detached HEAD — the
    /// distinction doesn't matter for a tab label; the empty-graph state (DESIGN_SPEC §11)
    /// covers the unborn case in the UI.
    pub branch: Option<String>,
    pub detached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecentRepo {
    pub path: String,
    pub name: String,
    pub last_opened_at: i64,
}

const MAX_RECENTS: usize = 20;

fn repo_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_string()
}

/// Current branch + detached state without requiring a born HEAD (an empty, freshly-initialized
/// repo has no commits yet — DESIGN_SPEC §11 "empty graph" state must still open cleanly).
async fn branch_state(path: &Path) -> Result<(Option<String>, bool), AppError> {
    match git(
        path,
        &["symbolic-ref", "-q", "--short", "HEAD"],
        GitOpts::default(),
    )
    .await
    {
        Ok(output) => {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok((Some(branch), false))
        }
        // `symbolic-ref -q` exits 1 (no stderr) when HEAD doesn't point at a branch.
        Err(e) if e.kind == GitErrorKind::NonZeroExit && e.code == Some(1) => Ok((None, true)),
        Err(e) => Err(e.into()),
    }
}

async fn repo_info(handle: &RepoHandle) -> Result<RepoInfo, AppError> {
    let (branch, detached) = branch_state(&handle.path).await?;
    Ok(RepoInfo {
        id: handle.id.0.clone(),
        path: handle.path.to_string_lossy().to_string(),
        name: repo_name(&handle.path),
        branch,
        detached,
    })
}

fn recents_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app.path().app_config_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("recents.json"))
}

fn load_recents(app: &AppHandle) -> Vec<RecentRepo> {
    let Ok(path) = recents_path(app) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_recents(app: &AppHandle, recents: &[RecentRepo]) -> Result<(), AppError> {
    let path = recents_path(app)?;
    let text = serde_json::to_string_pretty(recents)?;
    std::fs::write(path, text)?;
    Ok(())
}

/// Moves (or inserts) `entry` to the front of `recents` by path, capped at `max` entries.
fn upsert_recent(mut recents: Vec<RecentRepo>, entry: RecentRepo, max: usize) -> Vec<RecentRepo> {
    recents.retain(|r| r.path != entry.path);
    recents.insert(0, entry);
    recents.truncate(max);
    recents
}

fn record_recent(app: &AppHandle, path: &Path, name: &str) {
    let recents = load_recents(app);
    let last_opened_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let entry = RecentRepo {
        path: path.to_string_lossy().to_string(),
        name: name.to_string(),
        last_opened_at,
    };
    let recents = upsert_recent(recents, entry, MAX_RECENTS);
    // Best-effort: a failed write here shouldn't fail the open/clone that triggered it.
    let _ = save_recents(app, &recents);
}

/// Opens (or, if already open, returns the existing handle for) the repo containing `path`.
/// Accepts any path inside a worktree, not just its root — `git rev-parse --show-toplevel`
/// resolves it and also validates that it *is* a git repo in the first place.
#[tauri::command]
pub async fn open_repo(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<RepoInfo, AppError> {
    let requested = PathBuf::from(&path);
    let toplevel = git(
        &requested,
        &["rev-parse", "--show-toplevel"],
        GitOpts::default(),
    )
    .await
    .map_err(|e| {
        AppError::new(
            format!(
                "\"{}\" doesn't look like a git repository",
                requested.display()
            ),
            e.to_string(),
        )
    })?;
    let canonical = PathBuf::from(String::from_utf8_lossy(&toplevel.stdout).trim().to_string());

    if let Some(existing) = state.find_by_path(&canonical) {
        let info = repo_info(&existing).await?;
        record_recent(&app, &canonical, &info.name);
        return Ok(info);
    }

    let id = state.next_repo_id();
    let handle = Arc::new(RepoHandle::new(id.clone(), canonical.clone()));
    let repo_watcher = watcher::start(app.clone(), handle.clone()).await?;
    *handle.watcher.lock().expect("watcher mutex poisoned") = Some(repo_watcher);
    state.repos.insert(id, handle.clone());

    let info = repo_info(&handle).await?;
    record_recent(&app, &canonical, &info.name);
    Ok(info)
}

/// Clones `url` into `destination`, streaming progress on `clone://{request_id}/progress`
/// (ARCHITECTURE.md §3.1), then opens the result exactly as `open_repo` would.
///
/// SPEC-DEVIATION: progress can't go out on `repo://{id}/changed` (ARCHITECTURE.md §2) because
/// there is no repo id until the clone finishes — `request_id` is caller-supplied so the
/// frontend can listen before the command call resolves.
#[tauri::command]
pub async fn clone_repo(
    app: AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    url: String,
    destination: String,
) -> Result<RepoInfo, AppError> {
    let dest = PathBuf::from(&destination);
    if dest.exists() {
        return Err(AppError::new(
            format!("\"{}\" already exists", dest.display()),
            "clone destination already exists".to_string(),
        ));
    }
    let parent = dest
        .parent()
        .map(Path::to_path_buf)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&parent)?;

    let dest_str = dest.to_string_lossy().to_string();
    let event_name = format!("clone://{request_id}/progress");
    let app_for_progress = app.clone();

    let clone_result = git_with_progress(
        &parent,
        &["clone", "--progress", &url, &dest_str],
        GitOpts::network(),
        move |update| {
            let _ = app_for_progress.emit(
                &event_name,
                ChangeKind::OperationProgress {
                    phase: update.phase,
                    percent: Some(update.percent),
                },
            );
        },
    )
    .await;

    if let Err(e) = clone_result {
        // Best-effort cleanup so a retry with the same destination doesn't hit the exists-guard.
        let _ = std::fs::remove_dir_all(&dest);
        return Err(e.into());
    }

    open_repo(app, state, dest_str).await
}

/// Removes a repo from the registry and stops watching it. Waits for any in-flight mutating op
/// to finish first so a close can never race a write to the working tree.
#[tauri::command]
pub async fn close_repo(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let Some((_, handle)) = state.repos.remove(&RepoId(id)) else {
        return Ok(());
    };
    let _guard = handle.op_queue.lock().await;
    *handle.watcher.lock().expect("watcher mutex poisoned") = None;
    Ok(())
}

#[tauri::command]
pub async fn list_recents(app: AppHandle) -> Result<Vec<RecentRepo>, AppError> {
    Ok(load_recents(&app))
}

#[tauri::command]
pub async fn check_git_identity() -> Result<GitIdentity, AppError> {
    Ok(identity::get_identity().await?)
}

#[tauri::command]
pub async fn set_git_identity(name: String, email: String) -> Result<(), AppError> {
    identity::set_identity(&name, &email).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, when: i64) -> RecentRepo {
        RecentRepo {
            path: path.to_string(),
            name: path.to_string(),
            last_opened_at: when,
        }
    }

    #[test]
    fn upsert_recent_moves_existing_path_to_front() {
        let recents = vec![entry("/a", 1), entry("/b", 2)];
        let result = upsert_recent(recents, entry("/a", 3), 20);
        assert_eq!(result[0].path, "/a");
        assert_eq!(result[0].last_opened_at, 3);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn upsert_recent_caps_at_max() {
        let recents = vec![entry("/a", 1), entry("/b", 2)];
        let result = upsert_recent(recents, entry("/c", 3), 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].path, "/c");
        assert_eq!(result[1].path, "/a");
    }

    #[test]
    fn repo_name_uses_final_path_component() {
        assert_eq!(
            repo_name(Path::new("/Users/jamie/Developer/BranchKit")),
            "BranchKit"
        );
    }

    async fn init_repo(dir: &Path) {
        git(
            dir,
            &["init", "--initial-branch=main", "-q"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        git(dir, &["config", "user.name", "T"], GitOpts::default())
            .await
            .unwrap();
        git(
            dir,
            &["config", "user.email", "t@example.com"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        git(
            dir,
            &["config", "commit.gpgsign", "false"],
            GitOpts::default(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn branch_state_on_unborn_branch_resolves_symbolic_name() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        // `main` is the symbolic HEAD target even before the first commit, so this is neither
        // detached nor unresolvable — it's the DESIGN_SPEC §11 empty-graph state.
        let (branch, detached) = branch_state(dir.path()).await.unwrap();
        assert_eq!(branch.as_deref(), Some("main"));
        assert!(!detached);
    }

    #[tokio::test]
    async fn branch_state_detects_detached_head() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        std::fs::write(dir.path().join("f.txt"), "hi").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default())
            .await
            .unwrap();
        git(
            dir.path(),
            &["commit", "-q", "-m", "init"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        let head = git(dir.path(), &["rev-parse", "HEAD"], GitOpts::default())
            .await
            .unwrap();
        let sha = String::from_utf8_lossy(&head.stdout).trim().to_string();
        git(dir.path(), &["checkout", "-q", &sha], GitOpts::default())
            .await
            .unwrap();

        let (branch, detached) = branch_state(dir.path()).await.unwrap();
        assert_eq!(branch, None);
        assert!(detached);
    }
}
