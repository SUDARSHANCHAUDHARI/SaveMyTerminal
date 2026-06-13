use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "smt", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run any command with SaveMyTerminal metadata reporting.
    Run(RunArgs),
    /// Run the local metadata service. Normally started automatically.
    Service(ServiceArgs),
    /// Open the authenticated local dashboard.
    Dashboard,
    /// Report whether the local service is reachable.
    Status,
    /// Receive privacy-safe lifecycle metadata from a supported agent hook.
    Hook(HookArgs),
    /// Detect local capabilities and preview or apply setup changes.
    Setup(SetupArgs),
    /// Inspect or change per-user settings.
    Config(ConfigArgs),
    /// Diagnose local configuration and integration health.
    Doctor(DoctorArgs),
    /// Preview or remove SaveMyTerminal-managed integrations.
    Uninstall(UninstallArgs),
}

#[derive(Debug, Args)]
pub struct HookArgs {
    #[arg(value_enum)]
    pub agent: crate::adapter::NativeAgent,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Disable concise SaveMyTerminal status messages.
    #[arg(long)]
    pub no_status: bool,
    /// Override the config directory. Intended for tests.
    #[arg(long, hide = true)]
    pub config_dir: Option<PathBuf>,
    /// Command and arguments to execute.
    #[arg(
        required = true,
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "command"
    )]
    pub command: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ServiceArgs {
    /// Override the config directory. Intended for tests.
    #[arg(long, hide = true)]
    pub config_dir: Option<PathBuf>,
    /// Override the runtime directory. Intended for tests.
    #[arg(long, hide = true)]
    pub runtime_dir: Option<PathBuf>,
    /// Override the data directory. Intended for tests.
    #[arg(long, hide = true)]
    pub data_dir: Option<PathBuf>,
    /// Override idle shutdown. Intended for tests.
    #[arg(long, hide = true)]
    pub idle_timeout_ms: Option<u64>,
}

#[derive(Debug, Args, Default)]
pub struct PathOverrides {
    /// Override the config directory. Intended for tests.
    #[arg(long, hide = true)]
    pub config_dir: Option<PathBuf>,
    /// Override the runtime directory. Intended for tests.
    #[arg(long, hide = true)]
    pub runtime_dir: Option<PathBuf>,
    /// Override the data directory. Intended for tests.
    #[arg(long, hide = true)]
    pub data_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(flatten)]
    pub paths: PathOverrides,
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the effective normalized settings.
    Show,
    /// Print the settings file path without creating it.
    Path,
    /// Set one documented dotted key.
    Set { key: String, value: String },
    /// Preview or apply resetting one key or all settings.
    Reset {
        key: Option<String>,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    #[command(flatten)]
    pub paths: PathOverrides,
    /// Apply the displayed setup plan.
    #[arg(long)]
    pub apply: bool,
    /// Limit setup to one or more registered integration identifiers.
    #[arg(long = "integration")]
    pub integrations: Vec<String>,
    /// Override the home directory. Intended for tests.
    #[arg(long, hide = true)]
    pub home_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[command(flatten)]
    pub paths: PathOverrides,
}

#[derive(Debug, Args)]
pub struct UninstallArgs {
    #[command(flatten)]
    pub paths: PathOverrides,
    /// Apply the displayed removal plan.
    #[arg(long)]
    pub apply: bool,
    /// Limit removal to one or more managed integration identifiers.
    #[arg(long = "integration")]
    pub integrations: Vec<String>,
    /// Remove SaveMyTerminal-owned settings and authentication state.
    #[arg(long)]
    pub remove_config: bool,
    /// Remove privacy-safe session history.
    #[arg(long)]
    pub purge_data: bool,
    /// Override the home directory. Intended for tests.
    #[arg(long, hide = true)]
    pub home_dir: Option<PathBuf>,
}
