//! Rendering logic for the Logs tab: search bar, filter bar, log list, and
//! log detail pane.

use crate::app::{App, InputMode};
use crate::ui::helpers::{
    BORDER_FOCUSED, BORDER_NORMAL, HIGHLIGHT_BG, SELECT_BG, level_color, level_filter_color,
    truncate_str,
};
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

/// Draw the entire Logs tab content into the given area.
pub fn draw_logs(frame: &mut Frame, app: &mut App, area: Rect) {
    // Layout: search bar + filter bar on top, then split list / detail.
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search bar
            Constraint::Length(1), // filter status line
            Constraint::Min(0),    // list + detail
        ])
        .split(area);

    draw_search_bar(frame, app, vert[0]);
    draw_filter_bar(frame, app, vert[1]);

    if app.show_log_detail {
        // Horizontal split: log list (left) and detail (right).
        let horiz = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(vert[2]);

        draw_log_list(frame, app, horiz[0]);
        draw_log_detail(frame, app, horiz[1]);
    } else {
        // Full-width log list.
        draw_log_list(frame, app, vert[2]);
    }
}

// ---------------------------------------------------------------------------
// Search bar
// ---------------------------------------------------------------------------

fn draw_search_bar(frame: &mut Frame, app: &App, area: Rect) {
    let (border_color, cursor_visible) = match app.input_mode {
        InputMode::Search => (Color::Yellow, true),
        InputMode::Normal | InputMode::Select => (BORDER_NORMAL, false),
    };

    let search_text = if app.search_query.is_empty() && app.input_mode == InputMode::Normal {
        "Press / to search...".to_string()
    } else {
        app.search_query.clone()
    };

    let style = if app.search_query.is_empty() && app.input_mode == InputMode::Normal {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    let input = Paragraph::new(search_text.as_str()).style(style).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" Search "),
    );

    frame.render_widget(input, area);

    if cursor_visible {
        // Position cursor at end of input.
        let cursor_x = area.x + 1 + app.search_query.len() as u16;
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x.min(area.x + area.width - 2), cursor_y));
    }
}

// ---------------------------------------------------------------------------
// Filter bar
// ---------------------------------------------------------------------------

fn draw_filter_bar(frame: &mut Frame, app: &App, area: Rect) {
    let count_text = format!(
        " {} / {} entries",
        app.filtered_indices.len(),
        app.all_entries.len(),
    );

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(" f ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(
            format!(" Level: {} ", app.level_filter.label()),
            Style::default().fg(level_filter_color(&app.level_filter)),
        ),
        Span::raw("  "),
        Span::styled(" s ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(
            format!(" Source: {} ", app.source_filter.label()),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw("  "),
        Span::styled(" S ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(" Pick source ", Style::default().fg(Color::Magenta)),
        Span::raw("  "),
        Span::styled(" l ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(
            format!(" Log File: {} ", app.log_file_filter.label()),
            Style::default().fg(Color::Blue),
        ),
        Span::raw("  "),
        Span::styled(" L ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(" Pick log file ", Style::default().fg(Color::Blue)),
        Span::raw("  "),
        Span::styled(" A ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(" All logs ", Style::default().fg(Color::Blue)),
        Span::styled(count_text, Style::default().fg(Color::DarkGray)),
    ]);

    let bar = Paragraph::new(line);
    frame.render_widget(bar, area);
}

// ---------------------------------------------------------------------------
// Log list
// ---------------------------------------------------------------------------

fn draw_log_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_color = if app.show_log_detail && app.detail_focused {
        BORDER_NORMAL
    } else {
        BORDER_FOCUSED
    };

    let inner_height = area.height.saturating_sub(2) as usize;
    app.viewport.log_list = inner_height as u16;
    app.log_list_state.ensure_visible(inner_height);

    let selection_range = app.selection_range();

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .skip(app.log_list_state.offset)
        .take(inner_height)
        .map(|(display_idx, &entry_idx)| {
            let entry = &app.all_entries[entry_idx];
            let is_cursor = display_idx == app.log_list_state.selected;
            let is_in_selection = selection_range
                .is_some_and(|(start, end)| display_idx >= start && display_idx <= end);

            let level_span = Span::styled(
                format!("{:<5}", entry.level),
                Style::default().fg(level_color(entry.level)),
            );

            let ts = entry.timestamp.format("%H:%M:%S%.3f");
            let ts_span = Span::styled(format!(" {ts} "), Style::default().fg(Color::DarkGray));

            // Truncate message to fit.
            let avail = (area.width as usize).saturating_sub(18);
            let msg = truncate_str(&entry.message, avail);
            let msg_span = Span::styled(msg, Style::default().fg(Color::White));

            let mut style = Style::default();
            if is_cursor {
                style = style.bg(HIGHLIGHT_BG).add_modifier(Modifier::BOLD);
            } else if is_in_selection {
                style = style.bg(SELECT_BG);
            }

            let continuation_marker = if entry.has_continuation() {
                Span::styled(" +", Style::default().fg(Color::Magenta))
            } else {
                Span::raw("")
            };

            ListItem::new(Line::from(vec![
                level_span,
                ts_span,
                msg_span,
                continuation_marker,
            ]))
            .style(style)
        })
        .collect();

    // Show "Copied!" flash or selection count in the title.
    let show_copied = app
        .copied_at
        .is_some_and(|t| t.elapsed() < Duration::from_secs(2));

    let title = if show_copied {
        let count = selection_range.map_or(1, |(s, e)| e - s + 1);
        format!(" Logs — Copied {count} entries! ✓ ")
    } else if let Some((start, end)) = selection_range {
        let count = end - start + 1;
        format!(
            " Logs [{}/{}] — {} selected (y:copy  Esc:cancel) ",
            if app.filtered_indices.is_empty() {
                0
            } else {
                app.log_list_state.selected + 1
            },
            app.filtered_indices.len(),
            count,
        )
    } else {
        format!(
            " Logs [{}/{}] ",
            if app.filtered_indices.is_empty() {
                0
            } else {
                app.log_list_state.selected + 1
            },
            app.filtered_indices.len(),
        )
    };

    let title_style = if show_copied {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if selection_range.is_some() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(title, title_style)),
    );

    frame.render_widget(list, area);
}

