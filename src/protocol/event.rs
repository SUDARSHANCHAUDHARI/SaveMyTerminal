use crate::protocol::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricQuality {
    Exact,
    Estimated,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricSource {
    Agent,
    Os,
    Heuristic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    Shell,
    FileRead,
    FileWrite,
    Search,
    Network,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    ProcessExit,
    Launch,
    Adapter,
    Protocol,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metric<T> {
    pub value: T,
    pub quality: MetricQuality,
    pub source: MetricSource,
}

impl<T> Metric<T> {
    pub fn new(value: T, quality: MetricQuality, source: MetricSource) -> Self {
        Self {
            value,
            quality,
            source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    Started,
    Thinking,
    ToolRunning {
        category: ToolCategory,
    },
    Waiting,
    Metrics {
        #[serde(skip_serializing_if = "Option::is_none")]
        cpu_percent: Option<Metric<f32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        memory_bytes: Option<Metric<u64>>,
    },
    Completed {
        exit_code: i32,
    },
    Failed {
        exit_code: i32,
        category: FailureCategory,
    },
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub protocol_version: u16,
    pub event_id: Uuid,
    pub session_id: Uuid,
    pub timestamp_ms: u64,
    pub adapter_id: String,
    pub agent_id: String,
    pub kind: EventKind,
}

impl Event {
    pub fn new(
        session_id: Uuid,
        adapter_id: impl Into<String>,
        agent_id: impl Into<String>,
        kind: EventKind,
    ) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            protocol_version: PROTOCOL_VERSION,
            event_id: Uuid::new_v4(),
            session_id,
            timestamp_ms,
            adapter_id: adapter_id.into(),
            agent_id: agent_id.into(),
            kind,
        }
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if self.adapter_id.is_empty() || self.agent_id.is_empty() {
            return Err(ProtocolError::EmptyIdentifier);
        }
        if self.adapter_id.len() > 64 || self.agent_id.len() > 64 {
            return Err(ProtocolError::IdentifierTooLong);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("unsupported protocol version")]
    UnsupportedVersion,
    #[error("adapter and agent identifiers must not be empty")]
    EmptyIdentifier,
    #[error("adapter and agent identifiers must not exceed 64 bytes")]
    IdentifierTooLong,
}
