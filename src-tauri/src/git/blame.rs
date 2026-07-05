//! Blame — DESIGN_SPEC.md §6.3. Parses `git blame --porcelain`, never the human-readable format:
//! porcelain gives one stable, unambiguous record per line with commit metadata attached, so we
//! never have to guess where an author name ends and a summary begins. Adjacent lines attributed
//! to the same commit are grouped into `BlameRun`s (ARCHITECTURE.md's "group line-runs by commit")
//! so the frontend gutter can draw one author disc per run instead of repeating it every line.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use super::exec::{git, GitError, GitOpts};
use super::ops::require_repo;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlameRun {
    pub sha: String,
    pub author_name: String,
    pub author_email: String,
    /// Unix seconds (author date).
    pub author_time: i64,
    pub summary: String,
    /// 1-based line number of the run's first line in the current file.
    pub start_line: u32,
    pub lines: Vec<String>,
}

#[derive(Default, Clone)]
struct BlameMeta {
    author_name: String,
    author_email: String,
    author_time: i64,
    summary: String,
}

/// One porcelain line record: `(final_line_number, sha, text)`.
type BlameLineRecord = (u32, String, String);

/// A group-header line is `<40-hex-sha> <orig-line> <final-line>[ <num-lines>]` — distinguished
/// from a metadata line by its first token being exactly 40 hex digits.
fn is_group_header(line: &str) -> bool {
    let Some(first) = line.split_ascii_whitespace().next() else {
        return false;
    };
    first.len() == 40 && first.chars().all(|c| c.is_ascii_hexdigit())
}

/// Single pass over `git blame --porcelain` output: one `(final_line, sha, text)` per line, plus
/// each commit's metadata (recorded only from its *first* appearance — porcelain omits the header
/// fields on later reuses of an already-seen commit).
fn parse_blame(text: &str) -> (Vec<BlameLineRecord>, HashMap<String, BlameMeta>) {
    let mut meta_by_sha: HashMap<String, BlameMeta> = HashMap::new();
    let mut current_sha = String::new();
    let mut current_final: u32 = 0;
    let mut lines: Vec<(u32, String, String)> = Vec::new();

    for line in text.split('\n') {
        if let Some(rest) = line.strip_prefix('\t') {
            lines.push((current_final, current_sha.clone(), rest.to_string()));
            continue;
        }
        if is_group_header(line) {
            let mut parts = line.split_ascii_whitespace();
            current_sha = parts.next().unwrap_or_default().to_string();
            let _orig_line = parts.next();
            current_final = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            meta_by_sha.entry(current_sha.clone()).or_default();
            continue;
        }
        let meta = meta_by_sha.entry(current_sha.clone()).or_default();
        if let Some(rest) = line.strip_prefix("author-mail ") {
            if meta.author_email.is_empty() {
                meta.author_email = rest.trim_start_matches('<').trim_end_matches('>').to_string();
            }
        } else if let Some(rest) = line.strip_prefix("author-time ") {
            if meta.author_time == 0 {
                meta.author_time = rest.trim().parse().unwrap_or(0);
            }
        } else if let Some(rest) = line.strip_prefix("author ") {
            if meta.author_name.is_empty() {
                meta.author_name = rest.to_string();
            }
        } else if let Some(rest) = line.strip_prefix("summary ") {
            if meta.summary.is_empty() {
                meta.summary = rest.to_string();
            }
        }
        // committer*, previous, filename, boundary — irrelevant to the gutter, ignored.
    }

    lines.sort_by_key(|(final_line, ..)| *final_line);
    (lines, meta_by_sha)
}

/// Run-length-encodes contiguous same-commit lines (already in final-line order) into `BlameRun`s.
fn group_into_runs(
    lines: Vec<(u32, String, String)>,
    meta_by_sha: &HashMap<String, BlameMeta>,
) -> Vec<BlameRun> {
    let mut runs: Vec<BlameRun> = Vec::new();
    for (final_line, sha, text) in lines {
        if let Some(last) = runs.last_mut() {
            if last.sha == sha {
                last.lines.push(text);
                continue;
            }
        }
        let meta = meta_by_sha.get(&sha).cloned().unwrap_or_default();
        runs.push(BlameRun {
            sha,
            author_name: meta.author_name,
            author_email: meta.author_email,
            author_time: meta.author_time,
            summary: meta.summary,
            start_line: final_line,
            lines: vec![text],
        });
    }
    runs
}

