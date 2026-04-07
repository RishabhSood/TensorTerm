use reqwest::Client;
use serde_json::Value;

use crate::app::PaperEntry;

pub async fn search_papers(client: &Client, query: &str) -> Result<Vec<PaperEntry>, String> {
    let url = format!(
        "https://huggingface.co/api/papers/search?q={}",
        query.replace(' ', "+")
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HF search failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HF search returned {}", resp.status()));
    }

    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("HF search parse failed: {}", e))?;

    let results = body.as_array().ok_or("Expected array response")?;

    let papers: Vec<PaperEntry> = results
        .iter()
        .filter_map(|item| {
            let paper = item.get("paper")?;
            let id = paper.get("id")?.as_str()?;
            let title = paper
                .get("title")
                .or_else(|| item.get("title"))
                .and_then(|v| v.as_str())?
                .replace('\n', " ")
                .trim()
                .to_string();

            let summary = paper
                .get("summary")
                .or_else(|| item.get("summary"))
                .and_then(|v| v.as_str())
                .map(|s| s.replace('\n', " ").trim().to_string());

            let authors = paper
                .get("authors")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    let names: Vec<String> = arr
                        .iter()
                        .filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                        .collect();
                    if names.len() > 3 {
                        format!("{} et al.", names[..3].join(", "))
                    } else {
                        names.join(", ")
                    }
                })
                .unwrap_or_default();

            let date = paper
                .get("publishedAt")
                .and_then(|v| v.as_str())
                .map(|s| s.chars().take(10).collect::<String>())
                .unwrap_or_default();

            // Use ai_keywords as domain hint, or default
            let domain = paper
                .get("ai_keywords")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .unwrap_or("search")
                .to_string();

            Some(PaperEntry {
                title,
                authors,
                date,
                domain,
                arxiv_id: Some(id.to_string()),
                abstract_text: summary,
                pdf_url: Some(format!("https://arxiv.org/pdf/{}", id)),
            })
        })
        .collect();

    Ok(papers)
}
