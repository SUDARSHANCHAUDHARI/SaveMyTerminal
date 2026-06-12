# SaveMyTerminal Design

**Date:** 2026-06-13
**Status:** Approved
**Target:** Version 1

## Summary

SaveMyTerminal is a local-only, cross-platform observability and visualization tool for terminal-based AI coding agents. It adapts the core idea of `ghostty-blackhole`: agent activity and context pressure become an ambient part of the terminal instead of living only in logs or an external dashboard.

The default experience combines:

- Agent state: starting, thinking, running a tool, waiting, completed, failed, or interrupted.
- Context-window pressure when the agent exposes it.
- Terminal-native ambient visuals where supported.
- A consistent status surface or shell fallback where ambient effects are unavailable.
- Optional CPU, memory, duration, and command-health diagnostics.
- A loopback-only dashboard for live sessions and privacy-safe 30-day history.

The product supports known agents through native hooks where available and supports unknown agents through a universal wrapper and process observation. It does not promise identical telemetry or visuals for every agent and terminal. Every reported metric carries a quality classification of `exact`, `estimated`, or `unavailable`.

## Goals

1. Work with any terminal-based AI agent through either a native adapter or generic wrapper.
2. Support macOS, Linux, and Windows from version 1.
3. Provide native integrations for Ghostty, Kitty, WezTerm, and iTerm2 where each terminal is available.
4. Communicate agent state and context pressure without obstructing terminal work.
5. Offer system diagnostics as an optional layer rather than permanent visual noise.
6. Keep all processing and storage local, with no account, cloud service, or normal-operation network dependency.
7. Store compact session summaries for 30 days without storing prompts, responses, terminal output, or command contents.
8. Make installation transparent, previewable, reversible, and resilient to partial integration failures.

## Non-Goals

Version 1 will not provide:

- Cloud sync or remote multi-machine monitoring.
- Prompt, response, terminal-output, command-argument, file-content, or environment-value recording.
- IDE extensions, mobile clients, or a native desktop application.
- Identical effects across all terminals.
- Exact context usage for agents that do not expose context information.
- Automatic modification of an integration without showing the proposed change and receiving user approval.

## Product Principles

### Graceful Degradation

Observability must never prevent an agent from starting or continuing. Failure of the local service, an adapter, a renderer, or the dashboard degrades only that capability. The original agent command still runs.

### Honest Telemetry

Every metric includes a provenance quality:

- `exact`: reported by the agent or a documented native integration.
- `estimated`: inferred from process behavior or a documented heuristic.
- `unavailable`: not safely measurable.

Estimated context usage must never be presented as exact. Unknown context usage must not be replaced by a fabricated percentage.

### Privacy By Construction

The event contract does not include fields for prompt text, model responses, terminal output, command arguments, file contents, or environment values. Redaction is a secondary safeguard, not the primary privacy mechanism.

### Ambient First, Details On Demand

The default terminal presentation is compact and glanceable. Resource and command-health diagnostics are optional. Detailed data belongs in the local dashboard or an explicitly expanded terminal surface.

## Architecture

SaveMyTerminal ships as a Rust application with an embedded web dashboard and a modular internal architecture.

### CLI

The executable is named `smt`. Its version 1 command surface is:

- `smt setup`: detect supported agents, terminals, and shells; preview and apply selected integrations.
- `smt run -- <command> [args...]`: run any agent or command through the generic adapter.
- `smt dashboard`: start the local service if necessary and open the dashboard.
- `smt status`: show service, integration, active-session, and storage status.
- `smt doctor`: validate configuration, permissions, connectivity, and adapter availability.
- `smt config`: inspect or change user settings.
- `smt uninstall`: preview and remove only SaveMyTerminal-managed integration entries.

### Local Session Service

The service starts on demand when the first native hook, wrapper session, or dashboard client connects. Concurrent clients reuse the same per-user service. The service:

- Authenticates local clients with a per-install token.
- Binds only to loopback or an equivalent platform-local transport.
- Receives and normalizes adapter events.
- Tracks active session lifecycle and resource measurements.
- Publishes live snapshots to terminal renderers and dashboard clients.
- Finalizes compact summaries at session end.
- Cleans expired summaries.
- Exits after a configurable idle period when no sessions or dashboard clients remain.

The default idle timeout is five minutes. Users can change it in configuration. The service must remain on-demand and stop when idle.

### Agent Adapters

Adapters translate agent-specific hooks or generic process observations into the normalized event protocol.

Version 1 includes:

- Rich adapters for Codex, Claude Code, and Gemini CLI, subject to their documented local integration surfaces at implementation time.
- A generic wrapper adapter for every executable launched through `smt run --`.
- Agent identification based on the invoked executable and adapter handshake, not prompt or output inspection.

Known adapters may report exact context and lifecycle events when exposed by the agent. The generic adapter guarantees process lifecycle, elapsed time, exit status, CPU, and memory where supported by the OS. It reports richer states or context only when they can be inferred reliably and labels them `estimated`.

