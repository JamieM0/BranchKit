//! Refs & ahead/behind — ARCHITECTURE.md §5.3. `%(upstream:track)` is computed by git itself,
//! so ahead/behind is always consistent with no extra `rev-list` calls.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

use super::exec::{git, GitError, GitErrorKind, GitOpts};

const UNIT_SEP: char = '\u{1f}';

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RefKind {
    Branch,
    RemoteBranch,
    Tag,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RefInfo {
    /// Full ref name, e.g. `refs/heads/main`.
    pub name: String,
    /// `refs/heads/`, `refs/remotes/`, `refs/tags/` stripped.
    pub short_name: String,
    pub kind: RefKind,
    pub sha: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Upstream existed but was deleted (`%(upstream:track)` reports `[gone]`).
    pub gone: bool,
    pub is_head: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HeadInfo {
    pub detached: bool,
    /// Short branch name, e.g. `main`. `None` when detached.
    pub branch: Option<String>,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RefsResponse {
    pub refs: Vec<RefInfo>,
    pub head: HeadInfo,
}

fn classify(refname: &str) -> Option<(RefKind, String)> {
    if let Some(short) = refname.strip_prefix("refs/heads/") {
        return Some((RefKind::Branch, short.to_string()));
    }
    if let Some(short) = refname.strip_prefix("refs/remotes/") {
        // Skip the symbolic `refs/remotes/<remote>/HEAD` pointer — it isn't a real branch and
        // would otherwise render as a confusing extra "HEAD" pill in the graph.
        if short == "HEAD" || short.ends_with("/HEAD") {
            return None;
        }
        return Some((RefKind::RemoteBranch, short.to_string()));
    }
    refname
        .strip_prefix("refs/tags/")
        .map(|short| (RefKind::Tag, short.to_string()))
}

/// Parses `%(upstream:track)` output, e.g. `[ahead 2, behind 1]`, `[ahead 2]`, `[behind 1]`,
/// `[gone]`, or empty (no upstream / up to date).
fn parse_track(track: &str) -> (u32, u32, bool) {
    let track = track.trim();
    if track.is_empty() {
        return (0, 0, false);
    }
    if track.contains("gone") {
        return (0, 0, true);
    }
    let inner = track.trim_start_matches('[').trim_end_matches(']');
    let mut ahead = 0u32;
    let mut behind = 0u32;
    for part in inner.split(',') {
        let part = part.trim();
        if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.trim().parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.trim().parse().unwrap_or(0);
        }
    }
    (ahead, behind, false)
}

pub async fn list_refs(repo: &Path) -> Result<Vec<RefInfo>, GitError> {
    let format = format!(
        "--format=%(refname){UNIT_SEP}%(objectname){UNIT_SEP}%(upstream:short){UNIT_SEP}%(upstream:track){UNIT_SEP}%(HEAD)"
    );
    let output = git(
        repo,
        &[
            "for-each-ref",
            &format,
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ],
        GitOpts::default(),
    )
    .await?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut result = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let mut fields = line.splitn(5, UNIT_SEP);
        let name = fields.next().unwrap_or_default().to_string();
        let Some((kind, short_name)) = classify(&name) else {
            continue;
        };
        let sha = fields.next().unwrap_or_default().to_string();
        let upstream_short = fields.next().unwrap_or_default();
        let upstream = if upstream_short.is_empty() {
            None
        } else {
            Some(upstream_short.to_string())
        };
        let track = fields.next().unwrap_or_default();
        let (ahead, behind, gone) = parse_track(track);
        let head_marker = fields.next().unwrap_or_default().trim();
        let is_head = head_marker == "*";

        result.push(RefInfo {
            name,
            short_name,
            kind,
            sha,
            upstream,
            ahead,
            behind,
            gone,
            is_head,
        });
    }
    Ok(result)
}

/// Current branch / detached-HEAD state — ARCHITECTURE.md §5.3.
pub async fn head_info(repo: &Path) -> Result<HeadInfo, GitError> {
    let sha_output = git(repo, &["rev-parse", "HEAD"], GitOpts::default()).await?;
    let sha = String::from_utf8_lossy(&sha_output.stdout)
        .trim()
        .to_string();

    match git(
        repo,
        &["symbolic-ref", "-q", "--short", "HEAD"],
        GitOpts::default(),
    )
    .await
    {
        Ok(output) => {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(HeadInfo {
                detached: false,
                branch: Some(branch),
                sha,
            })
        }
        // `symbolic-ref -q` exits 1 (no stderr) when HEAD doesn't point at a branch.
        Err(e) if e.kind == GitErrorKind::NonZeroExit && e.code == Some(1) => Ok(HeadInfo {
            detached: true,
            branch: None,
            sha,
        }),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn get_refs(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<RefsResponse, AppError> {
    let handle = state.get_repo(&repo_id).ok_or_else(|| {
        AppError::new(
            "Repository is not open",
            format!("unknown repo id {repo_id}"),
        )
    })?;
    Ok(RefsResponse {
        refs: list_refs(&handle.path).await?,
        head: head_info(&handle.path).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_branch_remote_tag() {
        assert_eq!(
            classify("refs/heads/main"),
            Some((RefKind::Branch, "main".to_string()))
        );
        assert_eq!(
            classify("refs/remotes/origin/main"),
            Some((RefKind::RemoteBranch, "origin/main".to_string()))
        );
        assert_eq!(
            classify("refs/tags/v1.0.0"),
            Some((RefKind::Tag, "v1.0.0".to_string()))
        );
        assert_eq!(classify("refs/stash"), None);
    }

    #[test]
    fn parses_ahead_behind_gone_and_empty_track() {
        assert_eq!(parse_track("[ahead 2, behind 1]"), (2, 1, false));
        assert_eq!(parse_track("[ahead 2]"), (2, 0, false));
        assert_eq!(parse_track("[behind 1]"), (0, 1, false));
        assert_eq!(parse_track("[gone]"), (0, 0, true));
        assert_eq!(parse_track(""), (0, 0, false));
    }
}
