use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationManifest {
    pub version: u32,
    pub integrations: Vec<IntegrationRecord>,
}

impl Default for IntegrationManifest {
    fn default() -> Self {
        Self {
            version: MANIFEST_VERSION,
            integrations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationRecord {
    pub id: String,
    pub descriptor_version: u32,
    pub target_path: PathBuf,
    pub marker_id: String,
    pub backup_path: Option<PathBuf>,
    pub post_write_sha256: String,
    pub applied_at_unix_ms: u64,
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("could not read integration manifest at {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not parse integration manifest at {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("integration manifest is invalid: {0}")]
    Invalid(String),
    #[error("could not serialize integration manifest: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("could not write integration manifest at {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub fn load_manifest(path: &Path) -> Result<IntegrationManifest, ManifestError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(IntegrationManifest::default());
        }
        Err(source) => {
            return Err(ManifestError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let mut manifest: IntegrationManifest =
        serde_json::from_slice(&bytes).map_err(|source| ManifestError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    validate(&manifest)?;
    manifest
        .integrations
        .sort_by(|left, right| left.id.cmp(&right.id));
    Ok(manifest)
}

pub fn save_manifest_atomic(
    path: &Path,
    manifest: &IntegrationManifest,
) -> Result<(), ManifestError> {
    let mut normalized = manifest.clone();
    validate(&normalized)?;
    normalized
        .integrations
        .sort_by(|left, right| left.id.cmp(&right.id));
    let mut bytes = serde_json::to_vec_pretty(&normalized)?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

fn validate(manifest: &IntegrationManifest) -> Result<(), ManifestError> {
    if manifest.version != MANIFEST_VERSION {
        return Err(ManifestError::Invalid(format!(
            "unsupported version {}",
            manifest.version
        )));
    }
    let mut ids = HashSet::new();
    let mut ownership = HashSet::new();
    for record in &manifest.integrations {
        if !valid_identifier(&record.id) || !valid_identifier(&record.marker_id) {
            return Err(ManifestError::Invalid(
                "integration and marker identifiers must use lowercase ASCII names".to_owned(),
            ));
        }
        if record.descriptor_version == 0 {
            return Err(ManifestError::Invalid(
                "descriptor versions must be positive".to_owned(),
            ));
        }
        if !ids.insert(&record.id) {
            return Err(ManifestError::Invalid(
                "duplicate integration identifier".to_owned(),
            ));
        }
        if !ownership.insert((&record.target_path, &record.marker_id)) {
            return Err(ManifestError::Invalid(
                "duplicate target marker ownership".to_owned(),
            ));
        }
        if record.post_write_sha256.len() != 64
            || !record
                .post_write_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ManifestError::Invalid(format!(
                "invalid checksum for {:?}",
                record.id
            )));
        }
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
}

fn write_atomic(path: &Path, content: &[u8]) -> Result<(), ManifestError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|source| ManifestError::Write {
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
        let mut file = options.open(&temp).map_err(|source| ManifestError::Write {
            path: temp.clone(),
            source,
        })?;
        file.write_all(content)
            .and_then(|_| file.sync_all())
            .map_err(|source| ManifestError::Write {
                path: temp.clone(),
                source,
            })?;
        std::fs::rename(&temp, path).map_err(|source| ManifestError::Write {
            path: path.to_path_buf(),
            source,
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
