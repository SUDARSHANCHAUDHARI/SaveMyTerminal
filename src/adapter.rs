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
    #[serde(default)]
    transcript_path: Option<String>,
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

/// Only the tail of a transcript is read, since the latest usage counters live
/// at the end. A large bound keeps the common case to a single short read.
const MAX_TRANSCRIPT_TAIL_BYTES: u64 = 512 * 1024;
const DEFAULT_CONTEXT_WINDOW: u64 = 200_000;

#[derive(Debug, Deserialize)]
struct TranscriptLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: Option<TranscriptMessage>,
}

#[derive(Debug, Deserialize)]
struct TranscriptMessage {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<TranscriptUsage>,
}

#[derive(Debug, Deserialize)]
struct TranscriptUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}

/// Extract the transcript path a supported hook payload advertises, if any.
pub fn transcript_path_from_hook(input: &[u8]) -> Option<String> {
    let envelope: HookEnvelope = serde_json::from_slice(input).ok()?;
    envelope
        .transcript_path
        .filter(|path| !path.is_empty() && path.len() <= 4096)
}

/// Derive context pressure from a transcript's latest token usage counters.
///
/// This reads only the numeric `usage` fields from the most recent assistant
/// entry and the model identifier. It never reads, stores, or transmits prompt
/// or response text. The caller gates this behind an explicit opt-in.
pub fn context_from_transcript(path: &std::path::Path) -> Option<Metric<f32>> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(MAX_TRANSCRIPT_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut tail = String::new();
    file.take(MAX_TRANSCRIPT_TAIL_BYTES)
        .read_to_string(&mut tail)
        .ok()?;
    // Drop a leading partial line when the read began mid-file.
    let lines = if start > 0 {
        tail.split_once('\n').map(|(_, rest)| rest).unwrap_or("")
    } else {
        tail.as_str()
    };

    let mut used = None;
    let mut limit = DEFAULT_CONTEXT_WINDOW;
    for line in lines.lines() {
        let Ok(parsed) = serde_json::from_str::<TranscriptLine>(line) else {
            continue;
        };
        if parsed.kind.as_deref() != Some("assistant") {
            continue;
        }
        let Some(message) = parsed.message else {
            continue;
        };
        if let Some(model) = message.model.as_deref() {
            limit = context_window_limit(model);
        }
        if let Some(usage) = message.usage {
            used = Some(
                usage.input_tokens
                    + usage.cache_read_input_tokens
                    + usage.cache_creation_input_tokens,
            );
        }
    }

    let used = used?;
    if limit == 0 {
        return None;
    }
    let percent = ((used as f64 / limit as f64) * 100.0) as f32;
    Some(Metric::new(
        percent.clamp(0.0, 100.0),
        MetricQuality::Estimated,
        MetricSource::Heuristic,
    ))
}

fn context_window_limit(model: &str) -> u64 {
    if model.to_ascii_lowercase().contains("1m") {
        1_000_000
    } else {
        DEFAULT_CONTEXT_WINDOW
    }
}
