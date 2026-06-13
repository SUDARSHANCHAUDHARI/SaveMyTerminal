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

Phases 1 and 2 provide the universal wrapper, validated metadata protocol, authenticated local service, portable status renderer, process metrics, SQLite summaries, live streaming, and embedded dashboard.

The following approved features are planned for later phases:

- Guided setup, diagnostics, and uninstall
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
