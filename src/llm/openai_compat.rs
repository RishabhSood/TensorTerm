use std::future::Future;
use std::pin::Pin;

use reqwest::Client;
use serde_json::{json, Value};

use super::{ChatMessage, LlmProvider};

pub struct OpenAiCompatProvider {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
}

impl LlmProvider for OpenAiCompatProvider {
    fn name(&self) -> &str {
        &self.name
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
            let api_messages: Vec<Value> = messages
                .iter()
                .map(|m| json!({ "role": m.role, "content": m.content }))
                .collect();

            let body = json!({
                "model": self.model,
                "max_tokens": max_tokens,
                "messages": api_messages,
            });

            let url = format!(
                "{}/chat/completions",
                self.base_url.trim_end_matches('/')
            );

            let mut req = client
                .post(&url)
                .header("content-type", "application/json")
                .json(&body);

            if let Some(ref key) = self.api_key {
                req = req.header("authorization", format!("Bearer {}", key));
            }

            let resp = req
                .send()
                .await
                .map_err(|e| format!("{} request failed: {}", self.name, e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(format!("{} {}: {}", self.name, status, &body[..body.len().min(200)]));
            }

            let data: Value = resp
                .json()
                .await
                .map_err(|e| format!("{} parse failed: {}", self.name, e))?;

            data.get("choices")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|choice| choice.get("message"))
                .and_then(|msg| msg.get("content"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| format!("{}: no content in response", self.name))
        })
    }
}
