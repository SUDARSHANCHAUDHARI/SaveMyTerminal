use reqwest::StatusCode;
use savemyterminal::{
    protocol::{Event, EventKind},
    service::{ServiceConfig, spawn_test_service},
};
use secrecy::SecretString;
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn rejects_missing_authentication() {
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from(
        "secret".to_owned(),
    )))
    .await
    .unwrap();

    let response = reqwest::Client::new()
        .get(format!("{}/v1/health", service.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    service.shutdown().await;
}

#[tokio::test]
async fn accepts_valid_event_and_returns_snapshot() {
    let token = "secret";
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from(
        token.to_owned(),
    )))
    .await
    .unwrap();
    let event = Event::new(Uuid::new_v4(), "generic", "unknown", EventKind::Started);

    let response = reqwest::Client::new()
        .post(format!("{}/v1/events", service.base_url))
        .bearer_auth(token)
        .json(&event)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["state"],
        "starting"
    );
    service.shutdown().await;
}

#[tokio::test]
async fn rejects_invalid_event_without_mutating_state() {
    let token = "secret";
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from(
        token.to_owned(),
    )))
    .await
    .unwrap();
    let mut event = Event::new(Uuid::new_v4(), "generic", "unknown", EventKind::Started);
    event.adapter_id.clear();

    let response = reqwest::Client::new()
        .post(format!("{}/v1/events", service.base_url))
        .bearer_auth(token)
        .json(&event)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    service.shutdown().await;
}

#[tokio::test]
async fn rejects_event_bodies_larger_than_16_kib() {
    let token = "secret";
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from(
        token.to_owned(),
    )))
    .await
    .unwrap();

    let response = reqwest::Client::new()
        .post(format!("{}/v1/events", service.base_url))
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(format!(r#"{{"padding":"{}"}}"#, "x".repeat(17 * 1024)))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    service.shutdown().await;
}

#[tokio::test]
async fn idle_service_stops_after_timeout() {
    let mut config = ServiceConfig::for_test(SecretString::from("secret".to_owned()));
    config.idle_timeout = Duration::from_millis(50);
    let service = spawn_test_service(config).await.unwrap();

    tokio::time::timeout(Duration::from_secs(1), service.finished())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn stale_active_session_does_not_prevent_idle_shutdown() {
    let token = "secret";
    let mut config = ServiceConfig::for_test(SecretString::from(token.to_owned()));
    config.idle_timeout = Duration::from_millis(50);
    let service = spawn_test_service(config).await.unwrap();
    let event = Event::new(Uuid::new_v4(), "generic", "unknown", EventKind::Started);

    reqwest::Client::new()
        .post(format!("{}/v1/events", service.base_url))
        .bearer_auth(token)
        .json(&event)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    tokio::time::timeout(Duration::from_secs(1), service.finished())
        .await
        .unwrap()
        .unwrap();
}
