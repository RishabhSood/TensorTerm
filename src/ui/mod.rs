mod theme;
pub mod markdown;
pub mod widgets;

pub use theme::Theme;

use ratatui::{
    layout::{Constraint, Layout},
    style::Style,
    widgets::Block,
    Frame,
};

use crate::app::{App, InputMode};

/// Braille spinner frames — shared across all widgets.
pub const SPINNER: &[char] = &[
    '\u{280b}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283c}',
    '\u{2834}', '\u{2826}', '\u{2827}', '\u{2807}', '\u{280f}',
];

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Dark background
    frame.render_widget(
        Block::default().style(Style::default().bg(Theme::DARK_BG)),
        area,
    );

    // Outer: header | content | status bar
    let outer = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(10),
        Constraint::Length(1),
    ])
    .split(area);

    widgets::header::render(frame, outer[0], app);

    // Content: left 38% | right 62%
    let content = Layout::horizontal([
        Constraint::Percentage(38),
        Constraint::Percentage(62),
    ])
    .split(outer[1]);

    // Left: feed 62% | spotlight 38%
    let left_col = Layout::vertical([
        Constraint::Percentage(62),
        Constraint::Percentage(38),
    ])
    .split(content[0]);

    widgets::feed::render(frame, left_col[0], app);
    widgets::highlight::render(frame, left_col[1], app);
    widgets::article::render(frame, content[1], app);
    widgets::status_bar::render(frame, outer[2], app);

    // Help popup overlay (rendered last, on top)
    if app.input_mode == InputMode::Help {
        widgets::help::render(frame, area, app);
    }

    // Modal overlays (confirm, scaffold prompt, collection picker, new collection)
    if matches!(app.input_mode, InputMode::Confirm | InputMode::ScaffoldPrompt | InputMode::CollectionPicker | InputMode::NewCollection) {
        widgets::modal::render(frame, area, app);
    }
}
