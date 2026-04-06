use reqwest::Client;

use crate::app::PaperEntry;

pub async fn fetch_papers(
    client: &Client,
    categories: &[String],
    max_results: usize,
) -> Result<Vec<PaperEntry>, String> {
    let cat_query = categories
        .iter()
        .map(|c| format!("cat:{}", c))
        .collect::<Vec<_>>()
        .join("+OR+");

    let url = format!(
        "http://export.arxiv.org/api/query?search_query={}&sortBy=submittedDate&sortOrder=descending&max_results={}",
        cat_query, max_results
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("ArXiv request failed: {}", e))?;

    let body = resp
        .text()
        .await
        .map_err(|e| format!("ArXiv body read failed: {}", e))?;

    parse_atom_feed(&body)
}

fn parse_atom_feed(xml: &str) -> Result<Vec<PaperEntry>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("XML parse error: {}", e))?;

    const ATOM_NS: &str = "http://www.w3.org/2005/Atom";
    const ARXIV_NS: &str = "http://arxiv.org/schemas/atom";

    let mut papers = Vec::new();

    for entry in doc.root_element().children().filter(|n| {
        n.is_element() && n.has_tag_name((ATOM_NS, "entry"))
    }) {
        let title = entry
            .children()
            .find(|n| n.has_tag_name((ATOM_NS, "title")))
            .and_then(|n| n.text())
            .unwrap_or("")
            .replace('\n', " ")
            .trim()
            .to_string();

        // Skip the feed-level boilerplate entry
        if title.is_empty() {
            continue;
        }

        let summary = entry
            .children()
            .find(|n| n.has_tag_name((ATOM_NS, "summary")))
            .and_then(|n| n.text())
            .unwrap_or("")
            .replace('\n', " ")
            .trim()
            .to_string();

        let authors: Vec<String> = entry
            .children()
            .filter(|n| n.has_tag_name((ATOM_NS, "author")))
            .filter_map(|a| {
                a.children()
                    .find(|n| n.has_tag_name((ATOM_NS, "name")))
                    .and_then(|n| n.text())
                    .map(|s| s.to_string())
            })
            .collect();

        let authors_str = if authors.len() > 3 {
            format!("{} et al.", authors[..3].join(", "))
        } else {
            authors.join(", ")
        };

        let published = entry
            .children()
            .find(|n| n.has_tag_name((ATOM_NS, "published")))
            .and_then(|n| n.text())
            .unwrap_or("")
            .chars()
            .take(10)
            .collect::<String>();

        // Extract arxiv_id from the <id> URL, e.g. "http://arxiv.org/abs/2604.01234v1"
        let raw_id = entry
            .children()
            .find(|n| n.has_tag_name((ATOM_NS, "id")))
            .and_then(|n| n.text())
            .unwrap_or("");
        let arxiv_id = raw_id
            .rsplit('/')
            .next()
            .unwrap_or("")
            .split('v')
            .next()
            .unwrap_or("")
            .to_string();

        let primary_category = entry
            .children()
            .find(|n| n.has_tag_name((ARXIV_NS, "primary_category")))
            .and_then(|n| n.attribute("term"))
            .unwrap_or("unknown")
            .to_string();

        let pdf_url = entry
            .children()
            .find(|n| {
                n.has_tag_name((ATOM_NS, "link"))
                    && n.attribute("title") == Some("pdf")
            })
            .and_then(|n| n.attribute("href"))
            .map(|s| s.to_string());

        papers.push(PaperEntry {
            title,
            authors: authors_str,
            date: published,
            domain: primary_category,
            arxiv_id: Some(arxiv_id),
            abstract_text: if summary.is_empty() { None } else { Some(summary) },
            pdf_url,
        });
    }

    Ok(papers)
}
