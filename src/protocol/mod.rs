mod event;
mod session;

pub use event::{
    Event, EventKind, FailureCategory, Metric, MetricQuality, MetricSource, ProtocolError,
    ToolCategory,
};
pub use session::{SessionSnapshot, SessionState};

pub const PROTOCOL_VERSION: u16 = 1;
