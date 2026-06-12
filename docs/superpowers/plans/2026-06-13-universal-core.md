# SaveMyTerminal Universal Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a cross-platform `smt run -- <command>` vertical slice that reports privacy-safe lifecycle and resource metadata to an authenticated local service while preserving the child command's terminal behavior and exit result.

**Architecture:** A single Rust package exposes a reusable library and the `smt` binary. The wrapper owns the child process and inherited terminal streams; a loopback Axum service owns normalized session state; adapters and renderers communicate only through versioned protocol types. Phase 1 keeps state in memory and uses a concise stderr renderer, leaving SQLite, the dashboard, config editing, native agent hooks, and terminal-specific visuals to later phases.

**Tech Stack:** Rust 2024 edition, Tokio, Axum, Clap, Serde, UUID, `sysinfo`, `directories`, `secrecy`, `subtle`, `tracing`, and Rust's built-in test framework with `assert_cmd`, `predicates`, and `tempfile`.

---

## Phase Boundary

This plan implements:

- The `smt run -- <command> [args...]`, `smt service`, and `smt status` command paths.
- A versioned protocol containing no prompt, output, raw command line, path, or environment fields.
- Exact process lifecycle and best-effort CPU/memory metrics with quality labels.
- A token-authenticated loopback HTTP service and in-memory active-session registry.
- Five-minute idle shutdown after all sessions have ended.
- Graceful fallback: the child still launches when service startup or event delivery fails.
- A portable, opt-out concise status renderer that never captures child output.
- Unit, contract, integration, and macOS/Linux/Windows CI coverage for this slice.

This plan deliberately excludes persistence, dashboard UI, setup/config mutation, native hooks, rich context reporting, and native terminal visuals.

## File Map

```text
Cargo.toml                         Package metadata and dependencies
src/main.rs                       Thin process entrypoint
src/lib.rs                        Public modules and top-level dispatch
src/cli.rs                        Clap command definitions
src/app.rs                        Command routing and exit-code conversion
src/protocol/mod.rs               Protocol exports and version constant
src/protocol/event.rs             Events, metric quality, and validation
src/protocol/session.rs           Session snapshots and lifecycle transitions
src/auth.rs                       Per-install token generation and loading
src/paths.rs                      Per-user runtime/config paths
src/service/mod.rs                Service exports
src/service/api.rs                Authenticated Axum routes
src/service/registry.rs           Concurrent in-memory session registry
src/service/runtime.rs            Listener, discovery file, and idle shutdown
src/client.rs                     Local service client and on-demand startup
src/runner/mod.rs                 Generic command wrapper orchestration
src/runner/child.rs               Inherited-I/O child execution
src/runner/metrics.rs             Best-effort process metric sampling
src/renderer/mod.rs               Renderer trait and exports
src/renderer/plain.rs             Concise portable stderr renderer
tests/cli_help.rs                 Public CLI contract tests
tests/service_api.rs              Auth, validation, and registry API tests
tests/run_command.rs              Wrapper, fallback, arguments, and exit tests
tests/privacy_contract.rs         Serialized payload prohibited-key tests
.github/workflows/ci.yml          Formatting, lint, and tests on three OSes
.gitignore                        Rust, runtime, and brainstorm artifacts
README.md                         Phase 1 usage and privacy contract
```

Before Task 1, create a feature branch from the reviewed design commit:

```bash
git switch -c feat/universal-core
```

