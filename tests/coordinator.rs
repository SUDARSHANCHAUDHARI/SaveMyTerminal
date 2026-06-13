use savemyterminal::{
    protocol::{Event, EventKind, Metric, MetricQuality, MetricSource, SessionState},
    service::{SessionCoordinator, SessionRegistry},
    storage::{HistoryStore, SqliteStore},
};
use tempfile::TempDir;
use uuid::Uuid;

fn persistent_coordinator(temp: &TempDir) -> (SessionCoordinator, HistoryStore) {
    let history =
        HistoryStore::available(SqliteStore::open(&temp.path().join("history.sqlite3")).unwrap());
    (
        SessionCoordinator::new(SessionRegistry::default(), history.clone()),
        history,
    )
}

#[tokio::test]
async fn successful_transitions_are_persisted_and_published() {
    let temp = tempfile::tempdir().unwrap();
    let (coordinator, history) = persistent_coordinator(&temp);
    let mut live = coordinator.subscribe();
    let session_id = Uuid::new_v4();

    coordinator
        .apply(Event::new(
            session_id,
            "generic",
            "codex",
            EventKind::Started,
        ))
        .await
        .unwrap();
    live.changed().await.unwrap();
    assert_eq!(live.borrow().len(), 1);

    coordinator
        .apply(Event::new(
            session_id,
            "generic",
            "codex",
            EventKind::Completed { exit_code: 0 },
        ))
        .await
        .unwrap();
    live.changed().await.unwrap();
    assert!(live.borrow().is_empty());

    let page = history.history(10, 0).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.sessions[0].final_state, SessionState::Completed);
}

#[tokio::test]
async fn invalid_transitions_are_neither_persisted_nor_published() {
    let temp = tempfile::tempdir().unwrap();
    let (coordinator, history) = persistent_coordinator(&temp);
    let live = coordinator.subscribe();

    assert!(
        coordinator
            .apply(Event::new(
                Uuid::new_v4(),
                "generic",
                "unknown",
                EventKind::Waiting,
            ))
            .await
            .is_err()
    );

    assert!(!live.has_changed().unwrap());
    assert_eq!(history.history(10, 0).unwrap().total, 0);
}

#[tokio::test]
async fn persistence_failure_does_not_reject_valid_live_events() {
    let coordinator = SessionCoordinator::new(
        SessionRegistry::default(),
        HistoryStore::unavailable("database unavailable"),
    );
    let session_id = Uuid::new_v4();

    let snapshot = coordinator
        .apply(Event::new(
            session_id,
            "generic",
            "unknown",
            EventKind::Started,
        ))
        .await
        .unwrap();

    assert_eq!(snapshot.session_id, session_id);
    assert_eq!(coordinator.active_sessions().await.len(), 1);
}

#[tokio::test]
async fn metric_events_update_persisted_aggregates_without_changing_state() {
    let temp = tempfile::tempdir().unwrap();
    let (coordinator, history) = persistent_coordinator(&temp);
    let session_id = Uuid::new_v4();
    coordinator
        .apply(Event::new(
            session_id,
            "generic",
            "codex",
            EventKind::Started,
        ))
        .await
        .unwrap();
    coordinator
        .apply(Event::new(
            session_id,
            "generic",
            "codex",
            EventKind::Metrics {
                cpu_percent: Some(Metric::new(12.0, MetricQuality::Exact, MetricSource::Os)),
                memory_bytes: Some(Metric::new(4096, MetricQuality::Exact, MetricSource::Os)),
            },
        ))
        .await
        .unwrap();
    let snapshot = coordinator
        .apply(Event::new(
            session_id,
            "generic",
            "codex",
            EventKind::Completed { exit_code: 0 },
        ))
        .await
        .unwrap();

    assert_eq!(snapshot.state, SessionState::Completed);
    let summary = &history.history(10, 0).unwrap().sessions[0];
    assert_eq!(summary.avg_cpu_percent, Some(12.0));
    assert_eq!(summary.avg_memory_bytes, Some(4096));
}
