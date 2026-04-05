//! State and computed data for the Analysis tab.
//!
//! [`AnalysisData`] holds pre-computed analytics derived from the parsed
//! log entries and crash reports. [`AnalysisState`] reuses
//! [`ScrollablePaneState`] for scrollable-pane navigation.

use super::pane_state::ScrollablePaneState;
use chrono::{DateTime, FixedOffset, TimeDelta};
use diagnostic_parser::{LogEntryRef, log_entry::LogLevel, model::CrashReportEntry};
use std::collections::HashMap;

/// Persistent state for the Analysis tab.
pub type AnalysisState = ScrollablePaneState;

// ---------------------------------------------------------------------------
// Computed analysis data
// ---------------------------------------------------------------------------

/// Pre-computed analytics over log entries and crash reports.
pub struct AnalysisData {
    /// Entry count per level, indexed as: [Error, Warn, Info, Debug, Trace].
    pub level_counts: [usize; 5],
    /// Total parsed entries.
    pub total_entries: usize,
    /// Top error messages, deduplicated and sorted by count descending.
    pub top_errors: Vec<ErrorGroup>,
    /// Per-component health statistics, sorted by error count descending.
    pub component_health: Vec<ComponentStats>,
    /// Timeline buckets for error/warn frequency.
    pub timeline_buckets: Vec<TimeBucket>,
    /// Human-readable label for the bucket width (e.g. "5 min", "1 hour").
    pub bucket_label: String,
    /// Detected bursts (spikes in error/warn rate).
    pub bursts: Vec<BurstInfo>,
    /// Detected gaps (periods with no log entries).
    pub gaps: Vec<GapInfo>,
    /// Summary of all panic log entries.
    pub panics: Vec<PanicSummary>,
    /// Crash-to-panic correlations.
    pub crash_correlations: Vec<CrashCorrelation>,
}

/// A group of deduplicated error messages.
pub struct ErrorGroup {
    /// The normalized/representative message (truncated).
    pub message: String,
    /// How many entries matched this group.
    pub count: usize,
    /// Components that produced this error.
    pub components: Vec<String>,
}

/// Per-component statistics.
pub struct ComponentStats {
    pub component: String,
    pub error_count: usize,
    pub warn_count: usize,
    pub total_count: usize,
}

/// A time bucket for the timeline sparkline.
pub struct TimeBucket {
    pub start: DateTime<FixedOffset>,
    pub error_count: u64,
    pub warn_count: u64,
}

/// A detected burst in the timeline.
pub struct BurstInfo {
    pub start: DateTime<FixedOffset>,
    /// Combined error + warn count in this bucket.
    pub count: u64,
}

/// A detected gap with no log entries.
pub struct GapInfo {
    pub start: DateTime<FixedOffset>,
    pub end: DateTime<FixedOffset>,
    pub duration: TimeDelta,
}

/// Summary of a single panic entry.
pub struct PanicSummary {
    pub timestamp: DateTime<FixedOffset>,
    pub message: String,
    pub log_file: String,
    pub has_stack_trace: bool,
    pub thread: String,
}

/// Crash report correlated to a panic log entry.
pub struct CrashCorrelation {
    pub report_id: String,
    pub report_type: String,
    pub crash_timestamp: Option<String>,
    pub matched_panic_message: Option<String>,
}

// ---------------------------------------------------------------------------
// Computation
// ---------------------------------------------------------------------------

