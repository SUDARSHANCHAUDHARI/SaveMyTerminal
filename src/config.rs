use serde::{Deserialize, Serialize};
use std::{collections::HashSet, time::Duration};
use thiserror::Error;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub version: u32,
    pub service: ServiceSettings,
    pub history: HistorySettings,
    pub presentation: PresentationSettings,
    pub diagnostics: DiagnosticSettings,
    pub logging: LoggingSettings,
    pub integrations: IntegrationSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            service: ServiceSettings::default(),
            history: HistorySettings::default(),
            presentation: PresentationSettings::default(),
            diagnostics: DiagnosticSettings::default(),
            logging: LoggingSettings::default(),
            integrations: IntegrationSettings::default(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != SETTINGS_VERSION {
            return Err(ConfigError::InvalidValue {
                key: "version",
                reason: format!("expected {SETTINGS_VERSION}"),
            });
        }
        if !(5..=86_400).contains(&self.service.idle_timeout_seconds) {
            return Err(ConfigError::InvalidValue {
                key: "service.idle_timeout_seconds",
                reason: "must be between 5 and 86400".to_owned(),
            });
        }
        if matches!(self.service.dashboard_port, DashboardPort::Fixed(port) if port < 1024) {
            return Err(ConfigError::InvalidValue {
                key: "service.dashboard_port",
                reason: "fixed ports must be between 1024 and 65535".to_owned(),
            });
        }
        if !(1..=3_650).contains(&self.history.retention_days) {
            return Err(ConfigError::InvalidValue {
                key: "history.retention_days",
                reason: "must be between 1 and 3650".to_owned(),
            });
        }
        if self.presentation.ambient_intensity > 100 {
            return Err(ConfigError::InvalidValue {
                key: "presentation.ambient_intensity",
                reason: "must be between 0 and 100".to_owned(),
            });
        }
        validate_identifiers("integrations.agents", &self.integrations.agents)?;
        validate_identifiers("integrations.renderers", &self.integrations.renderers)?;
        Ok(())
    }

    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs(self.service.idle_timeout_seconds)
    }

    pub fn history_retention(&self) -> Duration {
        Duration::from_secs(u64::from(self.history.retention_days) * 24 * 60 * 60)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceSettings {
    pub idle_timeout_seconds: u64,
    pub dashboard_port: DashboardPort,
}

impl Default for ServiceSettings {
    fn default() -> Self {
        Self {
            idle_timeout_seconds: 300,
            dashboard_port: DashboardPort::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardPort {
    Auto,
    Fixed(u16),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistorySettings {
    pub enabled: bool,
    pub retention_days: u16,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationSettings {
    pub status_enabled: bool,
    pub status_compact: bool,
    pub ambient_enabled: bool,
    pub ambient_intensity: u8,
}

impl Default for PresentationSettings {
    fn default() -> Self {
        Self {
            status_enabled: true,
            status_compact: true,
            ambient_enabled: true,
            ambient_intensity: 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticSettings {
    pub cpu: bool,
    pub memory: bool,
    pub duration: bool,
    pub command_health: bool,
}

impl Default for DiagnosticSettings {
    fn default() -> Self {
        Self {
            cpu: true,
            memory: true,
            duration: true,
            command_health: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoggingSettings {
    pub level: LogLevel,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: LogLevel::Warn,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationSettings {
    pub agents: Vec<String>,
    pub renderers: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid {key}: {reason}")]
    InvalidValue { key: &'static str, reason: String },
}

fn validate_identifiers(key: &'static str, identifiers: &[String]) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();
    for identifier in identifiers {
        if identifier.is_empty()
            || !identifier.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte)
            })
        {
            return Err(ConfigError::InvalidValue {
                key,
                reason: format!("invalid integration identifier {identifier:?}"),
            });
        }
        if !seen.insert(identifier) {
            return Err(ConfigError::InvalidValue {
                key,
                reason: format!("duplicate integration identifier {identifier:?}"),
            });
        }
    }
    Ok(())
}
