use crate::service::{
    DashboardAuth, SessionCoordinator, SessionRegistry,
    api::{ApiState, router},
};
use crate::storage::{HistoryStore, SqliteStore};
use anyhow::{Context, Result};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::watch, task::JoinHandle};

#[derive(Clone)]
pub struct ServiceConfig {
    pub token: SecretString,
    pub discovery_file: Option<PathBuf>,
    pub lock_file: Option<PathBuf>,
    pub database_file: Option<PathBuf>,
    pub dashboard_launch_ttl: Duration,
    pub history_retention: Duration,
    pub history_cleanup_interval: Duration,
    pub idle_timeout: Duration,
    pub listen_port: Option<u16>,
}

impl ServiceConfig {
    pub fn for_test(token: SecretString) -> Self {
        Self {
            token,
            discovery_file: None,
            lock_file: None,
            database_file: None,
            dashboard_launch_ttl: Duration::from_secs(60),
            history_retention: Duration::from_secs(30 * 24 * 60 * 60),
            history_cleanup_interval: Duration::from_secs(60 * 60),
            idle_timeout: Duration::from_secs(300),
            listen_port: None,
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
                let retention_ms = duration_ms(config.history_retention);
                let now_ms = unix_time_ms();
                let _ = store.cleanup_before(now_ms.saturating_sub(retention_ms));
                HistoryStore::available(store)
            }
            Err(error) => HistoryStore::unavailable(error.to_string()),
        },
        None => HistoryStore::unavailable("history is not configured"),
    };
    let coordinator = SessionCoordinator::new(SessionRegistry::default(), history);
    let discovery_file = config.discovery_file.clone();
    let listener = TcpListener::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        config.listen_port.unwrap_or(0),
    ))
    .await?;
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

    let dashboard_clients = Arc::new(AtomicUsize::new(0));
    let app = router(ApiState {
        coordinator: coordinator.clone(),
        token: config.token,
        dashboard_auth: DashboardAuth::new(config.dashboard_launch_ttl),
        base_url: base_url.clone(),
        dashboard_clients: dashboard_clients.clone(),
    });
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let idle_timeout = config.idle_timeout;
    let history_retention = config.history_retention;
    let history_cleanup_interval = config.history_cleanup_interval;
    let task = tokio::spawn(async move {
        let _service_lock = service_lock;
        let cleanup_history = coordinator.history_store();
        let cleanup_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(history_cleanup_interval);
            interval.tick().await;
            loop {
                interval.tick().await;
                let store = cleanup_history.clone();
                let cutoff = unix_time_ms().saturating_sub(duration_ms(history_retention));
                let _ = tokio::task::spawn_blocking(move || store.cleanup_before(cutoff)).await;
            }
        });
        let idle_coordinator = coordinator.clone();
        let idle = async move {
            loop {
                tokio::time::sleep(idle_timeout.min(Duration::from_millis(250))).await;
                if idle_coordinator.idle_for().await >= idle_timeout
                    && dashboard_clients.load(Ordering::Relaxed) == 0
                {
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
        cleanup_task.abort();
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

fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
