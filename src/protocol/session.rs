use crate::protocol::{Metric, MetricQuality, MetricSource};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Starting,
    Thinking,
    ToolRunning,
    Waiting,
    Completed,
    Failed,
    Interrupted,
}

impl SessionState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Interrupted)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: Uuid,
    pub adapter_id: String,
    pub agent_id: String,
    pub state: SessionState,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    pub cpu_percent: Option<Metric<f32>>,
    pub memory_bytes: Option<Metric<u64>>,
}

impl SessionSnapshot {
    pub fn unavailable_metric<T: Default>(source: MetricSource) -> Metric<T> {
        Metric::new(T::default(), MetricQuality::Unavailable, source)
    }
}
