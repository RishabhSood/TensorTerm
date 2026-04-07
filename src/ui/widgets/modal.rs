use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, InputMode};
use crate::ui::Theme;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    match app.input_mode {
        InputMode::Confirm => render_confirm(frame, area, app),
        InputMode::ScaffoldPrompt => render_scaffold_prompt(frame, area, app),
        InputMode::CollectionPicker => render_collection_picker(frame, area, app),
        InputMode::NewCollection => render_new_collection(frame, area, app),
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

fn render_collection_picker(frame: &mut Frame, area: Rect, app: &App) {
    let names: Vec<String> = app.vault.collection_names().iter().map(|s| s.to_string()).collect();
    let height = (names.len() as u16 + 7).min(20);
    let popup = centered_rect(50, height, area);
    frame.render_widget(Clear, popup);

    let mut items: Vec<ListItem> = names
        .iter()
        .map(|name| {
            let count = app.vault.papers_in(name).len();
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("  {} ", name),
                    Style::default().fg(Theme::NEON_CYAN).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("({})", count),
                    Theme::dim(),
                ),
            ]))
        })
        .collect();

    // "New Collection..." option at the end
    items.push(ListItem::new(Line::from(Span::styled(
        "  + New Collection...",
        Style::default().fg(Theme::NEON_GREEN).add_modifier(Modifier::BOLD),
    ))));

    let block = Block::default()
        .title(Line::from(Span::styled(
            " BOOKMARK TO COLLECTION ",
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

    let list = List::new(items)
        .block(block)
        .highlight_style(Theme::highlight_bar())
        .highlight_symbol(" >> ");

    let mut picker_state = app.collection_picker_state.clone();
    frame.render_stateful_widget(list, popup, &mut picker_state);
}

fn render_new_collection(frame: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(50, 9, area);
    frame.render_widget(Clear, popup);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Collection name:", Theme::dim())),
        Line::from(vec![
            Span::styled("  > ", Style::default().fg(Theme::NEON_CYAN)),
            Span::styled(
                format!("{}\u{2588}", app.new_collection_name),
                Style::default()
                    .fg(Theme::NEON_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Enter] ", Style::default().fg(Theme::NEON_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("Create & Bookmark   ", Theme::text()),
            Span::styled("[Esc] ", Style::default().fg(Theme::NEON_MAGENTA).add_modifier(Modifier::BOLD)),
            Span::styled("Cancel", Theme::text()),
        ]),
        Line::from(""),
    ];

    let block = Block::default()
        .title(Line::from(Span::styled(
            " NEW COLLECTION ",
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
