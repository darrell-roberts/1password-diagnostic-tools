//! Rendering logic for the Logs tab: search bar, filter bar, log list, and
//! log detail pane.

use crate::{
    app::{InputMode, LogsState},
    ui::helpers::{BORDER_FOCUSED, BORDER_NORMAL, SELECT_BG, level_color, level_filter_color},
};
use chrono::Local;
use diagnostic_parser::log_entry::LogEntry;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Table, Widget as _, Wrap,
    },
};
use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

/// Widget for the Logs tab, holding borrowed immutable data.
pub struct LogsWidget<'a> {
    pub all_entries: &'a [LogEntry],
    pub input_mode: InputMode,
    pub copied_at: Option<Instant>,
    pub copied_count: usize,
}

impl StatefulWidget for LogsWidget<'_> {
    type State = LogsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut LogsState) {
        // Clear search cursor each frame.
        state.search_cursor_position = None;

        // Layout: search bar + filter bar on top, then split list / detail.
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // search bar
                Constraint::Length(1), // filter status line
                Constraint::Min(0),    // list + detail
            ])
            .split(area);

        render_search_bar(state, self.input_mode, vert[0], buf);
        render_filter_bar(state, self.all_entries.len(), self.input_mode, vert[1], buf);

        if state.show_detail {
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(vert[2]);

            render_log_list(
                state,
                self.all_entries,
                self.copied_at,
                self.copied_count,
                horizontal[0],
                buf,
            );
            render_log_detail(
                state,
                self.all_entries,
                self.copied_at,
                self.copied_count,
                horizontal[1],
                buf,
            );
        } else {
            render_log_list(
                state,
                self.all_entries,
                self.copied_at,
                self.copied_count,
                vert[2],
                buf,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Search bar
// ---------------------------------------------------------------------------

fn render_search_bar(state: &mut LogsState, input_mode: InputMode, area: Rect, buf: &mut Buffer) {
    let (border_color, cursor_visible) = match input_mode {
        InputMode::Search => (Color::Yellow, true),
        InputMode::Normal | InputMode::Select => (BORDER_NORMAL, false),
    };

    let search_text: Cow<'_, str> =
        if state.search_query.is_empty() && input_mode == InputMode::Normal {
            "Press / to search...".into()
        } else {
            state.search_query.as_str().into()
        };

    let style = if state.search_query.is_empty() && input_mode == InputMode::Normal {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    let title = if !state.search_query.is_empty() && input_mode == InputMode::Normal {
        " Search (n:next  N:prev  Esc:clear) "
    } else {
        " Search "
    };

    Paragraph::new(search_text)
        .style(style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(title),
        )
        .render(area, buf);

    if cursor_visible {
        let cursor_x = area.x + 1 + state.search_query.len() as u16;
        let cursor_y = area.y + 1;
        state.search_cursor_position = Some((cursor_x.min(area.x + area.width - 2), cursor_y));
    }
}

// ---------------------------------------------------------------------------
// Filter bar
// ---------------------------------------------------------------------------

fn render_filter_bar(
    state: &LogsState,
    total_entries: usize,
    input_mode: InputMode,
    area: Rect,
    buf: &mut Buffer,
) {
    let _ = input_mode; // kept for future use

    let count_text = format!(
        " {} / {} entries",
        state.filtered_indices.len(),
        total_entries,
    );

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(" f ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(
            format!(" Level: {} ", state.level_filter.label()),
            Style::default().fg(level_filter_color(&state.level_filter)),
        ),
        Span::raw("  "),
        Span::styled(" s ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(
            format!(" Source: {} ", state.source_filter.label()),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw("  "),
        Span::styled(" S ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(" Pick source ", Style::default().fg(Color::Magenta)),
        Span::raw("  "),
        Span::styled(" l ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(
            format!(" Log File: {} ", state.log_file_filter.label()),
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

    Paragraph::new(line).render(area, buf);
}

// ---------------------------------------------------------------------------
// Log list
// ---------------------------------------------------------------------------

fn render_log_list(
    state: &mut LogsState,
    all_entries: &[LogEntry],
    copied_at: Option<Instant>,
    copied_count: usize,
    area: Rect,
    buf: &mut Buffer,
) {
    let border_color = if state.show_detail && state.detail_focused {
        BORDER_NORMAL
    } else {
        BORDER_FOCUSED
    };

    let inner_height = area.height.saturating_sub(2) as usize;
    state.list_viewport_height = inner_height as u16;

    let total = state.filtered_indices.len();
    let selection_range = state.selection_range();

    // Virtual scrolling: only build Row objects for the visible window.
    if inner_height > 0 && total > 0 {
        let selected = state.list_state.selected().unwrap_or(0).min(total - 1);
        state.list_state.select(Some(selected));

        let offset = state.list_state.offset();
        let new_offset = if selected < offset {
            selected
        } else if selected >= offset + inner_height {
            selected + 1 - inner_height
        } else {
            offset
        };
        *state.list_state.offset_mut() = new_offset;
    }

    let offset = state.list_state.offset();
    let visible_start = offset;
    let visible_end = offset.saturating_add(inner_height + 1).min(total);

    let query_lower = state.search_query.to_lowercase();
    let highlight_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let items = state.filtered_indices[visible_start..visible_end]
        .iter()
        .enumerate()
        .map(|(local_idx, &entry_idx)| {
            let display_idx = visible_start + local_idx;
            let entry = &all_entries[entry_idx];
            let is_in_selection = selection_range
                .is_some_and(|(start, end)| display_idx >= start && display_idx <= end);

            let msg_spans = if !query_lower.is_empty() {
                highlight_matches(
                    &entry.message,
                    &query_lower,
                    Style::default().fg(Color::White),
                    highlight_style,
                )
            } else {
                vec![Span::styled(
                    &entry.message,
                    Style::default().fg(Color::White),
                )]
            };

            let mut style = Style::default();
            if is_in_selection {
                style = style.bg(SELECT_BG);
            }

            let continuation_marker = if entry.has_continuation() {
                Span::styled(" +", Style::default().fg(Color::Magenta))
            } else {
                Span::raw("")
            };

            Row::new([
                Cell::from(Span::styled(
                    entry.level.as_str(),
                    Style::default().fg(level_color(entry.level)),
                )),
                Cell::from(Span::styled(
                    entry
                        .timestamp
                        .with_timezone(&Local)
                        .format("%H:%M:%S%.3f")
                        .to_string(),
                    Style::default().fg(Color::DarkGray),
                )),
                Cell::from(Line::from(msg_spans)),
                Cell::from(continuation_marker),
            ])
            .style(style)
        })
        .collect::<Vec<_>>();

    let show_copied = copied_at.is_some_and(|t| t.elapsed() < Duration::from_secs(2));

    let title = if show_copied {
        format!(" Logs — Copied {copied_count} entries! ✓ ")
    } else if let Some((start, end)) = selection_range {
        let count = end - start + 1;
        format!(
            " Logs [{}/{}] — {} selected (y:copy  Esc:cancel) ",
            if total == 0 {
                0
            } else {
                state
                    .list_state
                    .selected()
                    .map(|i| i + 1)
                    .unwrap_or_default()
            },
            total,
            count,
        )
    } else {
        format!(
            " Logs [{}/{}] ",
            if total == 0 {
                0
            } else {
                state
                    .list_state
                    .selected()
                    .map(|i| i.saturating_add(1).clamp(0, total))
                    .unwrap_or_default()
            },
            total,
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

    let widths = [
        Constraint::Length(5),
        Constraint::Length(12),
        Constraint::Fill(1),
        Constraint::Length(2),
    ];

    // Render only the visible slice with local indices for the Table widget.
    let saved_selected = state.list_state.selected();
    let local_selected = saved_selected.and_then(|s| s.checked_sub(visible_start));
    state.list_state.select(local_selected);
    *state.list_state.offset_mut() = 0;

    let table = Table::new(items, widths)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(Span::styled(title, title_style)),
        )
        .row_highlight_style(Style::new().reversed());

    StatefulWidget::render(table, area, buf, &mut state.list_state);

    // Restore global state after rendering.
    state.list_state.select(saved_selected);
    *state.list_state.offset_mut() = offset;

    // Rebuild scrollbar state each frame from the selected position.
    state.list_scrollbar = ScrollbarState::new(total).position(saved_selected.unwrap_or(0));
    Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"))
        .render(
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            buf,
            &mut state.list_scrollbar,
        );
}

// ---------------------------------------------------------------------------
// Log detail pane
// ---------------------------------------------------------------------------

fn render_log_detail(
    state: &mut LogsState,
    all_entries: &[LogEntry],
    copied_at: Option<Instant>,
    copied_count: usize,
    area: Rect,
    buf: &mut Buffer,
) {
    let border_color = if state.detail_focused {
        BORDER_FOCUSED
    } else {
        BORDER_NORMAL
    };

    // Get the selected entry data.
    let entry_data = state
        .list_state
        .selected()
        .and_then(|sel| state.filtered_indices.get(sel).copied())
        .and_then(|idx| all_entries.get(idx))
        .map(|entry| {
            (
                entry.level,
                entry.timestamp.with_timezone(&Local).to_string(),
                entry.thread.clone(),
                entry.source.raw().into_owned(),
                entry.source.file_path().map(|s| s.to_owned()),
                entry.source.line_number(),
                entry.log_file_title.clone(),
                entry.message.clone(),
                entry.has_continuation(),
                entry.continuation.clone(),
            )
        });

    let show_copied = copied_at.is_some_and(|t| t.elapsed() < Duration::from_secs(2));
    let detail_sel = state.detail_selection_range();

    let title: Cow<'_, str> = if show_copied && state.detail_focused {
        format!(" Detail — Copied {copied_count} lines! ✓ ").into()
    } else if let Some((start, end)) = detail_sel {
        let count = end - start + 1;
        format!(" Detail — {} selected (y:copy  Esc:cancel) ", count).into()
    } else if state.detail_focused {
        " Detail (focused) ".into()
    } else {
        " Detail ".into()
    };

    let title_style = if show_copied && state.detail_focused {
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
        Paragraph::new("No log entry selected")
            .style(Style::default().fg(Color::DarkGray))
            .block(block)
            .render(area, buf);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("Level:     ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            level.as_str(),
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

    for msg_line in message.lines() {
        lines.push(Line::from(Span::raw(msg_line)));
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

    let total_lines = lines.len();
    state.detail_line_count = total_lines;

    if total_lines > 0 && state.detail_cursor >= total_lines {
        state.detail_cursor = total_lines - 1;
    }

    if state.detail_focused {
        let selection_range = state.detail_selection_range();
        for (i, line) in lines.iter_mut().enumerate() {
            let is_cursor = i == state.detail_cursor;
            let is_in_selection =
                selection_range.is_some_and(|(start, end)| i >= start && i <= end);

            if is_cursor {
                *line = Line::from(
                    line.spans
                        .iter()
                        .map(|span| {
                            Span::styled(
                                span.content.clone(),
                                span.style.reversed().add_modifier(Modifier::BOLD),
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

    let inner_height = area.height.saturating_sub(2) as usize;
    state.detail_viewport_height = inner_height as u16;
    let max_scroll = total_lines.saturating_sub(inner_height);
    if (state.detail_scroll as usize) > max_scroll {
        state.detail_scroll = max_scroll as u16;
    }

    Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((state.detail_scroll, 0))
        .render(area, buf);
}

// ---------------------------------------------------------------------------
// Search highlighting
// ---------------------------------------------------------------------------

fn highlight_matches<'a>(
    text: &'a str,
    query_lower: &str,
    base_style: Style,
    match_style: Style,
) -> Vec<Span<'a>> {
    let text_lower = text.to_lowercase();
    let mut spans = Vec::new();
    let mut start = 0;

    while let Some(pos) = text_lower[start..].find(query_lower) {
        let match_start = start + pos;
        let match_end = match_start + query_lower.len();

        if match_start > start {
            spans.push(Span::styled(&text[start..match_start], base_style));
        }
        spans.push(Span::styled(&text[match_start..match_end], match_style));
        start = match_end;
    }

    if start < text.len() {
        spans.push(Span::styled(&text[start..], base_style));
    } else if spans.is_empty() {
        spans.push(Span::styled(text, base_style));
    }

    spans
}
