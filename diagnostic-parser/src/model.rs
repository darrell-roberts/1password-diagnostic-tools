//! Data model for a 1Password `.1pdiagnostics` diagnostic report.
//!
//! The top-level [`DiagnosticReport`] struct mirrors the JSON structure of the file
//! and provides convenience methods for loading and inspecting the report.
//!
//! # Memory-efficient parsing
//!
//! For large diagnostic files, use [`DiagnosticReport::parse_log_entries_ref`]
//! instead of [`DiagnosticReport::parse_log_entries`]. The `_ref` variant
//! returns [`LogEntryRef`] values that borrow string data directly from the
//! log content already held by the report, avoiding ~33 MB of duplicate
//! `String` allocations for a typical 36 MB file.

use std::fmt;
use std::ops::Not as _;
use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, TimeZone, Utc};
use serde::de;
use serde::{Deserialize, Deserializer, Serialize};

/// Deserialize a Unix timestamp that may be either an integer or a
/// floating-point number, truncating any fractional seconds to produce an `i64`.
fn deserialize_timestamp<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i)
            } else if let Some(f) = n.as_f64() {
                Ok(f as i64)
            } else {
                Err(serde::de::Error::custom(format!(
                    "timestamp number out of range: {n}"
                )))
            }
        }
        _ => Err(serde::de::Error::custom(format!(
            "expected a number for timestamp, got: {value}"
        ))),
    }
}

use crate::error::{DiagnosticError, Result};
use crate::log_entry::{LogEntry, LogEntryRef, StringInterner};

// ---------------------------------------------------------------------------
// Top-level report
// ---------------------------------------------------------------------------

/// A fully-parsed 1Password diagnostic report.
///
/// This is the root object serialized inside a `.1pdiagnostics` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    /// Unix timestamp (seconds) when the report was created.
    /// Accepts both integer and floating-point values in JSON (fractional
    /// seconds are truncated).
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub created_at: i64,

    /// Unique identifier for this diagnostic report.
    pub uuid: String,

    /// Information about the client machine and 1Password installation.
    pub system: System,

    /// High-level counts of accounts, vaults, and items.
    /// Some clients (e.g. the Safari extension) omit this field entirely.
    #[serde(default)]
    pub overview: Option<Overview>,

    /// Per-account metadata including vault listings.
    pub accounts: Vec<Account>,

    /// Raw log files captured in the report. Each entry represents one
    /// rotated log file with a title (file name) and its full text content.
    pub logs: Vec<LogFile>,

    /// Crash / panic reports recorded by the client.
    pub crash_report_entries: Vec<CrashReportEntry>,
}

impl FromStr for DiagnosticReport {
    type Err = DiagnosticError;

    fn from_str(json: &str) -> std::result::Result<Self, Self::Err> {
        serde_json::from_str::<Self>(json).map_err(Into::into)
    }
}

