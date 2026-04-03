//! Core state types used throughout the application.

// ---------------------------------------------------------------------------
// Active tab / panel
// ---------------------------------------------------------------------------

use ratatui::widgets::TableState;

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

/// Helper functions for key bindings.
pub trait ContainerStateExt {
    /// Move selection up by one.
    fn up(&mut self);
    /// Move selection down by one.
    fn down(&mut self);
    /// Page up
    fn page_up(&mut self, n: u16);
    /// Page down
    fn page_down(&mut self, n: u16);
    /// Go to top.
    fn home(&mut self);
    /// Go to bottom.
    fn end(&mut self);
}

impl ContainerStateExt for TableState {
    #[inline]
    fn up(&mut self) {
        self.select_previous();
    }

    #[inline]
    fn down(&mut self) {
        self.select_next();
    }

    #[inline]
    fn page_up(&mut self, n: u16) {
        self.scroll_up_by(n);
    }

    #[inline]
    fn page_down(&mut self, n: u16) {
        self.scroll_down_by(n);
    }

    #[inline]
    fn home(&mut self) {
        self.select_first();
    }

    #[inline]
    fn end(&mut self) {
        self.select_last();
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