impl AnalysisData {
    /// Compute all analytics from parsed log entries and crash reports.
    pub fn compute(entries: &[LogEntryRef<'_>], crashes: &[CrashReportEntry]) -> Self {
        let total_entries = entries.len();

        // -- Level counts & component stats (single pass) --
        let mut level_counts = [0usize; 5];
        let mut component_map: HashMap<&str, (usize, usize, usize)> = HashMap::new();
        // (count, components, first full message)
        let mut error_map: HashMap<String, (usize, Vec<String>, String)> = HashMap::new();

        for entry in entries {
            let idx = match entry.level {
                LogLevel::Error => 0,
                LogLevel::Warn => 1,
                LogLevel::Info => 2,
                LogLevel::Debug => 3,
                LogLevel::Trace => 4,
            };
            level_counts[idx] += 1;

            let comp = entry.source.component;
            let stats = component_map.entry(comp).or_insert((0, 0, 0));
            stats.2 += 1; // total
            match entry.level {
                LogLevel::Error => stats.0 += 1,
                LogLevel::Warn => stats.1 += 1,
                _ => {}
            }

            // Collect errors for deduplication.
            if entry.level == LogLevel::Error {
                let key = normalize_error_message(entry.message);
                let group = error_map
                    .entry(key)
                    .or_insert_with(|| (0, Vec::new(), entry.message.to_string()));
                group.0 += 1;
                let comp_str = comp.to_string();
                if !group.1.contains(&comp_str) {
                    group.1.push(comp_str);
                }
            }
        }

        // -- Top errors --
        let mut top_errors: Vec<ErrorGroup> = error_map
            .into_iter()
            .map(|(_, (count, components, message))| ErrorGroup {
                message,
                count,
                components,
            })
            .collect();
        top_errors.sort_by(|a, b| b.count.cmp(&a.count));
        top_errors.truncate(20);

        // -- Component health --
        let mut component_health = component_map
            .into_iter()
            .map(|(component, (e, w, t))| ComponentStats {
                component: component.to_string(),
                error_count: e,
                warn_count: w,
                total_count: t,
            })
            .collect::<Vec<_>>();
        component_health.sort_by(|a, b| {
            b.error_count
                .cmp(&a.error_count)
                .then(b.warn_count.cmp(&a.warn_count))
        });

        // -- Timeline --
        let (timeline_buckets, bucket_label, bursts, gaps) = compute_timeline(entries);

        // -- Panics --
        let panics = entries
            .iter()
            .filter(|e| e.is_panic())
            .map(|e| PanicSummary {
                timestamp: e.timestamp,
                message: truncate_str(e.message, 100).to_string(),
                log_file: e.log_file_title.to_string(),
                has_stack_trace: e.has_continuation(),
                thread: e.thread.to_string(),
            })
            .collect::<Vec<_>>();

        // -- Crash correlations --
        let crash_correlations = crashes
            .iter()
            .map(|crash| {
                let panic_entry = crash.find_panic_entry(entries);
                CrashCorrelation {
                    report_id: crash.report_id.clone(),
                    report_type: crash.report_type.clone(),
                    crash_timestamp: crash
                        .timestamp_utc()
                        .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
                    matched_panic_message: panic_entry
                        .map(|e| truncate_str(e.message, 80).to_string()),
                }
            })
            .collect::<Vec<_>>();

        Self {
            level_counts,
            total_entries,
            top_errors,
            component_health,
            timeline_buckets,
            bucket_label,
            bursts,
            gaps,
            panics,
            crash_correlations,
        }
    }
}

// ---------------------------------------------------------------------------
// Timeline computation
// ---------------------------------------------------------------------------

fn compute_timeline(
    entries: &[LogEntryRef<'_>],
) -> (Vec<TimeBucket>, String, Vec<BurstInfo>, Vec<GapInfo>) {
    if entries.is_empty() {
        return (Vec::new(), String::new(), Vec::new(), Vec::new());
    }

    let Some((first_ts, last_ts)) = entries
        .first()
        .map(|first| first.timestamp)
        .zip(entries.last().map(|last| last.timestamp))
    else {
        return (Vec::new(), String::new(), Vec::new(), Vec::new());
    };

    let span = last_ts - first_ts;

    // Pick bucket width.
    let (bucket_secs, label) = if span < TimeDelta::hours(2) {
        (300i64, "5 min")
    } else if span < TimeDelta::hours(24) {
        (3600, "1 hour")
    } else {
        (21600, "6 hours")
    };

    let bucket_delta = TimeDelta::seconds(bucket_secs);
    let num_buckets = ((span.num_seconds() / bucket_secs) + 1).max(1) as usize;

    let mut buckets = (0..num_buckets)
        .map(|i| TimeBucket {
            start: first_ts + TimeDelta::seconds(bucket_secs * i as i64),
            error_count: 0,
            warn_count: 0,
        })
        .collect::<Vec<_>>();

    // Bucket entries.
    for entry in entries {
        let offset = (entry.timestamp - first_ts).num_seconds();
        let idx = (offset / bucket_secs) as usize;
        let idx = idx.min(buckets.len() - 1);
        match entry.level {
            LogLevel::Error => buckets[idx].error_count += 1,
            LogLevel::Warn => buckets[idx].warn_count += 1,
            _ => {}
        }
    }

    // Detect bursts: mean + 2*stddev.
    let counts = buckets
        .iter()
        .map(|b| b.error_count + b.warn_count)
        .collect::<Vec<_>>();
    let bursts = detect_bursts(&buckets, &counts);

    // Detect gaps: consecutive buckets with zero entries of any level.
    // We track zero-activity by checking if error+warn is 0 in consecutive buckets.
    let gaps = detect_gaps(&buckets, bucket_delta, entries, first_ts, bucket_secs);

    (buckets, label.to_string(), bursts, gaps)
}

fn detect_bursts(buckets: &[TimeBucket], counts: &[u64]) -> Vec<BurstInfo> {
    if counts.is_empty() {
        return Vec::new();
    }

    let sum: f64 = counts.iter().map(|&c| c as f64).sum();
    let n = counts.len() as f64;
    let mean = sum / n;
    let variance: f64 = counts
        .iter()
        .map(|&c| (c as f64 - mean).powi(2))
        .sum::<f64>()
        / n;
    let stddev = variance.sqrt();
    let threshold = mean + 2.0 * stddev;

    // Only report bursts if threshold is meaningful (> 2).
    if threshold < 2.0 {
        return Vec::new();
    }

    buckets
        .iter()
        .zip(counts.iter().copied())
        .filter(|(_, count)| *count as f64 > threshold)
        .map(|(bucket, count)| BurstInfo {
            start: bucket.start,
            count,
        })
        .collect()
}

fn detect_gaps(
    buckets: &[TimeBucket],
    bucket_delta: TimeDelta,
    entries: &[LogEntryRef<'_>],
    first_ts: DateTime<FixedOffset>,
    bucket_secs: i64,
) -> Vec<GapInfo> {
    if buckets.len() < 2 {
        return Vec::new();
    }

    // Count total entries per bucket (not just errors/warns).
    let mut total_per_bucket = vec![0u64; buckets.len()];
    for entry in entries {
        let offset = (entry.timestamp - first_ts).num_seconds();
        let idx = (offset / bucket_secs) as usize;
        let idx = idx.min(total_per_bucket.len() - 1);
        total_per_bucket[idx] += 1;
    }

    let min_gap = bucket_delta * 2;

    // Identify runs of consecutive zero-count buckets via fold, collecting
    // completed gaps and tracking the current run start.
    let (mut gaps, trailing) = total_per_bucket.iter().enumerate().fold(
        (Vec::new(), None::<usize>),
        |(mut gaps, gap_start), (i, &count)| {
            if count == 0 {
                (gaps, gap_start.or(Some(i)))
            } else {
                if let Some(start_idx) = gap_start {
                    let gap_duration = buckets[i].start - buckets[start_idx].start;
                    if gap_duration >= min_gap {
                        gaps.push(GapInfo {
                            start: buckets[start_idx].start,
                            end: buckets[i].start,
                            duration: gap_duration,
                        });
                    }
                }
                (gaps, None)
            }
        },
    );

    // Handle trailing gap.
    if let Some(start_idx) = trailing
        && let Some(last) = buckets.last()
    {
        let end = last.start + bucket_delta;
        let gap_duration = end - buckets[start_idx].start;
        if gap_duration >= min_gap {
            gaps.push(GapInfo {
                start: buckets[start_idx].start,
                end,
                duration: gap_duration,
            });
        }
    }

    gaps
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Normalize an error message for deduplication grouping.
/// Takes first 80 chars and replaces long hex sequences and numeric runs.
fn normalize_error_message(msg: &str) -> String {
    let truncated = truncate_str(msg, 80);
    let mut result = String::with_capacity(truncated.len());
    let mut run_buf = String::new();
    let mut hex_run = 0usize;
    let mut digit_run = 0usize;

    for ch in truncated.chars() {
        if ch.is_ascii_hexdigit() {
            hex_run += 1;
            if ch.is_ascii_digit() {
                digit_run += 1;
            }
            run_buf.push(ch);
        } else {
            if hex_run >= 8 {
                result.push_str("<id>");
            } else if digit_run >= 5 {
                result.push_str("<N>");
            } else {
                result.push_str(&run_buf);
            }
            run_buf.clear();
            hex_run = 0;
            digit_run = 0;
            result.push(ch);
        }
    }
    // Flush trailing run.
    if hex_run >= 8 {
        result.push_str("<id>");
    } else if digit_run >= 5 {
        result.push_str("<N>");
    } else {
        result.push_str(&run_buf);
    }

    result
}

fn truncate_str(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

// ---------------------------------------------------------------------------
// Plain-text builder (for clipboard)
// ---------------------------------------------------------------------------

impl AnalysisData {
    /// Build plain-text lines that mirror the rendered analysis view
    /// line-for-line. Used for clipboard copy operations.
    ///
    /// **Important**: every section here must produce the exact same number
    /// of lines as the corresponding section in `ui/analysis.rs`, including
    /// blank lines, so that cursor-based selection copies the right content.
    pub fn build_plain_text_lines(&self) -> Vec<String> {
        // -- Section 1: Log Level Summary --
        // UI: header, blank, 5 levels, total, blank = 9 lines
        let mut lines = Vec::from(["Log Level Summary".to_string(), String::new()]);

        lines.extend(
            ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"]
                .into_iter()
                .enumerate()
                .map(|(i, label)| format!("  {:<5}  {}", label, self.level_counts[i])),
        );

        lines.extend([format!("  Total  {}", self.total_entries), String::new()]);

        // -- Section 2: Top Errors --
        if !self.top_errors.is_empty() {
            lines.extend([
                format!("Top Errors ({})", self.top_errors.len()),
                String::new(),
            ]);

            lines.extend(self.top_errors.iter().enumerate().flat_map(|(i, group)| {
                let mut msg_lines = group.message.lines();
                msg_lines
                    .next()
                    .map(|first| format!("  {:>2}. {:>4}x {}", i + 1, group.count, first))
                    .into_iter()
                    .chain(msg_lines.map(|cont| format!("            {cont}")))
                    .chain(
                        (!group.components.is_empty())
                            .then(|| format!("       [{}]", group.components.join(", "))),
                    )
            }));

            lines.push(String::new());
        }

        // -- Section 3: Component Health --
        if !self.component_health.is_empty() {
            lines.extend([
                "Component Health".to_string(),
                String::new(),
                format!(
                    "  {:<20}  {:>5}  {:>5}  {:>8}",
                    "Component", "ERR", "WARN", "Total"
                ),
            ]);

            lines.extend(self.component_health.iter().map(|comp| {
                format!(
                    "  {:<20}  {:>5}  {:>5}  {:>8}",
                    comp.component, comp.error_count, comp.warn_count, comp.total_count
                )
            }));

            lines.push(String::new());
        }

        // -- Section 4: Timeline --
        // UI: header, blank, time-range label, blank = 4 Lines
        //     then Sparkline segment (height=3)
        //     then blank, bursts/gaps lines, trailing blank
        if !self.timeline_buckets.is_empty() {
            lines.extend([
                format!("Timeline — errors + warns per {}", self.bucket_label),
                String::new(),
            ]);

            // Time range label (matches UI line).
            if let Some((first, last)) = self
                .timeline_buckets
                .first()
                .zip(self.timeline_buckets.last())
            {
                lines.extend([
                    format!(
                        "  {} — {} ({} buckets)",
                        first.start.format("%H:%M"),
                        last.start.format("%H:%M"),
                        self.timeline_buckets.len()
                    ),
                    String::new(),
                ]);
            }

            // Sparkline occupies 3 rendered rows — emit 3 placeholder lines.
            lines.extend([
                "  [sparkline row 1]".to_string(),
                "  [sparkline row 2]".to_string(),
                "  [sparkline row 3]".to_string(),
                String::new(),
            ]);

            // Bursts.
            if !self.bursts.is_empty() {
                lines.push("  Bursts detected:".to_string());
                lines.extend(self.bursts.iter().map(|burst| {
                    format!(
                        "    {} — {} errors+warns in {}",
                        burst.start.format("%H:%M"),
                        burst.count,
                        self.bucket_label
                    )
                }));
                lines.push(String::new());
            }

            // Gaps.
            if !self.gaps.is_empty() {
                lines.push("  Gaps detected:".to_string());
                lines.extend(self.gaps.iter().map(|gap| {
                    let duration = if gap.duration.num_hours() > 0 {
                        format!(
                            "{}h {}m",
                            gap.duration.num_hours(),
                            gap.duration.num_minutes() % 60
                        )
                    } else {
                        format!("{}m", gap.duration.num_minutes())
                    };
                    format!(
                        "    {} — {} (no logs for {})",
                        gap.start.format("%H:%M"),
                        gap.end.format("%H:%M"),
                        duration
                    )
                }));
                lines.push(String::new());
            }
        }

        // -- Section 5: Panics --
        // UI: 3 lines per panic (timestamp+thread, message, log+stacktrace)
        if !self.panics.is_empty() {
            lines.extend([format!("Panics ({})", self.panics.len()), String::new()]);
            lines.extend(self.panics.iter().enumerate().flat_map(|(i, panic)| {
                [
                    format!(
                        "  {:>2}. {}  [{}]",
                        i + 1,
                        panic.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        panic.thread,
                    ),
                    format!("      {}", panic.message),
                    format!(
                        "      Log: {}{}",
                        panic.log_file,
                        if panic.has_stack_trace {
                            "  (has stack trace)"
                        } else {
                            ""
                        }
                    ),
                ]
            }));
            lines.push(String::new());
        }

        // -- Section 6: Crash Correlations --
        // UI: report_id line, crash_at line (conditional), matched line, blank
        if !self.crash_correlations.is_empty() {
            lines.extend([
                format!("Crash Reports ({})", self.crash_correlations.len()),
                String::new(),
            ]);
            lines.extend(self.crash_correlations.iter().flat_map(|corr| {
                [
                    Some(format!("  {}  ({})", corr.report_id, corr.report_type)),
                    corr.crash_timestamp
                        .as_ref()
                        .map(|ts| format!("    Crash at: {}", ts)),
                    corr.matched_panic_message
                        .as_ref()
                        .map(|msg| format!("    Matched: {}", msg))
                        .or_else(|| Some("    No matching panic entry found".to_string())),
                    Some(String::new()),
                ]
                .into_iter()
                .flatten()
            }));
        }

        lines
    }
}
