use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::{ActivePane, App, FeedMode, MetaFetchStatus, SummaryMode};
use crate::providers::social::SourceType;
use crate::ui::{Theme, SPINNER};
use super::pane_block;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let is_active = app.active_pane == ActivePane::Article;
    let title = match app.feed_mode {
        FeedMode::Papers => "ARTICLE VIEW",
        FeedMode::Social => "POST VIEW",
    };
    let block = pane_block(title, is_active, app.tick_count, false);

    let content = match app.feed_mode {
        FeedMode::Papers => render_paper_content(app, area),
        FeedMode::Social => render_social_content(app),
    };

    // Cap article scroll at content height
    let inner_height = area.height.saturating_sub(2) as u16;
    let content_lines = content.len() as u16;
    let max_scroll = content_lines.saturating_sub(inner_height / 2);
    if app.article_scroll > max_scroll {
        app.article_scroll = max_scroll;
    }

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((app.article_scroll, 0));

    frame.render_widget(paragraph, area);
}

fn render_paper_content(app: &mut App, area: Rect) -> Vec<Line<'static>> {
    let Some(paper) = app.selected_paper() else {
        return vec![
            Line::from(""),
            Line::from(Span::styled("  Select a paper from the feed.", Theme::dim())),
        ];
    };

    let title = paper.title.clone();
    let authors = paper.authors.clone();
    let date = paper.date.clone();
    let domain = paper.domain.clone();
    let arxiv_id = paper.arxiv_id.clone();
    let abstract_text = paper.abstract_text.clone();
    let separator = "\u{2500}".repeat(area.width.saturating_sub(4) as usize);

    let mut lines = vec![
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(Theme::NEON_CYAN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Authors  ", Theme::dim()),
            Span::styled(authors, Theme::text()),
        ]),
        Line::from(vec![
            Span::styled("Date     ", Theme::dim()),
            Span::styled(date, Theme::text()),
        ]),
        Line::from(vec![
            Span::styled("Domain   ", Theme::dim()),
            Span::styled(domain, Style::default().fg(Theme::DOMAIN_TAG)),
        ]),
    ];

    if let Some(id) = arxiv_id {
        lines.push(Line::from(vec![
            Span::styled("ArXiv    ", Theme::dim()),
            Span::styled(id, Theme::accent()),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(separator.clone(), Theme::dim())));

    if let Some(abs) = abstract_text {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(abs, Theme::text())));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(separator.clone(), Theme::dim())));
    }

    // ── Summary Section ─────────────────────────────────────
    {
        let arxiv_id = paper.arxiv_id.clone().unwrap_or_default();
        let mut has_summary = false;

        // HF TL;DR (always show if available, regardless of mode)
        if let Some(ref ai_sum) = app
            .selected_paper_meta()
            .and_then(|m| m.ai_summary.clone())
        {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " TL;DR ",
                Style::default()
                    .fg(Theme::DARK_BG)
                    .bg(Theme::NEON_MAGENTA)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(ai_sum.clone(), Theme::text())));
            has_summary = true;
        }

        // LLM summary (generated on demand via [M])
        if app.summary_mode.needs_llm() {
            let cache_key = format!("{}:{}", arxiv_id, app.summary_mode.api_key());
            if let Some(summary_text) = app.summary_cache.get(&cache_key) {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!(" {} ", app.summary_mode.label()),
                    Style::default()
                        .fg(Theme::DARK_BG)
                        .bg(Theme::NEON_MAGENTA)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
                lines.extend(crate::ui::markdown::render_markdown(summary_text));
                has_summary = true;
            } else if app.loading.iter().any(|t| matches!(t, crate::app::LoadingTask::LlmSummary(_))) {
                let spinner_char = SPINNER[(app.tick_count as usize) % SPINNER.len()];
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("{} Generating {} summary...", spinner_char, app.summary_mode.label()),
                    Style::default().fg(Theme::NEON_MAGENTA),
                )));
            } else {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("{} selected \u{2014} press [M] to generate", app.summary_mode.label()),
                    Theme::dim(),
                )));
            }
        }

        if has_summary {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(separator.clone(), Theme::dim())));
        }
    }

    // ── Scaffold Section ────────────────────────────────────
    {
        let arxiv_id = paper.arxiv_id.clone().unwrap_or_default();
        let scaffold_path = app.scaffold_index.get(&arxiv_id);
        let is_generating = app.loading.iter().any(|t| matches!(t, crate::app::LoadingTask::LlmScaffold(_)));

        if let Some(path) = scaffold_path {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " IMPLEMENTATION SCAFFOLD ",
                Style::default()
                    .fg(Theme::DARK_BG)
                    .bg(Theme::NEON_YELLOW)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("  Generated at: {}", path),
                Style::default().fg(Theme::NEON_GREEN),
            )));
            lines.push(Line::from(Span::styled(
                "  Ready to implement!",
                Style::default().fg(Theme::NEON_GREEN).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(separator.clone(), Theme::dim())));
        } else if is_generating {
            let spinner_char = SPINNER[(app.tick_count as usize) % SPINNER.len()];
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("{} Generating scaffold...", spinner_char),
                Style::default().fg(Theme::NEON_YELLOW),
            )));
        }
    }

    // ── Metadata Section ─────────────────────────────────
    let meta = app.selected_paper_meta();

    match meta.map(|m| &m.meta_status) {
        Some(MetaFetchStatus::Loading) => {
            let spinner_char = SPINNER[(app.tick_count as usize) % SPINNER.len()];
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("{} Loading metadata...", spinner_char),
                Style::default().fg(Theme::NEON_CYAN),
            )));
        }
        Some(MetaFetchStatus::Loaded) => {
            let meta = meta.unwrap();

            // HF Community Signal
            {
                let mut hf_lines: Vec<(String, String)> = Vec::new();

                if meta.upvotes > 0 {
                    hf_lines.push(("HF Upvotes".into(), format!("\u{2191}{}", meta.upvotes)));
                }
                if meta.num_comments > 0 {
                    hf_lines.push(("Discussion".into(), format!("{} comments", meta.num_comments)));
                }
                if let Some(ref by) = meta.submitted_by {
                    hf_lines.push(("Submitted by".into(), format!("@{}", by)));
                }
                if let Some(ref page) = meta.project_page {
                    hf_lines.push(("Project".into(), page.clone()));
                }

                if !hf_lines.is_empty() {
                    lines.push(Line::from(""));
                    for (label, value) in &hf_lines {
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("{:<14}", label),
                                Theme::dim(),
                            ),
                            Span::styled(
                                value.clone(),
                                Style::default()
                                    .fg(Theme::NEON_YELLOW)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    }
                }

                // Links row
                let arxiv_id = paper.arxiv_id.clone().unwrap_or_default();
                if !arxiv_id.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("HF Paper     ", Theme::dim()),
                        Span::styled(
                            format!("https://huggingface.co/papers/{}", arxiv_id),
                            Theme::accent(),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("PDF          ", Theme::dim()),
                        Span::styled(
                            format!("https://arxiv.org/pdf/{}", arxiv_id),
                            Theme::accent(),
                        ),
                    ]));
                }
            }

            // AI Keywords
            if !meta.ai_keywords.is_empty() {
                lines.push(Line::from(""));
                let kw_text = meta.ai_keywords.join(" \u{00b7} ");
                lines.push(Line::from(Span::styled(
                    kw_text,
                    Style::default().fg(Theme::KEYWORD_HIT),
                )));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(separator.clone(), Theme::dim())));

            // Citations
            if app.config.general.enable_semantic_scholar {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " CITATIONS ",
                    Style::default()
                        .fg(Theme::DARK_BG)
                        .bg(Theme::NEON_CYAN)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));

                if meta.s2_found {
                    lines.push(Line::from(vec![
                        Span::styled("Total        ", Theme::dim()),
                        Span::styled(
                            meta.citation_count.to_string(),
                            Style::default()
                                .fg(Theme::NEON_CYAN)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("Influential   ", Theme::dim()),
                        Span::styled(meta.influential_count.to_string(), Theme::accent()),
                    ]));
                } else {
                    lines.push(Line::from(Span::styled(
                        "Not indexed by Semantic Scholar yet (paper is too new).",
                        Theme::dim(),
                    )));
                }
            } else {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Citations: S2 disabled (enable_semantic_scholar in config)",
                    Theme::dim(),
                )));
            }

            if !meta.top_citations.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled("Top citing papers:", Theme::dim())));
                for (i, citing) in meta.top_citations.iter().enumerate() {
                    let cite_text = if citing.citation_count > 0 {
                        format!(" {}. {} [{}]", i + 1, citing.title, citing.citation_count)
                    } else {
                        format!(" {}. {}", i + 1, citing.title)
                    };
                    lines.push(Line::from(Span::styled(cite_text, Theme::text())));
                }
            }

            // Implementation (GitHub repo from HF)
            if let Some(ref repo_url) = meta.repo_url {
                if !repo_url.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(separator.clone(), Theme::dim())));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        " IMPLEMENTATION ",
                        Style::default()
                            .fg(Theme::DARK_BG)
                            .bg(Theme::NEON_GREEN)
                            .add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("Repo     ", Theme::dim()),
                        Span::styled(repo_url.clone(), Style::default().fg(Theme::NEON_GREEN)),
                    ]));
                    if let Some(stars) = meta.repo_stars {
                        lines.push(Line::from(vec![
                            Span::styled("Stars    ", Theme::dim()),
                            Span::styled(
                                format!("\u{2605} {}", stars),
                                Style::default().fg(Theme::NEON_GREEN),
                            ),
                        ]));
                    }
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(separator, Theme::dim())));
        }
        Some(MetaFetchStatus::Failed) => {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("Metadata fetch failed.", Theme::dim())));
        }
        _ => {}
    }

    lines.push(Line::from(""));
    let scaffold_exists = paper
        .arxiv_id
        .as_deref()
        .and_then(|id| app.scaffold_index.get(id))
        .is_some();
    let scaffold_label = if scaffold_exists {
        "[i]  Re-spawn implementation scaffold"
    } else {
        "[i]  Spawn implementation scaffold"
    };
    lines.push(Line::from(Span::styled(scaffold_label, Theme::accent())));
    lines.push(Line::from(Span::styled("[o]  Export to Obsidian vault", Theme::accent())));
    lines.push(Line::from(Span::styled(
        format!("[m]  Cycle summary mode ({})", app.summary_mode.label()),
        Theme::accent(),
    )));
    if app.summary_mode.needs_llm() {
        lines.push(Line::from(Span::styled("[M]  Generate summary", Theme::accent())));
    }
    lines.push(Line::from(Span::styled("[Enter]  Open in browser", Theme::accent())));

    lines
}

fn render_social_content(app: &App) -> Vec<Line<'static>> {
    let Some(post) = app.selected_social_post() else {
        return vec![
            Line::from(""),
            Line::from(Span::styled("  Select a post from the social feed.", Theme::dim())),
        ];
    };

    let source_name = post.source_name.clone();
    let source_badge = match post.source_type {
        SourceType::Twitter => "Twitter/X",
        SourceType::Blog => "Blog/RSS",
    };
    let title = post.title.clone();
    let content = post.content.clone();
    let url = post.url.clone();
    let published = post.published.clone();

    let mut lines = vec![
        Line::from(Span::styled(
            source_name,
            Style::default()
                .fg(Theme::NEON_CYAN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Source   ", Theme::dim()),
            Span::styled(source_badge, Style::default().fg(Theme::DOMAIN_TAG)),
        ]),
        Line::from(vec![
            Span::styled("Date     ", Theme::dim()),
            Span::styled(published, Theme::text()),
        ]),
    ];

    if let Some(t) = title {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            t,
            Style::default()
                .fg(Theme::NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        )));
    }

    if !content.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(content, Theme::text())));
    }

    if !url.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(url, Theme::accent())));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("[Enter]  Open in browser", Theme::accent())));

    lines
}
