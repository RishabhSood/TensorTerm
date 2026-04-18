use std::path::{Path, PathBuf};

use crate::app::{PaperEntry, PaperMeta};
use crate::config::ObsidianConfig;
use crate::providers::news::NewsArticle;

const KB_FOLDER: &str = "tensor_term_kb";
const NEWS_FOLDER: &str = "tensor_term_kb/news";

/// Check if a paper has already been exported (by arxiv_id prefix).
pub fn paper_exists(arxiv_id: &str, config: &ObsidianConfig) -> bool {
    if config.vault_path.is_empty() {
        return false;
    }
    let expanded = if config.vault_path.starts_with('~') {
        dirs::home_dir()
            .unwrap_or_default()
            .join(config.vault_path.strip_prefix("~/").unwrap_or(&config.vault_path[1..]))
    } else {
        PathBuf::from(&config.vault_path)
    };
    let kb_dir = expanded.join(KB_FOLDER);
    find_existing_note(&kb_dir, arxiv_id).is_some()
}

pub enum ExportResult {
    Created(PathBuf),
    AlreadyExists(PathBuf),
    Updated(PathBuf),
}

pub fn export_paper(
    paper: &PaperEntry,
    meta: Option<&PaperMeta>,
    summary: Option<&str>,
    scaffold: Option<&str>,
    full_text: Option<&str>,
    config: &ObsidianConfig,
    force: bool,
) -> Result<ExportResult, String> {
    if config.vault_path.is_empty() {
        return Err("Obsidian vault_path not configured. Set it in config.toml".into());
    }

    // Expand ~ to home directory
    let expanded = if config.vault_path.starts_with('~') {
        dirs::home_dir()
            .unwrap_or_default()
            .join(config.vault_path.strip_prefix("~/").unwrap_or(&config.vault_path[1..]))
    } else {
        PathBuf::from(&config.vault_path)
    };
    let vault = expanded.as_path();
    if !vault.exists() {
        return Err(format!("Vault path does not exist: {}", config.vault_path));
    }

    let kb_dir = vault.join(KB_FOLDER);
    std::fs::create_dir_all(&kb_dir)
        .map_err(|e| format!("Failed to create {}: {}", kb_dir.display(), e))?;

    let arxiv_id = paper.arxiv_id.as_deref().unwrap_or("unknown");
    let filename = generate_filename(arxiv_id, &paper.title);
    let filepath = kb_dir.join(&filename);

    // Check for existing file with same arxiv_id prefix
    if !force {
        if let Some(existing) = find_existing_note(&kb_dir, arxiv_id) {
            return Ok(ExportResult::AlreadyExists(existing));
        }
    }

    let content = generate_markdown(paper, meta, summary, scaffold, full_text);
    std::fs::write(&filepath, content)
        .map_err(|e| format!("Failed to write {}: {}", filepath.display(), e))?;

    if force {
        Ok(ExportResult::Updated(filepath))
    } else {
        Ok(ExportResult::Created(filepath))
    }
}

fn find_existing_note(dir: &Path, arxiv_id: &str) -> Option<PathBuf> {
    let prefix = format!("{}_", arxiv_id);
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) && name.ends_with(".md") {
            return Some(entry.path());
        }
    }
    None
}

fn generate_filename(arxiv_id: &str, title: &str) -> String {
    let sanitized: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");
    let truncated = if sanitized.len() > 60 {
        &sanitized[..60]
    } else {
        &sanitized
    };
    format!("{}_{}.md", arxiv_id, truncated.trim_end_matches('-'))
}

