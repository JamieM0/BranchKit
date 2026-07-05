//! Conflict state detection and Keep Panel region computation — ARCHITECTURE.md §7.4/§7.5,
//! DESIGN_SPEC.md §9. Two independent halves live in this one module (one command family):
//!
//! 1. **State detection**: probe `.git` for the in-progress operation (merge/rebase/cherry-pick/
//!    revert/stash-apply) and label it with real branch names, plus continue/abort commands.
//! 2. **Region computation**: for each conflicted file, diff `base` (`:1:`) against `ours` (`:2:`)
//!    and `theirs` (`:3:`) with the `similar` crate's Myers implementation, then walk both diffs
//!    together to classify every base line as context / auto-resolved / a conflict region. We
//!    never parse `<<<<<<<` markers — that breaks on marker-like content in real files and on
//!    diff3-style markers; this is an exact structural computation instead.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use similar::{capture_diff_slices, Algorithm, DiffOp};
use tauri::{AppHandle, State};

use crate::error::AppError;
use crate::events::{ChangeKind, WatchedKind};
use crate::state::AppState;

use super::exec::{git, git_with_env, GitError, GitOpts};
use super::ops::{emit_changes, require_repo};
use super::status::{status, StatusEntryKind};

// --- state detection — ARCHITECTURE.md §7.4 ---------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    StashApply,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictState {
    pub kind: ConflictKind,
    /// The incoming side, in plain words — a branch name where possible, never "ours/theirs"
    /// (DESIGN_SPEC.md §9.1's banner text, §4 principle "plain words, real names").
    pub source_label: String,
    /// The branch (or ref) the operation is being applied onto — usually the checked-out branch.
    pub target_label: String,
    pub files: Vec<String>,
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

async fn git_dir(repo: &Path) -> Option<PathBuf> {
    git(repo, &["rev-parse", "--absolute-git-dir"], GitOpts::default())
        .await
        .ok()
        .map(|out| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string()))
}

