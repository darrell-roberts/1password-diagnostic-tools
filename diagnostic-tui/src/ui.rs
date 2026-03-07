//! Rendering logic for the diagnostic TUI.
//!
//! Each top-level tab (Overview, Logs, Crash Reports) has its own drawing
//! function. The public [`draw`] function dispatches to the correct one
//! based on the current [`Tab`].

use crate::app::{App, InputMode, Tab};

use diagnostic_parser::log_entry::LogLevel;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState as RatatuiListState, Paragraph, Tabs, Wrap,
};

// ---------------------------------------------------------------------------
// Colour palette
// ---------------------------------------------------------------------------

fn level_color(level: LogLevel) -> Color {
    match level {
        LogLevel::Trace => Color::DarkGray,
        LogLevel::Debug => Color::Cyan,
        LogLevel::Info => Color::Green,
        LogLevel::Warn => Color::Yellow,
        LogLevel::Error => Color::Red,
    }
}

const HIGHLIGHT_BG: Color = Color::Rgb(50, 50, 80);
const BORDER_FOCUSED: Color = Color::Cyan;
const BORDER_NORMAL: Color = Color::DarkGray;
const TAB_ACTIVE: Color = Color::Cyan;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Draw the entire UI for one frame.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Top-level layout: tab bar (3 rows), then content.
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // status bar
        ])
        .split(size);

    draw_tab_bar(frame, app, outer[0]);

    match app.tab {
        Tab::Overview => draw_overview(frame, app, outer[1]),
        Tab::Logs => draw_logs(frame, app, outer[1]),
        Tab::CrashReports => draw_crash_reports(frame, app, outer[1]),
    }

    draw_status_bar(frame, app, outer[2]);

    if app.show_source_picker {
        draw_source_picker(frame, app, size);
    }

    if app.show_log_file_picker {
        draw_log_file_picker(frame, app, size);
    }

    if app.show_help {
        draw_help_overlay(frame, size);
    }
}

// ---------------------------------------------------------------------------
// Tab bar
// ---------------------------------------------------------------------------

