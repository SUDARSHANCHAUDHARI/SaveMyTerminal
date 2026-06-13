use anyhow::{Context, Result};
use std::{ffi::OsString, process::ExitStatus};
use tokio::process::Command;

pub async fn run_inherited(command: &[String]) -> Result<ExitStatus> {
    let (program, args) = command.split_first().context("command is required")?;
    Command::new(OsString::from(program))
        .args(args)
        .status()
        .await
        .with_context(|| format!("failed to launch {program}"))
}

pub fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        128 + status.signal().unwrap_or(1)
    }
    #[cfg(windows)]
    {
        1
    }
}
