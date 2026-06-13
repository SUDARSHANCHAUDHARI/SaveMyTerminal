use savemyterminal::{
    paths::AppPaths,
    protocol::{
        EventKind, FailureCategory, Metric, MetricQuality, MetricSource, SessionSnapshot,
        SessionState,
    },
    storage::{AdapterKind, DeleteOutcome, HistoryStore, SessionSummary, SqliteStore},
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

#[test]
fn recovery_finalizes_unfinished_sessions_as_interrupted() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.sqlite3");
    let session_id = Uuid::new_v4();
    let store = SqliteStore::open(&path).unwrap();
    let started = started_snapshot(session_id, "codex", 100);
    store
        .record(&started, &EventKind::Started, AdapterKind::Generic)
        .unwrap();
    let mut waiting = started;
    waiting.state = SessionState::Waiting;
    waiting.updated_at_ms = 250;
    store
        .record(&waiting, &EventKind::Waiting, AdapterKind::Generic)
        .unwrap();
    drop(store);

    let reopened = SqliteStore::open(&path).unwrap();
    assert_eq!(reopened.recover_interrupted().unwrap(), 1);
    let summary = &reopened.history(10, 0).unwrap().sessions[0];

    assert_eq!(summary.final_state, SessionState::Interrupted);
    assert_eq!(summary.ended_at_ms, 250);
    assert_eq!(summary.duration_ms, 150);
}

#[test]
fn retention_removes_only_finalized_sessions_older_than_cutoff() {
    let temp = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(&temp.path().join("history.sqlite3")).unwrap();
    let old_id = Uuid::new_v4();
    let boundary_id = Uuid::new_v4();
    let active_id = Uuid::new_v4();

    for (session_id, end) in [
        (old_id, Some(199)),
        (boundary_id, Some(200)),
        (active_id, None),
    ] {
        let started = started_snapshot(session_id, "unknown", 100);
        store
            .record(&started, &EventKind::Started, AdapterKind::Generic)
            .unwrap();
        if let Some(end) = end {
            let mut completed = started;
            completed.state = SessionState::Completed;
            completed.updated_at_ms = end;
            store
                .record(
                    &completed,
                    &EventKind::Completed { exit_code: 0 },
                    AdapterKind::Generic,
                )
                .unwrap();
        }
    }

    assert_eq!(store.cleanup_before(200).unwrap(), 1);
    let history = store.history(10, 0).unwrap();
    assert_eq!(history.total, 1);
    assert_eq!(history.sessions[0].session_id, boundary_id);
    assert_eq!(
        store.delete_finalized(active_id).unwrap(),
        DeleteOutcome::Active
    );
}

#[test]
fn stats_aggregate_duration_states_and_resource_samples() {
    let temp = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(&temp.path().join("history.sqlite3")).unwrap();
    let session_id = Uuid::new_v4();
    let mut snapshot = started_snapshot(session_id, "codex", 100);
    store
        .record(&snapshot, &EventKind::Started, AdapterKind::Generic)
        .unwrap();
    for (cpu, memory, timestamp) in [(10.0, 100, 120), (20.0, 300, 140)] {
        snapshot.updated_at_ms = timestamp;
        store
            .record(
                &snapshot,
                &EventKind::Metrics {
                    cpu_percent: Some(Metric::new(cpu, MetricQuality::Exact, MetricSource::Os)),
                    memory_bytes: Some(Metric::new(memory, MetricQuality::Exact, MetricSource::Os)),
                },
                AdapterKind::Generic,
            )
            .unwrap();
    }
    snapshot.state = SessionState::Completed;
    snapshot.updated_at_ms = 200;
    store
        .record(
            &snapshot,
            &EventKind::Completed { exit_code: 0 },
            AdapterKind::Generic,
        )
        .unwrap();

    let stats = store.stats().unwrap();

    assert_eq!(stats.session_count, 1);
    assert_eq!(stats.total_duration_ms, 100);
    assert_eq!(stats.average_duration_ms, Some(100));
    assert_eq!(stats.states.get("completed"), Some(&1));
    assert_eq!(stats.average_cpu_percent, Some(15.0));
    assert_eq!(stats.peak_cpu_percent, Some(20.0));
    assert_eq!(stats.average_memory_bytes, Some(200));
    assert_eq!(stats.peak_memory_bytes, Some(300));
}

#[test]
fn unsupported_newer_schema_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.sqlite3");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);

    assert!(SqliteStore::open(&path).is_err());
}

#[test]
fn unavailable_history_store_reports_a_stable_error() {
    let store = HistoryStore::unavailable("migration failed");

    assert_eq!(
        store.history(10, 0).unwrap_err().to_string(),
        "session history is unavailable"
    );
}
