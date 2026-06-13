use savemyterminal::{
    config::{DashboardPort, Settings},
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
