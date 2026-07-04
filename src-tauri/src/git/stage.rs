//! Whole-file staging mutations — DESIGN_SPEC.md §6.1, ARCHITECTURE.md §7.1. `git add` handles
//! tracked modifications and untracked files identically, so a single command covers both; hunk
//! and line-level staging (the patch-construction technique in ARCHITECTURE.md §6.3) is a
//! separate, larger piece of work saved for the Keep Panel / diff-viewer follow-up prompt.

use tauri::{AppHandle, State};

use crate::error::AppError;
use crate::events::{ChangeKind, WatchedKind};
use crate::state::AppState;

use super::exec::{git, GitOpts};
use super::ops::{emit_changes, require_repo};

/// Stage a single file (tracked or untracked) — `git add -- <path>`.
#[tauri::command]
pub async fn stage_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Index]);
    git(&handle.path, &["add", "--", &path], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Index]);
    Ok(())
}

/// Unstage a single file — `git reset -- <path>`.
#[tauri::command]
pub async fn unstage_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Index]);
    git(&handle.path, &["reset", "--", &path], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Index]);
    Ok(())
}

/// Stage every unstaged change (tracked + untracked) — the header's Stage All (§6.1).
#[tauri::command]
pub async fn stage_all(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Index]);
    git(&handle.path, &["add", "-A"], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Index]);
    Ok(())
}

/// Unstage everything — the header's Unstage All (§6.1).
#[tauri::command]
pub async fn unstage_all(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Index]);
    git(&handle.path, &["reset"], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Index]);
    Ok(())
}
