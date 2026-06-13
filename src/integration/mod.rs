mod apply;
pub mod json;
pub mod managed;

pub use apply::{ApplyError, apply_json_plan, apply_json_uninstall, apply_plan, apply_uninstall};

use crate::integration::managed::{Marker, insert_or_replace, remove};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

pub trait Validator: Send + Sync {
    fn validate(&self, target: &Path) -> Result<(), String>;
}

pub struct TextDescriptor {
    pub id: String,
    pub version: u32,
    pub target: PathBuf,
    pub marker: Marker,
    pub body: String,
    pub validator: Option<Arc<dyn Validator>>,
    placement: TextPlacement,
}

#[derive(Debug, Clone, Copy)]
enum TextPlacement {
    Append,
    Prepend,
}

impl TextDescriptor {
    pub fn new(
        id: impl Into<String>,
        version: u32,
        target: PathBuf,
        comment_prefix: impl Into<String>,
        body: impl Into<String>,
        validator: Option<Arc<dyn Validator>>,
    ) -> Result<Self, PlanError> {
        let id = id.into();
        let marker = Marker::new(id.clone(), comment_prefix).map_err(PlanError::Managed)?;
        if version == 0 {
            return Err(PlanError::InvalidDescriptor("version must be positive"));
        }
        Ok(Self {
            id,
            version,
            target,
            marker,
            body: body.into(),
            validator,
            placement: TextPlacement::Append,
        })
    }

    pub fn new_prepend(
        id: impl Into<String>,
        version: u32,
        target: PathBuf,
        comment_prefix: impl Into<String>,
        body: impl Into<String>,
        validator: Option<Arc<dyn Validator>>,
    ) -> Result<Self, PlanError> {
        let mut descriptor = Self::new(id, version, target, comment_prefix, body, validator)?;
        descriptor.placement = TextPlacement::Prepend;
        Ok(descriptor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanAction {
    Create,
    Update,
    NoChange,
}

pub struct IntegrationPlan {
    pub id: String,
    pub target: PathBuf,
    pub action: PlanAction,
    pub before_sha256: Option<String>,
    pub after_sha256: String,
    pub preview: String,
    pub(crate) proposed: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum PlanError {
    #[error("integration descriptor is invalid: {0}")]
    InvalidDescriptor(&'static str),
    #[error("could not read integration target {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("integration target {0} is not UTF-8 text")]
    NonUtf8(PathBuf),
    #[error("integration target {path} is not valid {format}")]
    InvalidStructured { path: PathBuf, format: &'static str },
    #[error("integration transform failed: {0}")]
    Transform(String),
    #[error(transparent)]
    Managed(#[from] managed::ManagedError),
}

pub fn plan_install(descriptor: &TextDescriptor) -> Result<IntegrationPlan, PlanError> {
    let before = match std::fs::read(&descriptor.target) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(PlanError::Read {
                path: descriptor.target.clone(),
                source,
            });
        }
    };
    let original = match before.as_deref() {
        Some(bytes) => {
            std::str::from_utf8(bytes).map_err(|_| PlanError::NonUtf8(descriptor.target.clone()))?
        }
        None => "",
    };
    let proposed = match descriptor.placement {
        TextPlacement::Append => insert_or_replace(original, &descriptor.marker, &descriptor.body)?,
        TextPlacement::Prepend => {
            managed::insert_or_replace_prepend(original, &descriptor.marker, &descriptor.body)?
        }
    }
    .into_bytes();
    let action = match &before {
        None => PlanAction::Create,
        Some(bytes) if bytes == &proposed => PlanAction::NoChange,
        Some(_) => PlanAction::Update,
    };
    Ok(IntegrationPlan {
        id: descriptor.id.clone(),
        target: descriptor.target.clone(),
        action,
        before_sha256: before.as_deref().map(sha256_hex),
        after_sha256: sha256_hex(&proposed),
        preview: bounded_preview(&proposed),
        proposed,
    })
}

pub fn plan_uninstall(descriptor: &TextDescriptor) -> Result<IntegrationPlan, PlanError> {
    let before = std::fs::read(&descriptor.target).map_err(|source| PlanError::Read {
        path: descriptor.target.clone(),
        source,
    })?;
    let original =
        std::str::from_utf8(&before).map_err(|_| PlanError::NonUtf8(descriptor.target.clone()))?;
    let proposed = remove(original, &descriptor.marker)?.into_bytes();
    Ok(IntegrationPlan {
        id: descriptor.id.clone(),
        target: descriptor.target.clone(),
        action: if before == proposed {
            PlanAction::NoChange
        } else {
            PlanAction::Update
        },
        before_sha256: Some(sha256_hex(&before)),
        after_sha256: sha256_hex(&proposed),
        preview: bounded_preview(&proposed),
        proposed,
    })
}

pub(crate) fn sha256_hex(content: &[u8]) -> String {
    Sha256::digest(content)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn bounded_preview(content: &[u8]) -> String {
    const MAX_BYTES: usize = 4096;
    if content.len() <= MAX_BYTES {
        return String::from_utf8_lossy(content).into_owned();
    }
    let start = content.len() - MAX_BYTES;
    String::from_utf8_lossy(&content[start..]).into_owned()
}
