use super::{IntegrationPlan, PlanAction, PlanError, Validator, bounded_preview, sha256_hex};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

pub trait JsonTransform: Send + Sync {
    fn transform(&self, root: &mut Value) -> Result<(), String>;
}

impl<F> JsonTransform for F
where
    F: Fn(&mut Value) -> Result<(), String> + Send + Sync,
{
    fn transform(&self, root: &mut Value) -> Result<(), String> {
        self(root)
    }
}

pub struct JsonDescriptor {
    pub id: String,
    pub version: u32,
    pub target: PathBuf,
    pub install: Arc<dyn JsonTransform>,
    pub uninstall: Arc<dyn JsonTransform>,
    pub validator: Option<Arc<dyn Validator>>,
}

impl JsonDescriptor {
    pub fn new(
        id: impl Into<String>,
        version: u32,
        target: PathBuf,
        install: Arc<dyn JsonTransform>,
        uninstall: Arc<dyn JsonTransform>,
        validator: Option<Arc<dyn Validator>>,
    ) -> Result<Self, PlanError> {
        if version == 0 {
            return Err(PlanError::InvalidDescriptor("version must be positive"));
        }
        Ok(Self {
            id: id.into(),
            version,
            target,
            install,
            uninstall,
            validator,
        })
    }
}

pub fn plan_install(descriptor: &JsonDescriptor) -> Result<IntegrationPlan, PlanError> {
    plan(descriptor, &descriptor.install, true)
}

pub fn plan_uninstall(descriptor: &JsonDescriptor) -> Result<IntegrationPlan, PlanError> {
    plan(descriptor, &descriptor.uninstall, false)
}

fn plan(
    descriptor: &JsonDescriptor,
    transform: &Arc<dyn JsonTransform>,
    allow_missing: bool,
) -> Result<IntegrationPlan, PlanError> {
    let before = match std::fs::read(&descriptor.target) {
        Ok(bytes) => Some(bytes),
        Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(PlanError::Read {
                path: descriptor.target.clone(),
                source,
            });
        }
    };
    let mut root = match before.as_deref() {
        Some(bytes) => serde_json::from_slice(bytes).map_err(|_| PlanError::InvalidStructured {
            path: descriptor.target.clone(),
            format: "JSON",
        })?,
        None => Value::Object(Default::default()),
    };
    transform
        .transform(&mut root)
        .map_err(PlanError::Transform)?;
    let mut proposed =
        serde_json::to_vec_pretty(&root).map_err(|_| PlanError::InvalidStructured {
            path: descriptor.target.clone(),
            format: "JSON",
        })?;
    proposed.push(b'\n');
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
