# SaveMyTerminal

SaveMyTerminal adds local, privacy-safe lifecycle and resource visibility to terminal-based
AI agents. It works with any command through a universal wrapper and adds optional native
hooks and terminal presentation where supported.

## Installation

Build and install from source with Rust 1.95 or newer:

```bash
cargo install --path . --locked
```

Release archives include `smt`, documentation, and an adjacent SHA-256 checksum. Install a
verified local Unix archive with:

```bash
scripts/install.sh dist/savemyterminal-1.0.0-<target>.tar.gz
```

On Windows, run `scripts/install.ps1 <archive.zip>`. Installers only place the executable in
`${SMT_INSTALL_DIR:-$HOME/.local/bin}` (or its PowerShell equivalent); they do not edit shell,
agent, or terminal configuration.

## Run An Agent

Wrap any terminal agent or command:

```bash
smt run -- codex
smt run -- claude
smt run -- gemini
smt run -- your-agent --its-arguments
```

The child retains its input, output, error streams, arguments, signals, and exit result.
SaveMyTerminal records only approved lifecycle and resource metadata.

## Native Agent Hooks

Preview a native integration, then apply it explicitly:

```bash
smt setup --integration codex
smt setup --integration codex --apply
smt setup --integration claude --apply
smt setup --integration gemini --apply
```

Managed changes use bounded previews, checksums, backups, atomic writes, validators, and exact
owned-entry removal. The universal `smt run` adapter remains available for every other agent.

## Terminal Integrations

SaveMyTerminal provides optional renderers for Ghostty, Kitty, WezTerm, and iTerm2:

```bash
smt setup --integration ghostty --apply
smt setup --integration kitty --apply
smt setup --integration wezterm --apply
smt setup --integration iterm2 --apply
```

Setup installs generated local assets and the smallest managed configuration block supported
by the terminal. Unsupported environments fall back to the portable text renderer.

## Snapshot

Terminal integrations consume the same privacy-safe active-session view exposed by:

```bash
smt snapshot
smt snapshot --format json
```

## Dashboard

Open the authenticated loopback dashboard:

```bash
smt dashboard
```

It shows active sessions, finalized local history, 30-day summaries, per-session deletion, and
full-history purge. Browser launch uses a random single-use URL exchanged for an `HttpOnly`,
same-origin cookie; the installation token is never embedded in dashboard assets.

## Configuration And Diagnostics

```bash
smt config show
smt config path
smt config set history.retention_days 14
smt config set history.enabled false
smt doctor
smt status
```

Settings are typed and reject unknown keys. Persistence defaults to 30 days. On Unix, the app
data directory is private and the SQLite database is mode `0600`.

## Uninstall

Removal previews by default and changes only SaveMyTerminal-owned entries:

```bash
smt uninstall
smt uninstall --apply
smt uninstall --remove-config --apply
smt uninstall --purge-data --apply
```

Configuration and history are preserved unless their explicit flags are supplied.

## Privacy

SaveMyTerminal does not capture prompts, responses, terminal output, raw command lines,
arguments, environment values, file contents, or working-directory paths. Normal operation
uses only an authenticated loopback service and has no remote endpoint or telemetry.

See [compatibility](docs/compatibility.md), the [V1 design](docs/superpowers/specs/2026-06-13-savemyterminal-design.md),
and the [release checklist](docs/release-checklist.md).

## Development

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release
scripts/package.sh
scripts/verify-release.sh
```

Verification and release packaging run locally. This repository intentionally has no active
GitHub Actions workflows.
