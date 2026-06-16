use savemyterminal::protocol::{
    Event, EventKind, FailureCategory, Metric, MetricQuality, MetricSource, PROTOCOL_VERSION,
    SessionSnapshot, SessionState, ToolCategory,
};
use savemyterminal::{
    config::{Settings, load, normalized_toml, set_key},
    detection::{AgentId, EnvironmentReport, OsId, ShellId, TerminalId},
    manifest::{IntegrationManifest, IntegrationRecord},
};
use serde_json::Value;
use std::path::PathBuf;
use uuid::Uuid;

#[test]
fn repository_has_no_active_github_actions_workflow() {
    let workflows = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let active = std::fs::read_dir(workflows)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0);

    assert_eq!(active, 0);
}

#[test]
fn event_round_trip_contains_only_approved_top_level_fields() {
    let mut event = Event::new(Uuid::new_v4(), "generic", "unknown", EventKind::Started);
    event.context_pressure = Some(Metric::new(
        50.0,
        MetricQuality::Estimated,
        MetricSource::Heuristic,
    ));
    let value = serde_json::to_value(event).unwrap();
    let object = value.as_object().unwrap();
    let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "adapter_id",
            "agent_id",
            "context_pressure",
            "event_id",
            "kind",
            "protocol_version",
            "session_id",
            "timestamp_ms"
        ]
    );
}

#[test]
fn rejects_invalid_context_pressure_metrics() {
    for pressure in [f32::NAN, f32::INFINITY, -0.1, 100.1] {
        let mut event = Event::new(Uuid::new_v4(), "generic", "unknown", EventKind::Thinking);
        event.context_pressure = Some(Metric::new(
            pressure,
            MetricQuality::Exact,
            MetricSource::Agent,
        ));

        assert_eq!(
            event.validate().unwrap_err().to_string(),
            "context pressure must be finite and between 0 and 100"
        );
    }
}

#[test]
fn serialized_event_never_exposes_prohibited_field_names() {
    let event = Event::new(
        Uuid::new_v4(),
        "generic",
        "unknown",
        EventKind::Metrics {
            cpu_percent: Some(Metric::new(12.5, MetricQuality::Exact, MetricSource::Os)),
            memory_bytes: None,
        },
    );
    let value: Value = serde_json::to_value(event).unwrap();
    let json = value.to_string().to_ascii_lowercase();
    for prohibited in [
        "prompt",
        "response",
        "output",
        "command",
        "argument",
        "environment",
        "working_directory",
        "path",
        "file_content",
    ] {
        assert!(
            !json.contains(prohibited),
            "found prohibited key: {prohibited}"
        );
    }
}

#[test]
fn rejects_unknown_protocol_versions() {
    let mut event = Event::new(Uuid::new_v4(), "generic", "unknown", EventKind::Started);
    event.protocol_version = PROTOCOL_VERSION + 1;
    assert_eq!(
        event.validate().unwrap_err().to_string(),
        "unsupported protocol version"
    );
}

#[test]
fn rejects_empty_identifiers() {
    for (adapter_id, agent_id) in [("", "unknown"), ("generic", "")] {
        let event = Event::new(Uuid::new_v4(), adapter_id, agent_id, EventKind::Started);
        assert_eq!(
            event.validate().unwrap_err().to_string(),
            "adapter and agent identifiers must not be empty"
        );
    }
}

#[test]
fn rejects_identifiers_longer_than_64_bytes() {
    for (adapter_id, agent_id) in [
        ("a".repeat(65), "unknown".to_owned()),
        ("generic".to_owned(), "a".repeat(65)),
        ("é".repeat(33), "unknown".to_owned()),
    ] {
        let event = Event::new(Uuid::new_v4(), adapter_id, agent_id, EventKind::Started);
        assert_eq!(
            event.validate().unwrap_err().to_string(),
            "adapter and agent identifiers must not exceed 64 bytes"
        );
    }
}