/// Resolves a sha to a human ref name via `git name-rev`, falling back to a short sha when git
/// can't name it (e.g. the branch that pointed at it has since moved or been deleted).
async fn resolve_commit_label(repo: &Path, sha: &str) -> String {
    if let Ok(out) = git(
        repo,
        &["name-rev", "--name-only", "--no-undefined", sha],
        GitOpts::default(),
    )
    .await
    {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    short_sha(sha)
}

/// A commit's subject line, for cherry-pick/revert source labels — falls back to a short sha.
async fn commit_subject_label(repo: &Path, sha: &str) -> String {
    if let Ok(out) = git(repo, &["log", "-1", "--format=%s", sha], GitOpts::default()).await {
        let subject = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !subject.is_empty() {
            return subject;
        }
    }
    short_sha(sha)
}

/// Pulls the single-quoted ref name out of `MERGE_MSG`'s first line (`"Merge branch 'feature/x'
/// into main"` → `"feature/x"`) — real branch names beat a generic sha resolution when available.
fn extract_quoted_name(first_line: &str) -> Option<String> {
    let start = first_line.find('\'')? + 1;
    let rest = &first_line[start..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

async fn merge_source_label(repo: &Path, git_dir: &Path) -> String {
    if let Ok(msg) = std::fs::read_to_string(git_dir.join("MERGE_MSG")) {
        if let Some(first_line) = msg.lines().next() {
            if let Some(name) = extract_quoted_name(first_line) {
                return name;
            }
        }
    }
    if let Ok(sha) = std::fs::read_to_string(git_dir.join("MERGE_HEAD")) {
        return resolve_commit_label(repo, sha.trim()).await;
    }
    "the other branch".to_string()
}

/// `head-name` (the branch being rebased) and `onto` (resolved to a label) from the rebase state
/// dir — works for both the interactive (`rebase-merge`) and legacy (`rebase-apply`) layouts.
async fn rebase_labels(repo: &Path, git_dir: &Path) -> (String, Option<String>) {
    let state_dir = if git_dir.join("rebase-merge").is_dir() {
        git_dir.join("rebase-merge")
    } else {
        git_dir.join("rebase-apply")
    };
    let source_label = std::fs::read_to_string(state_dir.join("head-name"))
        .ok()
        .map(|s| s.trim().trim_start_matches("refs/heads/").to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "the branch being rebased".to_string());
    let onto_label = match std::fs::read_to_string(state_dir.join("onto")) {
        Ok(sha) => Some(resolve_commit_label(repo, sha.trim()).await),
        Err(_) => None,
    };
    (source_label, onto_label)
}

/// Probes `.git` for an in-progress merge/rebase/cherry-pick/revert, falling back to "stash-apply"
/// when there are unmerged paths but none of those op markers are present — ARCHITECTURE.md §7.4.
/// Returns `None` when the working tree has no conflicts at all.
pub async fn detect_conflict_state(repo: &Path) -> Result<Option<ConflictState>, GitError> {
    let report = status(repo).await?;
    let files: Vec<String> = report
        .entries
        .iter()
        .filter(|e| e.kind == StatusEntryKind::Unmerged)
        .map(|e| e.path.clone())
        .collect();

    let target_label = report.branch.head.clone().unwrap_or_else(|| "HEAD".to_string());
    let Some(dir) = git_dir(repo).await else {
        return Ok(None);
    };

    // Check the op-marker files *before* looking at `files`: once every conflict is resolved and
    // staged there are no more unmerged entries, but the operation (and its Continue/Abort) is
    // still in progress until the user explicitly finishes it (DESIGN_SPEC.md §9.1's "0 of N
    // conflicts" banner state). Only fall back to requiring `files` for the stash-apply case,
    // which has no marker file of its own.

    if dir.join("MERGE_HEAD").is_file() {
        let source_label = merge_source_label(repo, &dir).await;
        return Ok(Some(ConflictState {
            kind: ConflictKind::Merge,
            source_label,
            target_label,
            files,
        }));
    }
    if dir.join("rebase-merge").is_dir() || dir.join("rebase-apply").is_dir() {
        let (source_label, onto_label) = rebase_labels(repo, &dir).await;
        return Ok(Some(ConflictState {
            kind: ConflictKind::Rebase,
            source_label,
            target_label: onto_label.unwrap_or(target_label),
            files,
        }));
    }
    if dir.join("CHERRY_PICK_HEAD").is_file() {
        let sha = std::fs::read_to_string(dir.join("CHERRY_PICK_HEAD")).unwrap_or_default();
        let source_label = commit_subject_label(repo, sha.trim()).await;
        return Ok(Some(ConflictState {
            kind: ConflictKind::CherryPick,
            source_label,
            target_label,
            files,
        }));
    }
    if dir.join("REVERT_HEAD").is_file() {
        let sha = std::fs::read_to_string(dir.join("REVERT_HEAD")).unwrap_or_default();
        let source_label = commit_subject_label(repo, sha.trim()).await;
        return Ok(Some(ConflictState {
            kind: ConflictKind::Revert,
            source_label,
            target_label,
            files,
        }));
    }

    if files.is_empty() {
        // No op marker file, and nothing left unmerged — no conflict of any kind is active.
        return Ok(None);
    }

    // Unmerged paths with no op-marker file present — a `stash apply`/`stash pop` conflict, the
    // one case with no ref of its own to label (DESIGN_SPEC.md §9.1: label it "stash").
    Ok(Some(ConflictState {
        kind: ConflictKind::StashApply,
        source_label: "stash".to_string(),
        target_label,
        files,
    }))
}

#[tauri::command]
pub async fn get_conflict_state(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<Option<ConflictState>, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(detect_conflict_state(&handle.path).await?)
}

/// The actual continue logic, taking a repo path directly so it's testable against a plain
/// tempdir repo (`TestRepo`) instead of a live `AppHandle`/`RepoHandle`. `git commit --no-edit`
/// (merge) / `rebase --continue` / `cherry-pick --continue` / `revert --continue`, each with
/// `GIT_EDITOR=true` so git never opens an editor (ARCHITECTURE.md §7.4). A stash-apply conflict
/// has no "continue" of its own — once every file is resolved and staged there's nothing left for
/// git to do.
pub async fn continue_conflict_impl(repo: &Path, message: Option<&str>) -> Result<(), GitError> {
    let current = detect_conflict_state(repo).await?.ok_or_else(|| GitError {
        code: None,
        stderr: "no active conflict state".to_string(),
        cmd_summary: "continue conflict".to_string(),
        kind: super::exec::GitErrorKind::Spawn,
    })?;
    let editor_env = [("GIT_EDITOR", "true")];
    match current.kind {
        // The merge commit is the one place an edited message applies — DESIGN_SPEC.md §9.2's
        // inline "editable in a compact inline field" message. A blank/absent message falls back
        // to git's prefilled `MERGE_MSG` (`--no-edit`). Rebase/cherry-pick/revert reuse their own
        // stored messages and take no `-m` on `--continue`, so `message` is ignored for them.
        ConflictKind::Merge => match message.map(str::trim).filter(|m| !m.is_empty()) {
            Some(msg) => {
                git_with_env(repo, &["commit", "-m", msg], GitOpts::default(), &editor_env).await?;
            }
            None => {
                git_with_env(repo, &["commit", "--no-edit"], GitOpts::default(), &editor_env).await?;
            }
        },
        ConflictKind::Rebase => {
            git_with_env(repo, &["rebase", "--continue"], GitOpts::default(), &editor_env).await?;
        }
        ConflictKind::CherryPick => {
            git_with_env(repo, &["cherry-pick", "--continue"], GitOpts::default(), &editor_env).await?;
        }
        ConflictKind::Revert => {
            git_with_env(repo, &["revert", "--continue"], GitOpts::default(), &editor_env).await?;
        }
        ConflictKind::StashApply => {}
    }
    Ok(())
}

/// Continue the in-progress operation — DESIGN_SPEC.md §9.2's banner "Continue merge".
#[tauri::command]
pub async fn continue_conflict(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    message: Option<String>,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[
        WatchedKind::Head,
        WatchedKind::Refs,
        WatchedKind::WorkingTree,
        WatchedKind::Index,
    ]);
    let result = continue_conflict_impl(&handle.path, message.as_deref()).await;
    emit_changes(
        &app,
        &repo_id,
        &[ChangeKind::Head, ChangeKind::Refs, ChangeKind::WorkingTree, ChangeKind::Index],
    );
    result?;
    Ok(())
}

/// The actual abort logic, taking a repo path directly (same rationale as
/// [`continue_conflict_impl`]) — the corresponding `--abort` for merge/rebase/cherry-pick/revert
/// (ARCHITECTURE.md §7.4). A stash-apply conflict has no dedicated abort; `reset --merge` is
/// git's own documented recovery (restores the pre-apply working tree, leaving the stash entry
/// itself untouched in the stash list).
pub async fn abort_conflict_impl(repo: &Path) -> Result<(), GitError> {
    let current = detect_conflict_state(repo).await?.ok_or_else(|| GitError {
        code: None,
        stderr: "no active conflict state".to_string(),
        cmd_summary: "abort conflict".to_string(),
        kind: super::exec::GitErrorKind::Spawn,
    })?;
    match current.kind {
        ConflictKind::Merge => {
            git(repo, &["merge", "--abort"], GitOpts::default()).await?;
        }
        ConflictKind::Rebase => {
            git(repo, &["rebase", "--abort"], GitOpts::default()).await?;
        }
        ConflictKind::CherryPick => {
            git(repo, &["cherry-pick", "--abort"], GitOpts::default()).await?;
        }
        ConflictKind::Revert => {
            git(repo, &["revert", "--abort"], GitOpts::default()).await?;
        }
        ConflictKind::StashApply => {
            git(repo, &["reset", "--merge"], GitOpts::default()).await?;
        }
    }
    Ok(())
}

/// Abort the in-progress operation — DESIGN_SPEC.md §9.1/§9.3's banner "Abort…".
#[tauri::command]
pub async fn abort_conflict(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[
        WatchedKind::Head,
        WatchedKind::Refs,
        WatchedKind::WorkingTree,
        WatchedKind::Index,
    ]);
    let result = abort_conflict_impl(&handle.path).await;
    emit_changes(
        &app,
        &repo_id,
        &[ChangeKind::Head, ChangeKind::Refs, ChangeKind::WorkingTree, ChangeKind::Index],
    );
    result?;
    Ok(())
}

// --- Keep Panel region computer — ARCHITECTURE.md §7.5 -----------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Ours,
    Theirs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum FileRegion {
    /// Unchanged on both sides — plain document flow, no candidates.
    Context { lines: Vec<String> },
    /// Only one side touched this span — auto-resolved, no decision needed.
    AutoResolved { side: Side, lines: Vec<String> },
    /// Both sides touched overlapping base spans — a real Keep Panel decision. Lines identical on
    /// both sides at the region's edges are pre-dedupe'd out into `same_both_*` (DESIGN_SPEC.md
    /// §9.3): shown once, auto-kept, instead of asking the user to pick between two identical
    /// candidates.
    Conflict {
        base_start: usize,
        base_end: usize,
        same_both_prefix: Vec<String>,
        ours_lines: Vec<String>,
        theirs_lines: Vec<String>,
        same_both_suffix: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileConflictRegions {
    /// The file doesn't exist in `ours` (stage `:2:` missing) — a modify/delete conflict where
    /// this side deleted it.
    pub ours_deleted: bool,
    /// Same, for `theirs` (stage `:3:`).
    pub theirs_deleted: bool,
    pub regions: Vec<FileRegion>,
}

async fn read_stage(repo: &Path, stage: u8, path: &str) -> (Vec<String>, bool) {
    let spec = format!(":{stage}:{path}");
    match git(repo, &["show", &spec], GitOpts::default()).await {
        Ok(out) => (to_lines(&out.stdout), true),
        Err(_) => (Vec::new(), false),
    }
}

fn to_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes).lines().map(str::to_string).collect()
}

#[derive(Debug, Clone, Copy)]
struct SideOp {
    side: Side,
    base_start: usize,
    base_end: usize,
    new_start: usize,
    new_end: usize,
}

/// Turns Myers ops into base-coordinate spans, dropping `Equal` runs (they need no rendering).
/// An `Insert` becomes a zero-width span anchored at its base position — it still participates in
/// the overlap sweep below, so two insertions at the same point from both sides correctly conflict.
fn normalize_ops(ops: &[DiffOp], side: Side) -> Vec<SideOp> {
    ops.iter()
        .filter_map(|op| match *op {
            DiffOp::Equal { .. } => None,
            DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => Some(SideOp {
                side,
                base_start: old_index,
                base_end: old_index + old_len,
                new_start: new_index,
                new_end: new_index,
            }),
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => Some(SideOp {
                side,
                base_start: old_index,
                base_end: old_index,
                new_start: new_index,
                new_end: new_index + new_len,
            }),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => Some(SideOp {
                side,
                base_start: old_index,
                base_end: old_index + old_len,
                new_start: new_index,
                new_end: new_index + new_len,
            }),
        })
        .collect()
}

/// Renders one side's content for the base range `[start, end)`, given that side's own ops. Any
/// sub-range the side didn't touch (no op in range) falls back to the base's own content —
/// correct both for genuinely-unchanged gaps *and* for the auto-resolved case where this side has
/// no ops in the range at all (the whole range then just echoes `base`).
fn render_side(start: usize, end: usize, base: &[String], side_lines: &[String], ops: &[SideOp]) -> Vec<String> {
    let mut relevant: Vec<&SideOp> = ops
        .iter()
        .filter(|o| o.base_start >= start && o.base_end <= end)
        .collect();
    relevant.sort_by_key(|o| o.base_start);

    let mut out = Vec::new();
    let mut cursor = start;
    for op in relevant {
        if op.base_start > cursor {
            out.extend(base[cursor..op.base_start].iter().cloned());
        }
        out.extend(side_lines[op.new_start..op.new_end].iter().cloned());
        cursor = op.base_end;
    }
    if cursor < end {
        out.extend(base[cursor..end].iter().cloned());
    }
    out
}

/// Peels identical lines off the start and end of `ours`/`theirs` — DESIGN_SPEC.md §9.3's "same in
/// both" edge dedupe. Returns `(prefix, ours_mid, theirs_mid, suffix)`.
fn dedupe_edges(ours: &[String], theirs: &[String]) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let max_prefix = ours.len().min(theirs.len());
    let mut prefix_len = 0;
    while prefix_len < max_prefix && ours[prefix_len] == theirs[prefix_len] {
        prefix_len += 1;
    }

    let ours_rest = &ours[prefix_len..];
    let theirs_rest = &theirs[prefix_len..];
    let max_suffix = ours_rest.len().min(theirs_rest.len());
    let mut suffix_len = 0;
    while suffix_len < max_suffix
        && ours_rest[ours_rest.len() - 1 - suffix_len] == theirs_rest[theirs_rest.len() - 1 - suffix_len]
    {
        suffix_len += 1;
    }

    let prefix = ours[..prefix_len].to_vec();
    let ours_mid = ours_rest[..ours_rest.len() - suffix_len].to_vec();
    let theirs_mid = theirs_rest[..theirs_rest.len() - suffix_len].to_vec();
    let suffix = ours_rest[ours_rest.len() - suffix_len..].to_vec();
    (prefix, ours_mid, theirs_mid, suffix)
}

