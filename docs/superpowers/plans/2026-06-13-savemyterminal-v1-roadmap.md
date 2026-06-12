# SaveMyTerminal V1 Implementation Roadmap

The approved v1 design spans several independently testable subsystems. Each phase gets its own implementation plan and must leave the repository in a working state.

1. **Universal core**: Rust workspace, normalized event model, lifecycle validation, authenticated loopback service, generic wrapper, process metrics, and portable status rendering.
2. **Persistence and dashboard**: SQLite summaries, 30-day retention, authenticated live streaming, embedded dashboard, deletion, and purge.
3. **Setup and configuration**: detection, previewable config edits, backups, validation, rollback, manifests, doctor, and uninstall.
4. **Known-agent adapters**: Codex, Claude Code, and Gemini CLI native hooks with shared adapter contract tests and generic fallback.
5. **Native terminal renderers**: Ghostty, Kitty, WezTerm, and iTerm2 integrations plus ambient assets and compatibility declarations.
6. **Release hardening**: cross-platform packaging, installers, CI matrices, no-network verification, manual compatibility runs, and release documentation.

Phase 1 is detailed in `docs/superpowers/plans/2026-06-13-universal-core.md`. Later plans should be written only after the preceding phase is implemented and reviewed, so they can use the real APIs rather than speculative interfaces.
