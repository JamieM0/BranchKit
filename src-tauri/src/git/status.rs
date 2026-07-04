//! Working tree / index status — ARCHITECTURE.md §6.1. `--porcelain=v2 -z` gives NUL-terminated,
//! unambiguous records (no quoting to undo) and a `--branch` header we use as a cross-check.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

use super::exec::{git, GitError, GitOpts};
use super::ops::require_repo;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FileStatusCode {
    Unmodified,
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    UpdatedButUnmerged,
    TypeChanged,
    Untracked,
    Ignored,
}

impl FileStatusCode {
    fn from_char(c: char) -> Self {
        match c {
            'M' => Self::Modified,
            'A' => Self::Added,
            'D' => Self::Deleted,
            'R' => Self::Renamed,
            'C' => Self::Copied,
            'U' => Self::UpdatedButUnmerged,
            'T' => Self::TypeChanged,
            '?' => Self::Untracked,
            '!' => Self::Ignored,
            _ => Self::Unmodified,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StatusEntryKind {
    Ordinary,
    RenamedOrCopied,
    Unmerged,
    Untracked,
    Ignored,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    pub path: String,
    /// Set for renames/copies (type `2` records).
    pub orig_path: Option<String>,
    pub kind: StatusEntryKind,
    /// `X` — index/staged state. `.` maps to `Unmodified`.
    pub index_status: FileStatusCode,
    /// `Y` — worktree/unstaged state. `.` maps to `Unmodified`.
    ///
    /// A single entry can have both non-`Unmodified` — that's a partially-staged file; the
    /// frontend renders it into both the staged and unstaged lists from this one entry.
    pub worktree_status: FileStatusCode,
    pub is_submodule: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BranchStatus {
    pub oid: Option<String>,
    /// `None` when detached (`branch.head` is `(detached)`).
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatusReport {
    pub branch: BranchStatus,
    pub entries: Vec<StatusEntry>,
}

pub async fn status(repo: &Path) -> Result<StatusReport, GitError> {
    let output = git(
        repo,
        &["status", "--porcelain=v2", "--branch", "-z"],
        GitOpts::default(),
    )
    .await?;
    Ok(parse_status(&output.stdout))
}

/// Working-directory panel's status feed — DESIGN_SPEC.md §6.1. Re-fetched on `WorkingTree`,
/// `Index` and `Head` refresh events (a checkout changes the branch name in the header too).
#[tauri::command]
pub async fn get_status(state: State<'_, AppState>, repo_id: String) -> Result<StatusReport, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    Ok(status(&handle.path).await?)
}

fn parse_branch_header(report: &mut BranchStatus, header: &str) {
    // header is the text after "# ", e.g. "branch.oid <sha>" / "branch.ab +1 -2"
    if let Some(oid) = header.strip_prefix("branch.oid ") {
        report.oid = Some(oid.trim().to_string());
    } else if let Some(head) = header.strip_prefix("branch.head ") {
        let head = head.trim();
        report.head = if head == "(detached)" {
            None
        } else {
            Some(head.to_string())
        };
    } else if let Some(upstream) = header.strip_prefix("branch.upstream ") {
        report.upstream = Some(upstream.trim().to_string());
    } else if let Some(ab) = header.strip_prefix("branch.ab ") {
        for part in ab.split_ascii_whitespace() {
            if let Some(n) = part.strip_prefix('+') {
                report.ahead = n.parse().unwrap_or(0);
            } else if let Some(n) = part.strip_prefix('-') {
                report.behind = n.parse().unwrap_or(0);
            }
        }
    }
}

/// Parses the raw `-z` NUL-terminated byte stream from `git status --porcelain=v2 --branch -z`.
/// Records are split on NUL, but type `2` (rename/copy) records consume one extra NUL-delimited
/// token (the original path) after the record's own fields.
fn parse_status(stdout: &[u8]) -> StatusReport {
    let text = String::from_utf8_lossy(stdout);
    // Split on NUL, keep empty trailing fields out (a trailing NUL leaves one empty token).
    let mut tokens: std::collections::VecDeque<&str> =
        text.split('\0').filter(|t| !t.is_empty()).collect();

    let mut branch = BranchStatus::default();
    let mut entries = Vec::new();

    while let Some(token) = tokens.pop_front() {
        if let Some(header) = token.strip_prefix("# ") {
            parse_branch_header(&mut branch, header);
            continue;
        }

        let mut fields = token.splitn(2, ' ');
        let record_type = fields.next().unwrap_or_default();
        let rest = fields.next().unwrap_or_default();

        match record_type {
            "1" => {
                // 1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>
                let cols: Vec<&str> = rest.splitn(8, ' ').collect();
                if cols.len() < 8 {
                    continue;
                }
                let xy = cols[0];
                let sub = cols[1];
                let path = cols[7].to_string();
                entries.push(StatusEntry {
                    path,
                    orig_path: None,
                    kind: StatusEntryKind::Ordinary,
                    index_status: FileStatusCode::from_char(xy.chars().next().unwrap_or('.')),
                    worktree_status: FileStatusCode::from_char(xy.chars().nth(1).unwrap_or('.')),
                    is_submodule: sub != "N...",
                });
            }
            "2" => {
                // 2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <score> <path>  (origPath is the next NUL token)
                let cols: Vec<&str> = rest.splitn(9, ' ').collect();
                if cols.len() < 9 {
                    continue;
                }
                let xy = cols[0];
                let sub = cols[1];
                let path = cols[8].to_string();
                let orig_path = tokens.pop_front().map(|s| s.to_string());
                entries.push(StatusEntry {
                    path,
                    orig_path,
                    kind: StatusEntryKind::RenamedOrCopied,
                    index_status: FileStatusCode::from_char(xy.chars().next().unwrap_or('.')),
                    worktree_status: FileStatusCode::from_char(xy.chars().nth(1).unwrap_or('.')),
                    is_submodule: sub != "N...",
                });
            }
            "u" => {
                // u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>
                let cols: Vec<&str> = rest.splitn(10, ' ').collect();
                if cols.len() < 10 {
                    continue;
                }
                let xy = cols[0];
                let sub = cols[1];
                let path = cols[9].to_string();
                entries.push(StatusEntry {
                    path,
                    orig_path: None,
                    kind: StatusEntryKind::Unmerged,
                    index_status: FileStatusCode::from_char(xy.chars().next().unwrap_or('.')),
                    worktree_status: FileStatusCode::from_char(xy.chars().nth(1).unwrap_or('.')),
                    is_submodule: sub != "N...",
                });
            }
            "?" => {
                entries.push(StatusEntry {
                    path: rest.to_string(),
                    orig_path: None,
                    kind: StatusEntryKind::Untracked,
                    index_status: FileStatusCode::Untracked,
                    worktree_status: FileStatusCode::Untracked,
                    is_submodule: false,
                });
            }
            "!" => {
                entries.push(StatusEntry {
                    path: rest.to_string(),
                    orig_path: None,
                    kind: StatusEntryKind::Ignored,
                    index_status: FileStatusCode::Ignored,
                    worktree_status: FileStatusCode::Ignored,
                    is_submodule: false,
                });
            }
            _ => {}
        }
    }

    StatusReport { branch, entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join(records: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for r in records {
            out.extend_from_slice(r.as_bytes());
            out.push(0);
        }
        out
    }

    #[test]
    fn parses_branch_header_fields() {
        let bytes = join(&[
            "# branch.oid abc123",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -1",
        ]);
        let report = parse_status(&bytes);
        assert_eq!(report.branch.oid, Some("abc123".to_string()));
        assert_eq!(report.branch.head, Some("main".to_string()));
        assert_eq!(report.branch.upstream, Some("origin/main".to_string()));
        assert_eq!(report.branch.ahead, 2);
        assert_eq!(report.branch.behind, 1);
    }

    #[test]
    fn detached_head_maps_to_none() {
        let bytes = join(&["# branch.head (detached)"]);
        let report = parse_status(&bytes);
        assert_eq!(report.branch.head, None);
    }

    #[test]
    fn parses_ordinary_modified_entry() {
        let bytes = join(&["1 M. N... 100644 100644 100644 abc def file.txt"]);
        let report = parse_status(&bytes);
        assert_eq!(report.entries.len(), 1);
        let e = &report.entries[0];
        assert_eq!(e.path, "file.txt");
        assert_eq!(e.index_status, FileStatusCode::Modified);
        assert_eq!(e.worktree_status, FileStatusCode::Unmodified);
        assert!(!e.is_submodule);
    }

    #[test]
    fn parses_partially_staged_entry_both_sections() {
        // staged modification (X=M) plus further unstaged modification (Y=M)
        let bytes = join(&["1 MM N... 100644 100644 100644 abc def file.txt"]);
        let report = parse_status(&bytes);
        let e = &report.entries[0];
        assert_eq!(e.index_status, FileStatusCode::Modified);
        assert_eq!(e.worktree_status, FileStatusCode::Modified);
    }

    #[test]
    fn parses_rename_with_orig_path_from_next_token() {
        let bytes = join(&[
            "2 R. N... 100644 100644 100644 abc def R100 new_name.txt",
            "old_name.txt",
        ]);
        let report = parse_status(&bytes);
        assert_eq!(report.entries.len(), 1);
        let e = &report.entries[0];
        assert_eq!(e.path, "new_name.txt");
        assert_eq!(e.orig_path, Some("old_name.txt".to_string()));
        assert_eq!(e.kind, StatusEntryKind::RenamedOrCopied);
        assert_eq!(e.index_status, FileStatusCode::Renamed);
    }

    #[test]
    fn parses_unmerged_both_modified() {
        let bytes =
            join(&["u UU N... 100644 100644 100644 100644 base ours theirs conflicted.txt"]);
        let report = parse_status(&bytes);
        let e = &report.entries[0];
        assert_eq!(e.kind, StatusEntryKind::Unmerged);
        assert_eq!(e.index_status, FileStatusCode::UpdatedButUnmerged);
        assert_eq!(e.worktree_status, FileStatusCode::UpdatedButUnmerged);
        assert_eq!(e.path, "conflicted.txt");
    }

    #[test]
    fn parses_untracked_entry() {
        let bytes = join(&["? new_file.txt"]);
        let report = parse_status(&bytes);
        assert_eq!(report.entries[0].kind, StatusEntryKind::Untracked);
        assert_eq!(report.entries[0].path, "new_file.txt");
    }

    #[test]
    fn parses_ignored_entry() {
        let bytes = join(&["! build/output.log"]);
        let report = parse_status(&bytes);
        assert_eq!(report.entries[0].kind, StatusEntryKind::Ignored);
        assert_eq!(report.entries[0].path, "build/output.log");
    }

    #[test]
    fn parses_mixed_records_in_one_stream() {
        let bytes = join(&[
            "# branch.head main",
            "1 M. N... 100644 100644 100644 a b modified.txt",
            "? untracked.txt",
        ]);
        let report = parse_status(&bytes);
        assert_eq!(report.branch.head, Some("main".to_string()));
        assert_eq!(report.entries.len(), 2);
    }
}
