mod model;
mod sqlite;

pub use model::{AdapterKind, HistoryPage, HistoryStats, SessionSummary};
pub use sqlite::{DeleteOutcome, SqliteStore};
