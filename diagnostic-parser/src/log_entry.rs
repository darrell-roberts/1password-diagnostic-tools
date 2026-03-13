//! Structured log entry parser for 1Password diagnostic log content.
//!
//! Each log file captured in a `.1pdiagnostics` report contains newline-separated
//! log lines. Most lines follow the format:
//!
//! ```text
//! LEVEL  TIMESTAMP THREAD [SOURCE] MESSAGE
//! ```
//!
//! For example:
//!
//! ```text
//! INFO  2026-03-05T19:36:06.278+00:00 ThreadId(6) [1P:op-settings/src/store/json_store.rs:75] Settings loaded
//! ERROR 2026-03-05T19:22:01.469+00:00 runtime-worker(ThreadId(3)) [1P:op-crash-reporting/src/lib.rs:181] thread panicked
//! ```
//!
//! Some lines are *continuation lines* (e.g. stack traces) that belong to
//! the preceding structured entry. These are captured in the `continuation`
//! field of the parent entry.
//!
//! # Owned vs. Borrowed Entries
//!
//! This module provides two representations:
//!
//! - [`LogEntry`] — Fully owned. Every field is a `String` / `Vec<String>`.
//!   Simple to use, easy to store, but allocates ~5 `String`s per log line.
//!   For 127 k entries that's ~638 k allocations and ~33 MB of heap.
//!
//! - [`LogEntryRef<'a>`] — Zero-copy. String fields borrow `&'a str` slices
//!   directly from the log content that is already in memory, and high-
//!   repetition fields (`log_file_title`, `thread`) are shared via
//!   [`Arc<str>`]. Parsing into `LogEntryRef` performs **zero heap
//!   allocations** for the common case (no continuation lines). Continuation
//!   lines are stored as `&'a str` slices as well.
//!
//! A `LogEntryRef` can be promoted to a `LogEntry` via [`LogEntryRef::to_owned`]
//! when you need to store it beyond the lifetime of the backing data.
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, FixedOffset};

// ---------------------------------------------------------------------------
// Log level
// ---------------------------------------------------------------------------

/// Severity level of a log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Parse a log level from the keyword that begins a log line.
    fn parse(s: &str) -> Option<Self> {
        match s {
            "TRACE" => Some(Self::Trace),
            "DEBUG" => Some(Self::Debug),
            "INFO" => Some(Self::Info),
            "WARN" => Some(Self::Warn),
            "ERROR" => Some(Self::Error),
            _ => None,
        }
    }

    /// Return the canonical uppercase keyword for this level.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Log source (owned)
// ---------------------------------------------------------------------------

/// The bracketed source location / component tag on a log line.
///
/// The raw text inside the brackets (e.g. `1P:op-settings/src/store/json_store.rs:75`)
/// is split into a `component` prefix and optional `detail` suffix.
///
/// Known component prefixes:
/// - `1P`     – core 1Password Rust code (detail is `crate/path:line`)
/// - `client` – TypeScript / Electron client layer (detail is a module name)
/// - `status` – application status logger (detail is `crate/path:line`)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LogSource {
    /// The component prefix (text before the first `:`), e.g. `"1P"`, `"client"`, `"status"`.
    pub component: String,

    /// Everything after the first `:` separator, if any.
    /// For Rust sources this is typically `"crate/path/to/file.rs:line"`.
    /// For the `client` component this is often just a module name like `"typescript"`.
    pub detail: Option<String>,
}

impl LogSource {
    /// Parse the content between `[` and `]`.
    fn parse(raw: &str) -> Self {
        match raw.split_once(':') {
            Some((component, rest)) => Self {
                component: component.to_owned(),
                detail: Some(rest.to_owned()),
            },
            None => Self {
                component: raw.to_owned(),
                detail: None,
            },
        }
    }

    /// The raw source string reconstructed from its parts.
    pub fn raw(&self) -> String {
        match &self.detail {
            Some(detail) => format!("{}:{}", self.component, detail),
            None => self.component.clone(),
        }
    }

    /// Try to extract a source file path from the detail (Rust-style sources).
    ///
    /// Returns `Some("crate/path/to/file.rs")` when the detail looks like
    /// `crate/path/to/file.rs:42`, otherwise `None`.
    pub fn file_path(&self) -> Option<&str> {
        let detail = self.detail.as_deref()?;
        extract_file_path(detail)
    }

    /// Try to extract the source line number from the detail.
    pub fn line_number(&self) -> Option<u32> {
        let detail = self.detail.as_deref()?;
        extract_line_number(detail)
    }
}

impl fmt::Display for LogSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.raw())
    }
}

// ---------------------------------------------------------------------------
// Log source (borrowed)
// ---------------------------------------------------------------------------