### Task 1: Scaffold The Rust Package And CLI Contract

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/cli.rs`
- Create: `src/app.rs`
- Create: `tests/cli_help.rs`
- Create: `.gitignore`

- [ ] **Step 1: Write the failing CLI contract tests**

Create `tests/cli_help.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_phase_one_commands() {
    Command::cargo_bin("smt")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("service"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn run_requires_a_command_after_separator() {
    Command::cargo_bin("smt")
        .unwrap()
        .arg("run")
        .assert()
        .failure()
        .stderr(predicate::str::contains("command"));
}
```

- [ ] **Step 2: Create the manifest and run the test to verify it fails**

Create `Cargo.toml`:

```toml
[package]
name = "savemyterminal"
version = "0.1.0"
edition = "2024"
rust-version = "1.95"
license = "MIT"
description = "Local observability for terminal-based AI agents"
repository = "https://github.com/SUDARSHANCHAUDHARI/SaveMyTerminal"

[lib]
name = "savemyterminal"
path = "src/lib.rs"

[[bin]]
name = "smt"
path = "src/main.rs"

[dependencies]
anyhow = "1"
axum = { version = "0.8", features = ["json"] }
clap = { version = "4.5", features = ["derive"] }
directories = "6"
fs2 = "0.4"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
secrecy = "0.10"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
subtle = "2.6"
sysinfo = "0.39"
thiserror = "2"
tokio = { version = "1", features = ["macros", "net", "process", "rt-multi-thread", "signal", "sync", "time"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
uuid = { version = "1", features = ["serde", "v4"] }

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"

[profile.release]
lto = "thin"
strip = true
```

Run: `cargo test --test cli_help`

Expected: FAIL because `src/lib.rs` and the `smt` binary do not exist.

- [ ] **Step 3: Add the minimal CLI implementation**

Create `src/cli.rs`:

```rust
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
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
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
    /// Override idle shutdown. Intended for tests.
    #[arg(long, default_value_t = 300_000, hide = true)]
    pub idle_timeout_ms: u64,
}
```

Create `src/app.rs`:

```rust
use crate::cli::{Cli, Command};
use clap::Parser;

pub async fn run() -> anyhow::Result<i32> {
    match Cli::parse().command {
        Command::Run(_) => Ok(0),
        Command::Service(_) => Ok(0),
        Command::Status => Ok(0),
    }
}
```

Create `src/lib.rs`:

```rust
pub mod app;
pub mod cli;
```

Create `src/main.rs`:

```rust
#[tokio::main]
async fn main() {
    let code = match savemyterminal::app::run().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("smt: {error:#}");
            1
        }
    };
    std::process::exit(code);
}
```

Create `.gitignore`:

```gitignore
/target/
/.superpowers/
/.smt-runtime/
*.log
```

- [ ] **Step 4: Run formatting and the CLI tests**

Run: `cargo fmt --check && cargo test --test cli_help`

Expected: both tests PASS.

- [ ] **Step 5: Commit the scaffold**

```bash
git add Cargo.toml .gitignore src/main.rs src/lib.rs src/cli.rs src/app.rs tests/cli_help.rs
git commit -m "feat: scaffold smt command"
```

### Task 2: Define The Privacy-Safe Protocol

**Files:**
- Create: `src/protocol/mod.rs`
- Create: `src/protocol/event.rs`
- Create: `src/protocol/session.rs`
- Create: `tests/privacy_contract.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing event validation and privacy tests**

Create `tests/privacy_contract.rs`:

```rust
use savemyterminal::protocol::{
    Event, EventKind, Metric, MetricQuality, MetricSource, PROTOCOL_VERSION,
};
use serde_json::Value;
use uuid::Uuid;

#[test]
fn event_round_trip_contains_only_approved_top_level_fields() {
    let event = Event::new(
        Uuid::new_v4(),
        "generic",
        "unknown",
        EventKind::Started,
    );
    let value = serde_json::to_value(event).unwrap();
    let object = value.as_object().unwrap();
    let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "adapter_id",
            "agent_id",
            "event_id",
            "kind",
            "protocol_version",
            "session_id",
            "timestamp_ms"
        ]
    );
}

#[test]
fn serialized_event_never_exposes_prohibited_field_names() {
    let event = Event::new(
        Uuid::new_v4(),
        "generic",
        "unknown",
        EventKind::Metrics {
            cpu_percent: Some(Metric::new(12.5, MetricQuality::Exact, MetricSource::Os)),
            memory_bytes: None,
        },
    );
    let value: Value = serde_json::to_value(event).unwrap();
    let json = value.to_string().to_ascii_lowercase();
    for prohibited in [
        "prompt", "response", "output", "command", "argument", "environment",
        "working_directory", "path", "file_content",
    ] {
        assert!(!json.contains(prohibited), "found prohibited key: {prohibited}");
    }
}

#[test]
fn rejects_unknown_protocol_versions() {
    let mut event = Event::new(
        Uuid::new_v4(),
        "generic",
        "unknown",
        EventKind::Started,
    );
    event.protocol_version = PROTOCOL_VERSION + 1;
    assert_eq!(event.validate().unwrap_err().to_string(), "unsupported protocol version");
}
```

- [ ] **Step 2: Run the protocol tests to verify they fail**

Run: `cargo test --test privacy_contract`

Expected: FAIL because `savemyterminal::protocol` does not exist.

- [ ] **Step 3: Implement protocol types and validation**

Create `src/protocol/mod.rs`:

```rust
mod event;
mod session;

pub use event::{
    Event, EventKind, FailureCategory, Metric, MetricQuality, MetricSource,
    ProtocolError, ToolCategory,
};
pub use session::{SessionSnapshot, SessionState};

pub const PROTOCOL_VERSION: u16 = 1;
```

Create `src/protocol/event.rs`:

```rust
use crate::protocol::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricQuality {
    Exact,
    Estimated,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricSource {
    Agent,
    Os,
    Heuristic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    Shell,
    FileRead,
    FileWrite,
    Search,
    Network,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    ProcessExit,
    Launch,
    Adapter,
    Protocol,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metric<T> {
    pub value: T,
    pub quality: MetricQuality,
    pub source: MetricSource,
}

impl<T> Metric<T> {
    pub fn new(value: T, quality: MetricQuality, source: MetricSource) -> Self {
        Self { value, quality, source }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    Started,
    Thinking,
    ToolRunning { category: ToolCategory },
    Waiting,
    Metrics {
        #[serde(skip_serializing_if = "Option::is_none")]
        cpu_percent: Option<Metric<f32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        memory_bytes: Option<Metric<u64>>,
    },
    Completed { exit_code: i32 },
    Failed { exit_code: i32, category: FailureCategory },
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub protocol_version: u16,
    pub event_id: Uuid,
    pub session_id: Uuid,
    pub timestamp_ms: u64,
    pub adapter_id: String,
    pub agent_id: String,
    pub kind: EventKind,
}

impl Event {
    pub fn new(
        session_id: Uuid,
        adapter_id: impl Into<String>,
        agent_id: impl Into<String>,
        kind: EventKind,
    ) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            protocol_version: PROTOCOL_VERSION,
            event_id: Uuid::new_v4(),
            session_id,
            timestamp_ms,
            adapter_id: adapter_id.into(),
            agent_id: agent_id.into(),
            kind,
        }
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if self.adapter_id.is_empty() || self.agent_id.is_empty() {
            return Err(ProtocolError::EmptyIdentifier);
        }
        if self.adapter_id.len() > 64 || self.agent_id.len() > 64 {
            return Err(ProtocolError::IdentifierTooLong);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("unsupported protocol version")]
    UnsupportedVersion,
    #[error("adapter and agent identifiers must not be empty")]
    EmptyIdentifier,
    #[error("adapter and agent identifiers must not exceed 64 bytes")]
    IdentifierTooLong,
}
```

Create `src/protocol/session.rs`:

```rust
use crate::protocol::{Metric, MetricQuality, MetricSource};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Starting,
    Thinking,
    ToolRunning,
    Waiting,
    Completed,
    Failed,
    Interrupted,
}

impl SessionState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Interrupted)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: Uuid,
    pub adapter_id: String,
    pub agent_id: String,
    pub state: SessionState,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    pub cpu_percent: Option<Metric<f32>>,
    pub memory_bytes: Option<Metric<u64>>,
}

impl SessionSnapshot {
    pub fn unavailable_metric<T: Default>(source: MetricSource) -> Metric<T> {
        Metric::new(T::default(), MetricQuality::Unavailable, source)
    }
}
```

Add to `src/lib.rs`:

```rust
pub mod protocol;
```

- [ ] **Step 4: Run protocol tests and all unit tests**

Run: `cargo test --test privacy_contract && cargo test --lib`

Expected: PASS.

- [ ] **Step 5: Commit the protocol**

```bash
git add src/lib.rs src/protocol/mod.rs src/protocol/event.rs src/protocol/session.rs tests/privacy_contract.rs
git commit -m "feat: define privacy safe event protocol"
```

### Task 3: Implement Session Lifecycle Validation

**Files:**
- Create: `src/service/mod.rs`
- Create: `src/service/registry.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing registry transition tests**

Create `src/service/registry.rs` with the test module first:

```rust
use crate::protocol::{Event, SessionSnapshot};
use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SessionRegistry {
    sessions: Arc<RwLock<HashMap<Uuid, SessionSnapshot>>>,
    last_activity: Arc<RwLock<Instant>>,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            last_activity: Arc::new(RwLock::new(Instant::now())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{EventKind, SessionState};

    #[tokio::test]
    async fn started_event_opens_a_session() {
        let registry = SessionRegistry::default();
        let id = Uuid::new_v4();
        registry.apply(Event::new(id, "generic", "unknown", EventKind::Started)).await.unwrap();
        assert_eq!(registry.get(id).await.unwrap().state, SessionState::Starting);
    }

    #[tokio::test]
    async fn rejects_updates_after_a_terminal_state() {
        let registry = SessionRegistry::default();
        let id = Uuid::new_v4();
        registry.apply(Event::new(id, "generic", "unknown", EventKind::Started)).await.unwrap();
        registry.apply(Event::new(id, "generic", "unknown", EventKind::Completed { exit_code: 0 })).await.unwrap();
        let error = registry.apply(Event::new(id, "generic", "unknown", EventKind::Waiting)).await.unwrap_err();
        assert_eq!(error, RegistryError::AlreadyFinished);
    }

    #[tokio::test]
    async fn metrics_update_values_without_changing_state() {
        let registry = SessionRegistry::default();
        let id = Uuid::new_v4();
        registry.apply(Event::new(id, "generic", "unknown", EventKind::Started)).await.unwrap();
        registry.apply(Event::new(
            id,
            "generic",
            "unknown",
            EventKind::Metrics {
                cpu_percent: Some(crate::protocol::Metric::new(
                    7.0,
                    crate::protocol::MetricQuality::Exact,
                    crate::protocol::MetricSource::Os,
                )),
                memory_bytes: None,
            },
        )).await.unwrap();
        let snapshot = registry.get(id).await.unwrap();
        assert_eq!(snapshot.state, SessionState::Starting);
        assert_eq!(snapshot.cpu_percent.unwrap().value, 7.0);
    }
}
```

- [ ] **Step 2: Run the registry tests to verify they fail**

Run: `cargo test service::registry::tests`

Expected: FAIL because `apply`, `get`, and `RegistryError` are not implemented.

- [ ] **Step 3: Implement the registry beneath the struct**

Add to `src/service/registry.rs` before the test module:

```rust
use crate::protocol::{EventKind, SessionState};

impl SessionRegistry {
    pub async fn apply(&self, event: Event) -> Result<SessionSnapshot, RegistryError> {
        event.validate()?;
        *self.last_activity.write().await = Instant::now();
        let mut sessions = self.sessions.write().await;

        if matches!(event.kind, EventKind::Started) {
            if sessions.contains_key(&event.session_id) {
                return Err(RegistryError::DuplicateSession);
            }
            let snapshot = SessionSnapshot {
                session_id: event.session_id,
                adapter_id: event.adapter_id,
                agent_id: event.agent_id,
                state: SessionState::Starting,
                started_at_ms: event.timestamp_ms,
                updated_at_ms: event.timestamp_ms,
                cpu_percent: None,
                memory_bytes: None,
            };
            sessions.insert(snapshot.session_id, snapshot.clone());
            return Ok(snapshot);
        }

        let snapshot = sessions
            .get_mut(&event.session_id)
            .ok_or(RegistryError::UnknownSession)?;
        if snapshot.state.is_terminal() {
            return Err(RegistryError::AlreadyFinished);
        }
        if snapshot.adapter_id != event.adapter_id || snapshot.agent_id != event.agent_id {
            return Err(RegistryError::IdentityMismatch);
        }

        match event.kind {
            EventKind::Started => unreachable!(),
            EventKind::Thinking => snapshot.state = SessionState::Thinking,
            EventKind::ToolRunning { .. } => snapshot.state = SessionState::ToolRunning,
            EventKind::Waiting => snapshot.state = SessionState::Waiting,
            EventKind::Metrics { cpu_percent, memory_bytes } => {
                if cpu_percent.is_some() {
                    snapshot.cpu_percent = cpu_percent;
                }
                if memory_bytes.is_some() {
                    snapshot.memory_bytes = memory_bytes;
                }
            }
            EventKind::Completed { .. } => snapshot.state = SessionState::Completed,
            EventKind::Failed { .. } => snapshot.state = SessionState::Failed,
            EventKind::Interrupted => snapshot.state = SessionState::Interrupted,
        }
        snapshot.updated_at_ms = event.timestamp_ms;
        Ok(snapshot.clone())
    }

    pub async fn get(&self, id: Uuid) -> Option<SessionSnapshot> {
        self.sessions.read().await.get(&id).cloned()
    }

    pub async fn active_count(&self) -> usize {
        self.sessions
            .read()
            .await
            .values()
            .filter(|session| !session.state.is_terminal())
            .count()
    }

    pub async fn idle_for(&self) -> Duration {
        self.last_activity.read().await.elapsed()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error(transparent)]
    Protocol(#[from] crate::protocol::ProtocolError),
    #[error("session already exists")]
    DuplicateSession,
    #[error("session does not exist")]
    UnknownSession,
    #[error("session already finished")]
    AlreadyFinished,
    #[error("event identity does not match session")]
    IdentityMismatch,
}
```

Create `src/service/mod.rs`:

```rust
pub mod registry;

pub use registry::{RegistryError, SessionRegistry};
```

Add to `src/lib.rs`:

```rust
pub mod service;
```

- [ ] **Step 4: Run lifecycle tests**

Run: `cargo test service::registry::tests`

Expected: all three tests PASS.

- [ ] **Step 5: Commit lifecycle handling**

```bash
git add src/lib.rs src/service/mod.rs src/service/registry.rs
git commit -m "feat: validate session lifecycle"
```

### Task 4: Add Per-User Paths And Authentication

**Files:**
- Create: `src/paths.rs`
- Create: `src/auth.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing path and token tests**

Create `src/auth.rs` with tests first:

```rust
use secrecy::SecretString;
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reuses_a_nonempty_token() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("auth.token");
        let first = load_or_create_token(&path).unwrap();
        let second = load_or_create_token(&path).unwrap();
        use secrecy::ExposeSecret;
        assert_eq!(first.expose_secret(), second.expose_secret());
        assert!(std::fs::metadata(path).unwrap().len() >= 64);
    }

    #[cfg(unix)]
    #[test]
    fn token_file_is_user_only_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("auth.token");
        load_or_create_token(&path).unwrap();
        assert_eq!(std::fs::metadata(path).unwrap().permissions().mode() & 0o777, 0o600);
    }
}
```

- [ ] **Step 2: Run the auth tests to verify they fail**

Run: `cargo test auth::tests`

Expected: FAIL because `load_or_create_token` is missing.

- [ ] **Step 3: Implement runtime paths and token persistence**

Create `src/paths.rs`:

```rust
use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub runtime_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let dirs = ProjectDirs::from("com", "SudarshanTechLabs", "SaveMyTerminal")
            .context("could not determine per-user application directories")?;
        Ok(Self {
            config_dir: dirs.config_dir().to_path_buf(),
            runtime_dir: dirs.cache_dir().join("runtime"),
        })
    }

    pub fn token_file(&self) -> PathBuf {
        self.config_dir.join("auth.token")
    }

    pub fn discovery_file(&self) -> PathBuf {
        self.runtime_dir.join("service.json")
    }
}
```

Complete `src/auth.rs`:

```rust
use anyhow::{Context, Result};
use secrecy::SecretString;
use std::{fs::OpenOptions, io::Write, path::Path};
use uuid::Uuid;

pub fn load_or_create_token(path: &Path) -> Result<SecretString> {
    if path.exists() {
        return load_token(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let value = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return load_token(path);
        }
        Err(error) => return Err(error.into()),
    };
    file.write_all(value.as_bytes())?;
    file.sync_all()?;
    Ok(SecretString::from(value))
}

pub fn load_token(path: &Path) -> Result<SecretString> {
    let value = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read token file {}", path.display()))?;
    Ok(SecretString::from(value.trim().to_owned()))
}
```

Add to `src/lib.rs`:

```rust
pub mod auth;
pub mod paths;
```

- [ ] **Step 4: Run auth tests and inspect secret handling**

Run: `cargo test auth::tests && rg -n "println!|eprintln!|tracing::.*token|Debug.*SecretString" src`

Expected: tests PASS; the search finds no statement that prints a token value.

- [ ] **Step 5: Commit path and auth support**

```bash
git add src/lib.rs src/auth.rs src/paths.rs
git commit -m "feat: add local authentication token"
```

### Task 5: Expose An Authenticated Loopback Service

**Files:**
- Create: `src/service/api.rs`
- Create: `src/service/runtime.rs`
- Create: `tests/service_api.rs`
- Modify: `src/service/mod.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Write failing API tests**

Create `tests/service_api.rs`:

```rust
use reqwest::StatusCode;
use savemyterminal::{
    protocol::{Event, EventKind},
    service::{spawn_test_service, ServiceConfig},
};
use secrecy::SecretString;
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn rejects_missing_authentication() {
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from("secret".to_owned()))).await.unwrap();
    let response = reqwest::Client::new()
        .get(format!("{}/v1/health", service.base_url))
        .send().await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    service.shutdown().await;
}

#[tokio::test]
async fn accepts_valid_event_and_returns_snapshot() {
    let token = "secret";
    let service = spawn_test_service(ServiceConfig::for_test(SecretString::from(token.to_owned()))).await.unwrap();
    let event = Event::new(Uuid::new_v4(), "generic", "unknown", EventKind::Started);
    let response = reqwest::Client::new()
        .post(format!("{}/v1/events", service.base_url))
        .bearer_auth(token)
        .json(&event)
        .send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.json::<serde_json::Value>().await.unwrap()["state"], "starting");
    service.shutdown().await;
}