/// The simplified 3-way merge — ARCHITECTURE.md §7.5. Diffs `base` against each side, then sweeps
/// the two sides' non-equal ops together: touching/overlapping spans (from either side, including
/// zero-width insertion points) merge into one group; a group touched by both sides is a
/// [`FileRegion::Conflict`], a group touched by only one side is [`FileRegion::AutoResolved`], and
/// the untouched gaps between groups are [`FileRegion::Context`].
fn build_regions(base: &[String], ours: &[String], theirs: &[String]) -> Vec<FileRegion> {
    let base_refs: Vec<&str> = base.iter().map(String::as_str).collect();
    let ours_refs: Vec<&str> = ours.iter().map(String::as_str).collect();
    let theirs_refs: Vec<&str> = theirs.iter().map(String::as_str).collect();

    let ours_ops = normalize_ops(&capture_diff_slices(Algorithm::Myers, &base_refs, &ours_refs), Side::Ours);
    let theirs_ops = normalize_ops(
        &capture_diff_slices(Algorithm::Myers, &base_refs, &theirs_refs),
        Side::Theirs,
    );

    let mut all: Vec<SideOp> = Vec::with_capacity(ours_ops.len() + theirs_ops.len());
    all.extend(ours_ops.iter().copied());
    all.extend(theirs_ops.iter().copied());
    all.sort_by_key(|op| (op.base_start, op.base_end));

    struct Group {
        start: usize,
        end: usize,
        has_ours: bool,
        has_theirs: bool,
    }
    let mut groups: Vec<Group> = Vec::new();
    for op in &all {
        if let Some(last) = groups.last_mut() {
            if op.base_start <= last.end {
                last.end = last.end.max(op.base_end);
                match op.side {
                    Side::Ours => last.has_ours = true,
                    Side::Theirs => last.has_theirs = true,
                }
                continue;
            }
        }
        groups.push(Group {
            start: op.base_start,
            end: op.base_end,
            has_ours: op.side == Side::Ours,
            has_theirs: op.side == Side::Theirs,
        });
    }

    let mut regions = Vec::new();
    let mut cursor = 0usize;
    for g in &groups {
        if g.start > cursor {
            regions.push(FileRegion::Context {
                lines: base[cursor..g.start].to_vec(),
            });
        }

        let ours_lines = render_side(g.start, g.end, base, ours, &ours_ops);
        let theirs_lines = render_side(g.start, g.end, base, theirs, &theirs_ops);

        if g.has_ours && g.has_theirs {
            let (same_both_prefix, ours_mid, theirs_mid, same_both_suffix) =
                dedupe_edges(&ours_lines, &theirs_lines);
            regions.push(FileRegion::Conflict {
                base_start: g.start,
                base_end: g.end,
                same_both_prefix,
                ours_lines: ours_mid,
                theirs_lines: theirs_mid,
                same_both_suffix,
            });
        } else if g.has_ours {
            regions.push(FileRegion::AutoResolved {
                side: Side::Ours,
                lines: ours_lines,
            });
        } else {
            regions.push(FileRegion::AutoResolved {
                side: Side::Theirs,
                lines: theirs_lines,
            });
        }

        cursor = g.end;
    }
    if cursor < base.len() {
        regions.push(FileRegion::Context {
            lines: base[cursor..].to_vec(),
        });
    }
    regions
}

