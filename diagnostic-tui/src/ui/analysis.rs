//! Rendering logic for the Analysis tab.
use crate::{
    app::{
        InputMode, Tab,
        analysis_state::{AnalysisData, AnalysisState},
    },
    ui::helpers::{BORDER_FOCUSED, SELECT_BG},
};
use ratatui::{
    buffer::Buffer,
    layout::{Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Sparkline,
        StatefulWidget, Widget,
    },
};
use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

/// Widget for the Analysis tab, holding borrowed immutable data.
pub struct AnalysisWidget<'a> {
    pub data: &'a AnalysisData<'a>,
    pub input_mode: InputMode,
    pub tab: Tab,
    pub copied_at: Option<Instant>,
    pub copied_count: usize,
}

/// Segment enum for mixed content types.
enum Segment<'a> {
    Lines(Vec<Line<'a>>),
    Sparkline { data: Vec<u64>, height: u16 },
}

impl Segment<'_> {
    fn height(&self) -> usize {
        match self {
            Self::Lines(lines) => lines.len(),
            Self::Sparkline { height, .. } => *height as usize,
        }
    }
}

impl<'a> AnalysisWidget<'a> {
    /// Render segments into the scrollable area, handling clipping and selection.
    fn render_segments(
        &self,
        buf: &mut Buffer,
        segments: Vec<Segment>,
        area: Rect,
        state: &mut AnalysisState,
    ) {
        let in_select = self.input_mode == InputMode::Select && self.tab == Tab::Analysis;
        let cursor = state.cursor;
        let selection = state.selection_range();

        let inner = area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });

        let scroll = state.scroll as usize;
        let viewport_height = inner.height as usize;

        let mut global_line = 0;
        for segment in &segments {
            let segment_height = segment.height();
            let segment_end = global_line + segment_height;

            let visible_start = global_line.max(scroll);
            let visible_end = segment_end.min(scroll + viewport_height);

            if visible_start < visible_end {
                let skip = visible_start - global_line;
                let show = visible_end - visible_start;
                let y = inner.y + (visible_start - scroll) as u16;

                match segment {
                    Segment::Lines(seg_lines) => {
                        let visible = seg_lines[skip..skip + show]
                            .iter()
                            .enumerate()
                            .map(|(i, line)| {
                                let line_idx = global_line + skip + i;
                                style_line(line.clone(), line_idx, cursor, in_select, selection)
                            })
                            .collect::<Vec<_>>();
                        Paragraph::new(visible)
                            .render(Rect::new(inner.x, y, inner.width, show as u16), buf);
                    }
                    Segment::Sparkline { data, height } => {
                        // Only render sparkline when fully visible.
                        let sparkline_height = *height as usize;
                        if skip == 0 && show >= sparkline_height {
                            let sparkline = Sparkline::default()
                                .data(data)
                                .style(Style::default().fg(Color::Yellow));
                            sparkline.render(
                                Rect::new(inner.x + 2, y, inner.width.saturating_sub(4), *height),
                                buf,
                            );
                        }
                    }
                }
            }

            global_line = segment_end;
        }
    }
}

