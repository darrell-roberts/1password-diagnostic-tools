//! TUI application for viewing 1Password `.1pdiagnostics` diagnostic reports.
//!
//! Usage:
//!
//! ```sh
//! cargo run -- path/to/file.1pdiagnostics
//! ```
use app::App;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use diagnostic_parser::DiagnosticReport;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::{io, process};

mod app;
mod ui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: diagnostic-tui <path-to-.1pdiagnostics>");
        process::exit(1);
    };

    let report = DiagnosticReport::from_file(&path)?;

    // Ensure the terminal is restored if the app panics.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        default_hook(info);
    }));

    run_tui(report).map_err(Into::into)
}

fn run_tui(report: DiagnosticReport) -> io::Result<()> {
    // Setup terminal.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (log_entries, _) = report.parse_log_entries_ref();

    let mut app = App::new(&report, &log_entries);

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        match event::read()? {
            Event::Key(key) => {
                // Global quit: q (when not in search mode) or Ctrl-c.
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }

                if app.handle_key(key) {
                    break;
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => app.handle_scroll_up(),
                MouseEventKind::ScrollDown => app.handle_scroll_down(),
                _ => {}
            },
            _ => {}
        }
    }

    // Restore terminal.
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// String / byte formatting
// ---------------------------------------------------------------------------

/// Format a byte count as a human-readable string (KB / MB / GB).
pub fn format_bytes(bytes: u64) -> String {
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
