use reqwest::Client;

use crate::config::SocialFeedConfig;

#[derive(Debug, Clone)]
pub struct SocialPost {
    pub source_name: String,
    pub source_type: SourceType,
    pub title: Option<String>,
    pub content: String,
    pub url: String,
    pub published: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceType {
    Twitter,
    Blog,
}

/// Aggregate fetch with no progress reporting. Kept for callers that don't
/// need per-source events (the worker uses `fetch_one` directly for the splash).
#[allow(dead_code)]
pub async fn fetch_social_feeds(
    client: &Client,
    feeds: &[SocialFeedConfig],
    nitter_instance: &str,
) -> Result<Vec<SocialPost>, String> {
    let mut all_posts = Vec::new();
    for feed in feeds {
        if let Ok(posts) = fetch_one(client, feed, nitter_instance).await {
            all_posts.extend(posts);
        }
    }
    all_posts.sort_by(|a, b| b.published.cmp(&a.published));
    Ok(all_posts)
}

/// Fetch a single configured social feed. Used for per-source progress reporting.
pub async fn fetch_one(
    client: &Client,
    feed: &SocialFeedConfig,
    nitter_instance: &str,
) -> Result<Vec<SocialPost>, String> {
    let (url, source_type) = parse_source(&feed.source, nitter_instance);
    fetch_single_feed(client, &url, &feed.name, source_type, &feed.keywords, nitter_instance).await
}

fn parse_source(source: &str, nitter_instance: &str) -> (String, SourceType) {
    if let Some(handle) = source.strip_prefix("twitter:") {
        let url = format!("{}/{}/rss", nitter_instance.trim_end_matches('/'), handle);
        (url, SourceType::Twitter)
    } else if let Some(url) = source.strip_prefix("rss:") {
        (url.to_string(), SourceType::Blog)
    } else {
        // Assume it's a direct URL
        (source.to_string(), SourceType::Blog)
    }
}

/// Rewrite nitter URLs to x.com so links open on the real site.
fn rewrite_nitter_url(url: &str, nitter_instance: &str) -> String {
    let host = nitter_instance
        .trim_end_matches('/')
        .strip_prefix("https://")
        .or_else(|| nitter_instance.strip_prefix("http://"))
        .unwrap_or(nitter_instance);
    if url.contains(host) {
        url.replace(host, "x.com")
    } else {
        url.to_string()
    }
}

async fn fetch_single_feed(
    client: &Client,
    fetch_url: &str,
    source_name: &str,
    source_type: SourceType,
    keywords: &[String],
    nitter_instance: &str,
) -> Result<Vec<SocialPost>, String> {
    let resp = client
        .get(fetch_url)
        .send()
        .await
        .map_err(|e| format!("Social fetch failed for {}: {}", source_name, e))?;

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

    let mut posts = if root_name == "feed" {
        parse_atom(root, source_name, &source_type)
    } else if root_name == "rss" || root_name == "RDF" {
        parse_rss(root, source_name, &source_type)
    } else {
        return Err(format!("{}: unknown root element <{}>", source_name, root_name));
    };

    // Rewrite nitter URLs to x.com for Twitter sources
    if source_type == SourceType::Twitter {
        for post in &mut posts {
            post.url = rewrite_nitter_url(&post.url, nitter_instance);
        }
    }

    // Apply keyword filter
    if keywords.is_empty() {
        return Ok(posts);
    }

    let filtered = posts
        .into_iter()
        .filter(|post| {
            let text = format!(
                "{} {}",
                post.title.as_deref().unwrap_or(""),
                post.content
            )
            .to_lowercase();
            keywords.iter().any(|kw| text.contains(&kw.to_lowercase()))
        })
        .collect();

    Ok(filtered)
}

fn parse_atom(root: roxmltree::Node, source_name: &str, source_type: &SourceType) -> Vec<SocialPost> {
    const ATOM_NS: &str = "http://www.w3.org/2005/Atom";
    let mut posts = Vec::new();

    for entry in root.children().filter(|n| n.is_element() && n.has_tag_name((ATOM_NS, "entry"))) {
        let title = entry
            .children()
            .find(|n| n.has_tag_name((ATOM_NS, "title")))
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string());

        let content = entry
            .children()
            .find(|n| n.has_tag_name((ATOM_NS, "content")) || n.has_tag_name((ATOM_NS, "summary")))
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();

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

        if content.is_empty() && title.is_none() {
            continue;
        }

        posts.push(SocialPost {
            source_name: source_name.to_string(),
            source_type: source_type.clone(),
            title,
            content,
            url,
            published,
        });
    }

    posts
}

fn parse_rss(root: roxmltree::Node, source_name: &str, source_type: &SourceType) -> Vec<SocialPost> {
    let mut posts = Vec::new();

    // RSS 2.0: <rss><channel><item>...
    // Find <channel> first, then iterate <item>
    let channel = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "channel")
        .unwrap_or(root);

    for item in channel.children().filter(|n| n.is_element() && n.tag_name().name() == "item") {
        let title = item
            .children()
            .find(|n| n.tag_name().name() == "title")
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string());

        let content = item
            .children()
            .find(|n| {
                let name = n.tag_name().name();
                name == "description" || name == "encoded"
            })
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string();

        // Strip HTML tags from content (basic)
        let content = strip_html(&content);

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
            .take(25) // RSS dates like "Sat, 05 Apr 2025 12:00:00"
            .collect::<String>();

        if content.is_empty() && title.is_none() {
            continue;
        }

        posts.push(SocialPost {
            source_name: source_name.to_string(),
            source_type: source_type.clone(),
            title,
            content,
            url,
            published,
        });
    }

    posts
}

/// Basic HTML tag stripping for RSS content.
fn strip_html(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    // Collapse whitespace
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}
