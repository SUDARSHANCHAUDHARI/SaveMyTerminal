# SaveMyTerminal Phase 6: Release Hardening Implementation Plan

**Goal:** Produce a reproducible, documented SaveMyTerminal 1.0.0 release candidate without GitHub Actions.

**Architecture:** Shell and PowerShell scripts package locked release binaries into platform archives with SHA-256 files. Contract tests verify metadata, artifact contents, no-network source behavior, and repository policy. Documentation records compatibility and manual release steps.

**Tech Stack:** Cargo/Rust 1.95, POSIX shell, PowerShell, tar/zip, SHA-256 tooling, local Git/GitHub release process.

---

### Task 1: Release contracts

**Files:** `tests/release_contract.rs`, `Cargo.toml`, `.gitignore`

1. Add failing tests for version 1.0.0, required docs/scripts, package manifest, no remote source URLs, and workflow absence.
2. Bump Cargo metadata and add release-output ignores.
3. Run focused contracts.

### Task 2: Packaging and installers

**Files:** `scripts/package.sh`, `scripts/package.ps1`, `scripts/install.sh`, `scripts/install.ps1`, `scripts/verify-release.sh`

1. Implement locked host/target packaging with deterministic contents.
2. Generate and verify SHA-256 sidecars.
3. Implement local/archive installers that do not alter shell or terminal settings.
4. Add a local release verification entry point.

### Task 3: Release documentation

**Files:** `README.md`, `CHANGELOG.md`, `LICENSE`, `docs/compatibility.md`, `docs/release-checklist.md`

1. Replace phase-progress language with the completed V1 feature surface.
2. Document installation, setup selection, native adapters/renderers, fallback behavior, and uninstall.
3. Record automated and manual compatibility status plus publishing/rollback steps.

### Task 4: Package and ship check

1. Run focused release tests, formatting, Clippy, full tests, release build, package script, checksum verification, and archive inspection.
2. Run secret, no-network, workflow, diff, and TODO/FIXME checks.
3. Run the focused quality and ship reviews.
4. Commit, push, create, and merge the Phase 6 PR.
5. Leave tag and GitHub Release creation pending explicit publishing approval.
