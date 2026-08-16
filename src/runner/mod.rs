mod child;
mod metrics;

use crate::{
    client::ServiceClient,
    paths::AppPaths,
    protocol::{Event, EventKind, FailureCategory},
    renderer::Renderer,
};
use anyhow::Result;
use uuid::Uuid;

pub struct RunOptions {
    pub paths: AppPaths,
    pub cpu_diagnostics: bool,
    pub memory_diagnostics: bool,
    pub ambient_intensity: u8,
    pub session_id: Option<Uuid>,
}

pub async fn run(command: Vec<String>, renderer: &mut dyn Renderer) -> Result<i32> {
    let paths = AppPaths::discover()?;
    run_with_options(
        command,
        renderer,
        RunOptions {
            paths,
            cpu_diagnostics: true,
            memory_diagnostics: true,
            ambient_intensity: 60,
            session_id: None,
        },
    )
    .await
}

pub async fn run_with_options(
    command: Vec<String>,
    renderer: &mut dyn Renderer,
    options: RunOptions,
) -> Result<i32> {
    let agent_id = command
        .first()
        .map(|program| identify_agent(program))
        .unwrap_or("unknown")
        .to_owned();
    let session_id = options.session_id.unwrap_or_else(Uuid::new_v4);

    let client = if std::env::var_os("SAVEMYTERMINAL_TEST_FORCE_SERVICE_FAILURE").is_some() {
        None
    } else {
        match ServiceClient::ensure(&options.paths).await {
            Ok(client) => Some(client),
            Err(error) => {
                renderer.warning(&format!("observability unavailable: {error}"));
                None
            }
        }
    };

    renderer.started(&agent_id);
    if let Some(client) = &client {
        let _ = client
            .send(&Event::new(
                session_id,
                "generic",
                &agent_id,
                EventKind::Started,
            ))
            .await;
    }

    let mut child = child::spawn_inherited(&command, session_id)?;
    let pid = child.id().unwrap_or_default();
    let diagnostics_enabled = options.cpu_diagnostics || options.memory_diagnostics;
    let status = if let Some(client) = &client {
        let mut presentation_interval =
            tokio::time::interval(std::time::Duration::from_millis(100));
        let mut metrics_interval = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            tokio::select! {
                status = child.wait() => break status?,
                _ = presentation_interval.tick() => {
                    if let Ok(sessions) = client.active_sessions().await {
                        let attached = sessions
                            .into_iter()
                            .filter(|session| session.session_id == session_id)
                            .collect::<Vec<_>>();
                        let view = crate::renderer::SnapshotView::from_sessions(
                            &attached,
                            options.ambient_intensity,
                        );
                        renderer.snapshot(&view);
                    }
                }
                _ = metrics_interval.tick(), if diagnostics_enabled => {
                    let metrics = metrics::sample_once(pid).await;
                    let _ = client.send(&Event::new(
                        session_id,
                        "generic",
                        &agent_id,
                        EventKind::Metrics {
                            cpu_percent: options.cpu_diagnostics.then_some(metrics.cpu_percent).flatten(),
                            memory_bytes: options.memory_diagnostics.then_some(metrics.memory_bytes).flatten(),
                        },
                    )).await;
                }
            }
        }
    } else {
        child.wait().await?
    };
    let code = child::exit_code(status);
    let kind = if code == 0 {
        EventKind::Completed { exit_code: code }
    } else {
        EventKind::Failed {
            exit_code: code,
            category: FailureCategory::ProcessExit,
        }
    };
    if let Some(client) = &client {
        let _ = client
            .send(&Event::new(session_id, "generic", &agent_id, kind))
            .await;
    }
    renderer.finished(&agent_id, code);
    Ok(code)
}

fn identify_agent(program: &str) -> &'static str {
    let name = std::path::Path::new(program)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match name.as_str() {
        "codex" => "codex",
        "claude" => "claude",
        "gemini" => "gemini",
        "aider" => "aider",
        "opencode" => "opencode",
        _ => "unknown",
    }
}
