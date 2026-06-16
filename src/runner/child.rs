use anyhow::{Context, Result};
use std::{ffi::OsString, process::ExitStatus};
use tokio::process::Command;

pub fn spawn_inherited(
    command: &[String],
    attached_session_id: uuid::Uuid,
) -> Result<tokio::process::Child> {
    let (program, args) = command.split_first().context("command is required")?;
    Command::new(OsString::from(program))
        .args(args)
        .env(
            crate::adapter::ATTACHED_SESSION_ENV,
            attached_session_id.to_string(),
        )
        .spawn()
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