#[tokio::test]
async fn idle_service_stops_after_timeout() {
    let mut config = ServiceConfig::for_test(SecretString::from("secret".to_owned()));
    config.idle_timeout = Duration::from_millis(50);
    let service = spawn_test_service(config).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), service.finished()).await.unwrap().unwrap();
}
```

- [ ] **Step 2: Run the API tests to verify they fail**

Run: `cargo test --test service_api`

Expected: FAIL because the service runtime API does not exist.

- [ ] **Step 3: Implement authenticated routes**

Create `src/service/api.rs`:

```rust
use crate::{protocol::Event, service::SessionRegistry};
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub struct ApiState {
    pub registry: SessionRegistry,
    pub token: SecretString,
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/v1/health", get(|| async { StatusCode::NO_CONTENT }))
        .route("/v1/events", post(post_event))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .with_state(state)
}

async fn authenticate(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let supplied = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if supplied.as_bytes().ct_eq(state.token.expose_secret().as_bytes()).unwrap_u8() != 1 {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

async fn post_event(
    State(state): State<ApiState>,
    Json(event): Json<Event>,
) -> Result<Json<crate::protocol::SessionSnapshot>, (StatusCode, String)> {
    state.registry.apply(event).await.map(Json).map_err(|error| {
        (StatusCode::UNPROCESSABLE_ENTITY, error.to_string())
    })
}
```

- [ ] **Step 4: Implement loopback binding, discovery, and idle shutdown**

Create `src/service/runtime.rs`:

```rust
use crate::service::{api::{router, ApiState}, SessionRegistry};
use anyhow::Result;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, path::PathBuf, time::Duration};
use tokio::{net::TcpListener, sync::watch, task::JoinHandle};

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub token: SecretString,
    pub discovery_file: Option<PathBuf>,
    pub lock_file: Option<PathBuf>,
    pub idle_timeout: Duration,
}

