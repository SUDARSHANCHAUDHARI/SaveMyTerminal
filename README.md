# SaveMyTerminal

SaveMyTerminal adds local, privacy-safe lifecycle and resource visibility to terminal-based AI agents.

## Run An Agent

Run any command through the generic adapter:

```bash
cargo run --bin smt -- run -- codex
```

Disable concise status messages:

```bash
cargo run --bin smt -- run --no-status -- codex
```

Check whether the on-demand local service is already running:

```bash
cargo run --bin smt -- status
```

`smt run` preserves the child process's standard input, output, error streams, arguments, signals, and exit result. SaveMyTerminal starts its loopback service on demand and exits that service after five idle minutes.

## Local Dashboard

Open the authenticated dashboard in the system browser:

```bash
cargo run --bin smt -- dashboard
```

The dashboard shows active sessions through a local live stream and privacy-safe finalized history. It includes 30-day summary statistics, per-session deletion, and full-history purge.

The CLI uses the long-lived installation token only to request a random, single-use launch URL. The browser exchanges that short-lived token for an `HttpOnly`, same-origin session cookie; the installation token is never embedded in dashboard HTML or JavaScript.

History is stored in `sessions.sqlite3` under the operating system's per-user application data directory. Persistence is enabled by default, retention is 30 days, and unfinished sessions recover as `interrupted` after a service restart. On Unix, the data directory is `0700` and the database is `0600`.

## Setup And Configuration

Inspect the effective settings without creating a file:

```bash
cargo run --bin smt -- config show
cargo run --bin smt -- config path
```

Change one validated setting:

```bash
cargo run --bin smt -- config set history.retention_days 14
cargo run --bin smt -- config set history.enabled false
cargo run --bin smt -- config set presentation.status_enabled false
cargo run --bin smt -- config set diagnostics.cpu false
cargo run --bin smt -- config set service.dashboard_port 43123
```

Reset previews by default. Add `--apply` to write the change:

```bash
cargo run --bin smt -- config reset history.retention_days
cargo run --bin smt -- config reset history.retention_days --apply
```

`smt setup` detects the local operating system, shell, known agents, and known terminals. It previews creating the core settings file and performs no mutation unless `--apply` is supplied:

```bash
cargo run --bin smt -- setup
cargo run --bin smt -- setup --apply
```

Phase 3 provides the safe managed-integration engine, including bounded previews, precondition checksums, backups, atomic writes, validator hooks, rollback, manifests, and exact managed-marker removal. Codex, Claude Code, and Gemini CLI descriptors are added in Phase 4; terminal descriptors are added in Phase 5.

Run local diagnostics:

```bash
cargo run --bin smt -- doctor
```

Doctor reports settings validity, private file permissions, loopback service state, manifest integrity, marker ownership, backup availability, checksum drift, and the absence of remote endpoint or telemetry configuration. Warnings do not fail the command; failed safety checks return exit code `1`.

Uninstall is also preview-only unless `--apply` is supplied:

```bash
cargo run --bin smt -- uninstall
cargo run --bin smt -- uninstall --remove-config --apply
cargo run --bin smt -- uninstall --purge-data --apply
```

Managed integration blocks are removed by their current markers, never by replacing a complete user file with an old backup. Settings and the authentication token are preserved unless `--remove-config` is applied. Session history is preserved unless `--purge-data` is explicitly applied.

Per-user files are stored under the operating system's application directories:

- `settings.toml` and `auth.token` in the config directory
- `integrations.json` in the config directory
- Timestamped pre-edit backups under the data directory's `backups` folder
- `sessions.sqlite3` in the data directory

The settings schema is versioned and rejects unknown keys. Missing settings use privacy-oriented defaults without creating a file during ordinary commands.

## Privacy

SaveMyTerminal sends only random session identifiers, fixed agent/adapter identifiers, lifecycle state, exit category, duration, CPU, memory, and approved aggregate metadata to a token-authenticated loopback service. Metric values are labeled with their quality and source.

It does not capture or store:

- Prompts or model responses
- Terminal output
- Raw command lines or command arguments
- Environment values
- File contents or working-directory paths

The service and embedded dashboard make no outbound network requests during normal operation. Live event details remain in memory; SQLite receives only the closed summary schema documented above.

## Current Scope

Phases 1 through 3 provide the universal wrapper, validated metadata protocol, authenticated local service, portable status renderer, process metrics, SQLite summaries, live streaming, embedded dashboard, typed settings, capability detection, reversible setup foundations, diagnostics, and uninstall controls.

The following approved features are planned for later phases:

- Native Codex, Claude Code, and Gemini CLI hooks
- Ghostty, Kitty, WezTerm, and iTerm2 renderers
- Ambient terminal effects and release packaging

See the [v1 design](docs/superpowers/specs/2026-06-13-savemyterminal-design.md) and [implementation roadmap](docs/superpowers/plans/2026-06-13-savemyterminal-v1-roadmap.md).

## Development

Requires Rust 1.95 or newer.

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
```

Verification runs locally. This repository intentionally has no active GitHub Actions workflow.
