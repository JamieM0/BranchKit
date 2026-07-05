//! Commit + undo-commit mutations — DESIGN_SPEC.md §7, ARCHITECTURE.md §7.1.
//!
//! `commit` assembles the message from the composer's summary + optional description and runs
//! `git commit -m <summary> [-m <body>] [--amend]`, passing the message as argv unless the whole
//! message is over 8k chars, in which case it goes over stdin via `-F -` (ARCHITECTURE.md §7.1).
//! It returns the new HEAD sha so the toast can name it ("Committed `a1b2c3d` to `main`", §8).
//!
//! `undo_commit` is that toast's **Undo** — a soft reset of the last commit that returns its
//! changes to the index exactly as they were staged (DESIGN_SPEC.md §8/§15.13). The frontend only
//! offers it until the commit is pushed.

use tauri::{AppHandle, State};

use crate::error::AppError;
use crate::events::{ChangeKind, WatchedKind};
use crate::state::AppState;

use super::exec::{git, git_with_stdin, GitOpts};
use super::ops::{emit_changes, require_repo};

/// Messages at or below this length go on argv (`-m`); longer ones go over stdin (`-F -`) so we
/// never blow the platform argv limit — ARCHITECTURE.md §7.1.
const ARGV_MESSAGE_LIMIT: usize = 8192;

/// Assemble the argv-vs-stdin decision length: summary, plus the body and its blank-line separator
/// when present.
fn message_len(summary: &str, body: &str) -> usize {
    summary.len() + if body.is_empty() { 0 } else { body.len() + 2 }
}

/// Create a commit from the composer fields (DESIGN_SPEC.md §7). `summary` is required; a non-empty
/// `description` becomes the body after a blank line. `amend` replaces the tip commit (`--amend`).
/// Returns the new HEAD sha. Emits Head (topology + refs reload) and Index (the file lists clear).
#[tauri::command]
pub async fn commit(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    summary: String,
    description: String,
    amend: bool,
) -> Result<String, AppError> {
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        return Err(AppError::new(
            "A commit needs a summary",
            "empty commit summary".to_string(),
        ));
    }
    let body = description.trim().to_string();

    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[
        WatchedKind::Head,
        WatchedKind::Index,
        WatchedKind::WorkingTree,
    ]);

    if message_len(&summary, &body) > ARGV_MESSAGE_LIMIT {
        // Too long for argv — feed the whole message over stdin.
        let mut message = summary.clone();
        if !body.is_empty() {
            message.push_str("\n\n");
            message.push_str(&body);
        }
        let mut args = vec!["commit", "-F", "-"];
        if amend {
            args.push("--amend");
        }
        git_with_stdin(&handle.path, &args, GitOpts::default(), message.as_bytes()).await?;
    } else {
        let mut args = vec!["commit", "-m", &summary];
        if !body.is_empty() {
            args.push("-m");
            args.push(&body);
        }
        if amend {
            args.push("--amend");
        }
        git(&handle.path, &args, GitOpts::default()).await?;
    }

    let head = git(&handle.path, &["rev-parse", "HEAD"], GitOpts::default()).await?;
    let sha = String::from_utf8_lossy(&head.stdout).trim().to_string();

    emit_changes(&app, &repo_id, &[ChangeKind::Head, ChangeKind::Index]);
    Ok(sha)
}

/// Undo the last commit — `git reset --soft HEAD~1` — the commit toast's **Undo** (§8/§15.13). A
/// soft reset keeps the working tree and returns the commit's changes to the index, so the staged
/// set is restored exactly. Emits Head + Index.
#[tauri::command]
pub async fn undo_commit(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    handle.begin_self_op(&[
        WatchedKind::Head,
        WatchedKind::Index,
        WatchedKind::WorkingTree,
    ]);
    git(
        &handle.path,
        &["reset", "--soft", "HEAD~1"],
        GitOpts::default(),
    )
    .await?;
    emit_changes(&app, &repo_id, &[ChangeKind::Head, ChangeKind::Index]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    async fn init_repo(dir: &Path) {
        for args in [
            vec!["init", "--initial-branch=main", "-q"],
            vec!["config", "user.name", "T"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            git(dir, &args, GitOpts::default()).await.unwrap();
        }
    }

    async fn head_subject(dir: &Path) -> String {
        let out = git(dir, &["log", "-1", "--pretty=%s"], GitOpts::default())
            .await
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    async fn head_body(dir: &Path) -> String {
        let out = git(dir, &["log", "-1", "--pretty=%b"], GitOpts::default())
            .await
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn argv_message_len_accounts_for_the_blank_line_separator() {
        assert_eq!(message_len("hi", ""), 2);
        assert_eq!(message_len("hi", "body"), 2 + 4 + 2);
    }

    #[tokio::test]
    async fn commits_summary_and_body_then_undo_restores_the_staged_set() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path()).await;
        std::fs::write(dir.path().join("f.txt"), "one\n").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default())
            .await
            .unwrap();
        git(dir.path(), &["commit", "-q", "-m", "seed"], GitOpts::default())
            .await
            .unwrap();

        // Stage a change, then commit it via the bare `git` calls the command wraps.
        std::fs::write(dir.path().join("f.txt"), "two\n").unwrap();
        git(dir.path(), &["add", "-A"], GitOpts::default())
            .await
            .unwrap();
        git(
            dir.path(),
            &["commit", "-m", "change f", "-m", "the body"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        assert_eq!(head_subject(dir.path()).await, "change f");
        assert_eq!(head_body(dir.path()).await, "the body");

        // Soft reset (the Undo path) puts the change back in the index exactly as staged.
        git(
            dir.path(),
            &["reset", "--soft", "HEAD~1"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        assert_eq!(head_subject(dir.path()).await, "seed");
        let staged = git(
            dir.path(),
            &["diff", "--cached", "--name-only"],
            GitOpts::default(),
        )
        .await
        .unwrap();
        assert_eq!(String::from_utf8_lossy(&staged.stdout).trim(), "f.txt");
    }
}
