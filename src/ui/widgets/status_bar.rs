use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, FeedMode, InputMode};
use crate::ui::{Theme, SPINNER};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let pane_label = match app.active_pane {
        crate::app::ActivePane::Feed => match app.feed_mode {
            FeedMode::Papers => "PAPERS",
            FeedMode::Social => "SOCIAL",
        },
        crate::app::ActivePane::Highlight => "SPOTLIGHT",
        crate::app::ActivePane::Article => "ARTICLE",
    };

    let mut spans = vec![
        Span::styled(" [q] ", Theme::status_key()),
        Span::styled("quit ", Theme::dim()),
        Span::styled("[Tab] ", Theme::status_key()),
        Span::styled("pane ", Theme::dim()),
        Span::styled("[j/k] ", Theme::status_key()),
        Span::styled("scroll ", Theme::dim()),
        Span::styled("[f] ", Theme::status_key()),
        Span::styled("feed ", Theme::dim()),
        Span::styled("[/] ", Theme::status_key()),
        Span::styled("filter ", Theme::dim()),
        Span::styled("[s] ", Theme::status_key()),
        Span::styled("sort ", Theme::dim()),
        Span::styled("[t] ", Theme::status_key()),
        Span::styled("time ", Theme::dim()),
        Span::styled("[n] ", Theme::status_key()),
        Span::styled("max ", Theme::dim()),
        Span::styled("[p] ", Theme::status_key()),
        Span::styled("profile ", Theme::dim()),
        Span::styled("[?] ", Theme::status_key()),
        Span::styled("help", Theme::dim()),
        Span::styled(" | ", Theme::dim()),
        Span::styled(
            pane_label.to_string(),
            Style::default().fg(Theme::NEON_GREEN),
        ),
    ];

    // Loading spinner
    if app.is_loading() {
        let idx = (app.tick_count as usize) % SPINNER.len();
        spans.push(Span::styled("  ", Theme::dim()));
        spans.push(Span::styled(
            format!("{} fetching", SPINNER[idx]),
            Style::default().fg(Theme::NEON_CYAN),
        ));
    }

    // Profile badge
    let profile_name = app.active_profile_name().to_string();
    spans.push(Span::styled("  ", Theme::dim()));
    spans.push(Span::styled(
        format!("[{}]", profile_name),
        Style::default().fg(Theme::DOMAIN_TAG),
    ));

    // Filter indicator
    if app.input_mode == InputMode::Filter {
        spans.push(Span::styled("  ", Theme::dim()));
        spans.push(Span::styled(
            format!("FILTER: {}\u{2588}", app.filter_text),
            Style::default().fg(Theme::NEON_MAGENTA),
        ));
    } else if app.input_mode == InputMode::Confirm {
        spans.push(Span::styled("  ", Theme::dim()));
        spans.push(Span::styled(
            app.confirm_message.clone(),
            Style::default().fg(Theme::NEON_YELLOW),
        ));
    } else if !app.filter_text.is_empty() {
        spans.push(Span::styled("  ", Theme::dim()));
        spans.push(Span::styled(
            format!("filter: \"{}\"", app.filter_text),
            Style::default().fg(Theme::NEON_MAGENTA),
        ));
    }

    // LLM provider indicator
    if !app.llm_providers.is_empty() {
        let p = &app.llm_providers[app.active_provider_idx];
        spans.push(Span::styled("  ", Theme::dim()));
        spans.push(Span::styled(
            format!("LLM:{}", p.name()),
            Style::default().fg(Theme::NEON_CYAN),
        ));
    }

    // Transient status message
    if let Some((msg, _)) = &app.status_message {
        spans.push(Span::styled("  ", Theme::dim()));
        spans.push(Span::styled(
            msg.clone(),
            Style::default().fg(Theme::NEON_YELLOW),
        ));
    }

    let bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Theme::DARK_BG));

    frame.render_widget(bar, area);
}
