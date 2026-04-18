use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

use crate::app::{App, SourceKind, SourceStatus, SplashLogEntry, SPLASH_TIMEOUT_TICKS_PUB};
use crate::ui::{Theme, SPINNER};

const BAR_WIDTH: usize = 60;
/// How many ticks each rotating status line stays on screen (~720ms at 80ms/tick).
const STATUS_ROTATE_TICKS: u64 = 9;

/// Big block-letter banner (ANSI Shadow font), ~95 cols wide, 6 rows.
/// Used when the terminal is wide enough; otherwise we fall back to the single-line version.
const BIG_BANNER: &[&str] = &[
    "████████╗███████╗███╗   ██╗███████╗ ██████╗ ██████╗      ████████╗███████╗██████╗ ███╗   ███╗",
    "╚══██╔══╝██╔════╝████╗  ██║██╔════╝██╔═══██╗██╔══██╗     ╚══██╔══╝██╔════╝██╔══██╗████╗ ████║",
    "   ██║   █████╗  ██╔██╗ ██║███████╗██║   ██║██████╔╝ //     ██║   █████╗  ██████╔╝██╔████╔██║",
    "   ██║   ██╔══╝  ██║╚██╗██║╚════██║██║   ██║██╔══██╗ //     ██║   ██╔══╝  ██╔══██╗██║╚██╔╝██║",
    "   ██║   ███████╗██║ ╚████║███████║╚██████╔╝██║  ██║        ██║   ███████╗██║  ██║██║ ╚═╝ ██║",
    "   ╚═╝   ╚══════╝╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝        ╚═╝   ╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝",
];

const BIG_BANNER_WIDTH: u16 = 95;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    // Full-screen takeover.
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default().style(Style::default().bg(Theme::DARK_BG)),
        area,
    );

    // Use the big block-letter banner only if the terminal can hold it; fall back
    // to the single-line version on narrow terminals.
    let use_big_banner = area.width >= BIG_BANNER_WIDTH;
    let banner_rows: u16 = if use_big_banner { BIG_BANNER.len() as u16 } else { 1 };

    // Vertically center the cluster using flex spacers; pin footer to bottom.
    let chunks = Layout::vertical([
        Constraint::Min(0),               // top spacer
        Constraint::Length(banner_rows),  // banner (single line OR big block letters)
        Constraint::Length(1),            // gap
        Constraint::Length(1),            // subtitle
        Constraint::Length(1),            // gap
        Constraint::Length(1),            // [INITIALIZING DATA STREAMS]
        Constraint::Length(2),            // gap
        Constraint::Length(1),            // progress bar
        Constraint::Length(1),            // counter
        Constraint::Length(1),            // gap
        Constraint::Length(1),            // rotating status
        Constraint::Min(0),               // bottom spacer
        Constraint::Length(1),            // footer
    ])
    .split(area);

    render_banner(frame, chunks[1], app.tick_count, use_big_banner);
    render_centered_text(frame, chunks[3], "AI/ML Research Intelligence Terminal", Theme::dim());
    render_centered_text(
        frame,
        chunks[5],
        "[ INITIALIZING DATA STREAMS ]",
        Style::default()
            .fg(Theme::NEON_MAGENTA)
            .add_modifier(Modifier::BOLD),
    );
    render_bar(frame, chunks[7], app);
    render_counter(frame, chunks[8], app);
    render_status_line(frame, chunks[10], app);
    render_footer(frame, chunks[12]);
}

fn render_banner(frame: &mut Frame, area: Rect, tick: u64, big: bool) {
    // Match the main header's breathing cyan glow exactly so the splash and
    // post-splash UI feel like one continuous brand moment.
    let glow = (tick as f64 * 0.08).sin() * 0.5 + 0.5;
    let cyan_val = (180.0 + glow * 75.0) as u8;
    let banner_color = Color::Rgb(0, cyan_val, cyan_val);
    let fade_color = Color::Rgb(0, cyan_val / 3, cyan_val / 3);

    if big {
        // Multi-line ANSI Shadow block letters; entire block pulses in cyan.
        let style = Style::default()
            .fg(banner_color)
            .add_modifier(Modifier::BOLD);
        let lines: Vec<Line> = BIG_BANNER
            .iter()
            .map(|row| Line::from(Span::styled(row.to_string(), style)))
            .collect();
        let para = Paragraph::new(lines).alignment(Alignment::Center);
        frame.render_widget(para, area);
        // Suppress unused warning when only the big variant runs.
        let _ = fade_color;
        return;
    }

    // Compact single-line fallback (identical to main header.rs banner).
    let line = Line::from(vec![
        Span::styled("██▓▒░ ", Style::default().fg(fade_color)),
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
        Span::styled(" ░▒▓██", Style::default().fg(fade_color)),
    ]);

    let para = Paragraph::new(line).alignment(Alignment::Center);
    frame.render_widget(para, area);
}

fn render_centered_text(frame: &mut Frame, area: Rect, text: &str, style: Style) {
    let para = Paragraph::new(Line::from(Span::styled(text.to_string(), style)))
        .alignment(Alignment::Center);
    frame.render_widget(para, area);
}

