//! Rendering logic for the Overview tab.

use crate::app::{App, InputMode, Tab};
use crate::ui::helpers::{
    BORDER_FOCUSED, HIGHLIGHT_BG, SELECT_BG, format_bytes, kv_line, kv_line_indent,
};
use std::time::Duration;

use diagnostic_parser::log_entry::LogLevel;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Draw the Overview tab content into the given area.
pub fn draw_overview(frame: &mut Frame, app: &mut App, area: Rect) {
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

    // Update the total line count so the app knows bounds for selection.
    app.overview_line_count = lines.len();

    // Always show cursor highlight on overview; also highlight selection range
    // when in visual-select mode.
    let overview_selection = app.overview_selection_range();
    let in_select = app.input_mode == InputMode::Select && app.tab == Tab::Overview;
    let cursor = app.overview_cursor;
    for (i, line) in lines.iter_mut().enumerate() {
        let is_cursor = i == cursor;
        let is_in_selection =
            in_select && overview_selection.is_some_and(|(start, end)| i >= start && i <= end);

        if is_cursor {
            *line = line.clone().style(
                Style::default()
                    .bg(HIGHLIGHT_BG)
                    .add_modifier(Modifier::BOLD),
            );
        } else if is_in_selection {
            *line = line.clone().style(Style::default().bg(SELECT_BG));
        }
    }

    // Show "Copied!" flash or selection count in the title.
    let show_copied = app
        .copied_at
        .is_some_and(|t| t.elapsed() < Duration::from_secs(2));

    let title = if show_copied && app.tab == Tab::Overview {
        let count = overview_selection.map_or(1, |(s, e)| e - s + 1);
        format!(" Overview — Copied {count} lines! ✓ ")
    } else if in_select {
        let (start, end) = overview_selection.unwrap_or((cursor, cursor));
        let count = end - start + 1;
        format!(
            " Overview [{}/{}] — {} selected (y:copy  Esc:cancel) ",
            app.overview_cursor + 1,
            app.overview_line_count,
            count,
        )
    } else if app.overview_line_count > 0 {
        format!(
            " Overview [{}/{}] ",
            app.overview_cursor + 1,
            app.overview_line_count,
        )
    } else {
        " Overview ".to_string()
    };

    let title_style = if show_copied && app.tab == Tab::Overview {
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
