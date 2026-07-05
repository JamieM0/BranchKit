//! Anthropic Messages API adapter — ARCHITECTURE.md §10: same `AiProvider`-shaped output as
//! `openai_compat`, but Anthropic's request/response shape differs (`system` is a top-level
//! field, not a message; auth is `x-api-key` + `anthropic-version`; streaming events are typed
//! `content_block_delta`s rather than an OpenAI-style `delta.content`).

use serde_json::Value;

use crate::error::AppError;

use super::prompt::ChatMessage;

const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 1024;

/// Streams a Messages API completion from `base_url` + `/v1/messages`. `messages` is expected to
/// contain exactly the `{system, user}` pair `prompt::build_commit_prompt` produces — the system
/// message is lifted out to the top-level `system` field per Anthropic's shape.
pub async fn stream_chat(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: &[ChatMessage],
    mut on_token: impl FnMut(&str) + Send,
) -> Result<String, AppError> {
    let system: String = messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let turns: Vec<Value> = messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
        .collect();

    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "system": system,
        "messages": turns,
        "stream": true,
    });

    let mut response = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .json(&body)
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

/// Parses one `data: {...}` line from an Anthropic Messages stream into its token text, if the
/// event is a `content_block_delta` with a `text_delta`.
fn parse_sse_data_line(line: &str) -> Option<String> {
    let data = line.strip_prefix("data: ").or_else(|| line.strip_prefix("data:"))?;
    let data = data.trim();
    if data.is_empty() {
        return None;
    }
    let json: Value = serde_json::from_str(data).ok()?;
    if json["type"].as_str() != Some("content_block_delta") {
        return None;
    }
    json["delta"]["text"].as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_token_from_a_content_block_delta() {
        let line = r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#;
        assert_eq!(parse_sse_data_line(line).as_deref(), Some("Hi"));
    }

    #[test]
    fn ignores_other_event_types() {
        let line = r#"data: {"type":"message_start","message":{}}"#;
        assert_eq!(parse_sse_data_line(line), None);
    }

    #[test]
    fn ignores_non_data_lines() {
        assert_eq!(parse_sse_data_line("event: content_block_delta"), None);
    }
}
