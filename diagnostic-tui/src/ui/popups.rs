//! Rendering logic for popup overlays: source picker, log file picker, and
//! help screen.

use crate::app::App;
use crate::ui::helpers::{HIGHLIGHT_BG, centered_rect, help_entry};

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState as RatatuiListState, Paragraph,
};

// ---------------------------------------------------------------------------
// Source picker
// ---------------------------------------------------------------------------

/// Draw the source component picker popup centred on the screen.
pub fn draw_source_picker(frame: &mut Frame, app: &mut App, area: Rect) {
    // Compute popup dimensions based on content.
    let max_source_len = app
        .source_filter
        .available
        .iter()
        .map(|s| s.len())
        .max()
        .unwrap_or(0);
    // Width: enough for the longest source name + padding + border.
    let content_width = (max_source_len + 6).max(24) as u16;
    let popup_width = content_width.min(area.width.saturating_sub(4));
    // Height: 1 "All Sources" + N sources + 2 border rows + 1 hint row.
    let item_count = 1 + app.source_filter.available.len();
    let popup_height = ((item_count + 4) as u16).min(area.height.saturating_sub(4));
    let popup_area = centered_rect(popup_width, popup_height, area);
    app.viewport.source_picker = popup_area.height.saturating_sub(2);

    frame.render_widget(Clear, popup_area);

    // Build list items.
    let mut items: Vec<ListItem> = Vec::with_capacity(item_count);

    // "All Sources" entry.
    let all_style = if app.source_picker_selected == 0 {
        Style::default()
            .bg(HIGHLIGHT_BG)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let all_prefix = if app.source_filter.selected.is_none() {
        "● "
    } else {
        "  "
    };
    items.push(ListItem::new(Line::from(vec![
        Span::styled(all_prefix, all_style),
        Span::styled("All Sources", all_style),
    ])));

    // Individual source entries.
    for (i, source) in app.source_filter.available.iter().enumerate() {
        let picker_idx = i + 1;
        let is_highlighted = app.source_picker_selected == picker_idx;
        let is_active = app.source_filter.selected == Some(i);

        let style = if is_highlighted {
            Style::default()
                .bg(HIGHLIGHT_BG)
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let prefix = if is_active { "● " } else { "  " };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(source.clone(), style),
        ])));
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta))
            .title(" Select Source ")
            .title_bottom(
                Line::from(" ↑↓/PgUp/PgDn navigate  Enter select  Esc close ")
                    .style(Style::default().fg(Color::DarkGray)),
            ),
    );

    let mut list_state =
        RatatuiListState::default().with_selected(Some(app.source_picker_selected));
    frame.render_stateful_widget(list, popup_area, &mut list_state);
}

// ---------------------------------------------------------------------------
// Log file picker
// ---------------------------------------------------------------------------

/// Draw the log file picker popup centred on the screen.
pub fn draw_log_file_picker(frame: &mut Frame, app: &mut App, area: Rect) {
    // Compute popup dimensions based on content.
    let max_name_len = app
        .log_file_filter
        .available
        .iter()
        .map(|s| s.len())
        .max()
        .unwrap_or(0);
    // Width: enough for the longest log file name + padding + border.
    let content_width = (max_name_len + 6).max(24) as u16;
    let popup_width = content_width.min(area.width.saturating_sub(4));
    // Height: 1 "All Log Files" + N log files + 2 border rows + 1 hint row.
    let item_count = 1 + app.log_file_filter.available.len();
    let popup_height = ((item_count + 4) as u16).min(area.height.saturating_sub(4));
    let popup_area = centered_rect(popup_width, popup_height, area);
    app.viewport.log_file_picker = popup_area.height.saturating_sub(2);

    frame.render_widget(Clear, popup_area);

    // Build list items.
    let mut items: Vec<ListItem> = Vec::with_capacity(item_count);

    // "All Log Files" entry.
    let all_style = if app.log_file_picker_selected == 0 {
        Style::default()
            .bg(HIGHLIGHT_BG)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let all_prefix = if app.log_file_filter.selected.is_none() {
        "● "
    } else {
        "  "
    };
    items.push(ListItem::new(Line::from(vec![
        Span::styled(all_prefix, all_style),
        Span::styled("All Log Files", all_style),
    ])));

    // Individual log file entries.
    for (i, log_file) in app.log_file_filter.available.iter().enumerate() {
        let picker_idx = i + 1;
        let is_highlighted = app.log_file_picker_selected == picker_idx;
        let is_active = app.log_file_filter.selected == Some(i);

        let style = if is_highlighted {
            Style::default()
                .bg(HIGHLIGHT_BG)
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let prefix = if is_active { "● " } else { "  " };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(log_file.clone(), style),
        ])));
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Select Log File ")
            .title_bottom(
                Line::from(" ↑↓/PgUp/PgDn navigate  Enter select  Esc close ")
                    .style(Style::default().fg(Color::DarkGray)),
            ),
    );

    let mut list_state =
        RatatuiListState::default().with_selected(Some(app.log_file_picker_selected));
    frame.render_stateful_widget(list, popup_area, &mut list_state);
}

