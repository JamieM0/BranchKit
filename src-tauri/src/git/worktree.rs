//! Worktrees — ARCHITECTURE.md §7.1, DESIGN_SPEC.md §5 (left-panel WORKTREES section).
//! `git worktree list --porcelain` is a stable, parseable record format (blank-line separated,
//! `key value` lines). We surface just what the panel needs: path + branch (or detached sha).

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

use super::exec::{git, GitError, GitOpts};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub path: String,
    /// Short branch name, or `None` when the worktree is detached.
    pub branch: Option<String>,
    pub head: String,
    pub detached: bool,
    /// The main (non-linked) worktree — can't be removed; shown but not offered "Remove".
    pub is_main: bool,
    /// Locked worktrees can't be pruned/removed without `--force`.
    pub locked: bool,
}

fn short_branch(refname: &str) -> String {
    refname
        .strip_prefix("refs/heads/")
        .unwrap_or(refname)
        .to_string()
}

pub fn parse_worktree_porcelain(text: &str) -> Vec<WorktreeInfo> {
    let mut result = Vec::new();
    let mut path: Option<String> = None;
    let mut head = String::new();
    let mut branch: Option<String> = None;
    let mut detached = false;
    let mut locked = false;

    let flush = |path: &mut Option<String>,
                     head: &mut String,
                     branch: &mut Option<String>,
                     detached: &mut bool,
                     locked: &mut bool,
                     result: &mut Vec<WorktreeInfo>| {
        if let Some(p) = path.take() {
            let is_main = result.is_empty();
            result.push(WorktreeInfo {
                path: p,
                branch: branch.take(),
                head: std::mem::take(head),
                detached: *detached,
                is_main,
                locked: *locked,
            });
        }
        *detached = false;
        *locked = false;
    };

    for line in text.lines() {
        if line.is_empty() {
            flush(&mut path, &mut head, &mut branch, &mut detached, &mut locked, &mut result);
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            // A new record started without a blank separator (shouldn't happen, but be safe).
            flush(&mut path, &mut head, &mut branch, &mut detached, &mut locked, &mut result);
            path = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("HEAD ") {
            head = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("branch ") {
            branch = Some(short_branch(rest));
        } else if line == "detached" {
            detached = true;
        } else if line == "locked" || line.starts_with("locked ") {
            locked = true;
        }
    }
    flush(&mut path, &mut head, &mut branch, &mut detached, &mut locked, &mut result);
    result
}

pub async fn worktree_list(repo: &Path) -> Result<Vec<WorktreeInfo>, GitError> {
    let output = git(repo, &["worktree", "list", "--porcelain"], GitOpts::default()).await?;
    Ok(parse_worktree_porcelain(&String::from_utf8_lossy(&output.stdout)))
}

#[tauri::command]
pub async fn get_worktrees(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<WorktreeInfo>, AppError> {
    let handle = state.get_repo(&repo_id).ok_or_else(|| {
        AppError::new(
            "Repository is not open",
            format!("unknown repo id {repo_id}"),
        )
    })?;
    Ok(worktree_list(&handle.path).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_main_and_linked_worktrees() {
        let text = "\
worktree /home/u/repo
HEAD abc123
branch refs/heads/main

worktree /home/u/repo-side
HEAD def456
branch refs/heads/side

worktree /home/u/repo-detach
HEAD 9990000
detached
";
        let worktrees = parse_worktree_porcelain(text);
        assert_eq!(worktrees.len(), 3);
        assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
        assert!(worktrees[0].is_main);
        assert_eq!(worktrees[1].branch.as_deref(), Some("side"));
        assert!(!worktrees[1].is_main);
        assert!(worktrees[2].detached);
        assert_eq!(worktrees[2].branch, None);
    }
}