impl DiagnosticReport {
    /// Read and parse a `.1pdiagnostics` file from disk.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let data = std::fs::read_to_string(path).map_err(|source| DiagnosticError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        data.parse()
    }

    /// Parse a diagnostic report from a byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice::<Self>(bytes).map_err(Into::into)
    }

    /// The report creation time as a [`DateTime<Utc>`].
    pub fn created_at_utc(&self) -> Option<DateTime<Utc>> {
        Utc.timestamp_opt(self.created_at, 0).single()
    }

    /// Parse every line inside every [`LogFile`] into structured [`LogEntry`]
    /// values. Lines that cannot be parsed (e.g. stack-trace continuation
    /// lines) are attached to the preceding entry's `continuation` field.
    ///
    /// The returned entries are in file-order across all log files, each
    /// tagged with the originating [`LogFile::title`].
    ///
    /// This allocates owned `String`s for every field of every entry. For
    /// large files, prefer [`parse_log_entries_ref`](Self::parse_log_entries_ref)
    /// which borrows from the log content already in memory.
    pub fn parse_log_entries(&self) -> Vec<LogEntry> {
        self.logs
            .iter()
            .flat_map(|log_file| LogEntry::parse_log_content(&log_file.title, &log_file.content))
            .collect()
    }

    /// Zero-copy version of [`parse_log_entries`](Self::parse_log_entries).
    ///
    /// Returns [`LogEntryRef`] values that borrow `&str` slices directly
    /// from the [`LogFile::content`] strings already owned by this report.
    /// High-repetition fields (`log_file_title`, `thread`) are shared via
    /// [`Arc<str>`](std::sync::Arc) through a [`StringInterner`].
    ///
    /// For a typical 36 MB diagnostic file with ~127 k log entries this
    /// eliminates ~638 k `String` allocations (~33 MB of heap), reducing
    /// peak memory by roughly 30%.
    ///
    /// The returned entries borrow from `&self`, so the report must outlive
    /// the entries.
    pub fn parse_log_entries_ref(&self) -> (Vec<LogEntryRef<'_>>, StringInterner) {
        let mut interner = StringInterner::new();
        let mut all_entries: Vec<LogEntryRef<'_>> = Vec::new();

        for log_file in &self.logs {
            let entries =
                LogEntryRef::parse_log_content(&log_file.title, &log_file.content, &mut interner);
            all_entries.extend(entries);
        }

        (all_entries, interner)
    }

    /// Total number of individual log lines across all log files.
    pub fn total_log_lines(&self) -> usize {
        self.logs
            .iter()
            .map(|lf| lf.content.lines().filter(|l| !l.is_empty()).count())
            .sum()
    }
}

// ---------------------------------------------------------------------------
// System information
// ---------------------------------------------------------------------------

/// Information about the host system and 1Password client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct System {
    /// Human-readable client name, e.g. `"1Password for Linux"`.
    pub client_name: String,

    /// Numeric build identifier.
    pub client_build: u64,

    /// CPU model string.
    pub client_processor: String,

    /// Whether the client was locked at the time the report was generated.
    pub client_is_locked: bool,

    /// Operating system name, e.g. `"Linux"`, `"macOS"`, `"Windows"`.
    pub os_name: String,

    /// Operating system version string.
    pub os_version: String,

    /// Locale tag, e.g. `"en-US"`.
    pub locale: String,

    /// Total disk space (human-readable string).
    pub total_space: String,

    /// Free disk space (human-readable string).
    pub free_space: String,

    /// Total RAM (human-readable string).
    pub memory: String,

    /// Hardware model name (e.g. `"MacBookPro18,3"`).
    /// Some clients (e.g. browser extensions) report `"Unknown"`.
    #[serde(default)]
    pub model_name: String,

    /// Feature flags active on this client.
    pub features: Vec<Feature>,

    /// Browser extensions known to the client.
    /// Depending on the client, each element may be a JSON object with
    /// `name`/`version` keys **or** a plain string description such as
    /// `"1Password – Password Manager (8.12.4.46, Enabled)"`.
    #[serde(default)]
    pub extensions: Vec<Extension>,

    /// Filesystem path where the client is installed.
    #[serde(default)]
    pub install_location: String,

    /// Installer identifier (e.g. `"Unknown"` for browser extensions).
    #[serde(default)]
    pub installer: String,
}

/// A named feature flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
}

impl fmt::Display for Feature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// A browser extension entry (may be empty in many reports).
///
/// In desktop-client diagnostics each extension is a JSON object with
/// optional `name` and `version` keys.  In browser-extension diagnostics
/// the array contains plain strings like
/// `"1Password – Password Manager (8.12.4.46, Enabled)"`.
/// The custom [`Deserialize`] impl handles both representations.
#[derive(Debug, Clone, Serialize)]
pub struct Extension {
    pub name: Option<String>,
    pub version: Option<String>,
}

impl<'de> Deserialize<'de> for Extension {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ExtensionVisitor;

        impl<'de> de::Visitor<'de> for ExtensionVisitor {
            type Value = Extension;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a string or an object with optional name/version fields")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Extension, E> {
                Ok(Extension::from_description(v))
            }

            fn visit_map<A: de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Extension, A::Error> {
                let mut name: Option<String> = None;
                let mut version: Option<String> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "name" => name = map.next_value()?,
                        "version" => version = map.next_value()?,
                        _ => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                Ok(Extension { name, version })
            }
        }

        deserializer.deserialize_any(ExtensionVisitor)
    }
}

