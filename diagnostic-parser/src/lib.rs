//! Parser library for 1Password `.1pdiagnostics` diagnostic report files.
//!
//! A `.1pdiagnostics` file is a single JSON document containing system information,
//! account metadata, structured log entries, and crash reports from a 1Password client.
//!
//! # Usage
//!
//! ```no_run
//! use diagnostic_parser::DiagnosticReport;
//!
//! let report = DiagnosticReport::from_file("path/to/file.1pdiagnostics").unwrap();
//!
//! println!("Client: {}", report.system.client_name);
//! println!("OS: {} {}", report.system.os_name, report.system.os_version);
//! if let Some(ref overview) = report.overview {
//!     println!("Accounts: {}", overview.accounts);
//! }
//! println!("Log files: {}", report.logs.len());
//! println!("Crash reports: {}", report.crash_report_entries.len());
//! ```
//!
//! # Memory-efficient parsing
//!
//! For large diagnostic files, use the zero-copy parsing path to avoid
//! duplicating all log-line strings on the heap:
//!
//! ```no_run
//! use diagnostic_parser::DiagnosticReport;
//!
//! let report = DiagnosticReport::from_file("path/to/file.1pdiagnostics").unwrap();
//!
//! // Returns borrowed `LogEntryRef` values that point into the report's
//! // existing log content — no extra String allocations.
//! let (entries, _interner) = report.parse_log_entries_ref();
//!
//! for entry in &entries {
//!     println!("{} [{}] {}", entry.timestamp, entry.source, entry.message);
//! }
//! ```

pub mod error;
pub mod log_entry;
pub mod model;

pub use error::DiagnosticError;
pub use log_entry::{LogEntry, LogEntryRef, LogLevel, LogSource, LogSourceRef, StringInterner};
pub use model::{
    Account, AccountState, AccountType, BillingStatus, CrashReportEntry, DiagnosticReport, Feature,
    LogFile, Overview, System, Vault, VaultItems, VaultType,
};
