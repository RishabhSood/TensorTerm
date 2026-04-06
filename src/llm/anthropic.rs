use std::future::Future;
use std::pin::Pin;

use reqwest::Client;
use serde_json::{json, Value};

use super::{ChatMessage, LlmProvider};

pub struct AnthropicProvider {
    pub api_key: String,
    pub model: String,
}

impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn chat<'a>(
        &'a self,
        client: &'a Client,
        messages: &'a [ChatMessage],
        max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            let system_text: String = messages
                .iter()
                .filter(|m| m.role == "system")
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");

            let api_messages: Vec<Value> = messages
                .iter()
                .filter(|m| m.role != "system")
                .map(|m| json!({ "role": m.role, "content": m.content }))
                .collect();

            let mut body = json!({
                "model": self.model,
                "max_tokens": max_tokens,
                "messages": api_messages,
            });

            if !system_text.is_empty() {
                body["system"] = json!(system_text);
            }

            let resp = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Anthropic request failed: {}", e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(format!("Anthropic {}: {}", status, &body[..body.len().min(200)]));
            }

            let data: Value = resp
                .json()
                .await
                .map_err(|e| format!("Anthropic parse failed: {}", e))?;

            data.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|block| block.get("text"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| "Anthropic: no text in response".into())
        })
    }
}