/// Zero-copy version of [`LogSource`]. Borrows slices from the original log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogSourceRef<'a> {
    /// The component prefix, e.g. `"1P"`, `"client"`, `"status"`.
    pub component: &'a str,

    /// Everything after the first `:` separator, if any.
    pub detail: Option<&'a str>,
}

impl<'a> LogSourceRef<'a> {
    /// Parse the content between `[` and `]`.
    fn parse(raw: &'a str) -> Self {
        match raw.split_once(':') {
            Some((component, rest)) => Self {
                component,
                detail: Some(rest),
            },
            None => Self {
                component: raw,
                detail: None,
            },
        }
    }

    /// Try to extract a source file path from the detail (Rust-style sources).
    pub fn file_path(&self) -> Option<&'a str> {
        extract_file_path(self.detail?)
    }

    /// Try to extract the source line number from the detail.
    pub fn line_number(&self) -> Option<u32> {
        extract_line_number(self.detail?)
    }

    /// Convert to an owned [`LogSource`].
    pub fn to_owned(&self) -> LogSource {
        LogSource {
            component: self.component.to_owned(),
            detail: self.detail.map(|d| d.to_owned()),
        }
    }
}

impl fmt::Display for LogSourceRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.detail {
            Some(detail) => write!(f, "[{}:{}]", self.component, detail),
            None => write!(f, "[{}]", self.component),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers for LogSource / LogSourceRef
// ---------------------------------------------------------------------------

/// Extract the file path portion from a detail string like `"crate/path/file.rs:42"`.
fn extract_file_path(detail: &str) -> Option<&str> {
    let colon_pos = detail.rfind(':')?;
    let after_colon = &detail[colon_pos + 1..];
    if !after_colon.is_empty() && after_colon.bytes().all(|b| b.is_ascii_digit()) {
        Some(&detail[..colon_pos])
    } else {
        None
    }
}

/// Extract the line number from a detail string like `"crate/path/file.rs:42"`.
fn extract_line_number(detail: &str) -> Option<u32> {
    let colon_pos = detail.rfind(':')?;
    detail[colon_pos + 1..].parse().ok()
}

// ---------------------------------------------------------------------------
// Log entry (owned)
// ---------------------------------------------------------------------------

/// A single structured log entry parsed from the text content of a log file.
///
/// All string fields are fully owned. This is convenient for storing entries
/// in collections that outlive the source data, but costs ~5 heap allocations
/// per entry. For a lower-allocation alternative see [`LogEntryRef`].
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// The title of the [`LogFile`](crate::model::LogFile) this entry came from.
    pub log_file_title: String,

    /// Severity level.
    pub level: LogLevel,

    /// Timestamp with timezone offset as written in the log line.
    pub timestamp: DateTime<FixedOffset>,

    /// Thread identifier string, e.g. `"ThreadId(6)"` or `"runtime-worker(ThreadId(3))"`.
    pub thread: String,

    /// Parsed source / component tag from the brackets.
    pub source: LogSource,

    /// The log message text (everything after the `]`).
    pub message: String,

    /// Any continuation lines that immediately followed this entry
    /// (e.g. stack trace frames). Each element is one raw line.
    pub continuation: Vec<String>,
}

impl LogEntry {
    /// Parse all log lines from a single log file's content, associating
    /// continuation lines with their parent entry.
    ///
    /// `log_file_title` is stored on every returned entry so callers can
    /// trace entries back to their originating file.
    pub fn parse_log_content(log_file_title: &str, content: &str) -> Vec<Self> {
        let mut entries: Vec<Self> = Vec::new();

        for line in content.lines() {
            if line.trim_start().is_empty() {
                continue;
            }

            match Self::parse_line(log_file_title, line) {
                Some(entry) => entries.push(entry),
                None => {
                    // This is a continuation line — attach it to the last entry.
                    if let Some(last) = entries.last_mut() {
                        last.continuation.push(line.to_owned());
                    }
                }
            }
        }

        entries
    }

    /// Attempt to parse a single log line into a [`LogEntry`].
    ///
    /// Returns `None` if the line does not start with a recognized log level
    /// keyword (i.e. it is a continuation line).
    fn parse_line(log_file_title: &str, line: &str) -> Option<Self> {
        let (level, timestamp, thread, source_raw, message) = parse_line_fields(line)?;
        let source = LogSource::parse(source_raw);

        Some(Self {
            log_file_title: log_file_title.to_owned(),
            level,
            timestamp,
            thread: thread.to_owned(),
            source,
            message: message.to_owned(),
            continuation: Vec::new(),
        })
    }

    /// Returns `true` if this entry has associated continuation lines
    /// (typically a stack trace).
    pub fn has_continuation(&self) -> bool {
        !self.continuation.is_empty()
    }

