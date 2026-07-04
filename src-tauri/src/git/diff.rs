//! Unified diff parsing — ARCHITECTURE.md §6.2. Diffs are always requested with
//! `--no-color -U3`; this module turns that text into structured hunks/lines for the frontend.

use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

use super::exec::{git, GitError, GitOpts};
use super::ops::require_repo;

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tiff", "tif", "svg",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineKind {
    Context,
    Add,
    Del,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: String,
    pub no_newline_at_eof: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub is_binary: bool,
    pub is_image: bool,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub hunks: Vec<Hunk>,
}

/// Worktree vs index: `git diff -- <path>`.
pub async fn diff_worktree(repo: &Path, path: &str, ignore_whitespace: bool) -> Result<FileDiff, GitError> {
    run_diff(repo, &["diff", "--no-color", "-U3", "--", path], ignore_whitespace).await
}

/// Index vs HEAD: `git diff --cached -- <path>`.
pub async fn diff_staged(repo: &Path, path: &str, ignore_whitespace: bool) -> Result<FileDiff, GitError> {
    run_diff(
        repo,
        &["diff", "--no-color", "-U3", "--cached", "--", path],
        ignore_whitespace,
    )
    .await
}

/// A single commit vs its parent: `git show <sha> -- <path>`.
pub async fn diff_commit(
    repo: &Path,
    sha: &str,
    path: &str,
    ignore_whitespace: bool,
) -> Result<FileDiff, GitError> {
    run_diff(repo, &["show", "--no-color", "-U3", sha, "--", path], ignore_whitespace).await
}

/// Two arbitrary commits: `git diff <a> <b> -- <path>`.
pub async fn diff_two_commits(
    repo: &Path,
    a: &str,
    b: &str,
    path: &str,
    ignore_whitespace: bool,
) -> Result<FileDiff, GitError> {
    run_diff(repo, &["diff", "--no-color", "-U3", a, b, "--", path], ignore_whitespace).await
}

/// Appends `-w` (ignore all whitespace) when requested — the diff viewer's whitespace toggle
/// (DESIGN_SPEC.md §6.2).
async fn run_diff(repo: &Path, args: &[&str], ignore_whitespace: bool) -> Result<FileDiff, GitError> {
    let mut full_args: Vec<&str> = args.to_vec();
    if ignore_whitespace {
        full_args.push("-w");
    }
    let output = git(repo, &full_args, GitOpts::default()).await?;
    Ok(parse_diff_output(&output.stdout))
}

#[tauri::command]
pub async fn get_diff_worktree(
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
    ignore_whitespace: bool,
) -> Result<FileDiff, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(diff_worktree(&handle.path, &path, ignore_whitespace).await?)
}

#[tauri::command]
pub async fn get_diff_staged(
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
    ignore_whitespace: bool,
) -> Result<FileDiff, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(diff_staged(&handle.path, &path, ignore_whitespace).await?)
}

#[tauri::command]
pub async fn get_diff_commit(
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
    path: String,
    ignore_whitespace: bool,
) -> Result<FileDiff, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(diff_commit(&handle.path, &sha, &path, ignore_whitespace).await?)
}

#[tauri::command]
pub async fn get_diff_two_commits(
    state: State<'_, AppState>,
    repo_id: String,
    a: String,
    b: String,
    path: String,
    ignore_whitespace: bool,
) -> Result<FileDiff, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(diff_two_commits(&handle.path, &a, &b, &path, ignore_whitespace).await?)
}

// --- changed-file lists — commit-detail mode's file list and compare mode's file list
// (DESIGN_SPEC.md §6.1 "changed-file list", §15.5) ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangedFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    /// Set for renames/copies.
    pub orig_path: Option<String>,
    pub status: ChangedFileStatus,
}

