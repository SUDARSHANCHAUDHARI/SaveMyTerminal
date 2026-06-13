use crate::cli::{Cli, Command, ConfigCommand, PathOverrides};
use anyhow::{Context, Result, bail};
use clap::Parser;

pub trait BrowserOpener {
    fn open(&self, url: &str) -> Result<()>;
}

struct SystemBrowser;

impl BrowserOpener for SystemBrowser {
    fn open(&self, url: &str) -> Result<()> {
        webbrowser::open(url)
            .map(|_| ())
            .context("system browser launch failed")
    }
}

pub fn open_dashboard_url(opener: &dyn BrowserOpener, url: &str) -> Result<()> {
    opener
        .open(url)
        .with_context(|| format!("could not open browser; open this local URL manually:\n{url}"))
}

pub async fn run() -> Result<i32> {
    run_with_browser(&SystemBrowser).await
}

async fn run_with_browser(browser: &dyn BrowserOpener) -> Result<i32> {
    match Cli::parse().command {
        Command::Run(args) => {
            let discovered = crate::paths::AppPaths::discover()?;
            let paths = crate::paths::AppPaths {
                config_dir: args.config_dir.unwrap_or(discovered.config_dir),
                runtime_dir: discovered.runtime_dir,
                data_dir: discovered.data_dir,
            };
            let settings = match crate::config::load(&paths.settings_file()) {
                Ok(settings) => settings,
                Err(error) => {
                    eprintln!("smt warning: settings unavailable: {error}");
                    crate::config::Settings::default()
                }
            };
            let mut renderer = crate::renderer::PlainRenderer::stderr(
                !args.no_status && settings.presentation.status_enabled,
            );
            crate::runner::run_with_options(
                args.command,
                &mut renderer,
                crate::runner::RunOptions {
                    paths,
                    cpu_diagnostics: settings.diagnostics.cpu,
                    memory_diagnostics: settings.diagnostics.memory,
                },
            )
            .await
        }
        Command::Service(args) => {
            let discovered = crate::paths::AppPaths::discover()?;
            let paths = crate::paths::AppPaths {
                config_dir: args.config_dir.unwrap_or(discovered.config_dir),
                runtime_dir: args.runtime_dir.unwrap_or(discovered.runtime_dir),
                data_dir: args.data_dir.unwrap_or(discovered.data_dir),
            };
            let settings = crate::config::load(&paths.settings_file())?;
            let token = crate::auth::load_or_create_token(&paths.token_file())?;
            let service = crate::service::spawn_service(crate::service::ServiceConfig {
                token,
                discovery_file: Some(paths.discovery_file()),
                lock_file: Some(paths.runtime_dir.join("service.lock")),
                database_file: settings.history.enabled.then(|| paths.database_file()),
                dashboard_launch_ttl: std::time::Duration::from_secs(60),
                history_retention: settings.history_retention(),
                history_cleanup_interval: std::time::Duration::from_secs(60 * 60),
                idle_timeout: args
                    .idle_timeout_ms
                    .map(std::time::Duration::from_millis)
                    .unwrap_or_else(|| settings.idle_timeout()),
                listen_port: settings.service.dashboard_port.socket_port(),
            })
            .await?;
            service.finished().await?;
            Ok(0)
        }
        Command::Dashboard => {
            let paths = crate::paths::AppPaths::discover()?;
            let client = crate::client::ServiceClient::ensure(&paths).await?;
            let launch_url = client.dashboard_launch_url().await?;
            open_dashboard_url(browser, &launch_url)?;
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
        Command::Config(args) => {
            let paths = resolve_paths(args.paths)?;
            let settings_file = paths.settings_file();
            match args.command {
                ConfigCommand::Show => {
                    let settings = crate::config::load(&settings_file)?;
                    print!("{}", crate::config::normalized_toml(&settings)?);
                }
                ConfigCommand::Path => println!("{}", settings_file.display()),
                ConfigCommand::Set { key, value } => {
                    let mut settings = crate::config::load(&settings_file)?;
                    crate::config::set_key(&mut settings, &key, &value)?;
                    let backup = crate::config::save_with_backup(
                        &settings_file,
                        &paths.backup_dir(),
                        &settings,
                    )?;
                    println!("settings updated: {}", settings_file.display());
                    if let Some(backup) = backup {
                        println!("backup: {}", backup.display());
                    }
                }
                ConfigCommand::Reset { key, apply } => {
                    let mut settings = crate::config::load(&settings_file)?;
                    crate::config::reset_key(&mut settings, key.as_deref())?;
                    if apply {
                        let backup = crate::config::save_with_backup(
                            &settings_file,
                            &paths.backup_dir(),
                            &settings,
                        )?;
                        println!("settings updated: {}", settings_file.display());
                        if let Some(backup) = backup {
                            println!("backup: {}", backup.display());
                        }
                    } else {
                        println!("preview: settings reset");
                        print!("{}", crate::config::normalized_toml(&settings)?);
                    }
                }
            }
            Ok(0)
        }
        Command::Setup(args) => {
            let paths = resolve_paths(args.paths)?;
            if let Some(id) = args.integrations.first() {
                bail!("integration {id:?} is not registered in this phase");
            }
            let report = crate::detection::detect();
            println!("detected os: {:?}", report.os);
            println!("detected shell: {:?}", report.shell);
            println!("detected agents: {:?}", report.agents);
            println!("detected terminals: {:?}", report.terminals);
            let settings_file = paths.settings_file();
            if settings_file.exists() {
                crate::config::load(&settings_file)?;
                println!("settings already configured: {}", settings_file.display());
            } else if args.apply {
                crate::config::save_atomic(&settings_file, &crate::config::Settings::default())?;
                println!("settings created: {}", settings_file.display());
            } else {
                println!("preview: create settings at {}", settings_file.display());
            }
            Ok(0)
        }
        Command::Doctor(args) => {
            let paths = resolve_paths(args.paths)?;
            let report = crate::doctor::run_checks(&paths).await;
            let mut pass = 0;
            let mut warn = 0;
            let mut fail = 0;
            for check in &report.checks {
                let label = match check.level {
                    crate::doctor::CheckLevel::Pass => {
                        pass += 1;
                        "PASS"
                    }
                    crate::doctor::CheckLevel::Warn => {
                        warn += 1;
                        "WARN"
                    }
                    crate::doctor::CheckLevel::Fail => {
                        fail += 1;
                        "FAIL"
                    }
                };
                println!("{label} {}: {}", check.id, check.message);
            }
            println!("summary: {pass} pass, {warn} warn, {fail} fail");
            Ok(report.exit_code())
        }
        Command::Uninstall(args) => {
            let paths = resolve_paths(args.paths)?;
            if let Some(id) = args.integrations.first() {
                bail!("integration {id:?} is not registered in this phase");
            }
            if !args.apply {
                println!("preview: remove SaveMyTerminal-owned state");
                if args.remove_config {
                    println!("preview: remove settings and authentication token");
                }
                if args.purge_data {
                    println!("preview: purge privacy-safe session history");
                }
                return Ok(0);
            }

            remove_if_exists(&paths.discovery_file())?;
            remove_if_exists(&paths.runtime_dir.join("service.lock"))?;
            if args.remove_config {
                remove_if_exists(&paths.settings_file())?;
                remove_if_exists(&paths.token_file())?;
                remove_empty_dir(&paths.config_dir)?;
            }
            if args.purge_data {
                remove_if_exists(&paths.database_file())?;
            }
            println!("uninstall applied");
            Ok(0)
        }
    }
}

fn resolve_paths(overrides: PathOverrides) -> Result<crate::paths::AppPaths> {
    let discovered = crate::paths::AppPaths::discover()?;
    Ok(crate::paths::AppPaths {
        config_dir: overrides.config_dir.unwrap_or(discovered.config_dir),
        runtime_dir: overrides.runtime_dir.unwrap_or(discovered.runtime_dir),
        data_dir: overrides.data_dir.unwrap_or(discovered.data_dir),
    })
}

fn remove_if_exists(path: &std::path::Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("could not remove {}", path.display())),
    }
}

fn remove_empty_dir(path: &std::path::Path) -> Result<()> {
    match std::fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error).with_context(|| format!("could not remove {}", path.display())),
    }
}
