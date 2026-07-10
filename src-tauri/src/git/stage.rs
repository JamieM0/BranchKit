//! Whole-file and hunk/line staging mutations — DESIGN_SPEC.md §6.1/§6.2, ARCHITECTURE.md §6.3.
//!
//! Hunk/line staging works by constructing a patch and applying it to the index rather than
//! calling `git add`/`git reset` directly — ARCHITECTURE.md §6.3's "exact technique":
//! 1. Diff the file (worktree-vs-index for staging, `--cached` for unstaging).
//! 2. Rebuild the selected hunk, keeping selected `+`/`-` lines as-is, converting unselected `-`
//!    lines to context (they must remain in the result), and dropping unselected `+` lines.
//! 3. Feed the patch to `git apply --cached --recount --whitespace=nowarn -` (add `--reverse` to
//!    unstage), letting `--recount` fix the hunk-header counts we didn't hand-compute.
//!
//! Renames, mode-only changes and binary files can't be partially staged (§6.3(5)) — those, and
//! untracked files (which don't have an index blob to diff against yet), fall back to whole-file
//! `git add`/`git reset`.

use std::collections::HashSet;
use std::path::Path;

use tauri::{AppHandle, State};

use crate::error::AppError;
use crate::events::{ChangeKind, WatchedKind};
use crate::state::AppState;

use super::diff::{parse_diff_output, DiffLineKind, FileDiff};
use super::exec::{git, git_with_stdin, GitError, GitOpts};
use super::ops::{emit_changes, require_repo};
use super::status::{status, StatusEntryKind};

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

// --- hunk & line staging — ARCHITECTURE.md §6.3 ---------------------------------------------

/// Whether a file's diff can be partially staged at all — the whole-file-only edge cases from
/// §6.3(5): renames, mode-only changes and binary files. `Normal` carries the parsed structure so
/// callers don't reparse.
enum PatchScope {
    Normal,
    WholeFileOnly,
}

/// The raw preamble lines (`diff --git`, `index`, `---`, `+++`, or the mode/rename lines for the
/// whole-file-only cases) — everything before the first hunk header. Needed verbatim because
/// [`parse_diff_output`] only extracts paths/binary-ness from it, not the exact text `git apply`
/// needs back.
fn preamble(raw_text: &str) -> &str {
    match raw_text.find("\n@@ ") {
        Some(pos) => &raw_text[..=pos],
        None => raw_text,
    }
}

fn classify(raw_text: &str, file_diff: &FileDiff) -> PatchScope {
    if file_diff.is_binary {
        return PatchScope::WholeFileOnly;
    }
    if raw_text.lines().any(|l| l.starts_with("rename from ")) {
        return PatchScope::WholeFileOnly;
    }
    if file_diff.hunks.is_empty()
        && raw_text
            .lines()
            .any(|l| l.starts_with("old mode ") || l.starts_with("new mode "))
    {
        return PatchScope::WholeFileOnly;
    }
    PatchScope::Normal
}

