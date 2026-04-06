use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, InputMode};
use crate::ui::Theme;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    match app.input_mode {
        InputMode::Confirm => render_confirm(frame, area, app),
        InputMode::ScaffoldPrompt => render_scaffold_prompt(frame, area, app),
        _ => {}
    }
}

fn render_confirm(frame: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(60, 9, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", app.confirm_message),
            Theme::text(),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("      [y] ", Style::default().fg(Theme::NEON_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("Confirm   ", Theme::text()),
            Span::styled("[n] ", Style::default().fg(Theme::NEON_MAGENTA).add_modifier(Modifier::BOLD)),
            Span::styled("Cancel", Theme::text()),
        ]),
        Line::from(""),
    ];

    let block = Block::default()
        .title(Line::from(Span::styled(
            " CONFIRM ",
            Style::default()
                .fg(Theme::NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Theme::NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Theme::DARK_BG));

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, popup);
}

fn render_scaffold_prompt(frame: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(60, 11, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Project name:", Theme::dim())),
        Line::from(vec![
            Span::styled("  > ", Style::default().fg(Theme::NEON_CYAN)),
            Span::styled(
                format!("{}\u{2588}", app.scaffold_project_name),
                Style::default()
                    .fg(Theme::NEON_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Will be created at:", Theme::dim())),
        Line::from(Span::styled(
            format!("  {}/{}/", app.scaffold_output_dir, app.scaffold_project_name),
            Style::default().fg(Theme::NEON_CYAN),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Enter] ", Style::default().fg(Theme::NEON_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("Generate   ", Theme::text()),
            Span::styled("[Esc] ", Style::default().fg(Theme::NEON_MAGENTA).add_modifier(Modifier::BOLD)),
            Span::styled("Cancel", Theme::text()),
        ]),
        Line::from(""),
    ];

    let block = Block::default()
        .title(Line::from(Span::styled(
            " IMPLEMENTATION SCAFFOLD ",
            Style::default()
                .fg(Theme::NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Theme::NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Theme::DARK_BG));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
