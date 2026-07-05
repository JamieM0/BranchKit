//! Commit graph data — ARCHITECTURE.md §5.1. Topology (sha + parents) is fetched once for the
//! whole repo; per-commit metadata is fetched lazily in visible-window batches. Lane assignment
//! itself is pure TS (§5.2) and lives in the frontend.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

use super::exec::{git, GitError, GitOpts};

/// Batch size for `git show` metadata lookups — ARCHITECTURE.md §5.1.
const METADATA_BATCH_SIZE: usize = 200;

const UNIT_SEP: char = '\u{1f}';
const RECORD_SEP: char = '\u{1e}';

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitTopology {
    pub sha: String,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitMeta {
    pub sha: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    /// Unix seconds (author date).
    pub author_time: i64,
    pub subject: String,
    /// Full commit body (everything after the subject line). May be empty. May contain
    /// embedded newlines — the frontend shows only its first line in the graph preview.
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    pub sha: String,
    /// The commit the stash was taken on top of (its first parent).
    pub base_sha: String,
    /// Reflog selector, e.g. `stash@{0}`.
    pub selector: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GraphTopologyRow {
    Commit {
        sha: String,
        parents: Vec<String>,
    },
    Stash {
        sha: String,
        base_sha: String,
        selector: String,
        subject: String,
    },
}

/// Full sha+parents topology for the whole repo (`rev-list --all --topo-order --parents`) —
/// fast even at 100k commits per ARCHITECTURE.md §5.1. Do this once; fetch metadata on demand.
///
/// `--exclude=refs/stash` matters: `refs/` (and therefore `--all`) includes the stash ref, which
/// would pull the stash commit *and* its internal index/untracked commits into the walk. Those are
/// surfaced separately as Stash pseudo-rows (§5.1/DESIGN_SPEC §4.5), so leaving them in here would
/// render the stash commit twice — once as a commit, once as a stash — a duplicate graph row.
pub async fn topology(repo: &Path) -> Result<Vec<CommitTopology>, GitError> {
    let output = git(
        repo,
        &[
            "rev-list",
            "--exclude=refs/stash",
            "--all",
            "--topo-order",
            "--parents",
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
        let mut parts = line.split_ascii_whitespace();
        let sha = parts.next().unwrap_or_default().to_string();
        if sha.is_empty() {
            continue;
        }
        let parents = parts.map(|s| s.to_string()).collect();
        result.push(CommitTopology { sha, parents });
    }
    Ok(result)
}

pub async fn graph_rows(repo: &Path) -> Result<Vec<GraphTopologyRow>, GitError> {
    let commits = topology(repo).await?;
    let stashes = stash_list(repo).await?;

    let mut stashes_by_base: HashMap<String, Vec<StashEntry>> = HashMap::new();
    for stash in stashes {
        stashes_by_base
            .entry(stash.base_sha.clone())
            .or_default()
            .push(stash);
    }

    let mut result = Vec::with_capacity(commits.len() + stashes_by_base.len());
    for commit in commits {
        result.push(GraphTopologyRow::Commit {
            sha: commit.sha.clone(),
            parents: commit.parents,
        });
        if let Some(stashes) = stashes_by_base.remove(&commit.sha) {
            result.extend(stashes.into_iter().map(|stash| GraphTopologyRow::Stash {
                sha: stash.sha,
                base_sha: stash.base_sha,
                selector: stash.selector,
                subject: stash.subject,
            }));
        }
    }

    for stashes in stashes_by_base.into_values() {
        result.extend(stashes.into_iter().map(|stash| GraphTopologyRow::Stash {
            sha: stash.sha,
            base_sha: stash.base_sha,
            selector: stash.selector,
            subject: stash.subject,
        }));
    }

    Ok(result)
}

/// Batched metadata for `shas`, split into chunks of `METADATA_BATCH_SIZE` per ARCHITECTURE.md
/// §5.1. Order follows the order of `shas` (git show does not reorder or walk its arguments).
pub async fn commit_metadata(repo: &Path, shas: &[String]) -> Result<Vec<CommitMeta>, GitError> {
    let mut result = Vec::with_capacity(shas.len());
    for chunk in shas.chunks(METADATA_BATCH_SIZE) {
        result.extend(commit_metadata_chunk(repo, chunk).await?);
    }
    Ok(result)
}

async fn commit_metadata_chunk(repo: &Path, shas: &[String]) -> Result<Vec<CommitMeta>, GitError> {
    if shas.is_empty() {
        return Ok(Vec::new());
    }
    let format = format!("--pretty=format:%H{UNIT_SEP}%P{UNIT_SEP}%an{UNIT_SEP}%ae{UNIT_SEP}%at{UNIT_SEP}%s{UNIT_SEP}%b{RECORD_SEP}");
    let mut args: Vec<&str> = vec!["show", "-s", &format];
    args.extend(shas.iter().map(|s| s.as_str()));

    let output = git(repo, &args, GitOpts::default()).await?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_commit_metadata_records(&text))
}

fn parse_commit_metadata_records(text: &str) -> Vec<CommitMeta> {
    text.split(RECORD_SEP)
        .filter_map(|record| {
            let record = record.trim_start_matches('\n');
            if record.trim().is_empty() {
                return None;
            }
            let mut fields = record.splitn(7, UNIT_SEP);
            let sha = fields.next()?.to_string();
            let parents = fields
                .next()?
                .split_ascii_whitespace()
                .map(|s| s.to_string())
                .collect();
            let author_name = fields.next()?.to_string();
            let author_email = fields.next()?.to_string();
            let author_time: i64 = fields.next()?.trim().parse().ok()?;
            let subject = fields.next()?.to_string();
            let body = fields
                .next()
                .unwrap_or("")
                .trim_end_matches('\n')
                .to_string();
            Some(CommitMeta {
                sha,
                parents,
                author_name,
                author_email,
                author_time,
                subject,
                body,
            })
        })
        .collect()
}

/// Stash entries as pseudo-rows attached to their base commit — ARCHITECTURE.md §5.1.
pub async fn stash_list(repo: &Path) -> Result<Vec<StashEntry>, GitError> {
    let format = format!("--pretty=format:%H{UNIT_SEP}%P{UNIT_SEP}%gd{UNIT_SEP}%s{RECORD_SEP}");
    // `stash list` exits 0 with empty stdout when there is no stash ref yet — no error handling
    // needed beyond the usual `?`.
    let output = git(repo, &["stash", "list", &format], GitOpts::default()).await?;

    let text = String::from_utf8_lossy(&output.stdout);
    let entries = text
        .split(RECORD_SEP)
        .filter_map(|record| {
            let record = record.trim_start_matches('\n');
            if record.trim().is_empty() {
                return None;
            }
            let mut fields = record.splitn(4, UNIT_SEP);
            let sha = fields.next()?.to_string();
            let base_sha = fields
                .next()?
                .split_ascii_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            let selector = fields.next()?.to_string();
            let subject = fields
                .next()
                .unwrap_or("")
                .trim_end_matches('\n')
                .to_string();
            Some(StashEntry {
                sha,
                base_sha,
                selector,
                subject,
            })
        })
        .collect();
    Ok(entries)
}

#[tauri::command]
pub async fn get_graph(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<GraphTopologyRow>, AppError> {
    let handle = state.get_repo(&repo_id).ok_or_else(|| {
        AppError::new(
            "Repository is not open",
            format!("unknown repo id {repo_id}"),
        )
    })?;
    Ok(graph_rows(&handle.path).await?)
}

#[tauri::command]
pub async fn get_commit_meta(
    state: State<'_, AppState>,
    repo_id: String,
    shas: Vec<String>,
) -> Result<Vec<CommitMeta>, AppError> {
    let handle = state.get_repo(&repo_id).ok_or_else(|| {
        AppError::new(
            "Repository is not open",
            format!("unknown repo id {repo_id}"),
        )
    })?;
    Ok(commit_metadata(&handle.path, &shas).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_metadata_record() {
        let text = format!(
            "abc123{UNIT_SEP}parent1 parent2{UNIT_SEP}Jane Doe{UNIT_SEP}jane@example.com{UNIT_SEP}1700000000{UNIT_SEP}Fix bug{UNIT_SEP}Body line one\nBody line two{RECORD_SEP}"
        );
        let metas = parse_commit_metadata_records(&text);
        assert_eq!(metas.len(), 1);
        let m = &metas[0];
        assert_eq!(m.sha, "abc123");
        assert_eq!(m.parents, vec!["parent1", "parent2"]);
        assert_eq!(m.author_name, "Jane Doe");
        assert_eq!(m.author_email, "jane@example.com");
        assert_eq!(m.author_time, 1700000000);
        assert_eq!(m.subject, "Fix bug");
        assert_eq!(m.body, "Body line one\nBody line two");
    }

    #[test]
    fn parses_multiple_records_and_root_commit_has_no_parents() {
        let text = format!(
            "sha1{UNIT_SEP}{UNIT_SEP}A{UNIT_SEP}a@x.com{UNIT_SEP}1{UNIT_SEP}first{UNIT_SEP}{RECORD_SEP}\nsha2{UNIT_SEP}sha1{UNIT_SEP}B{UNIT_SEP}b@x.com{UNIT_SEP}2{UNIT_SEP}second{UNIT_SEP}body{RECORD_SEP}"
        );
        let metas = parse_commit_metadata_records(&text);
        assert_eq!(metas.len(), 2);
        assert!(metas[0].parents.is_empty());
        assert_eq!(metas[1].parents, vec!["sha1"]);
    }
}
