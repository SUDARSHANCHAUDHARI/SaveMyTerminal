# Phase 3 Setup And Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add strict user settings, previewable setup, reversible managed edits, diagnostics, and uninstall workflows without adding agent- or terminal-specific integrations.

**Architecture:** User settings are a closed TOML model loaded at CLI boundaries and resolved into existing runtime configuration. A separate integration engine owns detection, preview, managed markers, backups, atomic writes, validation, rollback, and manifest updates so later phases only provide descriptors. Doctor consumes structured checks and never blocks the universal wrapper when optional setup state is broken.

**Tech Stack:** Rust 2024, Clap, Serde, TOML, SHA-256, Tokio, Axum, SQLite, tempfile, assert_cmd

---

## File Map

- Create `src/config.rs`: typed settings, validation, dotted-key updates, atomic writes, and backups.
- Create `src/detection.rs`: privacy-safe OS, shell, agent, and terminal capability detection.
- Create `src/integration/mod.rs`: descriptors, plans, action rendering, and orchestration API.
- Create `src/integration/managed.rs`: strict managed-marker insertion, replacement, and removal.
- Create `src/integration/apply.rs`: preconditions, backups, atomic writes, validators, rollback.
- Create `src/manifest.rs`: strict machine-owned manifest model and atomic persistence.
- Create `src/doctor.rs`: structured diagnostic checks and summary status.
- Modify `src/paths.rs`: settings, manifest, backup, and integration path helpers.
- Modify `src/cli.rs`: setup, config, doctor, and uninstall command models.
- Modify `src/app.rs`: command dispatch and effective settings resolution.
- Modify `src/client.rs`: configured service startup.
- Modify `src/runner/mod.rs`: configured status and diagnostics behavior.
- Modify `src/service/runtime.rs`: configured dashboard port and existing service values.
- Modify `src/lib.rs`: export Phase 3 modules.
- Modify `Cargo.toml` and `Cargo.lock`: TOML and SHA-256 dependencies.
- Create `tests/config.rs`, `tests/detection.rs`, `tests/integration_manager.rs`, `tests/doctor.rs`, and `tests/setup_commands.rs`.
- Modify `tests/cli_help.rs`, `tests/service_startup.rs`, `tests/dashboard_api.rs`, and `tests/privacy_contract.rs`.
- Modify `README.md`: Phase 3 commands, files, privacy, and local verification.

## Task 1: Define Paths And Typed Settings

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `src/lib.rs`
- Modify: `src/paths.rs`
- Create: `src/config.rs`
- Create: `tests/config.rs`

- [ ] **Step 1: Add failing path and default-setting tests**

Create tests proving the new files stay under existing per-user directories and defaults preserve Phase 2 behavior:

```rust
#[test]
fn phase_three_paths_are_scoped_to_existing_app_directories() {
    let paths = test_paths();
    assert_eq!(paths.settings_file(), paths.config_dir.join("settings.toml"));
    assert_eq!(paths.manifest_file(), paths.config_dir.join("integrations.json"));
    assert_eq!(paths.backup_dir(), paths.data_dir.join("backups"));
}

#[test]
fn defaults_preserve_existing_runtime_behavior() {
    let settings = Settings::default();
    assert_eq!(settings.version, 1);
    assert_eq!(settings.service.idle_timeout_seconds, 300);
    assert_eq!(settings.service.dashboard_port, DashboardPort::Auto);
    assert!(settings.history.enabled);
    assert_eq!(settings.history.retention_days, 30);
    assert!(settings.presentation.status_enabled);
    assert!(settings.diagnostics.cpu);
    assert!(settings.diagnostics.memory);
}
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `cargo test --locked --test config`

Expected: compilation fails because `Settings` and the path helpers do not exist.

- [ ] **Step 3: Add dependencies and the closed settings model**

Add `toml` and `sha2` using Cargo, export `config`, and implement these public types with `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, and `deny_unknown_fields`:

