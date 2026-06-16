# Changelog

All notable changes to SaveMyTerminal are documented here.

## Unreleased

### Fixed

- Replaced the static Ghostty corner glow with a state-reactive black-hole shader.
- Linked native hook events to their attached wrapper session to prevent cross-session display.
- Added privacy-safe optional context pressure to live snapshots, history, dashboard, and shader.
- Corrected Ghostty's macOS managed configuration target to `config.ghostty`.
- Ensured the wrapper retains ownership of exact child completion and terminal cleanup.

## 1.0.0 - 2026-06-13

### Added

- Universal `smt run -- <command>` lifecycle and process-resource observation.
- Authenticated loopback service, embedded dashboard, and privacy-safe SQLite summaries.
- Typed configuration, diagnostics, reversible setup, backups, and uninstall controls.
- Native lifecycle hooks for Codex, Claude Code, and Gemini CLI.
- Snapshot-driven integrations for Ghostty, Kitty, WezTerm, and iTerm2.
- Local macOS, Linux, and Windows packaging, checksums, and installers.

### Privacy

- No prompts, responses, terminal output, raw arguments, environment values, file contents,
  or working-directory paths are captured.
- Normal runtime operation has no remote endpoint and communicates only over authenticated
  loopback HTTP.
