//! Rendering logic for the Crash Reports tab: crash list and crash detail pane.

use crate::{
    app::{CrashReportsState, Tab},
    ui::helpers::{BORDER_FOCUSED, BORDER_NORMAL, SELECT_BG},
};
use chrono::Local;
use diagnostic_parser::{
    log_entry::LogEntry,
    model::{CrashReportEntry, DiagnosticReport},
};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, StatefulWidget, Table, Widget as _, Wrap},
};
use std::time::{Duration, Instant};

/// Widget for the Crash Reports tab, holding borrowed immutable data.
pub struct CrashReportsWidget<'a> {
    pub report: &'a DiagnosticReport,
    pub all_entries: &'a [LogEntry],
    pub tab: Tab,
    pub copied_at: Option<Instant>,
    pub copied_count: usize,
}

impl StatefulWidget for CrashReportsWidget<'_> {
    type State = CrashReportsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut CrashReportsState) {
        if self.report.crash_report_entries.is_empty() {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_FOCUSED))
                .title(" Crash Reports ");

            Paragraph::new("No crash reports in this diagnostic file.")
                .style(Style::default().fg(Color::DarkGray))
                .block(block)
                .alignment(Alignment::Center)
                .render(area, buf);
            return;
        }

        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(area);

        render_crash_list(
            state,
            &self.report.crash_report_entries,
            self.tab,
            self.copied_at,
            self.copied_count,
            horizontal[0],
            buf,
        );
        render_crash_detail(
            state,
            &self.report.crash_report_entries,
            self.all_entries,
            self.copied_at,
            self.copied_count,
            horizontal[1],
            buf,
        );
    }
}

// ---------------------------------------------------------------------------
// Crash list
// ---------------------------------------------------------------------------

