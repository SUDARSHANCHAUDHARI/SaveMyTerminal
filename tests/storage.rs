use savemyterminal::{
    paths::AppPaths,
    protocol::{EventKind, FailureCategory, SessionSnapshot, SessionState},
    storage::{AdapterKind, DeleteOutcome, SessionSummary, SqliteStore},
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

fn started_snapshot(session_id: Uuid, agent_id: &str, timestamp_ms: u64) -> SessionSnapshot {
    SessionSnapshot {
        session_id,
        adapter_id: "generic".to_owned(),
        agent_id: agent_id.to_owned(),
        state: SessionState::Starting,
        started_at_ms: timestamp_ms,
        updated_at_ms: timestamp_ms,
        cpu_percent: None,
        memory_bytes: None,
    }
}

#[test]
fn sqlite_migration_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.sqlite3");

    assert_eq!(
        SqliteStore::open(&path).unwrap().schema_version().unwrap(),
        1
    );
    assert_eq!(
        SqliteStore::open(&path).unwrap().schema_version().unwrap(),
        1
    );
}

#[test]
fn sqlite_records_and_lists_finalized_summaries_newest_first() {
    let temp = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(&temp.path().join("history.sqlite3")).unwrap();
    let older_id = Uuid::new_v4();
    let newer_id = Uuid::new_v4();

    for (session_id, agent_id, started_at_ms, ended_at_ms) in [
        (older_id, "agent'one", 100, 200),
        (newer_id, "agent-two", 300, 500),
    ] {
        let started = started_snapshot(session_id, agent_id, started_at_ms);
        store
            .record(&started, &EventKind::Started, AdapterKind::Generic)
            .unwrap();
        let mut completed = started;
        completed.state = SessionState::Completed;
        completed.updated_at_ms = ended_at_ms;
        store
            .record(
                &completed,
                &EventKind::Completed { exit_code: 0 },
                AdapterKind::Generic,
            )
            .unwrap();
    }

    let history = store.history(50, 0).unwrap();

    assert_eq!(history.total, 2);
    assert_eq!(history.sessions[0].session_id, newer_id);
    assert_eq!(history.sessions[1].agent_id, "agent'one");
    assert_eq!(history.sessions[1].duration_ms, 100);
}

#[test]
fn sqlite_delete_rejects_active_rows_and_removes_finalized_rows() {
    let temp = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(&temp.path().join("history.sqlite3")).unwrap();
    let session_id = Uuid::new_v4();
    let started = started_snapshot(session_id, "codex", 100);
    store
        .record(&started, &EventKind::Started, AdapterKind::Generic)
        .unwrap();

    assert_eq!(
        store.delete_finalized(session_id).unwrap(),
        DeleteOutcome::Active
    );

    let mut failed = started;
    failed.state = SessionState::Failed;
    failed.updated_at_ms = 250;
    store
        .record(
            &failed,
            &EventKind::Failed {
                exit_code: 17,
                category: FailureCategory::ProcessExit,
            },
            AdapterKind::Generic,
        )
        .unwrap();

    assert_eq!(
        store.delete_finalized(session_id).unwrap(),
        DeleteOutcome::Deleted
    );
    assert_eq!(
        store.delete_finalized(session_id).unwrap(),
        DeleteOutcome::Missing
    );
}

#[test]
fn sqlite_purge_removes_only_finalized_rows() {
    let temp = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(&temp.path().join("history.sqlite3")).unwrap();
    let active_id = Uuid::new_v4();
    let completed_id = Uuid::new_v4();

    for session_id in [active_id, completed_id] {
        let started = started_snapshot(session_id, "unknown", 100);
        store
            .record(&started, &EventKind::Started, AdapterKind::Generic)
            .unwrap();
        if session_id == completed_id {
            let mut completed = started;
            completed.state = SessionState::Completed;
            completed.updated_at_ms = 150;
            store
                .record(
                    &completed,
                    &EventKind::Completed { exit_code: 0 },
                    AdapterKind::Generic,
                )
                .unwrap();
        }
    }

    assert_eq!(store.purge_finalized().unwrap(), 1);
    assert_eq!(
        store.delete_finalized(active_id).unwrap(),
        DeleteOutcome::Active
    );
    assert_eq!(store.history(50, 0).unwrap().total, 0);
}
