pub mod article;
pub mod feed;
pub mod header;
pub mod help;
pub mod highlight;
pub mod modal;
pub mod splash;
pub mod status_bar;

use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders},
};

use crate::ui::Theme;

/// Build a styled pane block, handling active/inactive/pulse states.
/// Eliminates the repeated boilerplate across every widget.
pub fn pane_block(
    title: impl Into<String>,
    is_active: bool,
    tick: u64,
    has_pulse: bool,
) -> Block<'static> {
    let title_text = title.into();

    let border_style = if has_pulse {
        Theme::pulse_border()
    } else if is_active {
        Theme::active_border(tick)
    } else {
        Theme::inactive_border()
    };

    let title_style = if is_active {
        Theme::title_active()
    } else {
        Theme::title_inactive()
    };

    Block::default()
        .title(Line::from(Span::styled(
            format!(" {} ", title_text),
            title_style,
        )))
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(Theme::PANEL_BG))
}