/// Full parse: line records grouped into contiguous same-commit runs — the shape the gutter wants.
pub fn parse_blame_porcelain(text: &str) -> Vec<BlameRun> {
    let (lines, meta_by_sha) = parse_blame(text);
    group_into_runs(lines, &meta_by_sha)
}

/// Blames the current worktree contents of `path` against its committed history — uncommitted
/// lines come back attributed to git's own all-zero sha with author "Not Committed Yet".
pub async fn blame_file(repo: &Path, path: &str) -> Result<Vec<BlameRun>, GitError> {
    let output = git(
        repo,
        &["blame", "--porcelain", "--", path],
        GitOpts::default(),
    )
    .await?;
    Ok(parse_blame_porcelain(&String::from_utf8_lossy(&output.stdout)))
}

#[tauri::command]
pub async fn get_blame(
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<Vec<BlameRun>, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(blame_file(&handle.path, &path).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_porcelain() -> String {
        [
            "a00000000000000000000000000000000000000a 1 1 2",
            "author Jane Doe",
            "author-mail <jane@example.com>",
            "author-time 1700000000",
            "author-tz +0000",
            "committer Jane Doe",
            "committer-mail <jane@example.com>",
            "committer-time 1700000000",
            "committer-tz +0000",
            "summary Add file",
            "filename f.txt",
            "\tline one",
            "a00000000000000000000000000000000000000a 2 2",
            "\tline two",
            "b00000000000000000000000000000000000000b 3 3 1",
            "author John Smith",
            "author-mail <john@example.com>",
            "author-time 1700100000",
            "author-tz +0000",
            "committer John Smith",
            "committer-mail <john@example.com>",
            "committer-time 1700100000",
            "committer-tz +0000",
            "summary Tweak line three",
            "previous a00000000000000000000000000000000000000a f.txt",
            "filename f.txt",
            "\tline three",
        ]
        .join("\n")
    }

    #[test]
    fn groups_consecutive_same_commit_lines_into_one_run() {
        let runs = parse_blame_porcelain(&sample_porcelain());
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].sha, "a00000000000000000000000000000000000000a");
        assert_eq!(runs[0].author_name, "Jane Doe");
        assert_eq!(runs[0].author_email, "jane@example.com");
        assert_eq!(runs[0].author_time, 1700000000);
        assert_eq!(runs[0].summary, "Add file");
        assert_eq!(runs[0].start_line, 1);
        assert_eq!(runs[0].lines, vec!["line one", "line two"]);

        assert_eq!(runs[1].sha, "b00000000000000000000000000000000000000b");
        assert_eq!(runs[1].author_name, "John Smith");
        assert_eq!(runs[1].start_line, 3);
        assert_eq!(runs[1].lines, vec!["line three"]);
    }

    #[test]
    fn identifies_group_headers_by_forty_hex_char_sha() {
        assert!(is_group_header(
            "a00000000000000000000000000000000000000a 1 1 2"
        ));
        assert!(!is_group_header("author Jane Doe"));
        assert!(!is_group_header("summary Add file"));
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
    async fn blame_file_attributes_lines_to_the_right_commits() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;

        std::fs::write(dir.path().join("f.txt"), "line1\nline2\n").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default()).await.unwrap();
        git(dir.path(), &["commit", "-q", "-m", "first"], GitOpts::default())
            .await
            .unwrap();

        std::fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default()).await.unwrap();
        git(dir.path(), &["commit", "-q", "-m", "second"], GitOpts::default())
            .await
            .unwrap();

        let runs = blame_file(dir.path(), "f.txt").await.unwrap();
        assert_eq!(runs.iter().map(|r| r.lines.len()).sum::<usize>(), 3);
        assert_eq!(runs.last().unwrap().summary, "second");
    }

    #[tokio::test]
    async fn blame_file_marks_uncommitted_lines() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;

        std::fs::write(dir.path().join("f.txt"), "line1\n").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default()).await.unwrap();
        git(dir.path(), &["commit", "-q", "-m", "first"], GitOpts::default())
            .await
            .unwrap();

        std::fs::write(dir.path().join("f.txt"), "line1\nline2 uncommitted\n").unwrap();

        let runs = blame_file(dir.path(), "f.txt").await.unwrap();
        let last = runs.last().unwrap();
        assert_eq!(last.sha, "0000000000000000000000000000000000000000");
        assert_eq!(last.author_name, "Not Committed Yet");
    }
}
