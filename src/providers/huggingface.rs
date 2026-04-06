use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct HfSpotlight {
    pub title: String,
    pub summary: String,
    pub authors: String,
    pub upvotes: u32,
    pub arxiv_id: String,
}

#[derive(Debug, Deserialize)]
struct DailyPaper {
    #[serde(default)]
    title: String,
    #[serde(default)]
    paper: Option<PaperInfo>,
    #[serde(default)]
    upvotes: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PaperInfo {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    authors: Option<Vec<AuthorInfo>>,
    #[serde(default)]
    upvotes: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AuthorInfo {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, rename = "user")]
    _user: Option<serde_json::Value>,
}

pub async fn fetch_spotlight(client: &Client) -> Result<HfSpotlight, String> {
    let url = "https://huggingface.co/api/daily_papers";

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("HF request failed: {}", e))?;

    let papers: Vec<DailyPaper> = resp
        .json()
        .await
        .map_err(|e| format!("HF JSON parse failed: {}", e))?;

    let best = papers
        .iter()
        .max_by_key(|p| {
            p.upvotes
                .or_else(|| p.paper.as_ref().and_then(|pp| pp.upvotes))
                .unwrap_or(0)
        })
        .ok_or_else(|| "No daily papers returned".to_string())?;

    let paper_info = best.paper.as_ref();

    let title = paper_info
        .and_then(|p| p.title.clone())
        .unwrap_or_else(|| best.title.clone());

    let summary = paper_info
        .and_then(|p| p.summary.clone())
        .unwrap_or_default();

    let authors = paper_info
        .and_then(|p| p.authors.as_ref())
        .map(|auths| {
            let names: Vec<String> = auths
                .iter()
                .filter_map(|a| a.name.clone())
                .collect();
            if names.len() > 3 {
                format!("{} et al.", names[..3].join(", "))
            } else {
                names.join(", ")
            }
        })
        .unwrap_or_default();

    let upvotes = best
        .upvotes
        .or_else(|| paper_info.and_then(|p| p.upvotes))
        .unwrap_or(0);

    let arxiv_id = paper_info
        .and_then(|p| p.id.clone())
        .unwrap_or_default();

    Ok(HfSpotlight {
        title,
        summary,
        authors,
        upvotes,
        arxiv_id,
    })
}
