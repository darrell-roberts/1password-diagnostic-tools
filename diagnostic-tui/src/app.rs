//! Application state and input handling for the diagnostic TUI.

use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use diagnostic_parser::log_entry::{LogEntry, LogLevel};
use diagnostic_parser::model::{CrashReportEntry, DiagnosticReport};
use std::time::Instant;

// ---------------------------------------------------------------------------
// Active tab / panel
// ---------------------------------------------------------------------------

/// The top-level tab currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Logs,
    CrashReports,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Overview, Tab::Logs, Tab::CrashReports];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Logs => "Logs",
            Tab::CrashReports => "Crash Reports",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Tab::Overview => Tab::Logs,
            Tab::Logs => Tab::CrashReports,
            Tab::CrashReports => Tab::Overview,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Tab::Overview => Tab::CrashReports,
            Tab::Logs => Tab::Overview,
            Tab::CrashReports => Tab::Logs,
        }
    }
}

// ---------------------------------------------------------------------------
// Input mode
// ---------------------------------------------------------------------------

/// Whether the user is in normal navigation mode, typing into the search bar,
/// or selecting a range of log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    /// Visual selection mode on the Logs, Crash Reports, or Overview tab.
    /// The anchor index is stored in `App::select_anchor` (Logs),
    /// `App::crash_select_anchor` (Crashes), or `App::overview_select_anchor` (Overview).
    Select,
}

// ---------------------------------------------------------------------------
// Log level filter
// ---------------------------------------------------------------------------

/// Minimum log level threshold for filtering entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelFilter {
    pub show_trace: bool,
    pub show_debug: bool,
    pub show_info: bool,
    pub show_warn: bool,
    pub show_error: bool,
}

impl Default for LevelFilter {
    fn default() -> Self {
        Self {
            show_trace: true,
            show_debug: true,
            show_info: true,
            show_warn: true,
            show_error: true,
        }
    }
}

impl LevelFilter {
    pub fn accepts(&self, level: LogLevel) -> bool {
        match level {
            LogLevel::Trace => self.show_trace,
            LogLevel::Debug => self.show_debug,
            LogLevel::Info => self.show_info,
            LogLevel::Warn => self.show_warn,
            LogLevel::Error => self.show_error,
        }
    }

    /// Cycle through preset filter levels.
    pub fn cycle(&mut self) {
        // All -> Error only -> Warn+ -> Info+ -> Debug+ -> All
        if self.show_trace {
            // Currently showing all → show Error only
            *self = Self {
                show_trace: false,
                show_debug: false,
                show_info: false,
                show_warn: false,
                show_error: true,
            };
        } else if !self.show_warn && !self.show_info && !self.show_debug {
            // Error only → Warn+
            *self = Self {
                show_trace: false,
                show_debug: false,
                show_info: false,
                show_warn: true,
                show_error: true,
            };
        } else if !self.show_info && !self.show_debug {
            // Warn+ → Info+
            *self = Self {
                show_trace: false,
                show_debug: false,
                show_info: true,
                show_warn: true,
                show_error: true,
            };
        } else if !self.show_debug {
            // Info+ → Debug+
            *self = Self {
                show_trace: false,
                show_debug: true,
                show_info: true,
                show_warn: true,
                show_error: true,
            };
        } else {
            // Debug+ → All
            *self = Self::default();
        }
    }

    pub fn label(&self) -> &'static str {
        if self.show_trace {
            "ALL"
        } else if self.show_debug {
            "DEBUG+"
        } else if self.show_info {
            "INFO+"
        } else if self.show_warn {
            "WARN+"
        } else if self.show_error {
            "ERROR"
        } else {
            "NONE"
        }
    }
}

// ---------------------------------------------------------------------------
// Source filter
// ---------------------------------------------------------------------------

/// Optional filter by source component (e.g. "1P", "client", "status").
#[derive(Debug, Clone)]
pub struct SourceFilter {
    /// Available distinct component names extracted from log entries.
    pub available: Vec<String>,
    /// Index into `available`, or `None` for "show all".
    pub selected: Option<usize>,
}

impl SourceFilter {
    pub fn new(entries: &[LogEntry]) -> Self {
        let mut components: Vec<String> =
            entries.iter().map(|e| e.source.component.clone()).collect();
        components.sort();
        components.dedup();
        Self {
            available: components,
            selected: None,
        }
    }

    pub fn accepts(&self, entry: &LogEntry) -> bool {
        match self.selected {
            None => true,
            Some(idx) => entry.source.component == self.available[idx],
        }
    }

    pub fn cycle_next(&mut self) {
        if self.available.is_empty() {
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(i) if i + 1 >= self.available.len() => None,
            Some(i) => Some(i + 1),
        };
    }

