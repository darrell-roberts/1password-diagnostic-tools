# diagnostic-parser

A Rust library for parsing 1Password `.1pdiagnostics` diagnostic report files.

A `.1pdiagnostics` file is a single-line JSON document exported by the 1Password
client. It contains system information, account metadata, structured log entries
from rotated log files, and crash/panic reports. This crate deserializes that
JSON into a rich, typed data model and provides a structured log-line parser that
turns raw log text into queryable `LogEntry` values — ready for a future TUI or
GUI application to sort, filter, search, and display.

## Features

- **Full deserialization** of the `.1pdiagnostics` JSON schema into strongly-typed
  Rust structs (`DiagnosticReport`, `System`, `Account`, `Vault`, `LogFile`,
  `CrashReportEntry`, and more).
- **Structured log parsing** — each log line is parsed into a `LogEntry` with
  discrete fields for level, timestamp, thread, source component, file path,
  line number, and message.
- **Continuation-line support** — multi-line entries such as stack traces are
  automatically attached to the preceding `LogEntry`.
- **Crash ↔ log correlation** — `CrashReportEntry::find_panic_entry()` matches
  a crash report to its panic log entry (and stack trace) by timestamp proximity.
- **Display implementations** — all key types implement `Display` for quick
  human-readable summaries.
- **Designed for downstream UIs** — `LogLevel` is `Ord` (for threshold
  filtering), timestamps are `chrono::DateTime` (for sorting and range queries),
  and log files carry a `LogFileCategory` tag (`App`, `BrowserSupport`,
  `CrashHandler`) for grouping.

## File Format Overview

The `.1pdiagnostics` JSON has the following top-level structure:

| Field                  | Description                                        |
|------------------------|----------------------------------------------------|
| `created_at`           | Unix timestamp (seconds) of report creation        |
| `uuid`                 | Unique report identifier                           |
| `system`               | Client name, build, OS, CPU, memory, feature flags |
| `overview`             | Counts of accounts, vaults, active/inactive items  |
| `accounts[]`           | Per-account metadata with nested vault listings    |
| `logs[]`               | Rotated log files, each with a `title` and `content` |
| `crash_report_entries[]` | Panic/crash records with timestamps and report IDs |

Each log file's `content` contains newline-separated lines in the format:

```
LEVEL TIMESTAMP THREAD [SOURCE] MESSAGE
```

For example:

```
INFO  2026-03-05T19:36:06.278+00:00 ThreadId(6) [1P:op-settings/src/store/json_store.rs:75] Settings loaded
ERROR 2026-03-05T19:22:01.469+00:00 runtime-worker(ThreadId(3)) [1P:op-crash-reporting/src/lib.rs:181] thread panicked
```

Known source component prefixes:

| Prefix   | Origin                                          |
|----------|-------------------------------------------------|
| `1P`     | Core 1Password Rust code (detail = `crate/path:line`) |
| `client` | TypeScript / Electron client layer              |
| `status` | Application status logger                       |

## Getting Started

### Requirements

- Rust **1.85+** (edition 2024)

### Build

```sh
cargo build
```

### Run Tests

```sh
cargo test
```

### Run the Demo Binary

A small binary is included that loads a `.1pdiagnostics` file and prints a
full summary including crash stack traces:

```sh
cargo run -- path/to/file.1pdiagnostics
```

## Library Usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
diagnostic-parser = { path = "../diagnostic-parser" }
```

### Load and Inspect a Report

```rust
use diagnostic_parser::DiagnosticReport;

let report = DiagnosticReport::from_file("path/to/file.1pdiagnostics")?;

println!("Client: {}", report.system.client_name);
println!("OS: {} {}", report.system.os_name, report.system.os_version);
println!("Accounts: {}", report.overview.accounts);
println!("Log files: {}", report.logs.len());
println!("Crash reports: {}", report.crash_report_entries.len());

// The Display impl prints a formatted summary:
println!("{report}");
```

### Parse Log Entries

```rust
use diagnostic_parser::{DiagnosticReport, LogLevel};

let report = DiagnosticReport::from_file("report.1pdiagnostics")?;
let entries = report.parse_log_entries();

