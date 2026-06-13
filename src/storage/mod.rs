mod model;
mod sqlite;

pub use model::{AdapterKind, HistoryPage, HistoryStats, SessionSummary};
pub use sqlite::{DeleteOutcome, SqliteStore};

use crate::protocol::{EventKind, SessionSnapshot};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone)]
pub enum HistoryStore {
    Available(Arc<SqliteStore>),
    Unavailable(Arc<str>),
}

#[derive(Debug, Error)]
#[error("session history is unavailable")]
pub struct HistoryUnavailable;

impl HistoryStore {
    pub fn available(store: SqliteStore) -> Self {
        Self::Available(Arc::new(store))
    }

    pub fn unavailable(reason: impl Into<Arc<str>>) -> Self {
        Self::Unavailable(reason.into())
    }

    pub fn unavailable_reason(&self) -> Option<&str> {
        match self {
            Self::Available(_) => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }

    pub fn record(
        &self,
        snapshot: &SessionSnapshot,
        event: &EventKind,
        adapter_kind: AdapterKind,
    ) -> Result<(), HistoryUnavailable> {
        self.store()?
            .record(snapshot, event, adapter_kind)
            .map_err(|_| HistoryUnavailable)
    }

    pub fn history(&self, limit: u32, offset: u32) -> Result<HistoryPage, HistoryUnavailable> {
        self.store()?
            .history(limit, offset)
            .map_err(|_| HistoryUnavailable)
    }

    pub fn stats(&self) -> Result<HistoryStats, HistoryUnavailable> {
        self.store()?.stats().map_err(|_| HistoryUnavailable)
    }

    pub fn delete_finalized(&self, session_id: Uuid) -> Result<DeleteOutcome, HistoryUnavailable> {
        self.store()?
            .delete_finalized(session_id)
            .map_err(|_| HistoryUnavailable)
    }

    pub fn purge_finalized(&self) -> Result<usize, HistoryUnavailable> {
        self.store()?
            .purge_finalized()
            .map_err(|_| HistoryUnavailable)
    }

    pub fn cleanup_before(&self, cutoff_ms: u64) -> Result<usize, HistoryUnavailable> {
        self.store()?
            .cleanup_before(cutoff_ms)
            .map_err(|_| HistoryUnavailable)
    }

    fn store(&self) -> Result<&SqliteStore, HistoryUnavailable> {
        match self {
            Self::Available(store) => Ok(store),
            Self::Unavailable(_) => Err(HistoryUnavailable),
        }
    }
}
