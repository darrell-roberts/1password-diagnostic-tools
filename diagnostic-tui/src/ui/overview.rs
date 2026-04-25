//! Rendering logic for the Overview tab.
use crate::{
    app::{InputMode, OverviewState, Tab},
    format_bytes,
    ui::helpers::{BORDER_FOCUSED, SELECT_BG, kv_line, kv_line_indent},
};
use diagnostic_parser::{LogEntry, log_entry::LogLevel, model::DiagnosticReport};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Table, Widget,
    },
};
use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

/// Widget for the Overview tab, holding borrowed immutable data.
pub struct OverviewWidget<'a> {
    pub report: &'a DiagnosticReport,
    pub total_log_lines: usize,
    pub all_entries: &'a [LogEntry<'a>],
    pub input_mode: InputMode,
    pub tab: Tab,
    pub copied_at: Option<Instant>,
    pub copied_count: usize,
}

impl<'a> OverviewWidget<'a> {
    /// Render the Paragraph or Table segments.
    fn render_segments(
        &self,
        buf: &mut Buffer,
        segments: Vec<Segment>,
        area: Rect,
        state: &mut OverviewState,
    ) {
        let in_select = self.input_mode == InputMode::Select && self.tab == Tab::Overview;
        let cursor = state.cursor;
        let overview_selection = state.selection_range();

        let inner = area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });

        let scroll = state.scroll as usize;
        let viewport_height = inner.height as usize;
        // -- Render each visible segment. --
        let mut global_line = 0;
        for segment in &segments {
            let segment_height = segment.height();
            let segment_end = global_line + segment_height;

            // Visible range intersection with viewport.
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
                                style_overview_line(
                                    line.clone(),
                                    line_idx,
                                    cursor,
                                    in_select,
                                    overview_selection,
                                )
                            })
                            .collect::<Vec<_>>();
                        Paragraph::new(visible)
                            .render(Rect::new(inner.x, y, inner.width, show as u16), buf);
                    }
                    Segment::VaultTable {
                        account_idx,
                        vault_count: _,
                    } => {
                        let account = &self.report.accounts[*account_idx];
                        let header_visible = skip == 0;
                        let data_skip = skip.saturating_sub(1);
                        let data_count = if header_visible {
                            show.saturating_sub(1)
                        } else {
                            show
                        };

                        let rows = account
                            .vaults
                            .iter()
                            .skip(data_skip)
                            .take(data_count)
                            .enumerate()
                            .map(|(row_idx, vault)| {
                                let line_idx = global_line
                                    + if header_visible { 1 } else { 0 }
                                    + data_skip
                                    + row_idx;
                                let row = Row::new([
                                    Cell::from(Span::styled(
                                        vault.vault_type.as_str(),
                                        Style::default().fg(Color::Magenta),
                                    )),
                                    Cell::from(Span::styled(
                                        &vault.uuid,
                                        Style::default().fg(Color::DarkGray),
                                    )),
                                    Cell::from(vault.items.active.to_string()),
                                    Cell::from(vault.items.archived.to_string()),
                                    Cell::from(vault.items.deleted.to_string()),
                                ]);
                                style_overview_row(
                                    row,
                                    line_idx,
                                    cursor,
                                    in_select,
                                    overview_selection,
                                )
                            })
                            .collect::<Vec<_>>();

                        let widths = [
                            Constraint::Length(14),
                            Constraint::Length(32),
                            Constraint::Length(8),
                            Constraint::Length(10),
                            Constraint::Length(9),
                        ];

                        let table_width = widths
                            .iter()
                            .filter_map(|constraint| match constraint {
                                Constraint::Length(length) => Some(*length),
                                Constraint::Min(_)
                                | Constraint::Max(_)
                                | Constraint::Percentage(_)
                                | Constraint::Ratio(_, _)
                                | Constraint::Fill(_) => None,
                            })
                            .sum::<u16>();

                        let table_width = table_width.min(inner.width);

                        let table_rect = Rect::new(inner.x + 6, y, table_width, show as u16);

                        let mut table = Table::new(rows, widths);
                        if header_visible {
                            let mut header_style = Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
                            if global_line == cursor {
                                header_style = header_style.reversed();
                            } else if in_select
                                && overview_selection
                                    .is_some_and(|(s, e)| global_line >= s && global_line <= e)
                            {
                                header_style = header_style.bg(SELECT_BG);
                            }
                            table = table.header(
                                Row::new(["Vault Type", "UUID", "Active", "Archived", "Deleted"])
                                    .style(header_style),
                            );
                        }

                        Widget::render(table, table_rect, buf);
                    }
                }
            }

            global_line = segment_end;
        }
    }
}

