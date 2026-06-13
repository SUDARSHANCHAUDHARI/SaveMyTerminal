use assert_cmd::Command;
use predicates::prelude::*;
use savemyterminal::{config, paths::AppPaths};

fn command(temp: &tempfile::TempDir) -> Command {
    let mut command = Command::cargo_bin("smt").unwrap();
    command.args([
        "config",
        "--config-dir",
        temp.path().join("config").to_str().unwrap(),
        "--runtime-dir",
        temp.path().join("runtime").to_str().unwrap(),
        "--data-dir",
        temp.path().join("data").to_str().unwrap(),
    ]);
    command
}

fn phase_command(temp: &tempfile::TempDir, name: &str) -> Command {
    let mut command = Command::cargo_bin("smt").unwrap();
    command.arg(name).args([
        "--config-dir",
        temp.path().join("config").to_str().unwrap(),
        "--runtime-dir",
        temp.path().join("runtime").to_str().unwrap(),
        "--data-dir",
        temp.path().join("data").to_str().unwrap(),
    ]);
    command
}

fn paths(temp: &tempfile::TempDir) -> AppPaths {
    AppPaths {
        config_dir: temp.path().join("config"),
        runtime_dir: temp.path().join("runtime"),
        data_dir: temp.path().join("data"),
    }
}

#[test]
fn config_path_and_show_do_not_create_settings() {
    let temp = tempfile::tempdir().unwrap();
    let settings_file = paths(&temp).settings_file();

    command(&temp)
        .arg("path")
        .assert()
        .success()
        .stdout(predicate::str::contains(settings_file.to_str().unwrap()));
    command(&temp)
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("version = 1"))
        .stdout(predicate::str::contains("dashboard_port = \"auto\""));

    assert!(!settings_file.exists());
}

#[test]
fn config_set_writes_a_validated_settings_file() {
    let temp = tempfile::tempdir().unwrap();
    let app_paths = paths(&temp);

    command(&temp)
        .args(["set", "history.retention_days", "14"])
        .assert()
        .success()
        .stdout(predicate::str::contains("settings updated"));

    assert_eq!(
        config::load(&app_paths.settings_file())
            .unwrap()
            .history
            .retention_days,
        14
    );
}

#[test]
fn config_reset_previews_before_apply() {
    let temp = tempfile::tempdir().unwrap();
    let app_paths = paths(&temp);
    command(&temp)
        .args(["set", "history.retention_days", "14"])
        .assert()
        .success();

    command(&temp)
        .args(["reset", "history.retention_days"])
        .assert()
        .success()
        .stdout(predicate::str::contains("preview"));
    assert_eq!(
        config::load(&app_paths.settings_file())
            .unwrap()
            .history
            .retention_days,
        14
    );

    command(&temp)
        .args(["reset", "history.retention_days", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains("settings updated"));
    assert_eq!(
        config::load(&app_paths.settings_file())
            .unwrap()
            .history
            .retention_days,
        30
    );
}

#[test]
fn invalid_config_set_leaves_existing_settings_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let app_paths = paths(&temp);
    command(&temp)
        .args(["set", "history.retention_days", "14"])
        .assert()
        .success();
    let before = std::fs::read(app_paths.settings_file()).unwrap();

    command(&temp)
        .args(["set", "history.retention_days", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("history.retention_days"));

    assert_eq!(std::fs::read(app_paths.settings_file()).unwrap(), before);
}

#[test]
fn setup_previews_detection_and_settings_creation_without_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let app_paths = paths(&temp);

    phase_command(&temp, "setup")
        .assert()
        .success()
        .stdout(predicate::str::contains("detected os:"))
        .stdout(predicate::str::contains("preview: create settings"));

    assert!(!app_paths.settings_file().exists());
}

#[test]
fn setup_apply_creates_valid_default_settings() {
    let temp = tempfile::tempdir().unwrap();
    let app_paths = paths(&temp);

    phase_command(&temp, "setup")
        .arg("--apply")
        .assert()
        .success()
        .stdout(predicate::str::contains("settings created"));

    assert_eq!(
        config::load(&app_paths.settings_file()).unwrap(),
        config::Settings::default()
    );
}

#[test]
fn setup_rejects_unknown_selected_integrations() {
    let temp = tempfile::tempdir().unwrap();

    phase_command(&temp, "setup")
        .args(["--integration", "not-registered"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not-registered"));
}

#[test]
fn uninstall_preview_preserves_owned_config_and_history() {
    let temp = tempfile::tempdir().unwrap();
    let app_paths = paths(&temp);
    config::save_atomic(&app_paths.settings_file(), &config::Settings::default()).unwrap();
    std::fs::write(app_paths.token_file(), "secret").unwrap();
    std::fs::create_dir_all(&app_paths.data_dir).unwrap();
    std::fs::write(app_paths.database_file(), "history").unwrap();

    phase_command(&temp, "uninstall")
        .args(["--remove-config", "--purge-data"])
        .assert()
        .success()
        .stdout(predicate::str::contains("preview"));

    assert!(app_paths.settings_file().exists());
    assert!(app_paths.token_file().exists());
    assert!(app_paths.database_file().exists());
}

#[test]
fn uninstall_apply_removes_only_explicit_owned_state() {
    let temp = tempfile::tempdir().unwrap();
    let app_paths = paths(&temp);
    config::save_atomic(&app_paths.settings_file(), &config::Settings::default()).unwrap();
    std::fs::write(app_paths.token_file(), "secret").unwrap();
    std::fs::create_dir_all(&app_paths.data_dir).unwrap();
    std::fs::write(app_paths.database_file(), "history").unwrap();

    phase_command(&temp, "uninstall")
        .args(["--remove-config", "--apply"])
        .assert()
        .success();
    assert!(!app_paths.settings_file().exists());
    assert!(!app_paths.token_file().exists());
    assert!(app_paths.database_file().exists());

    phase_command(&temp, "uninstall")
        .args(["--purge-data", "--apply"])
        .assert()
        .success();
    assert!(!app_paths.database_file().exists());
}

#[test]
fn doctor_prints_checks_and_uses_report_exit_status() {
    let temp = tempfile::tempdir().unwrap();
    phase_command(&temp, "doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS settings"))
        .stdout(predicate::str::contains("PASS service"))
        .stdout(predicate::str::contains("summary:"));

    std::fs::create_dir_all(paths(&temp).config_dir).unwrap();
    std::fs::write(paths(&temp).settings_file(), "invalid = true\n").unwrap();
    phase_command(&temp, "doctor")
        .assert()
        .failure()
        .stdout(predicate::str::contains("FAIL settings"));
}