/// Reads the three stages of a conflicted file and computes its Keep Panel regions.
pub async fn conflict_regions(repo: &Path, path: &str) -> FileConflictRegions {
    let (base_lines, _base_present) = read_stage(repo, 1, path).await;
    let (ours_lines, ours_present) = read_stage(repo, 2, path).await;
    let (theirs_lines, theirs_present) = read_stage(repo, 3, path).await;
    FileConflictRegions {
        ours_deleted: !ours_present,
        theirs_deleted: !theirs_present,
        regions: build_regions(&base_lines, &ours_lines, &theirs_lines),
    }
}

#[tauri::command]
pub async fn get_conflict_regions(
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<FileConflictRegions, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(conflict_regions(&handle.path, &path).await)
}

// --- confirm / reopen — DESIGN_SPEC.md §9.2 -----------------------------------------------------

/// Writes the Keep Panel's assembled resolved text to the worktree and stages it — the flagship
/// panel's Confirm button. Plain `std::fs::write` + `git add`; no patch construction needed since
/// the frontend already assembled the exact final text (context + auto-resolved + kept, in order).
#[tauri::command]
pub async fn confirm_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
    resolved_text: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::WorkingTree, WatchedKind::Index]);
    std::fs::write(handle.path.join(&path), resolved_text)?;
    git(&handle.path, &["add", "--", &path], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::WorkingTree, ChangeKind::Index]);
    Ok(())
}