// Filter to errors only.
let errors: Vec<_> = entries.iter()
    .filter(|e| e.level >= LogLevel::Error)
    .collect();

for entry in &errors {
    println!("{} [{}] {}", entry.timestamp, entry.source, entry.message);
}
```

### Sort and Search

```rust
let mut entries = report.parse_log_entries();

// Sort by timestamp (newest first).
entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

// Search messages by substring.
let ssh_entries: Vec<_> = entries.iter()
    .filter(|e| e.message.contains("ssh"))
    .collect();

// Group by source component.
use std::collections::HashMap;
let mut by_component: HashMap<&str, Vec<_>> = HashMap::new();
for entry in &entries {
    by_component.entry(&entry.source.component).or_default().push(entry);
}
```

### Correlate Crash Reports with Stack Traces

```rust
let report = DiagnosticReport::from_file("report.1pdiagnostics")?;
let entries = report.parse_log_entries();

for crash in &report.crash_report_entries {
    println!("Crash: {crash}");

    if let Some(panic_entry) = crash.find_panic_entry(&entries) {
        println!("  Message: {}", panic_entry.message);
        for frame in &panic_entry.continuation {
            println!("  {frame}");
        }
    }
}
```

### Inspect Vaults

```rust
use diagnostic_parser::VaultType;

for account in &report.accounts {
    println!("Account {} ({})", account.uuid, account.account_type);

    for vault in &account.vaults {
        println!(
            "  {} ({}) — {} active, {} archived",
            vault.uuid,
            vault.vault_type,
            vault.items.active,
            vault.items.archived,
        );
    }
}
```

## Project Structure

```
src/
├── lib.rs          Crate root — module declarations and public re-exports
├── error.rs        DiagnosticError enum (I/O, JSON, log-parse, timestamp)
├── model.rs        Data model mirroring the JSON schema
│                     DiagnosticReport, System, Overview, Account, Vault,
│                     VaultItems, LogFile, CrashReportEntry, and enums
│                     (AccountType, AccountState, BillingStatus, VaultType,
│                     LogFileCategory)
├── log_entry.rs    Structured log-line parser
│                     LogEntry, LogLevel, LogSource, and parsing helpers
└── main.rs         Demo binary — prints a full report summary
```

## Key Types

| Type                | Description |
|---------------------|-------------|
| `DiagnosticReport`  | Top-level report; entry point via `from_file()`, `from_str()`, or `from_bytes()` |
| `System`            | Client name, build, OS, CPU, memory, disk, feature flags, install path |
| `Overview`          | Aggregate counts: accounts, vaults, active/inactive items |
| `Account`           | UUID, URL, type, state, billing status, nested vaults and features |
| `Vault`             | UUID, type, timestamps, ACL, content version, item counts |
| `VaultItems`        | Breakdown: active, deleted, archived, rejected, offline changes |
| `LogFile`           | Title (filename) and raw text content; `category()` and `parse_entries()` |
| `LogFileCategory`   | `App`, `BrowserSupport`, or `CrashHandler` |
| `CrashReportEntry`  | Tag, timestamp, type, report ID; `find_panic_entry()` for correlation |
| `LogEntry`          | Parsed log line: level, timestamp, thread, source, message, continuation |
| `LogLevel`          | `Trace < Debug < Info < Warn < Error` — implements `Ord` for filtering |
| `LogSource`         | Component prefix + optional detail; `file_path()` and `line_number()` |
| `DiagnosticError`   | Error enum: `Io`, `Json`, `LogParse`, `TimestampParse` |

## Dependencies

| Crate        | Purpose                                      |
|--------------|----------------------------------------------|
| `serde`      | Derive `Serialize` / `Deserialize` for the data model |
| `serde_json` | JSON deserialization of the `.1pdiagnostics` file |
| `chrono`     | Timestamp parsing (`DateTime<FixedOffset>`, `DateTime<Utc>`) |
| `thiserror`  | Ergonomic `Error` derive for `DiagnosticError` |

## License

This project is provided as-is for personal use. No license has been specified yet.