impl StatefulWidget for AnalysisWidget<'_> {
    type State = AnalysisState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut AnalysisState) {
        let data = self.data;
        let mut segments: Vec<Segment> = Vec::new();

        // Section 1: Log Level Summary
        let mut lines = Vec::from([
            Line::from(Span::styled(
                "Log Level Summary",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )),
            Line::from(""),
        ]);

        let labels = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];
        let colors = [
            Color::Red,
            Color::Yellow,
            Color::Green,
            Color::Cyan,
            Color::DarkGray,
        ];

        lines.extend(labels.iter().enumerate().map(|(i, label)| {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{label:<5}"),
                    Style::default().fg(colors[i]).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  {:>8}", format_number(data.level_counts[i]))),
            ])
        }));

        lines.extend([
            Line::from(vec![
                Span::raw("  "),
                Span::styled("Total", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("  {:>8}", format_number(data.total_entries))),
            ]),
            Line::from(""),
        ]);

        // Section 2: Top Errors
        if !data.top_errors.is_empty() {
            lines.extend([
                Line::from(Span::styled(
                    format!("Top Errors ({})", data.top_errors.len()),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )),
                Line::from(""),
            ]);

            for (i, group) in data.top_errors.iter().enumerate() {
                let mut msg_lines = group.message.lines();
                // First line with the index and count prefix.
                if let Some(first_line) = msg_lines.next() {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {:>2}. ", i + 1),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("{:>4}x ", group.count),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(first_line.to_string()),
                    ]));
                }
                // Continuation lines indented to align with the message.
                for cont_line in msg_lines {
                    lines.push(Line::from(vec![
                        Span::raw("            "),
                        Span::raw(cont_line.to_string()),
                    ]));
                }
                if !group.components.is_empty() {
                    lines.push(Line::from(vec![
                        Span::raw("       "),
                        Span::styled(
                            format!("[{}]", group.components.join(", ")),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                }
            }
            lines.push(Line::from(""));
        }

        // Section 3: Component Health
        if !data.component_health.is_empty() {
            lines.extend([
                Line::from(Span::styled(
                    "Component Health",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )),
                Line::from(""),
                // Header row.
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!(
                            "{:<20}  {:>5}  {:>5}  {:>8}",
                            "Component", "ERR", "WARN", "Total"
                        ),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                ]),
            ]);

            lines.extend(data.component_health.iter().map(|comp| {
                let err_style = if comp.error_count > 0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default()
                };
                let warn_style = if comp.warn_count > 0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("{:<20}", comp.component),
                        Style::default().fg(Color::White),
                    ),
                    Span::raw("  "),
                    Span::styled(format!("{:>5}", comp.error_count), err_style),
                    Span::raw("  "),
                    Span::styled(format!("{:>5}", comp.warn_count), warn_style),
                    Span::raw("  "),
                    Span::raw(format!("{:>8}", format_number(comp.total_count))),
                ])
            }));

            lines.push(Line::from(""));
        }

        // Section 4: Timeline
        if !data.time_line.buckets.is_empty() {
            lines.extend([
                Line::from(Span::styled(
                    format!("Timeline — errors + warns per {}", data.time_line.label),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )),
                Line::from(""),
            ]);

            // Time range label.
            if let Some((first, last)) = data
                .time_line
                .buckets
                .first()
                .zip(data.time_line.buckets.last())
            {
                lines.extend([
                    Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!(
                                "{} — {} ({} buckets)",
                                first.start.format("%H:%M"),
                                last.start.format("%H:%M"),
                                data.time_line.buckets.len()
                            ),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]),
                    Line::from(""),
                ])
            }

            // Flush lines before sparkline segment.
            segments.push(Segment::Lines(std::mem::take(&mut lines)));

            // Sparkline data.
            let sparkline_data = data
                .time_line
                .buckets
                .iter()
                .map(|b| b.error_count + b.warn_count)
                .collect::<Vec<_>>();

            segments.push(Segment::Sparkline {
                data: sparkline_data,
                height: 3,
            });

            // Continue with text after sparkline.
            lines.push(Line::from(""));

            // Bursts.
            if !data.time_line.bursts.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  Bursts detected:",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
                lines.extend(
                    data.time_line
                        .bursts
                        .iter()
                        .map(|burst| {
                            Line::from(vec![
                                Span::raw("    "),
                                Span::styled(
                                    format!("{}", burst.start.format("%H:%M")),
                                    Style::default().fg(Color::Yellow),
                                ),
                                Span::raw(format!(
                                    " — {} errors+warns in {}",
                                    burst.count, data.time_line.label
                                )),
                            ])
                        })
                        .chain(std::iter::once(Line::from(""))),
                );
            }

            // Gaps.
            if !data.time_line.gaps.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  Gaps detected:",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));

                lines.extend(
                    data.time_line
                        .gaps
                        .iter()
                        .map(|gap| {
                            let duration = if gap.duration.num_hours() > 0 {
                                format!(
                                    "{}h {}m",
                                    gap.duration.num_hours(),
                                    gap.duration.num_minutes() % 60
                                )
                            } else {
                                format!("{}m", gap.duration.num_minutes())
                            };
                            Line::from(vec![
                                Span::raw("    "),
                                Span::styled(
                                    format!(
                                        "{} — {}",
                                        gap.start.format("%H:%M"),
                                        gap.end.format("%H:%M")
                                    ),
                                    Style::default().fg(Color::Yellow),
                                ),
                                Span::raw(format!(" (no logs for {})", duration)),
                            ])
                        })
                        .chain(std::iter::once(Line::from(""))),
                );
            }
        }

        // Section 5: Panics
        if !data.panics.is_empty() {
            lines.extend([
                Line::from(Span::styled(
                    format!("Panics ({})", data.panics.len()),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )),
                Line::from(""),
            ]);

            lines.extend(
                data.panics
                    .iter()
                    .enumerate()
                    .flat_map(|(i, panic)| {
                        [
                            Line::from(vec![
                                Span::styled(
                                    format!("  {:>2}. ", i + 1),
                                    Style::default().fg(Color::DarkGray),
                                ),
                                Span::styled(
                                    format!("{}", panic.timestamp.format("%Y-%m-%d %H:%M:%S")),
                                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                                ),
                                Span::raw(format!("  [{}]", panic.thread)),
                            ]),
                            Line::from(vec![Span::raw("      "), Span::raw(panic.message)]),
                            Line::from(vec![
                                Span::raw("      "),
                                Span::styled(
                                    format!(
                                        "Log: {}{}",
                                        panic.log_file,
                                        if panic.has_stack_trace {
                                            "  (has stack trace)"
                                        } else {
                                            ""
                                        }
                                    ),
                                    Style::default().fg(Color::DarkGray),
                                ),
                            ]),
                        ]
                    })
                    .chain(std::iter::once(Line::from(""))),
            );
        }

        // Section 6: Crash Report Correlations
        if !data.crash_correlations.is_empty() {
            lines.extend([
                Line::from(Span::styled(
                    format!("Crash Reports ({})", data.crash_correlations.len()),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )),
                Line::from(""),
            ]);

            lines.extend(data.crash_correlations.iter().flat_map(|corr| {
                [
                    Some(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            corr.report_id,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("  ({})", corr.report_type)),
                    ])),
                    corr.crash_timestamp.as_ref().map(|ts| {
                        Line::from(vec![Span::raw("    Crash at: "), Span::raw(ts.as_str())])
                    }),
                    corr.matched_panic_message
                        .map(|msg| {
                            Line::from(vec![
                                Span::raw("    "),
                                Span::styled("Matched: ", Style::default().fg(Color::Green)),
                                Span::raw(msg),
                            ])
                        })
                        .or_else(|| {
                            Some(Line::from(Span::styled(
                                "    No matching panic entry found",
                                Style::default().fg(Color::DarkGray),
                            )))
                        }),
                    Some(Line::from("")),
                ]
                .into_iter()
                .flatten()
            }));
        }

        // Flush remaining lines.
        if !lines.is_empty() {
            segments.push(Segment::Lines(lines));
        }

        // -- Compute total line count. --
        state.line_count = segments.iter().map(|s| s.height()).sum();

        let in_select = self.input_mode == InputMode::Select && self.tab == Tab::Analysis;
        let analysis_selection = state.selection_range();
        let cursor = state.cursor;

        // -- Title. --
        let show_copied = self
            .copied_at
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2));

        let title: Cow<'_, str> = if show_copied && self.tab == Tab::Analysis {
            let count = self.copied_count;
            format!(" Analysis — Copied {count} lines! \u{2713} ").into()
        } else if in_select {
            let (start, end) = analysis_selection.unwrap_or((cursor, cursor));
            let count = end - start + 1;
            format!(
                " Analysis [{}/{}] — {} selected (y:copy  Esc:cancel) ",
                state.cursor + 1,
                state.line_count,
                count,
            )
            .into()
        } else if state.line_count > 0 {
            format!(" Analysis [{}/{}] ", state.cursor + 1, state.line_count).into()
        } else {
            " Analysis ".into()
        };

        let title_style = if show_copied && self.tab == Tab::Analysis {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else if analysis_selection.is_some() {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_FOCUSED))
            .title(Span::styled(title, title_style));

        // Clamp scroll.
        let inner_height = area.height.saturating_sub(2);
        state.viewport_height = inner_height;
        let max_scroll = state.line_count.saturating_sub(inner_height as usize);
        if (state.scroll as usize) > max_scroll {
            state.scroll = max_scroll as u16;
        }

        // Render the border/block.
        block.render(area, buf);

        // Render collected Segments.
        self.render_segments(buf, segments, area, state);

        // Scrollbar.
        let mut scrollbar_state = ScrollbarState::new(state.line_count).position(state.cursor);
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("\u{2191}"))
            .end_symbol(Some("\u{2193}"))
            .render(
                area.inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                buf,
                &mut scrollbar_state,
            );
    }
}

/// Apply cursor/selection styling to a line.
fn style_line<'a>(
    line: Line<'a>,
    line_idx: usize,
    cursor: usize,
    in_select: bool,
    selection: Option<(usize, usize)>,
) -> Line<'a> {
    if line_idx == cursor {
        line.style(Style::new().reversed().add_modifier(Modifier::BOLD))
    } else if in_select && selection.is_some_and(|(s, e)| line_idx >= s && line_idx <= e) {
        line.style(Style::default().bg(SELECT_BG))
    } else {
        line
    }
}

/// Format a number with thousands separators.
fn format_number(n: usize) -> String {
    if n < 1000 {
        return n.to_string();
    }
    let s = n.to_string();
    s.as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join(",")
}
