# SaveMyTerminal Phase 5: Native Terminal Renderers Design

**Date:** 2026-06-13
**Status:** Approved through the V1 roadmap
**Target:** Phase 5 of SaveMyTerminal V1

## Summary

Phase 5 turns normalized agent state into terminal-native, ambient presentation for Ghostty, Kitty, WezTerm, and iTerm2 while preserving the portable status fallback. The implementation adapts the idea demonstrated by [ghostty-blackhole](https://github.com/s0xDk/ghostty-blackhole): agent activity should be visible at a glance without capturing prompts, responses, commands, or terminal output.

The terminals have materially different APIs, so SaveMyTerminal does not pretend they are visually identical. A shared renderer view model produces state, label, color, intensity, and active-session count. Terminal integrations consume that model through attached OSC signaling or the authenticated local snapshot command.

## Official Interfaces

- Ghostty configuration reference: <https://ghostty.org/docs/config/reference>
- Kitty configuration reference: <https://sw.kovidgoyal.net/kitty/conf/>
- WezTerm background layers: <https://wezterm.org/config/lua/config/background.html>
- iTerm2 dynamic profiles: <https://iterm2.com/documentation-dynamic-profiles.html>
- iTerm2 Python status components: <https://iterm2.com/python-api/examples/statusbar.html>

## Shared Renderer Contract

Renderers consume only `SessionSnapshot` values. The selected view contains:

- active session count
- agent identifier
- normalized session state
- short status label
- fixed state color
- configured ambient intensity

The most recently updated active session is primary. No working directory, command, prompt, response, model, file, host, or environment data enters the renderer model.

State colors are stable and theme-independent:

- starting: indigo
- thinking: violet
- tool running: amber
- waiting: cyan
- terminal or idle: neutral/reset

`smt snapshot --format text|json` exposes this model to terminal extension scripts. It connects to an existing local service but does not start one merely because a terminal polls while idle. Unavailable service and zero active sessions both render as idle.

## Portable Attached Renderer

`smt run` uses a hybrid renderer when stderr is a terminal:

- existing concise text status remains controlled by `presentation.status_enabled`
- OSC title and cursor-color signaling are controlled by `presentation.ambient_enabled`
- session start sets a sanitized SaveMyTerminal title and state color
- session completion restores the cursor color with OSC 112 and sets an idle title
- write failures never affect the child process

The cursor color is a presentation channel only. It contains a fixed SaveMyTerminal signature and normalized state, never a token count or captured content.

## Terminal Integrations

### Ghostty

Compatibility: Ghostty 1.3 or later on operating systems supported by Ghostty.

Setup installs an original SaveMyTerminal GLSL fragment shader and adds:

```text
custom-shader = <asset path>/savemyterminal.glsl
custom-shader-animation = always
```

The shader reads Ghostty's cursor-color uniforms and renders a restrained corner glow/ring while an attached `smt run` session is active. Unknown cursor colors pass the terminal through unchanged. Native agent hooks without an attached TTY continue to use the status/dashboard surfaces.

### Kitty

Compatibility: Kitty releases supporting `background_image`; remote-control enrichment remains optional.

Setup installs a generated ambient PNG and adds managed `background_image`, `background_image_layout cscaled`, and `background_tint` settings. Attached wrapper sessions also use OSC title signaling. Kitty's documented remote-control facilities can enrich a later release, but V1 does not enable remote control or weaken Kitty authorization by default.

### WezTerm

Compatibility: WezTerm versions supporting Lua `update-right-status` and background layers.

Setup prepends a managed Lua event block that periodically runs `smt snapshot --format text` and updates the right status. The generated ambient image remains available for users who choose to add a background layer, but V1 does not replace or merge an existing `config.background` table because arbitrary Lua is not safely machine-mergeable.

### iTerm2

Compatibility: macOS with iTerm2 Python API support.

Setup installs an AutoLaunch Python script that registers a SaveMyTerminal status bar component and polls `smt snapshot --format text` on a bounded cadence. The user chooses whether to place the component in a profile's status bar, matching iTerm2's official component workflow. Phase 5 does not invent undocumented profile attribute names for background images.

## Assets And Ownership

Generated assets live under SaveMyTerminal's own configuration directory:

```text
assets/savemyterminal-ambient.png
assets/savemyterminal.glsl
assets/savemyterminal-wezterm.lua
assets/savemyterminal-iterm2.py
```

Assets are deterministic, contain no user data, and are atomically regenerated during applied renderer setup. Terminal config edits use the Phase 3 planner, backups, atomic writes, validation, rollback, and manifest ownership records. Uninstall removes managed config blocks and SaveMyTerminal-owned assets only when no installed renderer still references them.

## Setup Selection

Renderer identifiers are `ghostty`, `kitty`, `wezterm`, and `iterm2`.

With no explicit `--integration`, setup plans detected agents and terminals. Explicit selection may preconfigure an unavailable terminal, except `iterm2` is rejected on non-macOS systems. Preview remains the default and `--apply` is required for mutation.

## Failure Isolation

- Snapshot polling never starts or keeps alive an otherwise idle service.
- Missing terminal runtimes, Python modules, or asset files degrade only that renderer.
- OSC writes ignore I/O errors.
- Setup rejects malformed or structurally incompatible config instead of overwriting it.
- Renderer scripts use short subprocess timeouts and render idle on failure.
- No renderer performs outbound network access.

## Testing

Shared renderer contract tests prove every normalized state is accepted, unavailable diagnostics are irrelevant, terminal state is reset at completion, and write failures are isolated.

Descriptor tests cover official config syntax, deterministic assets, preview/apply/uninstall, unrelated-content preservation, WezTerm prepend placement, OS compatibility, and manifest/doctor behavior. Existing agent, wrapper, dashboard, privacy, and setup tests remain green.

All checks run locally. Phase 5 adds no GitHub Actions workflows.

## Completion Criteria

Phase 5 is complete when the four renderer identifiers have truthful compatibility declarations, shared snapshot rendering is privacy-safe, attached wrapper sessions clean up OSC state, terminal assets and config changes are reversible, every renderer has a tested fallback, and the full local suite passes with no active GitHub Actions workflows.