Adapters are independently disableable. A broken or outdated native adapter falls back to generic wrapper behavior where possible.

### Normalized Event Model

The core protocol is versioned and independent of agents and terminals. An event contains only metadata required for session state:

- Protocol version and event identifier.
- Session identifier.
- Timestamp.
- Adapter and agent identifiers.
- Lifecycle state.
- Optional context usage and limit.
- Optional tool activity category without arguments or content.
- Optional CPU and memory measurements.
- Optional failure category and sanitized adapter error code.
- Quality and source metadata for each measured value.

Lifecycle states are:

- `starting`
- `thinking`
- `tool_running`
- `waiting`
- `completed`
- `failed`
- `interrupted`

Adapters may omit intermediate states they cannot observe. The service owns transition validation and session finalization.

### Terminal Renderers

Renderers consume normalized live snapshots and never depend directly on an agent adapter.

- **Ghostty:** ambient shader integration plus a portable status signal.
- **Kitty:** supported remote-control and tab/title presentation, plus fallback status output.
- **WezTerm:** Lua status and appearance integration across supported platforms.
- **iTerm2:** Python status component and supported session appearance integration on macOS.
- **Other terminals:** portable title/status output with optional shell prompt integration.

The hybrid model is the default. Native ambient visuals supplement the portable status surface; they do not replace it. Users can disable ambient visuals, the status surface, or optional diagnostics independently.

Terminal adapters must declare their OS and version compatibility. Unsupported combinations use the portable fallback.

### Storage

SQLite stores privacy-safe session summaries. Live event details remain in memory and are discarded after finalization.

A summary may contain:

- Random session identifier.
- Agent and adapter identifiers.
- Start time and duration.
- Terminal renderer identifier.
- Final state and sanitized failure category.
- Peak and final context pressure, with quality metadata.
- Counts of observed state transitions and tool events.
- Aggregated CPU and memory measurements.
- Whether the session used a native or generic adapter.

It must not contain prompt text, response text, terminal output, raw command lines, command arguments, working-directory paths, file names, file contents, environment values, hostnames, usernames, repository remotes, or API credentials.

Summaries are retained for 30 days by default. Cleanup runs at service startup and periodically while active. Users can change retention, delete one summary, purge all history, or disable summary persistence for future sessions.

### Dashboard

The dashboard is compiled into the Rust binary and served only by the local service. It uses the same authenticated local API as terminal clients.

The landing page shows:

- Active sessions.
- Agent identity and terminal.
- Current lifecycle state.
- Context pressure and its quality classification.
- Duration and optional CPU/memory diagnostics.

The history view shows privacy-safe 30-day trends, including session counts, durations, completion states, context peaks, and optional resource aggregates. It provides per-session deletion and full-history purge controls.

Closing the dashboard does not stop active sessions. The service exits only after all sessions and clients disconnect and the idle timeout expires.

### Integration Manager

`smt setup` detects the current OS, supported terminals, installed agents, and relevant shells. It presents selectable integrations and a human-readable diff before changing configuration.

For every approved change, the manager:

1. Parses structured configuration with an appropriate parser when one is available.
2. Uses explicit managed markers for text-based snippets.
3. Creates a timestamped backup before the first modification.
4. Writes atomically.
5. Validates the resulting configuration where the target provides a validator.
6. Rolls back the current change if validation fails.
7. Records an integration manifest containing the integration identifier, target configuration path, managed marker identifier, backup path, and post-write file checksum. The manifest does not copy user configuration contents.

`smt uninstall` previews removals and deletes only managed entries. It never replaces a current user configuration wholesale with an old backup.

## Data Flow

1. The user starts an agent normally through an installed native hook or uses `smt run -- <command>`.
2. The integration starts or connects to the on-demand local service.
3. The adapter opens a session and sends normalized metadata events.
4. The service validates state transitions, enriches the session with permitted local process metrics, and publishes a live snapshot.
5. Terminal renderers and connected dashboard clients render the same snapshot.
6. When the process exits or an exact completion event arrives, the service finalizes the session.
7. Transient live details are discarded and a compact privacy-safe summary is stored.
8. Summaries older than the configured retention period are deleted.

## Security And Privacy

- Normal operation performs no outbound network requests.
- The service binds only to loopback or an equivalent local transport.
- A random per-install token authenticates local API and streaming clients.
- Authentication material is stored with restrictive user-only permissions using platform-appropriate storage.
- Browser access uses a short-lived launch token or authenticated local session rather than exposing the long-lived installation token in normal page content.
- Cross-origin requests are rejected unless explicitly required by the embedded dashboard origin.
- Event payload sizes and rates are bounded.
- Database statements are parameterized.
- Logs default to metadata-only levels and apply the same prohibited-data policy as storage.
- `smt doctor` checks token permissions, service binding, database permissions, and unexpected outbound configuration.
- The product contains no analytics, crash-upload, update-check, cloud-sync, or account subsystem in version 1.