impl ServiceConfig {
    pub fn for_test(token: SecretString) -> Self {
        Self {
            token,
            discovery_file: None,
            lock_file: None,
            idle_timeout: Duration::from_secs(300),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDiscovery {
    pub base_url: String,
    pub pid: u32,
}

pub struct RunningService {
    pub base_url: String,
    shutdown_tx: watch::Sender<bool>,
    task: JoinHandle<anyhow::Result<()>>,
}

impl RunningService {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.task.await;
    }

    pub async fn finished(self) -> anyhow::Result<()> {
        self.task.await?
    }
}

pub async fn spawn_test_service(config: ServiceConfig) -> Result<RunningService> {
    spawn_service(config).await
}

pub async fn spawn_service(config: ServiceConfig) -> Result<RunningService> {
    let service_lock = if let Some(path) = &config.lock_file {
        use fs2::FileExt;
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let file = std::fs::OpenOptions::new().create(true).write(true).open(path)?;
        file.try_lock_exclusive()?;
        Some(file)
    } else {
        None
    };
    let registry = SessionRegistry::default();
    let discovery_file = config.discovery_file.clone();
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).await?;
    let address = listener.local_addr()?;
    let base_url = format!("http://{address}");
    if let Some(path) = &config.discovery_file {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, serde_json::to_vec(&ServiceDiscovery { base_url: base_url.clone(), pid: std::process::id() })?)?;
        if path.exists() { std::fs::remove_file(path)?; }
        std::fs::rename(temp, path)?;
    }
    let app = router(ApiState { registry: registry.clone(), token: config.token });
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let idle_timeout = config.idle_timeout;
    let task = tokio::spawn(async move {
        let _service_lock = service_lock;
        let idle_registry = registry.clone();
        let idle = async move {
            loop {
                tokio::time::sleep(idle_timeout.min(Duration::from_millis(250))).await;
                if idle_registry.active_count().await == 0
                    && idle_registry.idle_for().await >= idle_timeout
                {
                    break;
                }
            }
        };
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                tokio::select! {
                    _ = idle => {},
                    _ = shutdown_rx.changed() => {},
                }
            })
            .await?;
        if let Some(path) = discovery_file { let _ = std::fs::remove_file(path); }
        Ok(())
    });
    Ok(RunningService { base_url, shutdown_tx, task })
}
```

Update `src/service/mod.rs`:

```rust
pub mod api;
pub mod registry;
pub mod runtime;

