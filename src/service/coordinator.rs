use crate::{
    protocol::{Event, SessionSnapshot},
    service::{RegistryError, SessionRegistry},
    storage::{AdapterKind, HistoryStore},
};
use std::time::Duration;
use tokio::sync::watch;

#[derive(Clone)]
pub struct SessionCoordinator {
    registry: SessionRegistry,
    history: HistoryStore,
    live_tx: watch::Sender<Vec<SessionSnapshot>>,
}

impl SessionCoordinator {
    pub fn new(registry: SessionRegistry, history: HistoryStore) -> Self {
        let (live_tx, _) = watch::channel(Vec::new());
        Self {
            registry,
            history,
            live_tx,
        }
    }

    pub async fn apply(&self, event: Event) -> Result<SessionSnapshot, RegistryError> {
        let kind = event.kind.clone();
        let adapter_kind = if event.adapter_id == "generic" {
            AdapterKind::Generic
        } else {
            AdapterKind::Native
        };
        let snapshot = self.registry.apply(event).await?;

        let history = self.history.clone();
        let persisted_snapshot = snapshot.clone();
        let _ = tokio::task::spawn_blocking(move || {
            history.record(&persisted_snapshot, &kind, adapter_kind)
        })
        .await;

        self.live_tx
            .send_replace(self.registry.active_sessions().await);
        Ok(snapshot)
    }

    pub fn subscribe(&self) -> watch::Receiver<Vec<SessionSnapshot>> {
        self.live_tx.subscribe()
    }

    pub async fn active_sessions(&self) -> Vec<SessionSnapshot> {
        self.registry.active_sessions().await
    }

    pub async fn idle_for(&self) -> Duration {
        self.registry.idle_for().await
    }

    pub fn history(&self) -> &HistoryStore {
        &self.history
    }

    pub fn history_store(&self) -> HistoryStore {
        self.history.clone()
    }
}
