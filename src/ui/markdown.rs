use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::ui::Theme;

/// Convert markdown-formatted text into styled ratatui Lines.
pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_code_block = false;

    for raw_line in text.lines() {
        let trimmed = raw_line.trim();

        // Code block toggle
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            if in_code_block {
                lines.push(Line::from(Span::styled(
                    "\u{2500}".repeat(40),
                    Theme::dim(),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "\u{2500}".repeat(40),
                    Theme::dim(),
                )));
            }
            continue;
        }

        if in_code_block {
            lines.push(Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(crate::ui::Theme::NEON_GREEN),
            )));
            continue;
        }

        // Headings
        if trimmed.starts_with("### ") {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                trimmed[4..].to_string(),
                Style::default()
                    .fg(Theme::NEON_CYAN)
                    .add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        if trimmed.starts_with("## ") {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                trimmed[3..].to_string(),
                Style::default()
                    .fg(Theme::NEON_CYAN)
                    .add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        if trimmed.starts_with("# ") {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                trimmed[2..].to_string(),
                Style::default()
                    .fg(Theme::NEON_CYAN)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            lines.push(Line::from(Span::styled(
                "\u{2500}".repeat(40),
                Theme::dim(),
            )));
            continue;
        }

        // Bullet lists
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let content = &trimmed[2..];
            let mut spans = vec![Span::styled("  \u{2022} ", Theme::accent())];
            spans.extend(parse_inline_markdown(content));
            lines.push(Line::from(spans));
            continue;
        }

        // Numbered lists
        if let Some(rest) = try_strip_numbered_prefix(trimmed) {
            let mut spans = vec![Span::styled(
                format!("  {}. ", &trimmed[..trimmed.len() - rest.len() - 1]),
                Theme::accent(),
            )];
            spans.extend(parse_inline_markdown(rest));
            lines.push(Line::from(spans));
            continue;
        }

        // Empty lines
        if trimmed.is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        // Regular text with inline formatting
        let spans = parse_inline_markdown(trimmed);
        lines.push(Line::from(spans));
    }

    lines
}

/// Parse inline markdown: **bold**, *italic*, `code`
fn parse_inline_markdown(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        // Bold: **text**
        if let Some(start) = remaining.find("**") {
            if start > 0 {
                spans.push(Span::styled(remaining[..start].to_string(), Theme::text()));
            }
            let after = &remaining[start + 2..];
            if let Some(end) = after.find("**") {
                spans.push(Span::styled(
                    after[..end].to_string(),
                    Style::default()
                        .fg(Theme::NEON_GREEN)
                        .add_modifier(Modifier::BOLD),
                ));
                remaining = &after[end + 2..];
                continue;
            } else {
                spans.push(Span::styled(remaining[start..].to_string(), Theme::text()));
                break;
            }
        }

        // Inline code: `code`
        if let Some(start) = remaining.find('`') {
            if start > 0 {
                spans.push(Span::styled(remaining[..start].to_string(), Theme::text()));
            }
            let after = &remaining[start + 1..];
            if let Some(end) = after.find('`') {
                spans.push(Span::styled(
                    after[..end].to_string(),
                    Style::default().fg(Theme::NEON_YELLOW),
                ));
                remaining = &after[end + 1..];
                continue;
            } else {
                spans.push(Span::styled(remaining[start..].to_string(), Theme::text()));
                break;
            }
        }

        // Italic: *text* (only if not **)
        if let Some(start) = remaining.find('*') {
            if start > 0 {
                spans.push(Span::styled(remaining[..start].to_string(), Theme::text()));
            }
            let after = &remaining[start + 1..];
            if let Some(end) = after.find('*') {
                spans.push(Span::styled(
                    after[..end].to_string(),
                    Style::default()
                        .fg(Theme::NEON_GREEN)
                        .add_modifier(Modifier::ITALIC),
                ));
                remaining = &after[end + 1..];
                continue;
            } else {
                spans.push(Span::styled(remaining[start..].to_string(), Theme::text()));
                break;
            }
        }

        // Plain text
        spans.push(Span::styled(remaining.to_string(), Theme::text()));
        break;
    }

    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), Theme::text()));
    }

    spans
}

fn try_strip_numbered_prefix(s: &str) -> Option<&str> {
    let mut chars = s.chars();
    let first = chars.next()?;
    if !first.is_ascii_digit() {
        return None;
    }
    // Skip more digits
    let rest = chars.as_str();
    let dot_pos = rest.find(". ")?;
    // Verify everything before dot is digits
    if rest[..dot_pos].chars().all(|c| c.is_ascii_digit()) {
        Some(&rest[dot_pos + 2..])
    } else {
        None
    }
}
