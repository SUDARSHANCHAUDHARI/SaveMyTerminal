use super::{IntegrationPlan, PlanAction, TextDescriptor, sha256_hex};
use crate::manifest::{IntegrationRecord, load_manifest, save_manifest_atomic};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("integration plan does not match its descriptor")]
    DescriptorMismatch,
    #[error("integration target changed after preview")]
    StalePlan,
    #[error("could not read integration target {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write integration target {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("integration validation failed: {0}")]
    Validation(String),
    #[error(transparent)]
    Manifest(#[from] crate::manifest::ManifestError),
}

pub fn apply_plan(
    plan: &IntegrationPlan,
    descriptor: &TextDescriptor,
    manifest_path: &Path,
    backup_dir: &Path,
) -> Result<IntegrationRecord, ApplyError> {
    if plan.id != descriptor.id || plan.target != descriptor.target {
        return Err(ApplyError::DescriptorMismatch);
    }
    let original = match std::fs::read(&plan.target) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(ApplyError::Read {
                path: plan.target.clone(),
                source,
            });
        }
    };
    if original.as_deref().map(sha256_hex) != plan.before_sha256 {
        return Err(ApplyError::StalePlan);
    }

    let backup_path = if let Some(bytes) = &original {
        std::fs::create_dir_all(backup_dir).map_err(|source| ApplyError::Write {
            path: backup_dir.to_path_buf(),
            source,
        })?;
        let backup = backup_dir.join(format!(
            "{}-{}-{}.bak",
            descriptor.id,
            now_ms(),
            &sha256_hex(bytes)[..8]
        ));
        write_atomic(&backup, bytes)?;
        Some(backup)
    } else {
        None
    };

    if plan.action != PlanAction::NoChange {
        write_atomic(&plan.target, &plan.proposed)?;
    }
    let validation = descriptor
        .validator
        .as_ref()
        .map(|validator| validator.validate(&plan.target))
        .transpose();
    if let Err(message) = validation {
        rollback(&plan.target, original.as_deref())?;
        return Err(ApplyError::Validation(message));
    }

    let record = IntegrationRecord {
        id: descriptor.id.clone(),
        descriptor_version: descriptor.version,
        target_path: descriptor.target.clone(),
        marker_id: descriptor.id.clone(),
        backup_path,
        post_write_sha256: plan.after_sha256.clone(),
        applied_at_unix_ms: now_ms(),
    };
    let mut manifest = load_manifest(manifest_path)?;
    manifest
        .integrations
        .retain(|existing| existing.id != record.id);
    manifest.integrations.push(record.clone());
    if let Err(error) = save_manifest_atomic(manifest_path, &manifest) {
        rollback(&plan.target, original.as_deref())?;
        return Err(error.into());
    }
    Ok(record)
}

pub fn apply_uninstall(
    plan: &IntegrationPlan,
    descriptor: &TextDescriptor,
    manifest_path: &Path,
    backup_dir: &Path,
) -> Result<(), ApplyError> {
    if plan.id != descriptor.id || plan.target != descriptor.target {
        return Err(ApplyError::DescriptorMismatch);
    }
    let original = std::fs::read(&plan.target).map_err(|source| ApplyError::Read {
        path: plan.target.clone(),
        source,
    })?;
    if Some(sha256_hex(&original)) != plan.before_sha256 {
        return Err(ApplyError::StalePlan);
    }

    std::fs::create_dir_all(backup_dir).map_err(|source| ApplyError::Write {
        path: backup_dir.to_path_buf(),
        source,
    })?;
    let backup = backup_dir.join(format!(
        "{}-{}-{}.bak",
        descriptor.id,
        now_ms(),
        &sha256_hex(&original)[..8]
    ));
    write_atomic(&backup, &original)?;
    write_atomic(&plan.target, &plan.proposed)?;

    let validation = descriptor
        .validator
        .as_ref()
        .map(|validator| validator.validate(&plan.target))
        .transpose();
    if let Err(message) = validation {
        write_atomic(&plan.target, &original)?;
        return Err(ApplyError::Validation(message));
    }

    let mut manifest = load_manifest(manifest_path)?;
    manifest
        .integrations
        .retain(|record| record.id != descriptor.id);
    if let Err(error) = save_manifest_atomic(manifest_path, &manifest) {
        write_atomic(&plan.target, &original)?;
        return Err(error.into());
    }
    Ok(())
}

fn rollback(target: &Path, original: Option<&[u8]>) -> Result<(), ApplyError> {
    match original {
        Some(bytes) => write_atomic(target, bytes),
        None => match std::fs::remove_file(target) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(ApplyError::Write {
                path: target.to_path_buf(),
                source,
            }),
        },
    }
}

fn write_atomic(path: &Path, content: &[u8]) -> Result<(), ApplyError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|source| ApplyError::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    let temp = path.with_extension(format!("smt-tmp-{}", std::process::id()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp).map_err(|source| ApplyError::Write {
            path: temp.clone(),
            source,
        })?;
        file.write_all(content)
            .and_then(|_| file.sync_all())
            .map_err(|source| ApplyError::Write {
                path: temp.clone(),
                source,
            })?;
        std::fs::rename(&temp, path).map_err(|source| ApplyError::Write {
            path: path.to_path_buf(),
            source,
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}
