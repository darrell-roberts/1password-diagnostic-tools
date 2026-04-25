//! Shared UI helper functions, color palette, and small utilities used
//! across all rendering modules.
use crate::app::filters::LevelFilter;
use diagnostic_parser::log_entry::LogLevel;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// Color palette

pub const SELECT_BG: Color = Color::Rgb(60, 60, 40);
pub const BORDER_FOCUSED: Color = Color::Cyan;
pub const BORDER_NORMAL: Color = Color::DarkGray;
pub const TAB_ACTIVE: Color = Color::Cyan;

/// Map a log level to its display color.
pub fn level_color(level: LogLevel) -> Color {
    match level {
        LogLevel::Trace => Color::DarkGray,
        LogLevel::Debug => Color::Cyan,
        LogLevel::Info => Color::Green,
        LogLevel::Warn => Color::Yellow,
        LogLevel::Error => Color::Red,
    }
}

/// Pick a color that represents the current level-filter state.
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

// Key-value line builders

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

// Help overlay entry

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

// Layout helpers

/// Return a centered `Rect` of the given size within `area`.
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