```rust
pub struct Settings {
    pub version: u32,
    pub service: ServiceSettings,
    pub history: HistorySettings,
    pub presentation: PresentationSettings,
    pub diagnostics: DiagnosticSettings,
    pub logging: LoggingSettings,
    pub integrations: IntegrationSettings,
}

pub enum DashboardPort { Auto, Fixed(u16) }
pub enum LogLevel { Error, Warn, Info, Debug, Trace }
```

Implement the exact defaults from the design and add `AppPaths::settings_file`, `manifest_file`, and `backup_dir`.

- [ ] **Step 4: Add validation tests and minimal validation**

Test unsupported versions, idle timeout bounds, fixed port bounds, retention bounds, ambient intensity, invalid identifiers, and duplicates. Implement:

```rust
impl Settings {
    pub fn validate(&self) -> Result<(), ConfigError>;
    pub fn idle_timeout(&self) -> Duration;
    pub fn history_retention(&self) -> Duration;
}
```

Errors identify the invalid key without serializing the complete settings object.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test --locked --test config`

Expected: all Task 1 tests pass.

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/paths.rs src/config.rs tests/config.rs
git commit -m "feat: define typed user settings"
```

## Task 2: Load, Update, Back Up, And Atomically Write Settings

**Files:**
- Modify: `src/config.rs`
- Modify: `tests/config.rs`

- [ ] **Step 1: Write failing persistence tests**

Cover:

```rust
#[test]
fn missing_settings_load_defaults_without_creating_a_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    assert_eq!(load(&path).unwrap(), Settings::default());
    assert!(!path.exists());
}

#[test]
fn unknown_fields_and_empty_files_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    std::fs::write(&path, "unknown = true\n").unwrap();
    assert!(load(&path).unwrap_err().to_string().contains("unknown"));
    std::fs::write(&path, "").unwrap();
    assert!(load(&path).is_err());
}

#[test]
fn set_key_creates_a_backup_and_writes_normalized_toml() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    save_atomic(&path, &Settings::default()).unwrap();
    let mut settings = load(&path).unwrap();
    set_key(&mut settings, "history.retention_days", "14").unwrap();
    let backup = save_with_backup(&path, &temp.path().join("backups"), &settings)
        .unwrap()
        .unwrap();
    assert!(backup.exists());
    assert_eq!(load(&path).unwrap().history.retention_days, 14);
}

#[test]
fn failed_validation_leaves_the_original_file_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    save_atomic(&path, &Settings::default()).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut invalid = Settings::default();
    invalid.history.retention_days = 0;
    assert!(save_atomic(&path, &invalid).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
```

Also assert Unix settings and backups use mode `0600`.

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `cargo test --locked --test config`

Expected: failures for missing load/save/update APIs.

- [ ] **Step 3: Implement strict load and normalized serialization**

Add:

```rust
pub fn load(path: &Path) -> Result<Settings, ConfigError>;
pub fn normalized_toml(settings: &Settings) -> Result<String, ConfigError>;
pub fn save_atomic(path: &Path, settings: &Settings) -> Result<(), ConfigError>;
```

`load` returns defaults only for `NotFound`. It validates parsed settings. `save_atomic` validates first, writes a sibling temporary file, flushes it, applies user-only permissions for a new file, and renames atomically.

- [ ] **Step 4: Implement supported dotted-key mutation and backups**

Add:

```rust
pub fn set_key(settings: &mut Settings, key: &str, value: &str) -> Result<(), ConfigError>;
pub fn reset_key(settings: &mut Settings, key: Option<&str>) -> Result<(), ConfigError>;
pub fn save_with_backup(
    path: &Path,
    backup_dir: &Path,
    settings: &Settings,
) -> Result<Option<PathBuf>, ConfigError>;
```

Support every scalar key in the design plus comma-separated `integrations.agents` and `integrations.renderers`. Use a collision-resistant backup filename derived from timestamp plus a short checksum; never include settings content in the filename.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test --locked --test config`

Expected: all configuration tests pass.

```bash
git add src/config.rs tests/config.rs
git commit -m "feat: persist validated user settings"
```

## Task 3: Add The Configuration CLI

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/app.rs`
- Modify: `tests/cli_help.rs`
- Create: `tests/setup_commands.rs`