fn draw_tab_bar(frame: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .map(|t| {
            let num = match t {
                Tab::Overview => "1",
                Tab::Logs => "2",
                Tab::CrashReports => "3",
            };
            Line::from(vec![
                Span::styled(format!(" {num}:"), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{} ", t.title()), Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let selected = Tab::ALL.iter().position(|t| *t == app.tab).unwrap_or(0);

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Diagnostic Report "),
        )
        .select(selected)
        .highlight_style(Style::default().fg(TAB_ACTIVE).add_modifier(Modifier::BOLD))
        .divider(Span::raw("│"));

    frame.render_widget(tabs, area);
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode_hint = match app.input_mode {
        InputMode::Search => " SEARCH ",
        InputMode::Normal => match app.tab {
            Tab::Overview => " OVERVIEW ",
            Tab::Logs if app.show_log_detail => " LOG DETAIL ",
            Tab::Logs => " LOG LIST ",
            Tab::CrashReports if app.detail_focused => " CRASH DETAIL ",
            Tab::CrashReports => " CRASH LIST ",
        },
    };

    let help_hint = " ?:Help  Tab:Switch  q:Quit ";

    let left = Span::styled(
        mode_hint,
        Style::default()
            .bg(Color::Cyan)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let right = Span::styled(help_hint, Style::default().fg(Color::DarkGray));

    // Fill the rest with spaces.
    let fill_len = (area.width as usize)
        .saturating_sub(mode_hint.len())
        .saturating_sub(help_hint.len());
    let fill = Span::raw(" ".repeat(fill_len));

    let bar = Paragraph::new(Line::from(vec![left, fill, right]));
    frame.render_widget(bar, area);
}

// ---------------------------------------------------------------------------
// Overview tab
// ---------------------------------------------------------------------------

fn draw_overview(frame: &mut Frame, app: &mut App, area: Rect) {
    let report = &app.report;
    let sys = &report.system;

    let created = report
        .created_at_utc()
        .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| format!("{}", report.created_at));

    // -- Report header --
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "Report Information",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from(""),
        kv_line("UUID", &report.uuid),
        kv_line("Created", &created),
        Line::from(""),
        // -- System --
        Line::from(Span::styled(
            "System",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from(""),
        kv_line("Client", &sys.client_name),
    ];
    let build_str = sys.client_build.to_string();
    lines.push(kv_line("Build", &build_str));
    let os_str = format!("{} {}", sys.os_name, sys.os_version);
    lines.push(kv_line("OS", &os_str));
    lines.push(kv_line("Processor", &sys.client_processor));
    lines.push(kv_line("Memory", &sys.memory));
    lines.push(kv_line("Disk (total)", &sys.total_space));
    lines.push(kv_line("Disk (free)", &sys.free_space));
    lines.push(kv_line("Locale", &sys.locale));
    let locked_str = format!("{}", sys.client_is_locked);
    lines.push(kv_line("Locked", &locked_str));
    if !sys.install_location.is_empty() {
        lines.push(kv_line("Install Path", &sys.install_location));
    }
    lines.push(Line::from(""));

    // -- Overview counters --
    lines.push(Line::from(Span::styled(
        "Overview",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));
    lines.push(Line::from(""));
    if let Some(ref overview) = report.overview {
        let accounts_str = overview.accounts.to_string();
        lines.push(kv_line("Accounts", &accounts_str));
        let vaults_str = overview.vaults.to_string();
        lines.push(kv_line("Vaults", &vaults_str));
        let active_str = overview.active_items.to_string();
        lines.push(kv_line("Active Items", &active_str));
        let inactive_str = overview.inactive_items.to_string();
        lines.push(kv_line("Inactive Items", &inactive_str));
    } else {
        lines.push(Line::from(Span::styled(
            "  (not available for this client)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(""));

    // -- Accounts --
    lines.push(Line::from(Span::styled(
        "Accounts",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));
    lines.push(Line::from(""));

    for (i, account) in report.accounts.iter().enumerate() {
        lines.push(Line::from(Span::styled(
            format!("  Account {} - {}", i + 1, account.uuid),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(kv_line_indent(4, "URL", &account.url));
        let acct_type_str = account.account_type.to_string();
        lines.push(kv_line_indent(4, "Type", &acct_type_str));
        let acct_state_str = account
            .account_state
            .map(|s| s.to_string())
            .unwrap_or_else(|| "N/A".to_string());
        lines.push(kv_line_indent(4, "State", &acct_state_str));
        let billing_str = account
            .billing_status
            .map(|b| b.to_string())
            .unwrap_or_else(|| "N/A".to_string());
        lines.push(kv_line_indent(4, "Billing", &billing_str));
        let locked_str = account.account_is_locked.to_string();
        lines.push(kv_line_indent(4, "Locked", &locked_str));
        let storage_str = format_bytes(account.storage_used);
        lines.push(kv_line_indent(4, "Storage Used", &storage_str));
        let vaults_len_str = account.vaults.len().to_string();
        lines.push(kv_line_indent(4, "Vaults", &vaults_len_str));

        for vault in &account.vaults {
            lines.push(Line::from(vec![
                Span::raw("      "),
                Span::styled(
                    format!("{} ", vault.vault_type),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled(vault.uuid.clone(), Style::default().fg(Color::DarkGray)),
                Span::raw(format!(
                    "  {} active, {} archived, {} deleted",
                    vault.items.active, vault.items.archived, vault.items.deleted,
                )),
            ]));
        }
        lines.push(Line::from(""));
    }

    // -- Feature Flags --
    if !sys.features.is_empty() {
        lines.push(Line::from(Span::styled(
            "Feature Flags",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        for feat in &sys.features {
            lines.push(Line::from(format!("  * {}", feat.name)));
        }
        lines.push(Line::from(""));
    }

    // -- Log file summary --
    lines.push(Line::from(Span::styled(
        "Log Files",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));
    lines.push(Line::from(""));
    let files_str = report.logs.len().to_string();
    lines.push(kv_line("Files", &files_str));
    let total_lines_str = report.total_log_lines().to_string();
    lines.push(kv_line("Total Lines", &total_lines_str));
    let parsed_str = app.all_entries.len().to_string();
    lines.push(kv_line("Parsed Entries", &parsed_str));

    // Level breakdown.
    let mut by_level = [0usize; 5];
    for entry in &app.all_entries {
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
    let level_colors = [
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

    // -- Crash reports count --
    lines.push(Line::from(Span::styled(
        "Crash Reports",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));
    lines.push(Line::from(""));
    let crash_count_str = report.crash_report_entries.len().to_string();
    lines.push(kv_line("Count", &crash_count_str));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_FOCUSED))
        .title(" Overview ");

    // Clamp scroll.
    let inner_height = area.height.saturating_sub(2) as usize;
    app.viewport.overview = inner_height as u16;
    let max_scroll = lines.len().saturating_sub(inner_height);
    if (app.overview_scroll as usize) > max_scroll {
        app.overview_scroll = max_scroll as u16;
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.overview_scroll, 0));

    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Logs tab
// ---------------------------------------------------------------------------

fn draw_logs(frame: &mut Frame, app: &mut App, area: Rect) {
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

fn draw_search_bar(frame: &mut Frame, app: &App, area: Rect) {
    let (border_color, cursor_visible) = match app.input_mode {
        InputMode::Search => (Color::Yellow, true),
        InputMode::Normal => (BORDER_NORMAL, false),
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

fn level_filter_color(filter: &crate::app::LevelFilter) -> Color {
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

fn draw_log_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_color = BORDER_FOCUSED;

    let inner_height = area.height.saturating_sub(2) as usize;
    app.viewport.log_list = inner_height as u16;
    app.log_list_state.ensure_visible(inner_height);

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .skip(app.log_list_state.offset)
        .take(inner_height)
        .map(|(display_idx, &entry_idx)| {
            let entry = &app.all_entries[entry_idx];
            let is_selected = display_idx == app.log_list_state.selected;

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
            if is_selected {
                style = style.bg(HIGHLIGHT_BG).add_modifier(Modifier::BOLD);
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

    let title = format!(
        " Logs [{}/{}] ",
        if app.filtered_indices.is_empty() {
            0
        } else {
            app.log_list_state.selected + 1
        },
        app.filtered_indices.len(),
    );

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title),
    );

    frame.render_widget(list, area);
}

fn draw_log_detail(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_color = BORDER_FOCUSED;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" Detail ");

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

    // Build detail text.
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

    // Clamp scroll.
    let inner_height = area.height.saturating_sub(2) as usize;
    app.viewport.log_detail = inner_height as u16;
    let max_scroll = lines.len().saturating_sub(inner_height);
    if (app.detail_scroll as usize) > max_scroll {
        app.detail_scroll = max_scroll as u16;
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));

    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Crash Reports tab
// ---------------------------------------------------------------------------

fn draw_crash_reports(frame: &mut Frame, app: &mut App, area: Rect) {
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

fn draw_crash_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_color = if !app.detail_focused {
        BORDER_FOCUSED
    } else {
        BORDER_NORMAL
    };

    let inner_height = area.height.saturating_sub(2) as usize;
    app.viewport.crash_list = inner_height as u16;
    app.crash_list_state.ensure_visible(inner_height);

    let items: Vec<ListItem> = app
        .report
        .crash_report_entries
        .iter()
        .enumerate()
        .skip(app.crash_list_state.offset)
        .take(inner_height)
        .map(|(idx, crash)| {
            let is_selected = idx == app.crash_list_state.selected;

            let ts = crash
                .timestamp_utc()
                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
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
            if is_selected {
                style = style.bg(HIGHLIGHT_BG).add_modifier(Modifier::BOLD);
            }

            ListItem::new(Line::from(vec![type_span, ts_span, id_span])).style(style)
        })
        .collect();

    let title = format!(
        " Crashes [{}/{}] ",
        if app.report.crash_report_entries.is_empty() {
            0
        } else {
            app.crash_list_state.selected + 1
        },
        app.report.crash_report_entries.len(),
    );

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title),
    );

    frame.render_widget(list, area);
}

fn draw_crash_detail(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_color = if app.detail_focused {
        BORDER_FOCUSED
    } else {
        BORDER_NORMAL
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" Crash Detail ");

    // Clone data from crash report and optional panic entry to avoid borrow conflicts.
    let crash_data = app.selected_crash_report().map(|crash| {
        let ts = crash
            .timestamp_utc()
            .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
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
            entry.timestamp.to_string(),
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

// ---------------------------------------------------------------------------
// Help overlay
// ---------------------------------------------------------------------------

fn draw_source_picker(frame: &mut Frame, app: &mut App, area: Rect) {
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

fn draw_log_file_picker(frame: &mut Frame, app: &mut App, area: Rect) {
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

fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    // Centered popup.
    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 34u16.min(area.height.saturating_sub(4));
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

fn help_entry<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
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
// Helpers
// ---------------------------------------------------------------------------

/// Create a key-value line with default indentation.
fn kv_line(key: &str, value: &str) -> Line<'static> {
    kv_line_indent(2, key, value)
}

/// Create a key-value line with the specified indentation.
fn kv_line_indent(indent: usize, key: &str, value: &str) -> Line<'static> {
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

fn truncate_str(s: &str, max_len: usize) -> String {
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

fn format_bytes(bytes: u64) -> String {
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

/// Return a centered `Rect` of the given size within `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
