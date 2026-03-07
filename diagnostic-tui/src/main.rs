//! TUI application for viewing 1Password `.1pdiagnostics` diagnostic reports.
//!
//! Usage:
//!
//! ```sh
//! cargo run -- path/to/file.1pdiagnostics
//! ```

mod app;
mod ui;

use std::io;
use std::process;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use app::App;
use diagnostic_parser::DiagnosticReport;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: diagnostic-tui <path-to-.1pdiagnostics>");
            process::exit(1);
        }
    };

    let report = match DiagnosticReport::from_file(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    if let Err(e) = run_tui(report) {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

fn run_tui(report: DiagnosticReport) -> io::Result<()> {
    // Setup terminal.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(report);

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
