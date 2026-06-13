use crate::protocol::{Event, EventKind, ToolCategory};
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

pub const MAX_HOOK_INPUT_BYTES: usize = 64 * 1024;
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
    Ok(Some(Event::new(
        session_id,
        agent.adapter_id(),
        agent.id(),
        kind,
    )))
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
