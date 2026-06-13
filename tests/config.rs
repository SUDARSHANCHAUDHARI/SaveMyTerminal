use savemyterminal::{
    config::{
        DashboardPort, Settings, load, normalized_toml, reset_key, save_atomic, save_with_backup,
        set_key,
    },
    paths::AppPaths,
};
use std::{path::PathBuf, time::Duration};

fn test_paths() -> AppPaths {
    AppPaths {
        config_dir: PathBuf::from("config"),
        runtime_dir: PathBuf::from("runtime"),
        data_dir: PathBuf::from("data"),
    }
}

#[test]
fn phase_three_paths_are_scoped_to_existing_app_directories() {
    let paths = test_paths();

    assert_eq!(
        paths.settings_file(),
        paths.config_dir.join("settings.toml")
    );
    assert_eq!(
        paths.manifest_file(),
        paths.config_dir.join("integrations.json")
    );
    assert_eq!(paths.backup_dir(), paths.data_dir.join("backups"));
}

#[test]
fn defaults_preserve_existing_runtime_behavior() {
    let settings = Settings::default();

    assert_eq!(settings.version, 1);
    assert_eq!(settings.service.idle_timeout_seconds, 300);
    assert_eq!(settings.service.dashboard_port, DashboardPort::Auto);
    assert!(settings.history.enabled);
    assert_eq!(settings.history.retention_days, 30);
    assert!(settings.presentation.status_enabled);
    assert!(settings.presentation.status_compact);
    assert!(settings.presentation.ambient_enabled);
    assert_eq!(settings.presentation.ambient_intensity, 60);
    assert!(settings.diagnostics.cpu);
    assert!(settings.diagnostics.memory);
    assert!(settings.diagnostics.duration);
    assert!(!settings.diagnostics.command_health);
    assert_eq!(settings.idle_timeout(), Duration::from_secs(300));
    assert_eq!(
        settings.history_retention(),
        Duration::from_secs(30 * 24 * 60 * 60)
    );
}

#[test]
fn defaults_leave_future_integration_selection_empty() {
    let settings = Settings::default();

    assert!(settings.integrations.agents.is_empty());
    assert!(settings.integrations.renderers.is_empty());
}

#[test]
fn validation_rejects_unsupported_versions_and_runtime_ranges() {
    let mut settings = Settings {
        version: 2,
        ..Settings::default()
    };
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("version")
    );

    settings = Settings::default();
    settings.service.idle_timeout_seconds = 4;
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("service.idle_timeout_seconds")
    );

    settings = Settings::default();
    settings.service.dashboard_port = DashboardPort::Fixed(80);
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("service.dashboard_port")
    );
}

#[test]
fn validation_rejects_invalid_history_and_presentation_ranges() {
    let mut settings = Settings::default();
    settings.history.retention_days = 0;
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("history.retention_days")
    );

    settings = Settings::default();
    settings.presentation.ambient_intensity = 101;
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("presentation.ambient_intensity")
    );
}

#[test]
fn validation_rejects_unsafe_or_duplicate_integration_identifiers() {
    let mut settings = Settings::default();
    settings.integrations.agents = vec!["Claude Code".to_owned()];
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("integrations.agents")
    );

    settings.integrations.agents = vec!["codex".to_owned(), "codex".to_owned()];
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
}

#[test]
fn missing_settings_load_defaults_without_creating_a_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");

    assert_eq!(load(&path).unwrap(), Settings::default());
    assert!(!path.exists());
}

#[test]
fn unknown_fields_and_empty_files_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");

    std::fs::write(&path, "unknown = true\n").unwrap();
    assert!(load(&path).unwrap_err().to_string().contains("settings"));

    std::fs::write(&path, "").unwrap();
    assert!(load(&path).is_err());
}

#[test]
fn normalized_toml_round_trips_the_closed_settings_model() {
    let settings = Settings::default();

    let encoded = normalized_toml(&settings).unwrap();
    assert!(encoded.starts_with("version = 1\n"));
    assert!(encoded.contains("dashboard_port = \"auto\""));
    assert!(!encoded.contains("prompt"));

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    std::fs::write(&path, encoded).unwrap();
    assert_eq!(load(&path).unwrap(), settings);
}

#[test]
fn save_with_backup_preserves_the_previous_settings() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    let backup_dir = temp.path().join("backups");
    let original = Settings::default();
    save_atomic(&path, &original).unwrap();

    let mut updated = original.clone();
    updated.history.retention_days = 14;
    let backup = save_with_backup(&path, &backup_dir, &updated)
        .unwrap()
        .unwrap();

    assert_eq!(load(&path).unwrap(), updated);
    assert_eq!(load(&backup).unwrap(), original);
}

#[test]
fn failed_validation_leaves_the_original_file_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    save_atomic(&path, &Settings::default()).unwrap();
    let before = std::fs::read(&path).unwrap();

    let mut invalid = Settings::default();
    invalid.history.retention_days = 0;
    assert!(save_atomic(&path, &invalid).is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn settings_and_backups_are_user_only() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    let backup_dir = temp.path().join("backups");
    save_atomic(&path, &Settings::default()).unwrap();
    let backup = save_with_backup(&path, &backup_dir, &Settings::default())
        .unwrap()
        .unwrap();

    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(backup).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn dotted_keys_update_supported_scalar_and_list_settings() {
    let mut settings = Settings::default();

    set_key(&mut settings, "history.enabled", "false").unwrap();
    set_key(&mut settings, "history.retention_days", "14").unwrap();
    set_key(&mut settings, "service.dashboard_port", "43123").unwrap();
    set_key(&mut settings, "integrations.agents", "codex,claude").unwrap();

    assert!(!settings.history.enabled);
    assert_eq!(settings.history.retention_days, 14);
    assert_eq!(settings.service.dashboard_port, DashboardPort::Fixed(43123));
    assert_eq!(settings.integrations.agents, ["codex", "claude"]);
}

#[test]
fn dotted_key_updates_reject_unknown_keys_and_invalid_values_without_mutation() {
    let mut settings = Settings::default();
    let before = settings.clone();

    assert!(set_key(&mut settings, "history.days", "14").is_err());
    assert_eq!(settings, before);
    assert!(set_key(&mut settings, "history.retention_days", "0").is_err());
    assert_eq!(settings, before);
}

#[test]
fn reset_key_restores_one_value_or_all_defaults() {
    let mut settings = Settings::default();
    settings.history.retention_days = 14;
    settings.presentation.status_enabled = false;

    reset_key(&mut settings, Some("history.retention_days")).unwrap();
    assert_eq!(settings.history.retention_days, 30);
    assert!(!settings.presentation.status_enabled);

    reset_key(&mut settings, None).unwrap();
    assert_eq!(settings, Settings::default());
}
