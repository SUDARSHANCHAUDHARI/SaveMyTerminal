use savemyterminal::{
    adapter::{
        AdapterError, MAX_HOOK_INPUT_BYTES, NativeAgent, attach_to_wrapper, categorize_tool,
        map_hook,
    },
    protocol::{EventKind, MetricQuality, MetricSource, ToolCategory},
};

#[test]
fn maps_each_native_lifecycle_without_claiming_turn_end_is_completion() {
    let cases = [
        (NativeAgent::Codex, "SessionStart", EventKind::Started),
        (NativeAgent::Codex, "UserPromptSubmit", EventKind::Thinking),
        (NativeAgent::Codex, "Stop", EventKind::Waiting),
        (NativeAgent::Claude, "SessionEnd", EventKind::Interrupted),
        (NativeAgent::Gemini, "BeforeAgent", EventKind::Thinking),
        (NativeAgent::Gemini, "AfterAgent", EventKind::Waiting),
        (NativeAgent::Gemini, "SessionEnd", EventKind::Interrupted),
    ];
    for (agent, hook_event_name, expected) in cases {
        let input = serde_json::json!({
            "session_id": "native-session",
            "hook_event_name": hook_event_name,
        });
        let event = map_hook(agent, input.to_string().as_bytes())
            .unwrap()
            .unwrap();
        assert_eq!(event.kind, expected);
    }
}

#[test]
fn attached_hook_targets_only_its_generic_wrapper_session() {
    let wrapper_id = uuid::Uuid::new_v4();
    let input = br#"{"session_id":"native","hook_event_name":"PreToolUse","tool_name":"Bash"}"#;
    let event = map_hook(NativeAgent::Codex, input).unwrap().unwrap();

    let attached = attach_to_wrapper(event, Some(&wrapper_id.to_string()));

    assert_eq!(attached.session_id, wrapper_id);
    assert_eq!(attached.adapter_id, "generic");
}

#[test]
fn attached_wrapper_owns_process_completion() {
    let wrapper_id = uuid::Uuid::new_v4();
    let input = br#"{"session_id":"native","hook_event_name":"SessionEnd"}"#;
    let event = map_hook(NativeAgent::Claude, input).unwrap().unwrap();

    let attached = attach_to_wrapper(event, Some(&wrapper_id.to_string()));

    assert_eq!(attached.kind, EventKind::Waiting);
}

#[test]
fn maps_optional_context_pressure_without_reading_conversation_content() {
    let input = serde_json::json!({
        "session_id": "native-session",
        "hook_event_name": "UserPromptSubmit",
        "context_window": {"used_tokens": 7500, "max_tokens": 10000},
        "prompt": "must-not-be-captured"
    });

    let event = map_hook(NativeAgent::Codex, input.to_string().as_bytes())
        .unwrap()
        .unwrap();
    let context = event.context_pressure.unwrap();

    assert_eq!(context.value, 75.0);
    assert_eq!(context.quality, MetricQuality::Exact);
    assert_eq!(context.source, MetricSource::Agent);
}

#[test]
fn derives_stable_agent_isolated_session_ids() {
    let input = br#"{"session_id":"same","hook_event_name":"SessionStart"}"#;
    let first = map_hook(NativeAgent::Codex, input).unwrap().unwrap();
    let second = map_hook(NativeAgent::Codex, input).unwrap().unwrap();
    let other = map_hook(NativeAgent::Claude, input).unwrap().unwrap();
    assert_eq!(first.session_id, second.session_id);
    assert_ne!(first.session_id, other.session_id);
    assert_ne!(first.session_id.to_string(), "same");
}

#[test]
fn categorizes_only_from_the_tool_name() {
    assert_eq!(categorize_tool("Bash"), ToolCategory::Shell);
    assert_eq!(categorize_tool("read_file"), ToolCategory::FileRead);
    assert_eq!(categorize_tool("apply_patch"), ToolCategory::FileWrite);
    assert_eq!(categorize_tool("WebSearch"), ToolCategory::Search);
    assert_eq!(categorize_tool("WebFetch"), ToolCategory::Network);
    assert_eq!(categorize_tool("mcp_custom"), ToolCategory::Other);
}

#[test]
fn ignores_sensitive_hook_payload_fields() {
    let secret = "do-not-store-this-secret";
    let input = serde_json::json!({
        "session_id": "private-native-id",
        "hook_event_name": "PreToolUse",
        "tool_name": "Read",
        "prompt": secret,
        "prompt_response": secret,
        "tool_input": {"path": secret},
        "tool_response": {"content": secret},
        "transcript_path": secret,
        "cwd": secret,
    });
    let event = map_hook(NativeAgent::Claude, input.to_string().as_bytes())
        .unwrap()
        .unwrap();
    let encoded = serde_json::to_string(&event).unwrap();
    assert!(!encoded.contains(secret));
    assert!(!encoded.contains("private-native-id"));
    assert_eq!(
        event.kind,
        EventKind::ToolRunning {
            category: ToolCategory::FileRead
        }
    );
}

#[test]
fn rejects_oversized_or_invalid_input_without_echoing_it() {
    assert_eq!(
        map_hook(NativeAgent::Codex, &vec![b'x'; MAX_HOOK_INPUT_BYTES + 1]),
        Err(AdapterError::InputTooLarge)
    );
    let error = map_hook(NativeAgent::Codex, b"secret invalid json").unwrap_err();
    assert_eq!(error.to_string(), "hook input is not valid metadata JSON");
    assert!(!error.to_string().contains("secret"));
}