pub use registry::{RegistryError, SessionRegistry};
pub use runtime::{spawn_service, spawn_test_service, RunningService, ServiceConfig, ServiceDiscovery};
```

Update the `Command::Service` branch in `src/app.rs` to discover paths, load the token, pass a five-minute timeout, start the service, and await `finished()`:

```rust
Command::Service(args) => {
    let discovered = crate::paths::AppPaths::discover()?;
    let paths = crate::paths::AppPaths {
        config_dir: args.config_dir.unwrap_or(discovered.config_dir),
        runtime_dir: args.runtime_dir.unwrap_or(discovered.runtime_dir),
    };
    let token = crate::auth::load_or_create_token(&paths.token_file())?;
    let service = crate::service::spawn_service(crate::service::ServiceConfig {
        token,
        discovery_file: Some(paths.discovery_file()),
        lock_file: Some(paths.runtime_dir.join("service.lock")),
        idle_timeout: std::time::Duration::from_millis(args.idle_timeout_ms),
    }).await?;
    service.finished().await?;
    Ok(0)
}
```

- [ ] **Step 5: Run focused and full service tests**

Run: `cargo test --test service_api && cargo test service::`

Expected: authentication, event acceptance, lifecycle validation, and idle shutdown tests PASS.

- [ ] **Step 6: Commit the service**

```bash
git add src/app.rs src/service/mod.rs src/service/api.rs src/service/runtime.rs tests/service_api.rs
git commit -m "feat: add authenticated local service"
```

### Task 6: Add The Service Client And On-Demand Startup

**Files:**
- Create: `src/client.rs`
- Modify: `src/lib.rs`
- Modify: `src/app.rs`
- Create: `tests/service_startup.rs`

- [ ] **Step 1: Write failing client startup tests**

Create `tests/service_startup.rs`:

```rust
use savemyterminal::{client::ServiceClient, paths::AppPaths};
use std::time::Duration;