// ---------------------------------------------------------------------------
// Log detail pane
// ---------------------------------------------------------------------------

fn draw_log_detail(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_color = if app.detail_focused {
        BORDER_FOCUSED
    } else {
        BORDER_NORMAL
    };

    // Clone entry data to avoid borrow conflicts with app.detail_scroll.
    let entry_data = app.selected_log_entry().map(|entry| {
        (
            entry.level,
            entry.timestamp.to_string(),
            entry.thread.clone(),
            entry.source.raw(),
            entry.source.file_path().map(|s| s.to_owned()),
            entry.source.line_number(),
            entry.log_file_title.clone(),
            entry.message.clone(),
            entry.has_continuation(),
            entry.continuation.clone(),
        )
    });

    // Build title with selection / copied feedback.
    let show_copied = app
        .copied_at
        .is_some_and(|t| t.elapsed() < Duration::from_secs(2));
    let detail_sel = app.detail_selection_range();

    let title = if show_copied && app.detail_focused {
        let count = detail_sel.map_or(1, |(s, e)| e - s + 1);
        format!(" Detail — Copied {count} lines! ✓ ")
    } else if let Some((start, end)) = detail_sel {
        let count = end - start + 1;
        format!(" Detail — {} selected (y:copy  Esc:cancel) ", count)
    } else if app.detail_focused {
        " Detail (focused) ".to_string()
    } else {
        " Detail ".to_string()
    };

    let title_style = if show_copied && app.detail_focused {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if detail_sel.is_some() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(title, title_style));

    let Some((
        level,
        timestamp,
        thread,
        source_raw,
        file_path,
        line_number,
        log_file_title,
        message,
        has_continuation,
        continuation,
    )) = entry_data
    else {
        let empty = Paragraph::new("No log entry selected")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(empty, area);
        return;
    };

    // Build detail text as styled Lines, one per logical line.
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("Level:     ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            level.as_str().to_string(),
            Style::default()
                .fg(level_color(level))
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Timestamp: ", Style::default().fg(Color::DarkGray)),
        Span::raw(timestamp),
    ]));

    if !thread.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Thread:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(thread),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("Source:    ", Style::default().fg(Color::DarkGray)),
        Span::styled(source_raw, Style::default().fg(Color::Magenta)),
    ]));

    if let Some(fp) = file_path {
        lines.push(Line::from(vec![
            Span::styled("File:      ", Style::default().fg(Color::DarkGray)),
            Span::raw(fp),
        ]));
    }

    if let Some(ln) = line_number {
        lines.push(Line::from(vec![
            Span::styled("Line:      ", Style::default().fg(Color::DarkGray)),
            Span::raw(ln.to_string()),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("Log File:  ", Style::default().fg(Color::DarkGray)),
        Span::raw(log_file_title),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Message:",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Wrap message lines manually for display.
    for msg_line in message.lines() {
        lines.push(Line::from(Span::raw(msg_line.to_string())));
    }

    if has_continuation {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Stack Trace ({} frames):", continuation.len()),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for cont_line in &continuation {
            let style = if cont_line
                .trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
            {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(Span::styled(cont_line.clone(), style)));
        }
    }

    // Track line count so the app can clamp cursor navigation.
    let total_lines = lines.len();
    app.detail_line_count = total_lines;

    // Clamp cursor.
    if total_lines > 0 && app.detail_cursor >= total_lines {
        app.detail_cursor = total_lines - 1;
    }

    // Apply cursor / selection highlighting when the detail pane is focused.
    if app.detail_focused {
        let selection_range = app.detail_selection_range();
        for (i, line) in lines.iter_mut().enumerate() {
            let is_cursor = i == app.detail_cursor;
            let is_in_selection =
                selection_range.is_some_and(|(start, end)| i >= start && i <= end);

            if is_cursor {
                // Patch each span in the line with the highlight background.
                *line = Line::from(
                    line.spans
                        .iter()
                        .map(|span| {
                            Span::styled(
                                span.content.clone(),
                                span.style.bg(HIGHLIGHT_BG).add_modifier(Modifier::BOLD),
                            )
                        })
                        .collect::<Vec<_>>(),
                );
            } else if is_in_selection {
                *line = Line::from(
                    line.spans
                        .iter()
                        .map(|span| Span::styled(span.content.clone(), span.style.bg(SELECT_BG)))
                        .collect::<Vec<_>>(),
                );
            }
        }
    }

    // Clamp scroll.
    let inner_height = area.height.saturating_sub(2) as usize;
    app.viewport.log_detail = inner_height as u16;
    let max_scroll = total_lines.saturating_sub(inner_height);
    if (app.detail_scroll as usize) > max_scroll {
        app.detail_scroll = max_scroll as u16;
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));

    frame.render_widget(paragraph, area);
}
