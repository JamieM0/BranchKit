//! Ollama provider — ARCHITECTURE.md §10: `GET /api/tags` for the model dropdown, generation via
//! its OpenAI-compatible `/v1/chat/completions` (identical client code to Remote-OpenAI).

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::settings::AiSettings;

use super::prompt::ChatMessage;

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .expect("static reqwest client config is valid")
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
struct TagModel {
    name: String,
}

/// The Settings → AI → Ollama model dropdown's data source (DESIGN_SPEC.md §13).
#[tauri::command]
pub async fn list_ollama_models(base_url: String) -> Result<Vec<String>, AppError> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let resp = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::new("Could not reach Ollama", e.to_string()))?
        .error_for_status()
        .map_err(|e| AppError::new("Ollama returned an error", e.to_string()))?
        .json::<TagsResponse>()
        .await
        .map_err(|e| {
            AppError::new(
                "Ollama sent back something BranchKit didn't understand",
                e.to_string(),
            )
        })?;
    Ok(resp.models.into_iter().map(|m| m.name).collect())
}

/// The Ollama URL field's connection dot — a 2s-timeout ping (DESIGN_SPEC.md §13).
#[tauri::command]
pub async fn ping_ollama(base_url: String) -> bool {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    client()
        .get(&url)
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub ok: bool,
    pub message: String,
}

pub async fn generate(
    settings: &AiSettings,
    messages: &[ChatMessage],
    on_token: impl FnMut(&str) + Send,
) -> Result<String, AppError> {
    let model = settings.ollama_model.clone().ok_or_else(|| {
        AppError::new(
            "No Ollama model selected",
            "ollama_model is unset in AiSettings",
        )
    })?;
    let url = format!(
        "{}/v1/chat/completions",
        settings.ollama_base_url.trim_end_matches('/')
    );
    let http = reqwest::Client::builder()
        .build()
        .expect("static reqwest client config is valid");
    super::openai_compat::stream_chat(&http, &url, None, &model, messages, on_token).await
}
