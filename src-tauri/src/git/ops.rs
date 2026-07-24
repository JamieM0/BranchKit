//! Mutating git operations — ARCHITECTURE.md §7.1. Every command here runs through the repo's
//! serial op queue (so we never collide with ourselves on `index.lock`), opens a watcher
//! self-echo suppression window for the kinds it will churn (§4), and — on success — emits the
//! targeted `repo://{id}/changed` refresh itself, so the UI updates exactly once, immediately.
//!
//! These are the ops that back the branch pills, checkout gestures and drag-merge/rebase/ff drop
//! menu (DESIGN_SPEC.md §4.4) plus the left-panel branch operations (§5). Conflict *resolution*
//! UX comes later (prompts 12–13); a merge/rebase that conflicts here just returns the raw git
//! error and leaves the repo in its conflicted state for the graph to surface.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::error::AppError;
use crate::events::{ChangeKind, WatchedKind};
use crate::state::{AppState, RepoHandle};

use super::exec::{git, GitError, GitOpts};

const UNIT_SEP: char = '\u{1f}';
const RECORD_SEP: char = '\u{1e}';

/// Emits the targeted refresh(es) for an op that just completed — ARCHITECTURE.md §2/§4.
pub(crate) fn emit_changes(app: &AppHandle, repo_id: &str, kinds: &[ChangeKind]) {
    let channel = format!("repo://{repo_id}/changed");
    for kind in kinds {
        let _ = app.emit(&channel, kind.clone());
    }
}

/// Resolves the handle for `repo_id` or the standard "not open" error. Shared across `git/`
/// command modules (stage.rs, diff.rs) so the "not open" error stays worded identically.
pub(crate) fn require_repo(
    state: &State<'_, AppState>,
    repo_id: &str,
) -> Result<std::sync::Arc<RepoHandle>, AppError> {
    state.get_repo(repo_id).ok_or_else(|| {
        AppError::new(
            "Repository is not open",
            format!("unknown repo id {repo_id}"),
        )
    })
}

/// Splits a remote-tracking ref short name (`origin/feature/x`) into `(remote, branch)`
/// (`("origin", "feature/x")`). Splits on the *first* slash so branch names may contain slashes.
pub fn split_remote_ref(short_name: &str) -> Option<(&str, &str)> {
    short_name.split_once('/')
}

/// One line of `git log` output for divergence previews — DESIGN_SPEC.md §4.4 (badge tooltip +
/// the popover's "view commits").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitLine {
    pub sha: String,
    pub subject: String,
}

/// Outgoing (`↑` to push) and incoming (`↓` to pull) commits relative to a branch's upstream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Divergence {
    pub outgoing: Vec<CommitLine>,
    pub incoming: Vec<CommitLine>,
}

fn parse_commit_lines(text: &str) -> Vec<CommitLine> {
    text.split(RECORD_SEP)
        .filter_map(|record| {
            let record = record.trim_start_matches('\n');
            if record.trim().is_empty() {
                return None;
            }
            let mut fields = record.splitn(2, UNIT_SEP);
            let sha = fields.next()?.to_string();
            let subject = fields.next().unwrap_or("").trim_end_matches('\n').to_string();
            Some(CommitLine { sha, subject })
        })
        .collect()
}

async fn log_range(repo: &Path, range: &str, limit: usize) -> Result<Vec<CommitLine>, GitError> {
    let format = format!("--pretty=format:%H{UNIT_SEP}%s{RECORD_SEP}");
    let max = format!("-n{limit}");
    let output = git(repo, &["log", &max, &format, range], GitOpts::default()).await?;
    Ok(parse_commit_lines(&String::from_utf8_lossy(&output.stdout)))
}

/// Records a branch's current tip so a later delete can be undone (DESIGN_SPEC.md §15.13).
async fn branch_tip(repo: &Path, branch: &str) -> Result<String, GitError> {
    let refname = format!("refs/heads/{branch}");
    let output = git(repo, &["rev-parse", &refname], GitOpts::default()).await?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// --- commands --------------------------------------------------------------

/// Checkout an existing local branch — `git checkout <name>`. Emits Head (the frontend reloads the
/// head window + refs) — ARCHITECTURE.md §7.1.
#[tauri::command]
pub async fn checkout_branch(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Head, WatchedKind::WorkingTree, WatchedKind::Index]);
    git(&handle.path, &["checkout", &name], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Head]);
    Ok(())
}

