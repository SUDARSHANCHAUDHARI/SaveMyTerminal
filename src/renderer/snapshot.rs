use crate::protocol::{SessionSnapshot, SessionState};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotView {
    pub active_count: usize,
    pub agent_id: Option<String>,
    pub state: Option<SessionState>,
    pub label: String,
    pub color: String,
    pub intensity: u8,
}

impl SnapshotView {
    pub fn from_sessions(sessions: &[SessionSnapshot], intensity: u8) -> Self {
        let primary = sessions
            .iter()
            .max_by_key(|session| (session.updated_at_ms, session.session_id));
        let intensity = intensity.min(100);
        let Some(primary) = primary else {
            return Self {
                active_count: 0,
                agent_id: None,
                state: None,
                label: "smt idle".to_owned(),
                color: "#6b7280".to_owned(),
                intensity,
            };
        };
        let state_label = state_label(primary.state);
        let suffix = if sessions.len() > 1 {
            format!(" (+{})", sessions.len() - 1)
        } else {
            String::new()
        };
        Self {
            active_count: sessions.len(),
            agent_id: Some(primary.agent_id.clone()),
            state: Some(primary.state),
            label: format!("smt {} {state_label}{suffix}", primary.agent_id),
            color: state_color(primary.state).to_owned(),
            intensity,
        }
    }
}

fn state_label(state: SessionState) -> &'static str {
    match state {
        SessionState::Starting => "starting",
        SessionState::Thinking => "thinking",
        SessionState::ToolRunning => "tool",
        SessionState::Waiting => "waiting",
        SessionState::Completed => "completed",
        SessionState::Failed => "failed",
        SessionState::Interrupted => "interrupted",
    }
}

fn state_color(state: SessionState) -> &'static str {
    match state {
        SessionState::Starting => "#6366f1",
        SessionState::Thinking => "#8b5cf6",
        SessionState::ToolRunning => "#f59e0b",
        SessionState::Waiting => "#06b6d4",
        SessionState::Completed => "#22c55e",
        SessionState::Failed => "#ef4444",
        SessionState::Interrupted => "#6b7280",
    }
}
