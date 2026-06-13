# SaveMyTerminal Phase 3: Setup And Configuration Design

**Date:** 2026-06-13
**Status:** Approved through the existing V1 product design
**Target:** Phase 3 of SaveMyTerminal V1

## Summary

Phase 3 adds a typed per-user configuration system and the reversible integration-management foundation required by later agent and terminal phases. Users can inspect and change settings, preview setup changes, run diagnostics, and uninstall only SaveMyTerminal-owned state.

The implementation preserves the existing privacy and reliability boundaries: configuration contains no captured terminal data, setup never changes an external file without an explicit apply flag, writes are atomic, validation failure rolls back the current edit, and normal operation performs no outbound network requests.

## Scope

Phase 3 includes:

- A versioned TOML settings file with strict parsing and validated defaults.
- `smt config show`, `smt config path`, `smt config set`, and `smt config reset`.
- Runtime use of persistence, retention, idle timeout, dashboard port, status, diagnostics, and logging settings.
- Detection of the operating system, current shell, known agent executables, and known terminal environment.
- `smt setup` dry-run previews and explicit `--apply` behavior.
- A reusable managed-text integration engine for later agent and terminal descriptors.
- Timestamped backups, atomic writes, optional validators, rollback, and manifest records.
- `smt doctor` checks for settings, permissions, local service, storage, and manifest integrity.
- `smt uninstall` preview and explicit removal of managed entries.
- Optional removal of SaveMyTerminal-owned configuration and data through explicit flags.

Phase 3 does not include:

- Codex, Claude Code, or Gemini CLI hook descriptors. Those are Phase 4.
- Ghostty, Kitty, WezTerm, or iTerm2 renderer descriptors and assets. Those are Phase 5.
- Package installers, update checks, release CI, or compatibility matrices. Those are Phase 6.
- Any cloud service, analytics, crash upload, or normal-operation outbound request.

## Design Decisions

### Typed Settings Rather Than Arbitrary Key-Value Storage

The settings file is deserialized into a closed Rust model with `deny_unknown_fields`. This prevents misspelled keys from silently doing nothing and prevents unrelated data from becoming part of the configuration surface.

The file is stored at `AppPaths::settings_file()` as `settings.toml`. Missing files use defaults. Empty or invalid files return an actionable error; they are not silently replaced.

### Effective Settings Are Resolved Before Service Startup

The CLI loads settings and passes resolved values into `ServiceConfig`. The service remains unaware of TOML syntax and user-facing key names.

`ServiceClient::ensure` also loads the idle-timeout setting before spawning the detached service. Hidden service arguments remain available for tests and override only the values explicitly supplied by the test harness.

### Preview Is The Default Mutation Mode

`smt setup` and `smt uninstall` are non-mutating by default. They print an ordered plan containing the integration identifier, target path, action, and a bounded human-readable diff. Mutation requires `--apply`.

This is intentionally stricter than a confirmation prompt because it behaves consistently in interactive terminals, scripts, and tests.

### Manifests Record Ownership, Not User Content

Each applied external integration creates or updates a manifest record containing:

- Integration identifier and descriptor version.
- Target path.
- Managed marker identifier.
- Backup path, when a pre-existing file was changed.
- SHA-256 checksum of the complete post-write target.
- Applied timestamp.

The manifest never stores the target file contents. It is JSON because it is machine-owned state, while user settings remain TOML.

### Managed Markers Are The Uninstall Authority

Text integrations use explicit begin and end markers derived from a stable integration identifier. Uninstall removes only the current marked block. It never replaces the entire current file with an old backup.

If markers are missing, duplicated, malformed, or nested, uninstall reports a conflict and leaves the file unchanged. A checksum mismatch is reported as a warning because users may legitimately edit unrelated parts of the file after setup.

## Configuration Model

The top-level schema version is `1`.