#[test]
fn protocol_enums_reject_unknown_variants() {
    assert!(serde_json::from_str::<MetricQuality>(r#""unknown""#).is_err());
    assert!(serde_json::from_str::<MetricSource>(r#""unknown""#).is_err());
    assert!(serde_json::from_str::<ToolCategory>(r#""unknown""#).is_err());
    assert!(serde_json::from_str::<FailureCategory>(r#""mystery""#).is_err());
}

#[test]
fn rejects_unknown_top_level_event_fields() {
    let event = Event::new(Uuid::new_v4(), "generic", "unknown", EventKind::Started);
    let mut value = serde_json::to_value(event).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("prompt".to_owned(), Value::String("secret".to_owned()));

    assert!(serde_json::from_value::<Event>(value).is_err());
}

#[test]
fn rejects_unknown_event_kind_fields() {
    let event = Event::new(
        Uuid::new_v4(),
        "generic",
        "unknown",
        EventKind::ToolRunning {
            category: ToolCategory::Shell,
        },
    );
    let mut value = serde_json::to_value(event).unwrap();
    value["kind"]
        .as_object_mut()
        .unwrap()
        .insert("output".to_owned(), Value::String("secret".to_owned()));

    assert!(serde_json::from_value::<Event>(value).is_err());
}

#[test]
fn rejects_unknown_metric_fields() {
    let event = Event::new(
        Uuid::new_v4(),
        "generic",
        "unknown",
        EventKind::Metrics {
            cpu_percent: Some(Metric::new(12.5, MetricQuality::Exact, MetricSource::Os)),
            memory_bytes: None,
        },
    );
    let mut value = serde_json::to_value(event).unwrap();
    value["kind"]["cpu_percent"]
        .as_object_mut()
        .unwrap()
        .insert("command".to_owned(), Value::String("secret".to_owned()));

    assert!(serde_json::from_value::<Event>(value).is_err());
}

#[test]
fn rejects_unknown_session_snapshot_fields() {
    let snapshot = SessionSnapshot {
        session_id: Uuid::new_v4(),
        adapter_id: "generic".to_owned(),
        agent_id: "unknown".to_owned(),
        state: SessionState::Starting,
        started_at_ms: 1,
        updated_at_ms: 1,
        cpu_percent: None,
        memory_bytes: None,
        context_pressure: None,
    };
    let mut value = serde_json::to_value(snapshot).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("environment".to_owned(), Value::String("secret".to_owned()));

    assert!(serde_json::from_value::<SessionSnapshot>(value).is_err());
}

#[test]
fn rejects_non_finite_cpu_percent_metrics() {
    for cpu_percent in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let event = Event::new(
            Uuid::new_v4(),
            "generic",
            "unknown",
            EventKind::Metrics {
                cpu_percent: Some(Metric::new(
                    cpu_percent,
                    MetricQuality::Exact,
                    MetricSource::Os,
                )),
                memory_bytes: None,
            },
        );

        assert_eq!(
            event.validate().unwrap_err().to_string(),
            "cpu percent metric must be finite"
        );
    }
}

#[test]
fn settings_and_manifest_schemas_contain_only_approved_metadata_fields() {
    let settings = normalized_toml(&Settings::default())
        .unwrap()
        .to_ascii_lowercase();
    let manifest = serde_json::to_string(&IntegrationManifest {
        version: 1,
        integrations: vec![IntegrationRecord {
            id: "example".to_owned(),
            descriptor_version: 1,
            target_path: PathBuf::from("managed-target"),
            marker_id: "example".to_owned(),
            backup_path: Some(PathBuf::from("managed-backup")),
            post_write_sha256: "aa".repeat(32),
            applied_at_unix_ms: 1,
        }],
    })
    .unwrap()
    .to_ascii_lowercase();

    for prohibited in [
        "prompt",
        "response",
        "terminal_output",
        "command_argument",
        "working_directory",
        "file_contents",
        "environment_value",
        "username",
        "hostname",
        "repository_remote",
        "credential",
    ] {
        assert!(!settings.contains(prohibited));
        assert!(!manifest.contains(prohibited));
    }
}

#[test]
fn detection_serialization_is_limited_to_closed_identifiers() {
    let encoded = serde_json::to_value(EnvironmentReport {
        os: OsId::Macos,
        shell: Some(ShellId::Zsh),
        agents: vec![AgentId::Codex],
        terminals: vec![TerminalId::Ghostty],
    })
    .unwrap();
    let object = encoded.as_object().unwrap();
    let mut keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, ["agents", "os", "shell", "terminals"]);
}

#[test]
fn configuration_errors_do_not_echo_arbitrary_file_or_identifier_content() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    let secret = "sensitive/value-must-not-be-reflected";
    std::fs::write(&path, format!("unknown = {secret:?}\n")).unwrap();

    let parse_error = load(&path).unwrap_err().to_string();
    assert!(!parse_error.contains(secret));
    assert!(parse_error.contains(path.to_str().unwrap()));

    let mut settings = Settings::default();
    let validation_error = set_key(&mut settings, "integrations.agents", secret)
        .unwrap_err()
        .to_string();
    assert!(!validation_error.contains(secret));
    assert!(validation_error.contains("integrations.agents"));
}