    /// Returns `true` if this log entry records a panic (i.e. its message
    /// contains `"panicked at"`). Panic entries typically originate from the
    /// `op-crash-reporting` crate and carry a stack trace in their
    /// [`continuation`](Self::continuation) lines.
    pub fn is_panic(&self) -> bool {
        self.level == LogLevel::Error && self.message.contains("panicked at")
    }

    /// The full message including any continuation lines, joined by newlines.
    pub fn full_message(&self) -> String {
        if self.continuation.is_empty() {
            self.message.clone()
        } else {
            let mut buf = self.message.clone();
            for line in &self.continuation {
                buf.push('\n');
                buf.push_str(line);
            }
            buf
        }
    }

    /// The timestamp converted to UTC.
    pub fn timestamp_utc(&self) -> DateTime<chrono::Utc> {
        self.timestamp.to_utc()
    }
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<5} {} {} {} {}",
            self.level, self.timestamp, self.thread, self.source, self.message
        )?;
        for cont in &self.continuation {
            write!(f, "\n{cont}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Log entry (zero-copy / borrowed)
// ---------------------------------------------------------------------------

/// A zero-copy structured log entry that borrows string data from the
/// original log content.
///
/// `'a` is the lifetime of the backing log content string. The
/// `log_file_title` and `thread` fields use [`Arc<str>`] for cheap
/// sharing — there are typically only a handful of distinct values
/// repeated across thousands of entries.
///
/// For the common case (no continuation lines) parsing a `LogEntryRef`
/// performs **zero heap allocations** beyond the `Arc` lookups in the
/// intern table (which are shared across all entries).
#[derive(Debug, Clone)]
pub struct LogEntryRef<'a> {
    /// The title of the log file this entry came from (shared via `Arc`).
    pub log_file_title: Arc<str>,

    /// Severity level.
    pub level: LogLevel,

    /// Timestamp with timezone offset as written in the log line.
    pub timestamp: DateTime<FixedOffset>,

    /// Thread identifier string (shared via `Arc`). There are typically
    /// very few distinct thread IDs across an entire diagnostic report.
    pub thread: Arc<str>,

    /// Parsed source / component tag from the brackets (zero-copy).
    pub source: LogSourceRef<'a>,

    /// The log message text — a slice into the original log content.
    pub message: &'a str,

    /// Any continuation lines that immediately followed this entry
    /// (e.g. stack trace frames). Each element is a slice into the
    /// original log content.
    pub continuation: Vec<&'a str>,
}

impl<'a> LogEntryRef<'a> {
    /// Parse all log lines from a single log file's content into zero-copy
    /// entries. Uses `interner` to deduplicate `log_file_title` and `thread`
    /// strings via [`Arc<str>`].
    ///
    /// If you don't have an interner, use [`StringInterner::new()`] to create
    /// one. Sharing a single interner across multiple log files maximises
    /// deduplication.
    pub fn parse_log_content(
        log_file_title: &str,
        content: &'a str,
        interner: &mut StringInterner,
    ) -> Vec<Self> {
        let title_arc = interner.intern(log_file_title);
        let mut entries: Vec<Self> = Vec::new();

        for line in content.lines() {
            if line.trim_start().is_empty() {
                continue;
            }

            match Self::parse_line(&title_arc, line, interner) {
                Some(entry) => entries.push(entry),
                None => {
                    if let Some(last) = entries.last_mut() {
                        last.continuation.push(line);
                    }
                }
            }
        }

        entries
    }

    /// Attempt to parse a single log line into a zero-copy [`LogEntryRef`].
    fn parse_line(
        log_file_title: &Arc<str>,
        line: &'a str,
        interner: &mut StringInterner,
    ) -> Option<Self> {
        let (level, timestamp, thread_str, source_raw, message) = parse_line_fields(line)?;
        let source = LogSourceRef::parse(source_raw);
        let thread = interner.intern(thread_str);

        Some(Self {
            log_file_title: Arc::clone(log_file_title),
            level,
            timestamp,
            thread,
            source,
            message,
            continuation: Vec::new(),
        })
    }

    /// Returns `true` if this entry has associated continuation lines.
    pub fn has_continuation(&self) -> bool {
        !self.continuation.is_empty()
    }

    /// Returns `true` if this log entry records a panic.
    pub fn is_panic(&self) -> bool {
        self.level == LogLevel::Error && self.message.contains("panicked at")
    }

    /// The full message including any continuation lines, joined by newlines.
    pub fn full_message(&self) -> String {
        if self.continuation.is_empty() {
            self.message.to_owned()
        } else {
            let total_len =
                self.message.len() + self.continuation.iter().map(|c| 1 + c.len()).sum::<usize>();
            let mut buf = String::with_capacity(total_len);
            buf.push_str(self.message);
            for line in &self.continuation {
                buf.push('\n');
                buf.push_str(line);
            }
            buf
        }
    }

    /// The timestamp converted to UTC.
    pub fn timestamp_utc(&self) -> DateTime<chrono::Utc> {
        self.timestamp.to_utc()
    }

    /// Convert to a fully owned [`LogEntry`], allocating new `String`s for
    /// every field.
    pub fn to_owned(&self) -> LogEntry {
        LogEntry {
            log_file_title: self.log_file_title.to_string(),
            level: self.level,
            timestamp: self.timestamp,
            thread: self.thread.to_string(),
            source: self.source.to_owned(),
            message: self.message.to_owned(),
            continuation: self.continuation.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl fmt::Display for LogEntryRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<5} {} {} {} {}",
            self.level, self.timestamp, self.thread, self.source, self.message
        )?;
        for cont in &self.continuation {
            write!(f, "\n{cont}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// String interner
// ---------------------------------------------------------------------------

/// A simple string interner backed by a [`HashMap`]. Converts `&str` values
/// into `Arc<str>`, returning the same `Arc` for duplicate strings.
///
/// This is used to deduplicate high-repetition fields like `log_file_title`
/// (only ~212 unique values across 127 k entries) and `thread` (typically
/// fewer than 10 unique values).
#[derive(Debug, Default, Clone)]
pub struct StringInterner {
    map: HashMap<Arc<str>, ()>,
}

impl StringInterner {
    /// Create a new, empty interner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a string, returning a shared [`Arc<str>`]. If the string has
    /// been interned before, the existing `Arc` is cloned (cheap reference
    /// count bump). Otherwise a new `Arc<str>` is allocated.
    pub fn intern(&mut self, s: &str) -> Arc<str> {
        // Look up by borrowed str to avoid allocating an Arc for the probe.
        if let Some((existing, _)) = self.map.get_key_value(s) {
            return Arc::clone(existing);
        }
        let arc: Arc<str> = Arc::from(s);
        self.map.insert(Arc::clone(&arc), ());
        arc
    }

    /// Number of unique strings currently interned.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the interner contains no strings.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Shared line-parsing logic
// ---------------------------------------------------------------------------

/// Core line parser shared by both [`LogEntry`] and [`LogEntryRef`].
///
/// Parses a single log line and returns the extracted fields as borrowed
/// slices. The caller decides whether to clone them into `String`s or keep
/// them as `&str`.
///
/// Returns `None` if the line is not a structured log line (e.g. a
/// continuation / stack-trace line).
fn parse_line_fields(line: &str) -> Option<(LogLevel, DateTime<FixedOffset>, &str, &str, &str)> {
    let rest = line.trim_start();

    // 1. Log level — first whitespace-delimited token.
    let (level_str, rest) = split_first_token(rest)?;
    let level = LogLevel::parse(level_str)?;

    // 2. Timestamp — next token.
    //    Desktop clients emit full RFC-3339 with timezone offset
    //    (e.g. `2026-03-05T19:36:06.278+00:00`).
    //    Browser extension / Safari logs omit the timezone offset
    //    (e.g. `2026-02-12T13:17:47.496`). In that case we treat the
    //    timestamp as UTC.
    let (ts_str, rest) = split_first_token(rest)?;
    let timestamp = parse_timestamp(ts_str)?;

    // 3. Thread — next token, which may contain parentheses like
    //    `runtime-worker(ThreadId(3))`. Some clients (e.g. the Safari
    //    extension) omit the thread entirely and jump straight to `[SOURCE]`.
    let rest_trimmed = rest.trim_start();
    let (thread, rest) = if rest_trimmed.starts_with('[') {
        // No thread token — the next thing is the source bracket.
        ("", rest_trimmed)
    } else {
        parse_thread_token(rest)?
    };

    // 4. Source — bracketed section `[...]`.
    let rest = rest.trim_start();
    let (source_raw, rest) = parse_bracketed(rest)?;

    // 5. Message — the remainder of the line.
    let message = rest.trim_start();

    Some((level, timestamp, thread, source_raw, message))
}

/// Parse a timestamp string that is either full RFC-3339 (with timezone) or
/// a "naive" ISO-8601 local datetime without timezone offset. In the latter
/// case the timestamp is assumed to be UTC.
fn parse_timestamp(s: &str) -> Option<DateTime<FixedOffset>> {
    // Try RFC-3339 first (includes timezone offset).
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt);
    }

    // Fallback: try NaiveDateTime formats (no timezone).
    // Accept both with and without fractional seconds.
    use chrono::NaiveDateTime;

    let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .ok()?;

    // Assume UTC when no offset is provided.
    let utc_offset = FixedOffset::east_opt(0)?;
    naive.and_local_timezone(utc_offset).single()
}

// ---------------------------------------------------------------------------
// Tokeniser helpers
// ---------------------------------------------------------------------------

/// Split the first whitespace-delimited token from the rest of the string.
fn split_first_token(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    let end = s.find(|c: char| c.is_whitespace()).unwrap_or(s.len());
    Some((&s[..end], &s[end..]))
}

/// Parse the thread identifier token. The thread token can be simple like
/// `ThreadId(6)` or compound like `runtime-worker(ThreadId(3))`, so we
/// need to handle nested parentheses.
///
/// Returns `(&str_slice_of_token, &rest_of_line)`.
fn parse_thread_token(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }

    let mut depth: u32 = 0;
    let mut end = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = i + 1;
                    break;
                }
            }
            c if c.is_whitespace() && depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
        end = i + ch.len_utf8();
    }

    if end == 0 {
        return None;
    }

    Some((&s[..end], &s[end..]))
}

