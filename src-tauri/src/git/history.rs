//! File history — DESIGN_SPEC.md §6.3. `git log --follow -- <path>` gives the commit list across
//! renames; the per-commit diff for the selected row reuses the same `--follow` walk (limited to
//! one commit via `<sha> -1`) so the diff lands on whatever path the file had at that commit,
//! without the frontend needing to track renamed paths itself.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

use super::diff::{parse_diff_output, FileDiff};
use super::exec::{git, GitError, GitOpts};
use super::ops::require_repo;

const UNIT_SEP: char = '\u{1f}';
const RECORD_SEP: char = '\u{1e}';

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileHistoryEntry {
    pub sha: String,
    pub author_name: String,
    pub author_email: String,
    /// Unix seconds (author date).
    pub author_time: i64,
    pub subject: String,
}

fn parse_history_records(text: &str) -> Vec<FileHistoryEntry> {
    text.split(RECORD_SEP)
        .filter_map(|record| {
            let record = record.trim_start_matches('\n');
            if record.trim().is_empty() {
                return None;
            }
            let mut fields = record.splitn(5, UNIT_SEP);
            let sha = fields.next()?.to_string();
            let author_name = fields.next()?.to_string();
            let author_email = fields.next()?.to_string();
            let author_time: i64 = fields.next()?.trim().parse().ok()?;
            let subject = fields.next().unwrap_or("").trim_end_matches('\n').to_string();
            Some(FileHistoryEntry {
                sha,
                author_name,
                author_email,
                author_time,
                subject,
            })
        })
        .collect()
}

/// The commit list for `path`, newest first, following renames — the History view's left column.
pub async fn file_history(repo: &Path, path: &str) -> Result<Vec<FileHistoryEntry>, GitError> {
    let format = format!(
        "--pretty=format:%H{UNIT_SEP}%an{UNIT_SEP}%ae{UNIT_SEP}%at{UNIT_SEP}%s{RECORD_SEP}"
    );
    let output = git(
        repo,
        &["log", "--follow", &format, "--", path],
        GitOpts::default(),
    )
    .await?;
    Ok(parse_history_records(&String::from_utf8_lossy(&output.stdout)))
}

/// The diff for a single commit in `path`'s `--follow` history — `git log <sha> -1 --follow -p`
/// walks history from `sha` following renames and prints exactly one patch, landing on whatever
/// path the file had at that commit even if it differs from `path` (a later name).
pub async fn file_history_diff(repo: &Path, path: &str, sha: &str) -> Result<FileDiff, GitError> {
    let output = git(
        repo,
        &[
            "log",
            sha,
            "-1",
            "--follow",
            "-p",
            "--no-color",
            "-U3",
            "--pretty=format:",
            "--",
            path,
        ],
        GitOpts::default(),
    )
    .await?;
    Ok(parse_diff_output(&output.stdout))
}

#[tauri::command]
pub async fn get_file_history(
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<Vec<FileHistoryEntry>, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(file_history(&handle.path, &path).await?)
}

#[tauri::command]
pub async fn get_file_history_diff(
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
    sha: String,
) -> Result<FileDiff, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(file_history_diff(&handle.path, &path, &sha).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_history_record() {
        let text = format!(
            "abc123{UNIT_SEP}Jane Doe{UNIT_SEP}jane@example.com{UNIT_SEP}1700000000{UNIT_SEP}Fix bug{RECORD_SEP}"
        );
        let entries = parse_history_records(&text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sha, "abc123");
        assert_eq!(entries[0].author_name, "Jane Doe");
        assert_eq!(entries[0].author_time, 1700000000);
        assert_eq!(entries[0].subject, "Fix bug");
    }

    #[test]
    fn parses_multiple_history_records_in_order() {
        let text = format!(
            "sha2{UNIT_SEP}B{UNIT_SEP}b@x.com{UNIT_SEP}2{UNIT_SEP}second{RECORD_SEP}\nsha1{UNIT_SEP}A{UNIT_SEP}a@x.com{UNIT_SEP}1{UNIT_SEP}first{RECORD_SEP}"
        );
        let entries = parse_history_records(&text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sha, "sha2");
        assert_eq!(entries[1].sha, "sha1");
    }

    #[test]
    fn empty_subject_is_handled() {
        let text = format!("sha{UNIT_SEP}A{UNIT_SEP}a@x.com{UNIT_SEP}1{UNIT_SEP}{RECORD_SEP}");
        let entries = parse_history_records(&text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].subject, "");
    }

    async fn init_repo(dir: &Path) {
        git(dir, &["init", "--initial-branch=main", "-q"], GitOpts::default())
            .await
            .unwrap();
        git(dir, &["config", "user.name", "T"], GitOpts::default())
            .await
            .unwrap();
        git(dir, &["config", "user.email", "t@example.com"], GitOpts::default())
            .await
            .unwrap();
        git(dir, &["config", "commit.gpgsign", "false"], GitOpts::default())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn file_history_follows_a_rename() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;

        std::fs::write(dir.path().join("old.txt"), "line1\n").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default()).await.unwrap();
        git(dir.path(), &["commit", "-q", "-m", "add old.txt"], GitOpts::default())
            .await
            .unwrap();

        git(
            dir.path(),
            &["mv", "old.txt", "new.txt"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        git(dir.path(), &["commit", "-q", "-m", "rename to new.txt"], GitOpts::default())
            .await
            .unwrap();

        std::fs::write(dir.path().join("new.txt"), "line1\nline2\n").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default()).await.unwrap();
        git(dir.path(), &["commit", "-q", "-m", "edit new.txt"], GitOpts::default())
            .await
            .unwrap();

        let history = file_history(dir.path(), "new.txt").await.unwrap();
        let subjects: Vec<&str> = history.iter().map(|e| e.subject.as_str()).collect();
        assert_eq!(subjects, vec!["edit new.txt", "rename to new.txt", "add old.txt"]);
    }

    #[tokio::test]
    async fn file_history_diff_shows_the_commit_patch() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;

        std::fs::write(dir.path().join("f.txt"), "line1\n").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default()).await.unwrap();
        git(dir.path(), &["commit", "-q", "-m", "init"], GitOpts::default())
            .await
            .unwrap();

        std::fs::write(dir.path().join("f.txt"), "line1\nline2\n").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default()).await.unwrap();
        git(dir.path(), &["commit", "-q", "-m", "add line2"], GitOpts::default())
            .await
            .unwrap();

        let head = git(dir.path(), &["rev-parse", "HEAD"], GitOpts::default())
            .await
            .unwrap();
        let sha = String::from_utf8_lossy(&head.stdout).trim().to_string();

        let diff = file_history_diff(dir.path(), "f.txt", &sha).await.unwrap();
        assert_eq!(diff.hunks.len(), 1);
        assert!(diff.hunks[0].lines.iter().any(|l| l.text == "line2"));
    }
}
