use savemyterminal::{
    paths::AppPaths,
    protocol::SessionState,
    storage::{AdapterKind, SessionSummary},
};
use std::{collections::BTreeSet, path::PathBuf};
use uuid::Uuid;

fn sample_summary() -> SessionSummary {
    SessionSummary {
        session_id: Uuid::nil(),
        agent_id: "codex".to_owned(),
        adapter_id: "generic".to_owned(),
        renderer_id: None,
        adapter_kind: AdapterKind::Generic,
        started_at_ms: 100,
        ended_at_ms: 250,
        duration_ms: 150,
        final_state: SessionState::Completed,
        failure_category: None,
        exit_code: Some(0),
        transition_count: 2,
        tool_event_count: 0,
        avg_cpu_percent: Some(4.5),
        peak_cpu_percent: Some(7.0),
        avg_memory_bytes: Some(1024),
        peak_memory_bytes: Some(2048),
        context_peak: None,
        context_final: None,
    }
}

#[test]
fn database_file_lives_under_the_data_directory() {
    let paths = AppPaths {
        config_dir: PathBuf::from("config"),
        runtime_dir: PathBuf::from("runtime"),
        data_dir: PathBuf::from("data"),
    };

    assert_eq!(
        paths.database_file(),
        PathBuf::from("data/sessions.sqlite3")
    );
}

#[test]
fn summary_json_contains_only_approved_fields() {
    let value = serde_json::to_value(sample_summary()).unwrap();
    let keys: BTreeSet<_> = value.as_object().unwrap().keys().cloned().collect();

    assert_eq!(
        keys,
        BTreeSet::from([
            "adapter_id".to_owned(),
            "adapter_kind".to_owned(),
            "agent_id".to_owned(),
            "avg_cpu_percent".to_owned(),
            "avg_memory_bytes".to_owned(),
            "context_final".to_owned(),
            "context_peak".to_owned(),
            "duration_ms".to_owned(),
            "ended_at_ms".to_owned(),
            "exit_code".to_owned(),
            "failure_category".to_owned(),
            "final_state".to_owned(),
            "peak_cpu_percent".to_owned(),
            "peak_memory_bytes".to_owned(),
            "renderer_id".to_owned(),
            "session_id".to_owned(),
            "started_at_ms".to_owned(),
            "tool_event_count".to_owned(),
            "transition_count".to_owned(),
        ])
    );
}
