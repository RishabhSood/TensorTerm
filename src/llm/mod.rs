pub mod anthropic;
pub mod openai_compat;

use std::future::Future;
use std::pin::Pin;

use reqwest::Client;

use crate::config::LlmConfig;

// ── Chat Message ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
}

// ── Provider Trait ───────────────────────────────────────────

pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    fn chat<'a>(
        &'a self,
        client: &'a Client,
        messages: &'a [ChatMessage],
        max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;
}

// ── Provider Registry ───────────────────────────────────────

pub fn build_providers(config: &LlmConfig) -> Vec<Box<dyn LlmProvider>> {
    let mut providers: Vec<Box<dyn LlmProvider>> = Vec::new();

    // Anthropic
    if let Some(ref cfg) = config.anthropic {
        if cfg.api_key.is_some() {
            providers.push(Box::new(anthropic::AnthropicProvider {
                api_key: cfg.api_key.clone().unwrap(),
                model: cfg.model.clone(),
            }));
        }
    }

    // OpenAI
    if let Some(ref cfg) = config.openai {
        if cfg.api_key.is_some() {
            providers.push(Box::new(openai_compat::OpenAiCompatProvider {
                name: "openai".into(),
                base_url: "https://api.openai.com/v1".into(),
                api_key: cfg.api_key.clone(),
                model: cfg.model.clone(),
            }));
        }
    }

    // OpenAI-compatible endpoints (Ollama, OpenRouter, etc.)
    for entry in &config.openai_compatible {
        providers.push(Box::new(openai_compat::OpenAiCompatProvider {
            name: entry.name.clone(),
            base_url: entry.base_url.clone(),
            api_key: entry.api_key.clone(),
            model: entry.model.clone(),
        }));
    }

    providers
}

// ── Summary Prompts ─────────────────────────────────────────

pub fn summary_system_prompt(mode: &str) -> String {
    match mode {
        "eli5" => "You are an expert science communicator. Explain the following research paper in simple terms that a curious teenager could understand. Use analogies and avoid jargon. Keep it under 200 words.".into(),
        "technical" => "You are a senior ML researcher. Provide a technical deep-dive of this paper for an expert audience. Focus on methodology, architecture choices, training details, and how it advances the field. Keep it under 300 words.".into(),
        "key_findings" => "You are a research analyst. Extract the key findings and contributions of this paper as a concise bullet-point list. Include quantitative results where mentioned. 5-8 bullets max.".into(),
        "research_gaps" => "You are a critical peer reviewer. Identify limitations, open questions, and potential research gaps in this paper. What assumptions might not hold? What follow-up work is needed? Keep it under 200 words.".into(),
        _ => "Summarize this research paper concisely.".into(),
    }
}

pub fn scaffold_system_prompt() -> &'static str {
    "You are a research engineer. Given this ML paper, generate a concise implementation scaffold:\n\n\
     1. Project directory tree (show full structure)\n\
     2. For each key file: one-line description of its purpose and what to implement\n\
     3. requirements.txt with likely dependencies\n\
     4. A brief README.md outline\n\n\
     Do NOT write full code for each file. Instead, describe what each file should contain \
     and what the user needs to implement. Keep it concise and actionable — this is a roadmap, not a codebase.\n\
     Use PyTorch unless the paper specifies otherwise."
}