- [ ] **Step 1: Write failing CLI parsing and command tests**

Assert help lists `setup`, `config`, `doctor`, and `uninstall`. Parse:

```rust
smt config show
smt config path
smt config set history.retention_days 14
smt config reset history.retention_days
smt config reset --apply
```

Use hidden `--config-dir`, `--runtime-dir`, and `--data-dir` overrides on Phase 3 commands so integration tests never touch the real profile.

- [ ] **Step 2: Run tests and verify they fail**

Run: `cargo test --locked --test cli_help --test setup_commands`

Expected: Clap rejects the new commands.

- [ ] **Step 3: Add Clap command models**

Define:

```rust
Command::Setup(SetupArgs)
Command::Config(ConfigArgs)
Command::Doctor(DoctorArgs)
Command::Uninstall(UninstallArgs)

enum ConfigCommand {
    Show,
    Path,
    Set { key: String, value: String },
    Reset { key: Option<String>, apply: bool },
}
```

Factor test path overrides into a flattened `PathOverrides` type shared by Phase 3 commands.

- [ ] **Step 4: Dispatch config commands**

Implement `show`, `path`, `set`, and reset preview/apply in `app.rs`. Reset preview prints the affected key and normalized proposed value but does not create a file. Successful writes print the settings path and backup path when one exists.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test --locked --test cli_help --test setup_commands`

Expected: configuration command tests pass. At this checkpoint, setup, doctor, and uninstall are covered only by Clap parsing tests; their dispatch arms are added in their dedicated tasks before the complete suite runs.

```bash
git add src/cli.rs src/app.rs tests/cli_help.rs tests/setup_commands.rs
git commit -m "feat: add configuration commands"
```

## Task 4: Propagate Effective Settings Into Runtime Behavior

**Files:**
- Modify: `src/app.rs`
- Modify: `src/client.rs`
- Modify: `src/runner/mod.rs`
- Modify: `src/service/runtime.rs`
- Modify: `tests/service_startup.rs`
- Modify: `tests/dashboard_api.rs`
- Modify: `tests/run_command.rs`

- [ ] **Step 1: Write failing runtime propagation tests**

Add tests proving:

- Disabled history passes `None` as the database file and history endpoints remain gracefully unavailable.
- Configured retention reaches cleanup behavior.
- Fixed dashboard port binds the requested loopback port; `auto` binds port zero.
- Configured idle timeout is passed to detached service startup.
- Disabled status suppresses renderer output without requiring `--no-status`.
- Disabled CPU and memory diagnostics prevent metric sampling events while lifecycle events continue.

- [ ] **Step 2: Run focused tests and verify they fail**

Run: `cargo test --locked --test service_startup --test dashboard_api --test run_command`

Expected: settings cannot yet affect runtime.

- [ ] **Step 3: Resolve settings at process boundaries**

Change service construction to use:

```rust
database_file: settings.history.enabled.then(|| paths.database_file()),
history_retention: settings.history_retention(),
idle_timeout: settings.idle_timeout(),
listen_port: settings.service.dashboard_port.socket_port(),
```

Add `listen_port: Option<u16>` to `ServiceConfig`; bind only `127.0.0.1` with the configured port.

- [ ] **Step 4: Configure client startup and wrapper presentation**

Add a `ServiceStartupOptions` value containing `idle_timeout` and pass it to `ServiceClient::ensure`. The generic runner loads settings once, combines `presentation.status_enabled` with `--no-status`, and gates CPU/memory sampling independently while preserving lifecycle events and child behavior.

Invalid settings produce one concise observability warning and still launch the wrapped command, matching the existing failure contract.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test --locked --test service_startup --test dashboard_api --test run_command`

Expected: all runtime propagation tests pass.

