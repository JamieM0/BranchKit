//! Stash mutations — DESIGN_SPEC.md §4.5/§15.18, ARCHITECTURE.md §7.1. Reading stash contents
//! reuses `diff::diff_commit` / `diff::commit_files` unchanged: a stash commit's first parent is
//! the commit it was taken on top of, so treating the stash `selector` (e.g. `stash@{0}`) as an
//! ordinary commit-ish for those two functions already gives the right "what changed" view — no
//! separate stash-diff parser needed.

use tauri::{AppHandle, State};

use crate::error::AppError;
use crate::events::{ChangeKind, WatchedKind};
use crate::state::AppState;

use super::exec::{git, GitOpts};
use super::ops::{emit_changes, require_repo};

/// Stash all WIP — the toolbar Stash button and its dropdown's "with message…" / "including
/// untracked" variants (DESIGN_SPEC.md §3.2). `message` empty means no `-m` (git's default
/// "WIP on <branch>: …" message); `include_untracked` adds `-u`.
#[tauri::command]
pub async fn stash_push(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    message: Option<String>,
    include_untracked: bool,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::WorkingTree, WatchedKind::Index]);
    let mut args = vec!["stash", "push"];
    if include_untracked {
        args.push("-u");
    }
    let message = message.filter(|m| !m.trim().is_empty());
    if let Some(m) = message.as_deref() {
        args.push("-m");
        args.push(m);
    }
    let result = git(&handle.path, &args, GitOpts::default()).await;
    emit_changes(&app, &repo_id, &[ChangeKind::WorkingTree, ChangeKind::Index]);
    result?;
    Ok(())
}

/// Pop the latest (or a specific) stash — double-click a stash row (§4.5/§15.18), or the
/// toolbar's Pop button (always `stash@{0}`). A pop conflict leaves the repo mid-conflict for the
/// graph to surface, same as any other conflicting op.
#[tauri::command]
pub async fn stash_pop(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    selector: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::WorkingTree, WatchedKind::Index, WatchedKind::Refs]);
    let result = git(&handle.path, &["stash", "pop", &selector], GitOpts::default()).await;
    emit_changes(
        &app,
        &repo_id,
        &[ChangeKind::WorkingTree, ChangeKind::Index, ChangeKind::Refs],
    );
    result?;
    Ok(())
}

/// Apply a stash without dropping it — the stash row menu's Apply (§3.3).
#[tauri::command]
pub async fn stash_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    selector: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::WorkingTree, WatchedKind::Index]);
    let result = git(&handle.path, &["stash", "apply", &selector], GitOpts::default()).await;
    emit_changes(&app, &repo_id, &[ChangeKind::WorkingTree, ChangeKind::Index]);
    result?;
    Ok(())
}

/// Drop a stash without applying it — the stash row menu's Drop… (confirmed in the frontend,
/// §3.3).
#[tauri::command]
pub async fn stash_drop(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    selector: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Refs]);
    git(&handle.path, &["stash", "drop", &selector], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Refs]);
    Ok(())
}

/// Copy a stash as a patch to the clipboard — the stash row menu's "Copy patch to clipboard"
/// (§4.5). `git stash show -p` renders it exactly as `git show` would for a normal commit.
#[tauri::command]
pub async fn get_stash_patch(
    state: State<'_, AppState>,
    repo_id: String,
    selector: String,
) -> Result<String, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let output = git(
        &handle.path,
        &["stash", "show", "-p", "--no-color", &selector],
        GitOpts::default(),
    )
    .await?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
