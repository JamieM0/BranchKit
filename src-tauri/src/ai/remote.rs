//! Remote API provider — ARCHITECTURE.md §10: OpenAI format (base URL + `/chat/completions`,
//! `Authorization: Bearer`) or Anthropic format (`/v1/messages`, `x-api-key`), picked by
//! `AiSettings::remote_format`. The API key itself never touches `settings.json` — it lives in the
//! OS keychain under `credentials::AI_API_KEY_ACCOUNT` (ARCHITECTURE.md §8).

use serde::Serialize;

use crate::credentials;
use crate::error::AppError;
use crate::settings::{AiSettings, RemoteApiFormat};

use super::prompt::ChatMessage;

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .build()
        .expect("static reqwest client config is valid")
}

/// Streams a completion from whichever remote format is configured — the ✨ commit-message flow's
/// Remote branch (DESIGN_SPEC.md §7).
pub async fn generate(
    settings: &AiSettings,
    messages: &[ChatMessage],
    on_token: impl FnMut(&str) + Send,
) -> Result<String, AppError> {
    // Defensively re-trim on read too, not just on save (`set_remote_api_key`) — a key saved
    // before that trim existed could still have trailing whitespace sitting in the keychain,
    // which silently breaks the `Authorization` header and reads back from the provider as a
    // plain 401 with no other clue.
    let api_key = credentials::get_secret(credentials::AI_API_KEY_ACCOUNT)
        .map(|k| zeroize::Zeroizing::new(k.trim().to_string()));
    // Both remote formats need a key — without this guard a missing key silently sends an
    // unauthenticated request and the provider's generic 401 reads as a bad key when really
    // there's no key stored at all (see the Settings AI Test button investigation).
    let key = api_key.ok_or_else(|| {
        AppError::new(
            "No API key is stored for the remote provider",
            "ai-api-key keychain entry is missing",
        )
    })?;
    match settings.remote_format {
        RemoteApiFormat::OpenAi => {
            let url = format!("{}/chat/completions", settings.remote_base_url.trim_end_matches('/'));
            super::openai_compat::stream_chat(&client(), &url, Some(&key), &settings.remote_model, messages, on_token).await
        }
        RemoteApiFormat::Anthropic => {
            super::anthropic::stream_chat(
                &client(),
                &settings.remote_base_url,
                &key,
                &settings.remote_model,
                messages,
                on_token,
            )
            .await
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub ok: bool,
    pub message: String,
}

/// Settings → AI → Remote API's **Test** button: a minimal real request against the configured
/// endpoint, reporting inline ✓/✗ (DESIGN_SPEC.md §13). Takes `settings` fresh from the frontend
/// (rather than re-reading persisted settings) so Test reflects fields the user hasn't saved a
/// change-event for yet — every input already persists instantly, but this keeps the two decoupled.
#[tauri::command]
pub async fn test_remote_connection(settings: AiSettings) -> TestResult {
    let messages = vec![ChatMessage::user("Reply with exactly: OK")];
    match generate(&settings, &messages, |_| {}).await {
        Ok(_) => TestResult {
            ok: true,
            message: "Connected".to_string(),
        },
        Err(e) => TestResult {
            ok: false,
            message: e.user_message,
        },
    }
}

// ---- API key storage (ARCHITECTURE.md §8: keychain only, never settings.json) ----

#[tauri::command]
pub fn set_remote_api_key(key: String) -> Result<(), AppError> {
    // Trim whitespace/newlines a paste can leave behind — untrimmed, this silently breaks the
    // `Authorization: Bearer <key>` header and the provider just reports a plain 401.
    Ok(credentials::set_secret(credentials::AI_API_KEY_ACCOUNT, key.trim())?)
}

#[tauri::command]
pub fn remove_remote_api_key() {
    credentials::delete_secret(credentials::AI_API_KEY_ACCOUNT);
}

/// Whether a key is stored — never the key itself (ARCHITECTURE.md §8's "frontend never sees
/// them"). Drives the masked `••••` placeholder vs. empty-field state in Settings.
#[tauri::command]
pub fn remote_api_key_configured() -> bool {
    credentials::get_secret(credentials::AI_API_KEY_ACCOUNT).is_some()
}
