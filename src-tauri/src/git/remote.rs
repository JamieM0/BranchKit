//! Network operations — ARCHITECTURE.md §7.1/§3.1/§7.2. Fetch, pull, push and publish all run
//! through the repo's op queue like every other mutation, but additionally stream `--progress`
//! stderr as `OperationProgress` events on the repo's own `repo://{id}/changed` channel (unlike
//! `clone_repo`, which has no repo id yet — see repo.rs) so the toolbar/tab can show it live.
//! Force is **always `--force-with-lease`**, never plain `--force` (ARCHITECTURE.md §7.1) —
//! there is no code path here that can reach a bare `--force`.
//!
//! Auto-fetch (§7.2) also lives here: one tokio interval per open repo, gated on window focus and
//! on not having fetched too recently, that quietly runs the same fetch this module's `fetch_all`
//! command runs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, State};

use crate::credentials;
use crate::error::AppError;
use crate::events::{ChangeKind, WatchedKind};
use crate::settings;
use crate::state::{AppState, RepoHandle};

use super::exec::{git_with_progress, GitOpts};
use super::ops::{emit_changes, require_repo};

/// Prepends the credential-helper `-c` args (ARCHITECTURE.md §8) to a network command's argument
/// list — they must come before the git subcommand name.
fn with_credential_helper<'a>(helper: &'a [String], rest: &[&'a str]) -> Vec<&'a str> {
    let mut args: Vec<&str> = helper.iter().map(String::as_str).collect();
    args.extend(rest);
    args
}

/// How often the auto-fetch interval ticks to check whether a fetch is due — not the fetch
/// interval itself (that's `AUTO_FETCH_INTERVAL`), just how often we check the gates.
const AUTO_FETCH_TICK: Duration = Duration::from_secs(15);
/// Default auto-fetch interval — ARCHITECTURE.md §7.2 "setting; default 1min". Comfortably wider
/// than the "ran <30s ago" manual-fetch guard, so one `fetched_within` check against this covers
/// both "due for its regular interval" and "a manual fetch just happened".
const AUTO_FETCH_INTERVAL: Duration = Duration::from_secs(60);

fn progress_emitter(app: AppHandle, repo_id: String) -> impl FnMut(super::exec::ProgressUpdate) {
    move |update| {
        let _ = app.emit(
            &format!("repo://{repo_id}/changed"),
            ChangeKind::OperationProgress {
                phase: update.phase,
                percent: Some(update.percent),
            },
        );
    }
}

/// Fetch every remote, pruning deleted tracking refs — the toolbar dropdown's "Fetch all" and the
/// auto-fetch loop's underlying op (ARCHITECTURE.md §7.1/§7.2).
#[tauri::command]
pub async fn fetch_all(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Refs, WatchedKind::Remote]);
    let helper = credentials::helper_config_args();
    let result = git_with_progress(
        &handle.path,
        &with_credential_helper(&helper, &["fetch", "--all", "--prune", "--progress"]),
        GitOpts::network(),
        progress_emitter(app.clone(), repo_id.clone()),
    )
    .await;
    handle.record_fetch();
    emit_changes(&app, &repo_id, &[ChangeKind::Refs, ChangeKind::Remote]);
    result?;
    Ok(())
}

/// Pull the current branch — the ahead/behind fix-it popover's Pull actions (DESIGN_SPEC.md
/// §4.4/§15.7). `mode` is `ff` (`--ff-only`), `rebase` (`--rebase`) or `merge` (`--no-rebase`).
/// A conflict leaves the repo mid-operation for the graph to surface.
#[tauri::command]
pub async fn pull(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    mode: String,
) -> Result<(), AppError> {
    let mode_flag = match mode.as_str() {
        "rebase" => "--rebase",
        "merge" => "--no-rebase",
        _ => "--ff-only",
    };
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[
        WatchedKind::Head,
        WatchedKind::Refs,
        WatchedKind::Remote,
        WatchedKind::WorkingTree,
        WatchedKind::Index,
    ]);
    let helper = credentials::helper_config_args();
    let result = git_with_progress(
        &handle.path,
        &with_credential_helper(&helper, &["pull", mode_flag, "--progress"]),
        GitOpts::network(),
        progress_emitter(app.clone(), repo_id.clone()),
    )
    .await;
    handle.record_fetch();
    emit_changes(&app, &repo_id, &[ChangeKind::Head]);
    result?;
    Ok(())
}