impl Extension {
    /// Parse a plain-string extension description into structured fields.
    ///
    /// Handles strings like `"Name Here (1.2.3, Enabled)"` by splitting on
    /// the last `(` to extract an optional version and the name.
    fn from_description(s: &str) -> Self {
        // Try to split "Name (version, state)" on the last '('.
        if let Some(paren_start) = s.rfind('(') {
            let name_part = s[..paren_start].trim();
            let inner = s[paren_start + 1..].trim_end_matches(')').trim();
            // The part inside the parens is typically "version, Enabled/Disabled".
            let version = inner.split(',').next().map(|v| v.trim().to_owned());
            Extension {
                name: name_part.is_empty().not().then(|| name_part.to_owned()),
                version,
            }
        } else {
            Extension {
                name: Some(s.to_owned()),
                version: None,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Overview
// ---------------------------------------------------------------------------

/// High-level item / vault counts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Overview {
    /// Number of accounts configured.
    pub accounts: u32,

    /// Total number of vaults across all accounts.
    pub vaults: u32,

    /// Number of active (non-archived, non-deleted) items.
    pub active_items: u32,

    /// Number of inactive (archived + deleted) items.
    pub inactive_items: u32,
}

// ---------------------------------------------------------------------------
// Accounts & Vaults
// ---------------------------------------------------------------------------

/// Metadata for a single 1Password account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Account UUID.
    pub uuid: String,

    /// Server URL (e.g. `"1password.com"`).
    pub url: String,

    /// Account type code.
    pub account_type: AccountType,

    /// Account state code.
    /// Some accounts (e.g. locked/inaccessible ones) may have a `null` state.
    #[serde(default)]
    pub account_state: Option<AccountState>,

    /// Whether this account is currently locked.
    pub account_is_locked: bool,

    /// Attribute version counter.
    pub attr_version: u64,

    /// Storage used in bytes.
    pub storage_used: u64,

    /// Billing status code.
    /// Some clients (e.g. the Safari extension) omit this field.
    #[serde(default)]
    pub billing_status: Option<BillingStatus>,

    /// Device UUID registered for this account.
    pub device_uuid: String,

    /// Vaults belonging to this account.
    pub vaults: Vec<Vault>,

    /// User UUID within the account.
    pub user_uuid: String,

    /// User state code.
    /// Some clients (e.g. the Safari extension) omit this field.
    #[serde(default)]
    pub user_state: Option<String>,

    /// Feature flags specific to this account.
    pub features: Vec<Feature>,
}

/// Account type codes as seen in the diagnostic JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccountType {
    /// Individual account.
    #[serde(rename = "I")]
    Individual,

    /// Family account.
    #[serde(rename = "F")]
    Family,

    /// Business / Teams account.
    #[serde(rename = "B")]
    Business,

    /// Unknown / other type.
    #[serde(other)]
    Other,
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Individual => write!(f, "Individual"),
            Self::Family => write!(f, "Family"),
            Self::Business => write!(f, "Business"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Account state codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccountState {
    /// Active.
    #[serde(rename = "A")]
    Active,

    /// Suspended.
    #[serde(rename = "S")]
    Suspended,

    /// Unknown / other state.
    #[serde(other)]
    Other,
}

impl fmt::Display for AccountState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Suspended => write!(f, "Suspended"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Billing status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BillingStatus {
    /// Trialing.
    #[serde(rename = "T")]
    Trial,

    /// Active / paid.
    #[serde(rename = "A")]
    Active,

    /// Grace period.
    #[serde(rename = "G")]
    Grace,

    /// Frozen / suspended.
    #[serde(rename = "F")]
    Frozen,

    /// Unknown / other.
    #[serde(other)]
    Other,
}

impl fmt::Display for BillingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trial => write!(f, "Trial"),
            Self::Active => write!(f, "Active"),
            Self::Grace => write!(f, "Grace"),
            Self::Frozen => write!(f, "Frozen"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// A single vault within an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vault {
    /// Vault UUID.
    pub uuid: String,

    /// Unix timestamp (seconds) when the vault was created.
    pub created_at: i64,

    /// Vault type code.
    pub vault_type: VaultType,

    /// Unix timestamp (seconds) when the vault was last updated.
    pub updated_at: i64,

    /// Access-control bitmask.
    pub acl: u64,

    /// Content version counter.
    pub content_version: u64,

    /// Item counts within this vault.
    pub items: VaultItems,
}

impl Vault {
    /// The vault creation time as a [`DateTime<Utc>`].
    pub fn created_at_utc(&self) -> Option<DateTime<Utc>> {
        Utc.timestamp_opt(self.created_at, 0).single()
    }

    /// The vault last-updated time as a [`DateTime<Utc>`].
    pub fn updated_at_utc(&self) -> Option<DateTime<Utc>> {
        Utc.timestamp_opt(self.updated_at, 0).single()
    }

    /// Total number of items in this vault (all states combined).
    pub fn total_items(&self) -> u32 {
        self.items.active + self.items.deleted + self.items.archived + self.items.rejected
    }
}

/// Vault type codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VaultType {
    /// Personal / private vault.
    #[serde(rename = "P")]
    Personal,

    /// User-created vault.
    #[serde(rename = "U")]
    UserCreated,

    /// Everyone (shared) vault.
    #[serde(rename = "E")]
    Everyone,

    /// Unknown / other type.
    #[serde(other)]
    Other,
}

impl fmt::Display for VaultType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Personal => write!(f, "Personal"),
            Self::UserCreated => write!(f, "User Created"),
            Self::Everyone => write!(f, "Everyone"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Item counts within a vault, broken down by state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultItems {
    /// Number of active items.
    pub active: u32,

    /// Number of soft-deleted items.
    pub deleted: u32,

    /// Number of archived items.
    pub archived: u32,

    /// Number of rejected items (e.g. sharing conflicts).
    pub rejected: u32,

    /// Items that have local changes not yet synced.
    #[serde(default)]
    pub with_offline_changes: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Log files
// ---------------------------------------------------------------------------

/// A single log file captured in the diagnostic report.
///
/// Each log file has a title (its path/name on disk) and the full text
/// content. The content typically contains newline-separated log lines,
/// each beginning with a log level keyword (`INFO`, `WARN`, `ERROR`, etc.).
/// Some lines (e.g. stack traces) are continuation lines belonging to the
/// preceding log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFile {
    /// The log file path / name, e.g. `"/1Password_r00217"`.
    pub title: String,

    /// The full text content of the log file.
    pub content: String,
}

impl LogFile {
    /// Return the log file category inferred from the title.
    pub fn category(&self) -> LogFileCategory {
        if self.title.contains("/BrowserSupport/") {
            LogFileCategory::BrowserSupport
        } else if self.title.contains("/CrashHandler/") {
            LogFileCategory::CrashHandler
        } else {
            LogFileCategory::App
        }
    }

    /// Parse the content of this log file into structured [`LogEntry`] values.
    pub fn parse_entries(&self) -> Vec<LogEntry> {
        LogEntry::parse_log_content(&self.title, &self.content)
    }

    /// Zero-copy version of [`parse_entries`](Self::parse_entries).
    pub fn parse_entries_ref<'a>(&'a self, interner: &mut StringInterner) -> Vec<LogEntryRef<'a>> {
        LogEntryRef::parse_log_content(&self.title, &self.content, interner)
    }

    /// Number of non-empty lines in this log file.
    pub fn line_count(&self) -> usize {
        self.content.lines().filter(|l| !l.is_empty()).count()
    }
}

/// Category of a log file, derived from its title/path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogFileCategory {
    /// Main 1Password application logs (e.g. `/1Password_r00217`).
    App,
    /// Browser support / extension bridge logs.
    BrowserSupport,
    /// Crash handler process logs.
    CrashHandler,
}

impl fmt::Display for LogFileCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::App => write!(f, "App"),
            Self::BrowserSupport => write!(f, "Browser Support"),
            Self::CrashHandler => write!(f, "Crash Handler"),
        }
    }
}

