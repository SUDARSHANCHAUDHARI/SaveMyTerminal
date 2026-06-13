use savemyterminal::{
    doctor::{CheckLevel, DoctorReport, run_checks},
    manifest::{IntegrationManifest, IntegrationRecord, save_manifest_atomic},
    paths::AppPaths,
    service::ServiceDiscovery,
};
use std::path::PathBuf;

fn paths(temp: &tempfile::TempDir) -> AppPaths {
    AppPaths {
        config_dir: temp.path().join("config"),
        runtime_dir: temp.path().join("runtime"),
        data_dir: temp.path().join("data"),
    }
}

#[test]
fn report_exit_code_ignores_warnings_but_fails_on_failures() {
    let warning = DoctorReport::from_checks(vec![
        savemyterminal::doctor::CheckResult::pass("one", "ok"),
        savemyterminal::doctor::CheckResult::warn("two", "drift"),
    ]);
    assert_eq!(warning.exit_code(), 0);

    let failed = DoctorReport::from_checks(vec![
        savemyterminal::doctor::CheckResult::warn("one", "drift"),
        savemyterminal::doctor::CheckResult::fail("two", "broken"),
    ]);
    assert_eq!(failed.exit_code(), 1);
}

#[tokio::test]
async fn absent_on_demand_state_is_healthy() {
    let temp = tempfile::tempdir().unwrap();
    let report = run_checks(&paths(&temp)).await;

    assert_eq!(report.exit_code(), 0);
    assert!(report.checks.iter().any(|check| {
        check.id == "service"
            && check.level == CheckLevel::Pass
            && check.message.contains("on demand")
    }));
}

#[tokio::test]
async fn invalid_settings_and_non_loopback_discovery_are_independent_failures() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(&temp);
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::create_dir_all(&paths.runtime_dir).unwrap();
    std::fs::write(paths.settings_file(), "unknown = true\n").unwrap();
    std::fs::write(
        paths.discovery_file(),
        serde_json::to_vec(&ServiceDiscovery {
            base_url: "http://192.0.2.1:1234".to_owned(),
            pid: 1,
        })
        .unwrap(),
    )
    .unwrap();

    let report = run_checks(&paths).await;

    assert_eq!(report.exit_code(), 1);
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.id == "settings" && check.level == CheckLevel::Fail)
    );
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.id == "service" && check.level == CheckLevel::Fail)
    );
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.id == "network" && check.level == CheckLevel::Pass)
    );
}

#[tokio::test]
async fn manifest_drift_and_missing_backup_are_warnings() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(&temp);
    let target = temp.path().join("tool.conf");
    std::fs::write(
        &target,
        "# >>> SaveMyTerminal:example >>>\nmanaged\n# <<< SaveMyTerminal:example <<<\nuser edit\n",
    )
    .unwrap();
    save_manifest_atomic(
        &paths.manifest_file(),
        &IntegrationManifest {
            version: 1,
            integrations: vec![IntegrationRecord {
                id: "example".to_owned(),
                descriptor_version: 1,
                target_path: target,
                marker_id: "example".to_owned(),
                backup_path: Some(PathBuf::from("/missing/backup")),
                post_write_sha256: "aa".repeat(32),
                applied_at_unix_ms: 1,
            }],
        },
    )
    .unwrap();

    let report = run_checks(&paths).await;

    assert_eq!(report.exit_code(), 0);
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.id == "manifest_checksum" && check.level == CheckLevel::Warn)
    );
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.id == "manifest_backup" && check.level == CheckLevel::Warn)
    );
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.id == "manifest_markers" && check.level == CheckLevel::Pass)
    );
}

#[tokio::test]
async fn malformed_manifest_is_reported_without_hiding_other_checks() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(&temp);
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.manifest_file(), "not json").unwrap();

    let report = run_checks(&paths).await;

    assert_eq!(report.exit_code(), 1);
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.id == "manifest" && check.level == CheckLevel::Fail)
    );
    assert!(report.checks.iter().any(|check| check.id == "service"));
}