/// Rebuilds a single hunk into a standalone patch containing only the lines selected by
/// `selected` (indices into `hunk.lines`, aligned 1:1 with the `DiffLine`s the frontend already
/// has from the same diff call). Selected `-`/`+` lines are kept as-is; an unselected `-`/`+`
/// *pair* within a run collapses to a single context line (§6.3 point 2: "they must remain in the
/// result") using whichever side's text is the file's *current* content — the index's for a
/// forward (stage) patch, or the index's-current-staged-value for a `reverse` (unstage) one,
/// since that's the side `git apply`/`git apply --reverse` matches against. A run's `-` lines are
/// paired positionally with its `+` lines (git always emits a run's dels before its adds, so
/// simple naive line-order concatenation of "kept" lines silently reorders a replace — pairing
/// keeps each kept addition immediately after its own context/deletion instead of drifting to the
/// end of the run). Returns `None` when nothing in the hunk was selected (a no-op for the caller).
pub(crate) fn build_line_patch(
    raw_text: &str,
    file_diff: &FileDiff,
    hunk_index: usize,
    selected: &HashSet<usize>,
    reverse: bool,
) -> Option<String> {
    let hunk = file_diff.hunks.get(hunk_index)?;
    let mut body = String::new();
    let mut any_change = false;

    fn emit(body: &mut String, marker: char, text: &str, no_newline: bool) {
        body.push(marker);
        body.push_str(text);
        body.push('\n');
        if no_newline {
            body.push_str("\\ No newline at end of file\n");
        }
    }

    let lines = &hunk.lines;
    let n = lines.len();
    let mut i = 0;
    while i < n {
        let line = &lines[i];
        if line.kind == DiffLineKind::Context {
            emit(&mut body, ' ', &line.text, line.no_newline_at_eof);
            i += 1;
            continue;
        }

        // A run of consecutive non-context lines — git always groups a replace as dels then
        // adds, so pair them up positionally (k-th del with k-th add) to keep kept lines in
        // their original relative position instead of drifting to the run's end.
        let start = i;
        while i < n && lines[i].kind != DiffLineKind::Context {
            i += 1;
        }
        let run = &lines[start..i];
        let dels: Vec<(usize, &super::diff::DiffLine)> = run
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind == DiffLineKind::Del)
            .map(|(off, l)| (start + off, l))
            .collect();
        let adds: Vec<(usize, &super::diff::DiffLine)> = run
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind == DiffLineKind::Add)
            .map(|(off, l)| (start + off, l))
            .collect();

        for k in 0..dels.len().max(adds.len()) {
            let del = dels.get(k);
            let add = adds.get(k);
            let del_sel = del.is_some_and(|(idx, _)| selected.contains(idx));
            let add_sel = add.is_some_and(|(idx, _)| selected.contains(idx));

            if !del_sel && !add_sel {
                // Neither side of this slot is being touched. If there's a paired del, it still
                // exists in the file/index right now, so collapse to a context line using its
                // text (or the add's, when reversing against the staged side). A pure addition
                // with no del counterpart has no such existing line to fall back to — it must be
                // dropped entirely, or git can't match the resulting context line against the
                // index and rejects the whole hunk.
                match (del, add) {
                    (Some(_), Some((_, a))) if reverse => {
                        emit(&mut body, ' ', &a.text, a.no_newline_at_eof)
                    }
                    (Some((_, d)), _) => emit(&mut body, ' ', &d.text, d.no_newline_at_eof),
                    (None, Some(_)) => {}
                    (None, None) => unreachable!("run always has at least one del or add"),
                }
                continue;
            }

            if let Some((_, d)) = del {
                if del_sel {
                    emit(&mut body, '-', &d.text, d.no_newline_at_eof);
                    any_change = true;
                } else {
                    emit(&mut body, ' ', &d.text, d.no_newline_at_eof);
                }
            }
            if let Some((_, a)) = add {
                if add_sel {
                    emit(&mut body, '+', &a.text, a.no_newline_at_eof);
                    any_change = true;
                }
                // Unselected `+` half of a partially-selected slot is dropped, same as a fully
                // unselected one.
            }
        }
    }

    if !any_change {
        return None;
    }

    let mut patch = String::new();
    patch.push_str(preamble(raw_text));
    patch.push_str(&hunk.header);
    patch.push('\n');
    patch.push_str(&body);
    Some(patch)
}

/// All indices in `hunk` that are `+`/`-` lines — "select the whole hunk" (Stage hunk / Discard
/// hunk buttons, DESIGN_SPEC.md §6.2).
pub(crate) fn all_change_indices(hunk: &super::diff::Hunk) -> HashSet<usize> {
    hunk.lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind != DiffLineKind::Context)
        .map(|(i, _)| i)
        .collect()
}

async fn is_untracked(repo: &Path, path: &str) -> Result<bool, GitError> {
    let report = status(repo).await?;
    Ok(report
        .entries
        .iter()
        .any(|e| e.path == path && e.kind == StatusEntryKind::Untracked))
}

/// Whether `path` is currently a rename/copy, per `git status` — checked via status rather than
/// the pathspec-filtered `git diff` used for patch construction, because restricting a diff to a
/// single path hides the other side of the pair and defeats git's own rename detection (`git diff
/// -M -- new.txt` alone shows a plain new file, not a rename — status compares the whole repo, so
/// it still pairs them up).
async fn is_rename(repo: &Path, path: &str) -> Result<bool, GitError> {
    let report = status(repo).await?;
    Ok(report
        .entries
        .iter()
        .any(|e| e.path == path && e.kind == StatusEntryKind::RenamedOrCopied))
}

