use reqwest::Client;
use serde_json::Value;

/// Rich metadata from the HuggingFace Papers API.
#[derive(Debug, Clone)]
pub struct HfPaperMeta {
    pub repo_url: Option<String>,
    pub repo_stars: Option<u32>,
    pub upvotes: u32,
    pub ai_summary: Option<String>,
    pub ai_keywords: Vec<String>,
    pub num_comments: u32,
    pub project_page: Option<String>,
    pub submitted_by: Option<String>,
    pub published_at: Option<String>,
}

pub async fn fetch_paper_meta(client: &Client, arxiv_id: &str) -> Result<HfPaperMeta, String> {
    let url = format!("https://huggingface.co/api/papers/{}", arxiv_id);

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HF papers request failed: {}", e))?;

    let status = resp.status().as_u16();
    if status == 404 {
        return Ok(HfPaperMeta {
            repo_url: None,
            repo_stars: None,
            upvotes: 0,
            ai_summary: None,
            ai_keywords: Vec::new(),
            num_comments: 0,
            project_page: None,
            submitted_by: None,
            published_at: None,
        });
    }

    if !resp.status().is_success() {
        return Err(format!("HF papers returned {}", status));
    }

    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("HF papers parse failed: {}", e))?;

    let repo_url = body
        .get("githubRepo")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let repo_stars = body
        .get("githubStars")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);

    let upvotes = body
        .get("upvotes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let ai_summary = body
        .get("ai_summary")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let ai_keywords = body
        .get("ai_keywords")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Discussion comments — count from discussionId presence
    // (actual count requires separate API call, but we note it exists)
    let num_comments = body
        .get("numComments")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let project_page = body
        .get("projectPage")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let submitted_by = body
        .get("submittedOnDailyBy")
        .and_then(|v| v.get("fullname").or_else(|| v.get("user")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let published_at = body
        .get("publishedAt")
        .and_then(|v| v.as_str())
        .map(|s| s.chars().take(10).collect());

    Ok(HfPaperMeta {
        repo_url,
        repo_stars,
        upvotes,
        ai_summary,
        ai_keywords,
        num_comments,
        project_page,
        submitted_by,
        published_at,
    })
}