// ---------------------------------------------------------------------------
// Help overlay
// ---------------------------------------------------------------------------

/// Draw the full-screen help overlay listing all keyboard shortcuts.
pub fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    // Centered popup.
    let popup_width = 65u16.min(area.width.saturating_sub(4));
    let popup_height = 54u16.min(area.height.saturating_sub(4));
    let popup_area = centered_rect(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let help_lines = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        help_entry("Tab / Shift+Tab", "Switch between tabs"),
        help_entry("1 / 2 / 3", "Jump to Overview / Logs / Crashes"),
        help_entry("Up/k  Down/j", "Move selection up / down"),
        help_entry("PgUp / PgDn", "Page up / down"),
        help_entry("Home/g  End/G", "Jump to first / last item"),
        help_entry("Enter/d/Right", "Open/focus detail pane"),
        help_entry("Esc/Left", "Close detail / return to list"),
        help_entry("zz", "Scroll cursor line to center"),
        help_entry("zt", "Scroll cursor line to top"),
        help_entry("zb", "Scroll cursor line to bottom"),
        Line::from(""),
        Line::from(Span::styled(
            " Logs Tab",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        help_entry("/", "Open search bar"),
        help_entry("Esc", "Clear search (when in normal mode)"),
        help_entry("Enter / Esc", "Close search bar (when searching)"),
        help_entry("f", "Cycle log level filter"),
        help_entry("s", "Cycle source component filter"),
        help_entry("S", "Open source picker"),
        help_entry("a", "Reset to all sources"),
        help_entry("l", "Cycle log file filter"),
        help_entry("L", "Open log file picker"),
        help_entry("A", "Combine all logs (reset filters)"),
        Line::from(""),
        Line::from(Span::styled(
            " Selection (Logs & Crashes)",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        help_entry("v", "Start visual selection at cursor"),
        help_entry("y", "Copy selection / current entry / line"),
        help_entry("Esc", "Cancel selection"),
        Line::from(""),
        Line::from(Span::styled(
            " Log Detail Pane",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        help_entry("Enter/d/Right", "Focus detail pane for navigation"),
        help_entry("Esc/Left", "Unfocus detail / close detail"),
        help_entry("v", "Start visual line selection in detail"),
        help_entry("y", "Copy selected lines / current line"),
        Line::from(""),
        Line::from(Span::styled(
            " Overview Tab",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        help_entry("v", "Start visual line selection"),
        help_entry("y", "Copy selected lines / current line"),
        help_entry("Esc", "Cancel selection"),
        Line::from(""),
        Line::from(Span::styled(
            " General",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        help_entry("?", "Toggle this help"),
        help_entry("q", "Quit"),
        help_entry("Ctrl+c", "Force quit"),
        Line::from(""),
        Line::from(Span::styled(
            " Press any key to close ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help = Paragraph::new(help_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Help "),
        )
        .alignment(Alignment::Left);

    frame.render_widget(help, popup_area);
}
