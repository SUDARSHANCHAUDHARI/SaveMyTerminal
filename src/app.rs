use crate::cli::{Cli, Command};
use clap::Parser;

pub async fn run() -> anyhow::Result<i32> {
    match Cli::parse().command {
        Command::Run(_) => Ok(0),
        Command::Service(_) => Ok(0),
        Command::Status => Ok(0),
    }
}
