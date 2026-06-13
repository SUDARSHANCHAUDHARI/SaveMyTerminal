use crate::protocol::{FailureCategory, Metric, SessionState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    Generic,
    Native,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSummary {
    pub session_id: Uuid,
    pub agent_id: String,
    pub adapter_id: String,
    pub renderer_id: Option<String>,
    pub adapter_kind: AdapterKind,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub duration_ms: u64,
    pub final_state: SessionState,
    pub failure_category: Option<FailureCategory>,
    pub exit_code: Option<i32>,
    pub transition_count: u64,
    pub tool_event_count: u64,
    pub avg_cpu_percent: Option<f32>,
    pub peak_cpu_percent: Option<f32>,
    pub avg_memory_bytes: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
    pub context_peak: Option<Metric<f32>>,
    pub context_final: Option<Metric<f32>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryPage {
    pub sessions: Vec<SessionSummary>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryStats {
    pub session_count: u64,
    pub total_duration_ms: u64,
    pub average_duration_ms: Option<u64>,
    pub states: BTreeMap<String, u64>,
    pub average_cpu_percent: Option<f32>,
    pub peak_cpu_percent: Option<f32>,
    pub average_memory_bytes: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
    pub context_peak: Option<Metric<f32>>,
}