pub(crate) async fn apply_patch(
    repo: &Path,
    patch: &str,
    reverse: bool,
    cached: bool,
) -> Result<(), GitError> {
    let mut args = vec!["apply"];
    if cached {
        args.push("--cached");
    }
    if reverse {
        args.push("--reverse");
    }
    args.push("--recount");
    args.push("--whitespace=nowarn");
    args.push("-");
    git_with_stdin(repo, &args, GitOpts::default(), patch.as_bytes()).await?;
    Ok(())
}

/// Stage a subset of a file's unstaged (worktree-vs-index) hunk. `line_indices` selects which
/// `+`/`-` lines of `hunk_index` to stage — pass every changed index in the hunk for "Stage hunk".
/// Falls back to whole-file `git add` for untracked files and the whole-file-only edge cases.
pub async fn stage_lines_impl(
    repo: &Path,
    path: &str,
    hunk_index: usize,
    line_indices: &[usize],
) -> Result<(), GitError> {
    if is_untracked(repo, path).await? {
        git(repo, &["add", "--", path], GitOpts::default()).await?;
        return Ok(());
    }
    let output = git(
        repo,
        &["diff", "--no-color", "-U3", "--", path],
        GitOpts::default(),
    )
    .await?;
    let raw_text = String::from_utf8_lossy(&output.stdout).into_owned();
    let file_diff = parse_diff_output(&output.stdout);

    if matches!(classify(&raw_text, &file_diff), PatchScope::WholeFileOnly) {
        git(repo, &["add", "--", path], GitOpts::default()).await?;
        return Ok(());
    }

    let selected: HashSet<usize> = line_indices.iter().copied().collect();
    if let Some(patch) = build_line_patch(&raw_text, &file_diff, hunk_index, &selected, false) {
        apply_patch(repo, &patch, false, true).await?;
    }
    Ok(())
}

/// Unstage a subset of a file's staged (index-vs-HEAD) hunk — same technique, but the diff base
/// is `--cached` (§6.3 point 5) and the patch is applied with `--reverse --cached`. Falls back to
/// whole-file `git reset` for the whole-file-only edge cases (an unstaged untracked file doesn't
/// exist, so there's no untracked fallback on this side). `-M` enables rename detection on this
/// diff — off by default for `git diff`, but needed so a staged rename classifies as
/// whole-file-only instead of looking like an unrelated delete+add pair.
pub async fn unstage_lines_impl(
    repo: &Path,
    path: &str,
    hunk_index: usize,
    line_indices: &[usize],
) -> Result<(), GitError> {
    if is_rename(repo, path).await? {
        git(repo, &["reset", "--", path], GitOpts::default()).await?;
        return Ok(());
    }
    let output = git(
        repo,
        &["diff", "--no-color", "-M", "-U3", "--cached", "--", path],
        GitOpts::default(),
    )
    .await?;
    let raw_text = String::from_utf8_lossy(&output.stdout).into_owned();
    let file_diff = parse_diff_output(&output.stdout);

    if matches!(classify(&raw_text, &file_diff), PatchScope::WholeFileOnly) {
        git(repo, &["reset", "--", path], GitOpts::default()).await?;
        return Ok(());
    }

    let selected: HashSet<usize> = line_indices.iter().copied().collect();
    if let Some(patch) = build_line_patch(&raw_text, &file_diff, hunk_index, &selected, true) {
        apply_patch(repo, &patch, true, true).await?;
    }
    Ok(())
}

/// Stage a subset of lines within one hunk of a file's unstaged diff — the diff gutter's
/// click/click+drag staging (DESIGN_SPEC.md §6.2/§15.11). `line_indices` index into the hunk's
/// `lines` array exactly as returned by `get_diff_worktree` for the same path (fetched *without*
/// the whitespace-ignore toggle — the frontend disables gutter staging while that's on, since it
/// changes hunk structure and would desync the indices).
#[tauri::command]
pub async fn stage_lines(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
    hunk_index: usize,
    line_indices: Vec<usize>,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Index]);
    stage_lines_impl(&handle.path, &path, hunk_index, &line_indices).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Index]);
    Ok(())
}