/// Create a local tracking branch for a remote-tracking ref and check it out in one action — the
/// sacred GK workflow (DESIGN_SPEC.md §4.4/§15.1). `remote_ref` is a short name like
/// `origin/feature/x`; the local branch takes the branch portion (`feature/x`). If the local name
/// already exists, falls back to a plain checkout that sets the upstream.
#[tauri::command]
pub async fn checkout_remote(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    remote_ref: String,
) -> Result<String, AppError> {
    let (_, local) = split_remote_ref(&remote_ref).ok_or_else(|| {
        AppError::new(
            format!("\"{remote_ref}\" is not a remote branch"),
            "expected a remote/branch short name".to_string(),
        )
    })?;
    let local = local.to_string();
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Head, WatchedKind::Refs, WatchedKind::WorkingTree, WatchedKind::Index]);

    let local_exists = branch_tip(&handle.path, &local).await.is_ok();
    if local_exists {
        // Already have it locally — just check it out and (re)point its upstream.
        git(&handle.path, &["checkout", &local], GitOpts::default()).await?;
        let _ = git(
            &handle.path,
            &["branch", &format!("--set-upstream-to={remote_ref}"), &local],
            GitOpts::default(),
        )
        .await;
    } else {
        git(
            &handle.path,
            &["checkout", "-b", &local, "--track", &remote_ref],
            GitOpts::default(),
        )
        .await?;
    }
    emit_changes(&app, &repo_id, &[ChangeKind::Head, ChangeKind::Refs]);
    Ok(local)
}

/// Checkout the previously-checked-out ref — `git checkout -` (the checkout toast's **Back**
/// action, DESIGN_SPEC.md §8/§15.14).
#[tauri::command]
pub async fn checkout_previous(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Head, WatchedKind::WorkingTree, WatchedKind::Index]);
    git(&handle.path, &["checkout", "-"], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Head]);
    Ok(())
}

/// Detached checkout of a commit — `git checkout --detach <sha>` (double-click a commit,
/// DESIGN_SPEC.md §4.6).
#[tauri::command]
pub async fn checkout_detached(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Head, WatchedKind::WorkingTree, WatchedKind::Index]);
    git(&handle.path, &["checkout", "--detach", &sha], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Head]);
    Ok(())
}

/// Stash-and-checkout: the ARCHITECTURE.md §9 "would be overwritten by checkout" error's
/// suggested compound action. Stashes uncommitted changes (including untracked), checks out
/// `name`, then pops the stash back — so switching branches never forces the user to lose or
/// manually shelve work first. If the pop conflicts, the stash is left in place (not dropped) and
/// the conflict surfaces normally (§7.4) rather than being silently swallowed here.
#[tauri::command]
pub async fn checkout_stash_and_switch(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[
        WatchedKind::Head,
        WatchedKind::Refs,
        WatchedKind::WorkingTree,
        WatchedKind::Index,
    ]);
    git(
        &handle.path,
        &["stash", "push", "-u", "-m", "Before switching branches"],
        GitOpts::default(),
    )
    .await?;
    let checkout_result = git(&handle.path, &["checkout", &name], GitOpts::default()).await;
    if let Err(e) = checkout_result {
        // Checkout itself failed even with a clean tree — restore the stash so nothing is lost,
        // then surface the original error.
        let _ = git(&handle.path, &["stash", "pop"], GitOpts::default()).await;
        emit_changes(&app, &repo_id, &[ChangeKind::Head]);
        return Err(e.into());
    }
    let pop_result = git(&handle.path, &["stash", "pop"], GitOpts::default()).await;
    emit_changes(
        &app,
        &repo_id,
        &[ChangeKind::Head, ChangeKind::WorkingTree, ChangeKind::Index],
    );
    pop_result?;
    Ok(())
}

/// Create a branch at `sha` (default HEAD). When `checkout` is true, `git checkout -b` so the new
/// branch is entered immediately (the inline name-editor-at-HEAD flow, DESIGN_SPEC.md §15.  It's
/// "create branch here", GITKRAKEN_WORKFLOWS §2.3).
#[tauri::command]
pub async fn create_branch(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
    sha: Option<String>,
    checkout: bool,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    let start = sha.as_deref();
    if checkout {
        handle.begin_self_op(&[WatchedKind::Head, WatchedKind::Refs, WatchedKind::WorkingTree]);
        let mut args = vec!["checkout", "-b", &name];
        if let Some(start) = start {
            args.push(start);
        }
        git(&handle.path, &args, GitOpts::default()).await?;
        emit_changes(&app, &repo_id, &[ChangeKind::Head, ChangeKind::Refs]);
    } else {
        handle.begin_self_op(&[WatchedKind::Refs]);
        let mut args = vec!["branch", &name];
        if let Some(start) = start {
            args.push(start);
        }
        git(&handle.path, &args, GitOpts::default()).await?;
        emit_changes(&app, &repo_id, &[ChangeKind::Refs]);
    }
    Ok(())
}

