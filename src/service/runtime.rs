use crate::service::{
    SessionCoordinator, SessionRegistry,
    api::{ApiState, router},
};
use crate::storage::{HistoryStore, SqliteStore};
use anyhow::{Context, Result};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    time::Duration,
};
use tokio::{net::TcpListener, sync::watch, task::JoinHandle};

#[derive(Clone)]
pub struct ServiceConfig {
    pub token: SecretString,
    pub discovery_file: Option<PathBuf>,
    pub lock_file: Option<PathBuf>,
    pub database_file: Option<PathBuf>,
    pub idle_timeout: Duration,
}

impl ServiceConfig {
    pub fn for_test(token: SecretString) -> Self {
        Self {
            token,
            discovery_file: None,
            lock_file: None,
            database_file: None,
            idle_timeout: Duration::from_secs(300),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceDiscovery {
    pub base_url: String,
    pub pid: u32,
}

pub struct RunningService {
    pub base_url: String,
    shutdown_tx: watch::Sender<bool>,
    task: JoinHandle<Result<()>>,
}

impl RunningService {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.task.await;
    }

    pub async fn finished(self) -> Result<()> {
        self.task.await?
    }
}

pub async fn spawn_test_service(config: ServiceConfig) -> Result<RunningService> {
    spawn_service(config).await
}

pub async fn spawn_service(config: ServiceConfig) -> Result<RunningService> {
    let service_lock = if let Some(path) = &config.lock_file {
        use fs2::FileExt;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        file.try_lock_exclusive()
            .context("another SaveMyTerminal service is already running")?;
        Some(file)
    } else {
        None
    };

    let history = match config.database_file.as_deref() {
        Some(path) => match SqliteStore::open(path) {
            Ok(store) => {
                let _ = store.recover_interrupted();
                let retention_ms = Duration::from_secs(30 * 24 * 60 * 60).as_millis() as u64;
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or(0);
                let _ = store.cleanup_before(now_ms.saturating_sub(retention_ms));
                HistoryStore::available(store)
            }
            Err(error) => HistoryStore::unavailable(error.to_string()),
        },
        None => HistoryStore::unavailable("history is not configured"),
    };
    let coordinator = SessionCoordinator::new(SessionRegistry::default(), history);
    let discovery_file = config.discovery_file.clone();
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).await?;
    let address = listener.local_addr()?;
    let base_url = format!("http://{address}");

    if let Some(path) = &config.discovery_file {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("tmp");
        let discovery = ServiceDiscovery {
            base_url: base_url.clone(),
            pid: std::process::id(),
        };
        std::fs::write(&temp, serde_json::to_vec(&discovery)?)?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        std::fs::rename(temp, path)?;
    }

    let app = router(ApiState {
        coordinator: coordinator.clone(),
        token: config.token,
    });
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let idle_timeout = config.idle_timeout;
    let task = tokio::spawn(async move {
        let _service_lock = service_lock;
        let idle_coordinator = coordinator.clone();
        let idle = async move {
            loop {
                tokio::time::sleep(idle_timeout.min(Duration::from_millis(250))).await;
                if idle_coordinator.idle_for().await >= idle_timeout {
                    break;
                }
            }
        };

        let result = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                tokio::select! {
                    _ = idle => {},
                    _ = shutdown_rx.changed() => {},
                }
            })
            .await;
        if let Some(path) = discovery_file {
            let _ = std::fs::remove_file(path);
        }
        result?;
        Ok(())
    });

    Ok(RunningService {
        base_url,
        shutdown_tx,
        task,
    })
}