/// Push the current branch to its upstream — the badge popover's Push / Force push actions. Force
/// is **always `--force-with-lease`**, never `--force` (ARCHITECTURE.md §7.1). When the
/// `push_tags_with_commits` Git setting is on, `--follow-tags` is appended so annotated tags
/// reachable from the pushed commits go along (GITKRAKEN_WORKFLOWS.md §2.9 "push tags on push").
#[tauri::command]
pub async fn push(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    force: bool,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Refs, WatchedKind::Remote]);
    let mut args = vec!["push", "--progress"];
    if force {
        args.push("--force-with-lease");
    }
    if settings::get_settings(app.clone())?.git.push_tags_with_commits {
        args.push("--follow-tags");
    }
    let helper = credentials::helper_config_args();
    let result = git_with_progress(
        &handle.path,
        &with_credential_helper(&helper, &args),
        GitOpts::network(),
        progress_emitter(app.clone(), repo_id.clone()),
    )
    .await;
    emit_changes(&app, &repo_id, &[ChangeKind::Refs, ChangeKind::Remote]);
    result?;
    Ok(())
}

/// Push a branch with no upstream yet, setting `origin/<name>` as its tracking ref in the same
/// action — the toolbar's Push-becomes-**Publish** state (DESIGN_SPEC.md §3.2/§9). Honors the
/// `push_tags_with_commits` setting the same way [`push`] does.
#[tauri::command]
pub async fn publish(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Refs, WatchedKind::Remote]);
    let mut args = vec!["push", "--progress", "-u", "origin"];
    let push_tags = settings::get_settings(app.clone())?.git.push_tags_with_commits;
    if push_tags {
        args.push("--follow-tags");
    }
    args.push(&name);
    let helper = credentials::helper_config_args();
    let result = git_with_progress(
        &handle.path,
        &with_credential_helper(&helper, &args),
        GitOpts::network(),
        progress_emitter(app.clone(), repo_id.clone()),
    )
    .await;
    emit_changes(&app, &repo_id, &[ChangeKind::Refs, ChangeKind::Remote]);
    result?;
    Ok(())
}

/// Starts the auto-fetch interval for a just-opened repo — ARCHITECTURE.md §7.2. Ticks every
/// [`AUTO_FETCH_TICK`]; on each tick, fetches only if the window is focused, at least
/// [`AUTO_FETCH_INTERVAL`] has passed since the last fetch, and the op queue isn't currently held
/// (a `try_lock` — auto-fetch never waits behind a foreground op, it just skips this tick).
/// Cancelled by dropping/aborting the returned handle, which `close_repo` does via
/// `RepoHandle::auto_fetch_task`.
pub fn spawn_auto_fetch(
    app: AppHandle,
    handle: Arc<RepoHandle>,
    focused: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    let repo_id = handle.id.0.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(AUTO_FETCH_TICK);
        loop {
            ticker.tick().await;
            if !focused.load(Ordering::SeqCst) {
                continue;
            }
            // §7.2: skip if a fetch (auto or manual) already ran within the interval — this one
            // check covers both "due for its regular interval" and "a manual fetch just happened".
            if handle.fetched_within(AUTO_FETCH_INTERVAL) {
                continue;
            }
            let Ok(_guard) = handle.op_queue.try_lock() else {
                // An op is queued/running — skip this tick rather than wait behind it.
                continue;
            };
            handle.begin_self_op(&[WatchedKind::Refs, WatchedKind::Remote]);
            let helper = credentials::helper_config_args();
            let result = git_with_progress(
                &handle.path,
                &with_credential_helper(&helper, &["fetch", "--all", "--prune", "--progress"]),
                GitOpts::network(),
                progress_emitter(app.clone(), repo_id.clone()),
            )
            .await;
            handle.record_fetch();
            if result.is_ok() {
                emit_changes(&app, &repo_id, &[ChangeKind::Refs, ChangeKind::Remote]);
            }
            // Best-effort: auto-fetch failures (offline, auth) are silent — the toolbar's fetch
            // button and the offline indicator (via manual/foreground ops) are the surfaces for
            // that, not a background timer.
        }
    })
}
