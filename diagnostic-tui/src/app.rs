//! Application state and input handling for the diagnostic TUI.
//!
//! The `app` module is split into several sub-modules for maintainability:
//!
//! - [`state`] — core state types (`Tab`, `InputMode`)
//! - [`pane_state`] — shared scrollable-pane state (Overview, Analysis)
//! - [`logs_state`] — state for the Logs tab
//! - [`crash_state`] — state for the Crash Reports tab
//! - [`filters`] — log entry filter types (`LevelFilter`, `SourceFilter`, `LogFileFilter`)
//! - [`keys`] — keyboard input handlers for each mode
//! - [`navigation`] — directional movement, paging, and viewport scrolling
//! - [`clipboard`] — copy/paste, selection ranges, and plain-text builders

pub mod analysis_state;
pub mod clipboard;
pub mod crash_state;
pub mod filters;
pub mod keys;
pub mod logs_state;
pub mod navigation;
pub mod pane_state;
pub mod state;

// Re-export the most commonly used types so callers can write `app::App`, etc.
pub use analysis_state::{AnalysisData, AnalysisState};
pub use crash_state::CrashReportsState;
use diagnostic_parser::LogEntryRef;
pub use logs_state::LogsState;
pub use pane_state::OverviewState;
pub use state::{InputMode, Tab};

use arboard::Clipboard;
use crossterm::event::KeyEvent;
use diagnostic_parser::model::{CrashReportEntry, DiagnosticReport};
use std::time::Instant;

/// Root application state.
pub struct App<'a> {
    // -- Immutable data (loaded once) --
    /// The loaded diagnostic report.
    pub report: &'a DiagnosticReport,
    /// Cached total log line count (computed once at startup).
    pub total_log_lines: usize,
    /// All parsed log entries (immutable after construction).
    pub all_entries: &'a [LogEntryRef<'a>],

    // -- Global UI state --
    /// Currently active tab.
    pub tab: Tab,
    /// Whether we are in search input mode.
    pub input_mode: InputMode,
    /// Whether to show help overlay.
    pub show_help: bool,
    /// Whether the previous keypress was `z`, awaiting the second key of a
    /// two-key `z` command (`zz`, `zt`, `zb`).
    pub pending_z: bool,

    // -- Clipboard (shared across all views) --
    /// System clipboard handle, created once at startup.
    pub clipboard: Option<Clipboard>,
    /// Instant when the last successful copy occurred, used to flash feedback.
    pub copied_at: Option<Instant>,
    /// Number of items/lines in the last successful copy, for the flash message.
    pub copied_count: usize,

    // -- Per-view state --
    /// Overview tab state.
    pub overview: OverviewState,
    /// Logs tab state.
    pub logs: LogsState<'a>,
    /// Crash Reports tab state.
    pub crashes: CrashReportsState,
    /// Analysis tab state.
    pub analysis: AnalysisState,
    /// Pre-computed analysis data.
    pub analysis_data: AnalysisData<'a>,
}

impl<'a> App<'a> {
    pub fn new(report: &'a DiagnosticReport, all_entries: &'a [LogEntryRef<'a>]) -> Self {
        let total_log_lines = report.total_log_lines();
        let has_crashes = !report.crash_report_entries.is_empty();

        let logs = LogsState::new(all_entries);
        let crashes = CrashReportsState::new(has_crashes);
        let analysis_data = AnalysisData::analyze(all_entries, &report.crash_report_entries);

        Self {
            report,
            total_log_lines,
            all_entries,
            tab: Tab::Overview,
            input_mode: InputMode::Normal,
            show_help: false,
            pending_z: false,
            clipboard: Clipboard::new().ok(),
            copied_at: None,
            copied_count: 0,
            overview: OverviewState::default(),
            logs,
            crashes,
            analysis: AnalysisState::default(),
            analysis_data,
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Get the currently selected log entry (if any).
    pub fn selected_log_entry(&self) -> Option<&LogEntryRef<'_>> {
        let selected = self.logs.list_state.selected()?;
        let idx = *self.logs.filtered_indices.get(selected)?;
        self.all_entries.get(idx)
    }

    /// Get the currently selected crash report (if any).
    pub fn selected_crash_report(&self) -> Option<&CrashReportEntry> {
        let selected = self.crashes.list_state.selected()?;
        self.report.crash_report_entries.get(selected)
    }

    /// Find the panic log entry that corresponds to the selected crash report.
    pub fn selected_crash_panic_entry(&self) -> Option<&LogEntryRef<'_>> {
        let crash = self.selected_crash_report()?;
        crash.find_panic_entry(self.all_entries)
    }

    // -----------------------------------------------------------------------
    // Key dispatch
    // -----------------------------------------------------------------------

    /// Handle a key event. Returns `true` if the app should quit.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Help overlay intercepts all keys.
        if self.show_help {
            self.show_help = false;
            return false;
        }

        // Source picker overlay intercepts all keys when open.
        if self.logs.show_source_picker {
            return self.handle_source_picker_key(key);
        }

        // Log file picker overlay intercepts all keys when open.
        if self.logs.show_log_file_picker {
            return self.handle_log_file_picker_key(key);
        }

        use keys::{ListSelectTarget, PaneSelectTarget};
        match self.input_mode {
            InputMode::Search => self.handle_search_key(key),
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Select if self.tab == Tab::Overview => {
                self.handle_pane_select_key(key, PaneSelectTarget::Overview)
            }
            InputMode::Select if self.tab == Tab::Analysis => {
                self.handle_pane_select_key(key, PaneSelectTarget::Analysis)
            }
            InputMode::Select if self.tab == Tab::CrashReports && self.crashes.detail_selecting => {
                self.handle_pane_select_key(key, PaneSelectTarget::CrashDetail)
            }
            InputMode::Select if self.tab == Tab::CrashReports => {
                self.handle_list_select_key(key, ListSelectTarget::Crashes)
            }
            InputMode::Select if self.logs.detail_selecting => {
                self.handle_pane_select_key(key, PaneSelectTarget::LogDetail)
            }
            InputMode::Select => self.handle_list_select_key(key, ListSelectTarget::Logs),
        }
    }
}
