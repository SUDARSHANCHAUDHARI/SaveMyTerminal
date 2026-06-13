# Compatibility

SaveMyTerminal 1.0.0 uses a universal command wrapper everywhere and optional native
integrations where an agent or terminal exposes a stable local extension point.

## Release Targets

| Target | Package | Status |
| --- | --- | --- |
| `aarch64-apple-darwin` | `.tar.gz` | Host package verified on Apple silicon |
| `x86_64-apple-darwin` | `.tar.gz` | Declared; native package verification required before upload |
| `x86_64-unknown-linux-gnu` | `.tar.gz` | Declared; native package verification required before upload |
| `aarch64-unknown-linux-gnu` | `.tar.gz` | Declared; native package verification required before upload |
| `x86_64-pc-windows-msvc` | `.zip` | Declared; native package verification required before upload |

## Agent Adapters

| Agent | Integration | Automated coverage | Manual release check |
| --- | --- | --- | --- |
| Any CLI agent | `smt run -- <command>` | Yes | Host smoke test |
| Codex | Native hooks | Yes | Setup preview and one lifecycle run |
| Claude Code | Native hooks | Yes | Setup preview and one lifecycle run |
| Gemini CLI | Native hooks | Yes | Setup preview and one lifecycle run |

## Terminal Integrations

| Terminal | Integration | Automated coverage | Manual release check |
| --- | --- | --- | --- |
| Ghostty | Config block, PNG, GLSL shader | Fixture tests | Visual check pending before artifact upload |
| Kitty | Config block and PNG | Fixture tests | Visual check pending before artifact upload |
| WezTerm | Lua config block and PNG | Fixture tests | Visual check pending before artifact upload |
| iTerm2 | Dynamic profile and Python status component | Fixture tests | Visual check pending before artifact upload |

Unsupported or unavailable native integrations fall back to the universal wrapper and
portable text status. GPU, compositor, shell, multiplexer, and terminal-version differences
still require the manual checks recorded above.
