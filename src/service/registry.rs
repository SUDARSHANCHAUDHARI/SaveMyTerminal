use crate::protocol::{Event, EventKind, SessionSnapshot, SessionState};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SessionRegistry {
    sessions: Arc<RwLock<HashMap<Uuid, SessionSnapshot>>>,
    last_activity: Arc<RwLock<Instant>>,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            last_activity: Arc::new(RwLock::new(Instant::now())),
        }
    }
}

impl SessionRegistry {
    pub async fn apply(&self, event: Event) -> Result<SessionSnapshot, RegistryError> {
        event.validate()?;
        *self.last_activity.write().await = Instant::now();
        let mut sessions = self.sessions.write().await;

        if matches!(&event.kind, EventKind::Started) {
            if sessions.contains_key(&event.session_id) {
                return Err(RegistryError::DuplicateSession);
            }

            let snapshot = SessionSnapshot {
                session_id: event.session_id,
                adapter_id: event.adapter_id,
                agent_id: event.agent_id,
                state: SessionState::Starting,
                started_at_ms: event.timestamp_ms,
                updated_at_ms: event.timestamp_ms,
                cpu_percent: None,
                memory_bytes: None,
            };
            sessions.insert(snapshot.session_id, snapshot.clone());
            return Ok(snapshot);
        }

        let snapshot = sessions
            .get_mut(&event.session_id)
            .ok_or(RegistryError::UnknownSession)?;
        if snapshot.state.is_terminal() {
            return Err(RegistryError::AlreadyFinished);
        }
        if snapshot.adapter_id != event.adapter_id || snapshot.agent_id != event.agent_id {
            return Err(RegistryError::IdentityMismatch);
        }

        match event.kind {
            EventKind::Started => unreachable!("started events return before transition handling"),
            EventKind::Thinking => snapshot.state = SessionState::Thinking,
            EventKind::ToolRunning { .. } => snapshot.state = SessionState::ToolRunning,
            EventKind::Waiting => snapshot.state = SessionState::Waiting,
            EventKind::Metrics {
                cpu_percent,
                memory_bytes,
            } => {
                if cpu_percent.is_some() {
                    snapshot.cpu_percent = cpu_percent;
                }
                if memory_bytes.is_some() {
                    snapshot.memory_bytes = memory_bytes;
                }
            }
            EventKind::Completed { .. } => snapshot.state = SessionState::Completed,
            EventKind::Failed { .. } => snapshot.state = SessionState::Failed,
            EventKind::Interrupted => snapshot.state = SessionState::Interrupted,
        }
        snapshot.updated_at_ms = event.timestamp_ms;
        Ok(snapshot.clone())
    }

    pub async fn get(&self, id: Uuid) -> Option<SessionSnapshot> {
        self.sessions.read().await.get(&id).cloned()
    }

    pub async fn active_count(&self) -> usize {
        self.sessions
            .read()
            .await
            .values()
            .filter(|session| !session.state.is_terminal())
            .count()
    }

    pub async fn idle_for(&self) -> Duration {
        self.last_activity.read().await.elapsed()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error(transparent)]
    Protocol(#[from] crate::protocol::ProtocolError),
    #[error("session already exists")]
    DuplicateSession,
    #[error("session does not exist")]
    UnknownSession,
    #[error("session already finished")]
    AlreadyFinished,
    #[error("event identity does not match session")]
    IdentityMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        EventKind, Metric, MetricQuality, MetricSource, PROTOCOL_VERSION, ProtocolError,
        SessionState,
    };

    #[tokio::test]
    async fn started_event_opens_a_session() {
        let registry = SessionRegistry::default();
        let id = Uuid::new_v4();

        registry
            .apply(Event::new(id, "generic", "unknown", EventKind::Started))
            .await
            .unwrap();

        assert_eq!(
            registry.get(id).await.unwrap().state,
            SessionState::Starting
        );
        assert_eq!(registry.active_count().await, 1);
    }

    #[tokio::test]
    async fn rejects_updates_after_a_terminal_state() {
        let registry = SessionRegistry::default();
        let id = Uuid::new_v4();
        registry
            .apply(Event::new(id, "generic", "unknown", EventKind::Started))
            .await
            .unwrap();
        registry
            .apply(Event::new(
                id,
                "generic",
                "unknown",
                EventKind::Completed { exit_code: 0 },
            ))
            .await
            .unwrap();

        let error = registry
            .apply(Event::new(id, "generic", "unknown", EventKind::Waiting))
            .await
            .unwrap_err();

        assert_eq!(error, RegistryError::AlreadyFinished);
        assert_eq!(registry.active_count().await, 0);
    }

    #[tokio::test]
    async fn metrics_update_values_without_changing_state() {
        let registry = SessionRegistry::default();
        let id = Uuid::new_v4();
        registry
            .apply(Event::new(id, "generic", "unknown", EventKind::Started))
            .await
            .unwrap();

        registry
            .apply(Event::new(
                id,
                "generic",
                "unknown",
                EventKind::Metrics {
                    cpu_percent: Some(Metric::new(7.0, MetricQuality::Exact, MetricSource::Os)),
                    memory_bytes: None,
                },
            ))
            .await
            .unwrap();

        let snapshot = registry.get(id).await.unwrap();
        assert_eq!(snapshot.state, SessionState::Starting);
        assert_eq!(snapshot.cpu_percent.unwrap().value, 7.0);
    }

    #[tokio::test]
    async fn rejects_identity_changes() {
        let registry = SessionRegistry::default();
        let id = Uuid::new_v4();
        registry
            .apply(Event::new(id, "generic", "unknown", EventKind::Started))
            .await
            .unwrap();

        let error = registry
            .apply(Event::new(id, "native", "unknown", EventKind::Thinking))
            .await
            .unwrap_err();

        assert_eq!(error, RegistryError::IdentityMismatch);
    }

    #[tokio::test]
    async fn rejects_duplicate_session_start() {
        let registry = SessionRegistry::default();
        let id = Uuid::new_v4();
        let started = || Event::new(id, "generic", "unknown", EventKind::Started);
        registry.apply(started()).await.unwrap();

        let error = registry.apply(started()).await.unwrap_err();

        assert_eq!(error, RegistryError::DuplicateSession);
    }

    #[tokio::test]
    async fn rejects_event_for_unknown_session() {
        let registry = SessionRegistry::default();

        let error = registry
            .apply(Event::new(
                Uuid::new_v4(),
                "generic",
                "unknown",
                EventKind::Thinking,
            ))
            .await
            .unwrap_err();

        assert_eq!(error, RegistryError::UnknownSession);
    }

    #[tokio::test]
    async fn validates_before_mutating_registry() {
        let registry = SessionRegistry::default();
        let id = Uuid::new_v4();
        let mut event = Event::new(id, "generic", "unknown", EventKind::Started);
        event.protocol_version = PROTOCOL_VERSION + 1;

        let error = registry.apply(event).await.unwrap_err();

        assert_eq!(
            error,
            RegistryError::Protocol(ProtocolError::UnsupportedVersion)
        );
        assert!(registry.get(id).await.is_none());
    }

    #[tokio::test]
    async fn activity_timestamp_advances_when_an_event_is_applied() {
        let registry = SessionRegistry::default();
        tokio::time::sleep(Duration::from_millis(5)).await;
        let before = registry.idle_for().await;

        registry
            .apply(Event::new(
                Uuid::new_v4(),
                "generic",
                "unknown",
                EventKind::Started,
            ))
            .await
            .unwrap();

        assert!(registry.idle_for().await < before);
    }
}
