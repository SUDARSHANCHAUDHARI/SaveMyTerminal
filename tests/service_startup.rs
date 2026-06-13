use savemyterminal::{client::ServiceClient, paths::AppPaths};
use std::time::Duration;

#[tokio::test]
async fn concurrent_ensure_calls_reuse_one_reachable_endpoint() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths {
        config_dir: temp.path().join("config"),
        runtime_dir: temp.path().join("runtime"),
        data_dir: temp.path().join("data"),
    };
    let executable = assert_cmd::cargo::cargo_bin!("smt");

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
