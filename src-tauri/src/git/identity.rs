//! Global git identity — DESIGN_SPEC.md §11: "if `user.name`/`user.email` unset, inline form
//! (writes global config) — this is the #1 new-user papercut in every git client." This doesn't
//! need a repo path (it's `--global`), so it's not routed through a `RepoHandle`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::exec::{git, GitError, GitErrorKind, GitOpts};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GitIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
}

impl GitIdentity {
    pub fn is_complete(&self) -> bool {
        self.name.is_some() && self.email.is_some()
    }
}

async fn get_global(key: &str) -> Result<Option<String>, GitError> {
    // `--global` config reads don't need a repo; run from the cwd (any directory works).
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    match git(
        &cwd,
        &["config", "--global", "--get", key],
        GitOpts::default(),
    )
    .await
    {
        Ok(output) => {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(if value.is_empty() { None } else { Some(value) })
        }
        // `config --get` exits 1 (no stderr) when the key is unset.
        Err(e) if e.kind == GitErrorKind::NonZeroExit && e.code == Some(1) => Ok(None),
        Err(e) => Err(e),
    }
}

pub async fn get_identity() -> Result<GitIdentity, GitError> {
    Ok(GitIdentity {
        name: get_global("user.name").await?,
        email: get_global("user.email").await?,
    })
}

pub async fn set_identity(name: &str, email: &str) -> Result<(), GitError> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    git(
        &cwd,
        &["config", "--global", "user.name", name],
        GitOpts::default(),
    )
    .await?;
    git(
        &cwd,
        &["config", "--global", "user.email", email],
        GitOpts::default(),
    )
    .await?;
    Ok(())
}
