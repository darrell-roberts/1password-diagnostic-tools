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
    let mut cache = StringCache::new();
    let entries = LogEntryRef::parse_log_content("test", SAMPLE_INFO, &mut cache);
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
    let mut cache = StringCache::new();
    let entries = LogEntryRef::parse_log_content("test", SAMPLE_ERROR, &mut cache);
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

    let mut cache = StringCache::new();
    let entries = LogEntryRef::parse_log_content("test_file", content, &mut cache);
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

    let mut cache = StringCache::new();
    let entries = LogEntryRef::parse_log_content("f", content, &mut cache);
    assert_eq!(entries.len(), 1);
    let full = entries[0].full_message();
    assert!(full.starts_with("panic"));
    assert!(full.contains("frame_a"));
    assert!(full.contains("frame_b"));
}

#[test]
fn ref_to_owned_roundtrip() {
    let mut cache = StringCache::new();
    let refs = LogEntryRef::parse_log_content("test", SAMPLE_INFO, &mut cache);
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
    let mut cache = StringCache::new();
    let entries = LogEntryRef::parse_log_content("test", SAMPLE_INFO, &mut cache);
    let display = entries[0].to_string();
    assert!(display.contains("INFO"));
    assert!(display.contains("Settings file created"));
}

#[test]
fn ref_timestamp_utc() {
    let mut cache = StringCache::new();
    let entries = LogEntryRef::parse_log_content("test", SAMPLE_INFO, &mut cache);
    let utc = entries[0].timestamp_utc();
    assert_eq!(utc.date_naive().year(), 2026);
}

#[test]
fn ref_empty_content() {
    let mut cache = StringCache::new();
    assert!(LogEntryRef::parse_log_content("e", "", &mut cache).is_empty());
    assert!(LogEntryRef::parse_log_content("b", "\n\n\n", &mut cache).is_empty());
}

// ── String cache tests ────────────────────────────────────────

#[test]
fn cache_deduplicates() {
    let mut cache = StringCache::new();
    let a1 = cache.cached("ThreadId(6)");
    let a2 = cache.cached("ThreadId(6)");
    let b = cache.cached("ThreadId(7)");

    // Same pointer for identical strings.
    assert!(Arc::ptr_eq(&a1, &a2));
    // Different pointer for different strings.
    assert!(!Arc::ptr_eq(&a1, &b));
    assert_eq!(cache.len(), 2);
}

#[test]
fn cache_shared_across_files() {
    let mut cache = StringCache::new();

    let content1 = "INFO  2026-03-05T19:36:06.278+00:00 ThreadId(6) [1P:a.rs:1] msg1";
    let content2 = "INFO  2026-03-05T19:36:07.000+00:00 ThreadId(6) [1P:b.rs:2] msg2";

    let entries1 = LogEntryRef::parse_log_content("/file1", content1, &mut cache);
    let entries2 = LogEntryRef::parse_log_content("/file2", content2, &mut cache);

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
    let mut cache = StringCache::new();
    let entry =
        LogEntryRef::parse_line(&cache.cached("test"), SAMPLE_SAFARI_INFO, &mut cache).unwrap();
    assert_eq!(entry.level, LogLevel::Info);
    assert_eq!(&*entry.thread, "");
    assert_eq!(entry.source.component, "TrelicaReporting");
    assert_eq!(entry.message, "Purging stale Trelica activity");
}
