# SaveMyTerminal Phase 4: Known-Agent Adapters Design

**Date:** 2026-06-13
**Status:** Approved through the V1 roadmap
**Target:** Phase 4 of SaveMyTerminal V1

## Summary

Phase 4 adds native lifecycle adapters for Codex, Claude Code, and Gemini CLI. Each adapter accepts the agent's official hook JSON on standard input, extracts only privacy-safe metadata, maps it to the existing event protocol, and reports it to the local service without affecting the agent when observation fails.

The generic `smt run` wrapper remains the universal fallback and the only source of exact process exit codes. Native hooks improve turn and tool-state fidelity while preserving the existing local-only, metadata-only boundary.

## Official Interfaces

- Codex hooks: <https://developers.openai.com/codex/hooks>
- Codex configuration: <https://developers.openai.com/codex/config-reference>
- Claude Code hooks: <https://code.claude.com/docs/en/hooks>
- Gemini CLI hooks overview: <https://github.com/google-gemini/gemini-cli/blob/main/docs/hooks/index.md>
- Gemini CLI hooks reference: <https://github.com/google-gemini/gemini-cli/blob/main/docs/hooks/reference.md>

The implementation uses user-level hook files:

- Codex: `~/.codex/hooks.json`
- Claude Code: `~/.claude/settings.json`
- Gemini CLI: `~/.gemini/settings.json`

Each file is parsed and serialized as structured JSON. Setup merges exact SaveMyTerminal hook handlers into existing event arrays. Uninstall removes only handlers whose command and managed name match SaveMyTerminal's descriptor.

## CLI Contract

`smt hook <agent>` is the shared hook entry point, where `<agent>` is `codex`, `claude`, or `gemini`.

The command:

1. Reads at most 64 KiB from standard input.
2. Deserializes only `session_id`, `hook_event_name`, `tool_name`, `source`, and `reason`.
3. Never reads transcript files or stores prompt, response, tool input, tool output, current directory, model, or environment content.
4. Derives a stable UUID v5 from the adapter identifier and native session identifier.
5. Ensures the local service and submits the mapped event best effort.
6. Writes one empty JSON object to standard output and exits zero for valid or invalid input, service failure, timeout, and unsupported events.

The always-success behavior is deliberate: SaveMyTerminal is an observer and must never block an agent turn or tool call.

## Event Mapping

| SaveMyTerminal | Codex | Claude Code | Gemini CLI |
|---|---|---|---|
| `Started` | `SessionStart` | `SessionStart` | `SessionStart` |
| `Thinking` | `UserPromptSubmit`, `PostToolUse` | `UserPromptSubmit`, `PostToolUse` | `BeforeAgent`, `AfterTool` |
| `ToolRunning` | `PreToolUse` | `PreToolUse` | `BeforeTool` |
| `Waiting` | `Stop` | `Stop` | `AfterAgent` |
| `Interrupted` | unavailable | `SessionEnd` | `SessionEnd` |

Codex currently documents no stable session-end hook. Its `Stop` event is turn-scoped, so the native adapter never claims completion. Claude and Gemini session-end reasons also do not provide a process exit code; they map to `Interrupted` rather than fabricating a successful `Completed { exit_code: 0 }` event. Exact completion remains available through the generic wrapper.

Tool category mapping uses only `tool_name`:

- shell and command tools -> `Shell`
- read tools -> `FileRead`
- write, edit, and patch tools -> `FileWrite`
- search, glob, and grep tools -> `Search`
- web, fetch, and network tools -> `Network`
- all other names -> `Other`

## Session Semantics

Native hook processes may begin mid-session after setup or service restart. A non-start event is sent normally first. If delivery fails because no matching local session exists, the hook path sends `Started` for the deterministic session UUID and retries the lifecycle event. Explicit `SessionStart` delivery remains best effort and duplicate starts are harmless observer failures.

## Integration Descriptors

Phase 4 generalizes the Phase 3 integration engine from managed text blocks to structured transforms. A descriptor owns:

- stable identifier and version
- target path
- install transform
- uninstall transform
- syntax validator

The planner still computes precondition and result hashes, bounded previews, and create/update/no-change actions. The apply engine still performs backups, atomic writes, validation, rollback, and manifest updates.

SaveMyTerminal handlers are marked with the name `savemyterminal` where the host schema supports names, and always use the exact command `smt hook <agent>`. Existing hook groups and unrelated settings remain intact. Re-running setup is idempotent.

## Setup And Uninstall

`smt setup` registers descriptors for detected agents. Explicit `--integration` selection can install an adapter even when its executable is not detected, which supports alternate PATH layouts and preconfiguration. With no selection, only detected agent adapters are planned.

`smt uninstall` reads the ownership manifest, plans selected or all recorded known-agent descriptors, and removes only exact SaveMyTerminal handlers. Empty event arrays and hook objects introduced solely by SaveMyTerminal are pruned; unrelated user configuration is preserved.

Preview remains the default. Mutation requires `--apply`.

## Privacy And Security

- Hook input size is bounded before JSON parsing.
- Minimal structs ignore prohibited fields without cloning or logging them.
- Parse errors never include the input body.
- Native session IDs are transformed into UUID v5 values and never persisted directly.
- Hook commands return neutral output and zero status, preventing accidental policy decisions.
- Configuration changes use structured parsers, exact ownership matching, backups, atomic writes, and rollback.
- Normal operation performs no outbound network request.

## Testing

Tests cover:

- official lifecycle mapping for all three agents
- stable and isolated session UUID derivation
- tool-name-only categorization
- prohibited fields absent from events, errors, and persisted state
- bounded input and neutral success output
- idempotent duplicate native starts
- structured JSON merge and exact uninstall preservation
- setup detection, explicit selection, preview, apply, manifest, and uninstall workflows
- existing generic-wrapper behavior and all prior tests

All verification runs locally. Phase 4 adds no GitHub Actions workflows.

## Completion Criteria

Phase 4 is complete when all three agents can report native lifecycle metadata through `smt hook`, setup and uninstall modify only owned structured entries, hook failures cannot block agents, privacy tests cover hostile payloads, the generic wrapper remains functional, and the complete local test suite passes with no active GitHub Actions workflows.