```bash
git add src/app.rs src/client.rs src/runner/mod.rs src/service/runtime.rs tests/service_startup.rs tests/dashboard_api.rs tests/run_command.rs
git commit -m "feat: apply settings to local runtime"
```

## Task 5: Detect Privacy-Safe Local Capabilities

**Files:**
- Create: `src/detection.rs`
- Modify: `src/lib.rs`
- Create: `tests/detection.rs`

- [ ] **Step 1: Write failing detection tests**

Use injected environment and executable lookup interfaces to prove:

```rust
assert_eq!(report.shell, Some(ShellId::Zsh));
assert!(report.agents.contains(&AgentId::Codex));
assert!(report.terminals.contains(&TerminalId::Ghostty));
```

Serialize the report and assert it contains no executable path, home path, raw `PATH`, username, hostname, or unknown environment value.

- [ ] **Step 2: Run tests and verify they fail**

Run: `cargo test --locked --test detection`

Expected: module and report types are missing.

- [ ] **Step 3: Implement detection behind injectable traits**

Define closed identifiers and:

```rust
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
```

The production source searches `PATH` without returning discovered paths. Sort and deduplicate every identifier list.

- [ ] **Step 4: Run tests and commit**

Run: `cargo test --locked --test detection`

Expected: all detection and privacy assertions pass.

```bash
git add src/lib.rs src/detection.rs tests/detection.rs
git commit -m "feat: detect local integration capabilities"
```

## Task 6: Define Manifests And Managed Text Edits

**Files:**
- Create: `src/manifest.rs`
- Create: `src/integration/mod.rs`
- Create: `src/integration/managed.rs`
- Modify: `src/lib.rs`
- Create: `tests/integration_manager.rs`

- [ ] **Step 1: Write failing manifest and marker tests**

Cover strict manifest loading, stable sorting, and no copied content. Cover managed blocks:

```text
# >>> SaveMyTerminal:example >>>
managed line
# <<< SaveMyTerminal:example <<<
```

Tests must prove insertion preserves unrelated content, replacement changes only one block, removal leaves surrounding bytes intact, and missing/duplicate/malformed/nested markers are conflicts.

- [ ] **Step 2: Run tests and verify they fail**

Run: `cargo test --locked --test integration_manager`

Expected: manifest and integration modules are missing.

- [ ] **Step 3: Implement strict manifest storage**

Define:

```rust
pub struct IntegrationManifest {
    pub version: u32,
    pub integrations: Vec<IntegrationRecord>,
}

pub struct IntegrationRecord {
    pub id: String,
    pub descriptor_version: u32,
    pub target_path: PathBuf,
    pub marker_id: String,
    pub backup_path: Option<PathBuf>,
    pub post_write_sha256: String,
    pub applied_at_unix_ms: u64,
}
```

Use `deny_unknown_fields`, version `1`, unique IDs and target/marker pairs, normalized sorting, and atomic JSON persistence.

- [ ] **Step 4: Implement managed-block parsing and transformations**

Expose:

```rust
pub fn insert_or_replace(original: &str, marker: &Marker, body: &str) -> Result<String, ManagedError>;
pub fn remove(original: &str, marker: &Marker) -> Result<String, ManagedError>;
pub fn inspect(original: &str, marker: &Marker) -> Result<BlockState, ManagedError>;
```