#[tokio::test]
async fn concurrent_ensure_calls_reuse_one_reachable_endpoint() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths {
        config_dir: temp.path().join("config"),
        runtime_dir: temp.path().join("runtime"),
    };
    let executable = assert_cmd::cargo::cargo_bin!("smt");
    let (first, second) = tokio::join!(
        ServiceClient::ensure_with_executable(&paths, &executable, Duration::from_millis(200)),
        ServiceClient::ensure_with_executable(&paths, &executable, Duration::from_millis(200)),
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(first.base_url(), second.base_url());
}
```

- [ ] **Step 2: Run the startup test to verify it fails**

Run: `cargo test --test service_startup`

Expected: FAIL because `ServiceClient` does not exist.

- [ ] **Step 3: Implement authenticated client discovery and startup**

Create `src/client.rs`:

```rust
use crate::{
    auth::{load_or_create_token, load_token},
    paths::AppPaths,
    protocol::Event,
    service::ServiceDiscovery,
};
use anyhow::{bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::process::Command;

#[derive(Clone)]
pub struct ServiceClient {
    client: reqwest::Client,
    base_url: String,
    token: SecretString,
}

impl ServiceClient {
    pub async fn ensure(paths: &AppPaths) -> Result<Self> {
        let executable = std::env::current_exe()?;
        Self::ensure_with_executable(paths, &executable, Duration::from_secs(300)).await
    }

    pub async fn connect(paths: &AppPaths) -> Result<Self> {
        let token = load_token(&paths.token_file())?;
        Self::from_discovery(paths, token).await
    }

    pub async fn ensure_with_executable(
        paths: &AppPaths,
        executable: &Path,
        idle_timeout: Duration,
    ) -> Result<Self> {
        let token = load_or_create_token(&paths.token_file())?;
        if let Ok(client) = Self::from_discovery(paths, token.clone()).await {
            return Ok(client);
        }

        std::fs::create_dir_all(&paths.runtime_dir)?;
        let mut command = Command::new(executable);
        command
            .arg("service")
            .arg("--config-dir")
            .arg(&paths.config_dir)
            .arg("--runtime-dir")
            .arg(&paths.runtime_dir)
            .arg("--idle-timeout-ms")
            .arg(idle_timeout.as_millis().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(false);
        platform_detach(&mut command);
        command.spawn().context("failed to start local service")?;

        for _ in 0..40 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if let Ok(client) = Self::from_discovery(paths, token.clone()).await {
                return Ok(client);
            }
        }
        bail!("local service did not become ready")
    }

    async fn from_discovery(paths: &AppPaths, token: SecretString) -> Result<Self> {
        let discovery: ServiceDiscovery = serde_json::from_slice(&std::fs::read(paths.discovery_file())?)?;
        let client = Self {
            client: reqwest::Client::builder().timeout(Duration::from_secs(1)).build()?,
            base_url: discovery.base_url,
            token,
        };
        client.health().await?;
        Ok(client)
    }

    pub fn base_url(&self) -> &str { &self.base_url }

    pub async fn health(&self) -> Result<()> {
        self.client.get(format!("{}/v1/health", self.base_url))
            .bearer_auth(self.token.expose_secret())
            .send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn send(&self, event: &Event) -> Result<()> {
        self.client.post(format!("{}/v1/events", self.base_url))
            .bearer_auth(self.token.expose_secret())
            .json(event)
            .send().await?.error_for_status()?;
        Ok(())
    }
}

#[cfg(unix)]
fn platform_detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.as_std_mut().process_group(0);
}

#[cfg(windows)]
fn platform_detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    command.as_std_mut().creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
}
```

Add to `src/lib.rs`:

```rust
pub mod client;
```

Implement the `status` branch in `src/app.rs`:

```rust
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
```

- [ ] **Step 4: Run the startup tests on the current OS**

Run: `cargo test --test service_startup -- --nocapture`

Expected: PASS and both clients report the same loopback endpoint.

- [ ] **Step 5: Commit on-demand startup**

```bash
git add src/lib.rs src/app.rs src/client.rs tests/service_startup.rs
git commit -m "feat: start local service on demand"
```

### Task 7: Run Commands With Inherited I/O And Graceful Fallback

**Files:**
- Create: `src/runner/mod.rs`
- Create: `src/runner/child.rs`
- Create: `src/renderer/mod.rs`
- Create: `src/renderer/plain.rs`
- Modify: `src/lib.rs`
- Modify: `src/app.rs`
- Create: `tests/helpers/exit_with.rs`
- Create: `tests/run_command.rs`

- [ ] **Step 1: Add a non-shipping test fixture and write failing wrapper tests**

Create `tests/helpers/exit_with.rs`:

```rust
fn main() {
    let mut args = std::env::args().skip(1);
    let code: i32 = args.next().unwrap().parse().unwrap();
    for arg in args {
        println!("arg={arg}");
    }
    std::process::exit(code);
}
```

Create `tests/run_command.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn build_helper() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let binary = temp.path().join(format!("exit-with{}", std::env::consts::EXE_SUFFIX));
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/helpers/exit_with.rs");
    let status = std::process::Command::new("rustc")
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .status()
        .unwrap();
    assert!(status.success());
    (temp, binary)
}

#[test]
fn preserves_arguments_and_success_exit_code() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt").unwrap()
        .args(["run", "--no-status", "--"])
        .arg(helper)
        .args(["0", "hello world", "--flag"])
        .assert()
        .success()
        .stdout(predicate::str::contains("arg=hello world"))
        .stdout(predicate::str::contains("arg=--flag"));
}

#[test]
fn preserves_nonzero_exit_code() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt").unwrap()
        .args(["run", "--no-status", "--"])
        .arg(helper)
        .arg("23")
        .assert()
        .code(23);
}

