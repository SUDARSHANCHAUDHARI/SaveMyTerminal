use futures_util::StreamExt;
use reqwest::{StatusCode, redirect::Policy};
use savemyterminal::{
    protocol::{Event, EventKind},
    service::{ServiceConfig, spawn_test_service},
};
use secrecy::SecretString;
use serde::Deserialize;
use std::time::Duration;

#[tokio::test]
async fn fixed_dashboard_port_binds_the_requested_loopback_port() {
    let reservation = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);

    let mut config = ServiceConfig::for_test(SecretString::from("secret".to_owned()));
    config.listen_port = Some(port);
    let service = spawn_test_service(config).await.unwrap();

    assert_eq!(service.base_url, format!("http://127.0.0.1:{port}"));
    service.shutdown().await;
}
use uuid::Uuid;

#[derive(Deserialize)]
struct LaunchResponse {
    launch_url: String,
}

#[tokio::test]
async fn dashboard_launch_token_is_single_use_and_sets_a_private_cookie() {
    let token = "secret";
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from(
        token.to_owned(),
    )))
    .await
    .unwrap();
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .unwrap();

    let launch = client
        .post(format!("{}/v1/dashboard-launch", service.base_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<LaunchResponse>()
        .await
        .unwrap();
    assert!(launch.launch_url.starts_with(&service.base_url));

    let first = client.get(&launch.launch_url).send().await.unwrap();
    assert_eq!(first.status(), StatusCode::SEE_OTHER);
    assert_eq!(first.headers()["location"], "/dashboard");
    let cookie = first.headers()["set-cookie"].to_str().unwrap();
    assert!(cookie.contains("smt_dashboard="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));

    let reused = client.get(&launch.launch_url).send().await.unwrap();
    assert_eq!(reused.status(), StatusCode::UNAUTHORIZED);
    service.shutdown().await;
}

#[tokio::test]
async fn dashboard_launch_token_expires() {
    let token = "secret";
    let mut config = ServiceConfig::for_test(SecretString::from(token.to_owned()));
    config.dashboard_launch_ttl = Duration::from_millis(1);
    let service = spawn_test_service(config).await.unwrap();
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .unwrap();
    let launch = client
        .post(format!("{}/v1/dashboard-launch", service.base_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json::<LaunchResponse>()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(5)).await;

    assert_eq!(
        client.get(launch.launch_url).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    service.shutdown().await;
}

#[tokio::test]
async fn browser_cookie_authenticates_reads_but_requires_origin_for_mutations() {
    let token = "secret";
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from(
        token.to_owned(),
    )))
    .await
    .unwrap();
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .unwrap();
    let launch = client
        .post(format!("{}/v1/dashboard-launch", service.base_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json::<LaunchResponse>()
        .await
        .unwrap();
    let response = client.get(launch.launch_url).send().await.unwrap();
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();

    let health = client
        .get(format!("{}/v1/health", service.base_url))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::NO_CONTENT);

    let event = Event::new(Uuid::new_v4(), "generic", "unknown", EventKind::Started);
    let missing_origin = client
        .post(format!("{}/v1/events", service.base_url))
        .header("cookie", cookie)
        .json(&event)
        .send()
        .await
        .unwrap();
    assert_eq!(missing_origin.status(), StatusCode::FORBIDDEN);

    let same_origin = client
        .post(format!("{}/v1/events", service.base_url))
        .header("cookie", cookie)
        .header("origin", &service.base_url)
        .json(&event)
        .send()
        .await
        .unwrap();
    assert_eq!(same_origin.status(), StatusCode::OK);
    service.shutdown().await;
}

#[tokio::test]
async fn active_history_stats_delete_and_purge_follow_session_lifecycle() {
    let token = "secret";
    let temp = tempfile::tempdir().unwrap();
    let mut config = ServiceConfig::for_test(SecretString::from(token.to_owned()));
    config.database_file = Some(temp.path().join("history.sqlite3"));
    let service = spawn_test_service(config).await.unwrap();
    let client = reqwest::Client::new();
    let active_id = Uuid::new_v4();
    let completed_id = Uuid::new_v4();

    for session_id in [active_id, completed_id] {
        client
            .post(format!("{}/v1/events", service.base_url))
            .bearer_auth(token)
            .json(&Event::new(
                session_id,
                "generic",
                "codex",
                EventKind::Started,
            ))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    client
        .post(format!("{}/v1/events", service.base_url))
        .bearer_auth(token)
        .json(&Event::new(
            completed_id,
            "generic",
            "codex",
            EventKind::Completed { exit_code: 0 },
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let active = client
        .get(format!("{}/v1/sessions/active", service.base_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(active.as_array().unwrap().len(), 1);
    assert_eq!(active[0]["session_id"], active_id.to_string());

    let history = client
        .get(format!("{}/v1/history?limit=50&offset=0", service.base_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(history["total"], 1);
    assert_eq!(
        history["sessions"][0]["session_id"],
        completed_id.to_string()
    );

    let stats = client
        .get(format!("{}/v1/history/stats", service.base_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(stats["session_count"], 1);

    assert_eq!(
        client
            .delete(format!("{}/v1/history/{active_id}", service.base_url))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        client
            .delete(format!("{}/v1/history/{completed_id}", service.base_url))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        client
            .delete(format!("{}/v1/history", service.base_url))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    service.shutdown().await;
}

#[tokio::test]
async fn history_unavailable_does_not_break_live_sessions() {
    let token = "secret";
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from(
        token.to_owned(),
    )))
    .await
    .unwrap();
    let client = reqwest::Client::new();
    client
        .post(format!("{}/v1/events", service.base_url))
        .bearer_auth(token)
        .json(&Event::new(
            Uuid::new_v4(),
            "generic",
            "unknown",
            EventKind::Started,
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    assert_eq!(
        client
            .get(format!("{}/v1/history", service.base_url))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        client
            .get(format!("{}/v1/sessions/active", service.base_url))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    service.shutdown().await;
}

#[tokio::test]
async fn sse_sends_initial_and_changed_session_snapshots() {
    let token = "secret";
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from(
        token.to_owned(),
    )))
    .await
    .unwrap();
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/v1/sessions/stream", service.base_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let mut stream = response.bytes_stream();
    let initial = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&initial).contains("event: sessions"));

    let session_id = Uuid::new_v4();
    client
        .post(format!("{}/v1/events", service.base_url))
        .bearer_auth(token)
        .json(&Event::new(
            session_id,
            "generic",
            "codex",
            EventKind::Started,
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let changed = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&changed).contains(&session_id.to_string()));
    drop(stream);
    service.shutdown().await;
}

#[tokio::test]
async fn connected_sse_client_prevents_idle_shutdown_until_disconnect() {
    let token = "secret";
    let mut config = ServiceConfig::for_test(SecretString::from(token.to_owned()));
    config.idle_timeout = Duration::from_millis(50);
    let service = spawn_test_service(config).await.unwrap();
    let response = reqwest::Client::new()
        .get(format!("{}/v1/sessions/stream", service.base_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        reqwest::Client::new()
            .get(format!("{}/v1/health", service.base_url))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );

    drop(response);
    tokio::time::timeout(Duration::from_secs(1), service.finished())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn dashboard_assets_are_embedded_same_origin_and_hardened() {
    let token = "secret";
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from(
        token.to_owned(),
    )))
    .await
    .unwrap();
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .unwrap();
    let launch = client
        .post(format!("{}/v1/dashboard-launch", service.base_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json::<LaunchResponse>()
        .await
        .unwrap();
    let launched = client.get(launch.launch_url).send().await.unwrap();
    let cookie = launched.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();

    let html_response = client
        .get(format!("{}/dashboard", service.base_url))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(html_response.status(), StatusCode::OK);
    assert_eq!(
        html_response.headers()["content-type"],
        "text/html; charset=utf-8"
    );
    assert_eq!(html_response.headers()["x-content-type-options"], "nosniff");
    let csp = html_response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    assert!(csp.contains("default-src 'self'"));
    assert!(csp.contains("connect-src 'self'"));
    let html = html_response.text().await.unwrap();
    assert!(html.contains("/dashboard/app.css"));
    assert!(html.contains("/dashboard/app.js"));
    assert!(!html.contains("https://"));
    assert!(!html.contains("http://"));

    let javascript = client
        .get(format!("{}/dashboard/app.js", service.base_url))
        .header("cookie", cookie)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!javascript.contains("localStorage"));
    assert!(!javascript.contains("https://"));
    assert!(!javascript.contains("http://"));
    assert!(javascript.contains("function escapeAttribute"));
    assert!(javascript.contains("&quot;"));
    service.shutdown().await;
}

#[tokio::test]
async fn periodic_retention_removes_expired_finalized_history() {
    let token = "secret";
    let temp = tempfile::tempdir().unwrap();
    let mut config = ServiceConfig::for_test(SecretString::from(token.to_owned()));
    config.database_file = Some(temp.path().join("history.sqlite3"));
    config.history_retention = Duration::from_millis(50);
    config.history_cleanup_interval = Duration::from_millis(10);
    let service = spawn_test_service(config).await.unwrap();
    let client = reqwest::Client::new();
    let session_id = Uuid::new_v4();
    let mut started = Event::new(session_id, "generic", "unknown", EventKind::Started);
    started.timestamp_ms = 1;
    client
        .post(format!("{}/v1/events", service.base_url))
        .bearer_auth(token)
        .json(&started)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let mut completed = Event::new(
        session_id,
        "generic",
        "unknown",
        EventKind::Completed { exit_code: 0 },
    );
    completed.timestamp_ms = 2;
    client
        .post(format!("{}/v1/events", service.base_url))
        .bearer_auth(token)
        .json(&completed)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    tokio::time::sleep(Duration::from_millis(40)).await;
    let history = client
        .get(format!("{}/v1/history", service.base_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(history["total"], 0);
    service.shutdown().await;
}
