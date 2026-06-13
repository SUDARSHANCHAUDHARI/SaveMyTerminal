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
