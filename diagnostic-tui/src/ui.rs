//! Rendering logic for the diagnostic TUI.
//!
//! The `ui` module is split into several sub-modules for maintainability:
//!
//! - [`helpers`] — shared colour palette, formatting utilities, and layout helpers
//! - [`overview`] — rendering for the Overview tab
//! - [`logs`] — rendering for the Logs tab (search bar, filter bar, log list, detail pane)
//! - [`crashes`] — rendering for the Crash Reports tab (crash list and detail pane)
//! - [`popups`] — popup overlays (source picker, log file picker, help screen)

mod crashes;
mod helpers;
mod logs;
mod overview;
mod popups;

use crate::app::{App, InputMode, Tab};

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};

use helpers::TAB_ACTIVE;

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
        Tab::Overview => overview::draw_overview(frame, app, outer[1]),
        Tab::Logs => logs::draw_logs(frame, app, outer[1]),
        Tab::CrashReports => crashes::draw_crash_reports(frame, app, outer[1]),
    }

    draw_status_bar(frame, app, outer[2]);

    if app.show_source_picker {
        popups::draw_source_picker(frame, app, size);
    }

    if app.show_log_file_picker {
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

fn draw_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mode_hint = match app.input_mode {
        InputMode::Search => " SEARCH ",
        InputMode::Select => " VISUAL ",
        InputMode::Normal => match app.tab {
            Tab::Overview => " OVERVIEW ",
            Tab::Logs if app.show_log_detail => " LOG DETAIL ",
            Tab::Logs => " LOG LIST ",
            Tab::CrashReports if app.detail_focused => " CRASH DETAIL ",
            Tab::CrashReports => " CRASH LIST ",
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

    let bar = Paragraph::new(Line::from(vec![left, fill, right]));
    frame.render_widget(bar, area);
}
