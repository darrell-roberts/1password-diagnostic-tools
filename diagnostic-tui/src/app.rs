//! Application state and input handling for the diagnostic TUI.

use crossterm::event::{KeyCode, KeyEvent};
use diagnostic_parser::log_entry::{LogEntry, LogLevel};
use diagnostic_parser::model::{CrashReportEntry, DiagnosticReport};

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

/// Whether the user is in normal navigation mode or typing into the search bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
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
            detail_focused: false,
            show_log_detail: false,
            show_help: false,
            show_source_picker: false,
            source_picker_selected: 0,
            show_log_file_picker: false,
            log_file_picker_selected: 0,
            viewport: ViewportHeights::default(),
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
        }
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
            KeyCode::Char('f') if self.tab == Tab::Logs => {
                self.level_filter.cycle();
                self.refilter();
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
                self.overview_scroll = self.overview_scroll.saturating_sub(1);
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
                self.overview_scroll += 1;
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
                let page = self.viewport.overview;
                self.overview_scroll = self.overview_scroll.saturating_sub(page);
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
                let page = self.viewport.overview;
                self.overview_scroll += page;
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
                self.overview_scroll = 0;
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
                // Will be clamped by the renderer.
                self.overview_scroll = u16::MAX;
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
