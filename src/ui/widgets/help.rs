use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::ui::Theme;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(58, 32, area);
    frame.render_widget(Clear, popup);

    let sep = "\u{2500}".repeat(52);
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Navigation",
            Style::default()
                .fg(Theme::NEON_CYAN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(format!("  {}", sep), Theme::dim())),
        help_line("  j/k  \u{2191}/\u{2193}", "Scroll up / down"),
        help_line("  h/l  \u{2190}/\u{2192}", "Switch pane"),
        help_line("  Tab / S-Tab", "Next / prev pane"),
        help_line("  g", "Jump to top"),
        help_line("  G", "Jump to bottom"),
        Line::from(""),
        Line::from(Span::styled(
            "  Actions",
            Style::default()
                .fg(Theme::NEON_CYAN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(format!("  {}", sep), Theme::dim())),
        help_line("  i", "Spawn implementation scaffold"),
        help_line("  o", "Export to Obsidian vault"),
        help_line("  b", "Bookmark paper to Reading List"),
        help_line("  B", "Bookmark to collection..."),
        help_line("  d", "Remove paper (vault mode)"),
        help_line("  Enter", "Open in browser / drill into"),
        help_line("  m", "Cycle summary mode"),
        help_line("  M", "Generate LLM summary"),
        help_line("  L", "Cycle LLM provider"),
        help_line("  p", "Cycle research profile"),
        help_line("  r", "Refresh feed"),
        help_line("  f", "Cycle feed (papers/social/vault)"),
        help_line("  /", "Filter feed (type to search)"),
        help_line("  S", "Search papers (HuggingFace)"),
        help_line("  s", "Cycle sort (papers only)"),
        help_line("  t", "Cycle time window (24h/7d/30d/all)"),
        help_line("  n", "Cycle max items (10/25/50/75/100)"),
        Line::from(""),
        Line::from(Span::styled(
            "  General",
            Style::default()
                .fg(Theme::NEON_CYAN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(format!("  {}", sep), Theme::dim())),
        help_line("  ?", "Toggle this help"),
        help_line("  Esc", "Dismiss / close / back"),
        help_line("  q", "Quit"),
        Line::from(""),
    ];

    let block = Block::default()
        .title(Line::from(Span::styled(
            " \u{2328}  KEYBINDINGS ",
            Style::default()
                .fg(Theme::NEON_MAGENTA)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Theme::NEON_MAGENTA)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Theme::DARK_BG));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.help_scroll, 0));
    frame.render_widget(paragraph, popup);
}

fn help_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{:<16}", key),
            Theme::status_key(),
        ),
        Span::styled(desc, Theme::text()),
    ])
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
