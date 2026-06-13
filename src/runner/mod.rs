mod child;

use crate::{
    client::ServiceClient,
    paths::AppPaths,
    protocol::{Event, EventKind, FailureCategory},
    renderer::Renderer,
};
use anyhow::Result;
use uuid::Uuid;

pub async fn run(command: Vec<String>, renderer: &mut dyn Renderer) -> Result<i32> {
    let agent_id = command
        .first()
        .map(|program| identify_agent(program))
        .unwrap_or("unknown")
        .to_owned();
    let session_id = Uuid::new_v4();

    let client = if std::env::var_os("SMT_TEST_FORCE_SERVICE_FAILURE").is_some() {
        None
    } else {
        match AppPaths::discover() {
            Ok(paths) => match ServiceClient::ensure(&paths).await {
                Ok(client) => Some(client),
                Err(error) => {
                    renderer.warning(&format!("observability unavailable: {error}"));
                    None
                }
            },
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

    let status = child::run_inherited(&command).await?;
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