/// Rename a branch — `git branch -m <old> <new>`.
#[tauri::command]
pub async fn rename_branch(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    old_name: String,
    new_name: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Refs, WatchedKind::Head]);
    git(&handle.path, &["branch", "-m", &old_name, &new_name], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Refs, ChangeKind::Head]);
    Ok(())
}

/// Delete a branch, returning its recorded tip sha so a toast can offer **Undo** (recreate at that
/// sha) — DESIGN_SPEC.md §15.13. `force` selects `-D` (past the not-fully-merged guard); the
/// frontend only sets it after an armed confirm.
#[tauri::command]
pub async fn delete_branch(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
    force: bool,
) -> Result<String, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    // Capture the tip *before* deleting so Undo can recreate it exactly.
    let sha = branch_tip(&handle.path, &name).await?;
    handle.begin_self_op(&[WatchedKind::Refs]);
    let flag = if force { "-D" } else { "-d" };
    git(&handle.path, &["branch", flag, "--", &name], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Refs]);
    Ok(sha)
}

/// Recreate a branch at a recorded sha — the branch-delete toast's **Undo** (DESIGN_SPEC.md
/// §15.13).
#[tauri::command]
pub async fn recreate_branch(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
    sha: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Refs]);
    git(&handle.path, &["branch", &name, &sha], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Refs]);
    Ok(())
}

/// Merge `source` into the current branch — `git merge --no-edit <source>` (drag pill → "Merge X
/// into Y" with Y checked out, DESIGN_SPEC.md §4.4). A conflict returns the raw git error and
/// leaves the repo mid-merge; full conflict UX arrives in prompts 12–13. `allow_unrelated` passes
/// `--allow-unrelated-histories` — the retry the ARCHITECTURE.md §9 "refusing to merge unrelated
/// histories" suggestion offers; the frontend always calls this with `false` first and only
/// retries with `true` after the user picks that suggestion.
#[tauri::command]
pub async fn merge_ref(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    source: String,
    allow_unrelated: bool,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Head, WatchedKind::Refs, WatchedKind::WorkingTree, WatchedKind::Index]);
    let mut args = vec!["merge", "--no-edit"];
    if allow_unrelated {
        args.push("--allow-unrelated-histories");
    }
    args.push(&source);
    let result = git(&handle.path, &args, GitOpts::default()).await;
    // Whether it merged cleanly or hit a conflict, HEAD/worktree changed — refresh either way.
    emit_changes(&app, &repo_id, &[ChangeKind::Head]);
    result?;
    Ok(())
}

/// Rebase the current branch onto `onto` — `git rebase <onto>` (drag pill → "Rebase X onto Y").
#[tauri::command]
pub async fn rebase_onto(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    onto: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Head, WatchedKind::Refs, WatchedKind::WorkingTree, WatchedKind::Index]);
    let result = git(&handle.path, &["rebase", &onto], GitOpts::default()).await;
    emit_changes(&app, &repo_id, &[ChangeKind::Head]);
    result?;
    Ok(())
}

/// Fast-forward `branch` to `source` — the drop menu's "Fast-forward Y to X" (DESIGN_SPEC.md
/// §4.4), only offered when it's actually a fast-forward. If `branch` is checked out this is
/// `git merge --ff-only <source>`; otherwise it moves the ref without checkout via
/// `git fetch . <source>:<branch>` (which refuses a non-ff, so it stays safe).
#[tauri::command]
pub async fn fast_forward(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    branch: String,
    source: String,
    is_current: bool,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    if is_current {
        handle.begin_self_op(&[WatchedKind::Head, WatchedKind::WorkingTree, WatchedKind::Index]);
        git(&handle.path, &["merge", "--ff-only", &source], GitOpts::default()).await?;
        emit_changes(&app, &repo_id, &[ChangeKind::Head]);
    } else {
        handle.begin_self_op(&[WatchedKind::Refs]);
        let refspec = format!("{source}:{branch}");
        git(&handle.path, &["fetch", ".", &refspec], GitOpts::default()).await?;
        emit_changes(&app, &repo_id, &[ChangeKind::Refs]);
    }
    Ok(())
}

