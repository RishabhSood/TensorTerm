use reqwest::Client;

/// Fetch the full paper text from ArXiv's HTML rendering.
/// Returns None if the paper doesn't have an HTML version (older papers).
pub async fn fetch_full_text(client: &Client, arxiv_id: &str) -> Result<Option<String>, String> {
    // ArXiv HTML rendering URL
    let url = format!("https://arxiv.org/html/{}", arxiv_id);

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("ArXiv HTML fetch failed: {}", e))?;

    let status = resp.status().as_u16();
    if status == 404 {
        return Ok(None); // No HTML version available
    }
    if !resp.status().is_success() {
        return Err(format!("ArXiv HTML returned {}", status));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| format!("ArXiv HTML body read failed: {}", e))?;

    let text = extract_paper_content(&html);
    if text.len() < 200 {
        return Ok(None); // Too short, probably not a real paper page
    }

    Ok(Some(text))
}

/// Extract the main paper content from ArXiv HTML.
/// ArXiv uses LaTeXML which puts content in <article> or <div class="ltx_page_content">.
fn extract_paper_content(html: &str) -> String {
    // Try to find the main article content between markers
    let content = if let Some(start) = html.find("<article") {
        if let Some(end) = html[start..].find("</article>") {
            &html[start..start + end + 10]
        } else {
            html
        }
    } else if let Some(start) = html.find("ltx_page_content") {
        // Find the opening tag
        let tag_start = html[..start].rfind('<').unwrap_or(0);
        html.get(tag_start..).unwrap_or(html)
    } else {
        // Fall back to body
        if let Some(start) = html.find("<body") {
            if let Some(end) = html[start..].find("</body>") {
                &html[start..start + end]
            } else {
                html
            }
        } else {
            html
        }
    };

    // Strip HTML tags and clean up
    let mut text = strip_html_to_markdown(content);

    // Remove excessive blank lines
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }

    text.trim().to_string()
}

/// Convert HTML to readable text, preserving some structure.
fn strip_html_to_markdown(html: &str) -> String {
    let mut output = String::with_capacity(html.len() / 3);
    let mut in_tag = false;
    let mut tag_name = String::new();
    let mut collecting_tag = false;
    let mut skip_content = false;

    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                collecting_tag = true;
                tag_name.clear();
            }
            '>' => {
                in_tag = false;
                collecting_tag = false;
                let tag_lower = tag_name.to_lowercase();

                // Skip script/style/nav content
                if tag_lower.starts_with("script") || tag_lower.starts_with("style")
                    || tag_lower.starts_with("nav") || tag_lower.starts_with("footer")
                    || tag_lower.starts_with("header")
                {
                    skip_content = true;
                }
                if tag_lower.starts_with("/script") || tag_lower.starts_with("/style")
                    || tag_lower.starts_with("/nav") || tag_lower.starts_with("/footer")
                    || tag_lower.starts_with("/header")
                {
                    skip_content = false;
                }

                // Add structure markers
                if !skip_content {
                    if tag_lower.starts_with("h1") || tag_lower.starts_with("h2")
                        || tag_lower.starts_with("h3")
                    {
                        output.push_str("\n\n## ");
                    } else if tag_lower.starts_with("/h1") || tag_lower.starts_with("/h2")
                        || tag_lower.starts_with("/h3")
                    {
                        output.push('\n');
                    } else if tag_lower == "p" || tag_lower == "br" || tag_lower == "br/"
                        || tag_lower.starts_with("/p")
                        || tag_lower.starts_with("div")
                        || tag_lower.starts_with("/div")
                    {
                        output.push_str("\n\n");
                    } else if tag_lower.starts_with("li") {
                        output.push_str("\n- ");
                    }
                }
            }
            _ if in_tag => {
                if collecting_tag && (ch.is_alphanumeric() || ch == '/') {
                    tag_name.push(ch);
                } else {
                    collecting_tag = false;
                }
            }
            _ if !skip_content => {
                output.push(ch);
            }
            _ => {}
        }
    }

    // Decode common HTML entities
    output
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
