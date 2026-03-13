//! Clipboard operations and plain-text building for copy/paste.
//!
//! This module contains all methods on [`App`] that deal with copying
//! selected content to the system clipboard, as well as helpers that
//! build plain-text representations of the overview, log detail, and
//! crash report panes.

use super::App;
use crate::app::state::InputMode;
use chrono::Local;
use diagnostic_parser::log_entry::LogLevel;
use diagnostic_parser::model::CrashReportEntry;
use std::time::Instant;

impl App {
    // -----------------------------------------------------------------------
    // Selection ranges
    // -----------------------------------------------------------------------

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

    /// Returns the ordered (start, end) selection range for the log detail if in select mode.
    pub fn detail_selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.detail_select_anchor?;
        let cursor = self.detail_cursor;
        Some((anchor.min(cursor), anchor.max(cursor)))
    }

    // -----------------------------------------------------------------------
    // Copy: log list
    // -----------------------------------------------------------------------

    /// Copy the selected log entries to the system clipboard.
    pub(super) fn copy_selection(&mut self) {
        let Some((start, end)) = self.selection_range() else {
            return;
        };

        let text: String = (start..=end)
            .filter_map(|i| self.filtered_indices.get(i).copied())
            .filter_map(|idx| self.all_entries.get(idx))
            .map(|entry| {
                let mut line = format!(
                    "{} {} [{}] {}",
                    entry.timestamp.with_timezone(&Local),
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

    // -----------------------------------------------------------------------
    // Copy: log detail pane
    // -----------------------------------------------------------------------

    /// Copy the selected detail lines to the system clipboard.
    pub(super) fn copy_detail_selection(&mut self) {
        let Some((start, end)) = self.detail_selection_range() else {
            return;
        };

        let lines = self.build_detail_plain_lines();
        let text: String = lines[start..=end.min(lines.len().saturating_sub(1))].join("\n");

        if let Some(ref mut cb) = self.clipboard
            && cb.set_text(text).is_ok()
        {
            self.copied_at = Some(Instant::now());
        }

        // Exit detail select mode.
        self.detail_select_anchor = None;
        self.detail_selecting = false;
        self.input_mode = InputMode::Normal;
    }

    /// Build the plain-text lines shown in the log detail pane for the
    /// currently selected entry. Returns an empty vec when nothing is selected.
    pub fn build_detail_plain_lines(&self) -> Vec<String> {
        let Some(entry) = self.selected_log_entry() else {
            return Vec::new();
        };

        let mut lines: Vec<String> = Vec::new();

        lines.push(format!("Level:     {}", entry.level.as_str()));
        lines.push(format!(
            "Timestamp: {}",
            entry.timestamp.with_timezone(&Local)
        ));

        if !entry.thread.is_empty() {
            lines.push(format!("Thread:    {}", entry.thread));
        }

        lines.push(format!("Source:    {}", entry.source.raw()));

        if let Some(fp) = entry.source.file_path() {
            lines.push(format!("File:      {}", fp));
        }

        if let Some(ln) = entry.source.line_number() {
            lines.push(format!("Line:      {}", ln));
        }

        lines.push(format!("Log File:  {}", entry.log_file_title));

        lines.push(String::new());
        lines.push("Message:".to_string());
        lines.push(String::new());

        for msg_line in entry.message.lines() {
            lines.push(msg_line.to_string());
        }

        if entry.has_continuation() {
            lines.push(String::new());
            lines.push(format!(
                "Stack Trace ({} frames):",
                entry.continuation.len()
            ));
            lines.push(String::new());

            for cont_line in &entry.continuation {
                lines.push(cont_line.clone());
            }
        }

        lines
    }

    // -----------------------------------------------------------------------
    // Copy: crash reports
    // -----------------------------------------------------------------------

    /// Format a single crash report (and its linked panic entry) as copyable plain text.
    fn format_crash_text(&self, crash: &CrashReportEntry) -> String {
        let ts = crash
            .timestamp_utc()
            .map(|d| {
                d.with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
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
                entry.timestamp.with_timezone(&Local),
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
    pub(super) fn copy_crash_selection(&mut self) {
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

    // -----------------------------------------------------------------------
    // Copy: overview
    // -----------------------------------------------------------------------

    /// Copy the selected overview lines to the system clipboard.
    pub(super) fn copy_overview_selection(&mut self) {
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
    /// produced by `draw_overview` in `ui`).
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

    /// Format a byte count as a human-readable string (KB / MB / GB).
    pub(super) fn format_bytes_static(bytes: u64) -> String {
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
}