fn generate_markdown(
    paper: &PaperEntry,
    meta: Option<&PaperMeta>,
    summary: Option<&str>,
    scaffold: Option<&str>,
    full_text: Option<&str>,
) -> String {
    let arxiv_id = paper.arxiv_id.as_deref().unwrap_or("unknown");
    let authors_yaml = paper
        .authors
        .split(", ")
        .map(|a| format!("  - \"{}\"", a.trim()))
        .collect::<Vec<_>>()
        .join("\n");

    let mut tags = Vec::new();
    if let Some(m) = meta {
        for kw in &m.ai_keywords {
            let tag = kw.to_lowercase().replace(' ', "-");
            tags.push(format!("  - {}", tag));
        }
    }
    let tags_yaml = if tags.is_empty() {
        "  - research".to_string()
    } else {
        tags.join("\n")
    };

    let repo_line = meta
        .and_then(|m| m.repo_url.as_deref())
        .unwrap_or("");

    let today = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = (secs / 86400) as i64;
        let (y, m, d) = crate::app::days_to_ymd(days);
        format!("{:04}-{:02}-{:02}", y, m, d)
    };

    let default_pdf = format!("https://arxiv.org/pdf/{}", arxiv_id);
    let pdf_url = paper
        .pdf_url
        .as_deref()
        .unwrap_or(&default_pdf);

    let mut md = format!(
        r#"---
title: "{title}"
authors:
{authors_yaml}
date: {date}
domain: {domain}
arxiv_id: "{arxiv_id}"
arxiv_url: "https://arxiv.org/abs/{arxiv_id}"
pdf_url: "{pdf_url}"
citations: {citations}
influential_citations: {influential}
hf_upvotes: {upvotes}
repo: "{repo}"
tags:
{tags_yaml}
exported: {today}
---

# {title}

**Authors**: {authors}
**Date**: {date} | **Domain**: {domain}
**ArXiv**: [{arxiv_id}](https://arxiv.org/abs/{arxiv_id}) | **PDF**: [link]({pdf_url})
"#,
        title = paper.title,
        authors_yaml = authors_yaml,
        date = paper.date,
        domain = paper.domain,
        arxiv_id = arxiv_id,
        pdf_url = pdf_url,
        citations = meta.map_or(0, |m| m.citation_count),
        influential = meta.map_or(0, |m| m.influential_count),
        upvotes = meta.map_or(0, |m| m.upvotes),
        repo = repo_line,
        tags_yaml = tags_yaml,
        today = today,
        authors = paper.authors,
    );

    // Abstract
    if let Some(ref abs) = paper.abstract_text {
        md.push_str("\n## Abstract\n\n");
        md.push_str(abs);
        md.push('\n');
    }

    // Full Paper Text (from ArXiv HTML)
    if let Some(text) = full_text {
        md.push_str("\n## Full Paper\n\n");
        md.push_str(text);
        md.push('\n');
    }

    // AI Summary (from LLM or HF)
    if let Some(sum) = summary {
        md.push_str("\n## AI Summary\n\n");
        md.push_str(sum);
        md.push('\n');
    } else if let Some(m) = meta {
        if let Some(ref ai) = m.ai_summary {
            md.push_str("\n## AI Summary (HuggingFace)\n\n");
            md.push_str(ai);
            md.push('\n');
        }
    }

    // Keywords
    if let Some(m) = meta {
        if !m.ai_keywords.is_empty() {
            md.push_str("\n## Keywords\n\n");
            for kw in &m.ai_keywords {
                md.push_str(&format!("- {}\n", kw));
            }
        }
    }

    // Citation Metrics
    if let Some(m) = meta {
        md.push_str("\n## Citation Metrics\n\n");
        if m.s2_found {
            md.push_str(&format!(
                "- **Total Citations**: {}\n- **Influential Citations**: {}\n",
                m.citation_count, m.influential_count
            ));
        } else {
            md.push_str("- *Not yet indexed by Semantic Scholar*\n");
        }
        if m.upvotes > 0 {
            md.push_str(&format!("- **HF Upvotes**: {}\n", m.upvotes));
        }

        // Top Citing Papers
        if !m.top_citations.is_empty() {
            md.push_str("\n### Top Citing Papers\n\n");
            for (i, citing) in m.top_citations.iter().enumerate() {
                if citing.citation_count > 0 {
                    md.push_str(&format!(
                        "{}. **{}** (cited by {})\n",
                        i + 1, citing.title, citing.citation_count
                    ));
                } else {
                    md.push_str(&format!("{}. {}\n", i + 1, citing.title));
                }
            }
        }
    }

    // Implementation / Repository
    if let Some(m) = meta {
        if let Some(ref url) = m.repo_url {
            if !url.is_empty() {
                md.push_str("\n## Implementation\n\n");
                md.push_str(&format!("- **Repository**: [{}]({})\n", url, url));
                if let Some(stars) = m.repo_stars {
                    md.push_str(&format!("- **Stars**: {}\n", stars));
                }
            }
        }
    }

    // Scaffold
    if let Some(scaf) = scaffold {
        md.push_str("\n## Implementation Scaffold\n\n```python\n");
        md.push_str(scaf);
        md.push_str("\n```\n");
    }

    md.push_str("\n## Notes\n\n<!-- Your research notes here -->\n");

    md
}

