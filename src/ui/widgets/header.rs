use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::ui::Theme;

pub fn render(frame: &mut Frame, area: Rect, app: &crate::app::App) {
    let glow = (app.tick_count as f64 * 0.08).sin() * 0.5 + 0.5;
    let cyan_val = (180.0 + glow * 75.0) as u8;
    let banner_color = Color::Rgb(0, cyan_val, cyan_val);
    let fade_color = Color::Rgb(0, cyan_val / 3, cyan_val / 3);

    let profile_name = app.active_profile_name().to_string();

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{2588}\u{2588}\u{2593}\u{2592}\u{2591} ", Style::default().fg(fade_color)),
            Span::styled(
                "T E N S O R",
                Style::default()
                    .fg(banner_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  //  ", Style::default().fg(Theme::DIM)),
            Span::styled(
                "T E R M",
                Style::default()
                    .fg(banner_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" \u{2591}\u{2592}\u{2593}\u{2588}\u{2588}", Style::default().fg(fade_color)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Research Intelligence Terminal",
                Style::default().fg(Theme::NEON_MAGENTA),
            ),
            Span::styled("  \u{00b7}  ", Theme::dim()),
            Span::styled("v0.1.0", Style::default().fg(Theme::NEON_GREEN)),
            Span::styled("  \u{00b7}  ", Theme::dim()),
            Span::styled(
                format!("[{}]", profile_name),
                Style::default().fg(Theme::DOMAIN_TAG),
            ),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Theme::BORDER_INACTIVE));

    let header = Paragraph::new(lines).block(block);
    frame.render_widget(header, area);
}
