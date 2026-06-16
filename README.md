# SaveMyTerminal

![Rust](https://img.shields.io/badge/Rust-1.95+-CE412B?logo=rust&logoColor=white)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)
![License: MIT](https://img.shields.io/badge/License-MIT-blue)
![Privacy](https://img.shields.io/badge/telemetry-none-success)

SaveMyTerminal adds local, privacy-safe lifecycle and resource visibility to terminal-based
AI agents. It works with any command through a universal wrapper and adds optional native
hooks and terminal presentation where supported. In Ghostty, an attached agent can drive a
state-reactive black-hole shader without exposing prompts, responses, or terminal output.

## Features

- **Universal wrapper** — `smt run -- <any-command>` observes lifecycle and resource use while
  the child keeps its input, output, error streams, arguments, signals, and exit code.
- **Native agent hooks** — first-class lifecycle adapters for Codex, Claude Code, and Gemini CLI,
  with a generic fallback for everything else.
- **State-reactive Ghostty shader** — a black hole whose color and speed track the agent's state
  (starting, thinking, tool-running, waiting); its accretion disk grows with context pressure.
- **Multi-terminal presentation** — Ghostty shader, Kitty ambient image, WezTerm/iTerm2 status,
  and a portable text fallback for everything else.
- **Authenticated local dashboard** — active sessions, 30-day history, and per-session deletion
  over a loopback-only service.
- **Privacy-first by construction** — no prompts, responses, output, command lines, arguments,
  environment values, file contents, or paths. No remote endpoint, no telemetry.
- **Safe, reversible setup** — bounded previews, checksums, backups, atomic writes, validators,
  and exact owned-entry removal.

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
SaveMyTerminal records only approved lifecycle and resource metadata. Use the wrapper when you
want terminal animation: native hooks alone update snapshots and the dashboard, but they cannot
write presentation signals into an already-running terminal process.

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

### Ghostty Black-Hole Mode

Install the native agent hooks and Ghostty shader, then restart Ghostty:

```bash
smt setup --integration claude --apply
smt setup --integration ghostty --apply
```

On macOS, the managed block is written to Ghostty's documented `config.ghostty` file. Start the
agent through the attached wrapper:

```bash
smt run -- claude
```

The shader reacts to normalized states: indigo while starting, purple while thinking, amber
while a tool runs, and cyan while waiting. Animation speed changes with state. When context
pressure is available, it changes the accretion-disk radius and the disk shifts toward red as
the window approaches full; otherwise the shader uses a neutral fallback.
Context pressure remains unavailable when an agent's hook payload does not expose safe usage
metadata, unless you enable the optional transcript source described below.

By default SaveMyTerminal never opens transcript files. If you want the disk to track context
for an agent that records local usage counters (such as Claude Code), you can opt in:

```bash
smt config set presentation.context_from_transcript true
```

When enabled, only the numeric token counters and model identifier are read from the agent's
local transcript — never prompt or response text. It stays off unless you set it, and you can
disable it again at any time:

```bash
smt config set presentation.context_from_transcript false
```

Kitty receives the generated ambient image, while WezTerm and iTerm2 show snapshot-driven status.
Those integrations are intentionally not described as visually identical to Ghostty.

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
full-history purge. Live and historical views include context pressure when available and label
its quality. Browser launch uses a random single-use URL exchanged for an `HttpOnly`,
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
the [verification matrix](docs/verification-matrix.md), and the
[release checklist](docs/release-checklist.md).

## Tech Stack

- **Language:** Rust 2024 (edition), Rust 1.95+
- **Async runtime:** Tokio
- **Local service:** Axum over loopback, authenticated with a per-install token
- **Storage:** SQLite (bundled `rusqlite`), private file mode on Unix
- **CLI:** clap
- **Serialization:** Serde / `serde_json`, TOML for settings
- **Presentation:** Ghostty GLSL shader, Kitty PNG asset, WezTerm Lua, iTerm2 Python, terminal OSC
- **Metrics:** `sysinfo` for process CPU and memory

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

## Acknowledgements

The Ghostty presentation idea is inspired by
[ghostty-blackhole](https://github.com/s0xDk/ghostty-blackhole) by **s0xDk** — making agent
activity an ambient part of the terminal. SaveMyTerminal is an independent, privacy-first
reimplementation with its own original shader, a universal wrapper and native agent hooks,
multi-terminal renderers, and a local service, dashboard, and history.

## License

Released under the [MIT License](LICENSE). Copyright (c) 2026 Sudarshan Chaudhari.

## Author

Built by [Sudarshan Chaudhari](https://github.com/SUDARSHANCHAUDHARI).
