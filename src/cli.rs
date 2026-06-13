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
    /// Report whether the local service is reachable.
    Status,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Disable concise SaveMyTerminal status messages.
    #[arg(long)]
    pub no_status: bool,
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
    #[arg(long, default_value_t = 300_000, hide = true)]
    pub idle_timeout_ms: u64,
}