    pub fn label(&self) -> &str {
        match self.selected {
            None => "All Sources",
            Some(idx) => &self.available[idx],
        }
    }
}

// ---------------------------------------------------------------------------
// Log file filter
// ---------------------------------------------------------------------------

/// Optional filter by log file name (e.g. "app.log", "network.log").
#[derive(Debug, Clone)]
pub struct LogFileFilter {
    /// Available distinct log file names extracted from log entries.
    pub available: Vec<String>,
    /// Index into `available`, or `None` for "show all".
    pub selected: Option<usize>,
}

impl LogFileFilter {
    pub fn new(entries: &[LogEntry]) -> Self {
        let mut log_files: Vec<String> = entries.iter().map(|e| e.log_file_title.clone()).collect();
        log_files.sort();
        log_files.dedup();
        Self {
            available: log_files,
            selected: None,
        }
    }

    pub fn accepts(&self, entry: &LogEntry) -> bool {
        match self.selected {
            None => true,
            Some(idx) => entry.log_file_title == self.available[idx],
        }
    }

    pub fn cycle_next(&mut self) {
        if self.available.is_empty() {
            return;
        }
        self.selected = match self.selected {
            None => Some(0),
            Some(i) if i + 1 >= self.available.len() => None,
            Some(i) => Some(i + 1),
        };
    }

