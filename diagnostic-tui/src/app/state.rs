//! Core state types used throughout the application.

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
// List state
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Viewport heights
// ---------------------------------------------------------------------------

/// Last-known viewport heights (in rows) for each scrollable region.
///
/// These are updated by the rendering code in `ui` each frame and read
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