/// Toggle a branch's upstream tracking — GITKRAKEN_WORKFLOWS §3.2 "Set upstream".
#[tauri::command]
pub async fn set_upstream(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    branch: String,
    upstream: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Refs]);
    git(
        &handle.path,
        &["branch", &format!("--set-upstream-to={upstream}"), &branch],
        GitOpts::default(),
    )
    .await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Refs]);
    Ok(())
}

/// Outgoing/incoming commit lists for a branch relative to its upstream. Returns empty lists when
/// the branch has no upstream. Split out from the command so it's testable against a real repo.
pub async fn divergence(repo: &Path, branch: &str) -> Result<Divergence, GitError> {
    // `@{upstream}` only resolves when tracking is configured; probe it first so a branch without
    // an upstream returns cleanly rather than erroring.
    let upstream_ref = format!("{branch}@{{upstream}}");
    let has_upstream = git(
        repo,
        &["rev-parse", "--abbrev-ref", &upstream_ref],
        GitOpts::default(),
    )
    .await
    .is_ok();
    if !has_upstream {
        return Ok(Divergence::default());
    }
    let outgoing_range = format!("{upstream_ref}..{branch}");
    let incoming_range = format!("{branch}..{upstream_ref}");
    let outgoing = log_range(repo, &outgoing_range, 50).await?;
    let incoming = log_range(repo, &incoming_range, 50).await?;
    Ok(Divergence { outgoing, incoming })
}

/// Outgoing/incoming commit lists for a branch relative to its upstream — powers the ahead/behind
/// badge tooltip (up to 5) and the fix-it popover's "view commits" (DESIGN_SPEC.md §4.4).
#[tauri::command]
pub async fn branch_divergence(
    state: State<'_, AppState>,
    repo_id: String,
    branch: String,
) -> Result<Divergence, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(divergence(&handle.path, &branch).await?)
}

/// Cherry-pick a commit onto HEAD — the commit row menu's "Cherry-pick commit"
/// (GITKRAKEN_WORKFLOWS.md §2.6/§3.1). A conflict leaves the repo mid-cherry-pick for the graph to
/// surface, same as merge/rebase.
#[tauri::command]
pub async fn cherry_pick(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Head, WatchedKind::WorkingTree, WatchedKind::Index]);
    let result = git(&handle.path, &["cherry-pick", &sha], GitOpts::default()).await;
    emit_changes(&app, &repo_id, &[ChangeKind::Head]);
    result?;
    Ok(())
}

/// Revert a commit, committing the inverse immediately — the commit row menu's "Revert commit"
/// (GITKRAKEN_WORKFLOWS.md §2.6/§3.1). A conflict leaves the repo mid-revert for the graph to
/// surface.
#[tauri::command]
pub async fn revert_commit(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Head, WatchedKind::WorkingTree, WatchedKind::Index]);
    let result = git(&handle.path, &["revert", "--no-edit", &sha], GitOpts::default()).await;
    emit_changes(&app, &repo_id, &[ChangeKind::Head]);
    result?;
    Ok(())
}

/// Reset the current branch to `sha` — the commit row menu's "Reset `<current>` to this commit"
/// submenu (Soft/Mixed/Hard, GITKRAKEN_WORKFLOWS.md §2.6/§3.1, guarded per DESIGN_SPEC.md §4.6 in
/// the frontend for Hard). `mode` is `"soft"`, `"mixed"`, or anything else falls back to `"hard"`.
#[tauri::command]
pub async fn reset_to(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
    mode: String,
) -> Result<(), AppError> {
    let flag = match mode.as_str() {
        "soft" => "--soft",
        "mixed" => "--mixed",
        _ => "--hard",
    };
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Head, WatchedKind::WorkingTree, WatchedKind::Index]);
    git(&handle.path, &["reset", flag, &sha], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Head]);
    Ok(())
}

/// Create a tag at `sha` — lightweight, or annotated when `message` is non-empty (the commit row
/// menu's "Create tag here" / "Create annotated tag here", GITKRAKEN_WORKFLOWS.md §2.9/§3.1).
#[tauri::command]
pub async fn create_tag(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
    sha: String,
    message: Option<String>,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Refs]);
    let message = message.filter(|m| !m.trim().is_empty());
    let result = match message.as_deref() {
        Some(m) => git(&handle.path, &["tag", "-a", "-m", m, &name, &sha], GitOpts::default()).await,
        None => git(&handle.path, &["tag", &name, &sha], GitOpts::default()).await,
    };
    emit_changes(&app, &repo_id, &[ChangeKind::Refs]);
    result?;
    Ok(())
}

