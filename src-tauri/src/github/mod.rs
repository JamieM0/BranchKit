//! GitHub integration — ARCHITECTURE.md §11, DESIGN_SPEC.md §12. This file owns the OAuth device
//! flow and the signed-in/out state; `api.rs` owns the REST calls once signed in (PRs, CI checks,
//! create/merge, PR-head checkout).

pub mod api;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::credentials;
use crate::error::AppError;

/// GitHub OAuth App client id — device flow needs no client secret, so shipping this in source is
/// the normal, documented pattern (ARCHITECTURE.md §11).
///
/// SPEC-DEVIATION / ACTION NEEDED: Jamie must register an OAuth App at
/// github.com → Settings → Developer settings → OAuth Apps, enable "Device Flow" on it, and paste
/// its client id here before sign-in will work. Left as an obvious placeholder rather than a
/// blank string so a build with it unset fails loudly (`start_device_flow` below) instead of
/// silently hitting GitHub with an invalid id.
pub const GITHUB_OAUTH_CLIENT_ID: &str = "REPLACE_WITH_YOUR_GITHUB_OAUTH_APP_CLIENT_ID";

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("BranchKit")
        .build()
        .expect("static reqwest client config is valid")
}

fn client_id_configured() -> Result<&'static str, AppError> {
    if GITHUB_OAUTH_CLIENT_ID.starts_with("REPLACE_WITH_") {
        return Err(AppError::new(
            "BranchKit's GitHub OAuth app isn't configured yet",
            "GITHUB_OAUTH_CLIENT_ID is still the placeholder in github/mod.rs",
        ));
    }
    Ok(GITHUB_OAUTH_CLIENT_ID)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeApiResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u32,
    interval: u32,
}

#[derive(Debug, Deserialize)]
struct AccessTokenApiResponse {
    access_token: Option<String>,
    error: Option<String>,
    interval: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GithubUser {
    pub login: String,
    pub avatar_url: String,
}

/// Step 1 of the device flow (DESIGN_SPEC.md §12): request the huge user code to show, copy, and
/// send the user to `github.com/login/device` to enter.
#[tauri::command]
pub async fn start_device_flow() -> Result<DeviceCode, AppError> {
    let client_id = client_id_configured()?;
    let resp = client()
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", "repo")])
        .send()
        .await?
        .error_for_status()?
        .json::<DeviceCodeApiResponse>()
        .await
        .map_err(|e| AppError::new("GitHub sent back something BranchKit didn't understand", e.to_string()))?;

    Ok(DeviceCode {
        device_code: resp.device_code,
        user_code: resp.user_code,
        verification_uri: resp.verification_uri,
        expires_in: resp.expires_in,
        interval: resp.interval,
    })
}

/// Step 2: poll quietly at (at least) `interval` seconds until the user finishes entering the code
/// on GitHub, then store the token in the keychain and cache the (non-secret) profile for the
/// signed-in UI (DESIGN_SPEC.md §12's "success state with avatar + username").
#[tauri::command]
pub async fn poll_device_flow(
    app: AppHandle,
    device_code: String,
    interval: u32,
    expires_in: u32,
) -> Result<GithubUser, AppError> {
    let client_id = client_id_configured()?;
    let http = client();
    let mut interval = Duration::from_secs(interval.max(1) as u64);
    let deadline = std::time::Instant::now() + Duration::from_secs(expires_in as u64);

    loop {
        tokio::time::sleep(interval).await;
        if std::time::Instant::now() > deadline {
            return Err(AppError::new(
                "That sign-in code expired — try again",
                "device code expired",
            ));
        }

        let resp = http
            .post(ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?
            .json::<AccessTokenApiResponse>()
            .await
            .map_err(|e| AppError::new("GitHub sent back something BranchKit didn't understand", e.to_string()))?;

        if let Some(token) = resp.access_token {
            credentials::set_secret(credentials::GITHUB_ACCOUNT, &token)?;
            let user = api::fetch_current_user(&token).await?;
            save_cached_user(&app, Some(&user));
            return Ok(user);
        }

        match resp.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval = Duration::from_secs(resp.interval.unwrap_or(5).max(1) as u64);
            }
            Some("expired_token") => {
                return Err(AppError::new("That sign-in code expired — try again", "expired_token"));
            }
            Some("access_denied") => {
                return Err(AppError::new("Sign-in was cancelled", "access_denied"));
            }
            other => {
                return Err(AppError::new(
                    "GitHub sign-in failed",
                    other.unwrap_or("unknown device flow error").to_string(),
                ));
            }
        }
    }
}

fn cached_user_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    use tauri::Manager;
    let dir = app.path().app_config_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("github_user.json"))
}

fn save_cached_user(app: &AppHandle, user: Option<&GithubUser>) {
    let Some(path) = cached_user_path(app) else { return };
    match user {
        Some(u) => {
            if let Ok(text) = serde_json::to_string(u) {
                let _ = std::fs::write(path, text);
            }
        }
        None => {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Whether BranchKit is currently signed in, and as whom — read from the local cache only (no
/// network call), so the PULL REQUESTS section and CI dots know instantly whether to render at
/// all (DESIGN_SPEC.md §12: "degrade invisibly when not connected").
#[tauri::command]
pub fn get_github_connection(app: AppHandle) -> Option<GithubUser> {
    credentials::get_secret(credentials::GITHUB_ACCOUNT)?;
    let path = cached_user_path(&app)?;
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Sign out = revoke locally only (DESIGN_SPEC.md §12) — drop the keychain token and the cached
/// profile; BranchKit never calls GitHub's token-revocation endpoint since that requires the OAuth
/// app's client secret, which a device-flow desktop app deliberately doesn't have.
#[tauri::command]
pub fn github_sign_out(app: AppHandle) {
    credentials::delete_secret(credentials::GITHUB_ACCOUNT);
    save_cached_user(&app, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_client_id_is_rejected() {
        assert!(client_id_configured().is_err());
    }
}
