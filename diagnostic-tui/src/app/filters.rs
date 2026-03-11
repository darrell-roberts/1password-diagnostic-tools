//! Log entry filter types: level, source component, and log file.

use diagnostic_parser::log_entry::{LogEntry, LogLevel};

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