Package installation may require the user's normal package-manager network access. This does not change the no-network guarantee for normal SaveMyTerminal operation.

## Failure Handling

- If the service cannot start, the original agent command launches without SaveMyTerminal observability and a concise warning is emitted.
- If an adapter fails, the session falls back to generic process telemetry when possible.
- If one renderer fails, other renderers and the dashboard continue.
- Invalid or oversized events are rejected without terminating valid sessions.
- If the service restarts, previously active but unfinished persisted session headers are finalized as `interrupted`; no prompt or output replay is attempted.
- Database migration failure disables history while preserving live visualization when possible.
- Configuration failures trigger rollback of only the current managed change.
- Diagnostic errors include actionable local remediation without exposing secrets or captured content.

## Cross-Platform Boundary

The Rust core, CLI, generic wrapper, local service, SQLite storage, dashboard, and portable terminal fallback support macOS, Linux, and Windows in version 1.

Native terminal integrations are conditional on terminal availability:

- Ghostty: supported operating systems offered by Ghostty.
- Kitty: supported operating systems offered by Kitty.
- WezTerm: macOS, Linux, and Windows where its integration APIs are available.
- iTerm2: macOS only.

Process metrics use platform-specific implementations behind a shared interface. Missing OS metrics are labeled `unavailable` rather than blocking a session.

## Configuration

Configuration is per user. Version 1 settings include:

- Enabled agent adapters and terminal renderers.
- Ambient effect enablement and intensity.
- Status surface enablement and compactness.
- Optional CPU, memory, duration, and command-health diagnostics.
- Service idle timeout.
- Summary persistence and retention duration.
- Dashboard port selection policy.
- Logging level.

Defaults prioritize low visual noise, local privacy, and graceful fallback.

## Testing Strategy

### Unit Tests

- Event validation and protocol versioning.
- Lifecycle transition rules.
- Exact, estimated, and unavailable quality propagation.
- Summary aggregation and prohibited-field checks.
- Retention and deletion behavior.
- Configuration merging and managed-marker ownership.
- Authentication and authorization decisions.
- Idle shutdown and recovery logic.

### Contract Tests

Every agent adapter must pass a shared suite proving that it:

- Opens and closes sessions correctly.
- Emits only permitted metadata.
- Labels metric quality accurately.
- Handles missing native capabilities.
- Does not prevent the wrapped agent from launching on adapter failure.

Every terminal renderer must pass a shared suite proving that it:

- Accepts all normalized states.
- Handles unavailable context and diagnostics.
- Cleans up terminal-owned state at session end.
- Fails independently.

### Integration Tests

- Generic wrapper process lifecycle and exit-code preservation.
- Concurrent sessions sharing one service.
- On-demand service startup and idle shutdown.
- Authenticated local API and live dashboard streaming.
- SQLite migrations, retention, purge, and interrupted-session recovery.
- Setup preview, atomic application, validation failure, rollback, and uninstall.
- Portable renderer behavior.
- No-network operation under a denied-network test environment.

### Platform And Manual Tests

Continuous integration runs on macOS, Linux, and Windows. Terminal-specific visuals and configuration changes receive manual compatibility checks against supported versions because visual and scripting APIs cannot all be reproduced faithfully in headless CI.

## Version 1 Acceptance Criteria

Version 1 is complete when:

1. `smt run -- <command>` preserves arguments, interactive terminal behavior, signals, and exit status on macOS, Linux, and Windows.
2. Codex, Claude Code, and Gemini CLI each have a documented native adapter where a stable hook exists, with generic fallback otherwise.
3. Unknown agents receive lifecycle and supported process telemetry through the wrapper.
4. Ghostty, Kitty, WezTerm, and iTerm2 have supported native renderers on their applicable operating systems, plus a tested portable fallback.
5. Agent state and context pressure render together, with context quality clearly identified.
6. Optional diagnostics can be enabled or disabled independently.
7. The loopback dashboard shows live sessions and 30-day privacy-safe history.
8. No prohibited content is present in events, logs, or SQLite summaries under automated privacy tests.
9. Setup previews every change, backs it up, validates it where possible, and can remove only managed entries.
10. Any observability failure leaves the underlying agent usable.
11. Normal operation succeeds with outbound network access denied.
12. Automated tests pass on macOS, Linux, and Windows, followed by documented manual terminal checks.

## Recommended Delivery Order

The implementation plan should preserve the cross-platform contract while delivering vertical slices:

1. Core event model, generic wrapper, service lifecycle, and portable renderer.
2. SQLite summaries and authenticated dashboard.
3. Setup, managed configuration, doctor, and uninstall workflows.
4. Known-agent adapters.
5. Native terminal renderers and ambient visual assets.
6. Cross-platform packaging, compatibility validation, and release hardening.

This ordering keeps a usable universal path available before specialized integrations are added.