fn render_crash_list(
    state: &mut CrashReportsState,
    crash_entries: &[CrashReportEntry],
    tab: Tab,
    copied_at: Option<Instant>,
    copied_count: usize,
    area: Rect,
    buf: &mut Buffer,
) {
    let border_color = if !state.detail_focused {
        BORDER_FOCUSED
    } else {
        BORDER_NORMAL
    };

    let inner_height = area.height.saturating_sub(2) as usize;
    state.list_viewport_height = inner_height as u16;

    let crash_selection_range = state.selection_range();

    let items = crash_entries
        .iter()
        .enumerate()
        .map(|(idx, crash)| {
            let is_in_selection =
                crash_selection_range.is_some_and(|(start, end)| idx >= start && idx <= end);

            let ts = crash
                .timestamp_utc()
                .map(|ts| {
                    ts.with_timezone(&Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(|| format!("{}", crash.timestamp));

            let type_span = Span::styled(
                &crash.report_type,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            );

            let ts_span = Span::styled(ts, Style::default().fg(Color::DarkGray));
            let id_span = Span::styled(&crash.report_id, Style::default().fg(Color::White));

            let mut style = Style::default();
            if is_in_selection {
                style = Style::new().reversed();
            }

            Row::new([type_span, ts_span, id_span]).style(style)
        })
        .collect::<Vec<_>>();

    let show_copied =
        tab == Tab::CrashReports && copied_at.is_some_and(|t| t.elapsed() < Duration::from_secs(2));

    let title = if show_copied {
        format!(" Crashes — Copied {copied_count} entries! ✓ ")
    } else if let Some((start, end)) = crash_selection_range {
        let count = end - start + 1;
        format!(
            " Crashes [{}/{}] — {} selected (y:copy  Esc:cancel) ",
            if crash_entries.is_empty() {
                0
            } else {
                state.list_state.selected().map(|i| i + 1).unwrap_or(1)
            },
            crash_entries.len(),
            count,
        )
    } else {
        format!(
            " Crashes [{}/{}] ",
            state
                .list_state
                .selected()
                .unwrap_or_default()
                .clamp(0, crash_entries.len() - 1)
                + 1,
            crash_entries.len(),
        )
    };

    let title_style = if show_copied {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if crash_selection_range.is_some() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let widths = [
        Constraint::Length(6),
        Constraint::Length(19),
        Constraint::Fill(1),
    ];

    let list = Table::new(items, widths)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(Span::styled(title, title_style)),
        )
        .row_highlight_style(Style::new().reversed());

    StatefulWidget::render(list, area, buf, &mut state.list_state);
}

// ---------------------------------------------------------------------------
// Crash detail pane
// ---------------------------------------------------------------------------

fn render_crash_detail(
    state: &mut CrashReportsState,
    crash_entries: &[CrashReportEntry],
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

    // Get selected crash data.
    let crash_data = state
        .list_state
        .selected()
        .and_then(|sel| crash_entries.get(sel))
        .map(|crash| {
            let ts = crash
                .timestamp_utc()
                .map(|d| {
                    d.with_timezone(&Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(|| format!("{}", crash.timestamp));
            (
                crash.report_id.clone(),
                crash.report_type.clone(),
                ts,
                crash.diagnostic_report_tag.clone(),
                crash.find_panic_entry(all_entries),
            )
        });

    let show_copied = copied_at.is_some_and(|t| t.elapsed() < Duration::from_secs(2));
    let detail_sel = state.detail_selection_range();

    let (detail_title, detail_title_style) = if show_copied && state.detail_focused {
        (
            format!(" Crash Detail — Copied {copied_count} lines! ✓ "),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else if let Some((start, end)) = detail_sel {
        let count = end - start + 1;
        (
            format!(" Crash Detail — {} selected (y:copy  Esc:cancel) ", count),
            Style::default().fg(Color::Yellow),
        )
    } else if state.detail_focused {
        (" Crash Detail (focused) ".to_string(), Style::default())
    } else {
        (" Crash Detail ".to_string(), Style::default())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(detail_title, detail_title_style));

    let Some((report_id, report_type, ts, tag, panic_entry)) = crash_data else {
        Paragraph::new("No crash report selected")
            .style(Style::default().fg(Color::DarkGray))
            .block(block)
            .render(area, buf);
        return;
    };

    let mut lines = Vec::from([
        Line::from(vec![
            Span::styled("Report ID: ", Style::default().fg(Color::DarkGray)),
            Span::raw(report_id),
        ]),
        Line::from(vec![
            Span::styled("Type:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                report_type,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Timestamp: ", Style::default().fg(Color::DarkGray)),
            Span::raw(ts),
        ]),
        Line::from(vec![
            Span::styled("Tag:       ", Style::default().fg(Color::DarkGray)),
            Span::raw(tag),
        ]),
        Line::from(""),
    ]);

    if let Some(entry) = panic_entry {
        lines.extend([
            Line::from(Span::styled(
                "Linked Panic Entry",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Log File:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(entry.log_file_title.clone()),
            ]),
            Line::from(vec![
                Span::styled("Thread:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(entry.thread.clone()),
            ]),
            Line::from(vec![
                Span::styled("Source:    ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    entry.source.raw().into_owned(),
                    Style::default().fg(Color::Magenta),
                ),
            ]),
            Line::from(vec![
                Span::styled("Timestamp: ", Style::default().fg(Color::DarkGray)),
                Span::raw(entry.timestamp.with_timezone(&Local).to_string()),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Message:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ]);

        lines.extend(
            entry
                .message
                .lines()
                .map(|line| Line::from(Span::raw(line))),
        );

        if entry.has_continuation() {
            lines.extend([
                Line::from(""),
                Line::from(Span::styled(
                    format!("Call Stack ({} frames):", entry.continuation.len()),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ]);

            lines.extend(
                entry
                    .continuation
                    .iter()
                    .enumerate()
                    .map(|(i, frame_line)| {
                        let trimmed = frame_line.trim_start();
                        let fg = if i % 2 == 0 {
                            Color::Yellow
                        } else {
                            Color::White
                        };
                        Line::from(Span::styled(trimmed, Style::default().fg(fg)))
                    }),
            );
        } else {
            lines.extend([
                Line::from(""),
                Line::from(Span::styled(
                    "(no stack trace attached to panic entry)",
                    Style::default().fg(Color::DarkGray),
                )),
            ]);
        }
    } else {
        lines.extend([
            Line::from(Span::styled(
                "No matching panic log entry found.",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "The crash report could not be correlated with a panic log entry.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "This may happen if the log file has been rotated or the crash",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "occurred outside the captured log window.",
                Style::default().fg(Color::DarkGray),
            )),
        ]);
    }

    let total_lines = lines.len();
    state.detail_line_count = total_lines;

    // Build plain cache for copy operations.
    if let Some(entry) = panic_entry {
        let crash = state
            .list_state
            .selected()
            .and_then(|sel| crash_entries.get(sel));
        if let Some(crash) = crash {
            state.detail_plain_cache = Some(crash_report_plain_lines(crash, Some(entry)));
        }
    } else {
        let crash = state
            .list_state
            .selected()
            .and_then(|sel| crash_entries.get(sel));
        if let Some(crash) = crash {
            state.detail_plain_cache = Some(crash_report_plain_lines(crash, None));
        }
    }

    if total_lines > 0 && state.detail_cursor >= total_lines {
        state.detail_cursor = total_lines - 1;
    }

    if state.detail_focused {
        let selection_range = state.detail_selection_range();
        // Update line styles.
        for (i, line) in lines.iter_mut().enumerate() {
            let is_cursor = i == state.detail_cursor;
            let is_in_selection =
                selection_range.is_some_and(|(start, end)| i >= start && i <= end);

            if is_cursor {
                let mut new_line = std::mem::take(line)
                    .style(Style::new().reversed().add_modifier(Modifier::BOLD));
                std::mem::swap(line, &mut new_line);
            } else if is_in_selection {
                let mut new_line = std::mem::take(line).style(Style::default().bg(SELECT_BG));
                std::mem::swap(line, &mut new_line);
            }
        }
    }

    let inner_height = area.height.saturating_sub(2) as usize;
    state.detail_viewport_height = inner_height as u16;
    let max_scroll = lines.len().saturating_sub(inner_height);
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
// Plain-text builder (used for cache during render)
// ---------------------------------------------------------------------------

fn crash_report_plain_lines(
    crash: &CrashReportEntry,
    panic_entry: Option<&LogEntry>,
) -> Vec<String> {
    let ts = crash
        .timestamp_utc()
        .map(|d| {
            d.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| format!("{}", crash.timestamp));

    let mut lines = vec![
        format!("Report ID: {}", crash.report_id),
        format!("Type:      {}", crash.report_type),
        format!("Timestamp: {}", ts),
        format!("Tag:       {}", crash.diagnostic_report_tag),
        String::new(),
    ];

    if let Some(entry) = panic_entry {
        lines.extend([
            "Linked Panic Entry".to_string(),
            String::new(),
            format!("Log File:  {}", entry.log_file_title),
            format!("Thread:    {}", entry.thread),
            format!("Source:    {}", entry.source.raw()),
            format!("Timestamp: {}", entry.timestamp.with_timezone(&Local)),
            String::new(),
            "Message:".to_string(),
            String::new(),
        ]);
        lines.extend(entry.message.lines().map(ToOwned::to_owned));

        if entry.has_continuation() {
            lines.extend([
                String::new(),
                format!("Call Stack ({} frames):", entry.continuation.len()),
                String::new(),
            ]);
            lines.extend(
                entry
                    .continuation
                    .iter()
                    .map(|frame_line| frame_line.trim_start().to_string()),
            );
        } else {
            lines.extend([
                String::new(),
                "(no stack trace attached to panic entry)".to_string(),
            ]);
        }
    } else {
        lines.push("No matching panic log entry found.".to_string());
    }

    lines
}
