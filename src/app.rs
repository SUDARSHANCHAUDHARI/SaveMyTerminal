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
            let ambient_terminal = !crate::detection::detect().terminals.is_empty();
            let mut renderer = crate::renderer::HybridRenderer::stderr(
                !args.no_status && settings.presentation.status_enabled,
                settings.presentation.ambient_enabled && ambient_terminal,
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
        Command::Snapshot(args) => {
            let paths = resolve_paths(args.paths)?;
            let settings = crate::config::load(&paths.settings_file()).unwrap_or_default();
            let sessions = match crate::client::ServiceClient::connect(&paths).await {
                Ok(client) => client.active_sessions().await.unwrap_or_default(),
                Err(_) => Vec::new(),
            };
            let view = crate::renderer::SnapshotView::from_sessions(
                &sessions,
                settings.presentation.ambient_intensity,
            );
            match args.format {
                crate::cli::SnapshotFormat::Text => println!("{}", view.label),
                crate::cli::SnapshotFormat::Json => println!("{}", serde_json::to_string(&view)?),
            }
            Ok(0)
        }
        Command::Hook(args) => {
            run_native_hook(args.agent).await;
            println!("{{}}");
            Ok(0)
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
            let report = crate::detection::detect();
            println!("detected os: {:?}", report.os);
            println!("detected shell: {:?}", report.shell);
            println!("detected agents: {:?}", report.agents);
            println!("detected terminals: {:?}", report.terminals);
            let home = resolve_home(args.home_dir)?;
            let agent_descriptors = crate::agents::descriptors(&home);
            let terminal_descriptors = crate::terminals::descriptors(&home, &paths, report.os);
            let defaults = detected_integration_ids(&report);
            let available = agent_descriptors
                .iter()
                .map(|item| item.id.as_str())
                .chain(terminal_descriptors.iter().filter_map(|item| {
                    terminal_supported(&item.id, report.os).then_some(item.id.as_str())
                }))
                .collect::<Vec<_>>();
            let selected_ids = select_integration_ids(&args.integrations, &defaults, &available)?;
            let selected_agents = agent_descriptors
                .iter()
                .filter(|item| selected_ids.contains(&item.id.as_str()))
                .collect::<Vec<_>>();
            let selected_terminals = terminal_descriptors
                .iter()
                .filter(|item| selected_ids.contains(&item.id.as_str()))
                .collect::<Vec<_>>();
            let agent_plans = selected_agents
                .iter()
                .map(|descriptor| {
                    crate::integration::json::plan_install(descriptor)
                        .map(|plan| (*descriptor, plan))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let terminal_plans = selected_terminals
                .iter()
                .map(|descriptor| {
                    crate::integration::plan_install(descriptor).map(|plan| (*descriptor, plan))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !selected_terminals.is_empty() {
                println!(
                    "assets: {} and {}",
                    crate::terminal_assets::ambient_path(&paths).display(),
                    crate::terminal_assets::shader_path(&paths).display()
                );
            }
            for (descriptor, plan) in &agent_plans {
                println!(
                    "integration {}: {:?} {}",
                    descriptor.id,
                    plan.action,
                    descriptor.target.display()
                );
                print!("{}", plan.preview);
            }
            for (descriptor, plan) in &terminal_plans {
                println!(
                    "integration {}: {:?} {}",
                    descriptor.id,
                    plan.action,
                    descriptor.target.display()
                );
                print!("{}", plan.preview);
            }
            if args.apply {
                if !terminal_plans.is_empty() {
                    crate::terminal_assets::install(&paths)?;
                }
                for (descriptor, plan) in &agent_plans {
                    crate::integration::apply_json_plan(
                        plan,
                        descriptor,
                        &paths.manifest_file(),
                        &paths.backup_dir(),
                    )?;
                    println!("integration applied: {}", descriptor.id);
                }
                for (descriptor, plan) in &terminal_plans {
                    crate::integration::apply_plan(
                        plan,
                        descriptor,
                        &paths.manifest_file(),
                        &paths.backup_dir(),
                    )?;
                    println!("integration applied: {}", descriptor.id);
                }
            }
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
            let home = resolve_home(args.home_dir)?;
            let manifest = crate::manifest::load_manifest(&paths.manifest_file())?;
            let report = crate::detection::detect();
            let agent_descriptors = crate::agents::descriptors(&home);
            let terminal_descriptors = crate::terminals::descriptors(&home, &paths, report.os);
            let defaults = manifest
                .integrations
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>();
            let available = agent_descriptors
                .iter()
                .map(|item| item.id.as_str())
                .chain(terminal_descriptors.iter().map(|item| item.id.as_str()))
                .collect::<Vec<_>>();
            let selected_ids = select_integration_ids(&args.integrations, &defaults, &available)?;
            for descriptor in agent_descriptors
                .iter()
                .filter(|item| selected_ids.contains(&item.id.as_str()))
            {
                let plan = crate::integration::json::plan_uninstall(descriptor)?;
                println!(
                    "integration {}: {:?} {}",
                    descriptor.id,
                    plan.action,
                    descriptor.target.display()
                );
                print!("{}", plan.preview);
                if args.apply {
                    crate::integration::apply_json_uninstall(
                        &plan,
                        descriptor,
                        &paths.manifest_file(),
                        &paths.backup_dir(),
                    )?;
                    println!("integration removed: {}", descriptor.id);
                }
            }
            for descriptor in terminal_descriptors
                .iter()
                .filter(|item| selected_ids.contains(&item.id.as_str()))
            {
                let plan = crate::integration::plan_uninstall(descriptor)?;
                println!(
                    "integration {}: {:?} {}",
                    descriptor.id,
                    plan.action,
                    descriptor.target.display()
                );
                print!("{}", plan.preview);
                if args.apply {
                    crate::integration::apply_uninstall(
                        &plan,
                        descriptor,
                        &paths.manifest_file(),
                        &paths.backup_dir(),
                    )?;
                    println!("integration removed: {}", descriptor.id);
                }
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

            let remaining = crate::manifest::load_manifest(&paths.manifest_file())?;
            if !remaining
                .integrations
                .iter()
                .any(|record| is_renderer_id(&record.id))
            {
                crate::terminal_assets::uninstall(&paths)?;
            }

            if args.remove_config {
                let manifest = crate::manifest::load_manifest(&paths.manifest_file())?;
                if !manifest.integrations.is_empty() {
                    bail!("managed integrations must be removed before configuration state");
                }
            }

            remove_if_exists(&paths.discovery_file())?;
            remove_if_exists(&paths.runtime_dir.join("service.lock"))?;
            if args.remove_config {
                remove_if_exists(&paths.settings_file())?;
                remove_if_exists(&paths.token_file())?;
                remove_if_exists(&paths.manifest_file())?;
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

fn resolve_home(override_path: Option<std::path::PathBuf>) -> Result<std::path::PathBuf> {
    if let Some(path) = override_path {
        return Ok(path);
    }
    directories::BaseDirs::new()
        .map(|directories| directories.home_dir().to_path_buf())
        .context("home directory is unavailable")
}

fn select_integration_ids<'a>(
    requested: &'a [String],
    defaults: &[&'a str],
    available: &[&'a str],
) -> Result<Vec<&'a str>> {
    let selected_ids = if requested.is_empty() {
        defaults.to_vec()
    } else {
        requested.iter().map(String::as_str).collect()
    };
    for id in &selected_ids {
        if !available.contains(id) {
            bail!("integration {id:?} is not available on this system");
        }
    }
    Ok(selected_ids)
}

fn detected_integration_ids(report: &crate::detection::EnvironmentReport) -> Vec<&'static str> {
    report
        .agents
        .iter()
        .map(|agent| match agent {
            crate::detection::AgentId::Codex => "codex",
            crate::detection::AgentId::Claude => "claude",
            crate::detection::AgentId::Gemini => "gemini",
        })
        .chain(report.terminals.iter().map(|terminal| match terminal {
            crate::detection::TerminalId::Ghostty => "ghostty",
            crate::detection::TerminalId::Kitty => "kitty",
            crate::detection::TerminalId::Wezterm => "wezterm",
            crate::detection::TerminalId::Iterm2 => "iterm2",
        }))
        .collect()
}

fn terminal_supported(id: &str, os: crate::detection::OsId) -> bool {
    match id {
        "ghostty" | "kitty" => matches!(
            os,
            crate::detection::OsId::Macos | crate::detection::OsId::Linux
        ),
        "wezterm" => !matches!(os, crate::detection::OsId::Other),
        "iterm2" => os == crate::detection::OsId::Macos,
        _ => false,
    }
}

fn is_renderer_id(id: &str) -> bool {
    matches!(id, "ghostty" | "kitty" | "wezterm" | "iterm2")
}

async fn run_native_hook(agent: crate::adapter::NativeAgent) {
    use std::io::Read;

    let mut input = Vec::new();
    if std::io::stdin()
        .take((crate::adapter::MAX_HOOK_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut input)
        .is_err()
    {
        return;
    }
    let Ok(Some(event)) = crate::adapter::map_hook(agent, &input) else {
        return;
    };
    let Ok(paths) = crate::paths::AppPaths::discover() else {
        return;
    };
    let Ok(client) = crate::client::ServiceClient::ensure(&paths).await else {
        return;
    };
    if client.send(&event).await.is_ok()
        || matches!(event.kind, crate::protocol::EventKind::Started)
    {
        return;
    }
    let started = crate::protocol::Event::new(
        event.session_id,
        event.adapter_id.clone(),
        event.agent_id.clone(),
        crate::protocol::EventKind::Started,
    );
    if client.send(&started).await.is_ok() {
        let _ = client.send(&event).await;
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
