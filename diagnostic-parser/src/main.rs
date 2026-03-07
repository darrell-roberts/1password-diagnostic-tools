//! Demo binary that loads a `.1pdiagnostics` file and prints a summary.
//!
//! Usage:
//!
//! ```sh
//! cargo run -- path/to/file.1pdiagnostics
//! ```

use std::collections::HashMap;
use std::process;

use diagnostic_parser::DiagnosticReport;
use diagnostic_parser::log_entry::LogLevel;
use diagnostic_parser::model::LogFileCategory;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: diagnostic-parser <path-to-.1pdiagnostics>");
            process::exit(1);
        }
    };

    let report = match DiagnosticReport::from_file(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // ── Report overview ──────────────────────────────────────────────
    println!("{report}");

    // ── System details ───────────────────────────────────────────────
    println!("{}", report.system);

    // ── Accounts ─────────────────────────────────────────────────────
    for account in &report.accounts {
        println!("{account}");
        for vault in &account.vaults {
            println!(
                "    Vault {} ({}) — {} items ({} active, {} archived, {} deleted)",
                vault.uuid,
                vault.vault_type,
                vault.total_items(),
                vault.items.active,
                vault.items.archived,
                vault.items.deleted,
            );
        }
        println!();
    }

    // ── Log file breakdown ───────────────────────────────────────────
    let mut by_category: HashMap<LogFileCategory, usize> = HashMap::new();
    for log_file in &report.logs {
        *by_category.entry(log_file.category()).or_default() += 1;
    }
    println!("Log Files by Category");
    for (cat, count) in &by_category {
        println!("  {cat}: {count}");
    }
    println!("  Total log lines: {}", report.total_log_lines());
    println!();

    // ── Parsed log entries (zero-copy) ───────────────────────────────
    let (entries, interner) = report.parse_log_entries_ref();
    let mut by_level: HashMap<LogLevel, usize> = HashMap::new();
    let mut with_stack_trace = 0usize;
    for entry in &entries {
        *by_level.entry(entry.level).or_default() += 1;
        if entry.has_continuation() {
            with_stack_trace += 1;
        }
    }

    println!("Parsed Log Entries: {}", entries.len());
    for level in [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ] {
        if let Some(&count) = by_level.get(&level) {
            println!("  {level:<5}: {count}");
        }
    }
    println!("  With stack traces: {with_stack_trace}");
    println!("  Interned strings: {}", interner.len());
    println!();

    // ── Crash reports with stack traces ──────────────────────────────
    if !report.crash_report_entries.is_empty() {
        println!("Crash Reports");
        for (i, cr) in report.crash_report_entries.iter().enumerate() {
            println!("  ── Crash {} ──", i + 1);
            println!("  {cr}");

            match cr.find_panic_entry_ref(&entries) {
                Some(entry) => {
                    println!("  Log file:  {}", entry.log_file_title);
                    println!("  Thread:    {}", entry.thread);
                    println!("  Source:    {}", entry.source);
                    println!("  Message:   {}", entry.message);
                    if entry.has_continuation() {
                        println!("  Stack trace ({} frames):", entry.continuation.len());
                        for line in &entry.continuation {
                            println!("    {line}");
                        }
                    }
                }
                None => {
                    println!("  (no matching panic log entry found)");
                }
            }
            println!();
        }
    }

    // ── Sample: most recent errors ───────────────────────────────────
    let mut errors: Vec<_> = entries
        .iter()
        .filter(|e| e.level == LogLevel::Error)
        .collect();
    errors.sort_by_key(|e| e.timestamp);

    let recent_count = 5.min(errors.len());
    if recent_count > 0 {
        println!("Most Recent Errors (last {recent_count})");
        for entry in errors.iter().rev().take(recent_count) {
            println!(
                "  {} [{}] {}",
                entry.timestamp,
                entry.source,
                truncate(entry.message, 120),
            );
            if entry.has_continuation() {
                println!("    + {} continuation line(s)", entry.continuation.len());
            }
        }
    }
}

/// Truncate a string to `max_len` characters, appending `…` if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_owned()
    } else {
        let mut end = max_len;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}
