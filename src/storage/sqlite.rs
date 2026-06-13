use crate::{
    protocol::{EventKind, FailureCategory, SessionSnapshot, SessionState},
    storage::{AdapterKind, HistoryPage, SessionSummary},
};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use std::{path::Path, sync::Mutex, time::Duration};
use uuid::Uuid;

const SCHEMA_VERSION: i64 = 1;

pub struct SqliteStore {
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    Active,
    Missing,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_millis(500))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn schema_version(&self) -> Result<i64> {
        let connection = self.connection()?;
        Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    pub fn record(
        &self,
        snapshot: &SessionSnapshot,
        event: &EventKind,
        adapter_kind: AdapterKind,
    ) -> Result<()> {
        let connection = self.connection()?;
        let updated_at_ms = to_i64(snapshot.updated_at_ms)?;
        match event {
            EventKind::Started => {
                connection.execute(
                    "INSERT INTO sessions (
                        session_id, agent_id, adapter_id, adapter_kind,
                        started_at_ms, updated_at_ms, final_state, transition_count
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
                    params![
                        snapshot.session_id.to_string(),
                        snapshot.agent_id,
                        snapshot.adapter_id,
                        adapter_kind_name(adapter_kind),
                        to_i64(snapshot.started_at_ms)?,
                        updated_at_ms,
                        state_name(snapshot.state),
                    ],
                )?;
            }
            EventKind::Metrics {
                cpu_percent,
                memory_bytes,
            } => {
                let cpu = cpu_percent.as_ref().map(|metric| f64::from(metric.value));
                let memory = memory_bytes
                    .as_ref()
                    .map(|metric| to_i64(metric.value))
                    .transpose()?;
                connection.execute(
                    "UPDATE sessions SET
                        updated_at_ms = ?2,
                        cpu_sample_count = cpu_sample_count + CASE WHEN ?3 IS NULL THEN 0 ELSE 1 END,
                        cpu_sum = cpu_sum + COALESCE(?3, 0),
                        peak_cpu_percent = CASE
                            WHEN ?3 IS NULL THEN peak_cpu_percent
                            WHEN peak_cpu_percent IS NULL OR ?3 > peak_cpu_percent THEN ?3
                            ELSE peak_cpu_percent
                        END,
                        memory_sample_count = memory_sample_count + CASE WHEN ?4 IS NULL THEN 0 ELSE 1 END,
                        memory_sum = memory_sum + COALESCE(?4, 0),
                        peak_memory_bytes = CASE
                            WHEN ?4 IS NULL THEN peak_memory_bytes
                            WHEN peak_memory_bytes IS NULL OR ?4 > peak_memory_bytes THEN ?4
                            ELSE peak_memory_bytes
                        END
                    WHERE session_id = ?1 AND finalized = 0",
                    params![snapshot.session_id.to_string(), updated_at_ms, cpu, memory],
                )?;
            }
            EventKind::Completed { exit_code } => {
                finalize(&connection, snapshot, updated_at_ms, Some(*exit_code), None)?;
            }
            EventKind::Failed {
                exit_code,
                category,
            } => {
                finalize(
                    &connection,
                    snapshot,
                    updated_at_ms,
                    Some(*exit_code),
                    Some(failure_name(*category)),
                )?;
            }
            EventKind::Interrupted => {
                finalize(&connection, snapshot, updated_at_ms, None, None)?;
            }
            EventKind::Thinking | EventKind::Waiting | EventKind::ToolRunning { .. } => {
                connection.execute(
                    "UPDATE sessions SET
                        updated_at_ms = ?2,
                        final_state = ?3,
                        transition_count = transition_count + 1,
                        tool_event_count = tool_event_count + ?4
                    WHERE session_id = ?1 AND finalized = 0",
                    params![
                        snapshot.session_id.to_string(),
                        updated_at_ms,
                        state_name(snapshot.state),
                        i64::from(matches!(event, EventKind::ToolRunning { .. })),
                    ],
                )?;
            }
        }
        Ok(())
    }

    pub fn history(&self, limit: u32, offset: u32) -> Result<HistoryPage> {
        let connection = self.connection()?;
        let total: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sessions WHERE finalized = 1",
            [],
            |row| row.get(0),
        )?;
        let mut statement = connection.prepare(
            "SELECT
                session_id, agent_id, adapter_id, renderer_id, adapter_kind,
                started_at_ms, ended_at_ms, duration_ms, final_state,
                failure_category, exit_code, transition_count, tool_event_count,
                CASE WHEN cpu_sample_count = 0 THEN NULL ELSE cpu_sum / cpu_sample_count END,
                peak_cpu_percent,
                CASE WHEN memory_sample_count = 0 THEN NULL ELSE memory_sum / memory_sample_count END,
                peak_memory_bytes
             FROM sessions
             WHERE finalized = 1
             ORDER BY ended_at_ms DESC, session_id ASC
             LIMIT ?1 OFFSET ?2",
        )?;
        let sessions = statement
            .query_map(params![limit, offset], summary_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(HistoryPage {
            sessions,
            total: u64::try_from(total).context("negative history count")?,
            limit,
            offset,
        })
    }

    pub fn delete_finalized(&self, session_id: Uuid) -> Result<DeleteOutcome> {
        let connection = self.connection()?;
        let finalized = connection
            .query_row(
                "SELECT finalized FROM sessions WHERE session_id = ?1",
                [session_id.to_string()],
                |row| row.get::<_, bool>(0),
            )
            .optional()?;
        match finalized {
            None => Ok(DeleteOutcome::Missing),
            Some(false) => Ok(DeleteOutcome::Active),
            Some(true) => {
                connection.execute(
                    "DELETE FROM sessions WHERE session_id = ?1 AND finalized = 1",
                    [session_id.to_string()],
                )?;
                Ok(DeleteOutcome::Deleted)
            }
        }
    }

    pub fn purge_finalized(&self) -> Result<usize> {
        let connection = self.connection()?;
        Ok(connection.execute("DELETE FROM sessions WHERE finalized = 1", [])?)
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| anyhow::anyhow!("SQLite connection lock was poisoned"))
    }
}