Normalize the managed body to one trailing newline while preserving all unrelated bytes. Marker IDs must pass the same safe identifier validation as integration settings.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test --locked --test integration_manager`

Expected: manifest and managed-edit tests pass.

```bash
git add src/lib.rs src/manifest.rs src/integration/mod.rs src/integration/managed.rs tests/integration_manager.rs
git commit -m "feat: track managed integration ownership"
```

## Task 7: Apply Plans With Backup, Validation, And Rollback

**Files:**
- Create: `src/integration/apply.rs`
- Modify: `src/integration/mod.rs`
- Modify: `src/manifest.rs`
- Modify: `tests/integration_manager.rs`

- [ ] **Step 1: Write failing planning and apply tests**

Cover dry-run immutability, precondition mismatch, backup creation, atomic replacement, validator success, validator failure rollback, and unchanged manifest on failure.

Use an injected validator:

```rust
pub trait Validator: Send + Sync {
    fn validate(&self, target: &Path) -> Result<(), String>;
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run: `cargo test --locked --test integration_manager`

Expected: apply APIs are missing.

- [ ] **Step 3: Define descriptors and immutable plans**

Define a text descriptor containing ID, version, target path, comment prefix, managed body, availability, and optional validator. Planning reads the current target once and returns:

```rust
pub struct IntegrationPlan {
    pub id: String,
    pub target: PathBuf,
    pub action: PlanAction,
    pub before_sha256: Option<String>,
    pub after_sha256: String,
    pub preview: String,
}
```

Bound previews by lines and bytes so setup cannot dump an entire user configuration.

- [ ] **Step 4: Implement apply and per-edit rollback**

Immediately re-read and verify `before_sha256`, create the backup before mutation, atomically write proposed content, run validation, restore original content or remove a newly created target on failure, then update the manifest only after success.

Use SHA-256 only for change detection and integrity; do not present it as a security boundary.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test --locked --test integration_manager`

Expected: all apply, conflict, backup, validator, and rollback tests pass.

```bash
git add src/integration/mod.rs src/integration/apply.rs src/manifest.rs tests/integration_manager.rs
git commit -m "feat: apply integration plans safely"
```

## Task 8: Implement Setup And Uninstall Workflows

**Files:**
- Modify: `src/app.rs`
- Modify: `src/cli.rs`
- Modify: `src/integration/mod.rs`
- Modify: `tests/setup_commands.rs`
- Modify: `tests/integration_manager.rs`

- [ ] **Step 1: Write failing setup tests**

Test that default setup:

- Prints detected OS, shell, known agents, and terminal identifiers.
- Previews settings creation when missing.
- Does not mutate without `--apply`.
- Creates valid default settings with `--apply`.
- Rejects unknown selected integration IDs.

- [ ] **Step 2: Write failing uninstall tests**

Test that default uninstall preserves settings, token, database, and managed files. With `--apply`, remove selected managed blocks only. `--remove-config` removes settings/token but preserves data. `--purge-data` removes the database only when explicitly applied.

- [ ] **Step 3: Run tests and verify they fail**

Run: `cargo test --locked --test setup_commands --test integration_manager`

Expected: setup and uninstall dispatch are incomplete.

- [ ] **Step 4: Implement setup orchestration**

Register no external descriptors in Phase 3. Render capability detection and the settings creation plan. Apply settings atomically only with `--apply`. Keep descriptor selection plumbing ready for Phases 4 and 5.

- [ ] **Step 5: Implement uninstall orchestration**

Load the manifest, plan marker removals, and apply through the same precondition/backup/validation engine. Remove manifest records only after successful target writes. Remove runtime discovery/lock files best-effort. Require `--apply` for every mutation flag.

- [ ] **Step 6: Run tests and commit**

Run: `cargo test --locked --test setup_commands --test integration_manager`

Expected: setup and uninstall workflows pass, including no-mutation previews.

```bash
git add src/app.rs src/cli.rs src/integration/mod.rs tests/setup_commands.rs tests/integration_manager.rs
git commit -m "feat: add reversible setup and uninstall"
```

## Task 9: Add Structured Doctor Checks

**Files:**
- Create: `src/doctor.rs`
- Modify: `src/lib.rs`
- Modify: `src/app.rs`
- Create: `tests/doctor.rs`
- Modify: `tests/setup_commands.rs`

- [ ] **Step 1: Write failing doctor model tests**

Define expected semantics:

```rust
assert_eq!(report.exit_code(), 0); // pass + warning only
assert_eq!(failed_report.exit_code(), 1);
```

Test invalid settings, insecure Unix token/database modes, non-loopback discovery, absent on-demand service, malformed manifests, missing markers, missing backups, and checksum drift.

- [ ] **Step 2: Run tests and verify they fail**

Run: `cargo test --locked --test doctor --test setup_commands`

Expected: doctor module and CLI behavior are missing.

- [ ] **Step 3: Implement independent structured checks**

Define:

```rust
pub enum CheckLevel { Pass, Warn, Fail }
pub struct CheckResult { pub id: &'static str, pub level: CheckLevel, pub message: String }
pub struct DoctorReport { pub checks: Vec<CheckResult> }
```

Each check handles its own errors and appends a result so one corrupt optional file does not suppress unrelated diagnostics. Messages contain paths only when needed for remediation and never include file contents or token values.

- [ ] **Step 4: Render doctor output and exit status**

Print deterministic `PASS`, `WARN`, and `FAIL` lines followed by totals. Treat absent discovery as a pass because service startup is on demand. If discovery exists, require a loopback URL before attempting health.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test --locked --test doctor --test setup_commands`

Expected: all doctor checks and CLI exit tests pass.

```bash
git add src/lib.rs src/doctor.rs src/app.rs tests/doctor.rs tests/setup_commands.rs
git commit -m "feat: diagnose local setup health"
```

## Task 10: Privacy Hardening, Documentation, And Final Verification

**Files:**
- Modify: `tests/privacy_contract.rs`
- Modify: `README.md`
- Modify: any Phase 3 file required by findings

- [ ] **Step 1: Add failing privacy and policy tests**

Assert normalized settings and manifests contain no fields for prompts, responses, terminal output, command arguments, working directories, file contents, environment values, usernames, hostnames, remotes, or credentials. Assert detection serialization includes only closed identifiers. Keep the existing no-active-GitHub-Actions test.

- [ ] **Step 2: Run privacy tests and fix any findings**

Run: `cargo test --locked --test privacy_contract`

Expected: all privacy tests pass after narrowly scoped fixes.

- [ ] **Step 3: Document exact Phase 3 behavior**

Update README with:

- Config, setup, doctor, and uninstall examples.
- Default preview behavior and explicit `--apply` requirement.
- Settings, manifest, backup, and database locations.
- Retention/persistence/status/diagnostic controls.
- Backup versus managed-marker ownership semantics.
- History preservation unless `--purge-data` is explicitly applied.
- Local-only verification and absence of GitHub Actions.

- [ ] **Step 4: Run focused quality review**

Review the aggregate diff for hidden assumptions, unnecessary abstractions, prohibited-data leakage, unsafe path handling, non-atomic mutations, unbounded previews, and behavior changes outside Phase 3.

- [ ] **Step 5: Run complete local verification**

Run:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo build --locked --release
cargo test --locked --all-targets --all-features -- --test-threads=1
git diff --check
test -z "$(find .github/workflows -type f -print 2>/dev/null)"
SMT_TEST_FORCE_SERVICE_FAILURE=1 target/release/smt run --no-status -- sh -c 'printf secret-output'
```

Expected: all commands pass and smoke output is exactly `secret-output`.

- [ ] **Step 6: Commit final hardening and docs**

```bash
git add README.md tests/privacy_contract.rs
git commit -m "docs: verify phase 3 setup and configuration"
```

## Final Phase 3 Review

Before calling Phase 3 complete:

- [ ] All commands preserve Phase 1 and Phase 2 behavior by default.
- [ ] Invalid user configuration is reported and never silently replaced.
- [ ] Ordinary commands do not create settings files when defaults are sufficient.
- [ ] Setup and uninstall previews perform no mutations.
- [ ] Every external edit has a precondition checksum, backup, atomic write, validator hook, and rollback path.
- [ ] Uninstall removes managed markers rather than restoring complete stale backups.
- [ ] Manifests contain ownership metadata but no copied user configuration.
- [ ] Doctor checks continue after independent failures.
- [ ] The wrapper still launches the child when configuration or observability fails.
- [ ] No active GitHub Actions workflow exists or runs.
- [ ] The branch is clean and the aggregate diff is limited to Phase 3.
