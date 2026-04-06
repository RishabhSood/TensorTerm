use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::{ActivePane, App};
use crate::ui::Theme;
use super::pane_block;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let is_active = app.active_pane == ActivePane::Highlight;
    let block = pane_block("SPOTLIGHT", is_active, app.tick_count, false);

    let mut lines: Vec<Line> = Vec::new();

    if let Some(ref hf) = app.hf_spotlight {
        // Rich HF daily paper spotlight
        lines.push(Line::from(Span::styled(
            " HF DAILY TOP PAPER ",
            Style::default()
                .fg(Theme::DARK_BG)
                .bg(Theme::NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            hf.title.clone(),
            Style::default()
                .fg(Theme::NEON_CYAN)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Authors   ", Theme::dim()),
            Span::styled(hf.authors.clone(), Theme::text()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Upvotes   ", Theme::dim()),
            Span::styled(
                format!("\u{2191}{}", hf.upvotes),
                Style::default()
                    .fg(Theme::NEON_YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        if !hf.arxiv_id.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Link      ", Theme::dim()),
                Span::styled(
                    format!("huggingface.co/papers/{}", hf.arxiv_id),
                    Theme::accent(),
                ),
            ]));
        }
        if !hf.summary.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(hf.summary.clone(), Theme::text())));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "[Enter] Open in browser",
            Theme::accent(),
        )));
    } else {
        // Fallback: keyword-matched spotlight
        lines.push(Line::from(Span::styled(
            app.spotlight_title.clone(),
            Theme::spotlight(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            app.spotlight_body.clone(),
            Theme::text(),
        )));
    }

    // Clamp scroll — estimate wrapped content as ~3x line count for long text
    let inner_height = area.height.saturating_sub(2);
    let estimated_content = (lines.len() as u16).saturating_mul(3);
    let max_scroll = estimated_content.saturating_sub(inner_height);
    if app.highlight_scroll > max_scroll {
        app.highlight_scroll = max_scroll;
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((app.highlight_scroll, 0));

    frame.render_widget(paragraph, area);
}
