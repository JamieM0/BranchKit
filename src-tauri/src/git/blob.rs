//! Raw file content for image-diff rendering — DESIGN_SPEC.md §6.2 "Images: side-by-side
//! before/after with checkerboard background and dimensions". `revision: None` reads the
//! worktree file straight off disk; `Some(rev)` reads it out of git via `git show <rev>:<path>`
//! (`rev` is a commit sha, or `:` for the index/staged blob).

use std::path::Path;

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

use super::exec::{git, GitError, GitErrorKind, GitOpts};
use super::ops::require_repo;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Blob {
    pub base64: String,
}

pub async fn read_blob(repo: &Path, revision: Option<&str>, path: &str) -> Result<Vec<u8>, GitError> {
    match revision {
        None => {
            let full = repo.join(path);
            std::fs::read(&full).map_err(|e| GitError {
                code: None,
                stderr: e.to_string(),
                cmd_summary: format!("read {}", full.display()),
                kind: GitErrorKind::Spawn,
            })
        }
        Some(rev) => {
            // `:path` (no sha before the colon) is git's own syntax for "index stage 0 of path" —
            // `rev` is already `:` in that case, so don't double it up into `::path`.
            let spec = if rev == ":" {
                format!(":{path}")
            } else {
                format!("{rev}:{path}")
            };
            let output = git(repo, &["show", &spec], GitOpts::default()).await?;
            Ok(output.stdout)
        }
    }
}

/// Working-directory/staged/commit/compare blob fetch behind one command — the frontend passes
/// `null` for the worktree side, `":"` for the staged (index) side, or a sha otherwise.
#[tauri::command]
pub async fn get_blob(
    state: State<'_, AppState>,
    repo_id: String,
    revision: Option<String>,
    path: String,
) -> Result<Blob, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let bytes = read_blob(&handle.path, revision.as_deref(), &path).await?;
    Ok(Blob {
        base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}