```toml
version = 1

[service]
idle_timeout_seconds = 300
dashboard_port = "auto"

[history]
enabled = true
retention_days = 30

[presentation]
status_enabled = true
status_compact = true
ambient_enabled = true
ambient_intensity = 60

[diagnostics]
cpu = true
memory = true
duration = true
command_health = false

[logging]
level = "warn"

[integrations]
agents = []
renderers = []
```

Validation rules:

- `version` must equal `1`.
- Idle timeout must be between 5 and 86,400 seconds.
- Dashboard port is `auto` or an integer from 1024 through 65,535.
- Retention must be between 1 and 3,650 days when history is enabled.
- Ambient intensity must be from 0 through 100.
- Logging level is `error`, `warn`, `info`, `debug`, or `trace`.
- Integration identifiers are lowercase ASCII names containing letters, digits, `_`, or `-`.
- Duplicate integration identifiers are rejected.

Unknown fields and unknown enum values are errors. Configuration errors include the settings path but never echo full file contents.

## CLI Surface

### `smt config`

- `smt config show`: print the effective configuration as normalized TOML.
- `smt config path`: print the settings path without creating it.
- `smt config set <key> <value>`: validate and atomically write one supported key.
- `smt config reset [<key>]`: preview resetting one key or all settings; `--apply` performs the write.

`config set` accepts only documented dotted keys. It writes immediately because the requested key and value are explicit in the command. Before changing an existing settings file, it creates a timestamped backup in the backup directory.

### `smt setup`

`smt setup` reports detected capabilities and previews the core settings file creation when it is missing. Later phases register agent and renderer descriptors with the same planner.

Options:

- `--apply`: apply the displayed plan.
- `--integration <id>`: limit planning to selected registered descriptors; repeatable.
- `--config-dir`, `--data-dir`, and `--home-dir`: hidden test overrides.

An unknown or unavailable selected integration is an error. With no registered external descriptors in Phase 3, detection remains informative and setup can create only the core settings file.

### `smt doctor`

Doctor prints one line per check with `PASS`, `WARN`, or `FAIL`, followed by a summary. It exits:

- `0` when there are no failures.
- `1` when at least one check fails.

Checks include:

- Settings parse and validation.
- Config directory and sensitive file permissions on Unix.
- Auth token permissions when the token exists.
- Database permissions when the database exists.
- Service discovery binding is loopback when discovery exists.
- Local service reachability when discovery exists.
- Manifest parse and target-marker integrity.
- Manifest backup references and post-write checksum drift.
- Confirmation that no remote endpoint or telemetry setting exists.

An absent service is `PASS` with an on-demand note, not a failure.

### `smt uninstall`

Uninstall previews removal of all or selected managed integrations. `--apply` performs removals atomically and updates the manifest.

Options:

- `--apply`: perform the previewed removals.
- `--integration <id>`: limit removal; repeatable.
- `--remove-config`: also remove SaveMyTerminal's settings, token, and empty config directory.
- `--purge-data`: also remove the local session database after a clear warning.

Runtime discovery and lock files are always eligible for cleanup because they are ephemeral and SaveMyTerminal-owned. History is preserved unless `--purge-data` is supplied.

## Components

### `config`

Owns the closed settings model, defaults, validation, TOML parsing, normalized serialization, dotted-key updates, and atomic persistence.

### `detection`

Produces a privacy-safe `EnvironmentReport` from local process environment and `PATH` lookup. It reports only known capability identifiers and booleans; it does not persist paths, usernames, hostnames, working directories, or environment values.

Known agent identifiers are `codex`, `claude`, and `gemini`. Known terminal identifiers are `ghostty`, `kitty`, `wezterm`, and `iterm2`. Shell identifiers are `zsh`, `bash`, `fish`, `pwsh`, and `cmd`.

### `integration`

Defines descriptors, plans, diffs, managed-marker parsing, atomic text edits, validator execution, backup creation, rollback, and uninstall planning.

Descriptors provide data; the engine owns mutation. This prevents later adapters and renderers from implementing ad hoc file-writing behavior.