// ---------------------------------------------------------------------------
// Crash report entries
// ---------------------------------------------------------------------------

/// A crash / panic report entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReportEntry {
    /// Tag linking this crash to a diagnostic report.
    pub diagnostic_report_tag: String,

    /// Unix timestamp (seconds) of the crash.
    /// Accepts both integer and floating-point values in JSON (fractional
    /// seconds are truncated).
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub timestamp: i64,

    /// Type of crash, e.g. `"panic"`.
    pub report_type: String,

    /// Identifier string for the report (contains version + timestamp).
    pub report_id: String,
}

impl CrashReportEntry {
    /// The crash timestamp as a [`DateTime<Utc>`].
    pub fn timestamp_utc(&self) -> Option<DateTime<Utc>> {
        Utc.timestamp_opt(self.timestamp, 0).single()
    }

    /// Find the panic [`LogEntry`] that corresponds to this crash report by
    /// matching timestamps. The crash report records a Unix-second timestamp
    /// while the panic log entry has a sub-second precision timestamp, so we
    /// look for the panic entry whose UTC timestamp is closest to the crash
    /// timestamp and within a `max_drift` tolerance (default: 2 seconds).
    ///
    /// `entries` should be the full set of parsed log entries (e.g. from
    /// [`DiagnosticReport::parse_log_entries`]).
    ///
    /// Returns `None` if no panic entry is found within the tolerance.
    pub fn find_panic_entry<'a>(&self, entries: &'a [LogEntry]) -> Option<&'a LogEntry> {
        self.find_panic_entry_with_drift(entries, chrono::TimeDelta::seconds(2))
    }

    /// Like [`find_panic_entry`](Self::find_panic_entry) but with a custom
    /// maximum drift tolerance.
    pub fn find_panic_entry_with_drift<'a>(
        &self,
        entries: &'a [LogEntry],
        max_drift: chrono::TimeDelta,
    ) -> Option<&'a LogEntry> {
        let crash_ts = self.timestamp_utc()?;

        entries
            .iter()
            .filter(|e| e.is_panic())
            .filter_map(|e| {
                let diff = (e.timestamp_utc() - crash_ts).abs();
                (diff <= max_drift).then_some((diff, e))
            })
            .min_by_key(|(diff, _)| *diff)
            .map(|(_, entry)| entry)
    }

    /// Zero-copy version of [`find_panic_entry`](Self::find_panic_entry).
    ///
    /// Searches a slice of [`LogEntryRef`] values for the panic entry
    /// matching this crash report's timestamp.
    pub fn find_panic_entry_ref<'a>(
        &self,
        entries: &'a [LogEntryRef<'a>],
    ) -> Option<&'a LogEntryRef<'a>> {
        self.find_panic_entry_ref_with_drift(entries, chrono::TimeDelta::seconds(2))
    }

    /// Like [`find_panic_entry_ref`](Self::find_panic_entry_ref) but with a
    /// custom maximum drift tolerance.
    pub fn find_panic_entry_ref_with_drift<'a>(
        &self,
        entries: &'a [LogEntryRef<'a>],
        max_drift: chrono::TimeDelta,
    ) -> Option<&'a LogEntryRef<'a>> {
        let crash_ts = self.timestamp_utc()?;

        entries
            .iter()
            .filter(|e| e.is_panic())
            .filter_map(|e| {
                let diff = (e.timestamp_utc() - crash_ts).abs();
                (diff <= max_drift).then_some((diff, e))
            })
            .min_by_key(|(diff, _)| *diff)
            .map(|(_, entry)| entry)
    }
}

