//! Rendering logic for the diagnostic TUI.
//!
//! The `ui` module is split into several sub-modules for maintainability:
//!
//! - [`helpers`] — shared colour palette, formatting utilities, and layout helpers
//! - [`overview`] — rendering for the Overview tab
//! - [`logs`] — rendering for the Logs tab (search bar, filter bar, log list, detail pane)
//! - [`crashes`] — rendering for the Crash Reports tab (crash list and detail pane)
//! - [`popups`] — popup overlays (source picker, log file picker, help screen)

mod analysis;
mod crashes;
mod helpers;
mod logs;
mod overview;
mod popups;

use crate::app::{App, InputMode, Tab};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Tabs},
};

use analysis::AnalysisWidget;
use crashes::CrashReportsWidget;
use helpers::TAB_ACTIVE;
use logs::LogsWidget;
use overview::OverviewWidget;

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
        Tab::Overview => {
            OverviewWidget {
                report: app.report,
                total_log_lines: app.total_log_lines,
                all_entries: app.all_entries,
                input_mode: app.input_mode,
                tab: app.tab,
                copied_at: app.copied_at,
                copied_count: app.copied_count,
            }
            .render(outer[1], frame.buffer_mut(), &mut app.overview);
        }
        Tab::Logs => {
            LogsWidget {
                all_entries: app.all_entries,
                input_mode: app.input_mode,
                copied_at: app.copied_at,
                copied_count: app.copied_count,
            }
            .render(outer[1], frame.buffer_mut(), &mut app.logs);

            // Handle search bar cursor position (can't be done from StatefulWidget::render).
            if let Some((x, y)) = app.logs.search_cursor_position {
                frame.set_cursor_position((x, y));
            }
        }
        Tab::CrashReports => {
            CrashReportsWidget {
                report: app.report,
                all_entries: app.all_entries,
                tab: app.tab,
                copied_at: app.copied_at,
                copied_count: app.copied_count,
            }
            .render(outer[1], frame.buffer_mut(), &mut app.crashes);
        }
        Tab::Analysis => {
            AnalysisWidget {
                data: &app.analysis_data,
                input_mode: app.input_mode,
                tab: app.tab,
                copied_at: app.copied_at,
                copied_count: app.copied_count,
            }
            .render(outer[1], frame.buffer_mut(), &mut app.analysis);
        }
    }

    draw_status_bar(frame, app, outer[2]);

    if app.logs.show_source_picker {
        popups::draw_source_picker(frame, app, size);
    }

    if app.logs.show_log_file_picker {
        popups::draw_log_file_picker(frame, app, size);
    }

    if app.show_help {
        popups::draw_help_overlay(frame, size);
    }
}

// ---------------------------------------------------------------------------
// Tab bar
// ---------------------------------------------------------------------------

fn draw_tab_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let titles = Tab::ALL
        .iter()
        .map(|t| {
            let num = match t {
                Tab::Overview => "1",
                Tab::Logs => "2",
                Tab::CrashReports => "3",
                Tab::Analysis => "4",
            };
            Line::from(vec![
                Span::styled(format!(" {num}:"), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{} ", t.title()), Style::default().fg(Color::White)),
            ])
        })
        .collect::<Vec<_>>();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Diagnostic Report "),
        )
        .select(Tab::ALL.iter().position(|t| *t == app.tab).unwrap_or(0))
        .highlight_style(Style::default().fg(TAB_ACTIVE).add_modifier(Modifier::BOLD))
        .divider(Span::raw("│"));

    frame.render_widget(tabs, area);
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

fn draw_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mode_hint = match app.input_mode {
        InputMode::Search => " SEARCH ",
        InputMode::Select => " VISUAL ",
        InputMode::Normal => match app.tab {
            Tab::Overview => " OVERVIEW ",
            Tab::Logs if app.logs.show_detail => " LOG DETAIL ",
            Tab::Logs => " LOG LIST ",
            Tab::CrashReports if app.crashes.detail_focused => " CRASH DETAIL ",
            Tab::CrashReports => " CRASH LIST ",
            Tab::Analysis => " ANALYSIS ",
        },
    };

    let help_hint = match app.input_mode {
        InputMode::Select => " y:Copy  Esc:Cancel  ↑↓:Extend ",
        _ => " ?:Help  Tab:Switch  q:Quit ",
    };

    let mode_bg = match app.input_mode {
        InputMode::Select => Color::Yellow,
        _ => Color::Cyan,
    };

    let left = Span::styled(
        mode_hint,
        Style::default()
            .bg(mode_bg)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let right = Span::styled(help_hint, Style::default().fg(Color::DarkGray));

    // Fill the rest with spaces.
    let fill_len = (area.width as usize)
        .saturating_sub(mode_hint.len())
        .saturating_sub(help_hint.len());
    let fill = Span::raw(" ".repeat(fill_len));

    frame.render_widget(Paragraph::new(Line::from(vec![left, fill, right])), area);
}
