# SaveMyTerminal Phase 5: Native Terminal Renderers Implementation Plan

**Goal:** Add truthful, reversible native presentation for Ghostty, Kitty, WezTerm, and iTerm2 on top of one privacy-safe renderer model.

**Architecture:** A shared snapshot formatter selects the newest active session and emits text/JSON view models. The generic wrapper gains best-effort OSC presentation. Setup installs deterministic owned assets and terminal-specific managed descriptors, while unsupported capabilities retain the portable fallback.

**Tech Stack:** Rust 2024, existing local HTTP service, Serde JSON, terminal OSC, Ghostty GLSL, Kitty config, WezTerm Lua, iTerm2 Python API.

---

### Task 1: Shared snapshot and renderer contracts

**Files:** `src/renderer/mod.rs`, `src/renderer/snapshot.rs`, `src/client.rs`, `src/cli.rs`, `src/app.rs`, `tests/renderer_contract.rs`

1. Add failing tests for every session state, deterministic selection, privacy-safe JSON, idle fallback, and intensity clamping.
2. Implement the shared renderer view and `smt snapshot --format text|json`.
3. Verify snapshot polling does not start a missing service.

### Task 2: Attached OSC renderer

**Files:** `src/renderer/osc.rs`, `src/renderer/hybrid.rs`, `src/app.rs`, `tests/run_command.rs`

1. Add failing tests for start, finish, reset, disabled ambient mode, and failed writers.
2. Implement hybrid text plus OSC output for recognized terminals and TTY output.
3. Preserve child arguments, signals, and exit status.

### Task 3: Deterministic owned assets

**Files:** `src/terminal_assets.rs`, `tests/terminal_assets.rs`

1. Add failing golden/contract tests for PNG signature, GLSL privacy and uniforms, Lua module, and iTerm2 script.
2. Generate the original ambient image and static scripts deterministically.
3. Add atomic owned-asset writes and exact cleanup.

### Task 4: Terminal descriptors

**Files:** `src/terminals.rs`, integration modules, `src/app.rs`, `tests/terminal_integrations.rs`, `tests/setup_commands.rs`

1. Add failing compatibility and config fixture tests for all four terminals.
2. Implement Ghostty and Kitty managed text descriptors.
3. Implement prepended WezTerm event configuration and the iTerm2 AutoLaunch owned script.
4. Connect detection, explicit selection, preview, apply, doctor, and uninstall.

### Task 5: Verification and publish

1. Run focused tests, formatting, Clippy, release build, full tests, privacy scan, secret scan, and workflow absence check.
2. Run the focused quality review and document residual manual compatibility risk.
3. Commit, push, create, and merge the Phase 5 PR without GitHub Actions.
4. Synchronize `main` and begin Phase 6 automatically.