#[test]
fn launches_child_when_service_is_unavailable() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt").unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--no-status", "--"])
        .arg(helper)
        .args(["0", "still-ran"])
        .assert()
        .success()
        .stdout(predicate::str::contains("arg=still-ran"));
}
```

- [ ] **Step 2: Run wrapper tests to verify they fail**

Run: `cargo test --test run_command`

Expected: FAIL because `Command::Run` still returns immediately.

- [ ] **Step 3: Implement the renderer contract**

Create `src/renderer/mod.rs`:

```rust
mod plain;

pub use plain::PlainRenderer;

pub trait Renderer: Send {
    fn started(&mut self, agent_id: &str);
    fn finished(&mut self, agent_id: &str, exit_code: i32);
    fn warning(&mut self, message: &str);
}
```

Create `src/renderer/plain.rs`:

```rust
use crate::renderer::Renderer;
use std::io::{self, Write};

pub struct PlainRenderer<W: Write = io::Stderr> {
    writer: W,
    enabled: bool,
}

impl PlainRenderer<io::Stderr> {
    pub fn stderr(enabled: bool) -> Self {
        Self { writer: io::stderr(), enabled }
    }
}

impl<W: Write + Send> Renderer for PlainRenderer<W> {
    fn started(&mut self, agent_id: &str) {
        if self.enabled { let _ = writeln!(self.writer, "smt [{agent_id}] starting"); }
    }

    fn finished(&mut self, agent_id: &str, exit_code: i32) {
        if self.enabled { let _ = writeln!(self.writer, "smt [{agent_id}] exited {exit_code}"); }
    }

    fn warning(&mut self, message: &str) {
        if self.enabled { let _ = writeln!(self.writer, "smt warning: {message}"); }
    }
}
```

- [ ] **Step 4: Implement inherited-I/O child execution and event delivery**

Create `src/runner/child.rs`:

```rust
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
    if let Some(code) = status.code() { return code; }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        return 128 + status.signal().unwrap_or(1);
    }
    #[cfg(windows)]
    { 1 }
}
```

Create `src/runner/mod.rs`:

```rust
mod child;

use crate::{
    client::ServiceClient,
    paths::AppPaths,
    protocol::{Event, EventKind, FailureCategory},
    renderer::Renderer,
};
use anyhow::Result;
use uuid::Uuid;

pub async fn run(
    command: Vec<String>,
    renderer: &mut dyn Renderer,
) -> Result<i32> {
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
        let _ = client.send(&Event::new(session_id, "generic", &agent_id, EventKind::Started)).await;
    }

    let status = child::run_inherited(&command).await?;
    let code = child::exit_code(status);
    let kind = if code == 0 {
        EventKind::Completed { exit_code: code }
    } else {
        EventKind::Failed { exit_code: code, category: FailureCategory::ProcessExit }
    };
    if let Some(client) = &client {
        let _ = client.send(&Event::new(session_id, "generic", &agent_id, kind)).await;
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
```

Add to `src/lib.rs`:

```rust
pub mod renderer;
pub mod runner;
```

Replace the `Command::Run` branch in `src/app.rs`:

```rust
Command::Run(args) => {
    let mut renderer = crate::renderer::PlainRenderer::stderr(!args.no_status);
    crate::runner::run(args.command, &mut renderer).await
}
```

- [ ] **Step 5: Run wrapper and regression tests**

Run: `cargo test --test run_command && cargo test`

Expected: arguments and exit codes are preserved; forced service failure still runs the child; all previous tests PASS.

- [ ] **Step 6: Commit the generic wrapper**

```bash
git add src/lib.rs src/app.rs src/runner/mod.rs src/runner/child.rs src/renderer/mod.rs src/renderer/plain.rs tests/helpers/exit_with.rs tests/run_command.rs
git commit -m "feat: wrap commands without capturing output"
```

### Task 8: Add Best-Effort Process Metrics

**Files:**
- Create: `src/runner/metrics.rs`
- Modify: `src/runner/mod.rs`
- Modify: `src/runner/child.rs`

- [ ] **Step 1: Write failing metric sampler tests**

Add to `src/runner/metrics.rs`:

```rust
use crate::protocol::Metric;

#[derive(Debug, Clone, Default)]
pub struct ProcessMetrics {
    pub cpu_percent: Option<Metric<f32>>,
    pub memory_bytes: Option<Metric<u64>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sampler_reports_current_process_memory_or_unavailable() {
        let metrics = sample_once(std::process::id()).await;
        assert!(metrics.memory_bytes.is_some());
        let memory = metrics.memory_bytes.unwrap();
        assert!(memory.value > 0 || memory.quality == crate::protocol::MetricQuality::Unavailable);
    }
}
```

- [ ] **Step 2: Run the sampler test to verify it fails**

Run: `cargo test runner::metrics::tests`

Expected: FAIL because `sample_once` is missing.

- [ ] **Step 3: Implement a bounded sampler without command inspection**

Complete `src/runner/metrics.rs`:

```rust
use crate::protocol::{Metric, MetricQuality, MetricSource};
use sysinfo::{Pid, ProcessesToUpdate, System};

pub async fn sample_once(pid: u32) -> ProcessMetrics {
    tokio::task::spawn_blocking(move || {
        let mut system = System::new();
        let pid = Pid::from_u32(pid);
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        match system.process(pid) {
            Some(process) => ProcessMetrics {
                cpu_percent: Some(Metric::new(process.cpu_usage(), MetricQuality::Exact, MetricSource::Os)),
                memory_bytes: Some(Metric::new(process.memory(), MetricQuality::Exact, MetricSource::Os)),
            },
            None => ProcessMetrics {
                cpu_percent: Some(Metric::new(0.0, MetricQuality::Unavailable, MetricSource::Os)),
                memory_bytes: Some(Metric::new(0, MetricQuality::Unavailable, MetricSource::Os)),
            },
        }
    }).await.unwrap_or_default()
}
```

Modify `src/runner/child.rs` so `run_inherited` returns the child PID and final status without changing stdio inheritance:

```rust
pub async fn spawn_inherited(command: &[String]) -> Result<tokio::process::Child> {
    let (program, args) = command.split_first().context("command is required")?;
    Command::new(OsString::from(program))
        .args(args)
        .spawn()
        .with_context(|| format!("failed to launch {program}"))
}
```

Modify `src/runner/mod.rs` to:

1. Add `mod metrics;` beside `mod child;`.
2. Spawn the child.
3. Read its PID.
4. Every 500 ms, sample metrics and send an `EventKind::Metrics` event while concurrently awaiting process exit.
5. Stop sampling immediately when the child exits.
6. Never inspect `Child` arguments, stdout, stderr, or environment.

Use this code in place of `child::run_inherited`:

```rust
let mut child = child::spawn_inherited(&command).await?;
let pid = child.id().unwrap_or_default();
let status = if let Some(client) = &client {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
    loop {
        tokio::select! {
            status = child.wait() => break status?,
            _ = interval.tick() => {
                let metrics = metrics::sample_once(pid).await;
                let _ = client.send(&Event::new(
                    session_id,
                    "generic",
                    &agent_id,
                    EventKind::Metrics {
                        cpu_percent: metrics.cpu_percent,
                        memory_bytes: metrics.memory_bytes,
                    },
                )).await;
            }
        }
    }
} else {
    child.wait().await?
};
```

- [ ] **Step 4: Run metric, wrapper, and privacy tests**

Run: `cargo test runner::metrics::tests && cargo test --test run_command && cargo test --test privacy_contract`

Expected: PASS; process output and arguments remain untouched; serialized events still contain no prohibited fields.

- [ ] **Step 5: Commit metrics**

```bash
git add src/runner/mod.rs src/runner/child.rs src/runner/metrics.rs
git commit -m "feat: report local process metrics"
```

### Task 9: Document And Verify The Universal Core

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `README.md`
- Modify: `src/renderer/plain.rs`
- Modify: `tests/run_command.rs`

- [ ] **Step 1: Add a renderer unit test before final polish**

Add to `src/renderer/plain.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_renderer_writes_nothing() {
        let mut output = Vec::new();
        {
            let mut renderer = PlainRenderer { writer: &mut output, enabled: false };
            renderer.started("codex");
            renderer.warning("offline");
            renderer.finished("codex", 0);
        }
        assert!(output.is_empty());
    }

    #[test]
    fn renderer_never_receives_command_arguments() {
        let mut output = Vec::new();
        {
            let mut renderer = PlainRenderer { writer: &mut output, enabled: true };
            renderer.started("codex");
            renderer.finished("codex", 0);
        }
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text, "smt [codex] starting\nsmt [codex] exited 0\n");
    }
}
```

- [ ] **Step 2: Run renderer and end-to-end tests**

Run: `cargo test renderer::plain::tests && cargo test --test run_command`

Expected: PASS.

- [ ] **Step 3: Add cross-platform CI**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
  pull_request:

permissions:
  contents: read

jobs:
  test:
    strategy:
      fail-fast: false
      matrix:
        os: [macos-latest, ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo test --all-targets --all-features
```

