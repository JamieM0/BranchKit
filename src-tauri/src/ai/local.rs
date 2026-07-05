//! Local AI provider — ARCHITECTURE.md §10: downloads and manages the `llama-server` sidecar and
//! its pinned GGUF model, then speaks the same OpenAI-compatible client as Ollama/Remote-OpenAI
//! once the sidecar is up.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::AppError;

use super::download::{download_resumable, DownloadOutcome};
use super::prompt::ChatMessage;
use super::versions;
use super::{AiState, SidecarHandle};

/// ARCHITECTURE.md §10: "keep-alive 5min idle then kill".
const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const HEALTH_POLL_TIMEOUT: Duration = Duration::from_secs(15);

fn ai_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app.path().app_data_dir()?.join("ai");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn model_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    Ok(ai_dir(app)?.join("models").join(versions::MODEL_FILE_NAME))
}

fn server_root(app: &AppHandle) -> Result<PathBuf, AppError> {
    Ok(ai_dir(app)?.join("llama-server").join(versions::LLAMA_CPP_RELEASE))
}

fn pidfile_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    Ok(ai_dir(app)?.join("llama-server.pid"))
}

fn server_binary_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let asset = versions::current_platform_asset().ok_or_else(|| {
        AppError::new(
            "The local AI model isn't available on this platform",
            format!(
                "no pinned llama-server asset for {}/{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        )
    })?;
    Ok(server_root(app)?.join(asset.binary_relative_path))
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ModelState {
    NotDownloaded,
    Ready,
}

/// Settings → AI's model card state on open (DESIGN_SPEC.md §13) — download progress itself is
/// carried by `ai://local/download-progress` events, not polled here.
#[tauri::command]
pub fn get_local_model_state(app: AppHandle) -> Result<ModelState, AppError> {
    Ok(if model_path(&app)?.exists() {
        ModelState::Ready
    } else {
        ModelState::NotDownloaded
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub phase: String,
    pub percent: Option<u32>,
    pub mbps: f64,
}

fn emit_progress(app: &AppHandle, phase: &str, downloaded: u64, total: Option<u64>, mbps: f64) {
    let percent = total.map(|t| {
        if t == 0 {
            100
        } else {
            ((downloaded as f64 / t as f64) * 100.0).min(100.0) as u32
        }
    });
    let _ = app.emit(
        "ai://local/download-progress",
        DownloadProgress {
            phase: phase.to_string(),
            percent,
            mbps,
        },
    );
}

/// Downloads the pinned `llama-server` binary (if not already extracted) and the pinned GGUF
/// model (if missing), in that order, emitting `ai://local/download-progress` throughout — the
/// model card's Download → progress state (DESIGN_SPEC.md §13). Cancellable via
/// `cancel_local_download`; a cancellation leaves whatever `.part` file exists for a later resume.
#[tauri::command]
pub async fn download_local_model(app: AppHandle, ai_state: State<'_, AiState>) -> Result<(), AppError> {
    let cancel = Arc::new(AtomicBool::new(false));
    *ai_state
        .download_cancel
        .lock()
        .expect("download_cancel mutex poisoned") = Some(cancel.clone());

    let result = download_local_model_inner(&app, cancel).await;
    *ai_state
        .download_cancel
        .lock()
        .expect("download_cancel mutex poisoned") = None;
    result
}

async fn download_local_model_inner(app: &AppHandle, cancel: Arc<AtomicBool>) -> Result<(), AppError> {
    let http = reqwest::Client::new();

    let binary_path = server_binary_path(app)?;
    if !binary_path.exists() {
        let asset = versions::current_platform_asset().ok_or_else(|| {
            AppError::new(
                "The local AI model isn't available on this platform",
                "no pinned llama-server asset for this OS/arch",
            )
        })?;
        let archive_path = ai_dir(app)?.join(asset.asset_name);
        let url = format!(
            "https://github.com/ggml-org/llama.cpp/releases/download/{}/{}",
            versions::LLAMA_CPP_RELEASE,
            asset.asset_name
        );
        let outcome = download_resumable(&http, &url, &archive_path, asset.sha256, cancel.clone(), {
            move |downloaded, total, mbps| emit_progress(app, "Downloading llama-server", downloaded, total, mbps)
        })
        .await?;
        if outcome == DownloadOutcome::Cancelled {
            return Ok(());
        }

        emit_progress(app, "Extracting llama-server", 0, None, 0.0);
        extract_archive(&archive_path, &server_root(app)?)?;
        let _ = std::fs::remove_file(&archive_path);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&binary_path) {
                let mut perms = meta.permissions();
                perms.set_mode(perms.mode() | 0o111);
                let _ = std::fs::set_permissions(&binary_path, perms);
            }
        }
    }

    let model_path = model_path(app)?;
    if !model_path.exists() {
        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }
        let outcome = download_resumable(
            &http,
            versions::MODEL_URL,
            &model_path,
            versions::MODEL_SHA256,
            cancel,
            move |downloaded, total, mbps| emit_progress(app, "Downloading model", downloaded, total, mbps),
        )
        .await?;
        if outcome == DownloadOutcome::Cancelled {
            return Ok(());
        }
    }

    Ok(())
}

/// Extracts a `.tar.gz` (macOS/Linux) or `.zip` (Windows) archive via the system `tar` — bsdtar
/// (shipped on Windows 10 1803+, macOS, and available on every mainstream Linux distro) auto-
/// detects the format, so one code path covers both without a zip-decoding dependency.
fn extract_archive(archive: &Path, dest: &Path) -> Result<(), AppError> {
    std::fs::create_dir_all(dest)?;
    let output = std::process::Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .output()?;
    if !output.status.success() {
        return Err(AppError::new(
            "Could not extract the downloaded llama-server archive",
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(())
}

/// The model card's Cancel button (DESIGN_SPEC.md §13).
#[tauri::command]
pub fn cancel_local_download(ai_state: State<'_, AiState>) {
    if let Some(flag) = ai_state
        .download_cancel
        .lock()
        .expect("download_cancel mutex poisoned")
        .as_ref()
    {
        flag.store(true, Ordering::SeqCst);
    }
}

/// Removes the GGUF model (keeps the extracted `llama-server` binary cached for a later
/// re-download) and kills the sidecar if it's running against it — the model card's
/// "✓ Ready · Remove" (DESIGN_SPEC.md §13).
#[tauri::command]
pub async fn remove_local_model(app: AppHandle, ai_state: State<'_, AiState>) -> Result<(), AppError> {
    kill_sidecar(&ai_state).await;
    let path = model_path(&app)?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

async fn kill_sidecar(ai_state: &AiState) {
    let mut guard = ai_state.sidecar.lock().await;
    if let Some(mut handle) = guard.take() {
        let _ = handle.child.kill().await;
    }
}

/// Picks a free localhost port by binding then immediately releasing it — ARCHITECTURE.md §10.
fn pick_free_port() -> Result<u16, AppError> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

async fn wait_for_health(port: u16) -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(300))
        .build()
        .expect("static reqwest client config is valid");
    let url = format!("http://127.0.0.1:{port}/health");
    let deadline = Instant::now() + HEALTH_POLL_TIMEOUT;
    while Instant::now() < deadline {
        if client.get(&url).send().await.is_ok_and(|r| r.status().is_success()) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    false
}

/// Ensures the sidecar is running (spawning it on first use) and returns its port —
/// ARCHITECTURE.md §10: spawn on first generate, poll `/health`; idle-kill is driven by
/// `last_used`, checked periodically by `run_idle_monitor` (started from `lib.rs`'s `setup()`).
async fn ensure_running(app: &AppHandle, ai_state: &AiState) -> Result<u16, AppError> {
    *ai_state.last_used.lock().expect("last_used mutex poisoned") = Some(Instant::now());

    let mut guard = ai_state.sidecar.lock().await;
    if let Some(handle) = guard.as_ref() {
        return Ok(handle.port);
    }

    let model = model_path(app)?;
    if !model.exists() {
        return Err(AppError::new(
            "The local model hasn't been downloaded yet",
            "model gguf is missing",
        ));
    }
    let binary = server_binary_path(app)?;
    if !binary.exists() {
        return Err(AppError::new(
            "The local llama-server binary hasn't been downloaded yet",
            "llama-server binary is missing",
        ));
    }

    let port = pick_free_port()?;
    let mut command = tokio::process::Command::new(&binary);
    command
        .current_dir(binary.parent().unwrap_or_else(|| Path::new(".")))
        .args([
            "-m",
            model.to_string_lossy().as_ref(),
            "--port",
            &port.to_string(),
            "-c",
            "4096",
            "--host",
            "127.0.0.1",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let child = command
        .spawn()
        .map_err(|e| AppError::new("Could not start the local AI model", e.to_string()))?;

    if let Some(pid) = child.id() {
        let _ = std::fs::write(pidfile_path(app)?, pid.to_string());
    }

    if !wait_for_health(port).await {
        return Err(AppError::new(
            "The local AI model didn't start in time",
            format!("llama-server on port {port} never answered /health"),
        ));
    }

    *guard = Some(SidecarHandle { child, port });
    Ok(port)
}

pub async fn generate(
    app: &AppHandle,
    ai_state: &AiState,
    messages: &[ChatMessage],
    on_token: impl FnMut(&str) + Send,
) -> Result<String, AppError> {
    let port = ensure_running(app, ai_state).await?;
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let client = reqwest::Client::new();
    let result = super::openai_compat::stream_chat(&client, &url, None, "local", messages, on_token).await;
    *ai_state.last_used.lock().expect("last_used mutex poisoned") = Some(Instant::now());
    result
}

/// The idle-kill monitor (ARCHITECTURE.md §10) — spawned once from `lib.rs`'s `setup()`; runs for
/// the lifetime of the app. Re-fetches the managed `AiState` each tick rather than holding a
/// long-lived reference, so it needs nothing more than a cheap `AppHandle` clone to run as a
/// detached 'static task.
pub async fn run_idle_monitor(app: AppHandle) {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let ai_state = app.state::<AiState>();
        let idle = ai_state
            .last_used
            .lock()
            .expect("last_used mutex poisoned")
            .is_some_and(|t| t.elapsed() > IDLE_TIMEOUT);
        if idle {
            kill_sidecar(&ai_state).await;
        }
    }
}

/// Best-effort: kills a pid left behind by a previous run that never got to shut down cleanly
/// (crash, force-quit) — ARCHITECTURE.md §10's "pidfile to reap orphans on next start". Always
/// removes the pidfile regardless of whether the kill succeeded (a stale pid is harmless once
/// reaped; leaving the file behind would just re-attempt the same kill forever).
pub fn reap_orphan(app: &AppHandle) {
    let Ok(path) = pidfile_path(app) else { return };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    if let Ok(pid) = text.trim().parse::<u32>() {
        kill_pid_best_effort(pid);
    }
    let _ = std::fs::remove_file(&path);
}

#[cfg(unix)]
fn kill_pid_best_effort(pid: u32) {
    // SAFETY: signaling a pid with SIGKILL is always safe to attempt; a missing/reused pid just
    // fails harmlessly (ESRCH), which is fine here — see `reap_orphan`'s doc comment.
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

#[cfg(windows)]
fn kill_pid_best_effort(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();
}

/// Kills the running sidecar synchronously — called from `lib.rs`'s `RunEvent::Exit`, where an
/// async context isn't available. `start_kill` is the sync half of `kill()`; we don't wait for the
/// exit since the app is already tearing down.
pub fn shutdown_sidecar_blocking(app: &AppHandle, ai_state: &AiState) {
    if let Ok(mut guard) = ai_state.sidecar.try_lock() {
        if let Some(mut handle) = guard.take() {
            let _ = handle.child.start_kill();
        }
    }
    if let Ok(path) = pidfile_path(app) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_state_serializes_camel_case() {
        let json = serde_json::to_string(&ModelState::NotDownloaded).unwrap();
        assert_eq!(json, "\"notDownloaded\"");
    }
}
