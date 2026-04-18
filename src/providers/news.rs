use reqwest::Client;

use crate::config::NewsFeedConfig;

#[derive(Debug, Clone)]
pub struct NewsArticle {
    pub source_name: String,
    pub title: String,
    pub summary: String,
    pub url: String,
    pub published: String,
}

/// Aggregate fetch with no progress reporting. Kept for callers that don't
/// need per-source events (the worker uses `fetch_one` directly for the splash).
#[allow(dead_code)]
pub async fn fetch_news_feeds(
    client: &Client,
    feeds: &[NewsFeedConfig],
) -> Result<Vec<NewsArticle>, String> {
    let mut all_articles = Vec::new();
    for feed in feeds {
        if let Ok(articles) = fetch_one(client, feed).await {
            all_articles.extend(articles);
        }
    }
    all_articles.sort_by(|a, b| b.published.cmp(&a.published));
    Ok(all_articles)
}

/// Fetch a single configured news feed. Used for per-source progress reporting.
pub async fn fetch_one(
    client: &Client,
    feed: &NewsFeedConfig,
) -> Result<Vec<NewsArticle>, String> {
    fetch_single_feed(client, &feed.url, &feed.name, &feed.keywords).await
}

async fn fetch_single_feed(
    client: &Client,
    fetch_url: &str,
    source_name: &str,
    keywords: &[String],
) -> Result<Vec<NewsArticle>, String> {
    let resp = client
        .get(fetch_url)
        .send()
        .await
        .map_err(|e| format!("News fetch failed for {}: {}", source_name, e))?;

    if !resp.status().is_success() {
        return Err(format!("{} returned {}", source_name, resp.status()));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Body read failed for {}: {}", source_name, e))?;

    let trimmed = body.trim();
    if trimmed.is_empty() || !trimmed.starts_with('<') {
        return Err(format!("{}: not XML", source_name));
    }

    let doc = roxmltree::Document::parse(trimmed)
        .map_err(|e| format!("{}: XML parse error: {}", source_name, e))?;

    let root = doc.root_element();
    let root_name = root.tag_name().name();

    let articles = if root_name == "feed" {
        parse_atom(root, source_name)
    } else if root_name == "rss" || root_name == "RDF" {
        parse_rss(root, source_name)
    } else {
        return Err(format!("{}: unknown root <{}>", source_name, root_name));
    };

    if keywords.is_empty() {
        return Ok(articles);
    }

    let filtered = articles
        .into_iter()
        .filter(|a| {
            let text = format!("{} {}", a.title, a.summary).to_lowercase();
            keywords.iter().any(|kw| text.contains(&kw.to_lowercase()))
        })
        .collect();

    Ok(filtered)
}

fn parse_rss(root: roxmltree::Node, source_name: &str) -> Vec<NewsArticle> {
    let mut articles = Vec::new();

    let channel = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "channel")
        .unwrap_or(root);

    for item in channel.children().filter(|n| n.is_element() && n.tag_name().name() == "item") {
        let title = item
            .children()
            .find(|n| n.tag_name().name() == "title")
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();

        let summary_raw = item
            .children()
            .find(|n| {
                let name = n.tag_name().name();
                name == "description" || name == "encoded"
            })
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();

        let summary = html_to_markdown(&summary_raw);

        let url = item
            .children()
            .find(|n| n.tag_name().name() == "link")
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();

        let published = item
            .children()
            .find(|n| {
                let name = n.tag_name().name();
                name == "pubDate" || name == "date"
            })
            .and_then(|n| n.text())
            .unwrap_or("")
            .chars()
            .take(31) // RFC 2822: "Sat, 05 Apr 2025 12:00:00 +0000"
            .collect::<String>();

        if title.is_empty() && summary.is_empty() {
            continue;
        }

        articles.push(NewsArticle {
            source_name: source_name.to_string(),
            title,
            summary,
            url,
            published,
        });
    }

    articles
}