/// Mixed widgets.
enum Segment<'a> {
    Lines(Vec<Line<'a>>),
    VaultTable {
        account_idx: usize,
        vault_count: usize,
    },
}

impl Segment<'_> {
    fn height(&self) -> usize {
        match self {
            Self::Lines(lines) => lines.len(),
            Self::VaultTable { vault_count, .. } => 1 + vault_count,
        }
    }
}

impl StatefulWidget for OverviewWidget<'_> {
    type State = OverviewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut OverviewState) {
        let report = self.report;
        let sys = &report.system;

        let mut segments: Vec<Segment> = Vec::new();

        // -- Report header --
        let mut lines = Vec::from([
            Line::from(Span::styled(
                "Report Information",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )),
            Line::from(""),
            kv_line("UUID", &report.uuid),
            kv_line(
                "Created",
                &report
                    .created_at_utc()
                    .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| format!("{}", report.created_at)),
            ),
            Line::from(""),
            Line::from(Span::styled(
                "System",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )),
            Line::from(""),
            kv_line("Client", &sys.client_name),
            kv_line("Build", &sys.client_build.to_string()),
            kv_line("OS", &format!("{} {}", sys.os_name, sys.os_version)),
            kv_line("Processor", &sys.client_processor),
            kv_line("Memory", &sys.memory),
            kv_line("Disk (total)", &sys.total_space),
            kv_line("Disk (free)", &sys.free_space),
            kv_line("Locale", &sys.locale),
            kv_line("Locked", &format!("{}", sys.client_is_locked)),
        ]);

        if !sys.install_location.is_empty() {
            lines.push(kv_line("Install Path", &sys.install_location));
        }

        lines.extend([
            Line::from(""),
            // -- Overview counters --
            Line::from(Span::styled(
                "Overview",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )),
            Line::from(""),
        ]);

        lines.extend(report.overview.iter().flat_map(|overview| {
            [
                kv_line("Accounts", &overview.accounts.to_string()),
                kv_line("Vaults", &overview.vaults.to_string()),
                kv_line("Active Items", &overview.active_items.to_string()),
                kv_line("Inactive Items", &overview.inactive_items.to_string()),
            ]
        }));

        if report.overview.is_none() {
            lines.push(Line::from(Span::styled(
                "  (not available for this client)",
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines.extend([
            Line::from(""),
            // -- Accounts --
            Line::from(Span::styled(
                "Accounts",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )),
            Line::from(""),
        ]);

        for (i, account) in report.accounts.iter().enumerate() {
            lines.extend([
                Line::from(Span::styled(
                    format!("  Account {} - {}", i + 1, account.uuid),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                kv_line_indent(4, "URL", &account.url),
                kv_line_indent(4, "Type", account.account_type.as_str()),
                kv_line_indent(
                    4,
                    "State",
                    account
                        .account_state
                        .as_ref()
                        .map(|state| state.as_str())
                        .unwrap_or_else(|| "N/A"),
                ),
                kv_line_indent(
                    4,
                    "Billing",
                    account
                        .billing_status
                        .as_ref()
                        .map(|status| status.as_str())
                        .unwrap_or_else(|| "N/A"),
                ),
                kv_line_indent(4, "Locked", &account.account_is_locked.to_string()),
                kv_line_indent(4, "Storage Used", &format_bytes(account.storage_used)),
                kv_line_indent(4, "Vaults", &account.vaults.len().to_string()),
            ]);

            if !account.vaults.is_empty() {
                // Flush accumulated lines before the vault table.
                segments.extend([
                    Segment::Lines(std::mem::take(&mut lines)),
                    Segment::VaultTable {
                        account_idx: i,
                        vault_count: account.vaults.len(),
                    },
                ]);
            }
            lines.push(Line::from(""));
        }

        // Feature Flags
        if !sys.features.is_empty() {
            lines.extend([
                Line::from(Span::styled(
                    "Feature Flags",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )),
                Line::from(""),
            ]);

            lines.extend(
                sys.features
                    .iter()
                    .map(|feature| Line::from(format!("  * {}", feature.name))),
            );

            lines.push(Line::from(""));
        }

        // Log file summary
        lines.extend([
            Line::from(Span::styled(
                "Log Files",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )),
            Line::from(""),
            kv_line("Files", &report.logs.len().to_string()),
            kv_line("Total Lines", &self.total_log_lines.to_string()),
            kv_line("Parsed Entries", &self.all_entries.len().to_string()),
        ]);

        // Level breakdown.
        let mut by_level = [0; 5];
        for entry in self.all_entries {
            let idx = match entry.level {
                LogLevel::Error => 0,
                LogLevel::Warn => 1,
                LogLevel::Info => 2,
                LogLevel::Debug => 3,
                LogLevel::Trace => 4,
            };
            by_level[idx] += 1;
        }
        let level_labels = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];
        let level_colors: [Color; 5] = [
            Color::Red,
            Color::Yellow,
            Color::Green,
            Color::Cyan,
            Color::DarkGray,
        ];
        for i in 0..5 {
            if by_level[i] > 0 {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("{:<5}", level_labels[i]),
                        Style::default().fg(level_colors[i]),
                    ),
                    Span::raw(format!(" {}", by_level[i])),
                ]));
            }
        }
        lines.push(Line::from(""));

        // Crash reports count
        lines.extend([
            Line::from(Span::styled(
                "Crash Reports",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )),
            Line::from(""),
            kv_line("Count", &report.crash_report_entries.len().to_string()),
        ]);

        // Flush remaining lines.
        if !lines.is_empty() {
            segments.push(Segment::Lines(lines));
        }

        // Compute total line count and state.
        let total_lines = segments.iter().map(|s| s.height()).sum();
        state.line_count = total_lines;

        let overview_selection = state.selection_range();
        let in_select = self.input_mode == InputMode::Select && self.tab == Tab::Overview;
        let cursor = state.cursor;

        // Title.
        let show_copied = self
            .copied_at
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2));

        let title: Cow<'_, str> = if show_copied && self.tab == Tab::Overview {
            let count = self.copied_count;
            format!(" Overview — Copied {count} lines! ✓ ").into()
        } else if in_select {
            let (start, end) = overview_selection.unwrap_or((cursor, cursor));
            let count = end - start + 1;
            format!(
                " Overview [{}/{}] — {} selected (y:copy  Esc:cancel) ",
                state.cursor + 1,
                state.line_count,
                count,
            )
            .into()
        } else if state.line_count > 0 {
            format!(" Overview [{}/{}] ", state.cursor + 1, state.line_count,).into()
        } else {
            " Overview ".into()
        };

        let title_style = if show_copied && self.tab == Tab::Overview {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else if overview_selection.is_some() {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_FOCUSED))
            .title(Span::styled(title, title_style));

        // Clamp scroll.
        let inner_height = area.height.saturating_sub(2) as usize;
        state.viewport_height = inner_height as u16;
        let max_scroll = total_lines.saturating_sub(inner_height);
        if (state.scroll as usize) > max_scroll {
            state.scroll = max_scroll as u16;
        }

        // Render the border/block.
        block.render(area, buf);

        // Render collected Segments.
        self.render_segments(buf, segments, area, state);

        // Scrollbar.
        let mut scrollbar_state = ScrollbarState::new(total_lines).position(state.cursor);
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
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

/// Apply cursor/selection styling to an overview [`Line`].
fn style_overview_line<'a>(
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

/// Apply cursor/selection styling to a vault table [`Row`].
fn style_overview_row<'a>(
    row: Row<'a>,
    line_idx: usize,
    cursor: usize,
    in_select: bool,
    selection: Option<(usize, usize)>,
) -> Row<'a> {
    if line_idx == cursor {
        row.style(Style::new().reversed().add_modifier(Modifier::BOLD))
    } else if in_select && selection.is_some_and(|(s, e)| line_idx >= s && line_idx <= e) {
        row.style(Style::default().bg(SELECT_BG))
    } else {
        row
    }
}
