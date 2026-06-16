use assert_cmd::Command;
use predicates::prelude::*;
use savemyterminal::config::{Settings, save_atomic};
use savemyterminal::{
    client::ServiceClient,
    paths::AppPaths,
    protocol::{Event, EventKind, SessionState},
    renderer::{Renderer, SnapshotView},
    runner::{RunOptions, run_with_options},
    service::{ServiceConfig, spawn_test_service},
    storage::SqliteStore,
};
use secrecy::SecretString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Default)]
struct RecordingRenderer {
    states: Arc<Mutex<Vec<SessionState>>>,
}

impl Renderer for RecordingRenderer {
    fn started(&mut self, _agent_id: &str) {}
    fn finished(&mut self, _agent_id: &str, _exit_code: i32) {}
    fn warning(&mut self, _message: &str) {}

    fn snapshot(&mut self, view: &SnapshotView) {
        if let Some(state) = view.state {
            self.states.lock().unwrap().push(state);
        }
    }
}

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
            ambient_intensity: 60,
            session_id: None,
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

#[tokio::test]
async fn attached_wrapper_relays_native_state_snapshots_to_the_renderer() {
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
    let service = spawn_test_service(service_config).await.unwrap();
    let client = ServiceClient::connect(&paths).await.unwrap();
    let session_id = Uuid::new_v4();
    let unrelated_id = Uuid::new_v4();
    let events = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        client
            .send(&Event::new(
                session_id,
                "generic",
                "unknown",
                EventKind::Thinking,
            ))
            .await
            .unwrap();
        client
            .send(&Event::new(
                unrelated_id,
                "codex-hooks",
                "codex",
                EventKind::Started,
            ))
            .await
            .unwrap();
        client
            .send(&Event::new(
                unrelated_id,
                "codex-hooks",
                "codex",
                EventKind::ToolRunning {
                    category: savemyterminal::protocol::ToolCategory::Shell,
                },
            ))
            .await
            .unwrap();
    });

    let mut renderer = RecordingRenderer::default();
    let recorded = renderer.states.clone();
    let code = run_with_options(
        vec![
            helper.to_string_lossy().into_owned(),
            "0".to_owned(),
            "sleep=900".to_owned(),
        ],
        &mut renderer,
        RunOptions {
            paths,
            cpu_diagnostics: true,
            memory_diagnostics: true,
            ambient_intensity: 60,
            session_id: Some(session_id),
        },
    )
    .await
    .unwrap();
    events.await.unwrap();

    assert_eq!(code, 0);
    {
        let states = recorded.lock().unwrap();
        assert!(states.contains(&SessionState::Thinking));
        assert!(!states.contains(&SessionState::ToolRunning));
    }
    service.shutdown().await;
}