/// Unstage a subset of lines within one hunk of a file's staged diff — the same gutter mechanics
/// reversed in the Staged view (DESIGN_SPEC.md §6.2).
#[tauri::command]
pub async fn unstage_lines(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
    hunk_index: usize,
    line_indices: Vec<usize>,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[WatchedKind::Index]);
    unstage_lines_impl(&handle.path, &path, hunk_index, &line_indices).await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Index]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn init_repo(dir: &Path) {
        git(
            dir,
            &["init", "--initial-branch=main", "-q"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        git(
            dir,
            &["config", "user.name", "T"],
            GitOpts::default(),
        )
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

    async fn commit_all(dir: &Path, msg: &str) {
        git(dir, &["add", "-A"], GitOpts::default()).await.unwrap();
        git(dir, &["commit", "-q", "-m", msg], GitOpts::default())
            .await
            .unwrap();
    }

    async fn staged_diff_text(dir: &Path, path: &str) -> String {
        let out = git(
            dir,
            &["diff", "--no-color", "--cached", "--", path],
            GitOpts::default(),
        )
        .await
        .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    async fn worktree_diff_text(dir: &Path, path: &str) -> String {
        let out = git(
            dir,
            &["diff", "--no-color", "--", path],
            GitOpts::default(),
        )
        .await
        .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    async fn worktree_hunks(dir: &Path, path: &str) -> FileDiff {
        let out = git(
            dir,
            &["diff", "--no-color", "-U3", "--", path],
            GitOpts::default(),
        )
        .await
        .unwrap();
        parse_diff_output(&out.stdout)
    }

    // --- normal partial line staging (the happy path underlying all the edge cases) ---

    #[tokio::test]
    async fn stages_a_subset_of_lines_in_one_hunk() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        std::fs::write(dir.path().join("f.txt"), "a\nb\nc\nd\ne\n").unwrap();
        commit_all(dir.path(), "init").await;

        // Change all 5 lines so the hunk has 5 del + 5 add lines; stage only lines 0 and 1 (the
        // "two of five" from the verification recipe).
        std::fs::write(dir.path().join("f.txt"), "a1\nb1\nc1\nd1\ne1\n").unwrap();
        let file_diff = worktree_hunks(dir.path(), "f.txt").await;
        assert_eq!(file_diff.hunks.len(), 1);
        let hunk = &file_diff.hunks[0];
        let del_indices: Vec<usize> = hunk
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind == DiffLineKind::Del)
            .map(|(i, _)| i)
            .collect();
        let add_indices: Vec<usize> = hunk
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind == DiffLineKind::Add)
            .map(|(i, _)| i)
            .collect();
        // Stage the del+add pair for line "a" only.
        let selection = vec![del_indices[0], add_indices[0]];

        stage_lines_impl(dir.path(), "f.txt", 0, &selection)
            .await
            .unwrap();

        let staged = staged_diff_text(dir.path(), "f.txt").await;
        assert!(staged.contains("-a\n") || staged.contains("-a"));
        assert!(staged.contains("+a1"));
        assert!(!staged.contains("-b\n"));
        assert!(!staged.contains("+b1"));

        // The actual index content must have "a1" staged *in place* of "a" — not reordered to
        // the end of the file, which is what a naive "emit all context, then all selected adds"
        // reconstruction would produce.
        let index_blob = git(
            dir.path(),
            &["show", ":f.txt"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&index_blob.stdout),
            "a1\nb\nc\nd\ne\n"
        );

        // Unstaged diff should still show the remaining 4 changed lines.
        let remaining = worktree_diff_text(dir.path(), "f.txt").await;
        assert!(remaining.contains("+b1"));
        assert!(remaining.contains("+e1"));
        assert!(!remaining.contains("+a1"));
    }

    #[tokio::test]
    async fn stages_a_subset_of_a_pure_addition_run() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        std::fs::write(dir.path().join("f.txt"), "a\nb\n").unwrap();
        commit_all(dir.path(), "init").await;

        // Append two new lines with no deletions, so the hunk is a run of pure additions.
        std::fs::write(dir.path().join("f.txt"), "a\nb\nc\nd\n").unwrap();
        let file_diff = worktree_hunks(dir.path(), "f.txt").await;
        assert_eq!(file_diff.hunks.len(), 1);
        let hunk = &file_diff.hunks[0];
        let add_indices: Vec<usize> = hunk
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind == DiffLineKind::Add)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(add_indices.len(), 2);

        // Stage only the last added line ("d"), leaving "c" unstaged.
        let selection = vec![add_indices[1]];
        stage_lines_impl(dir.path(), "f.txt", 0, &selection)
            .await
            .unwrap();

        let index_blob = git(dir.path(), &["show", ":f.txt"], GitOpts::default())
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&index_blob.stdout),
            "a\nb\nd\n"
        );

        let remaining = worktree_diff_text(dir.path(), "f.txt").await;
        assert!(remaining.contains("+c"));
        assert!(!remaining.contains("+d"));
    }

    #[tokio::test]
    async fn unstage_reverses_a_previously_staged_line_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        std::fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        commit_all(dir.path(), "init").await;

        std::fs::write(dir.path().join("f.txt"), "a1\nb1\nc1\n").unwrap();
        let file_diff = worktree_hunks(dir.path(), "f.txt").await;
        let all: Vec<usize> = (0..file_diff.hunks[0].lines.len()).collect();
        stage_lines_impl(dir.path(), "f.txt", 0, &all).await.unwrap();

        // Fully staged now; worktree diff for this path should be empty.
        let remaining = worktree_diff_text(dir.path(), "f.txt").await;
        assert!(remaining.trim().is_empty());

        // Now unstage just the "b" line pair from the staged side.
        let staged_file_diff = {
            let out = git(
                dir.path(),
                &["diff", "--no-color", "-U3", "--cached", "--", "f.txt"],
                GitOpts::default(),
            )
            .await
            .unwrap();
            parse_diff_output(&out.stdout)
        };
        let hunk = &staged_file_diff.hunks[0];
        let b_indices: Vec<usize> = hunk
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.text == "b" || l.text == "b1")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(b_indices.len(), 2);

        unstage_lines_impl(dir.path(), "f.txt", 0, &b_indices)
            .await
            .unwrap();

        let staged = staged_diff_text(dir.path(), "f.txt").await;
        assert!(!staged.contains("b1"));
        assert!(staged.contains("+a1"));
        assert!(staged.contains("+c1"));

        let back_in_worktree = worktree_diff_text(dir.path(), "f.txt").await;
        assert!(back_in_worktree.contains("+b1"));
    }

    // --- edge case 1: no-trailing-newline marker preserved with its line ---

    #[tokio::test]
    async fn preserves_no_newline_marker_when_staging_the_line_it_belongs_to() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        std::fs::write(dir.path().join("f.txt"), "a\nb\nlast").unwrap();
        commit_all(dir.path(), "init").await;

        std::fs::write(dir.path().join("f.txt"), "a\nb\nlast changed").unwrap();
        let file_diff = worktree_hunks(dir.path(), "f.txt").await;
        let hunk = &file_diff.hunks[0];
        assert!(hunk.lines.iter().any(|l| l.no_newline_at_eof));

        let all = all_change_indices(hunk);
        stage_lines_impl(dir.path(), "f.txt", 0, &all.into_iter().collect::<Vec<_>>())
            .await
            .unwrap();

        let staged = staged_diff_text(dir.path(), "f.txt").await;
        assert!(staged.contains("\\ No newline at end of file"));
        assert!(staged.contains("+last changed"));

        let remaining = worktree_diff_text(dir.path(), "f.txt").await;
        assert!(remaining.trim().is_empty());
    }

    // --- edge case 2: untracked files stage whole (no partial) ---

    #[tokio::test]
    async fn untracked_file_stages_whole_file_not_partial() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        std::fs::write(dir.path().join("README.md"), "seed").unwrap();
        commit_all(dir.path(), "init").await;

        std::fs::write(dir.path().join("new.txt"), "one\ntwo\nthree\n").unwrap();
        assert!(is_untracked(dir.path(), "new.txt").await.unwrap());

        // hunk_index/line_indices are irrelevant for an untracked file — whole file is staged.
        stage_lines_impl(dir.path(), "new.txt", 0, &[0]).await.unwrap();

        let staged = staged_diff_text(dir.path(), "new.txt").await;
        assert!(staged.contains("+one"));
        assert!(staged.contains("+two"));
        assert!(staged.contains("+three"));
    }

    // --- edge case 3: renames are whole-file only ---

    #[tokio::test]
    async fn rename_falls_back_to_whole_file_staging() {
        // A rename is only representable as such once staged (`git diff` on the *unstaged* side
        // never detects renames against an untracked file — an unstaged rename's new path is
        // simply untracked, which `is_untracked` already routes to whole-file `git add`). The
        // meaningful whole-file-only fallback is therefore on the *unstage* side: reverting a
        // partial line selection out of a staged rename must not be attempted as a line patch.
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        std::fs::write(
            dir.path().join("old.txt"),
            "one\ntwo\nthree\nfour\nfive\nsix\n",
        )
        .unwrap();
        commit_all(dir.path(), "init").await;

        std::fs::rename(dir.path().join("old.txt"), dir.path().join("new.txt")).unwrap();
        std::fs::write(
            dir.path().join("new.txt"),
            "one\ntwo\nthree CHANGED\nfour\nfive\nsix\n",
        )
        .unwrap();
        // Stage it so git's rename detection (on by default for `status`, off for plain `diff`)
        // has a HEAD-vs-index comparison to actually detect the rename against.
        git(dir.path(), &["add", "-A"], GitOpts::default())
            .await
            .unwrap();

        let staged_before = git(
            dir.path(),
            &["diff", "--no-color", "--cached", "--name-status", "-M"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        assert!(
            String::from_utf8_lossy(&staged_before.stdout).starts_with('R'),
            "test setup should produce a staged rename"
        );

        unstage_lines_impl(dir.path(), "new.txt", 0, &[0]).await.unwrap();

        // Whole-file fallback (`git reset -- new.txt`) fully unstages it — nothing rename-shaped
        // should remain in the index for that path.
        let staged_after = git(
            dir.path(),
            &["diff", "--no-color", "--cached", "--name-status", "-M"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        let text = String::from_utf8_lossy(&staged_after.stdout);
        assert!(!text.contains("new.txt"), "expected new.txt fully unstaged, got: {text}");
    }

    // --- edge case 4: mode-only changes are whole-file only ---

    #[tokio::test]
    #[cfg(unix)]
    async fn mode_only_change_falls_back_to_whole_file_staging() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        let path = dir.path().join("run.sh");
        std::fs::write(&path, "echo hi\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        commit_all(dir.path(), "init").await;

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let raw = git(
            dir.path(),
            &["diff", "--no-color", "-U3", "--", "run.sh"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        let raw_text = String::from_utf8_lossy(&raw.stdout);
        assert!(raw_text.contains("old mode"));

        stage_lines_impl(dir.path(), "run.sh", 0, &[0]).await.unwrap();

        let staged = git(
            dir.path(),
            &["diff", "--no-color", "--cached", "--", "run.sh"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        let staged_text = String::from_utf8_lossy(&staged.stdout);
        assert!(staged_text.contains("new mode 100755"));
    }

    // --- edge case 5: binary files are whole-file only ---

    #[tokio::test]
    async fn binary_file_falls_back_to_whole_file_staging() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        std::fs::write(dir.path().join(".gitattributes"), "*.bin binary\n").unwrap();
        std::fs::write(dir.path().join("data.bin"), [0u8, 1, 2, 3, 0, 255]).unwrap();
        commit_all(dir.path(), "init").await;

        std::fs::write(dir.path().join("data.bin"), [9u8, 8, 7, 0, 6, 5]).unwrap();

        stage_lines_impl(dir.path(), "data.bin", 0, &[0]).await.unwrap();

        let staged = git(
            dir.path(),
            &["diff", "--no-color", "--cached", "--", "data.bin"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        let text = String::from_utf8_lossy(&staged.stdout);
        assert!(text.contains("Binary files"));
    }

    // --- edge case 6: staged-side line unstaging (diff base is --cached) ---

    #[tokio::test]
    async fn unstages_a_subset_of_lines_from_the_staged_side() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        std::fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        commit_all(dir.path(), "init").await;

        std::fs::write(dir.path().join("f.txt"), "a1\nb1\nc1\n").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default())
            .await
            .unwrap();

        let staged_file_diff = {
            let out = git(
                dir.path(),
                &["diff", "--no-color", "-U3", "--cached", "--", "f.txt"],
                GitOpts::default(),
            )
            .await
            .unwrap();
            parse_diff_output(&out.stdout)
        };
        let hunk = &staged_file_diff.hunks[0];
        let c_indices: Vec<usize> = hunk
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.text == "c" || l.text == "c1")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(c_indices.len(), 2);

        unstage_lines_impl(dir.path(), "f.txt", 0, &c_indices)
            .await
            .unwrap();

        let staged = staged_diff_text(dir.path(), "f.txt").await;
        assert!(staged.contains("+a1"));
        assert!(staged.contains("+b1"));
        assert!(!staged.contains("c1"));

        let unstaged = worktree_diff_text(dir.path(), "f.txt").await;
        assert!(unstaged.contains("+c1"));
        // a1/b1 remain staged, so they're unchanged context in the *unstaged* diff (may still
        // appear as plain context lines) — only check they aren't shown as newly-added.
        assert!(!unstaged.contains("+a1"));
        assert!(!unstaged.contains("+b1"));
    }
}
