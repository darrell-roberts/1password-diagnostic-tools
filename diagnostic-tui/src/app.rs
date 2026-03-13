//! Application state and input handling for the diagnostic TUI.
//!
//! The `app` module is split into several sub-modules for maintainability:
//!
//! - [`state`] — core state types (`Tab`, `InputMode`, `ListState`, `ViewportHeights`)
//! - [`filters`] — log entry filter types (`LevelFilter`, `SourceFilter`, `LogFileFilter`)
//! - [`keys`] — keyboard input handlers for each mode
//! - [`navigation`] — directional movement, paging, and viewport scrolling
//! - [`clipboard`] — copy/paste, selection ranges, and plain-text builders

pub mod clipboard;
pub mod filters;
pub mod keys;
pub mod navigation;
pub mod state;

// Re-export the most commonly used types so callers can write `app::App`, etc.
pub use filters::{LevelFilter, LogFileFilter, SourceFilter};
pub use state::{InputMode, ListState, Tab, ViewportHeights};

use arboard::Clipboard;
use crossterm::event::KeyEvent;
use diagnostic_parser::log_entry::LogEntry;
use diagnostic_parser::model::{CrashReportEntry, DiagnosticReport};
use std::time::Instant;

/// Root application state.
pub struct App {
    /// The loaded diagnostic report.
    pub report: DiagnosticReport,

    /// All parsed log entries (immutable after construction).
    pub all_entries: Vec<LogEntry>,

    /// Indices into `all_entries` that pass the current filters.
    pub filtered_indices: Vec<usize>,

    /// Currently active tab.
    pub tab: Tab,

    /// Whether we are in search input mode.
    pub input_mode: InputMode,

    /// The search query string.
    pub search_query: String,

    /// Log level filter.
    pub level_filter: LevelFilter,

    /// Source component filter.
    pub source_filter: SourceFilter,

    /// Log file filter.
    pub log_file_filter: LogFileFilter,

    /// Selected row in the log list.
    pub log_list_state: ListState,

    /// Vertical scroll offset inside the log detail pane.
    pub detail_scroll: u16,

    /// Cursor line index in the log detail content (used in visual selection mode).
    pub detail_cursor: usize,

    /// Anchor line index for visual selection mode on the log detail pane.
    /// `None` when not in select mode.
    pub detail_select_anchor: Option<usize>,

    /// Total number of lines in the log detail content (set during rendering).
    pub detail_line_count: usize,

    /// Selected crash report index.
    pub crash_list_state: ListState,

    /// Vertical scroll offset inside the crash detail pane.
    pub crash_detail_scroll: u16,

    /// Overview tab scroll offset.
    pub overview_scroll: u16,

    /// Cursor line index in the overview content (used in visual selection mode).
    pub overview_cursor: usize,

    /// Anchor line index for visual selection mode on the Overview tab.
    /// `None` when not in select mode.
    pub overview_select_anchor: Option<usize>,

    /// Total number of lines in the overview content (set during rendering).
    pub overview_line_count: usize,

    /// Whether the detail pane is focused (for scrolling).
    pub detail_focused: bool,

    /// Whether the log detail pane is visible (toggled by Enter/Esc).
    pub show_log_detail: bool,

    /// Whether to show help overlay.
    pub show_help: bool,

    /// Whether to show the source picker popup.
    pub show_source_picker: bool,

    /// Currently highlighted index in the source picker list.
    /// Index 0 = "All Sources", index 1.. = individual sources.
    pub source_picker_selected: usize,

    /// Whether to show the log file picker popup.
    pub show_log_file_picker: bool,

    /// Currently highlighted index in the log file picker list.
    /// Index 0 = "All Log Files", index 1.. = individual log files.
    pub log_file_picker_selected: usize,

    /// Last-known viewport heights, updated each frame by `ui::draw`.
    pub viewport: ViewportHeights,

    /// Anchor index (into `filtered_indices`) for visual selection mode on the Logs tab.
    /// `None` when not in select mode.
    pub select_anchor: Option<usize>,

    /// Anchor index (into `crash_report_entries`) for visual selection on the Crash list.
    /// `None` when not in select mode.
    pub crash_select_anchor: Option<usize>,

    /// Whether the detail pane is in select mode (separate from list select mode).
    pub detail_selecting: bool,

    /// System clipboard handle, created once at startup.
    pub clipboard: Option<Clipboard>,

    /// Instant when the last successful copy occurred, used to flash feedback.
    pub copied_at: Option<Instant>,

    /// Whether the previous keypress was `z`, awaiting the second key of a
    /// two-key `z` command (`zz`, `zt`, `zb`).
    pub pending_z: bool,
}

impl App {
    pub fn new(report: DiagnosticReport) -> Self {
        let all_entries = report.parse_log_entries();
        let source_filter = SourceFilter::new(&all_entries);
        let log_file_filter = LogFileFilter::new(&all_entries);
        let filtered_indices: Vec<usize> = (0..all_entries.len()).collect();

        Self {
            report,
            all_entries,
            filtered_indices,
            tab: Tab::Overview,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            level_filter: LevelFilter::default(),
            source_filter,
            log_file_filter,
            log_list_state: ListState::new(),
            detail_scroll: 0,
            detail_cursor: 0,
            detail_select_anchor: None,
            detail_line_count: 0,
            detail_selecting: false,
            crash_list_state: ListState::new(),
            crash_detail_scroll: 0,
            overview_scroll: 0,
            overview_cursor: 0,
            overview_select_anchor: None,
            overview_line_count: 0,
            detail_focused: false,
            show_log_detail: false,
            show_help: false,
            show_source_picker: false,
            source_picker_selected: 0,
            show_log_file_picker: false,
            log_file_picker_selected: 0,
            viewport: ViewportHeights::default(),
            select_anchor: None,
            crash_select_anchor: None,
            clipboard: Clipboard::new().ok(),
            copied_at: None,
            pending_z: false,
        }
    }

