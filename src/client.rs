use crate::{
    auth::{load_or_create_token, load_token},
    paths::AppPaths,
    protocol::Event,
    service::ServiceDiscovery,
};
use anyhow::{Context, Result, bail};
use secrecy::{ExposeSecret, SecretString};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::process::Command;

#[derive(Clone)]
pub struct ServiceClient {
    client: reqwest::Client,
    base_url: String,
    token: SecretString,
}

impl ServiceClient {
    pub async fn ensure(paths: &AppPaths) -> Result<Self> {
        let executable = std::env::current_exe()?;
        Self::ensure_with_executable(paths, &executable, Duration::from_secs(300)).await
    }

    pub async fn connect(paths: &AppPaths) -> Result<Self> {
        let token = load_token(&paths.token_file())?;
        Self::from_discovery(paths, token).await
    }

    pub async fn ensure_with_executable(
        paths: &AppPaths,
        executable: &Path,
        idle_timeout: Duration,
    ) -> Result<Self> {
        let token = load_or_create_token(&paths.token_file())?;
        if let Ok(client) = Self::from_discovery(paths, token.clone()).await {
            return Ok(client);
        }

        std::fs::create_dir_all(&paths.runtime_dir)?;
        let mut command = Command::new(executable);
        command
            .arg("service")
            .arg("--config-dir")
            .arg(&paths.config_dir)
            .arg("--runtime-dir")
            .arg(&paths.runtime_dir)
            .arg("--idle-timeout-ms")
            .arg(idle_timeout.as_millis().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(false);
        platform_detach(&mut command);
        command.spawn().context("failed to start local service")?;

        for _ in 0..40 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if let Ok(client) = Self::from_discovery(paths, token.clone()).await {
                return Ok(client);
            }
        }
        bail!("local service did not become ready")
    }

    async fn from_discovery(paths: &AppPaths, token: SecretString) -> Result<Self> {
        let discovery: ServiceDiscovery =
            serde_json::from_slice(&std::fs::read(paths.discovery_file())?)?;
        let client = Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(1))
                .build()?,
            base_url: discovery.base_url,
            token,
        };
        client.health().await?;
        Ok(client)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn health(&self) -> Result<()> {
        self.client
            .get(format!("{}/v1/health", self.base_url))
            .bearer_auth(self.token.expose_secret())
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn send(&self, event: &Event) -> Result<()> {
        self.client
            .post(format!("{}/v1/events", self.base_url))
            .bearer_auth(self.token.expose_secret())
            .json(event)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[cfg(unix)]
fn platform_detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.as_std_mut().process_group(0);
}

#[cfg(windows)]
fn platform_detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    command
        .as_std_mut()
        .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
}