fn render_bar(frame: &mut Frame, area: Rect, app: &App) {
    let total = app.splash_total_sources.max(1);
    let done = app.splash_completed.min(total);
    let pct = (done as f32 / total as f32 * 100.0) as u16;

    let filled = ((BAR_WIDTH as f32) * (done as f32 / total as f32)) as usize;
    let filled = filled.min(BAR_WIDTH);

    // Pulse the leading edge between ▰ and ▱.
    let pulse_filled = (app.tick_count / 6) % 2 == 0;
    let leading = if filled < BAR_WIDTH && filled > 0 && pct < 100 {
        Some(if pulse_filled { '▰' } else { '▱' })
    } else {
        None
    };

    let pre_filled = filled.saturating_sub(if leading.is_some() { 1 } else { 0 });
    let empty = BAR_WIDTH.saturating_sub(pre_filled + leading.iter().count());

    let bar_color = if pct >= 100 {
        Theme::NEON_GREEN
    } else {
        Theme::NEON_CYAN
    };
    let pct_color = if pct >= 100 {
        Theme::NEON_GREEN
    } else {
        Theme::NEON_YELLOW
    };

    let line = Line::from(vec![
        Span::styled(
            "▰".repeat(pre_filled),
            Style::default().fg(bar_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            leading.map(|c| c.to_string()).unwrap_or_default(),
            Style::default()
                .fg(Theme::NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("▱".repeat(empty), Style::default().fg(Theme::DIM)),
        Span::styled(
            format!("  {:>3}%", pct),
            Style::default().fg(pct_color).add_modifier(Modifier::BOLD),
        ),
    ]);

    let para = Paragraph::new(line).alignment(Alignment::Center);
    frame.render_widget(para, area);
}

fn render_counter(frame: &mut Frame, area: Rect, app: &App) {
    let total = app.splash_total_sources.max(1);
    let done = app.splash_completed.min(total);
    let elapsed_s = app.tick_count.wrapping_sub(app.splash_started_at) as f32 * 0.08;

    let line = Line::from(vec![
        Span::styled(format!("{}/{} sources", done, total), Theme::dim()),
        Span::styled("   ·   ", Theme::dim()),
        Span::styled(format!("{:.1}s", elapsed_s), Theme::dim()),
    ]);

    let para = Paragraph::new(line).alignment(Alignment::Center);
    frame.render_widget(para, area);
}

fn render_status_line(frame: &mut Frame, area: Rect, app: &App) {
    let line = pick_status_line(app);
    let para = Paragraph::new(line).alignment(Alignment::Center);
    frame.render_widget(para, area);
}

fn pick_status_line<'a>(app: &'a App) -> Line<'a> {
    let in_flight: Vec<&SplashLogEntry> = app
        .splash_log
        .iter()
        .filter(|e| matches!(e.status, SourceStatus::Started))
        .collect();

    if !in_flight.is_empty() {
        let idx = ((app.tick_count / STATUS_ROTATE_TICKS) as usize) % in_flight.len();
        let entry = in_flight[idx];
        let spinner = SPINNER[(app.tick_count as usize) % SPINNER.len()];
        let msg = format_in_flight(entry);
        return Line::from(vec![
            Span::styled(
                format!("{}  ", spinner),
                Style::default()
                    .fg(Theme::NEON_CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(msg, Theme::text()),
        ]);
    }

    if let Some(entry) = app.splash_log.last() {
        let (icon, color) = match entry.status {
            SourceStatus::Done => ("✓", Theme::NEON_GREEN),
            SourceStatus::Failed => ("✗", Theme::NEON_MAGENTA),
            SourceStatus::Started => ("·", Theme::DIM),
        };
        return Line::from(vec![
            Span::styled(
                format!("{}  ", icon),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format_finished(entry), Theme::dim()),
        ]);
    }

    Line::from(Span::styled("warming up...", Theme::dim()))
}

fn format_in_flight(entry: &SplashLogEntry) -> String {
    match entry.kind {
        SourceKind::Social => format!("pulling tweets from {}", entry.name),
        SourceKind::News => format!("fetching news from {}", entry.name),
        SourceKind::Papers => format!("scanning ArXiv for {}", entry.name),
        SourceKind::HfSpotlight => "fetching HuggingFace daily paper".into(),
    }
}

fn format_finished(entry: &SplashLogEntry) -> String {
    match (entry.kind, entry.status) {
        (SourceKind::Social, SourceStatus::Done) => format!("synced {}", entry.name),
        (SourceKind::Social, SourceStatus::Failed) => format!("could not reach {}", entry.name),
        (SourceKind::News, SourceStatus::Done) => format!("indexed {}", entry.name),
        (SourceKind::News, SourceStatus::Failed) => format!("could not reach {}", entry.name),
        (SourceKind::Papers, SourceStatus::Done) => format!("ArXiv feed ready ({})", entry.name),
        (SourceKind::Papers, SourceStatus::Failed) => "ArXiv feed unavailable".into(),
        (SourceKind::HfSpotlight, SourceStatus::Done) => "HuggingFace daily paper ready".into(),
        (SourceKind::HfSpotlight, SourceStatus::Failed) => "HuggingFace spotlight unavailable".into(),
        (_, SourceStatus::Started) => format!("loading {}", entry.name),
    }
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled("▶ ", Style::default().fg(Theme::NEON_GREEN)),
        Span::styled("press any key to skip", Theme::dim()),
        Span::styled("    ·    ", Theme::dim()),
        Span::styled(
            format!("auto-dismiss after {}s", SPLASH_TIMEOUT_TICKS_PUB / 12),
            Theme::dim(),
        ),
    ]);
    let para = Paragraph::new(line).alignment(Alignment::Center);
    frame.render_widget(para, area);
}
