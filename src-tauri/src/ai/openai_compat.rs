//! OpenAI-compatible `POST /v1/chat/completions` streaming client — ARCHITECTURE.md §10: shared
//! verbatim by the local `llama-server` sidecar, Ollama, and the Remote-API OpenAI format, since
//! all three speak the identical SSE chat-completions shape.

use serde_json::Value;

use crate::error::AppError;

use super::prompt::ChatMessage;

/// Streams a chat completion from `url` (the full `.../chat/completions` endpoint), calling
/// `on_token` with each incremental text delta as it arrives. Returns the full concatenated text.
pub async fn stream_chat(
    client: &reqwest::Client,
    url: &str,
    api_key: Option<&str>,
    model: &str,
    messages: &[ChatMessage],
    mut on_token: impl FnMut(&str) + Send,
) -> Result<String, AppError> {
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });

    let mut request = client.post(url).json(&body);
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {key}"));
    }

    let mut response = request
        .send()
        .await
        .map_err(|e| AppError::new("Could not reach the AI provider", e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AppError::new(
            format!("The AI provider returned an error ({status})"),
            text,
        ));
    }

    let mut buf = String::new();
    let mut full = String::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| AppError::new("Lost connection to the AI provider", e.to_string()))?
    {
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim_end_matches('\r').to_string();
            buf.drain(..=pos);
            if let Some(token) = parse_sse_data_line(&line) {
                on_token(&token);
                full.push_str(&token);
            }
        }
    }
    Ok(full)
}

/// Parses one `data: {...}` SSE line from an OpenAI-compatible stream into its token text, if
/// any (`data: [DONE]` and non-`data:` lines yield `None`).
fn parse_sse_data_line(line: &str) -> Option<String> {
    let data = line
        .strip_prefix("data: ")
        .or_else(|| line.strip_prefix("data:"))?;
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    let json: Value = serde_json::from_str(data).ok()?;
    json["choices"][0]["delta"]["content"]
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_token_from_a_delta_line() {
        let line = r#"data: {"id":"1","choices":[{"delta":{"content":"Hello"}}]}"#;
        assert_eq!(parse_sse_data_line(line).as_deref(), Some("Hello"));
    }

    #[test]
    fn ignores_the_done_sentinel() {
        assert_eq!(parse_sse_data_line("data: [DONE]"), None);
    }

    #[test]
    fn ignores_lines_without_a_content_delta() {
        let line = r#"data: {"id":"1","choices":[{"delta":{"role":"assistant"}}]}"#;
        assert_eq!(parse_sse_data_line(line), None);
    }

    #[test]
    fn ignores_non_data_lines() {
        assert_eq!(parse_sse_data_line("event: ping"), None);
        assert_eq!(parse_sse_data_line(""), None);
    }
}