fn migrate(connection: &mut Connection) -> Result<()> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        bail!("database schema version {version} is newer than supported version {SCHEMA_VERSION}");
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE sessions (
            session_id TEXT PRIMARY KEY NOT NULL,
            agent_id TEXT NOT NULL,
            adapter_id TEXT NOT NULL,
            renderer_id TEXT,
            adapter_kind TEXT NOT NULL,
            started_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            ended_at_ms INTEGER,
            duration_ms INTEGER,
            final_state TEXT NOT NULL,
            failure_category TEXT,
            exit_code INTEGER,
            transition_count INTEGER NOT NULL DEFAULT 0,
            tool_event_count INTEGER NOT NULL DEFAULT 0,
            cpu_sample_count INTEGER NOT NULL DEFAULT 0,
            cpu_sum REAL NOT NULL DEFAULT 0,
            peak_cpu_percent REAL,
            memory_sample_count INTEGER NOT NULL DEFAULT 0,
            memory_sum INTEGER NOT NULL DEFAULT 0,
            peak_memory_bytes INTEGER,
            context_peak REAL,
            context_final REAL,
            context_quality TEXT,
            context_source TEXT,
            finalized INTEGER NOT NULL DEFAULT 0 CHECK(finalized IN (0, 1))
        );
        CREATE INDEX sessions_history_idx
        ON sessions(finalized, ended_at_ms DESC);",
    )?;
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    transaction.commit()?;
    Ok(())
}

fn finalize(
    connection: &Connection,
    snapshot: &SessionSnapshot,
    ended_at_ms: i64,
    exit_code: Option<i32>,
    failure_category: Option<&str>,
) -> Result<()> {
    let duration_ms = snapshot
        .updated_at_ms
        .saturating_sub(snapshot.started_at_ms);
    connection.execute(
        "UPDATE sessions SET
            updated_at_ms = ?2,
            ended_at_ms = ?2,
            duration_ms = ?3,
            final_state = ?4,
            failure_category = ?5,
            exit_code = ?6,
            transition_count = transition_count + 1,
            finalized = 1
         WHERE session_id = ?1 AND finalized = 0",
        params![
            snapshot.session_id.to_string(),
            ended_at_ms,
            to_i64(duration_ms)?,
            state_name(snapshot.state),
            failure_category,
            exit_code,
        ],
    )?;
    Ok(())
}