/// Regenerates the conflict for a previously-confirmed file — "Reset file" (DESIGN_SPEC.md §9.2),
/// available until the whole operation is finalized (`continue`/`abort`).
#[tauri::command]
pub async fn reopen_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::WorkingTree, WatchedKind::Index]);
    git(&handle.path, &["checkout", "-m", "--", &path], GitOpts::default()).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::WorkingTree, ChangeKind::Index]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn extracts_quoted_branch_name_from_merge_msg() {
        assert_eq!(
            extract_quoted_name("Merge branch 'feature/x' into main"),
            Some("feature/x".to_string())
        );
        assert_eq!(extract_quoted_name("Merge remote-tracking branch 'origin/main'"), Some("origin/main".to_string()));
        assert_eq!(extract_quoted_name("no quotes here"), None);
    }

    #[test]
    fn context_only_when_neither_side_touches_base() {
        let base = lines(&["a", "b", "c"]);
        let regions = build_regions(&base, &base, &base);
        assert_eq!(regions, vec![FileRegion::Context { lines: base }]);
    }

    #[test]
    fn auto_resolves_when_only_one_side_changes() {
        let base = lines(&["a", "b", "c"]);
        let ours = lines(&["a", "B", "c"]);
        let theirs = base.clone();
        let regions = build_regions(&base, &ours, &theirs);
        assert_eq!(
            regions,
            vec![
                FileRegion::Context { lines: lines(&["a"]) },
                FileRegion::AutoResolved {
                    side: Side::Ours,
                    lines: lines(&["B"])
                },
                FileRegion::Context { lines: lines(&["c"]) },
            ]
        );
    }

    #[test]
    fn both_modified_produces_a_conflict_region() {
        let base = lines(&["a", "b", "c"]);
        let ours = lines(&["a", "OURS", "c"]);
        let theirs = lines(&["a", "THEIRS", "c"]);
        let regions = build_regions(&base, &ours, &theirs);
        assert_eq!(
            regions,
            vec![
                FileRegion::Context { lines: lines(&["a"]) },
                FileRegion::Conflict {
                    base_start: 1,
                    base_end: 2,
                    same_both_prefix: vec![],
                    ours_lines: lines(&["OURS"]),
                    theirs_lines: lines(&["THEIRS"]),
                    same_both_suffix: vec![],
                },
                FileRegion::Context { lines: lines(&["c"]) },
            ]
        );
    }

    #[test]
    fn both_added_with_no_base_produces_one_conflict_region() {
        let base: Vec<String> = vec![];
        let ours = lines(&["ours one", "ours two"]);
        let theirs = lines(&["theirs one"]);
        let regions = build_regions(&base, &ours, &theirs);
        assert_eq!(
            regions,
            vec![FileRegion::Conflict {
                base_start: 0,
                base_end: 0,
                same_both_prefix: vec![],
                ours_lines: ours,
                theirs_lines: theirs,
                same_both_suffix: vec![],
            }]
        );
    }

    #[test]
    fn modify_delete_conflict_renders_deleted_side_empty() {
        // ours deletes the whole file (empty content); theirs modifies one line.
        let base = lines(&["a", "b", "c"]);
        let ours: Vec<String> = vec![];
        let theirs = lines(&["a", "B", "c"]);
        let regions = build_regions(&base, &ours, &theirs);
        assert_eq!(
            regions,
            vec![FileRegion::Conflict {
                base_start: 0,
                base_end: 3,
                same_both_prefix: vec![],
                ours_lines: vec![],
                theirs_lines: theirs,
                same_both_suffix: vec![],
            }]
        );
    }

    #[test]
    fn dedupe_edges_peels_identical_prefix_and_suffix() {
        let ours = lines(&["shared", "ours middle", "tail"]);
        let theirs = lines(&["shared", "theirs middle", "tail"]);
        let (prefix, ours_mid, theirs_mid, suffix) = dedupe_edges(&ours, &theirs);
        assert_eq!(prefix, lines(&["shared"]));
        assert_eq!(ours_mid, lines(&["ours middle"]));
        assert_eq!(theirs_mid, lines(&["theirs middle"]));
        assert_eq!(suffix, lines(&["tail"]));
    }

    #[test]
    fn identical_edges_are_deduped_out_of_the_conflict_region() {
        // Both branches independently add the same new line at the top ("both added the same
        // import") while also each editing a different one of the two original lines — a
        // realistic way for a conflict region's rendered ours/theirs content to share a literal
        // edge line, since both sides' diffs anchor their insert at the very same base position.
        let base = lines(&["line1", "line2"]);
        let ours = lines(&["shared new line", "line1 edited", "line2"]);
        let theirs = lines(&["shared new line", "line1", "line2 edited"]);
        let regions = build_regions(&base, &ours, &theirs);
        assert_eq!(
            regions,
            vec![FileRegion::Conflict {
                base_start: 0,
                base_end: 2,
                same_both_prefix: lines(&["shared new line"]),
                ours_lines: lines(&["line1 edited", "line2"]),
                theirs_lines: lines(&["line1", "line2 edited"]),
                same_both_suffix: vec![],
            }]
        );
    }

    #[test]
    fn marker_like_content_is_treated_as_plain_text() {
        // A file whose real content contains marker-like lines must not be specially parsed —
        // it should flow through the diff exactly like any other line.
        let base = lines(&["<<<<<<< literal", "=======", ">>>>>>> literal"]);
        let ours = lines(&["<<<<<<< literal", "CHANGED", ">>>>>>> literal"]);
        let theirs = base.clone();
        let regions = build_regions(&base, &ours, &theirs);
        assert_eq!(
            regions,
            vec![
                FileRegion::Context {
                    lines: lines(&["<<<<<<< literal"])
                },
                FileRegion::AutoResolved {
                    side: Side::Ours,
                    lines: lines(&["CHANGED"])
                },
                FileRegion::Context {
                    lines: lines(&[">>>>>>> literal"])
                },
            ]
        );
    }
}
