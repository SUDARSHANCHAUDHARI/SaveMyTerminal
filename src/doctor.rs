use crate::{
    client::ServiceClient, config, manifest::load_manifest, paths::AppPaths,
    service::ServiceDiscovery,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckLevel {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub id: &'static str,
    pub level: CheckLevel,
    pub message: String,
}

impl CheckResult {
    pub fn pass(id: &'static str, message: impl Into<String>) -> Self {
        Self {
            id,
            level: CheckLevel::Pass,
            message: message.into(),
        }
    }

    pub fn warn(id: &'static str, message: impl Into<String>) -> Self {
        Self {
            id,
            level: CheckLevel::Warn,
            message: message.into(),
        }
    }

    pub fn fail(id: &'static str, message: impl Into<String>) -> Self {
        Self {
            id,
            level: CheckLevel::Fail,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub checks: Vec<CheckResult>,
}

impl DoctorReport {
    pub fn from_checks(checks: Vec<CheckResult>) -> Self {
        Self { checks }
    }

    pub fn exit_code(&self) -> i32 {
        i32::from(
            self.checks
                .iter()
                .any(|check| check.level == CheckLevel::Fail),
        )
    }
}

pub async fn run_checks(paths: &AppPaths) -> DoctorReport {
    let mut checks = Vec::new();
    match config::load(&paths.settings_file()) {
        Ok(_) => checks.push(CheckResult::pass("settings", "settings are valid")),
        Err(error) => checks.push(CheckResult::fail("settings", error.to_string())),
    }

    check_permissions(&mut checks, "config_permissions", &paths.config_dir, true);
    check_permissions(&mut checks, "token_permissions", &paths.token_file(), false);
    check_permissions(
        &mut checks,
        "database_permissions",
        &paths.database_file(),
        false,
    );

    check_service(paths, &mut checks).await;
    check_manifest(paths, &mut checks);
    checks.push(CheckResult::pass(
        "network",
        "configuration contains no remote endpoint or telemetry setting",
    ));
    DoctorReport::from_checks(checks)
}

async fn check_service(paths: &AppPaths, checks: &mut Vec<CheckResult>) {
    let bytes = match std::fs::read(paths.discovery_file()) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            checks.push(CheckResult::pass(
                "service",
                "service is on demand and currently stopped",
            ));
            return;
        }
        Err(error) => {
            checks.push(CheckResult::fail(
                "service",
                format!("could not read discovery state: {error}"),
            ));
            return;
        }
    };
    let discovery: ServiceDiscovery = match serde_json::from_slice(&bytes) {
        Ok(discovery) => discovery,
        Err(error) => {
            checks.push(CheckResult::fail(
                "service",
                format!("discovery state is invalid: {error}"),
            ));
            return;
        }
    };
    let url = match reqwest::Url::parse(&discovery.base_url) {
        Ok(url) => url,
        Err(error) => {
            checks.push(CheckResult::fail(
                "service",
                format!("discovery URL is invalid: {error}"),
            ));
            return;
        }
    };
    if !url.host_str().is_some_and(is_loopback_host) {
        checks.push(CheckResult::fail(
            "service",
            "discovery URL is not loopback-only",
        ));
        return;
    }
    match ServiceClient::connect(paths).await {
        Ok(_) => checks.push(CheckResult::pass(
            "service",
            "local service is reachable on loopback",
        )),
        Err(error) => checks.push(CheckResult::fail(
            "service",
            format!("local service is unreachable: {error}"),
        )),
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn check_manifest(paths: &AppPaths, checks: &mut Vec<CheckResult>) {
    let manifest_path = paths.manifest_file();
    let exists = manifest_path.exists();
    let manifest = match load_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            checks.push(CheckResult::fail("manifest", error.to_string()));
            return;
        }
    };
    checks.push(CheckResult::pass(
        "manifest",
        if exists {
            "integration manifest is valid"
        } else {
            "no managed integrations are recorded"
        },
    ));
    for record in manifest.integrations {
        let bytes = match std::fs::read(&record.target_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                checks.push(CheckResult::fail(
                    "manifest_markers",
                    format!("managed target for {:?} is unavailable: {error}", record.id),
                ));
                continue;
            }
        };
        let ownership_intact = if let Some(agent) = record.marker_id.strip_prefix("json-") {
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .is_some_and(|value| json_contains_command(&value, &format!("savemyterminal hook {agent}")))
        } else {
            let text = String::from_utf8_lossy(&bytes);
            let begin = format!(">>> SaveMyTerminal:{} >>>", record.marker_id);
            let end = format!("<<< SaveMyTerminal:{} <<<", record.marker_id);
            text.matches(&begin).count() == 1 && text.matches(&end).count() == 1
        };
        if ownership_intact {
            checks.push(CheckResult::pass(
                "manifest_markers",
                format!("managed ownership for {:?} is intact", record.id),
            ));
        } else {
            checks.push(CheckResult::fail(
                "manifest_markers",
                format!(
                    "managed ownership for {:?} is missing or ambiguous",
                    record.id
                ),
            ));
        }
        let checksum = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if checksum == record.post_write_sha256 {
            checks.push(CheckResult::pass(
                "manifest_checksum",
                format!(
                    "managed target for {:?} matches its recorded checksum",
                    record.id
                ),
            ));
        } else {
            checks.push(CheckResult::warn(
                "manifest_checksum",
                format!("managed target for {:?} changed after setup", record.id),
            ));
        }
        if let Some(backup) = record.backup_path {
            if backup.exists() {
                checks.push(CheckResult::pass(
                    "manifest_backup",
                    format!("backup for {:?} is available", record.id),
                ));
            } else {
                checks.push(CheckResult::warn(
                    "manifest_backup",
                    format!("backup for {:?} is missing", record.id),
                ));
            }
        }
    }
}

fn json_contains_command(value: &serde_json::Value, command: &str) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            object.get("command").and_then(serde_json::Value::as_str) == Some(command)
                || object
                    .values()
                    .any(|value| json_contains_command(value, command))
        }
        serde_json::Value::Array(items) => items
            .iter()
            .any(|value| json_contains_command(value, command)),
        _ => false,
    }
}

#[cfg(unix)]
fn check_permissions(
    checks: &mut Vec<CheckResult>,
    id: &'static str,
    path: &std::path::Path,
    directory: bool,
) {
    use std::os::unix::fs::PermissionsExt;

    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            checks.push(CheckResult::pass(id, "owned path does not exist yet"));
            return;
        }
        Err(error) => {
            checks.push(CheckResult::fail(
                id,
                format!("could not inspect permissions: {error}"),
            ));
            return;
        }
    };
    let mode = metadata.permissions().mode() & 0o777;
    let allowed = mode & 0o077 == 0;
    if allowed {
        checks.push(CheckResult::pass(
            id,
            format!("permissions are private ({mode:o})"),
        ));
    } else if directory {
        checks.push(CheckResult::warn(
            id,
            format!("directory permissions are broader than recommended ({mode:o})"),
        ));
    } else {
        checks.push(CheckResult::fail(
            id,
            format!("sensitive file permissions are not private ({mode:o})"),
        ));
    }
}

#[cfg(not(unix))]
fn check_permissions(
    checks: &mut Vec<CheckResult>,
    id: &'static str,
    path: &std::path::Path,
    _directory: bool,
) {
    checks.push(CheckResult::pass(
        id,
        if path.exists() {
            "owned path is accessible"
        } else {
            "owned path does not exist yet"
        },
    ));
}
