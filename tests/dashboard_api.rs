use reqwest::{StatusCode, redirect::Policy};
use savemyterminal::{
    protocol::{Event, EventKind},
    service::{ServiceConfig, spawn_test_service},
};
use secrecy::SecretString;
use serde::Deserialize;
use std::time::Duration;
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
