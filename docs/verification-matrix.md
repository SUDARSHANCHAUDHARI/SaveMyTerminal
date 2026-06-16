# V1 Verification Matrix

This matrix tracks the top-level V1 acceptance criteria against production code, automated
verification, and manual platform evidence. A passing phase checklist is not sufficient on its
own. Manual items remain explicitly pending until they are observed on the named platform.

| # | Acceptance criterion | Implementation evidence | Automated evidence | Manual status |
| --- | --- | --- | --- | --- |
| 1 | Wrapper preserves child I/O, arguments, signals, and exit status | `runner/child.rs`, `runner/mod.rs` | `run_command`, `cli_help` | macOS host smoke verified; Linux and Windows native runs pending |
| 2 | Codex, Claude, and Gemini native lifecycle adapters | `adapter.rs`, `agents.rs` | `adapter_contract`, `hook_command`, `integration_manager` | Codex lifecycle observed on macOS; Claude and Gemini manual runs pending |
| 3 | Unknown agents receive generic lifecycle and process metrics | `runner/mod.rs`, `runner/metrics.rs` | `run_command`, metric unit tests | macOS host smoke verified |
| 4 | Native terminal integrations with portable fallback | `terminals.rs`, `terminal_assets.rs`, renderer modules | `terminal_assets`, `setup_commands`, `renderer_contract` | Ghostty 1.3.1 loads managed shader; pixel inspection and Kitty/WezTerm/iTerm2 checks pending |
| 5 | State and context pressure render together | protocol context metric, exact-session hook linking, OSC signal, Ghostty shader | adapter, renderer, relay, privacy, and storage tests | State transport observed; context remains unavailable when agent hooks omit usage metadata |
| 6 | Diagnostics are independently configurable | `config.rs`, `runner/mod.rs` | `config`, `run_command` | macOS host verified |
| 7 | Authenticated live dashboard and 30-day history | dashboard, service API, SQLite storage | `dashboard_api`, `dashboard_command`, `storage` | Local dashboard API verified |
| 8 | Prohibited content is absent | closed protocol/storage schemas and bounded adapters | `privacy_contract`, storage schema tests | Automated contract verified |
| 9 | Setup is previewable, backed up, validated, reversible, and ownership-scoped | integration manager, manifest, doctor, uninstall | `integration_manager`, `setup_commands`, `doctor` | Codex and Ghostty managed installs verified on macOS |
| 10 | Observation failure never blocks the agent | best-effort service client and renderer writes | fallback tests in `run_command`, hook neutral-success tests | macOS fallback verified |
| 11 | Normal runtime has no outbound endpoint | loopback-only service and embedded assets | release URL scan, privacy and no-network contracts | Automated contract verified |
| 12 | Cross-platform tests followed by manual terminal checks | conditional platform modules and packaging scripts | full macOS suite and Apple Silicon package verification | Linux/Windows native suites and remaining terminal visual checks pending |

## Ghostty Evidence

- Ghostty 1.3.1 is installed on the Apple Silicon host.
- The managed macOS target is `~/Library/Application Support/com.mitchellh.ghostty/config.ghostty`.
- `ghostty +show-config` reports the generated SaveMyTerminal shader path.
- `smt doctor` validates the Codex and Ghostty managed targets, checksums, and backups.
- The relay integration test proves that only the attached wrapper UUID receives hook-driven
  state updates, even while a competing session is newer.
- Pixel-level animation inspection remains manual because this execution environment cannot
  capture the macOS display.

## Completion Rule

Do not report V1 as fully platform-verified while any manual status above remains pending.
Implementation completeness and release verification must be reported separately.