### `manifest`

Owns strict JSON manifest loading and atomic persistence. Manifest schema version `1` contains a sorted list of integration records.

### `doctor`

Runs independent checks and returns structured results. CLI rendering is separate so tests assert check semantics without matching decorative output.

## File Layout

```text
src/config.rs                 typed settings and persistence
src/detection.rs              local capability detection
src/integration/mod.rs        descriptor and planning API
src/integration/managed.rs    managed text block parser/editor
src/integration/apply.rs      backup, atomic write, validate, rollback
src/manifest.rs               ownership manifest model and storage
src/doctor.rs                 structured diagnostics
tests/config.rs               defaults, validation, atomic updates
tests/detection.rs            privacy-safe capability reports
tests/integration_manager.rs  preview, apply, rollback, uninstall
tests/doctor.rs               diagnostic outcomes and exit behavior
tests/setup_commands.rs       CLI setup/config/uninstall workflows
```

## Mutation Algorithm

For each planned file edit:

1. Re-read the target immediately before mutation.
2. Verify the current content still matches the plan precondition checksum.
3. Create the parent directory if required.
4. If the target exists, write a timestamped backup with user-only permissions where supported.
5. Write the complete proposed content to a temporary sibling file.
6. Apply the target's existing permissions to the temporary file, or use user-only permissions for new sensitive files.
7. Flush and atomically rename the temporary file over the target.
8. Run the descriptor validator when one exists.
9. If validation fails, atomically restore the pre-write content and do not update the manifest.
10. On success, atomically update the manifest record.

If a later integration fails, earlier successful integrations remain applied and recorded. The command exits nonzero and reports exactly which integration failed. This avoids risky multi-file global rollback while preserving per-edit atomicity.

## Error Handling

- Invalid settings never start a differently configured service silently.
- Missing settings use defaults without creating files during ordinary commands.
- Setup conflicts never overwrite unknown content.
- Backup failure aborts before target mutation.
- Validator failure restores the target and preserves the old manifest.
- Manifest corruption blocks integration mutation but does not block `smt run` or the dashboard.
- Doctor reports every independent check even when earlier checks fail.
- Uninstall conflicts leave the target and manifest record unchanged.
- Configuration or integration failures never capture or print prohibited terminal content.

## Testing

Unit and integration tests cover:

- Default settings and normalized TOML round trips.
- Unknown keys, unsupported versions, invalid ranges, and duplicate identifiers.
- Atomic settings updates and backup creation.
- Runtime propagation of idle timeout, history enablement, retention, status enablement, diagnostics, and dashboard port.
- Detection without persisting paths or raw environment values.
- Managed-block insertion, replacement, duplicate detection, and exact removal.
- Dry-run setup producing no filesystem mutations.
- Apply creating backups and manifest records.
- Precondition conflicts, validator failure, rollback, and manifest preservation.
- Checksum drift warnings and malformed-marker failures.
- Doctor success, warning, and failure exit behavior.
- Uninstall preserving unrelated user content and history by default.
- Privacy contract checks for settings and manifests.
- Existing Phase 1 and Phase 2 tests.

All verification runs locally. Phase 3 does not add or run a GitHub Actions workflow.

## Completion Criteria

Phase 3 is complete when:

1. Typed settings can be inspected and safely changed.
2. Service startup consumes validated settings and preserves existing defaults.
3. Setup detects supported local capabilities and defaults to a non-mutating preview.
4. Managed edits are backed up, atomic, validated, and rolled back on failure.
5. Manifest records contain ownership metadata but no copied user content.
6. Doctor reports actionable local configuration and integrity results.
7. Uninstall removes only managed blocks unless explicit config or data removal flags are supplied.
8. Later agent and terminal phases can register descriptors without bypassing the integration engine.
9. Privacy tests prove settings, manifests, logs, and errors contain no prohibited captured content.
10. All existing and Phase 3 local checks pass with no active GitHub Actions workflows.
