use assert_cmd::Command;
use predicates::prelude::*;
use savemyterminal::config::{Settings, save_atomic};
use savemyterminal::{
    paths::AppPaths,
    protocol::SessionState,
    runner::{RunOptions, run_with_options},
    service::{ServiceConfig, spawn_test_service},
    storage::SqliteStore,
};
use secrecy::SecretString;
use std::path::PathBuf;

fn build_helper() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let binary = temp
        .path()
        .join(format!("exit-with{}", std::env::consts::EXE_SUFFIX));
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/helpers/exit_with.rs");
    let status = std::process::Command::new("rustc")
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .status()
        .unwrap();
    assert!(status.success());
    (temp, binary)
}

#[test]
fn preserves_arguments_and_success_exit_code() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt")
        .unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--no-status", "--"])
        .arg(helper)
        .args(["0", "hello world", "--flag"])
        .assert()
        .success()
        .stdout(predicate::str::contains("arg=hello world"))
        .stdout(predicate::str::contains("arg=--flag"));
}

#[test]
fn preserves_nonzero_exit_code() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt")
        .unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--no-status", "--"])
        .arg(helper)
        .arg("23")
        .assert()
        .code(23);
}

#[test]
fn launches_child_when_service_is_unavailable() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt")
        .unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--no-status", "--"])
        .arg(helper)
        .args(["0", "still-ran"])
        .assert()
        .success()
        .stdout(predicate::str::contains("arg=still-ran"));
}

#[test]
fn unknown_executable_name_is_not_rendered() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt")
        .unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--"])
        .arg(helper)
        .arg("0")
        .assert()
        .success()
        .stderr(predicate::str::contains("smt [unknown] starting"))
        .stderr(predicate::str::contains("exit-with").not());
}

#[test]
fn configured_status_disable_suppresses_renderer_without_no_status() {
    let (_temp, helper) = build_helper();
    let config_temp = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    settings.presentation.status_enabled = false;
    save_atomic(&config_temp.path().join("settings.toml"), &settings).unwrap();

    Command::cargo_bin("smt")
        .unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--config-dir"])
        .arg(config_temp.path())
        .arg("--")
        .arg(helper)
        .arg("0")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn invalid_settings_warn_but_still_launch_the_child() {
    let (_temp, helper) = build_helper();
    let config_temp = tempfile::tempdir().unwrap();
    std::fs::write(config_temp.path().join("settings.toml"), "invalid = true\n").unwrap();

    Command::cargo_bin("smt")
        .unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--config-dir"])
        .arg(config_temp.path())
        .arg("--")
        .arg(helper)
        .args(["0", "still-ran"])
        .assert()
        .success()
        .stdout(predicate::str::contains("arg=still-ran"))
        .stderr(predicate::str::contains("settings unavailable"));
}

#[tokio::test]
async fn disabled_resource_diagnostics_emit_lifecycle_without_metric_samples() {
    let (_temp, helper) = build_helper();
    let state = tempfile::tempdir().unwrap();
    let paths = AppPaths {
        config_dir: state.path().join("config"),
        runtime_dir: state.path().join("runtime"),
        data_dir: state.path().join("data"),
    };
    let token = "secret";
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.token_file(), token).unwrap();

    let mut service_config = ServiceConfig::for_test(SecretString::from(token.to_owned()));
    service_config.discovery_file = Some(paths.discovery_file());
    service_config.database_file = Some(paths.database_file());
    let service = spawn_test_service(service_config).await.unwrap();

    let mut renderer = savemyterminal::renderer::PlainRenderer::stderr(false);
    let code = run_with_options(
        vec![helper.to_string_lossy().into_owned(), "0".to_owned()],
        &mut renderer,
        RunOptions {
            paths: paths.clone(),
            cpu_diagnostics: false,
            memory_diagnostics: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(code, 0);

    let history = SqliteStore::open(&paths.database_file())
        .unwrap()
        .history(10, 0)
        .unwrap();
    assert_eq!(history.sessions.len(), 1);
    assert_eq!(history.sessions[0].final_state, SessionState::Completed);
    assert_eq!(history.sessions[0].avg_cpu_percent, None);
    assert_eq!(history.sessions[0].avg_memory_bytes, None);
    service.shutdown().await;
}
