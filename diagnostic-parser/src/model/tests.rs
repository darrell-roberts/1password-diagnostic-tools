use super::{
    AccountState, AccountType, BillingStatus, DiagnosticReport, LogFile, LogFileCategory, VaultType,
};
use std::str::FromStr as _;

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
fn parse_log_entries_ref() {
    let report = DiagnosticReport::from_str(minimal_json()).unwrap();
    let (entries, cache) = report.parse_log_entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].level, crate::LogLevel::Info);
    assert_eq!(entries[1].level, crate::LogLevel::Warn);
    assert_eq!(&*entries[0].log_file_title, "/1Password_r00001");
    // The cache should have the log file title + thread id(s).
    assert!(cache.len() >= 2);
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
