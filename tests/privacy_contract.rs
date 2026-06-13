use savemyterminal::protocol::{
    Event, EventKind, FailureCategory, Metric, MetricQuality, MetricSource, PROTOCOL_VERSION,
    SessionSnapshot, SessionState, ToolCategory,
};
use serde_json::Value;
use uuid::Uuid;

#[test]
fn event_round_trip_contains_only_approved_top_level_fields() {
    let event = Event::new(Uuid::new_v4(), "generic", "unknown", EventKind::Started);
    let value = serde_json::to_value(event).unwrap();
    let object = value.as_object().unwrap();
    let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "adapter_id",
            "agent_id",
            "event_id",
            "kind",
            "protocol_version",
            "session_id",
            "timestamp_ms"
        ]
    );
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
