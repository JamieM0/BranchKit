//! AI layer — ARCHITECTURE.md §10, DESIGN_SPEC.md §7/§13. One internal shape (a system+user
//! message pair in, a raw token stream out), three providers behind it: the bundled `llama-server`
//! sidecar (`local`), Ollama, and a Remote API (OpenAI- or Anthropic-shaped). This module owns the
//! provider dispatch and the ✨ commit-message command; each provider's own file owns everything
//! specific to reaching it.

pub mod anthropic;
pub mod download;
pub mod local;
pub mod ollama;
pub mod openai_compat;
pub mod prompt;
pub mod remote;
pub mod versions;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::{AppHandle, Emitter, State};

use crate::error::AppError;
use crate::git::exec::{git, GitOpts};
use crate::git::ops::require_repo;
use crate::settings::{self, AiProviderKind};
use crate::state::AppState;

use prompt::{
    build_commit_explanation_prompt, build_commit_prompt, parse_commit_message,
    GeneratedCommitExplanation, GeneratedCommitMessage,
};

/// A running local sidecar — ARCHITECTURE.md §10. Killed on idle timeout (`local::run_idle_monitor`),
/// on `remove_local_model`, and on app exit (`lib.rs`'s `RunEvent::Exit`).
pub struct SidecarHandle {
    pub child: tokio::process::Child,
    pub port: u16,
}

/// Global AI state — one sidecar and one in-flight download for the whole app (the local model and
/// its process are shared across every open repo, not per-repo).
#[derive(Default)]
pub struct AiState {
    pub sidecar: tokio::sync::Mutex<Option<SidecarHandle>>,
    pub last_used: Mutex<Option<Instant>>,
    pub download_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

/// The ✨ commit-message flow's IPC entry point (DESIGN_SPEC.md §7, ARCHITECTURE.md §10). Builds
/// the prompt from the requested diff scope, dispatches to whichever provider is configured,
/// streaming raw tokens to the frontend via `ai://commit/token` as they arrive, then returns the
/// fully parsed `{summary, description}` once generation completes.
#[tauri::command]
pub async fn generate_commit_message(
    app: AppHandle,
    ai_state: State<'_, AiState>,
    repo_state: State<'_, AppState>,
    repo_id: String,
    staged: bool,
) -> Result<GeneratedCommitMessage, AppError> {
    let handle = require_repo(&repo_state, &repo_id)?;
    let app_settings = settings::get_settings(app.clone())?;
    if !app_settings.ai.enabled {
        return Err(AppError::new(
            "AI commit messages aren't enabled",
            "AiSettings::enabled is false",
        ));
    }

    let mut stat_args = vec!["diff", "--stat"];
    let mut diff_args = vec!["diff", "--no-color", "-U3"];
    if staged {
        stat_args.push("--cached");
        diff_args.push("--cached");
    }

    let stat = git(&handle.path, &stat_args, GitOpts::default()).await?;
    let diff = git(&handle.path, &diff_args, GitOpts::default()).await?;
    let stat_text = String::from_utf8_lossy(&stat.stdout).into_owned();
    let diff_text = String::from_utf8_lossy(&diff.stdout).into_owned();

    let messages = build_commit_prompt(
        app_settings.ai.style.clone(),
        &stat_text,
        &diff_text,
        app_settings.ai.max_diff_size_kb,
    );

    let token_app = app.clone();
    let on_token = move |token: &str| {
        let _ = token_app.emit("ai://commit/token", token);
    };

    let full_text = match app_settings.ai.provider.clone() {
        AiProviderKind::Local => local::generate(&app, &ai_state, &messages, on_token).await?,
        AiProviderKind::Ollama => ollama::generate(&app_settings.ai, &messages, on_token).await?,
        AiProviderKind::Remote => remote::generate(&app_settings.ai, &messages, on_token).await?,
    };

    Ok(parse_commit_message(&full_text))
}

/// Explains one complete historical commit with exactly one provider request. The full `git show`
/// output is deliberately passed through untouched so AI never silently explains only a truncated
/// portion of the commit.
#[tauri::command]
pub async fn explain_commit(
    app: AppHandle,
    ai_state: State<'_, AiState>,
    repo_state: State<'_, AppState>,
    repo_id: String,
    sha: String,
) -> Result<GeneratedCommitExplanation, AppError> {
    let handle = require_repo(&repo_state, &repo_id)?;
    let app_settings = settings::get_settings(app.clone())?;
    if !app_settings.ai.enabled {
        return Err(AppError::new(
            "AI isn't enabled",
            "AiSettings::enabled is false",
        ));
    }

    let show = git(
        &handle.path,
        &[
            "show",
            "--no-color",
            "--format=fuller",
            "--binary",
            "--find-renames",
            "--find-copies",
            "--no-ext-diff",
            &sha,
        ],
        GitOpts::default(),
    )
    .await?;
    let commit_show = String::from_utf8_lossy(&show.stdout).into_owned();
    let messages = build_commit_explanation_prompt(&commit_show);

    let token_app = app.clone();
    let on_token = move |token: &str| {
        let _ = token_app.emit("ai://explanation/token", token);
    };
    let markdown = match app_settings.ai.provider.clone() {
        AiProviderKind::Local => local::generate(&app, &ai_state, &messages, on_token).await?,
        AiProviderKind::Ollama => ollama::generate(&app_settings.ai, &messages, on_token).await?,
        AiProviderKind::Remote => remote::generate(&app_settings.ai, &messages, on_token).await?,
    };

    Ok(GeneratedCommitExplanation { markdown })
}