- [ ] **Step 4: Document exact Phase 1 behavior**

Create `README.md` with these sections and commands:

````markdown
# SaveMyTerminal

SaveMyTerminal adds local, privacy-safe lifecycle and resource visibility to terminal-based AI agents.

## Phase 1

Run any command through the generic adapter:

```bash
cargo run --bin smt -- run -- codex
```

Disable concise status messages:

```bash
cargo run --bin smt -- run --no-status -- codex
```

Check the on-demand local service:

```bash
cargo run --bin smt -- status
```

## Privacy

Phase 1 sends only session identifiers, agent/adapter identifiers, lifecycle state, exit category, CPU, and memory metadata to a token-authenticated loopback service. It does not capture prompts, responses, terminal output, command arguments, environment values, file contents, or working-directory paths.

## Current Scope

The universal wrapper and local in-memory service are implemented first. Persistence, dashboard, guided setup, native agent hooks, and terminal-native visual effects follow in later phases.
````

- [ ] **Step 5: Run the full verification suite**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
git diff --check
```

Expected: all commands PASS. The release binary is `target/release/smt` on macOS/Linux and `target/release/smt.exe` on Windows.

- [ ] **Step 6: Perform a local privacy smoke check**

Run:

```bash
SMT_TEST_FORCE_SERVICE_FAILURE=1 cargo run --bin smt -- run --no-status -- sh -c 'printf secret-output'
```

Expected on macOS/Linux: stdout is exactly `secret-output`; the wrapper does not copy it into logs or status output. On Windows CI, use `cmd /C "echo|set /p=secret-output"` for the equivalent manual smoke check.

- [ ] **Step 7: Commit documentation and CI**

```bash
git add .github/workflows/ci.yml README.md src/renderer/plain.rs tests/run_command.rs
git commit -m "docs: verify universal core behavior"
```

## Final Phase 1 Review

Before calling Phase 1 complete:

1. Confirm `smt run -- <command>` preserves argument boundaries and exit codes.
2. Manually run an interactive command such as `smt run -- sh` or `smt run -- cmd` and verify keyboard input and terminal output remain direct.
3. Stop or corrupt the service discovery file and verify the child still launches.
4. Inspect all serialized protocol structs and logs for prohibited data fields.
5. Confirm the service listener is `127.0.0.1`, never `0.0.0.0`.
6. Confirm the auth token is not printed and has mode `0600` on Unix.
7. Confirm five-minute idle shutdown with no active sessions.
8. Confirm macOS, Linux, and Windows CI pass before starting the persistence/dashboard plan.
