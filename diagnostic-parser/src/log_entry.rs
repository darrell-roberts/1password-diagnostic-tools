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
//! - [`LogEntry<'a>`] — Zero-copy. String fields borrow `&'a str` slices
//!   directly from the log content that is already in memory, and high-
//!   repetition fields (`log_file_title`, `thread`) are shared via
//!   [`Arc<str>`]. Parsing into `LogEntry` performs **zero heap
//!   allocations** for the common case (no continuation lines). Continuation
//!   lines are stored as `&'a str` slices as well.
//!
//! A `LogEntry` can be promoted to a `LogEntry` via [`LogEntry::to_owned`]
//! when you need to store it beyond the lifetime of the backing data.
use chrono::{DateTime, FixedOffset};
use std::{borrow::Cow, collections::HashSet, fmt, sync::Arc};

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
    /// The raw source string reconstructed from its parts.
    pub fn raw(&self) -> Cow<'_, str> {
        match &self.detail {
            Some(detail) => Cow::Owned(format!("{}:{}", self.component, detail)),
            None => Cow::Borrowed(&self.component),
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
        match &self.detail {
            Some(detail) => write!(f, "[{}:{}]", self.component, detail),
            None => write!(f, "[{}]", self.component),
        }
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

    /// The raw source string reconstructed from its parts.
    pub fn raw(&self) -> Cow<'_, str> {
        match &self.detail {
            Some(detail) => Cow::Owned(format!("{}:{}", self.component, detail)),
            None => Cow::Borrowed(self.component),
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

/// A zero-copy structured log entry that borrows string data from the
/// original log content.
///
/// `'a` is the lifetime of the backing log content string. The
/// `log_file_title` and `thread` fields use [`Arc<str>`] for cheap
/// sharing — there are typically only a handful of distinct values
/// repeated across thousands of entries.
///
/// For the common case (no continuation lines) parsing a `LogEntry`
/// performs **zero heap allocations** beyond the `Arc` lookups in the
/// cache set (which are shared across all entries).
#[derive(Debug, Clone)]
pub struct LogEntry<'a> {
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

impl<'a> LogEntry<'a> {
    /// Parse all log lines from a single log file's content into zero-copy
    /// entries. Uses [`Arc<str>`] cache to deduplicate `log_file_title` and `thread`
    /// strings via [`Arc<str>`].
    ///
    /// If you don't have a cache, use [`StringCache::new()`] to create
    /// one. Sharing a single cache across multiple log files maximizes
    /// deduplication.
    pub fn parse_log_content(
        log_file_title: &str,
        content: &'a str,
        cache: &mut StringCache,
    ) -> Vec<Self> {
        let title_arc = cache.cached(log_file_title);
        parse_log_lines(
            content,
            |line| Self::parse_line(&title_arc, line, cache),
            |entry, line| entry.continuation.push(line),
        )
    }

    /// Attempt to parse a single log line into a zero-copy [`LogEntry`].
    fn parse_line(
        log_file_title: &Arc<str>,
        line: &'a str,
        cache: &mut StringCache,
    ) -> Option<Self> {
        let line_parts = parse_line_fields(line)?;
        let source = LogSourceRef::parse(line_parts.source_raw);
        let thread = cache.cached(line_parts.thread);

        Some(Self {
            log_file_title: Arc::clone(log_file_title),
            level: line_parts.level,
            timestamp: line_parts.timestamp,
            thread,
            source,
            message: line_parts.message,
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
    #[cfg(test)]
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
}

impl fmt::Display for LogEntry<'_> {
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
// String Cache
// ---------------------------------------------------------------------------

/// A simple string cache backed by a [`HashMap`]. Converts `&str` values
/// into `Arc<str>`, returning the same `Arc` for duplicate strings.
///
/// This is used to deduplicate high-repetition fields like `log_file_title`
/// (only ~212 unique values across 127 k entries) and `thread` (typically
/// fewer than 10 unique values).
#[derive(Debug, Default, Clone)]
pub struct StringCache {
    cache: HashSet<Arc<str>>,
}

impl StringCache {
    /// Create a new, empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cache a string, returning a shared [`Arc<str>`]. If the string has
    /// been cached before, the existing `Arc` is cloned (cheap reference
    /// count bump). Otherwise a new `Arc<str>` is allocated.
    pub fn cached(&mut self, s: &str) -> Arc<str> {
        match self.cache.get(s) {
            Some(s) => Arc::clone(s),
            None => {
                let copy: Arc<str> = Arc::from(s);
                self.cache.insert(copy.clone());
                copy
            }
        }
    }

    /// Number of unique strings currently cached.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns `true` if the string cache contains no strings.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Shared line-parsing logic
// ---------------------------------------------------------------------------

/// Generic log content parser shared by owned and zero-copy paths.
///
/// Iterates lines, calling `try_parse` on each. If it returns `Some(entry)`,
/// the entry is collected. Otherwise the line is a continuation and is
/// attached to the last entry via `push_continuation`.
fn parse_log_lines<'a, T>(
    content: &'a str,
    mut try_parse: impl FnMut(&'a str) -> Option<T>,
    mut push_continuation: impl FnMut(&mut T, &'a str),
) -> Vec<T> {
    let mut entries = Vec::with_capacity(content.lines().count());
    for line in content.lines().filter(|line| !line.trim_start().is_empty()) {
        match try_parse(line) {
            Some(entry) => entries.push(entry),
            None => {
                if let Some(last) = entries.last_mut() {
                    push_continuation(last, line);
                }
            }
        }
    }
    entries
}

struct LogLineFields<'a> {
    level: LogLevel,
    timestamp: DateTime<FixedOffset>,
    thread: &'a str,
    source_raw: &'a str,
    message: &'a str,
}

/// Core line parser shared by both [`LogEntry`] and [`LogEntry`].
///
/// Parses a single log line and returns the extracted fields as borrowed
/// slices. The caller decides whether to clone them into `String`s or keep
/// them as `&str`.
///
/// Returns `None` if the line is not a structured log line (e.g. a
/// continuation / stack-trace line).
fn parse_line_fields(line: &str) -> Option<LogLineFields<'_>> {
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

    Some(LogLineFields {
        level,
        timestamp,
        thread,
        source_raw,
        message,
    })
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

    (end > 0).then(|| (&s[..end], &s[end..]))
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
mod tests;
