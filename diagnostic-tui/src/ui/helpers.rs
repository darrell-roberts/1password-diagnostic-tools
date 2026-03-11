//! Shared UI helper functions, colour palette, and small utilities used
//! across all rendering modules.

use crate::app::filters::LevelFilter;
use diagnostic_parser::log_entry::LogLevel;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

// ---------------------------------------------------------------------------
// Colour palette
// ---------------------------------------------------------------------------

pub const HIGHLIGHT_BG: Color = Color::Rgb(50, 50, 80);
pub const SELECT_BG: Color = Color::Rgb(60, 60, 40);
pub const BORDER_FOCUSED: Color = Color::Cyan;
pub const BORDER_NORMAL: Color = Color::DarkGray;
pub const TAB_ACTIVE: Color = Color::Cyan;

/// Map a log level to its display colour.
pub fn level_color(level: LogLevel) -> Color {
    match level {
        LogLevel::Trace => Color::DarkGray,
        LogLevel::Debug => Color::Cyan,
        LogLevel::Info => Color::Green,
        LogLevel::Warn => Color::Yellow,
        LogLevel::Error => Color::Red,
    }
}

/// Pick a colour that represents the current level-filter state.
pub fn level_filter_color(filter: &LevelFilter) -> Color {
    if filter.show_trace {
        Color::White
    } else if filter.show_debug {
        Color::Cyan
    } else if filter.show_info {
        Color::Green
    } else if filter.show_warn {
        Color::Yellow
    } else {
        Color::Red
    }
}

// ---------------------------------------------------------------------------
// Key-value line builders
// ---------------------------------------------------------------------------

/// Create a key-value line with default indentation (2 spaces).
pub fn kv_line(key: &str, value: &str) -> Line<'static> {
    kv_line_indent(2, key, value)
}

/// Create a key-value line with the specified indentation.
pub fn kv_line_indent(indent: usize, key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw(" ".repeat(indent)),
        Span::styled(
            format!("{key}: "),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(value.to_owned()),
    ])
}

// ---------------------------------------------------------------------------
// Help overlay entry
// ---------------------------------------------------------------------------

/// Build a single help-overlay line: a highlighted key label followed by a
/// description.
pub fn help_entry<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::raw("   "),
        Span::styled(
            format!("{:<18}", key),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(desc),
    ])
}

// ---------------------------------------------------------------------------
// String / byte formatting
// ---------------------------------------------------------------------------

/// Truncate a string to at most `max_len` characters, appending `...` when
/// truncation occurs.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_owned()
    } else if max_len <= 1 {
        "...".to_owned()
    } else {
        let mut end = max_len - 1;
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

/// Format a byte count as a human-readable string (KB / MB / GB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

/// Return a centered `Rect` of the given size within `area`.
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
