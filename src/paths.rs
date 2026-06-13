use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let dirs = ProjectDirs::from("com", "SudarshanTechLabs", "SaveMyTerminal")
            .context("could not determine per-user application directories")?;
        Ok(Self {
            config_dir: dirs.config_dir().to_path_buf(),
            runtime_dir: dirs.cache_dir().join("runtime"),
            data_dir: dirs.data_dir().to_path_buf(),
        })
    }

    pub fn token_file(&self) -> PathBuf {
        self.config_dir.join("auth.token")
    }

    pub fn discovery_file(&self) -> PathBuf {
        self.runtime_dir.join("service.json")
    }

    pub fn database_file(&self) -> PathBuf {
        self.data_dir.join("sessions.sqlite3")
    }
}
