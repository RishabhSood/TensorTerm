pub mod arxiv;
pub mod arxiv_html;
pub mod hf_papers;
pub mod hf_search;
pub mod huggingface;
pub mod semantic_scholar;
pub mod social;

use reqwest::Client;
use std::time::Duration;

pub fn build_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("tensorterm/0.1 (research-dashboard; +https://github.com/tensorterm)")
        .build()
        .expect("failed to build reqwest client")
}

/// Client with longer timeout for LLM calls (scaffold/summary generation).
pub fn build_llm_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent("tensorterm/0.1 (research-dashboard)")
        .build()
        .expect("failed to build LLM reqwest client")
}
