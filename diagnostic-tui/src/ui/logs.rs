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

impl LogsWidget<'_> {
    // ---------------------------------------------------------------------------
    // Log list
    // ---------------------------------------------------------------------------

    fn render_log_list(&self, state: &mut LogsState, area: Rect, buf: &mut Buffer) {
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
                let entry = &self.all_entries[entry_idx];
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

        let show_copied = self
            .copied_at
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2));

        let title = if show_copied {
            format!(" Logs — Copied {} entries! ✓ ", self.copied_count)
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

    fn render_log_detail(&self, state: &mut LogsState, area: Rect, buf: &mut Buffer) {
        // Get the selected entry data.
        let entry_data = state
            .list_state
            .selected()
            .and_then(|sel| state.filtered_indices.get(sel).copied())
            .and_then(|idx| self.all_entries.get(idx));

        let show_copied = self
            .copied_at
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2));
        let detail_sel = state.detail_selection_range();

        let title: Cow<'_, str> = if show_copied && state.detail_focused {
            format!(" Detail — Copied {} lines! ✓ ", self.copied_count).into()
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
            .border_style(Style::default().fg(if state.detail_focused {
                BORDER_FOCUSED
            } else {
                BORDER_NORMAL
            }))
            .title(Span::styled(title, title_style));

        let Some(entry_data) = entry_data else {
            Paragraph::new("No log entry selected")
                .style(Style::default().fg(Color::DarkGray))
                .block(block)
                .render(area, buf);
            return;
        };

        let mut lines = Vec::from([
            Line::from(vec![
                Span::styled("Level:     ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    entry_data.level.as_str(),
                    Style::default()
                        .fg(level_color(entry_data.level))
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Timestamp: ", Style::default().fg(Color::DarkGray)),
                Span::raw(entry_data.timestamp.with_timezone(&Local).to_string()),
            ]),
        ]);

        if !entry_data.thread.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Thread:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(&entry_data.thread),
            ]));
        }

        lines.push(Line::from(vec![
            Span::styled("Source:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry_data.source.raw(), Style::default().fg(Color::Magenta)),
        ]));

        if let Some(fp) = entry_data.source.file_path() {
            lines.push(Line::from(vec![
                Span::styled("File:      ", Style::default().fg(Color::DarkGray)),
                Span::raw(fp),
            ]));
        }

        if let Some(ln) = entry_data.source.line_number() {
            lines.push(Line::from(vec![
                Span::styled("Line:      ", Style::default().fg(Color::DarkGray)),
                Span::raw(ln.to_string()),
            ]));
        }

        lines.extend([
            Line::from(vec![
                Span::styled("Log File:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(&entry_data.log_file_title),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Message:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ]);

        lines.extend(
            entry_data
                .message
                .lines()
                .map(|line| Line::from(Span::raw(line))),
        );

        if entry_data.has_continuation() {
            lines.extend([
                Line::from(""),
                Line::from(Span::styled(
                    format!("Stack Trace ({} frames):", entry_data.continuation.len()),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ]);

            lines.extend(entry_data.continuation.iter().map(|cont_line| {
                Line::from(Span::styled(
                    cont_line,
                    if cont_line
                        .trim_start()
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit())
                    {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ))
            }));
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
                    let mut new_line = std::mem::take(line)
                        .style(Style::default().reversed().add_modifier(Modifier::BOLD));
                    std::mem::swap(&mut new_line, line);
                } else if is_in_selection {
                    let mut new_line = std::mem::take(line).style(Style::default().bg(SELECT_BG));
                    std::mem::swap(&mut new_line, line);
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
    // Search bar
    // ---------------------------------------------------------------------------

    fn render_search_bar(&self, state: &mut LogsState, area: Rect, buf: &mut Buffer) {
        let input_mode = self.input_mode;
        let (border_color, cursor_visible) = match input_mode {
            InputMode::Search => (Color::Yellow, true),
            InputMode::Normal | InputMode::Select => (BORDER_NORMAL, false),
        };

        Paragraph::new(
            if state.search_query.is_empty() && input_mode == InputMode::Normal {
                "Press / to search..."
            } else {
                state.search_query.as_str()
            },
        )
        .style(
            if state.search_query.is_empty() && input_mode == InputMode::Normal {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            },
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(
                    if !state.search_query.is_empty() && input_mode == InputMode::Normal {
                        " Search (n:next  N:prev  Esc:clear) "
                    } else {
                        " Search "
                    },
                ),
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

    fn render_filter_bar(&self, state: &LogsState, area: Rect, buf: &mut Buffer) {
        Paragraph::new(Line::from(Vec::from([
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
            Span::styled(
                format!(
                    " {} / {} entries",
                    state.filtered_indices.len(),
                    self.all_entries.len(),
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ])))
        .render(area, buf);
    }
}

impl StatefulWidget for LogsWidget<'_> {
    type State = LogsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut LogsState) {
        // Clear search cursor each frame.
        state.search_cursor_position = None;

        // Layout: search bar + filter bar on top, then split list / detail.
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // search bar
                Constraint::Length(1), // filter status line
                Constraint::Min(0),    // list + detail
            ])
            .split(area);

        self.render_search_bar(state, vertical[0], buf);
        self.render_filter_bar(state, vertical[1], buf);

        if state.show_detail {
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(vertical[2]);

            self.render_log_list(state, horizontal[0], buf);
            self.render_log_detail(state, horizontal[1], buf);
        } else {
            self.render_log_list(state, vertical[2], buf);
        }
    }
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