/// Returns whether `origin` is configured. Repositories without a remote still support the
/// local-only form of tag deletion.
async fn has_origin(repo: &Path) -> Result<bool, GitError> {
    let output = git(repo, &["remote"], GitOpts::default()).await?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|remote| remote == "origin"))
}

/// Delete a tag from `origin` (when configured) and locally. The remote is updated first so a
/// failed push leaves the local tag intact rather than silently creating a local/remote mismatch.
#[tauri::command]
pub async fn delete_tag(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Refs]);

    if has_origin(&handle.path).await? {
        let remote_ref = format!("refs/tags/{name}");
        git(
            &handle.path,
            &["push", "--progress", "origin", "--delete", &remote_ref],
            GitOpts::network(),
        )
        .await?;
    }
    git(&handle.path, &["tag", "-d", &name], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Refs]);
    Ok(())
}

/// Delete only the local tag, leaving any tag published to `origin` untouched.
#[tauri::command]
pub async fn delete_local_tag(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Refs]);
    git(&handle.path, &["tag", "-d", &name], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Refs]);
    Ok(())
}

/// The configured URL for `remote` — used to build "Copy link to this commit on remote" (the
/// frontend turns this into a GitHub/GitLab web URL; unsupported hosts just fall back to copying
/// the sha, GITKRAKEN_WORKFLOWS.md §2.9/§3.1).
#[tauri::command]
pub async fn get_remote_url(
    state: State<'_, AppState>,
    repo_id: String,
    remote: String,
) -> Result<String, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let output = git(&handle.path, &["remote", "get-url", &remote], GitOpts::default()).await?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Configured remote names (`git remote`) — lets the toolbar disable Publish (with an explanatory
/// tooltip) on a repo that has no remote at all, instead of letting the push fail with git's raw
/// "'origin' does not appear to be a git repository" error.
#[tauri::command]
pub async fn list_remotes(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<String>, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let output = git(&handle.path, &["remote"], GitOpts::default()).await?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

/// `git remote add <name> <url>` — the counterpart to `list_remotes`'s "no remote" check. Used by
/// the GitHub "create repo & publish" flow (ARCHITECTURE.md §11, SPEC-DEVIATION: §11 only scoped
/// six read/PR endpoints, this adds repo creation) once a fresh GitHub repo exists but the local
/// repo has nothing to push to yet. Errors (e.g. `name` already configured) surface as-is — git's
/// own message is clear enough here.
#[tauri::command]
pub async fn add_remote(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
    url: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Remote]);
    let result = git(&handle.path, &["remote", "add", &name, &url], GitOpts::default()).await;
    emit_changes(&app, &repo_id, &[ChangeKind::Remote]);
    result?;
    Ok(())
}

/// Appends `pattern` as a new line to the repo root's `.gitignore` (creating it if needed) — the
/// file row menu's Ignore submenu (this file / by extension / folder, GITKRAKEN_WORKFLOWS.md
/// §2.9/§3.4). A no-op (no watcher suppression needed beyond WorkingTree) when the pattern is
/// already present verbatim on its own line.
#[tauri::command]
pub async fn ignore_path(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    pattern: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    let gitignore = handle.path.join(".gitignore");
    let existing = std::fs::read_to_string(&gitignore).unwrap_or_default();
    if existing.lines().any(|l| l == pattern) {
        return Ok(());
    }
    handle.begin_self_op(&[WatchedKind::WorkingTree]);
    let mut contents = existing;
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(&pattern);
    contents.push('\n');
    std::fs::write(&gitignore, contents)?;
    emit_changes(&app, &repo_id, &[ChangeKind::WorkingTree]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_remote_ref_on_first_slash() {
        assert_eq!(split_remote_ref("origin/main"), Some(("origin", "main")));
        assert_eq!(
            split_remote_ref("origin/feature/x"),
            Some(("origin", "feature/x"))
        );
        assert_eq!(split_remote_ref("nothing"), None);
    }

    #[test]
    fn parses_commit_lines() {
        let text = format!("abc{UNIT_SEP}First{RECORD_SEP}\ndef{UNIT_SEP}Second{RECORD_SEP}");
        let lines = parse_commit_lines(&text);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].sha, "abc");
        assert_eq!(lines[0].subject, "First");
        assert_eq!(lines[1].subject, "Second");
    }
}
