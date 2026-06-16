use crate::protocol::{Event, EventKind, Metric, MetricQuality, MetricSource, ToolCategory};
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

pub const MAX_HOOK_INPUT_BYTES: usize = 64 * 1024;
pub const ATTACHED_SESSION_ENV: &str = "SMT_ATTACHED_SESSION_ID";
const SESSION_NAMESPACE: Uuid = Uuid::from_u128(0x35de3ca4_54e8_4b76_8a64_eb320718cab4);

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum NativeAgent {
    Codex,
    Claude,
    Gemini,
}

impl NativeAgent {
    pub fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
        }
    }

    fn adapter_id(self) -> &'static str {
        match self {
            Self::Codex => "codex-hooks",
            Self::Claude => "claude-hooks",
            Self::Gemini => "gemini-hooks",
        }
    }
}

#[derive(Debug, Deserialize)]
struct HookEnvelope {
    session_id: String,
    hook_event_name: String,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    context_percent: Option<f32>,
    #[serde(default)]
    context_window: Option<ContextWindow>,
}

#[derive(Debug, Deserialize)]
struct ContextWindow {
    #[serde(alias = "used", alias = "current_tokens")]
    used_tokens: u64,
    #[serde(alias = "limit", alias = "context_window_size")]
    max_tokens: u64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdapterError {
    #[error("hook input exceeds the size limit")]
    InputTooLarge,
    #[error("hook input is not valid metadata JSON")]
    InvalidJson,
    #[error("hook metadata is invalid")]
    InvalidMetadata,
}

pub fn map_hook(agent: NativeAgent, input: &[u8]) -> Result<Option<Event>, AdapterError> {
    if input.len() > MAX_HOOK_INPUT_BYTES {
        return Err(AdapterError::InputTooLarge);
    }
    let envelope: HookEnvelope =
        serde_json::from_slice(input).map_err(|_| AdapterError::InvalidJson)?;
    if envelope.session_id.is_empty()
        || envelope.session_id.len() > 512
        || envelope.hook_event_name.is_empty()
        || envelope.hook_event_name.len() > 64
        || envelope
            .tool_name
            .as_ref()
            .is_some_and(|name| name.len() > 256)
        || envelope
            .reason
            .as_ref()
            .is_some_and(|reason| reason.len() > 64)
        || envelope
            .source
            .as_ref()
            .is_some_and(|source| source.len() > 64)
    {
        return Err(AdapterError::InvalidMetadata);
    }

    let kind = match (agent, envelope.hook_event_name.as_str()) {
        (_, "SessionStart") => EventKind::Started,
        (NativeAgent::Codex | NativeAgent::Claude, "UserPromptSubmit")
        | (NativeAgent::Codex | NativeAgent::Claude, "PostToolUse")
        | (NativeAgent::Gemini, "BeforeAgent" | "AfterTool") => EventKind::Thinking,
        (NativeAgent::Codex | NativeAgent::Claude, "PreToolUse")
        | (NativeAgent::Gemini, "BeforeTool") => EventKind::ToolRunning {
            category: categorize_tool(envelope.tool_name.as_deref().unwrap_or("")),
        },
        (NativeAgent::Codex | NativeAgent::Claude, "Stop")
        | (NativeAgent::Gemini, "AfterAgent") => EventKind::Waiting,
        (NativeAgent::Claude | NativeAgent::Gemini, "SessionEnd") => EventKind::Interrupted,
        _ => return Ok(None),
    };
    let name = format!("{}:{}", agent.id(), envelope.session_id);
    let session_id = Uuid::new_v5(&SESSION_NAMESPACE, name.as_bytes());
    let mut event = Event::new(session_id, agent.adapter_id(), agent.id(), kind);
    event.context_pressure = context_pressure(&envelope);
    Ok(Some(event))
}

fn context_pressure(envelope: &HookEnvelope) -> Option<Metric<f32>> {
    let percent = envelope.context_percent.or_else(|| {
        let window = envelope.context_window.as_ref()?;
        (window.max_tokens > 0)
            .then(|| (window.used_tokens as f64 * 100.0 / window.max_tokens as f64) as f32)
    })?;
    (percent.is_finite() && (0.0..=100.0).contains(&percent))
        .then(|| Metric::new(percent, MetricQuality::Exact, MetricSource::Agent))
}

pub fn categorize_tool(name: &str) -> ToolCategory {
    let name = name.to_ascii_lowercase();
    if ["bash", "shell", "command", "exec", "terminal"]
        .iter()
        .any(|token| name.contains(token))
    {
        ToolCategory::Shell
    } else if ["write", "edit", "patch", "replace"]
        .iter()
        .any(|token| name.contains(token))
    {
        ToolCategory::FileWrite
    } else if ["read", "view", "cat"]
        .iter()
        .any(|token| name.contains(token))
    {
        ToolCategory::FileRead
    } else if ["search", "grep", "glob", "find"]
        .iter()
        .any(|token| name.contains(token))
    {
        ToolCategory::Search
    } else if ["web", "fetch", "http", "network"]
        .iter()
        .any(|token| name.contains(token))
    {
        ToolCategory::Network
    } else {
        ToolCategory::Other
    }
}

pub fn attach_to_wrapper(mut event: Event, wrapper_session_id: Option<&str>) -> Event {
    if let Some(session_id) = wrapper_session_id.and_then(|value| Uuid::parse_str(value).ok()) {
        event.session_id = session_id;
        event.adapter_id = "generic".to_owned();
        if matches!(event.kind, EventKind::Interrupted) {
            event.kind = EventKind::Waiting;
        }
    }
    event
}