/// Parses `git diff --name-status -M -z` / `git diff-tree --name-status -M -r -z` output. Each
/// record is `<code>\0<path>\0` (or `<code>\0<origPath>\0<newPath>\0` for `R`/`C`, where `code`
/// carries a trailing similarity score digit string like `R100` that we ignore).
fn parse_name_status(stdout: &[u8]) -> Vec<ChangedFile> {
    let text = String::from_utf8_lossy(stdout);
    let tokens: Vec<&str> = text.split('\0').filter(|t| !t.is_empty()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let code = tokens[i];
        i += 1;
        let letter = code.chars().next().unwrap_or('M');
        match letter {
            'R' | 'C' => {
                if i + 1 >= tokens.len() {
                    break;
                }
                let orig_path = tokens[i].to_string();
                let path = tokens[i + 1].to_string();
                i += 2;
                out.push(ChangedFile {
                    path,
                    orig_path: Some(orig_path),
                    status: if letter == 'R' {
                        ChangedFileStatus::Renamed
                    } else {
                        ChangedFileStatus::Copied
                    },
                });
            }
            _ => {
                if i >= tokens.len() {
                    break;
                }
                let path = tokens[i].to_string();
                i += 1;
                let status = match letter {
                    'A' => ChangedFileStatus::Added,
                    'D' => ChangedFileStatus::Deleted,
                    'T' => ChangedFileStatus::TypeChanged,
                    _ => ChangedFileStatus::Modified,
                };
                out.push(ChangedFile {
                    path,
                    orig_path: None,
                    status,
                });
            }
        }
    }
    out
}

/// Changed files for a single commit, diffed against its first parent (root commits diff
/// against the empty tree via `--root`) — mirrors what `git show`'s file list would contain.
pub async fn commit_files(repo: &Path, sha: &str) -> Result<Vec<ChangedFile>, GitError> {
    let parent_ref = format!("{sha}^");
    match git(repo, &["rev-parse", &parent_ref], GitOpts::default()).await {
        Ok(parent_out) => {
            let parent = String::from_utf8_lossy(&parent_out.stdout).trim().to_string();
            let output = git(
                repo,
                &["diff", "--no-color", "--name-status", "-M", "-z", &parent, sha],
                GitOpts::default(),
            )
            .await?;
            Ok(parse_name_status(&output.stdout))
        }
        Err(_) => {
            let output = git(
                repo,
                &[
                    "diff-tree",
                    "--no-commit-id",
                    "--name-status",
                    "-M",
                    "-r",
                    "-z",
                    "--root",
                    sha,
                ],
                GitOpts::default(),
            )
            .await?;
            Ok(parse_name_status(&output.stdout))
        }
    }
}

/// Changed files between two arbitrary commits — compare mode's file list (§15.5).
pub async fn diff_files_two_commits(repo: &Path, a: &str, b: &str) -> Result<Vec<ChangedFile>, GitError> {
    let output = git(
        repo,
        &["diff", "--no-color", "--name-status", "-M", "-z", a, b],
        GitOpts::default(),
    )
    .await?;
    Ok(parse_name_status(&output.stdout))
}

#[tauri::command]
pub async fn get_commit_files(
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
) -> Result<Vec<ChangedFile>, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(commit_files(&handle.path, &sha).await?)
}

#[tauri::command]
pub async fn get_diff_files(
    state: State<'_, AppState>,
    repo_id: String,
    a: String,
    b: String,
) -> Result<Vec<ChangedFile>, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(diff_files_two_commits(&handle.path, &a, &b).await?)
}

fn extract_diff_path(raw: &str) -> Option<String> {
    let raw = raw.trim();
    // `git show` sometimes trails a tab + extra info on --- / +++ lines; drop it.
    let raw = raw.split('\t').next().unwrap_or(raw).trim();
    if raw == "/dev/null" {
        return None;
    }
    let stripped = raw
        .strip_prefix("a/")
        .or_else(|| raw.strip_prefix("b/"))
        .unwrap_or(raw);
    Some(stripped.to_string())
}