// ---------------------------------------------------------------------------
// Display implementations for summary output
// ---------------------------------------------------------------------------

impl fmt::Display for DiagnosticReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "1Password Diagnostic Report")?;
        writeln!(f, "  UUID:       {}", self.uuid)?;
        if let Some(dt) = self.created_at_utc() {
            writeln!(f, "  Created:    {dt}")?;
        }
        writeln!(f, "  Client:     {}", self.system.client_name)?;
        writeln!(
            f,
            "  OS:         {} {}",
            self.system.os_name, self.system.os_version
        )?;
        writeln!(f, "  Locale:     {}", self.system.locale)?;
        if let Some(ref overview) = self.overview {
            writeln!(f, "  Accounts:   {}", overview.accounts)?;
            writeln!(f, "  Vaults:     {}", overview.vaults)?;
            writeln!(
                f,
                "  Items:      {} active, {} inactive",
                overview.active_items, overview.inactive_items
            )?;
        }
        writeln!(f, "  Log files:  {}", self.logs.len())?;
        writeln!(f, "  Crashes:    {}", self.crash_report_entries.len())?;
        Ok(())
    }
}

impl fmt::Display for System {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "System Information")?;
        writeln!(
            f,
            "  Client:     {} (build {})",
            self.client_name, self.client_build
        )?;
        writeln!(f, "  Processor:  {}", self.client_processor)?;
        writeln!(f, "  Locked:     {}", self.client_is_locked)?;
        writeln!(f, "  OS:         {} {}", self.os_name, self.os_version)?;
        writeln!(f, "  Locale:     {}", self.locale)?;
        writeln!(
            f,
            "  Disk:       {} total, {} free",
            self.total_space, self.free_space
        )?;
        writeln!(f, "  Memory:     {}", self.memory)?;
        writeln!(f, "  Features:   {}", self.features.len())?;
        writeln!(f, "  Extensions: {}", self.extensions.len())?;
        writeln!(f, "  Install:    {}", self.install_location)?;
        Ok(())
    }
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Account {}", self.uuid)?;
        writeln!(f, "  URL:      {}", self.url)?;
        writeln!(f, "  Type:     {}", self.account_type)?;
        match self.account_state {
            Some(ref state) => writeln!(f, "  State:    {state}")?,
            None => writeln!(f, "  State:    N/A")?,
        }
        writeln!(f, "  Locked:   {}", self.account_is_locked)?;
        if let Some(ref billing) = self.billing_status {
            writeln!(f, "  Billing:  {billing}")?;
        }
        writeln!(f, "  Vaults:   {}", self.vaults.len())?;
        writeln!(f, "  Features: {}", self.features.len())?;
        Ok(())
    }
}

