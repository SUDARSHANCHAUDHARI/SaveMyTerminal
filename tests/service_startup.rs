use savemyterminal::{
    auth::load_token,
    client::ServiceClient,
    config::{DashboardPort, Settings, save_atomic},
    paths::AppPaths,
    service::ServiceDiscovery,
};
use secrecy::ExposeSecret;
use std::time::Duration;

#[tokio::test]
async fn concurrent_ensure_calls_reuse_one_reachable_endpoint() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths {
        config_dir: temp.path().join("config"),
        runtime_dir: temp.path().join("runtime"),
        data_dir: temp.path().join("data"),
    };
    let executable = assert_cmd::cargo::cargo_bin!("savemyterminal");

    let (first, second) = tokio::join!(
        ServiceClient::ensure_with_executable(&paths, executable, Duration::from_millis(500)),
        ServiceClient::ensure_with_executable(&paths, executable, Duration::from_millis(500)),
    );
    let first = first.unwrap();
    let second = second.unwrap();

    assert_eq!(first.base_url(), second.base_url());
    first.health().await.unwrap();
}

#[tokio::test]
async fn connect_does_not_create_missing_auth_state() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths {
        config_dir: temp.path().join("config"),
        runtime_dir: temp.path().join("runtime"),
        data_dir: temp.path().join("data"),
    };

    assert!(ServiceClient::connect(&paths).await.is_err());
    assert!(!paths.token_file().exists());
}

#[tokio::test]
async fn service_command_applies_fixed_port_and_disabled_history_settings() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths {
        config_dir: temp.path().join("config"),
        runtime_dir: temp.path().join("runtime"),
        data_dir: temp.path().join("data"),
    };
    let reservation = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let mut settings = Settings::default();
    settings.service.dashboard_port = DashboardPort::Fixed(port);
    settings.history.enabled = false;
    save_atomic(&paths.settings_file(), &settings).unwrap();

    let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("savemyterminal"))
        .arg("service")
        .arg("--config-dir")
        .arg(&paths.config_dir)
        .arg("--runtime-dir")
        .arg(&paths.runtime_dir)
        .arg("--data-dir")
        .arg(&paths.data_dir)
        .arg("--idle-timeout-ms")
        .arg("500")
        .spawn()
        .unwrap();

    let discovery = loop {
        if let Ok(bytes) = std::fs::read(paths.discovery_file()) {
            break serde_json::from_slice::<ServiceDiscovery>(&bytes).unwrap();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(discovery.base_url, format!("http://127.0.0.1:{port}"));

    let response = reqwest::Client::new()
        .get(format!("{}/v1/history", discovery.base_url))
        .bearer_auth(load_token(&paths.token_file()).unwrap().expose_secret())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    assert!(!paths.database_file().exists());

    tokio::time::timeout(Duration::from_secs(2), child.wait())
        .await
        .unwrap()
        .unwrap();
}
