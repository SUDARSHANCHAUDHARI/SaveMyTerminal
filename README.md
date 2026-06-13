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

Check whether the on-demand local service is already running:

```bash
cargo run --bin smt -- status
```

`smt run` preserves the child process's standard input, output, error streams, arguments, signals, and exit result. SaveMyTerminal starts its loopback service on demand and exits that service after five idle minutes.

## Privacy

Phase 1 sends only random session identifiers, fixed agent/adapter identifiers, lifecycle state, exit category, CPU, and memory metadata to a token-authenticated loopback service. Metric values are labeled with their quality and source.

It does not capture or store:

- Prompts or model responses
- Terminal output
- Raw command lines or command arguments
- Environment values
- File contents or working-directory paths

The service makes no outbound network requests during normal operation. Phase 1 keeps session state in memory and does not yet persist history.

## Current Scope

Phase 1 provides the universal wrapper, validated metadata protocol, authenticated in-memory service, portable status renderer, and process metrics.

The following approved features are planned for later phases:

- SQLite summaries and the local dashboard
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