/// Parse a `[...]` bracketed section, returning the inner text and the
/// rest of the string after the closing `]`.
fn parse_bracketed(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if !s.starts_with('[') {
        return None;
    }
    let close = s.find(']')?;
    Some((&s[1..close], &s[close + 1..]))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    const SAMPLE_INFO: &str = "INFO  2026-03-05T19:36:06.278+00:00 ThreadId(6) [1P:op-settings/src/store/json_store.rs:75] Settings file created";

    const SAMPLE_ERROR: &str = "ERROR 2026-03-05T19:22:01.469+00:00 runtime-worker(ThreadId(3)) [1P:op-crash-reporting/src/lib.rs:181] thread panicked";

    const SAMPLE_WARN: &str = r#"WARN  2026-03-05T18:30:29.541+00:00 ThreadId(6) [1P:op-settings/src/store/generic_entry.rs:418] Error parsing settings file "/home/<redacted-username>/.config/1Password/settings/settings.json", using defaults"#;

    const SAMPLE_CLIENT: &str = "INFO  2026-03-05T19:36:06.280+00:00 ThreadId(6) [client:typescript] 1Password is already running, closing.";

    const SAMPLE_STATUS: &str = "INFO  2026-03-05T19:40:59.760+00:00 ThreadId(6) [status:app/op-app/src/app.rs:1108] App::new(1Password for Linux)";

    // ── Owned LogEntry tests ─────────────────────────────────────────

    #[test]
    fn parse_info_line() {
        let entry = LogEntry::parse_line("test", SAMPLE_INFO).unwrap();
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.thread, "ThreadId(6)");
        assert_eq!(entry.source.component, "1P");
        assert_eq!(
            entry.source.detail.as_deref(),
            Some("op-settings/src/store/json_store.rs:75")
        );
        assert_eq!(
            entry.source.file_path(),
            Some("op-settings/src/store/json_store.rs")
        );
        assert_eq!(entry.source.line_number(), Some(75));
        assert_eq!(entry.message, "Settings file created");
        assert!(entry.continuation.is_empty());
    }

    #[test]
    fn parse_error_with_compound_thread() {
        let entry = LogEntry::parse_line("test", SAMPLE_ERROR).unwrap();
        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.thread, "runtime-worker(ThreadId(3))");
        assert_eq!(entry.source.component, "1P");
        assert_eq!(
            entry.source.detail.as_deref(),
            Some("op-crash-reporting/src/lib.rs:181")
        );
        assert_eq!(entry.message, "thread panicked");
    }

    #[test]
    fn parse_warn_line() {
        let entry = LogEntry::parse_line("test", SAMPLE_WARN).unwrap();
        assert_eq!(entry.level, LogLevel::Warn);
        assert!(entry.message.contains("Error parsing settings file"));
    }

    #[test]
    fn parse_client_source() {
        let entry = LogEntry::parse_line("test", SAMPLE_CLIENT).unwrap();
        assert_eq!(entry.source.component, "client");
        assert_eq!(entry.source.detail.as_deref(), Some("typescript"));
        assert_eq!(entry.source.file_path(), None);
        assert_eq!(entry.source.line_number(), None);
    }

    #[test]
    fn parse_status_source() {
        let entry = LogEntry::parse_line("test", SAMPLE_STATUS).unwrap();
        assert_eq!(entry.source.component, "status");
        assert_eq!(entry.source.file_path(), Some("app/op-app/src/app.rs"));
        assert_eq!(entry.source.line_number(), Some(1108));
    }

    #[test]
    fn continuation_lines_attached() {
        let content = "\
ERROR 2026-03-05T19:22:01.469+00:00 runtime-worker(ThreadId(3)) [1P:op-crash-reporting/src/lib.rs:181] thread panicked
   0: op_crash_reporting::enable_panic_hook::{{closure}}
   1: std::panicking::panic_with_hook
   2: std::panicking::panic_handler::{{closure}}
INFO  2026-03-05T19:22:02.000+00:00 ThreadId(6) [1P:some/module.rs:10] recovered";

        let entries = LogEntry::parse_log_content("test_file", content);
        assert_eq!(entries.len(), 2);

        let error_entry = &entries[0];
        assert_eq!(error_entry.level, LogLevel::Error);
        assert_eq!(error_entry.continuation.len(), 3);
        assert!(error_entry.continuation[0].contains("op_crash_reporting"));
        assert!(error_entry.continuation[1].contains("panic_with_hook"));
        assert!(error_entry.continuation[2].contains("panic_handler"));
        assert!(error_entry.has_continuation());

        let info_entry = &entries[1];
        assert_eq!(info_entry.level, LogLevel::Info);
        assert!(info_entry.continuation.is_empty());
        assert!(!info_entry.has_continuation());
    }

    #[test]
    fn full_message_includes_continuation() {
        let content = "\
ERROR 2026-03-05T19:22:01.469+00:00 ThreadId(1) [1P:lib.rs:1] panic
   0: frame_a
   1: frame_b";

        let entries = LogEntry::parse_log_content("f", content);
        assert_eq!(entries.len(), 1);
        let full = entries[0].full_message();
        assert!(full.starts_with("panic"));
        assert!(full.contains("frame_a"));
        assert!(full.contains("frame_b"));
    }

    #[test]
    fn empty_content_produces_no_entries() {
        let entries = LogEntry::parse_log_content("empty", "");
        assert!(entries.is_empty());

        let entries = LogEntry::parse_log_content("blank", "\n\n\n");
        assert!(entries.is_empty());
    }

    #[test]
    fn is_panic_detection() {
        let content = "\
ERROR 2026-03-05T19:22:01.469+00:00 runtime-worker(ThreadId(3)) [1P:op-crash-reporting/src/lib.rs:181] thread 'runtime-worker(ThreadId(3))' panicked at /root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/flexi_logger-0.28.5/src/util.rs:86
   0: op_crash_reporting::enable_panic_hook::{{closure}}::{{closure}}
   1: std::panicking::panic_with_hook
INFO  2026-03-05T19:22:02.000+00:00 ThreadId(6) [1P:some/module.rs:10] recovered";

        let entries = LogEntry::parse_log_content("test_file", content);
        assert_eq!(entries.len(), 2);
        assert!(entries[0].is_panic());
        assert!(!entries[1].is_panic());
    }

    #[test]
    fn non_log_line_returns_none() {
        assert!(LogEntry::parse_line("test", "   0: some_frame").is_none());
        assert!(LogEntry::parse_line("test", "just some random text").is_none());
        assert!(LogEntry::parse_line("test", "").is_none());
    }

    #[test]
    fn log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "TRACE");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Warn.to_string(), "WARN");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
    }

    #[test]
    fn log_source_display() {
        let src = LogSource::parse("1P:op-settings/src/store/json_store.rs:75");
        assert_eq!(
            src.to_string(),
            "[1P:op-settings/src/store/json_store.rs:75]"
        );
    }

    #[test]
    fn log_entry_display() {
        let entry = LogEntry::parse_line("test", SAMPLE_INFO).unwrap();
        let display = entry.to_string();
        assert!(display.contains("INFO"));
        assert!(display.contains("Settings file created"));
    }

    #[test]
    fn timestamp_utc_conversion() {
        let entry = LogEntry::parse_line("test", SAMPLE_INFO).unwrap();
        let utc = entry.timestamp_utc();
        assert_eq!(utc.date_naive().year(), 2026);
    }

    #[test]
    fn log_file_title_preserved() {
        let entries = LogEntry::parse_log_content("/1Password_r00217", SAMPLE_INFO);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].log_file_title, "/1Password_r00217");
    }

    // ── Zero-copy LogEntryRef tests ──────────────────────────────────

    #[test]
    fn ref_parse_info_line() {
        let mut interner = StringInterner::new();
        let entries = LogEntryRef::parse_log_content("test", SAMPLE_INFO, &mut interner);
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(&*entry.thread, "ThreadId(6)");
        assert_eq!(entry.source.component, "1P");
        assert_eq!(
            entry.source.detail,
            Some("op-settings/src/store/json_store.rs:75")
        );
        assert_eq!(
            entry.source.file_path(),
            Some("op-settings/src/store/json_store.rs")
        );
        assert_eq!(entry.source.line_number(), Some(75));
        assert_eq!(entry.message, "Settings file created");
        assert!(entry.continuation.is_empty());
    }

    #[test]
    fn ref_parse_error_compound_thread() {
        let mut interner = StringInterner::new();
        let entries = LogEntryRef::parse_log_content("test", SAMPLE_ERROR, &mut interner);
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(&*entry.thread, "runtime-worker(ThreadId(3))");
        assert_eq!(entry.source.component, "1P");
        assert_eq!(entry.message, "thread panicked");
    }

    #[test]
    fn ref_continuation_lines() {
        let content = "\
ERROR 2026-03-05T19:22:01.469+00:00 runtime-worker(ThreadId(3)) [1P:op-crash-reporting/src/lib.rs:181] thread 'runtime-worker(ThreadId(3))' panicked at /root/.cargo/registry/src/flexi_logger-0.28.5/src/util.rs:86
   0: op_crash_reporting::enable_panic_hook::{{closure}}
   1: std::panicking::panic_with_hook
INFO  2026-03-05T19:22:02.000+00:00 ThreadId(6) [1P:some/module.rs:10] recovered";

        let mut interner = StringInterner::new();
        let entries = LogEntryRef::parse_log_content("test_file", content, &mut interner);
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].continuation.len(), 2);
        assert!(entries[0].continuation[0].contains("op_crash_reporting"));
        assert!(entries[0].continuation[1].contains("panic_with_hook"));
        assert!(entries[0].has_continuation());
        assert!(entries[0].is_panic());

        assert!(!entries[1].has_continuation());
        assert!(!entries[1].is_panic());
    }

    #[test]
    fn ref_full_message() {
        let content = "\
ERROR 2026-03-05T19:22:01.469+00:00 ThreadId(1) [1P:lib.rs:1] panic
   0: frame_a
   1: frame_b";

        let mut interner = StringInterner::new();
        let entries = LogEntryRef::parse_log_content("f", content, &mut interner);
        assert_eq!(entries.len(), 1);
        let full = entries[0].full_message();
        assert!(full.starts_with("panic"));
        assert!(full.contains("frame_a"));
        assert!(full.contains("frame_b"));
    }

    #[test]
    fn ref_to_owned_roundtrip() {
        let mut interner = StringInterner::new();
        let refs = LogEntryRef::parse_log_content("test", SAMPLE_INFO, &mut interner);
        let owned = refs[0].to_owned();

        assert_eq!(owned.log_file_title, "test");
        assert_eq!(owned.level, refs[0].level);
        assert_eq!(owned.timestamp, refs[0].timestamp);
        assert_eq!(owned.thread, &*refs[0].thread);
        assert_eq!(owned.source.component, refs[0].source.component);
        assert_eq!(owned.source.detail.as_deref(), refs[0].source.detail);
        assert_eq!(owned.message, refs[0].message);
    }

    #[test]
    fn ref_display() {
        let mut interner = StringInterner::new();
        let entries = LogEntryRef::parse_log_content("test", SAMPLE_INFO, &mut interner);
        let display = entries[0].to_string();
        assert!(display.contains("INFO"));
        assert!(display.contains("Settings file created"));
    }

    #[test]
    fn ref_timestamp_utc() {
        let mut interner = StringInterner::new();
        let entries = LogEntryRef::parse_log_content("test", SAMPLE_INFO, &mut interner);
        let utc = entries[0].timestamp_utc();
        assert_eq!(utc.date_naive().year(), 2026);
    }

    #[test]
    fn ref_empty_content() {
        let mut interner = StringInterner::new();
        assert!(LogEntryRef::parse_log_content("e", "", &mut interner).is_empty());
        assert!(LogEntryRef::parse_log_content("b", "\n\n\n", &mut interner).is_empty());
    }

    // ── String interner tests ────────────────────────────────────────

    #[test]
    fn interner_deduplicates() {
        let mut interner = StringInterner::new();
        let a1 = interner.intern("ThreadId(6)");
        let a2 = interner.intern("ThreadId(6)");
        let b = interner.intern("ThreadId(7)");

        // Same pointer for identical strings.
        assert!(Arc::ptr_eq(&a1, &a2));
        // Different pointer for different strings.
        assert!(!Arc::ptr_eq(&a1, &b));
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn interner_shared_across_files() {
        let mut interner = StringInterner::new();

        let content1 = "INFO  2026-03-05T19:36:06.278+00:00 ThreadId(6) [1P:a.rs:1] msg1";
        let content2 = "INFO  2026-03-05T19:36:07.000+00:00 ThreadId(6) [1P:b.rs:2] msg2";

        let entries1 = LogEntryRef::parse_log_content("/file1", content1, &mut interner);
        let entries2 = LogEntryRef::parse_log_content("/file2", content2, &mut interner);

        // Thread Arc is shared across files.
        assert!(Arc::ptr_eq(&entries1[0].thread, &entries2[0].thread));
        // Log file titles are different.
        assert!(!Arc::ptr_eq(
            &entries1[0].log_file_title,
            &entries2[0].log_file_title
        ));
    }

    // ── LogSourceRef tests ───────────────────────────────────────────

    #[test]
    fn source_ref_parse_with_detail() {
        let src = LogSourceRef::parse("1P:op-settings/src/store/json_store.rs:75");
        assert_eq!(src.component, "1P");
        assert_eq!(src.detail, Some("op-settings/src/store/json_store.rs:75"));
        assert_eq!(src.file_path(), Some("op-settings/src/store/json_store.rs"));
        assert_eq!(src.line_number(), Some(75));
    }

    #[test]
    fn source_ref_parse_no_detail() {
        let src = LogSourceRef::parse("standalone");
        assert_eq!(src.component, "standalone");
        assert_eq!(src.detail, None);
        assert_eq!(src.file_path(), None);
        assert_eq!(src.line_number(), None);
    }

    #[test]
    fn source_ref_client() {
        let src = LogSourceRef::parse("client:typescript");
        assert_eq!(src.component, "client");
        assert_eq!(src.detail, Some("typescript"));
        assert_eq!(src.file_path(), None);
        assert_eq!(src.line_number(), None);
    }

    #[test]
    fn source_ref_display() {
        let src = LogSourceRef::parse("1P:op-settings/src/store/json_store.rs:75");
        assert_eq!(
            src.to_string(),
            "[1P:op-settings/src/store/json_store.rs:75]"
        );

        let src2 = LogSourceRef::parse("standalone");
        assert_eq!(src2.to_string(), "[standalone]");
    }

    #[test]
    fn source_ref_to_owned() {
        let src_ref = LogSourceRef::parse("status:app/op-app/src/app.rs:1108");
        let src_owned = src_ref.to_owned();
        assert_eq!(src_owned.component, "status");
        assert_eq!(
            src_owned.detail.as_deref(),
            Some("app/op-app/src/app.rs:1108")
        );
        assert_eq!(src_owned.file_path(), Some("app/op-app/src/app.rs"));
        assert_eq!(src_owned.line_number(), Some(1108));
    }

    // ── Safari / browser extension log format tests ──────────────────

    const SAMPLE_SAFARI_INFO: &str =
        "INFO  2026-02-12T13:17:47.496 [TrelicaReporting] Purging stale Trelica activity";
    const SAMPLE_SAFARI_ERROR: &str = "ERROR  2026-02-13T05:00:07.447 [AccountHandlers] Account handler doesn't have a client so it can't sign device trust public key";

    #[test]
    fn parse_safari_info_line() {
        let entry = LogEntry::parse_line("test", SAMPLE_SAFARI_INFO).unwrap();
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.thread, ""); // Safari logs have no thread
        assert_eq!(entry.source.component, "TrelicaReporting");
        assert_eq!(entry.source.detail, None);
        assert_eq!(entry.message, "Purging stale Trelica activity");
    }

    #[test]
    fn parse_safari_error_line() {
        let entry = LogEntry::parse_line("test", SAMPLE_SAFARI_ERROR).unwrap();
        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.thread, "");
        assert_eq!(entry.source.component, "AccountHandlers");
        assert!(entry.message.contains("Account handler"));
    }

    #[test]
    fn parse_safari_log_content() {
        let content = "\
INFO  2026-02-12T13:17:47.496 [TrelicaReporting] Purging stale Trelica activity
ERROR  2026-02-13T05:00:07.447 [AccountHandlers] Account handler problem
INFO  2026-02-19T12:44:38.703 [UnleashFeatureFlags] Initialized feature flags manager.";

        let entries = LogEntry::parse_log_content("safari_test", content);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].level, LogLevel::Info);
        assert_eq!(entries[1].level, LogLevel::Error);
        assert_eq!(entries[2].source.component, "UnleashFeatureFlags");
    }

    #[test]
    fn parse_safari_timestamp_assumed_utc() {
        let entry = LogEntry::parse_line("test", SAMPLE_SAFARI_INFO).unwrap();
        // Timestamp should be parsed as UTC (offset 0)
        assert_eq!(entry.timestamp.offset().local_minus_utc(), 0);
        assert_eq!(entry.timestamp.date_naive().year(), 2026);
    }

    #[test]
    fn parse_safari_ref_info_line() {
        let mut interner = StringInterner::new();
        let entry =
            LogEntryRef::parse_line(&interner.intern("test"), SAMPLE_SAFARI_INFO, &mut interner)
                .unwrap();
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(&*entry.thread, "");
        assert_eq!(entry.source.component, "TrelicaReporting");
        assert_eq!(entry.message, "Purging stale Trelica activity");
    }
}