fn parse_atom(root: roxmltree::Node, source_name: &str) -> Vec<NewsArticle> {
    const ATOM_NS: &str = "http://www.w3.org/2005/Atom";
    let mut articles = Vec::new();

    for entry in root.children().filter(|n| n.is_element() && n.has_tag_name((ATOM_NS, "entry"))) {
        let title = entry
            .children()
            .find(|n| n.has_tag_name((ATOM_NS, "title")))
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();

        let summary_raw = entry
            .children()
            .find(|n| n.has_tag_name((ATOM_NS, "content")) || n.has_tag_name((ATOM_NS, "summary")))
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();

        let summary = html_to_markdown(&summary_raw);

        let url = entry
            .children()
            .find(|n| {
                n.has_tag_name((ATOM_NS, "link"))
                    && n.attribute("rel").unwrap_or("alternate") == "alternate"
            })
            .and_then(|n| n.attribute("href"))
            .unwrap_or("")
            .to_string();

        let published = entry
            .children()
            .find(|n| n.has_tag_name((ATOM_NS, "published")) || n.has_tag_name((ATOM_NS, "updated")))
            .and_then(|n| n.text())
            .unwrap_or("")
            .chars()
            .take(10)
            .collect::<String>();

        if title.is_empty() && summary.is_empty() {
            continue;
        }

        articles.push(NewsArticle {
            source_name: source_name.to_string(),
            title,
            summary,
            url,
            published,
        });
    }

    articles
}

fn html_to_markdown(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    // Quick path: if there are no HTML tags at all, return as-is
    if !input.contains('<') {
        return input.trim().to_string();
    }
    let md = html2md::parse_html(input);
    // Collapse excessive blank lines
    let mut out = md;
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.trim().to_string()
}

/// Fetch the full HTML page at `url` and convert the main article content to markdown.
pub async fn fetch_article_markdown(client: &Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("News article fetch failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("News article returned {}", resp.status()));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| format!("News article body read failed: {}", e))?;

    let body = extract_main_content(&html);
    let body = strip_noise(&body);
    let md = html_to_markdown(&body);

    if md.len() < 80 {
        return Err("Article extraction produced too little content".into());
    }

    Ok(md)
}

/// Pull the most likely main-content region out of a full HTML page.
/// Tries <article>, then <main>, then falls back to <body>.
fn extract_main_content(html: &str) -> String {
    if let Some(s) = slice_between_tag(html, "article") {
        return s;
    }
    if let Some(s) = slice_between_tag(html, "main") {
        return s;
    }
    if let Some(s) = slice_between_tag(html, "body") {
        return s;
    }
    html.to_string()
}

fn slice_between_tag(html: &str, tag: &str) -> Option<String> {
    let open_marker = format!("<{}", tag);
    let close_marker = format!("</{}>", tag);
    let lower = html.to_ascii_lowercase();
    let start = lower.find(&open_marker)?;
    // Skip past the opening tag's '>'
    let after_open = html[start..].find('>')? + start + 1;
    let end_rel = lower[after_open..].find(&close_marker)?;
    Some(html[after_open..after_open + end_rel].to_string())
}

/// Remove script, style, nav, header, footer, aside blocks before HTML→markdown.
fn strip_noise(html: &str) -> String {
    let mut out = html.to_string();
    for tag in &["script", "style", "nav", "header", "footer", "aside", "form", "noscript"] {
        out = strip_block_tag(&out, tag);
    }
    out
}

fn strip_block_tag(html: &str, tag: &str) -> String {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let lower = html.to_ascii_lowercase();
    let mut result = String::with_capacity(html.len());
    let mut cursor = 0;
    while cursor < html.len() {
        let remaining_lower = &lower[cursor..];
        match remaining_lower.find(&open) {
            Some(rel_start) => {
                let abs_start = cursor + rel_start;
                result.push_str(&html[cursor..abs_start]);
                // Find end of this opening tag
                let after_open = match html[abs_start..].find('>') {
                    Some(p) => abs_start + p + 1,
                    None => break,
                };
                // Find closing tag
                match lower[after_open..].find(&close) {
                    Some(end_rel) => {
                        let close_end = after_open + end_rel + close.len();
                        cursor = close_end;
                    }
                    None => {
                        cursor = after_open;
                    }
                }
            }
            None => {
                result.push_str(&html[cursor..]);
                break;
            }
        }
    }
    result
}
