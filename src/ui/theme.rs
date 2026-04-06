use ratatui::style::{Color, Modifier, Style};

/// Cyberpunk neon color palette and derived styles.
pub struct Theme;

impl Theme {
    // ── Core Palette ─────────────────────────────────────────
    pub const NEON_GREEN: Color = Color::Rgb(57, 255, 20);
    pub const NEON_MAGENTA: Color = Color::Rgb(255, 0, 255);
    pub const NEON_CYAN: Color = Color::Rgb(0, 255, 255);
    pub const NEON_YELLOW: Color = Color::Rgb(255, 234, 0);
    pub const DARK_BG: Color = Color::Rgb(10, 10, 18);
    pub const PANEL_BG: Color = Color::Rgb(16, 16, 28);
    pub const DIM: Color = Color::Rgb(80, 80, 100);
    pub const BORDER_INACTIVE: Color = Color::Rgb(35, 35, 55);
    pub const DOMAIN_TAG: Color = Color::Rgb(255, 140, 0);
    pub const KEYWORD_HIT: Color = Color::Rgb(0, 230, 180);

    // ── Derived Styles ───────────────────────────────────────

    pub fn text() -> Style {
        Style::default().fg(Self::NEON_GREEN)
    }

    pub fn dim() -> Style {
        Style::default().fg(Self::DIM)
    }

    pub fn accent() -> Style {
        Style::default().fg(Self::NEON_CYAN)
    }

    /// Active list item highlight bar.
    pub fn highlight_bar() -> Style {
        Style::default()
            .fg(Self::DARK_BG)
            .bg(Self::NEON_GREEN)
            .add_modifier(Modifier::BOLD)
    }

    /// Pulsing border for the active pane (sin-wave on green channel).
    pub fn active_border(tick: u64) -> Style {
        let phase = (tick as f64 * 0.12).sin() * 0.5 + 0.5;
        let g = (160.0 + phase * 95.0) as u8;
        Style::default()
            .fg(Color::Rgb(15, g, 15))
            .add_modifier(Modifier::BOLD)
    }

    /// Brief cyan flash when new data arrives.
    pub fn pulse_border() -> Style {
        Style::default()
            .fg(Self::NEON_CYAN)
            .add_modifier(Modifier::BOLD)
    }

    pub fn inactive_border() -> Style {
        Style::default().fg(Self::BORDER_INACTIVE)
    }

    pub fn title_active() -> Style {
        Style::default()
            .fg(Self::NEON_MAGENTA)
            .add_modifier(Modifier::BOLD)
    }

    pub fn title_inactive() -> Style {
        Style::default().fg(Self::DIM)
    }

    pub fn spotlight() -> Style {
        Style::default()
            .fg(Self::NEON_YELLOW)
            .add_modifier(Modifier::BOLD)
    }

    pub fn status_key() -> Style {
        Style::default().fg(Self::NEON_MAGENTA)
    }

    /// Style for papers matching profile keywords.
    pub fn keyword_match() -> Style {
        Style::default()
            .fg(Self::KEYWORD_HIT)
            .add_modifier(Modifier::BOLD)
    }
}