// ── News export ──────────────────────────────────────────────

pub fn news_article_exists(article: &NewsArticle, config: &ObsidianConfig) -> bool {
    if config.vault_path.is_empty() {
        return false;
    }
    let expanded = expand_vault(&config.vault_path);
    let news_dir = expanded.join(NEWS_FOLDER);
    let filename = generate_news_filename(article);
    news_dir.join(filename).exists()
}

pub fn export_news_article(
    article: &NewsArticle,
    body_markdown: &str,
    config: &ObsidianConfig,
    force: bool,
) -> Result<ExportResult, String> {
    if config.vault_path.is_empty() {
        return Err("Obsidian vault_path not configured. Set it in config.toml".into());
    }

    let expanded = expand_vault(&config.vault_path);
    if !expanded.exists() {
        return Err(format!("Vault path does not exist: {}", config.vault_path));
    }

    let news_dir = expanded.join(NEWS_FOLDER);
    std::fs::create_dir_all(&news_dir)
        .map_err(|e| format!("Failed to create {}: {}", news_dir.display(), e))?;

    let filename = generate_news_filename(article);
    let filepath = news_dir.join(&filename);

    if !force && filepath.exists() {
        return Ok(ExportResult::AlreadyExists(filepath));
    }

    let content = generate_news_markdown(article, body_markdown);
    std::fs::write(&filepath, content)
        .map_err(|e| format!("Failed to write {}: {}", filepath.display(), e))?;

    if force {
        Ok(ExportResult::Updated(filepath))
    } else {
        Ok(ExportResult::Created(filepath))
    }
}

fn expand_vault(vault_path: &str) -> PathBuf {
    if vault_path.starts_with('~') {
        dirs::home_dir()
            .unwrap_or_default()
            .join(vault_path.strip_prefix("~/").unwrap_or(&vault_path[1..]))
    } else {
        PathBuf::from(vault_path)
    }
}

fn slugify(input: &str, max_len: usize) -> String {
    let s: String = input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");
    if s.len() > max_len {
        s[..max_len].trim_end_matches('-').to_string()
    } else {
        s
    }
}

fn generate_news_filename(article: &NewsArticle) -> String {
    let date_prefix = article
        .published
        .chars()
        .take(10)
        .collect::<String>();
    let safe_date = if date_prefix.contains('-') && date_prefix.len() == 10 {
        date_prefix
    } else {
        // RFC 2822 dates need different prefix; use sanitized first 10 chars
        article.published.chars().take(10).collect::<String>().replace(' ', "-").replace(',', "")
    };
    let source_slug = slugify(&article.source_name, 24);
    let title_slug = slugify(&article.title, 60);
    format!("{}_{}_{}.md", safe_date, source_slug, title_slug)
}

fn generate_news_markdown(article: &NewsArticle, body: &str) -> String {
    let today = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = (secs / 86400) as i64;
        let (y, m, d) = crate::app::days_to_ymd(days);
        format!("{:04}-{:02}-{:02}", y, m, d)
    };

    let title_escaped = article.title.replace('"', "\\\"");
    let source_escaped = article.source_name.replace('"', "\\\"");

    format!(
        r#"---
title: "{title}"
source: "{source}"
url: "{url}"
published: "{published}"
exported: {today}
tags:
  - news
  - {source_tag}
---

# {title}

**Source**: {source}  |  **Published**: {published}
**URL**: [{url}]({url})

---

{body}
"#,
        title = title_escaped,
        source = source_escaped,
        url = article.url,
        published = article.published,
        today = today,
        source_tag = slugify(&article.source_name, 32),
        body = body.trim(),
    )
}