fn is_image_path(path: &Option<String>) -> bool {
    path.as_ref()
        .and_then(|p| p.rsplit('.').next())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Parses unified diff output. Tolerates a preamble before the first `diff --git` line (`git
/// show` prefixes the diff with the commit message).
pub fn parse_diff_output(stdout: &[u8]) -> FileDiff {
    let contains_nul = stdout.contains(&0u8);
    let text = String::from_utf8_lossy(stdout);
    let lines: Vec<&str> = text.lines().collect();

    let hunk_re = Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@.*$").unwrap();
    let binary_re = Regex::new(r"^Binary files (.+) and (.+) differ$").unwrap();

    let mut old_path: Option<String> = None;
    let mut new_path: Option<String> = None;
    let mut is_binary = false;
    let mut hunks: Vec<Hunk> = Vec::new();

    let mut i = 0;
    while i < lines.len() && !lines[i].starts_with("diff --git ") {
        i += 1;
    }

    while i < lines.len() {
        let line = lines[i];
        if let Some(rest) = line.strip_prefix("--- ") {
            old_path = extract_diff_path(rest);
            i += 1;
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            new_path = extract_diff_path(rest);
            i += 1;
        } else if let Some(caps) = binary_re.captures(line) {
            is_binary = true;
            if old_path.is_none() {
                old_path = extract_diff_path(&caps[1]);
            }
            if new_path.is_none() {
                new_path = extract_diff_path(&caps[2]);
            }
            i += 1;
        } else if let Some(caps) = hunk_re.captures(line) {
            let old_start: u32 = caps[1].parse().unwrap_or(0);
            let new_start: u32 = caps[3].parse().unwrap_or(0);
            let header = line.to_string();
            i += 1;

            let mut hunk_lines: Vec<DiffLine> = Vec::new();
            let mut old_no = old_start;
            let mut new_no = new_start;

            while i < lines.len() {
                let l = lines[i];
                if l.starts_with("@@ ") || l.starts_with("diff --git ") {
                    break;
                }
                if let Some(stripped) = l.strip_prefix('\\') {
                    if stripped.trim_start().starts_with("No newline") {
                        if let Some(last) = hunk_lines.last_mut() {
                            last.no_newline_at_eof = true;
                        }
                    }
                    i += 1;
                    continue;
                }
                if let Some(rest) = l.strip_prefix('+') {
                    hunk_lines.push(DiffLine {
                        kind: DiffLineKind::Add,
                        old_no: None,
                        new_no: Some(new_no),
                        text: rest.to_string(),
                        no_newline_at_eof: false,
                    });
                    new_no += 1;
                } else if let Some(rest) = l.strip_prefix('-') {
                    hunk_lines.push(DiffLine {
                        kind: DiffLineKind::Del,
                        old_no: Some(old_no),
                        new_no: None,
                        text: rest.to_string(),
                        no_newline_at_eof: false,
                    });
                    old_no += 1;
                } else {
                    let rest = l.strip_prefix(' ').unwrap_or(l);
                    hunk_lines.push(DiffLine {
                        kind: DiffLineKind::Context,
                        old_no: Some(old_no),
                        new_no: Some(new_no),
                        text: rest.to_string(),
                        no_newline_at_eof: false,
                    });
                    old_no += 1;
                    new_no += 1;
                }
                i += 1;
            }
            hunks.push(Hunk {
                header,
                lines: hunk_lines,
            });
        } else {
            i += 1;
        }
    }

    // Defensive backstop: a real text diff never contains a NUL byte, so if one slipped through
    // treat the file as binary regardless of whether git's own header said so.
    if contains_nul {
        is_binary = true;
        hunks.clear();
    }

    let is_image = is_image_path(&old_path) || is_image_path(&new_path);

    FileDiff {
        is_binary,
        is_image,
        old_path,
        new_path,
        hunks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_modification() {
        let diff = "diff --git a/file.txt b/file.txt\nindex e69de29..4b825dc 100644\n--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-line2\n+line2 modified\n line3\n";
        let result = parse_diff_output(diff.as_bytes());
        assert!(!result.is_binary);
        assert_eq!(result.old_path.as_deref(), Some("file.txt"));
        assert_eq!(result.new_path.as_deref(), Some("file.txt"));
        assert_eq!(result.hunks.len(), 1);
        let lines = &result.hunks[0].lines;
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].kind, DiffLineKind::Context);
        assert_eq!(lines[0].old_no, Some(1));
        assert_eq!(lines[0].new_no, Some(1));
        assert_eq!(lines[1].kind, DiffLineKind::Del);
        assert_eq!(lines[1].old_no, Some(2));
        assert_eq!(lines[1].new_no, None);
        assert_eq!(lines[2].kind, DiffLineKind::Add);
        assert_eq!(lines[2].new_no, Some(2));
        assert_eq!(lines[3].old_no, Some(3));
        assert_eq!(lines[3].new_no, Some(3));
    }

    #[test]
    fn parses_added_file_from_dev_null() {
        let diff = "diff --git a/new.txt b/new.txt\nnew file mode 100644\nindex 0000000..e69de29\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+hello\n+world\n";
        let result = parse_diff_output(diff.as_bytes());
        assert_eq!(result.old_path, None);
        assert_eq!(result.new_path.as_deref(), Some("new.txt"));
        assert_eq!(result.hunks[0].lines.len(), 2);
        assert!(result.hunks[0]
            .lines
            .iter()
            .all(|l| l.kind == DiffLineKind::Add));
    }

    #[test]
    fn detects_binary_files_differ() {
        let diff = "diff --git a/image.png b/image.png\nindex abc123..def456 100644\nBinary files a/image.png and b/image.png differ\n";
        let result = parse_diff_output(diff.as_bytes());
        assert!(result.is_binary);
        assert!(result.is_image);
        assert!(result.hunks.is_empty());
        assert_eq!(result.new_path.as_deref(), Some("image.png"));
    }

    #[test]
    fn null_byte_backstop_forces_binary() {
        let mut diff = b"diff --git a/weird b/weird\n--- a/weird\n+++ b/weird\n@@ -1,1 +1,1 @@\n-old\x00stuff\n+new\n".to_vec();
        diff.extend_from_slice(b"\n");
        let result = parse_diff_output(&diff);
        assert!(result.is_binary);
        assert!(result.hunks.is_empty());
    }

    #[test]
    fn no_newline_marker_attaches_to_preceding_line() {
        let diff = "diff --git a/file.txt b/file.txt\nindex abc..def 100644\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old content\n\\ No newline at end of file\n+new content\n\\ No newline at end of file\n";
        let result = parse_diff_output(diff.as_bytes());
        let lines = &result.hunks[0].lines;
        assert_eq!(lines.len(), 2);
        assert!(lines[0].no_newline_at_eof);
        assert!(lines[1].no_newline_at_eof);
    }

    #[test]
    fn skips_commit_preamble_from_git_show() {
        let diff = "commit abc123\nAuthor: Jane <jane@example.com>\nDate:   Mon Jan 1 00:00:00 2024\n\n    Commit subject\n\ndiff --git a/file.txt b/file.txt\nindex abc..def 100644\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n";
        let result = parse_diff_output(diff.as_bytes());
        assert_eq!(result.hunks.len(), 1);
        assert_eq!(result.hunks[0].lines.len(), 2);
    }

    #[test]
    fn hunk_header_without_explicit_counts_defaults_to_one() {
        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n";
        let result = parse_diff_output(diff.as_bytes());
        let lines = &result.hunks[0].lines;
        assert_eq!(lines[0].old_no, Some(1));
        assert_eq!(lines[1].new_no, Some(1));
    }

    fn join_nul(records: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for r in records {
            out.extend_from_slice(r.as_bytes());
            out.push(0);
        }
        out
    }

    #[test]
    fn parses_added_modified_deleted_name_status() {
        let bytes = join_nul(&["A", "new.txt", "M", "changed.txt", "D", "gone.txt"]);
        let files = parse_name_status(&bytes);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "new.txt");
        assert_eq!(files[0].status, ChangedFileStatus::Added);
        assert_eq!(files[1].status, ChangedFileStatus::Modified);
        assert_eq!(files[2].status, ChangedFileStatus::Deleted);
    }

    #[test]
    fn parses_rename_as_rename_not_delete_plus_add() {
        let bytes = join_nul(&["R100", "old_name.txt", "new_name.txt"]);
        let files = parse_name_status(&bytes);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, ChangedFileStatus::Renamed);
        assert_eq!(files[0].path, "new_name.txt");
        assert_eq!(files[0].orig_path.as_deref(), Some("old_name.txt"));
    }
}
