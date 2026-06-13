use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsId {
    Macos,
    Linux,
    Windows,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellId {
    Zsh,
    Bash,
    Fish,
    Pwsh,
    Cmd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentId {
    Codex,
    Claude,
    Gemini,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalId {
    Ghostty,
    Kitty,
    Wezterm,
    Iterm2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentReport {
    pub os: OsId,
    pub shell: Option<ShellId>,
    pub agents: Vec<AgentId>,
    pub terminals: Vec<TerminalId>,
}

pub trait EnvironmentSource {
    fn variable(&self, name: &str) -> Option<OsString>;
    fn executable_exists(&self, name: &str) -> bool;
}

pub struct ProcessEnvironment;

impl EnvironmentSource for ProcessEnvironment {
    fn variable(&self, name: &str) -> Option<OsString> {
        std::env::var_os(name)
    }

    fn executable_exists(&self, name: &str) -> bool {
        let Some(path) = std::env::var_os("PATH") else {
            return false;
        };
        std::env::split_paths(&path).any(|directory| executable_in(&directory, name))
    }
}

pub fn detect() -> EnvironmentReport {
    detect_with(&ProcessEnvironment, current_os())
}

pub fn detect_with(source: &dyn EnvironmentSource, os: OsId) -> EnvironmentReport {
    let shell = source
        .variable("SHELL")
        .or_else(|| source.variable("COMSPEC"))
        .and_then(|value| shell_id(&value));
    let mut agents = [
        ("codex", AgentId::Codex),
        ("claude", AgentId::Claude),
        ("gemini", AgentId::Gemini),
    ]
    .into_iter()
    .filter_map(|(name, id)| source.executable_exists(name).then_some(id))
    .collect::<Vec<_>>();
    agents.sort_unstable();
    agents.dedup();

    let mut terminals = Vec::new();
    for name in ["TERM_PROGRAM", "TERM"] {
        if let Some(value) = source.variable(name)
            && let Some(id) = terminal_id(&value)
        {
            terminals.push(id);
        }
    }
    for (variable, id) in [
        ("GHOSTTY_RESOURCES_DIR", TerminalId::Ghostty),
        ("KITTY_WINDOW_ID", TerminalId::Kitty),
        ("WEZTERM_EXECUTABLE", TerminalId::Wezterm),
        ("ITERM_SESSION_ID", TerminalId::Iterm2),
    ] {
        if source.variable(variable).is_some() {
            terminals.push(id);
        }
    }
    terminals.sort_unstable();
    terminals.dedup();

    EnvironmentReport {
        os,
        shell,
        agents,
        terminals,
    }
}

fn current_os() -> OsId {
    match std::env::consts::OS {
        "macos" => OsId::Macos,
        "linux" => OsId::Linux,
        "windows" => OsId::Windows,
        _ => OsId::Other,
    }
}

fn shell_id(value: &OsString) -> Option<ShellId> {
    let name = Path::new(value)
        .file_stem()?
        .to_string_lossy()
        .to_ascii_lowercase();
    match name.as_str() {
        "zsh" => Some(ShellId::Zsh),
        "bash" => Some(ShellId::Bash),
        "fish" => Some(ShellId::Fish),
        "pwsh" | "powershell" => Some(ShellId::Pwsh),
        "cmd" => Some(ShellId::Cmd),
        _ => None,
    }
}

fn terminal_id(value: &OsString) -> Option<TerminalId> {
    let name = value.to_string_lossy().to_ascii_lowercase();
    if name.contains("ghostty") {
        Some(TerminalId::Ghostty)
    } else if name.contains("kitty") {
        Some(TerminalId::Kitty)
    } else if name.contains("wezterm") {
        Some(TerminalId::Wezterm)
    } else if name.contains("iterm") {
        Some(TerminalId::Iterm2)
    } else {
        None
    }
}

fn executable_in(directory: &Path, name: &str) -> bool {
    if directory.join(name).is_file() {
        return true;
    }
    #[cfg(windows)]
    {
        return ["exe", "cmd", "bat", "com"]
            .iter()
            .any(|extension| directory.join(format!("{name}.{extension}")).is_file());
    }
    #[cfg(not(windows))]
    false
}