impl fmt::Display for CrashReportEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] ", self.report_type)?;
        if let Some(dt) = self.timestamp_utc() {
            write!(f, "{dt} ")?;
        }
        write!(f, "{}", self.report_id)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_json() -> &'static str {
        r#"{
            "created_at": 1772740461,
            "uuid": "test-uuid",
            "system": {
                "client_name": "1Password for Linux",
                "client_build": 81208022,
                "client_processor": "Test CPU",
                "client_is_locked": false,
                "os_name": "Linux",
                "os_version": "Ubuntu 24.04",
                "locale": "en-US",
                "total_space": "100 GB",
                "free_space": "50 GB",
                "memory": "8 GB",
                "features": [{"name": "test-feature"}],
                "extensions": [],
                "install_location": "/opt/1Password/1password"
            },
            "overview": {
                "accounts": 1,
                "vaults": 2,
                "active_items": 100,
                "inactive_items": 5
            },
            "accounts": [{
                "uuid": "ACCT-UUID",
                "url": "1password.com",
                "account_type": "B",
                "account_state": "A",
                "account_is_locked": false,
                "attr_version": 1,
                "storage_used": 0,
                "billing_status": "T",
                "device_uuid": "device-1",
                "vaults": [{
                    "uuid": "vault-1",
                    "created_at": 1706091553,
                    "vault_type": "P",
                    "updated_at": 1772739697,
                    "acl": 15730674,
                    "content_version": 100,
                    "items": {
                        "active": 50,
                        "deleted": 2,
                        "archived": 3,
                        "rejected": 0,
                        "with_offline_changes": []
                    }
                }],
                "user_uuid": "USER-UUID",
                "user_state": "A",
                "features": [{"name": "test-account-feature"}]
            }],
            "logs": [{
                "title": "/1Password_r00001",
                "content": "INFO  2026-03-05T19:36:06.278+00:00 ThreadId(6) [1P:op-settings/src/store/json_store.rs:75] Settings file created\nWARN  2026-03-05T19:36:07.000+00:00 ThreadId(6) [client:typescript] Some warning"
            }],
            "crash_report_entries": [{
                "diagnostic_report_tag": "tag-1",
                "timestamp": 1772739881,
                "report_type": "panic",
                "report_id": "1Password_8.12.8_2026-03-05_21-44-19"
            }]
        }"#
    }

    #[test]
    fn parse_minimal_report() {
        let report = DiagnosticReport::from_str(minimal_json()).unwrap();
        assert_eq!(report.uuid, "test-uuid");
        assert_eq!(report.system.client_name, "1Password for Linux");
        assert_eq!(report.overview.as_ref().unwrap().accounts, 1);
        assert_eq!(report.overview.as_ref().unwrap().active_items, 100);
        assert_eq!(report.accounts.len(), 1);
        assert_eq!(report.accounts[0].account_type, AccountType::Business);
        assert_eq!(report.accounts[0].account_state, Some(AccountState::Active));
        assert_eq!(
            report.accounts[0].billing_status,
            Some(BillingStatus::Trial)
        );
        assert_eq!(report.accounts[0].vaults.len(), 1);
        assert_eq!(report.accounts[0].vaults[0].vault_type, VaultType::Personal);
        assert_eq!(report.accounts[0].vaults[0].total_items(), 55);
        assert_eq!(report.logs.len(), 1);
        assert_eq!(report.crash_report_entries.len(), 1);
    }

    #[test]
    fn created_at_utc() {
        let report = DiagnosticReport::from_str(minimal_json()).unwrap();
        let dt = report.created_at_utc().unwrap();
        assert_eq!(dt.year(), 2026);
    }

    #[test]
    fn log_file_category() {
        let app = LogFile {
            title: "/1Password_r00001".into(),
            content: String::new(),
        };
        let browser = LogFile {
            title: "/BrowserSupport/1Password_r00001".into(),
            content: String::new(),
        };
        let crash = LogFile {
            title: "/CrashHandler/1Password_r00001".into(),
            content: String::new(),
        };
        assert_eq!(app.category(), LogFileCategory::App);
        assert_eq!(browser.category(), LogFileCategory::BrowserSupport);
        assert_eq!(crash.category(), LogFileCategory::CrashHandler);
    }

    #[test]
    fn parse_log_entries() {
        let report = DiagnosticReport::from_str(minimal_json()).unwrap();
        let entries = report.parse_log_entries();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn parse_log_entries_ref() {
        let report = DiagnosticReport::from_str(minimal_json()).unwrap();
        let (entries, interner) = report.parse_log_entries_ref();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].level, crate::LogLevel::Info);
        assert_eq!(entries[1].level, crate::LogLevel::Warn);
        assert_eq!(&*entries[0].log_file_title, "/1Password_r00001");
        // The interner should have the log file title + thread id(s).
        assert!(interner.len() >= 2);
    }

    #[test]
    fn parse_log_entries_ref_matches_owned() {
        let report = DiagnosticReport::from_str(minimal_json()).unwrap();
        let owned = report.parse_log_entries();
        let (refs, _interner) = report.parse_log_entries_ref();

        assert_eq!(owned.len(), refs.len());
        for (o, r) in owned.iter().zip(refs.iter()) {
            assert_eq!(o.level, r.level);
            assert_eq!(o.timestamp, r.timestamp);
            assert_eq!(o.thread, &*r.thread);
            assert_eq!(o.source.component, r.source.component);
            assert_eq!(o.source.detail.as_deref(), r.source.detail);
            assert_eq!(o.message, r.message);
            assert_eq!(o.continuation.len(), r.continuation.len());
        }
    }

    #[test]
    fn display_report() {
        let report = DiagnosticReport::from_str(minimal_json()).unwrap();
        let display = format!("{report}");
        assert!(display.contains("1Password Diagnostic Report"));
        assert!(display.contains("test-uuid"));
    }

    #[test]
    fn vault_type_display() {
        assert_eq!(VaultType::Personal.to_string(), "Personal");
        assert_eq!(VaultType::UserCreated.to_string(), "User Created");
        assert_eq!(VaultType::Everyone.to_string(), "Everyone");
    }

    #[test]
    fn account_type_display() {
        assert_eq!(AccountType::Individual.to_string(), "Individual");
        assert_eq!(AccountType::Family.to_string(), "Family");
        assert_eq!(AccountType::Business.to_string(), "Business");
    }

    use chrono::Datelike;
}
