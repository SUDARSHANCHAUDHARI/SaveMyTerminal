use savemyterminal::{
    protocol::{Metric, MetricQuality, MetricSource, SessionSnapshot, SessionState},
    renderer::SnapshotView,
};
use uuid::Uuid;

fn session(agent: &str, state: SessionState, updated_at_ms: u64) -> SessionSnapshot {
    SessionSnapshot {
        session_id: Uuid::new_v4(),
        adapter_id: "native".to_owned(),
        agent_id: agent.to_owned(),
        state,
        started_at_ms: 1,
        updated_at_ms,
        cpu_percent: None,
        memory_bytes: None,
        context_pressure: None,
    }
}

#[test]
fn snapshot_command_reports_idle_without_creating_service_state() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config");
    let runtime = temp.path().join("runtime");
    let data = temp.path().join("data");
    assert_cmd::Command::cargo_bin("smt")
        .unwrap()
        .args([
            "snapshot",
            "--config-dir",
            config.to_str().unwrap(),
            "--runtime-dir",
            runtime.to_str().unwrap(),
            "--data-dir",
            data.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"label\":\"smt idle\""));
    assert!(!config.join("auth.token").exists());
    assert!(!runtime.join("service.json").exists());
}

#[test]
fn renderer_accepts_every_normalized_state() {
    for state in [
        SessionState::Starting,
        SessionState::Thinking,
        SessionState::ToolRunning,
        SessionState::Waiting,
        SessionState::Completed,
        SessionState::Failed,
        SessionState::Interrupted,
    ] {
        let view = SnapshotView::from_sessions(&[session("codex", state, 1)], 60);
        assert_eq!(view.state, Some(state));
        assert!(view.label.starts_with("smt codex "));
        assert!(view.color.starts_with('#'));
    }
}

#[test]
fn newest_session_is_primary_and_multiple_sessions_are_counted() {
    let view = SnapshotView::from_sessions(
        &[
            session("claude", SessionState::Waiting, 10),
            session("gemini", SessionState::ToolRunning, 20),
        ],
        60,
    );
    assert_eq!(view.active_count, 2);
    assert_eq!(view.agent_id.as_deref(), Some("gemini"));
    assert_eq!(view.label, "smt gemini tool (+1)");
}

#[test]
fn unrelated_native_session_does_not_hijack_newer_wrapper_view() {
    let mut native = session("codex", SessionState::ToolRunning, 20);
    native.adapter_id = "codex-hooks".to_owned();
    native.context_pressure = Some(Metric::new(72.5, MetricQuality::Exact, MetricSource::Agent));
    let mut wrapper = session("codex", SessionState::Starting, 30);
    wrapper.adapter_id = "generic".to_owned();

    let view = SnapshotView::from_sessions(&[native, wrapper], 60);

    assert_eq!(view.agent_id.as_deref(), Some("codex"));
    assert_eq!(view.state, Some(SessionState::Starting));
    assert!(view.context_pressure.is_none());
}

#[test]
fn snapshot_exposes_context_pressure_honestly_when_available() {
    let mut active = session("claude", SessionState::Thinking, 10);
    active.context_pressure = Some(Metric::new(
        41.0,
        MetricQuality::Estimated,
        MetricSource::Heuristic,
    ));

    let view = SnapshotView::from_sessions(&[active], 60);

    assert_eq!(
        view.context_pressure.unwrap().quality,
        MetricQuality::Estimated
    );
}

#[test]
fn idle_and_json_views_contain_only_renderer_metadata() {
    let view = SnapshotView::from_sessions(&[], 200);
    assert_eq!(view.label, "smt idle");
    assert_eq!(view.intensity, 100);
    let encoded = serde_json::to_string(&view).unwrap();
    for prohibited in [
        "prompt",
        "response",
        "command",
        "cwd",
        "path",
        "environment",
    ] {
        assert!(!encoded.contains(prohibited));
    }
}
