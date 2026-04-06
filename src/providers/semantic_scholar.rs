use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct S2Result {
    pub citation_count: u32,
    pub influential_count: u32,
    pub top_citations: Vec<CitingPaper>,
    /// true if S2 actually found the paper; false if 404 (not indexed yet)
    pub found: bool,
}

#[derive(Debug, Clone)]
pub struct CitingPaper {
    pub title: String,
    pub citation_count: u32,
}

#[derive(Debug, Deserialize)]
struct PaperDetail {
    #[serde(default, rename = "citationCount")]
    citation_count: Option<u32>,
    #[serde(default, rename = "influentialCitationCount")]
    influential_citation_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CitationsResponse {
    #[serde(default)]
    data: Vec<CitationEntry>,
}

#[derive(Debug, Deserialize)]
struct CitationEntry {
    #[serde(default, rename = "citingPaper")]
    citing_paper: Option<CitingPaperRaw>,
}

#[derive(Debug, Deserialize)]
struct CitingPaperRaw {
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "citationCount")]
    citation_count: Option<u32>,
}

const S2_BASE: &str = "https://api.semanticscholar.org/graph/v1/paper";

pub async fn fetch_paper_meta(client: &Client, arxiv_id: &str) -> Result<S2Result, String> {
    // 1. Get paper detail
    let detail_url = format!(
        "{}/ArXiv:{}?fields=citationCount,influentialCitationCount",
        S2_BASE, arxiv_id
    );

    let resp = client
        .get(&detail_url)
        .send()
        .await
        .map_err(|e| format!("S2 request failed: {}", e))?;

    let status = resp.status().as_u16();

    // 404 = paper not indexed by S2 yet (common for brand-new papers)
    if status == 404 {
        return Ok(S2Result {
            citation_count: 0,
            influential_count: 0,
            top_citations: Vec::new(),
            found: false,
        });
    }

    // 429 = rate limited
    if status == 429 {
        return Err("S2 rate limited (429), try again later".into());
    }

    if !resp.status().is_success() {
        return Err(format!("S2 returned {}", status));
    }

    let detail: PaperDetail = resp
        .json()
        .await
        .map_err(|e| format!("S2 detail parse failed: {}", e))?;

    // 2. Get citations (skip if paper has 0 citations to save a request)
    let citation_count = detail.citation_count.unwrap_or(0);
    let influential_count = detail.influential_citation_count.unwrap_or(0);

    let top_citations = if citation_count > 0 {
        fetch_top_citations(client, arxiv_id).await.unwrap_or_default()
    } else {
        Vec::new()
    };

    Ok(S2Result {
        citation_count,
        influential_count,
        top_citations,
        found: true,
    })
}

async fn fetch_top_citations(client: &Client, arxiv_id: &str) -> Result<Vec<CitingPaper>, String> {
    let cit_url = format!(
        "{}/ArXiv:{}/citations?fields=title,citationCount&limit=10",
        S2_BASE, arxiv_id
    );

    let resp = client
        .get(&cit_url)
        .send()
        .await
        .map_err(|e| format!("S2 citations request failed: {}", e))?;

    if !resp.status().is_success() {
        return Ok(Vec::new());
    }

    let cit_resp: CitationsResponse = resp
        .json()
        .await
        .map_err(|e| format!("S2 citations parse failed: {}", e))?;

    let mut citing: Vec<CitingPaper> = cit_resp
        .data
        .into_iter()
        .filter_map(|entry| {
            let raw = entry.citing_paper?;
            Some(CitingPaper {
                title: raw.title.unwrap_or_default(),
                citation_count: raw.citation_count.unwrap_or(0),
            })
        })
        .collect();

    citing.sort_by(|a, b| b.citation_count.cmp(&a.citation_count));
    citing.truncate(5);

    Ok(citing)
}