fn summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummary> {
    let session_id: String = row.get(0)?;
    let adapter_kind: String = row.get(4)?;
    let final_state: String = row.get(8)?;
    let failure_category: Option<String> = row.get(9)?;
    let failure_category = failure_category
        .as_deref()
        .map(|value| parse_failure(value).ok_or_else(invalid_sql_value))
        .transpose()?;
    Ok(SessionSummary {
        session_id: Uuid::parse_str(&session_id).map_err(to_sql_error)?,
        agent_id: row.get(1)?,
        adapter_id: row.get(2)?,
        renderer_id: row.get(3)?,
        adapter_kind: parse_adapter_kind(&adapter_kind).ok_or_else(invalid_sql_value)?,
        started_at_ms: row_u64(row, 5)?,
        ended_at_ms: row_u64(row, 6)?,
        duration_ms: row_u64(row, 7)?,
        final_state: parse_state(&final_state).ok_or_else(invalid_sql_value)?,
        failure_category,
        exit_code: row.get(10)?,
        transition_count: row_u64(row, 11)?,
        tool_event_count: row_u64(row, 12)?,
        avg_cpu_percent: row.get(13)?,
        peak_cpu_percent: row.get(14)?,
        avg_memory_bytes: row_optional_u64(row, 15)?,
        peak_memory_bytes: row_optional_u64(row, 16)?,
        context_peak: None,
        context_final: None,
    })
}

fn state_name(state: SessionState) -> &'static str {
    match state {
        SessionState::Starting => "starting",
        SessionState::Thinking => "thinking",
        SessionState::ToolRunning => "tool_running",
        SessionState::Waiting => "waiting",
        SessionState::Completed => "completed",
        SessionState::Failed => "failed",
        SessionState::Interrupted => "interrupted",
    }
}

fn parse_state(value: &str) -> Option<SessionState> {
    match value {
        "starting" => Some(SessionState::Starting),
        "thinking" => Some(SessionState::Thinking),
        "tool_running" => Some(SessionState::ToolRunning),
        "waiting" => Some(SessionState::Waiting),
        "completed" => Some(SessionState::Completed),
        "failed" => Some(SessionState::Failed),
        "interrupted" => Some(SessionState::Interrupted),
        _ => None,
    }
}

fn adapter_kind_name(kind: AdapterKind) -> &'static str {
    match kind {
        AdapterKind::Generic => "generic",
        AdapterKind::Native => "native",
    }
}

fn parse_adapter_kind(value: &str) -> Option<AdapterKind> {
    match value {
        "generic" => Some(AdapterKind::Generic),
        "native" => Some(AdapterKind::Native),
        _ => None,
    }
}

fn failure_name(category: FailureCategory) -> &'static str {
    match category {
        FailureCategory::ProcessExit => "process_exit",
        FailureCategory::Launch => "launch",
        FailureCategory::Adapter => "adapter",
        FailureCategory::Protocol => "protocol",
        FailureCategory::Unknown => "unknown",
    }
}

fn parse_failure(value: &str) -> Option<FailureCategory> {
    match value {
        "process_exit" => Some(FailureCategory::ProcessExit),
        "launch" => Some(FailureCategory::Launch),
        "adapter" => Some(FailureCategory::Adapter),
        "protocol" => Some(FailureCategory::Protocol),
        "unknown" => Some(FailureCategory::Unknown),
        _ => None,
    }
}

fn to_i64(value: u64) -> Result<i64> {
    i64::try_from(value).context("value exceeds SQLite integer range")
}

fn to_sql_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn invalid_sql_value() -> rusqlite::Error {
    to_sql_error(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid persisted session value",
    ))
}

fn row_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(to_sql_error)
}

fn row_optional_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| u64::try_from(value).map_err(to_sql_error))
        .transpose()
}
