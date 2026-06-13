use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DashboardPort {
    Auto,
    Fixed(u16),
}

impl DashboardPort {
    pub fn socket_port(&self) -> Option<u16> {
        match self {
            Self::Auto => None,
            Self::Fixed(port) => Some(*port),
        }
    }
}

impl Serialize for DashboardPort {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Fixed(port) => serializer.serialize_u16(*port),
        }
    }
}

impl<'de> Deserialize<'de> for DashboardPort {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value {
            Name(String),
            Port(u16),
        }

        match Value::deserialize(deserializer)? {
            Value::Name(name) if name == "auto" => Ok(Self::Auto),
            Value::Name(name) => Err(serde::de::Error::unknown_variant(&name, &["auto"])),
            Value::Port(port) => Ok(Self::Fixed(port)),
        }
    }
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
    #[error("could not read settings at {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not parse settings at {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("could not serialize settings: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("could not write settings at {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unknown settings key {0}")]
    UnknownKey(String),
    #[error("invalid value for {key}: {reason}")]
    InvalidInput { key: String, reason: String },
}

pub fn load(path: &Path) -> Result<Settings, ConfigError> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Settings::default());
        }
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let settings: Settings = toml::from_str(&content).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    settings.validate()?;
    Ok(settings)
}

pub fn normalized_toml(settings: &Settings) -> Result<String, ConfigError> {
    settings.validate()?;
    let mut encoded = toml::to_string_pretty(settings)?;
    if !encoded.ends_with('\n') {
        encoded.push('\n');
    }
    Ok(encoded)
}

pub fn save_atomic(path: &Path, settings: &Settings) -> Result<(), ConfigError> {
    write_private_atomic(path, normalized_toml(settings)?.as_bytes())
}

pub fn save_with_backup(
    path: &Path,
    backup_dir: &Path,
    settings: &Settings,
) -> Result<Option<PathBuf>, ConfigError> {
    settings.validate()?;
    let backup = if path.exists() {
        let original = std::fs::read(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        std::fs::create_dir_all(backup_dir).map_err(|source| ConfigError::Write {
            path: backup_dir.to_path_buf(),
            source,
        })?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let digest = Sha256::digest(&original);
        let short_hash = digest[..4]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let backup = backup_dir.join(format!("settings-{timestamp}-{short_hash}.toml"));
        write_private_atomic(&backup, &original)?;
        Some(backup)
    } else {
        None
    };
    save_atomic(path, settings)?;
    Ok(backup)
}

pub fn set_key(settings: &mut Settings, key: &str, value: &str) -> Result<(), ConfigError> {
    let mut candidate = settings.clone();
    set_key_inner(&mut candidate, key, value)?;
    candidate.validate()?;
    *settings = candidate;
    Ok(())
}

pub fn reset_key(settings: &mut Settings, key: Option<&str>) -> Result<(), ConfigError> {
    let Some(key) = key else {
        *settings = Settings::default();
        return Ok(());
    };
    let defaults = Settings::default();
    let mut candidate = settings.clone();
    match key {
        "service.idle_timeout_seconds" => {
            candidate.service.idle_timeout_seconds = defaults.service.idle_timeout_seconds;
        }
        "service.dashboard_port" => {
            candidate.service.dashboard_port = defaults.service.dashboard_port;
        }
        "history.enabled" => candidate.history.enabled = defaults.history.enabled,
        "history.retention_days" => {
            candidate.history.retention_days = defaults.history.retention_days;
        }
        "presentation.status_enabled" => {
            candidate.presentation.status_enabled = defaults.presentation.status_enabled;
        }
        "presentation.status_compact" => {
            candidate.presentation.status_compact = defaults.presentation.status_compact;
        }
        "presentation.ambient_enabled" => {
            candidate.presentation.ambient_enabled = defaults.presentation.ambient_enabled;
        }
        "presentation.ambient_intensity" => {
            candidate.presentation.ambient_intensity = defaults.presentation.ambient_intensity;
        }
        "diagnostics.cpu" => candidate.diagnostics.cpu = defaults.diagnostics.cpu,
        "diagnostics.memory" => candidate.diagnostics.memory = defaults.diagnostics.memory,
        "diagnostics.duration" => candidate.diagnostics.duration = defaults.diagnostics.duration,
        "diagnostics.command_health" => {
            candidate.diagnostics.command_health = defaults.diagnostics.command_health;
        }
        "logging.level" => candidate.logging.level = defaults.logging.level,
        "integrations.agents" => candidate.integrations.agents = defaults.integrations.agents,
        "integrations.renderers" => {
            candidate.integrations.renderers = defaults.integrations.renderers;
        }
        _ => return Err(ConfigError::UnknownKey(key.to_owned())),
    }
    candidate.validate()?;
    *settings = candidate;
    Ok(())
}

fn set_key_inner(settings: &mut Settings, key: &str, value: &str) -> Result<(), ConfigError> {
    match key {
        "service.idle_timeout_seconds" => {
            settings.service.idle_timeout_seconds = parse_value(key, value)?;
        }
        "service.dashboard_port" => {
            settings.service.dashboard_port = if value == "auto" {
                DashboardPort::Auto
            } else {
                DashboardPort::Fixed(parse_value(key, value)?)
            };
        }
        "history.enabled" => settings.history.enabled = parse_value(key, value)?,
        "history.retention_days" => settings.history.retention_days = parse_value(key, value)?,
        "presentation.status_enabled" => {
            settings.presentation.status_enabled = parse_value(key, value)?;
        }
        "presentation.status_compact" => {
            settings.presentation.status_compact = parse_value(key, value)?;
        }
        "presentation.ambient_enabled" => {
            settings.presentation.ambient_enabled = parse_value(key, value)?;
        }
        "presentation.ambient_intensity" => {
            settings.presentation.ambient_intensity = parse_value(key, value)?;
        }
        "diagnostics.cpu" => settings.diagnostics.cpu = parse_value(key, value)?,
        "diagnostics.memory" => settings.diagnostics.memory = parse_value(key, value)?,
        "diagnostics.duration" => settings.diagnostics.duration = parse_value(key, value)?,
        "diagnostics.command_health" => {
            settings.diagnostics.command_health = parse_value(key, value)?;
        }
        "logging.level" => {
            settings.logging.level = match value {
                "error" => LogLevel::Error,
                "warn" => LogLevel::Warn,
                "info" => LogLevel::Info,
                "debug" => LogLevel::Debug,
                "trace" => LogLevel::Trace,
                _ => {
                    return Err(ConfigError::InvalidInput {
                        key: key.to_owned(),
                        reason: "expected error, warn, info, debug, or trace".to_owned(),
                    });
                }
            };
        }
        "integrations.agents" => settings.integrations.agents = parse_identifiers(value),
        "integrations.renderers" => settings.integrations.renderers = parse_identifiers(value),
        _ => return Err(ConfigError::UnknownKey(key.to_owned())),
    }
    Ok(())
}

fn parse_value<T>(key: &str, value: &str) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error: T::Err| ConfigError::InvalidInput {
            key: key.to_owned(),
            reason: error.to_string(),
        })
}

fn parse_identifiers(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn write_private_atomic(path: &Path, content: &[u8]) -> Result<(), ConfigError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp).map_err(|source| ConfigError::Write {
            path: temp.clone(),
            source,
        })?;
        file.write_all(content)
            .and_then(|_| file.sync_all())
            .map_err(|source| ConfigError::Write {
                path: temp.clone(),
                source,
            })?;
        std::fs::rename(&temp, path).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
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
