# SaveMyTerminal Phase 6: Release Hardening Design

**Date:** 2026-06-13
**Status:** Approved through the V1 roadmap
**Target:** SaveMyTerminal 1.0.0 release candidate

## Summary

Phase 6 converts the completed V1 implementation into a reproducible, documented release candidate. It adds local packaging for macOS, Linux, and Windows targets, checksum generation, installation scripts, compatibility declarations, release notes, no-network contracts, and a single local release verification entry point.

The repository intentionally remains free of GitHub Actions workflows. Cross-platform builds are described as a target matrix and executed manually or by a future external release system; Phase 6 does not add hidden automation that contradicts the repository policy.

## Version

The crate and binary version become `1.0.0`. The first stable release includes all six roadmap phases:

- universal wrapper and event protocol
- authenticated local service and process metrics
- SQLite summaries and embedded dashboard
- typed configuration, doctor, and reversible setup
- Codex, Claude Code, and Gemini CLI hooks
- Ghostty, Kitty, WezTerm, and iTerm2 renderers

## Supported Targets

Release packaging declares these initial Rust targets:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

A target is publishable only after its package script, full tests where runnable, smoke test, and manual compatibility row are recorded. Cross-compilation toolchains are not downloaded implicitly.

## Artifacts

Unix packaging creates:

```text
dist/savemyterminal-1.0.0-<target>.tar.gz
dist/savemyterminal-1.0.0-<target>.tar.gz.sha256
```

Windows packaging creates a `.zip` and matching `.sha256` file. Packages contain `smt`/`smt.exe`, `README.md`, `LICENSE`, `CHANGELOG.md`, and `docs/compatibility.md`.

Packaging always uses `cargo build --locked --release`. It rejects a missing binary, derives the package version from Cargo metadata, and recreates only its target-specific staging directory.

## Installation

The Unix installer accepts either a local release archive or a version/target GitHub release URL. It verifies the adjacent SHA-256 file before extraction and installs only the `smt` executable into `${SMT_INSTALL_DIR:-$HOME/.local/bin}`.

The PowerShell installer provides the equivalent Windows flow. Neither installer modifies shell startup files, terminal configuration, or agent hooks. Users run `smt setup` separately so all external changes retain preview/apply semantics.

## Verification

`scripts/verify-release.sh` is the local release gate. It runs:

- formatting check
- Clippy with warnings denied
- complete locked test suite
- locked release build
- package creation for the host target
- package content and checksum verification
- secret-shaped diff scan
- active GitHub Actions absence check
- source no-network contract check

Loopback integration tests may require sandbox permission to bind `127.0.0.1`; they never contact an external host.

## No-Network Contract

Normal binary operation contains no configured remote endpoint and constructs only authenticated loopback HTTP URLs. Release and installer scripts are the only repository components permitted to reference GitHub download URLs. Tests reject non-loopback URL literals in `src/` and continue checking embedded dashboard assets for remote references.

## Documentation

Phase 6 updates:

- README installation, setup, adapter, renderer, privacy, and development sections
- stable changelog for 1.0.0
- MIT license file matching Cargo metadata
- compatibility matrix with automated/manual status
- release checklist and rollback instructions

Manual terminal checks remain explicit because automated fixture tests cannot prove visual behavior in every GPU, compositor, shell, tmux, or terminal-version combination.

## Publishing Boundary

The Phase 6 pull request may be merged after local verification. Creating a Git tag, GitHub Release, or uploading artifacts is a separate publishing action and requires explicit approval after the release candidate commit is on `main`.

## Completion Criteria

Phase 6 is complete when version 1.0.0 packages reproducibly on the host, checksums verify, installers are non-destructive, release contracts pass, docs match the implemented product, no workflows are active, the full local suite passes, and the Phase 6 PR is merged.
