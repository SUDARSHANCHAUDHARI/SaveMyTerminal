use crate::cli::{Cli, Command};
use clap::Parser;

pub async fn run() -> anyhow::Result<i32> {
    match Cli::parse().command {
        Command::Run(args) => {
            let mut renderer = crate::renderer::PlainRenderer::stderr(!args.no_status);
            crate::runner::run(args.command, &mut renderer).await
        }
        Command::Service(args) => {
            let discovered = crate::paths::AppPaths::discover()?;
            let paths = crate::paths::AppPaths {
                config_dir: args.config_dir.unwrap_or(discovered.config_dir),
                runtime_dir: args.runtime_dir.unwrap_or(discovered.runtime_dir),
                data_dir: discovered.data_dir,
            };
            let token = crate::auth::load_or_create_token(&paths.token_file())?;
            let service = crate::service::spawn_service(crate::service::ServiceConfig {
                token,
                discovery_file: Some(paths.discovery_file()),
                lock_file: Some(paths.runtime_dir.join("service.lock")),
                idle_timeout: std::time::Duration::from_millis(args.idle_timeout_ms),
            })
            .await?;
            service.finished().await?;
            Ok(0)
        }
        Command::Status => {
            let paths = crate::paths::AppPaths::discover()?;
            match crate::client::ServiceClient::connect(&paths).await {
                Ok(client) => {
                    println!("running {}", client.base_url());
                    Ok(0)
                }
                Err(error) => {
                    eprintln!("unavailable: {error}");
                    Ok(1)
                }
            }
        }
    }
}