    pub fn label(&self) -> &str {
        match self.selected {
            None => "All Log Files",
            Some(idx) => &self.available[idx],
        }
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Root application state.
/// Last-known viewport heights (in rows) for each scrollable region.
///
/// These are updated by the rendering code in `ui.rs` each frame and read
/// by the key handlers so that Page Up / Page Down move by exactly one
/// screen height.
#[derive(Debug, Clone, Copy)]
pub struct ViewportHeights {
    pub overview: u16,
    pub log_list: u16,
    pub log_detail: u16,
    pub crash_list: u16,
    pub crash_detail: u16,
    pub source_picker: u16,
    pub log_file_picker: u16,
}

impl Default for ViewportHeights {
    fn default() -> Self {
        Self {
            overview: 20,
            log_list: 20,
            log_detail: 20,
            crash_list: 20,
            crash_detail: 20,
            source_picker: 20,
            log_file_picker: 20,
        }
    }
}

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

    /// System clipboard handle, created once at startup.
    pub clipboard: Option<Clipboard>,

    /// Instant when the last successful copy occurred, used to flash feedback.
    pub copied_at: Option<Instant>,
}

/// Minimal list state tracker (selected index + offset for scrolling).
pub struct ListState {
    pub selected: usize,
    pub offset: usize,
}

impl ListState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            offset: 0,
        }
    }

    /// Move selection up by one, clamping at 0.
    pub fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Move selection down by one, clamping at `max - 1`.
    pub fn down(&mut self, max: usize) {
        if max > 0 && self.selected < max - 1 {
            self.selected += 1;
        }
    }

    /// Jump up by `n` items.
    pub fn page_up(&mut self, n: usize) {
        self.selected = self.selected.saturating_sub(n);
    }

    /// Jump down by `n` items.
    pub fn page_down(&mut self, n: usize, max: usize) {
        if max > 0 {
            self.selected = (self.selected + n).min(max - 1);
        }
    }

    /// Go to first item.
    pub fn home(&mut self) {
        self.selected = 0;
    }

    /// Go to last item.
    pub fn end(&mut self, max: usize) {
        if max > 0 {
            self.selected = max - 1;
        }
    }

    /// Ensure `selected` is visible given a viewport height. Updates `offset`.
    pub fn ensure_visible(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + viewport_height {
            self.offset = self.selected - viewport_height + 1;
        }
    }
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
        }
    }

    /// Recompute `filtered_indices` based on current search query, level
    /// filter, and source filter.
    pub fn refilter(&mut self) {
        self.refilter_inner(None);
    }

    /// Refilter while preserving the currently selected `all_entries` index.
    /// After the new filtered list is built, the selection is moved to the
    /// position of the previously selected entry (or the nearest earlier entry
    /// if it was filtered out).
    pub fn refilter_preserving_selection(&mut self) {
        // Resolve the current selection to an all_entries index.
        let pinned = self
            .filtered_indices
            .get(self.log_list_state.selected)
            .copied();
        self.refilter_inner(pinned);
    }

    fn refilter_inner(&mut self, pinned_all_entry_idx: Option<usize>) {
        let query_lower = self.search_query.to_lowercase();
        self.filtered_indices = self
            .all_entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                self.level_filter.accepts(entry.level)
                    && self.source_filter.accepts(entry)
                    && self.log_file_filter.accepts(entry)
            })
            .filter(|(_, entry)| {
                if query_lower.is_empty() {
                    return true;
                }
                entry.message.to_lowercase().contains(&query_lower)
                    || entry
                        .continuation
                        .iter()
                        .any(|c| c.to_lowercase().contains(&query_lower))
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
            InputMode::Select => self.handle_select_key(key),
        }
    }

    /// Returns the ordered (start, end) selection range for the Overview tab if in select mode.
    pub fn overview_selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.overview_select_anchor?;
        let cursor = self.overview_cursor;
        Some((anchor.min(cursor), anchor.max(cursor)))
    }

    /// Returns the ordered (start, end) selection range for the Logs tab if in select mode.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.select_anchor?;
        let cursor = self.log_list_state.selected;
        Some((anchor.min(cursor), anchor.max(cursor)))
    }

    /// Returns the ordered (start, end) selection range for the Crash list if in select mode.
    pub fn crash_selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.crash_select_anchor?;
        let cursor = self.crash_list_state.selected;
        Some((anchor.min(cursor), anchor.max(cursor)))
    }

    /// Copy the selected log entries to the system clipboard.
    fn copy_selection(&mut self) {
        let Some((start, end)) = self.selection_range() else {
            return;
        };

        let text: String = (start..=end)
            .filter_map(|i| self.filtered_indices.get(i).copied())
            .filter_map(|idx| self.all_entries.get(idx))
            .map(|entry| {
                let mut line = format!(
                    "{} {} [{}] {}",
                    entry.timestamp,
                    entry.level,
                    entry.source.raw(),
                    entry.message,
                );
                for cont in &entry.continuation {
                    line.push('\n');
                    line.push_str(cont);
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n");

        if let Some(ref mut cb) = self.clipboard
            && cb.set_text(text).is_ok()
        {
            self.copied_at = Some(Instant::now());
        }

        // Exit select mode.
        self.select_anchor = None;
        self.input_mode = InputMode::Normal;
    }

    /// Format a single crash report (and its linked panic entry) as copyable plain text.
    fn format_crash_text(&self, crash: &CrashReportEntry) -> String {
        let ts = crash
            .timestamp_utc()
            .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| format!("{}", crash.timestamp));

        let mut text = format!(
            "Report ID: {}\nType:      {}\nTimestamp: {}\nTag:       {}",
            crash.report_id, crash.report_type, ts, crash.diagnostic_report_tag,
        );

        if let Some(entry) = crash.find_panic_entry(&self.all_entries) {
            text.push_str(&format!(
                "\n\nLinked Panic Entry\nLog File:  {}\nThread:    {}\nSource:    {}\nTimestamp: {}\n\nMessage:\n{}",
                entry.log_file_title,
                entry.thread,
                entry.source.raw(),
                entry.timestamp,
                entry.message,
            ));
            if entry.has_continuation() {
                text.push_str(&format!(
                    "\n\nCall Stack ({} frames):",
                    entry.continuation.len()
                ));
                for frame_line in &entry.continuation {
                    text.push('\n');
                    text.push_str(frame_line.trim_start());
                }
            }
        } else {
            text.push_str("\n\nNo matching panic log entry found.");
        }

        text
    }

    /// Copy the selected crash reports to the system clipboard.
    fn copy_crash_selection(&mut self) {
        let (start, end) = match self.crash_selection_range() {
            Some(range) => range,
            None => {
                // Single-entry copy when no visual selection is active.
                let idx = self.crash_list_state.selected;
                (idx, idx)
            }
        };

        let text: String = (start..=end)
            .filter_map(|i| self.report.crash_report_entries.get(i))
            .map(|crash| self.format_crash_text(crash))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        if let Some(ref mut cb) = self.clipboard
            && cb.set_text(text).is_ok()
        {
            self.copied_at = Some(Instant::now());
        }

        // Exit select mode.
        self.crash_select_anchor = None;
        self.input_mode = InputMode::Normal;
    }

    /// Handle keys while in visual-select mode on the Overview tab.
    fn handle_overview_select_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Cancel selection.
            KeyCode::Esc => {
                self.overview_select_anchor = None;
                self.input_mode = InputMode::Normal;
            }
            // Yank (copy) selection.
            KeyCode::Char('y') => {
                self.copy_overview_selection();
            }
            // Navigation still works while selecting.
            KeyCode::Up | KeyCode::Char('k') => {
                if self.overview_cursor > 0 {
                    self.overview_cursor -= 1;
                    self.ensure_overview_cursor_visible();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.overview_line_count > 0
                    && self.overview_cursor + 1 < self.overview_line_count
                {
                    self.overview_cursor += 1;
                    self.ensure_overview_cursor_visible();
                }
            }
            KeyCode::PageUp => {
                let page = self.viewport.overview as usize;
                self.overview_cursor = self.overview_cursor.saturating_sub(page);
                self.ensure_overview_cursor_visible();
            }
            KeyCode::PageDown => {
                let page = self.viewport.overview as usize;
                if self.overview_line_count > 0 {
                    self.overview_cursor =
                        (self.overview_cursor + page).min(self.overview_line_count - 1);
                }
                self.ensure_overview_cursor_visible();
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.overview_cursor = 0;
                self.ensure_overview_cursor_visible();
            }
            KeyCode::End | KeyCode::Char('G') => {
                if self.overview_line_count > 0 {
                    self.overview_cursor = self.overview_line_count - 1;
                }
                self.ensure_overview_cursor_visible();
            }
            _ => {}
        }
        false
    }

    /// Ensure the overview cursor line is visible within the current viewport.
    fn ensure_overview_cursor_visible(&mut self) {
        let viewport_h = self.viewport.overview as usize;
        let scroll = self.overview_scroll as usize;
        if self.overview_cursor < scroll {
            self.overview_scroll = self.overview_cursor as u16;
        } else if viewport_h > 0 && self.overview_cursor >= scroll + viewport_h {
            self.overview_scroll = (self.overview_cursor - viewport_h + 1) as u16;
        }
    }

    /// Copy the selected overview lines to the system clipboard.
    fn copy_overview_selection(&mut self) {
        let Some((start, end)) = self.overview_selection_range() else {
            return;
        };

        let text = self.build_overview_plain_text(start, end);

        if let Some(ref mut cb) = self.clipboard
            && cb.set_text(text).is_ok()
        {
            self.copied_at = Some(Instant::now());
        }

        // Exit select mode.
        self.overview_select_anchor = None;
        self.input_mode = InputMode::Normal;
    }

    /// Build plain-text representation of overview lines in the given range.
    pub fn build_overview_plain_text(&self, start: usize, end: usize) -> String {
        let lines = self.build_overview_text_lines();
        lines[start..=end.min(lines.len().saturating_sub(1))]
            .to_vec()
            .join("\n")
    }

    /// Build the overview content as plain-text lines (mirrors the styled lines
    /// produced by `draw_overview` in `ui.rs`).
    pub fn build_overview_text_lines(&self) -> Vec<String> {
        let report = &self.report;
        let sys = &report.system;

        let created = report
            .created_at_utc()
            .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| format!("{}", report.created_at));

        let mut lines: Vec<String> = Vec::new();

        // Report Information
        lines.push("Report Information".to_string());
        lines.push(String::new());
        lines.push(format!("  UUID: {}", report.uuid));
        lines.push(format!("  Created: {}", created));
        lines.push(String::new());

        // System
        lines.push("System".to_string());
        lines.push(String::new());
        lines.push(format!("  Client: {}", sys.client_name));
        lines.push(format!("  Build: {}", sys.client_build));
        lines.push(format!("  OS: {} {}", sys.os_name, sys.os_version));
        lines.push(format!("  Processor: {}", sys.client_processor));
        lines.push(format!("  Memory: {}", sys.memory));
        lines.push(format!("  Disk (total): {}", sys.total_space));
        lines.push(format!("  Disk (free): {}", sys.free_space));
        lines.push(format!("  Locale: {}", sys.locale));
        lines.push(format!("  Locked: {}", sys.client_is_locked));
        if !sys.install_location.is_empty() {
            lines.push(format!("  Install Path: {}", sys.install_location));
        }
        lines.push(String::new());

        // Overview counters
        lines.push("Overview".to_string());
        lines.push(String::new());
        if let Some(ref overview) = report.overview {
            lines.push(format!("  Accounts: {}", overview.accounts));
            lines.push(format!("  Vaults: {}", overview.vaults));
            lines.push(format!("  Active Items: {}", overview.active_items));
            lines.push(format!("  Inactive Items: {}", overview.inactive_items));
        } else {
            lines.push("  (not available for this client)".to_string());
        }
        lines.push(String::new());

        // Accounts
        lines.push("Accounts".to_string());
        lines.push(String::new());

        for (i, account) in report.accounts.iter().enumerate() {
            lines.push(format!("  Account {} - {}", i + 1, account.uuid));
            lines.push(format!("    URL: {}", account.url));
            lines.push(format!("    Type: {}", account.account_type));
            let acct_state_str = account
                .account_state
                .map(|s| s.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            lines.push(format!("    State: {}", acct_state_str));
            let billing_str = account
                .billing_status
                .map(|b| b.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            lines.push(format!("    Billing: {}", billing_str));
            lines.push(format!("    Locked: {}", account.account_is_locked));
            let storage_str = Self::format_bytes_static(account.storage_used);
            lines.push(format!("    Storage Used: {}", storage_str));
            lines.push(format!("    Vaults: {}", account.vaults.len()));

            for vault in &account.vaults {
                lines.push(format!(
                    "      {} {}  {} active, {} archived, {} deleted",
                    vault.vault_type,
                    vault.uuid,
                    vault.items.active,
                    vault.items.archived,
                    vault.items.deleted,
                ));
            }
            lines.push(String::new());
        }

        // Feature Flags
        if !sys.features.is_empty() {
            lines.push("Feature Flags".to_string());
            lines.push(String::new());
            for feat in &sys.features {
                lines.push(format!("  * {}", feat.name));
            }
            lines.push(String::new());
        }

        // Log Files
        lines.push("Log Files".to_string());
        lines.push(String::new());
        lines.push(format!("  Files: {}", report.logs.len()));
        lines.push(format!("  Total Lines: {}", report.total_log_lines()));
        lines.push(format!("  Parsed Entries: {}", self.all_entries.len()));

        // Level breakdown
        let mut by_level = [0usize; 5];
        for entry in &self.all_entries {
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
        for i in 0..5 {
            if by_level[i] > 0 {
                lines.push(format!("  {:<5} {}", level_labels[i], by_level[i]));
            }
        }
        lines.push(String::new());

        // Crash Reports
        lines.push("Crash Reports".to_string());
        lines.push(String::new());
        lines.push(format!("  Count: {}", report.crash_report_entries.len()));

        lines
    }

    fn format_bytes_static(bytes: u64) -> String {
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

    /// Handle keys while in visual-select mode.
    fn handle_select_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Cancel selection.
            KeyCode::Esc => {
                self.select_anchor = None;
                self.input_mode = InputMode::Normal;
            }
            // Yank (copy) selection.
            KeyCode::Char('y') => {
                self.copy_selection();
            }
            // Navigation still works while selecting.
            KeyCode::Up | KeyCode::Char('k') => {
                self.log_list_state.up();
                self.detail_scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.filtered_indices.len();
                self.log_list_state.down(max);
                self.detail_scroll = 0;
            }
            KeyCode::PageUp => {
                let page = self.viewport.log_list as usize;
                self.log_list_state.page_up(page);
                self.detail_scroll = 0;
            }
            KeyCode::PageDown => {
                let page = self.viewport.log_list as usize;
                let max = self.filtered_indices.len();
                self.log_list_state.page_down(page, max);
                self.detail_scroll = 0;
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.log_list_state.home();
                self.detail_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                let max = self.filtered_indices.len();
                self.log_list_state.end(max);
                self.detail_scroll = 0;
            }
            _ => {}
        }
        false
    }

    /// Handle keys while in visual-select mode on the Crash list.
    fn handle_crash_select_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Cancel selection.
            KeyCode::Esc => {
                self.crash_select_anchor = None;
                self.input_mode = InputMode::Normal;
            }
            // Yank (copy) selection.
            KeyCode::Char('y') => {
                self.copy_crash_selection();
            }
            // Navigation still works while selecting.
            KeyCode::Up | KeyCode::Char('k') => {
                self.crash_list_state.up();
                self.crash_detail_scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.report.crash_report_entries.len();
                self.crash_list_state.down(max);
                self.crash_detail_scroll = 0;
            }
            KeyCode::PageUp => {
                let page = self.viewport.crash_list as usize;
                self.crash_list_state.page_up(page);
                self.crash_detail_scroll = 0;
            }
            KeyCode::PageDown => {
                let page = self.viewport.crash_list as usize;
                let max = self.report.crash_report_entries.len();
                self.crash_list_state.page_down(page, max);
                self.crash_detail_scroll = 0;
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.crash_list_state.home();
                self.crash_detail_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                let max = self.report.crash_report_entries.len();
                self.crash_list_state.end(max);
                self.crash_detail_scroll = 0;
            }
            _ => {}
        }
        false
    }

    /// Handle mouse scroll-up events.
    pub fn handle_scroll_up(&mut self) {
        // Dismiss help overlay on any scroll.
        if self.show_help {
            self.show_help = false;
            return;
        }

        // Scroll inside the source picker when it is open.
        if self.show_source_picker {
            if self.source_picker_selected > 0 {
                self.source_picker_selected -= 1;
            }
            return;
        }

        // Scroll inside the log file picker when it is open.
        if self.show_log_file_picker {
            if self.log_file_picker_selected > 0 {
                self.log_file_picker_selected -= 1;
            }
            return;
        }

        // Scroll 3 lines at a time for a comfortable feel.
        for _ in 0..3 {
            self.navigate_up();
        }
    }

    /// Handle mouse scroll-down events.
    pub fn handle_scroll_down(&mut self) {
        // Dismiss help overlay on any scroll.
        if self.show_help {
            self.show_help = false;
            return;
        }

        // Scroll inside the source picker when it is open.
        if self.show_source_picker {
            let total = 1 + self.source_filter.available.len();
            if self.source_picker_selected + 1 < total {
                self.source_picker_selected += 1;
            }
            return;
        }

        // Scroll inside the log file picker when it is open.
        if self.show_log_file_picker {
            let total = 1 + self.log_file_filter.available.len();
            if self.log_file_picker_selected + 1 < total {
                self.log_file_picker_selected += 1;
            }
            return;
        }

        // Scroll 3 lines at a time for a comfortable feel.
        for _ in 0..3 {
            self.navigate_down();
        }
    }

    /// Handle keys when the log file picker popup is open.
    fn handle_log_file_picker_key(&mut self, key: KeyEvent) -> bool {
        let total = 1 + self.log_file_filter.available.len();
        let page = self.viewport.log_file_picker as usize;

        match key.code {
            KeyCode::Esc | KeyCode::Char('L') | KeyCode::Char('l') => {
                self.show_log_file_picker = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.log_file_picker_selected > 0 {
                    self.log_file_picker_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.log_file_picker_selected + 1 < total {
                    self.log_file_picker_selected += 1;
                }
            }
            KeyCode::PageUp => {
                self.log_file_picker_selected = self.log_file_picker_selected.saturating_sub(page);
            }
            KeyCode::PageDown => {
                if total > 0 {
                    self.log_file_picker_selected =
                        (self.log_file_picker_selected + page).min(total - 1);
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.log_file_picker_selected = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if total > 0 {
                    self.log_file_picker_selected = total - 1;
                }
            }
            KeyCode::Enter => {
                if self.log_file_picker_selected == 0 {
                    self.log_file_filter.selected = None;
                } else {
                    self.log_file_filter.selected = Some(self.log_file_picker_selected - 1);
                }
                self.show_log_file_picker = false;
                self.refilter();
            }
            _ => {}
        }
        false
    }

    /// Handle keys when the source picker popup is open.
    fn handle_source_picker_key(&mut self, key: KeyEvent) -> bool {
        // Total items: 1 ("All Sources") + number of available sources.
        let total = 1 + self.source_filter.available.len();
        let page = self.viewport.source_picker as usize;

        match key.code {
            KeyCode::Esc | KeyCode::Char('S') | KeyCode::Char('s') => {
                self.show_source_picker = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.source_picker_selected > 0 {
                    self.source_picker_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.source_picker_selected + 1 < total {
                    self.source_picker_selected += 1;
                }
            }
            KeyCode::PageUp => {
                self.source_picker_selected = self.source_picker_selected.saturating_sub(page);
            }
            KeyCode::PageDown => {
                if total > 0 {
                    self.source_picker_selected =
                        (self.source_picker_selected + page).min(total - 1);
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.source_picker_selected = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if total > 0 {
                    self.source_picker_selected = total - 1;
                }
            }
            KeyCode::Enter => {
                if self.source_picker_selected == 0 {
                    self.source_filter.selected = None;
                } else {
                    self.source_filter.selected = Some(self.source_picker_selected - 1);
                }
                self.show_source_picker = false;
                self.refilter();
            }
            _ => {}
        }
        false
    }

    /// Handle keys when in search input mode.
    fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.refilter();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.refilter();
            }
            _ => {}
        }
        false
    }

    /// Handle keys when in normal navigation mode.
    fn handle_normal_key(&mut self, key: KeyEvent) -> bool {
        // Clear the "Copied!" flash after a short time on any keypress.
        if self
            .copied_at
            .is_some_and(|t| t.elapsed().as_millis() > 300)
        {
            self.copied_at = None;
        }

        let control_pressed = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            // Quit.
            KeyCode::Char('q') => return true,

            // Help.
            KeyCode::Char('?') => {
                self.show_help = true;
            }

            // Tab switching.
            KeyCode::Tab | KeyCode::Right if key.modifiers.is_empty() && self.tab_nav_keys() => {
                // Right arrow is only for tab nav on overview; on logs/crashes it
                // might be used differently. We use Tab universally.
                if key.code == KeyCode::Tab {
                    self.tab = self.tab.next();
                    self.detail_focused = false;
                    self.show_log_detail = false;
                }
            }
            KeyCode::BackTab => {
                self.tab = self.tab.prev();
                self.detail_focused = false;
                self.show_log_detail = false;
            }
            KeyCode::Char('1') => {
                self.tab = Tab::Overview;
                self.detail_focused = false;
                self.show_log_detail = false;
            }
            KeyCode::Char('2') => {
                self.tab = Tab::Logs;
                self.detail_focused = false;
            }
            KeyCode::Char('3') => {
                self.tab = Tab::CrashReports;
                self.detail_focused = false;
                self.show_log_detail = false;
            }

            // Search (only on Logs tab).
            KeyCode::Char('/') if self.tab == Tab::Logs => {
                self.input_mode = InputMode::Search;
            }

            // Clear search / close log detail / unfocus detail.
            KeyCode::Esc => {
                if self.tab == Tab::Logs && self.show_log_detail {
                    self.show_log_detail = false;
                    self.detail_scroll = 0;
                } else if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.refilter_preserving_selection();
                } else {
                    self.detail_focused = false;
                }
            }

            // Level filter cycle.
            KeyCode::Char('f') if self.tab == Tab::Logs && !control_pressed => {
                self.level_filter.cycle();
                self.refilter();
            }

            KeyCode::Char('f') if control_pressed => {
                self.navigate_page_down();
            }

            KeyCode::Char('u') if control_pressed => {
                self.navigate_page_up();
            }

            // Source filter cycle.
            KeyCode::Char('s') if self.tab == Tab::Logs => {
                self.source_filter.cycle_next();
                self.refilter();
            }

            // Source picker popup.
            KeyCode::Char('S') if self.tab == Tab::Logs => {
                // Sync picker selection with current filter state.
                self.source_picker_selected = match self.source_filter.selected {
                    None => 0,
                    Some(idx) => idx + 1,
                };
                self.show_source_picker = true;
            }

            // Reset source filter to All Sources.
            KeyCode::Char('a') if self.tab == Tab::Logs => {
                self.source_filter.selected = None;
                self.refilter();
            }

            // Log file filter cycle.
            KeyCode::Char('l') if self.tab == Tab::Logs => {
                self.log_file_filter.cycle_next();
                self.refilter();
            }

            // Log file picker popup.
            KeyCode::Char('L') if self.tab == Tab::Logs => {
                self.log_file_picker_selected = match self.log_file_filter.selected {
                    None => 0,
                    Some(idx) => idx + 1,
                };
                self.show_log_file_picker = true;
            }

            // Reset log file filter to All Log Files (combine all logs).
            KeyCode::Char('A') if self.tab == Tab::Logs => {
                self.log_file_filter.selected = None;
                self.refilter();
            }

            // Toggle detail view.
            KeyCode::Char('d') | KeyCode::Enter => {
                if self.tab == Tab::Logs {
                    self.show_log_detail = !self.show_log_detail;
                    self.detail_scroll = 0;
                } else if self.tab == Tab::CrashReports {
                    self.detail_focused = !self.detail_focused;
                    self.crash_detail_scroll = 0;
                }
            }

            // Navigation.
            KeyCode::Up | KeyCode::Char('k') => self.navigate_up(),
            KeyCode::Down | KeyCode::Char('j') => self.navigate_down(),
            KeyCode::PageUp => self.navigate_page_up(),
            KeyCode::PageDown => self.navigate_page_down(),
            KeyCode::Home | KeyCode::Char('g') => self.navigate_home(),
            KeyCode::End | KeyCode::Char('G') => self.navigate_end(),

            // Visual select mode (Overview tab).
            KeyCode::Char('v') if self.tab == Tab::Overview => {
                self.overview_select_anchor = Some(self.overview_cursor);
                self.input_mode = InputMode::Select;
            }

            // Visual select mode (Logs tab).
            KeyCode::Char('v') if self.tab == Tab::Logs => {
                self.select_anchor = Some(self.log_list_state.selected);
                self.input_mode = InputMode::Select;
            }

            // Visual select mode (Crash Reports list — only when list is focused).
            KeyCode::Char('v') if self.tab == Tab::CrashReports && !self.detail_focused => {
                self.crash_select_anchor = Some(self.crash_list_state.selected);
                self.input_mode = InputMode::Select;
            }

            // Copy single line under cursor (Overview tab) — copies visible top line.
            KeyCode::Char('y') if self.tab == Tab::Overview => {
                self.overview_cursor = self.overview_scroll as usize;
                self.overview_select_anchor = Some(self.overview_cursor);
                self.copy_overview_selection();
            }

            // Copy single entry under cursor (Logs tab).
            KeyCode::Char('y') if self.tab == Tab::Logs => {
                self.select_anchor = Some(self.log_list_state.selected);
                self.copy_selection();
            }

            // Copy crash detail or single crash entry (Crash Reports tab).
            KeyCode::Char('y') if self.tab == Tab::CrashReports => {
                self.copy_crash_selection();
            }

            // Right arrow to open detail, left arrow to close it.
            KeyCode::Right if self.tab == Tab::Logs => {
                if !self.show_log_detail {
                    self.show_log_detail = true;
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Right if self.tab == Tab::CrashReports => {
                self.detail_focused = true;
                self.crash_detail_scroll = 0;
            }
            KeyCode::Left if self.tab == Tab::Logs => {
                if self.show_log_detail {
                    self.show_log_detail = false;
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Left if self.tab == Tab::CrashReports => {
                self.detail_focused = false;
            }

            _ => {}
        }
        false
    }

    fn tab_nav_keys(&self) -> bool {
        // Prevent Right arrow from being interpreted as tab-switch on Logs/Crashes.
        true
    }

    fn navigate_up(&mut self) {
        match self.tab {
            Tab::Overview => {
                if self.overview_cursor > 0 {
                    self.overview_cursor -= 1;
                    self.ensure_overview_cursor_visible();
                }
            }
            Tab::Logs => {
                self.log_list_state.up();
                self.detail_scroll = 0;
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    self.crash_detail_scroll = self.crash_detail_scroll.saturating_sub(1);
                } else {
                    self.crash_list_state.up();
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    fn navigate_down(&mut self) {
        match self.tab {
            Tab::Overview => {
                if self.overview_line_count > 0
                    && self.overview_cursor + 1 < self.overview_line_count
                {
                    self.overview_cursor += 1;
                    self.ensure_overview_cursor_visible();
                }
            }
            Tab::Logs => {
                let max = self.filtered_indices.len();
                self.log_list_state.down(max);
                self.detail_scroll = 0;
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    self.crash_detail_scroll += 1;
                } else {
                    let max = self.report.crash_report_entries.len();
                    self.crash_list_state.down(max);
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    fn navigate_page_up(&mut self) {
        match self.tab {
            Tab::Overview => {
                let page = self.viewport.overview as usize;
                self.overview_cursor = self.overview_cursor.saturating_sub(page);
                self.ensure_overview_cursor_visible();
            }
            Tab::Logs => {
                let page = self.viewport.log_list as usize;
                self.log_list_state.page_up(page);
                self.detail_scroll = 0;
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    let page = self.viewport.crash_detail;
                    self.crash_detail_scroll = self.crash_detail_scroll.saturating_sub(page);
                } else {
                    let page = self.viewport.crash_list as usize;
                    self.crash_list_state.page_up(page);
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    fn navigate_page_down(&mut self) {
        match self.tab {
            Tab::Overview => {
                let page = self.viewport.overview as usize;
                if self.overview_line_count > 0 {
                    self.overview_cursor =
                        (self.overview_cursor + page).min(self.overview_line_count - 1);
                }
                self.ensure_overview_cursor_visible();
            }
            Tab::Logs => {
                let page = self.viewport.log_list as usize;
                let max = self.filtered_indices.len();
                self.log_list_state.page_down(page, max);
                self.detail_scroll = 0;
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    let page = self.viewport.crash_detail;
                    self.crash_detail_scroll += page;
                } else {
                    let page = self.viewport.crash_list as usize;
                    let max = self.report.crash_report_entries.len();
                    self.crash_list_state.page_down(page, max);
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    fn navigate_home(&mut self) {
        match self.tab {
            Tab::Overview => {
                self.overview_cursor = 0;
                self.ensure_overview_cursor_visible();
            }
            Tab::Logs => {
                self.log_list_state.home();
                self.detail_scroll = 0;
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    self.crash_detail_scroll = 0;
                } else {
                    self.crash_list_state.home();
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }

    fn navigate_end(&mut self) {
        match self.tab {
            Tab::Overview => {
                if self.overview_line_count > 0 {
                    self.overview_cursor = self.overview_line_count - 1;
                }
                self.ensure_overview_cursor_visible();
            }
            Tab::Logs => {
                let max = self.filtered_indices.len();
                self.log_list_state.end(max);
                self.detail_scroll = 0;
            }
            Tab::CrashReports => {
                if self.detail_focused {
                    self.crash_detail_scroll = u16::MAX;
                } else {
                    let max = self.report.crash_report_entries.len();
                    self.crash_list_state.end(max);
                    self.crash_detail_scroll = 0;
                }
            }
        }
    }
}
