use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};

use crate::app::{ActivePane, App, FeedMode, InputMode, VaultLevel};
use crate::ui::{Theme, SPINNER};
use super::pane_block;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let is_active = app.active_pane == ActivePane::Feed;
    let has_pulse = app.has_load_pulse();
    let (pos, total) = app.feed_position();

    let mode_label = if app.is_search_active() || app.input_mode == InputMode::Search {
        "PAPERS [Search Mode]".to_string()
    } else {
        match app.feed_mode {
            FeedMode::Papers => "PAPER FEED".to_string(),
            FeedMode::Social => "SOCIAL FEED".to_string(),
            FeedMode::Vault => match &app.vault_level {
                VaultLevel::Collections => "VAULT".to_string(),
                VaultLevel::Papers(col) => format!("VAULT \u{2192} {}", col),
            },
        }
    };

    let sort_badge = match app.feed_mode {
        FeedMode::Papers if !app.is_search_active() && app.input_mode != InputMode::Search && app.paper_sort != crate::app::PaperSort::Date => {
            format!(" [{}]", app.paper_sort.label())
        }
        _ => String::new(),
    };

    let time_badge = if app.feed_mode == FeedMode::Vault || app.is_search_active() || app.input_mode == InputMode::Search {
        String::new()
    } else if app.time_window != crate::app::TimeWindow::Day {
        format!(" <{}>", app.time_window.label())
    } else {
        " <24h>".to_string()
    };

    let title = if app.input_mode == InputMode::Search {
        // Typing search query — no counts yet
        "PAPERS [Search Mode]".to_string()
    } else if app.is_search_active() {
        // Showing search results — show counts
        if total > 0 {
            format!("PAPERS [Search Mode] [{}/{}]", pos, total)
        } else {
            "PAPERS [Search Mode]".to_string()
        }
    } else if total > 0 {
        format!("{} [{}/{}]{}{}", mode_label, pos, total, sort_badge, time_badge)
    } else {
        format!("{}{}{}", mode_label, sort_badge, time_badge)
    };

    let block = pane_block(title, is_active, app.tick_count, has_pulse);

    match app.feed_mode {
        FeedMode::Papers => render_papers(frame, area, app, block),
        FeedMode::Social => render_social(frame, area, app, block),
        FeedMode::Vault => render_vault(frame, area, app, block),
    }
}

fn render_papers(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    block: ratatui::widgets::Block<'static>,
) {
    // Search input bar
    if app.input_mode == InputMode::Search {
        let cursor = "\u{2588}";
        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    " \u{1f50d} Search: ",
                    Style::default().fg(Theme::NEON_MAGENTA).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}{}", app.search_query, cursor),
                    Style::default().fg(Theme::NEON_CYAN).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled("  Enter to search, Esc to cancel", Theme::dim())),
        ];
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    // Search results mode
    if let Some(ref results) = app.search_results {
        if results.is_empty() {
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled("  No results found.", Theme::dim())),
                Line::from(Span::styled("  Press [Esc] to return to feed.", Theme::dim())),
            ];
            let paragraph = Paragraph::new(lines).block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let max = app.max_items;
        let items: Vec<ListItem> = results
            .iter()
            .take(max)
            .map(|paper| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("[{}] ", paper.domain),
                        Style::default().fg(Theme::DOMAIN_TAG),
                    ),
                    Span::styled(paper.title.clone(), Theme::text()),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Theme::highlight_bar())
            .highlight_symbol(" >> ");

        frame.render_stateful_widget(list, area, &mut app.search_state);
        return;
    }

    // Loading state
    if app.is_loading() && app.feed_items.is_empty() {
        let idx = (app.tick_count as usize) % SPINNER.len();
        let loading = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {} Fetching papers\u{2026}", SPINNER[idx]),
                Theme::accent(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  Profile: {}", app.active_profile_name()),
                Theme::dim(),
            )),
        ];
        let paragraph = Paragraph::new(loading).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let indices = app.filtered_paper_indices();

    if indices.is_empty() {
        let mut lines = filter_bar_lines(app);
        lines.push(Line::from(""));
        if !app.filter_text.is_empty() {
            lines.push(Line::from(Span::styled("  No matches.", Theme::dim())));
        } else {
            lines.push(Line::from(Span::styled(
                format!("  No papers in the past {}.", app.time_window.label()),
                Theme::accent(),
            )));
            lines.push(Line::from(Span::styled(
                "  You're all caught up! Press [t] to increase the time window.",
                Theme::dim(),
            )));
        }
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = indices
        .iter()
        .map(|&i| {
            let paper = &app.feed_items[i];
            let matches = app.paper_matches_keywords(paper);
            let title_style = if matches {
                Theme::keyword_match()
            } else {
                Theme::text()
            };
            let prefix = if matches { "\u{2726} " } else { "" };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("[{}] {}", paper.domain, prefix),
                    Style::default().fg(Theme::DOMAIN_TAG),
                ),
                Span::styled(paper.title.clone(), title_style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Theme::highlight_bar())
        .highlight_symbol(" >> ");

    frame.render_stateful_widget(list, area, &mut app.feed_state);
}

fn render_social(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    block: ratatui::widgets::Block<'static>,
) {
    // Loading state
    if app.is_loading() && app.social_items.is_empty() {
        let idx = (app.tick_count as usize) % SPINNER.len();
        let loading = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {} Fetching social feeds\u{2026}", SPINNER[idx]),
                Theme::accent(),
            )),
        ];
        let paragraph = Paragraph::new(loading).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let indices = app.filtered_social_indices();

    if indices.is_empty() {
        let mut lines = filter_bar_lines(app);
        lines.push(Line::from(""));
        if !app.filter_text.is_empty() {
            lines.push(Line::from(Span::styled("  No matches.", Theme::dim())));
        } else if app.social_items.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No posts loaded. Check social config.",
                Theme::dim(),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!("  No posts in the past {}.", app.time_window.label()),
                Theme::accent(),
            )));
            lines.push(Line::from(Span::styled(
                "  Press [t] to increase the time window.",
                Theme::dim(),
            )));
        }
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = indices
        .iter()
        .map(|&i| {
            let post = &app.social_items[i];
            let display = post
                .title
                .as_deref()
                .unwrap_or(&post.content);
            // Truncate for list view
            let truncated: String = display.chars().take(120).collect();

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("[@{}] ", post.source_name),
                    Style::default()
                        .fg(Theme::NEON_MAGENTA)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(truncated, Theme::text()),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Theme::highlight_bar())
        .highlight_symbol(" >> ");

    frame.render_stateful_widget(list, area, &mut app.social_state);
}

