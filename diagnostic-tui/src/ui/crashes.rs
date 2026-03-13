//! Rendering logic for the Crash Reports tab: crash list and crash detail pane.

use crate::app::{App, Tab};
use crate::ui::helpers::{BORDER_FOCUSED, BORDER_NORMAL, HIGHLIGHT_BG, SELECT_BG, truncate_str};
use chrono::Local;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

/// Draw the entire Crash Reports tab content into the given area.
pub fn draw_crash_reports(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.report.crash_report_entries.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_FOCUSED))
            .title(" Crash Reports ");

        let msg = Paragraph::new("No crash reports in this diagnostic file.")
            .style(Style::default().fg(Color::DarkGray))
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(msg, area);
        return;
    }

    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    draw_crash_list(frame, app, horiz[0]);
    draw_crash_detail(frame, app, horiz[1]);
}

// ---------------------------------------------------------------------------
// Crash list
// ---------------------------------------------------------------------------

fn draw_crash_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_color = if !app.detail_focused {
        BORDER_FOCUSED
    } else {
        BORDER_NORMAL
    };

    let inner_height = area.height.saturating_sub(2) as usize;
    app.viewport.crash_list = inner_height as u16;
    app.crash_list_state.ensure_visible(inner_height);

    let crash_selection_range = app.crash_selection_range();

    let items: Vec<ListItem> = app
        .report
        .crash_report_entries
        .iter()
        .enumerate()
        .skip(app.crash_list_state.offset)
        .take(inner_height)
        .map(|(idx, crash)| {
            let is_cursor = idx == app.crash_list_state.selected;
            let is_in_selection =
                crash_selection_range.is_some_and(|(start, end)| idx >= start && idx <= end);

            let ts = crash
                .timestamp_utc()
                .map(|d| {
                    d.with_timezone(&Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(|| format!("{}", crash.timestamp));

            let type_span = Span::styled(
                format!("{:<6}", crash.report_type),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            );

            let ts_span = Span::styled(format!(" {ts} "), Style::default().fg(Color::DarkGray));

            let id_avail = (area.width as usize).saturating_sub(ts.len() + 10);
            let id_span = Span::styled(
                truncate_str(&crash.report_id, id_avail),
                Style::default().fg(Color::White),
            );

            let mut style = Style::default();
            if is_cursor {
                style = style.bg(HIGHLIGHT_BG).add_modifier(Modifier::BOLD);
            } else if is_in_selection {
                style = style.bg(SELECT_BG);
            }

            ListItem::new(Line::from(vec![type_span, ts_span, id_span])).style(style)
        })
        .collect();

    let show_copied = app.tab == Tab::CrashReports
        && app
            .copied_at
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2));

    let title = if show_copied {
        let count = crash_selection_range.map_or(1, |(s, e)| e - s + 1);
        format!(" Crashes — Copied {count} entries! ✓ ")
    } else if let Some((start, end)) = crash_selection_range {
        let count = end - start + 1;
        format!(
            " Crashes [{}/{}] — {} selected (y:copy  Esc:cancel) ",
            if app.report.crash_report_entries.is_empty() {
                0
            } else {
                app.crash_list_state.selected + 1
            },
            app.report.crash_report_entries.len(),
            count,
        )
    } else {
        format!(
            " Crashes [{}/{}] ",
            if app.report.crash_report_entries.is_empty() {
                0
            } else {
                app.crash_list_state.selected + 1
            },
            app.report.crash_report_entries.len(),
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

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(title, title_style)),
    );

    frame.render_widget(list, area);
}

// ---------------------------------------------------------------------------
// Crash detail pane
// ---------------------------------------------------------------------------

fn draw_crash_detail(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_color = if app.detail_focused {
        BORDER_FOCUSED
    } else {
        BORDER_NORMAL
    };

    let show_copied = app
        .copied_at
        .is_some_and(|t| t.elapsed() < Duration::from_secs(2));

    let (detail_title, detail_title_style) = if show_copied {
        (
            " Crash Detail — Copied! ✓ ".to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (" Crash Detail ".to_string(), Style::default())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(detail_title, detail_title_style));

    // Clone data from crash report and optional panic entry to avoid borrow conflicts.
    let crash_data = app.selected_crash_report().map(|crash| {
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
        )
    });

    let panic_data = app.selected_crash_panic_entry().map(|entry| {
        (
            entry.log_file_title.clone(),
            entry.thread.clone(),
            entry.source.raw(),
            entry.timestamp.with_timezone(&Local).to_string(),
            entry.message.clone(),
            entry.has_continuation(),
            entry.continuation.clone(),
        )
    });

    let Some((report_id, report_type, ts, tag)) = crash_data else {
        let empty = Paragraph::new("No crash report selected")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(empty, area);
        return;
    };

    let mut lines: Vec<Line> = vec![
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
    ];

    match panic_data {
        Some((
            log_file_title,
            thread,
            source_raw,
            timestamp,
            message,
            has_continuation,
            continuation,
        )) => {
            lines.push(Line::from(Span::styled(
                "Linked Panic Entry",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));
            lines.push(Line::from(""));

            lines.push(Line::from(vec![
                Span::styled("Log File:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(log_file_title),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Thread:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(thread),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Source:    ", Style::default().fg(Color::DarkGray)),
                Span::styled(source_raw, Style::default().fg(Color::Magenta)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Timestamp: ", Style::default().fg(Color::DarkGray)),
                Span::raw(timestamp),
            ]));

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Message:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            for msg_line in message.lines() {
                lines.push(Line::from(Span::raw(msg_line.to_string())));
            }

            if has_continuation {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("Call Stack ({} frames):", continuation.len()),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));

                for (i, frame_line) in continuation.iter().enumerate() {
                    let trimmed = frame_line.trim_start();
                    // Alternate colors for readability.
                    let fg = if i % 2 == 0 {
                        Color::Yellow
                    } else {
                        Color::White
                    };
                    lines.push(Line::from(Span::styled(
                        trimmed.to_string(),
                        Style::default().fg(fg),
                    )));
                }
            } else {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "(no stack trace attached to panic entry)",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
        None => {
            lines.push(Line::from(Span::styled(
                "No matching panic log entry found.",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "The crash report could not be correlated with a panic log entry.",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "This may happen if the log file has been rotated or the crash",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "occurred outside the captured log window.",
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Clamp scroll.
    let inner_height = area.height.saturating_sub(2) as usize;
    app.viewport.crash_detail = inner_height as u16;
    let max_scroll = lines.len().saturating_sub(inner_height);
    if (app.crash_detail_scroll as usize) > max_scroll {
        app.crash_detail_scroll = max_scroll as u16;
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.crash_detail_scroll, 0));

    frame.render_widget(paragraph, area);
}
