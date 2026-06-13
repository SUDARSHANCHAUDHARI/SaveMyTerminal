# SaveMyTerminal Phase 4: Known-Agent Adapters Implementation Plan

**Goal:** Add privacy-preserving native hook adapters for Codex, Claude Code, and Gemini CLI, including reversible setup and uninstall.

**Architecture:** A shared adapter module parses minimal agent-specific hook envelopes into the existing normalized event protocol. A structured JSON integration descriptor extends the Phase 3 planner/apply engine and installs exact user-level hook handlers. Native hooks are best-effort observers and always return neutral success output.

**Tech Stack:** Rust 2024, Tokio, Serde JSON, UUID v5, Clap, existing Axum service and integration engine.

---

### Task 1: Adapter contracts

**Files:** `src/adapter.rs`, `src/lib.rs`, `tests/adapter_contract.rs`, `Cargo.toml`

1. Add failing tests for event mapping, deterministic session IDs, tool categorization, ignored sensitive fields, malformed input, and size limits.
2. Implement minimal hook envelopes and adapter mapping.
3. Run the focused adapter tests.

### Task 2: Native hook CLI

**Files:** `src/cli.rs`, `src/app.rs`, `src/client.rs`, `src/service/registry.rs`, `tests/hook_command.rs`

1. Add failing command tests for neutral JSON output and zero exit status.
2. Add `smt hook codex|claude|gemini` with bounded stdin.
3. Add idempotent native session initialization and registry tests.
4. Verify service failure and invalid payloads never fail the host hook.

### Task 3: Structured integration transforms

**Files:** `src/integration/mod.rs`, `src/integration/apply.rs`, `src/integration/json.rs`, `tests/integration_manager.rs`

1. Add failing tests for JSON merge, idempotency, exact removal, unrelated-content preservation, stale plans, validation rollback, and manifest ownership.
2. Generalize descriptors and apply functions around proposed bytes plus validation.
3. Implement structured JSON transforms without ad hoc string editing.
4. Run integration-manager tests.

### Task 4: Agent hook descriptors and setup workflow

**Files:** `src/agents.rs`, `src/app.rs`, `tests/setup_commands.rs`

1. Add failing descriptor fixtures for Codex, Claude Code, and Gemini CLI official schemas.
2. Implement user-level descriptor registration and event handler sets.
3. Connect detection, explicit selection, preview, apply, manifest, and uninstall.
4. Verify settings and unrelated hooks survive round trips.

### Task 5: Privacy, quality, and release checks

**Files:** `tests/privacy_contract.rs`, documentation as needed

1. Add hostile hook payload privacy tests.
2. Run formatting, focused tests, full tests, Clippy, diff checks, secret scan, and workflow absence checks.
3. Commit and push the feature branch.
4. Create and merge the Phase 4 pull request without running or adding GitHub Actions.
5. Synchronize `main` and begin Phase 5 automatically.