fn filter_bar_lines(app: &App) -> Vec<Line<'static>> {
    if app.filter_text.is_empty() && app.input_mode != InputMode::Filter {
        return Vec::new();
    }

    let cursor = if app.input_mode == InputMode::Filter {
        "\u{2588}" // block cursor
    } else {
        ""
    };

    vec![Line::from(vec![
        Span::styled(
            " \u{2572} filter: ",
            Style::default().fg(Theme::NEON_MAGENTA),
        ),
        Span::styled(
            format!("{}{} ", app.filter_text, cursor),
            Style::default()
                .fg(Theme::NEON_CYAN)
                .add_modifier(Modifier::BOLD),
        ),
    ])]
}

fn render_vault(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    block: ratatui::widgets::Block<'static>,
) {
    match &app.vault_level {
        VaultLevel::Collections => {
            let names: Vec<String> = app.vault.collection_names().iter().map(|s| s.to_string()).collect();
            if names.is_empty() {
                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled("  No collections yet.", Theme::dim())),
                    Line::from(Span::styled("  Press [b] on a paper to bookmark it.", Theme::dim())),
                ];
                let paragraph = Paragraph::new(lines).block(block);
                frame.render_widget(paragraph, area);
                return;
            }

            let items: Vec<ListItem> = names
                .iter()
                .map(|name| {
                    let count = app.vault.papers_in(name).len();
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("[{}] ", name),
                            Style::default()
                                .fg(Theme::NEON_CYAN)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("({} papers)", count),
                            Theme::dim(),
                        ),
                    ]))
                })
                .collect();

            let list = List::new(items)
                .block(block)
                .highlight_style(Theme::highlight_bar())
                .highlight_symbol(" >> ");

            frame.render_stateful_widget(list, area, &mut app.vault_state);
        }
        VaultLevel::Papers(col) => {
            let paper_ids: Vec<String> = app.vault.papers_in(col).iter().map(|s| s.to_string()).collect();
            if paper_ids.is_empty() {
                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled("  No papers in this collection.", Theme::dim())),
                    Line::from(Span::styled("  Press [Esc] to go back.", Theme::dim())),
                ];
                let paragraph = Paragraph::new(lines).block(block);
                frame.render_widget(paragraph, area);
                return;
            }

            let items: Vec<ListItem> = paper_ids
                .iter()
                .map(|arxiv_id| {
                    if let Some(cached) = app.vault.paper_cache.get(arxiv_id.as_str()) {
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                format!("[{}] ", cached.domain),
                                Style::default().fg(Theme::DOMAIN_TAG),
                            ),
                            Span::styled(cached.title.clone(), Theme::text()),
                        ]))
                    } else {
                        ListItem::new(Line::from(Span::styled(
                            format!("  {}", arxiv_id),
                            Theme::dim(),
                        )))
                    }
                })
                .collect();

            let list = List::new(items)
                .block(block)
                .highlight_style(Theme::highlight_bar())
                .highlight_symbol(" >> ");

            frame.render_stateful_widget(list, area, &mut app.vault_state);
        }
    }
}