    // -----------------------------------------------------------------------
    // Filtering
    // -----------------------------------------------------------------------

    /// Recompute `filtered_indices` based on current search query, level
    /// filter, and source filter.
    pub fn refilter(&mut self) {
        self.refilter_inner(None);
    }

    fn refilter_inner(&mut self, pinned_all_entry_idx: Option<usize>) {
        self.filtered_indices = self
            .all_entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                self.level_filter.accepts(entry.level)
                    && self.source_filter.accepts(entry)
                    && self.log_file_filter.accepts(entry)
            })
            .map(|(i, _)| i)
            .collect();

        if let Some(pinned) = pinned_all_entry_idx {
            // Try to find the exact entry in the new filtered list.
            // If the pinned entry was filtered out, fall back to the nearest
            // earlier entry by finding the last filtered index <= pinned.
            if let Some(pos) = self.filtered_indices.iter().position(|&idx| idx == pinned) {
                self.log_list_state.selected = pos;
            } else if let Some(pos) = self.filtered_indices.iter().rposition(|&idx| idx <= pinned) {
                self.log_list_state.selected = pos;
            } else {
                self.log_list_state.selected = 0;
            }
        } else {
            // Clamp selection.
            if !self.filtered_indices.is_empty() {
                if self.log_list_state.selected >= self.filtered_indices.len() {
                    self.log_list_state.selected = self.filtered_indices.len() - 1;
                }
            } else {
                self.log_list_state.selected = 0;
            }
        }
        self.detail_scroll = 0;
    }

    // -----------------------------------------------------------------------
    // Search navigation
    // -----------------------------------------------------------------------

    /// Returns `true` if the entry at `all_entries[idx]` matches the current
    /// search query (case-insensitive substring in message or continuation).
    fn entry_matches_query(&self, idx: usize, query_lower: &str) -> bool {
        if query_lower.is_empty() {
            return false;
        }
        let entry = &self.all_entries[idx];
        entry.message.to_lowercase().contains(query_lower)
            || entry
                .continuation
                .iter()
                .any(|c| c.to_lowercase().contains(query_lower))
    }

    /// Move the cursor forward to the next entry matching the search query.
    /// Wraps around to the beginning if no match is found after the cursor.
    pub fn find_next(&mut self) {
        if self.search_query.is_empty() || self.filtered_indices.is_empty() {
            return;
        }
        let query_lower = self.search_query.to_lowercase();
        let len = self.filtered_indices.len();
        // Search from current+1, wrapping around.
        for offset in 1..=len {
            let pos = (self.log_list_state.selected + offset) % len;
            if self.entry_matches_query(self.filtered_indices[pos], &query_lower) {
                self.log_list_state.selected = pos;
                self.detail_scroll = 0;
                return;
            }
        }
    }

    /// Move the cursor backward to the previous entry matching the search query.
    /// Wraps around to the end if no match is found before the cursor.
    pub fn find_prev(&mut self) {
        if self.search_query.is_empty() || self.filtered_indices.is_empty() {
            return;
        }
        let query_lower = self.search_query.to_lowercase();
        let len = self.filtered_indices.len();
        for offset in 1..=len {
            let pos = (self.log_list_state.selected + len - offset) % len;
            if self.entry_matches_query(self.filtered_indices[pos], &query_lower) {
                self.log_list_state.selected = pos;
                self.detail_scroll = 0;
                return;
            }
        }
    }

    /// Move the cursor to the nearest matching entry at or after the current
    /// position. Used for live search-as-you-type.
    pub fn find_nearest(&mut self) {
        if self.search_query.is_empty() || self.filtered_indices.is_empty() {
            return;
        }
        let query_lower = self.search_query.to_lowercase();
        let len = self.filtered_indices.len();
        // First try from current position forward.
        for offset in 0..len {
            let pos = (self.log_list_state.selected + offset) % len;
            if self.entry_matches_query(self.filtered_indices[pos], &query_lower) {
                self.log_list_state.selected = pos;
                self.detail_scroll = 0;
                return;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Get the currently selected log entry (if any).
    pub fn selected_log_entry(&self) -> Option<&LogEntry> {
        let idx = *self.filtered_indices.get(self.log_list_state.selected)?;
        self.all_entries.get(idx)
    }

    /// Get the currently selected crash report (if any).
    pub fn selected_crash_report(&self) -> Option<&CrashReportEntry> {
        self.report
            .crash_report_entries
            .get(self.crash_list_state.selected)
    }

    /// Find the panic log entry that corresponds to the selected crash report.
    pub fn selected_crash_panic_entry(&self) -> Option<&LogEntry> {
        let crash = self.selected_crash_report()?;
        crash.find_panic_entry(&self.all_entries)
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
        if self.show_source_picker {
            return self.handle_source_picker_key(key);
        }

        // Log file picker overlay intercepts all keys when open.
        if self.show_log_file_picker {
            return self.handle_log_file_picker_key(key);
        }

        match self.input_mode {
            InputMode::Search => self.handle_search_key(key),
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Select if self.tab == Tab::Overview => self.handle_overview_select_key(key),
            InputMode::Select if self.tab == Tab::CrashReports => self.handle_crash_select_key(key),
            InputMode::Select if self.detail_selecting => self.handle_detail_select_key(key),
            InputMode::Select => self.handle_select_key(key),
        }
    }
}
