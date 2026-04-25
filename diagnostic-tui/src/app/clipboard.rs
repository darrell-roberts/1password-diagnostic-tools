//! Clipboard operations and plain-text building for copy/paste.
//!
//! This module contains all methods on [`App`] that deal with copying
//! selected content to the system clipboard, as well as helpers that
//! build plain text representations of the overview, log detail, and
//! crash report panes.
use crate::{
    app::{App, state::InputMode},
    format_bytes,
};
use chrono::Local;
use diagnostic_parser::{LogEntry, log_entry::LogLevel, model::CrashReportEntry};
use std::time::Instant;

/// Build plain-text lines for a single crash report and its linked panic entry.
fn crash_report_plain_lines(
    crash: &CrashReportEntry,
    panic_entry: Option<&LogEntry<'_>>,
) -> Vec<String> {
    let ts = crash
        .timestamp_utc()
        .map(|d| {
            d.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| format!("{}", crash.timestamp));

    let mut lines = vec![
        format!("Report ID: {}", crash.report_id),
        format!("Type:      {}", crash.report_type),
        format!("Timestamp: {}", ts),
        format!("Tag:       {}", crash.diagnostic_report_tag),
        String::new(),
    ];

    if let Some(entry) = panic_entry {
        lines.extend([
            "Linked Panic Entry".to_string(),
            String::new(),
            format!("Log File:  {}", entry.log_file_title),
            format!("Thread:    {}", entry.thread),
            format!("Source:    {}", entry.source.raw()),
            format!("Timestamp: {}", entry.timestamp.with_timezone(&Local)),
            String::new(),
            "Message:".to_string(),
            String::new(),
        ]);
        lines.extend(entry.message.lines().map(ToString::to_string));

        if entry.has_continuation() {
            lines.extend([
                String::new(),
                format!("Call Stack ({} frames):", entry.continuation.len()),
                String::new(),
            ]);
            lines.extend(
                entry
                    .continuation
                    .iter()
                    .map(|frame_line| frame_line.trim_start().to_string()),
            );
        } else {
            lines.extend([
                String::new(),
                "(no stack trace attached to panic entry)".to_string(),
            ])
        }
    } else {
        lines.push("No matching panic log entry found.".to_string());
    }

    lines
}

impl App<'_> {
    /// Copy the selected log entries to the system clipboard.
    pub(super) fn copy_selection(&mut self) {
        let Some((start, end)) = self.logs.selection_range() else {
            return;
        };

        let count = end - start + 1;
        let text = (start..=end)
            .filter_map(|i| self.logs.filtered_indices.get(i).copied())
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

        if let Some(cb) = self.clipboard.as_mut()
            && cb.set_text(text).is_ok()
        {
            self.copied_at = Some(Instant::now());
            self.copied_count = count;
        }

        // Exit select mode.
        self.logs.select_anchor = None;
        self.input_mode = InputMode::Normal;
    }

    /// Copy the selected detail lines to the system clipboard.
    pub(super) fn copy_detail_selection(&mut self) {
        let Some((start, end)) = self.logs.detail_selection_range() else {
            return;
        };

        let lines = self.build_detail_plain_lines();
        let clamped_end = end.min(lines.len().saturating_sub(1));
        let count = clamped_end - start + 1;
        let text = lines[start..=clamped_end].join("\n");

        if let Some(cb) = self.clipboard.as_mut()
            && cb.set_text(text).is_ok()
        {
            self.copied_at = Some(Instant::now());
            self.copied_count = count;
        }

        // Exit detail select mode.
        self.logs.detail_select_anchor = None;
        self.logs.detail_selecting = false;
        self.input_mode = InputMode::Normal;
    }

    /// Build the plain-text lines shown in the log detail pane for the
    /// currently selected entry. Returns an empty vec when nothing is selected.
    fn build_detail_plain_lines(&self) -> Vec<String> {
        let Some(entry) = self.selected_log_entry() else {
            return Vec::new();
        };

        let mut lines = Vec::from(&[
            format!("Level:     {}", entry.level.as_str()),
            format!("Timestamp: {}", entry.timestamp.with_timezone(&Local)),
        ]);

        lines.extend((!entry.thread.is_empty()).then(|| format!("Thread:    {}", entry.thread)));
        lines.push(format!("Source:    {}", entry.source.raw()));

        if let Some(fp) = entry.source.file_path() {
            lines.push(format!("File:      {}", fp));
        }

        if let Some(ln) = entry.source.line_number() {
            lines.push(format!("Line:      {}", ln));
        }

        lines.extend([
            format!("Log File:  {}", entry.log_file_title),
            String::new(),
            "Message:".to_string(),
            String::new(),
        ]);

        lines.extend(entry.message.lines().map(ToOwned::to_owned));

        if entry.has_continuation() {
            lines.extend([
                String::new(),
                format!("Stack Trace ({} frames):", entry.continuation.len()),
                String::new(),
            ]);

            lines.extend(entry.continuation.iter().copied().map(ToOwned::to_owned));
        }

        lines
    }

    /// Copy the selected crash reports to the system clipboard.
    pub(super) fn copy_crash_selection(&mut self) {
        let (start, end) = match self.crashes.selection_range() {
            Some(range) => range,
            None => {
                // Single entry copy when no visual selection is active.
                let idx = self.crashes.list_state.selected().unwrap_or_default();
                (idx, idx)
            }
        };

        let text: String = (start..=end)
            .filter_map(|i| self.report.crash_report_entries.get(i))
            .map(|crash| {
                let panic = crash.find_panic_entry(self.all_entries);
                crash_report_plain_lines(crash, panic).join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        let count = end - start + 1;
        if let Some(ref mut cb) = self.clipboard
            && cb.set_text(text).is_ok()
        {
            self.copied_at = Some(Instant::now());
            self.copied_count = count;
        }

        // Exit select mode.
        self.crashes.select_anchor = None;
        self.input_mode = InputMode::Normal;
    }

    /// Copy the selected crash detail lines to the system clipboard.
    pub(super) fn copy_crash_detail_selection(&mut self) {
        let Some((start, end)) = self.crashes.detail_selection_range() else {
            return;
        };

        let lines = self
            .crashes
            .detail_plain_cache
            .clone()
            .unwrap_or_else(|| self.build_crash_detail_plain_lines());
        let clamped_end = end.min(lines.len().saturating_sub(1));
        let count = clamped_end - start + 1;
        let text: String = lines[start..=clamped_end].join("\n");

        if let Some(ref mut cb) = self.clipboard
            && cb.set_text(text).is_ok()
        {
            self.copied_at = Some(Instant::now());
            self.copied_count = count;
        }

        self.crashes.detail_select_anchor = None;
        self.crashes.detail_selecting = false;
        self.input_mode = InputMode::Normal;
    }

    /// Build the plain text lines shown in the crash detail pane for the
    /// currently selected crash report. Returns an empty vec when nothing is selected.
    pub fn build_crash_detail_plain_lines(&self) -> Vec<String> {
        let Some(crash) = self.selected_crash_report() else {
            return Vec::new();
        };
        crash_report_plain_lines(crash, self.selected_crash_panic_entry())
    }

    /// Copy the selected overview lines to the system clipboard.
    pub(super) fn copy_overview_selection(&mut self) {
        let Some((start, end)) = self.overview.selection_range() else {
            return;
        };

        let count = end - start + 1;
        let text = self.build_overview_plain_text(start, end);

        if let Some(cb) = self.clipboard.as_mut()
            && cb.set_text(text).is_ok()
        {
            self.copied_at = Some(Instant::now());
            self.copied_count = count;
        }

        // Exit select mode.
        self.overview.select_anchor = None;
        self.input_mode = InputMode::Normal;
    }

    /// Copy the selected analysis lines to the system clipboard.
    pub(super) fn copy_analysis_selection(&mut self) {
        let Some((start, end)) = self.analysis.selection_range() else {
            return;
        };

        let lines = self.analysis_data.build_plain_text_lines();
        let clamped_end = end.min(lines.len().saturating_sub(1));
        let count = clamped_end - start + 1;
        let text = lines[start..=clamped_end].join("\n");

        if let Some(cb) = self.clipboard.as_mut()
            && cb.set_text(text).is_ok()
        {
            self.copied_at = Some(Instant::now());
            self.copied_count = count;
        }

        // Exit select mode.
        self.analysis.select_anchor = None;
        self.input_mode = InputMode::Normal;
    }

    /// Build plain text representation of overview lines in the given range.
    pub fn build_overview_plain_text(&self, start: usize, end: usize) -> String {
        let lines = self.build_overview_text_lines();
        lines[start..=end.min(lines.len().saturating_sub(1))]
            .to_vec()
            .join("\n")
    }

    /// Build the overview content as plain text lines (mirrors the styled lines
    /// produced by `draw_overview` in `ui`).
    pub fn build_overview_text_lines(&self) -> Vec<String> {
        let report = &self.report;
        let sys = &report.system;

        let created = report
            .created_at_utc()
            .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| format!("{}", report.created_at));

        let mut lines = Vec::from(&[
            // Report Information
            "Report Information".to_string(),
            String::new(),
            format!("  UUID: {}", report.uuid),
            format!("  Created: {}", created),
            String::new(),
            // System
            "System".to_string(),
            String::new(),
            format!("  Client: {}", sys.client_name),
            format!("  Build: {}", sys.client_build),
            format!("  OS: {} {}", sys.os_name, sys.os_version),
            format!("  Processor: {}", sys.client_processor),
            format!("  Memory: {}", sys.memory),
            format!("  Disk (total): {}", sys.total_space),
            format!("  Disk (free): {}", sys.free_space),
            format!("  Locale: {}", sys.locale),
            format!("  Locked: {}", sys.client_is_locked),
        ]);

        if !sys.install_location.is_empty() {
            lines.push(format!("  Install Path: {}", sys.install_location));
        }

        // Overview counters
        lines.extend([String::new(), "Overview".to_string(), String::new()]);
        if let Some(ref overview) = report.overview {
            lines.extend([
                format!("  Accounts: {}", overview.accounts),
                format!("  Vaults: {}", overview.vaults),
                format!("  Active Items: {}", overview.active_items),
                format!("  Inactive Items: {}", overview.inactive_items),
            ]);
        } else {
            lines.push("  (not available for this client)".to_string());
        }

        // Accounts
        lines.extend([String::new(), "Accounts".to_string(), String::new()]);

        for (i, account) in report.accounts.iter().enumerate() {
            lines.extend([
                format!("  Account {} - {}", i + 1, account.uuid),
                format!("    URL: {}", account.url),
                format!("    Type: {}", account.account_type),
                format!(
                    "    State: {}",
                    account
                        .account_state
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "N/A".to_string())
                ),
                format!(
                    "    Billing: {}",
                    account
                        .billing_status
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "N/A".to_string())
                ),
                format!("    Locked: {}", account.account_is_locked),
                format!("    Storage Used: {}", format_bytes(account.storage_used)),
                format!("    Vaults: {}", account.vaults.len()),
            ]);

            if !account.vaults.is_empty() {
                lines.push(format!(
                    "      {:<14}  {:<36}  {:>8}  {:>10}  {:>9}",
                    "Vault Type", "UUID", "Active", "Archived", "Deleted"
                ));
            }

            lines.extend(account.vaults.iter().map(|vault| {
                format!(
                    "      {:<14}  {:<36}  {:>8}  {:>10}  {:>9}",
                    vault.vault_type,
                    vault.uuid,
                    vault.items.active,
                    vault.items.archived,
                    vault.items.deleted,
                )
            }));

            lines.push(String::new());
        }

        // Feature Flags
        if !sys.features.is_empty() {
            lines.extend(["Feature Flags".to_string(), String::new()]);
            lines.extend(
                sys.features
                    .iter()
                    .map(|feature| format!("  * {}", feature.name)),
            );

            lines.push(String::new());
        }

        // Log Files
        lines.extend([
            "Log Files".to_string(),
            String::new(),
            format!("  Files: {}", report.logs.len()),
            format!("  Total Lines: {}", self.total_log_lines),
            format!("  Parsed Entries: {}", self.all_entries.len()),
        ]);

        // Level breakdown
        let mut level_counts = [0; 5];
        for entry in self.all_entries {
            let idx = match entry.level {
                LogLevel::Error => 0,
                LogLevel::Warn => 1,
                LogLevel::Info => 2,
                LogLevel::Debug => 3,
                LogLevel::Trace => 4,
            };
            level_counts[idx] += 1;
        }
        lines.extend(
            ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"]
                .iter()
                .enumerate()
                .map(|(index, level)| format!("  {:<5} {}", level, level_counts[index])),
        );

        lines.extend([
            String::new(),
            // Crash Reports
            "Crash Reports".to_string(),
            String::new(),
            format!("  Count: {}", report.crash_report_entries.len()),
        ]);

        lines
    }
}
