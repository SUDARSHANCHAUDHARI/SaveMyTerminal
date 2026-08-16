use crate::{
    detection::OsId,
    integration::TextDescriptor,
    paths::AppPaths,
    terminal_assets::{ambient_path, shader_path},
};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Compatibility {
    pub id: &'static str,
    pub requirement: &'static str,
    pub macos: bool,
    pub linux: bool,
    pub windows: bool,
    pub capability: &'static str,
}

pub const COMPATIBILITY: &[Compatibility] = &[
    Compatibility {
        id: "ghostty",
        requirement: "1.3 cursor shader uniforms",
        macos: true,
        linux: true,
        windows: false,
        capability: "shader+osc",
    },
    Compatibility {
        id: "kitty",
        requirement: "background_image option",
        macos: true,
        linux: true,
        windows: false,
        capability: "image+osc",
    },
    Compatibility {
        id: "wezterm",
        requirement: "Lua update-right-status event",
        macos: true,
        linux: true,
        windows: true,
        capability: "lua-status",
    },
    Compatibility {
        id: "iterm2",
        requirement: "Python API status components",
        macos: true,
        linux: false,
        windows: false,
        capability: "python-status",
    },
];

pub fn descriptors(home: &Path, paths: &AppPaths, os: OsId) -> Vec<TextDescriptor> {
    let ghostty_target = if os == OsId::Macos {
        home.join("Library/Application Support/com.mitchellh.ghostty/config.ghostty")
    } else {
        home.join(".config/ghostty/config")
    };
    let ghostty = TextDescriptor::new(
        "ghostty",
        1,
        ghostty_target,
        "#",
        format!(
            "custom-shader = {}\ncustom-shader-animation = always",
            shader_path(paths).display()
        ),
        None,
    )
    .expect("built-in Ghostty descriptor is valid");
    let kitty = TextDescriptor::new(
        "kitty",
        1,
        home.join(".config/kitty/kitty.conf"),
        "#",
        format!(
            "background_image {}\nbackground_image_layout cscaled\nbackground_tint 0.82",
            ambient_path(paths).display()
        ),
        None,
    )
    .expect("built-in Kitty descriptor is valid");
    let wezterm = TextDescriptor::new_prepend(
        "wezterm",
        1,
        home.join(".wezterm.lua"),
        "--",
        r#"local savemyterminal_wezterm = require 'wezterm'
savemyterminal_wezterm.on('update-right-status', function(window, pane)
  local ok, stdout = savemyterminal_wezterm.run_child_process({ 'savemyterminal', 'snapshot', '--format', 'text' })
  window:set_right_status(ok and stdout:gsub('%s+$', '') or 'savemyterminal idle')
end)"#,
        None,
    )
    .expect("built-in WezTerm descriptor is valid");
    vec![
        ghostty,
        kitty,
        wezterm,
        TextDescriptor::new(
            "iterm2",
            1,
            home.join("Library/Application Support/iTerm2/Scripts/AutoLaunch/savemyterminal.py"),
            "#",
            ITERM2_SCRIPT,
            None,
        )
        .expect("built-in iTerm2 descriptor is valid"),
    ]
}

const ITERM2_SCRIPT: &str = r#"import asyncio
import iterm2

async def main(connection):
    component = iterm2.StatusBarComponent(
        short_description="SaveMyTerminal",
        detailed_description="Privacy-safe local AI agent state",
        knobs=[],
        exemplar="savemyterminal codex thinking",
        update_cadence=1,
        identifier="com.sudarshantechlabs.savemyterminal.status")

    @iterm2.StatusBarRPC
    async def status(knobs):
        try:
            process = await asyncio.create_subprocess_exec(
                "savemyterminal", "snapshot", "--format", "text",
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.DEVNULL)
            stdout, _ = await asyncio.wait_for(process.communicate(), timeout=1.0)
            return stdout.decode("utf-8").strip() or "savemyterminal idle"
        except Exception:
            return "savemyterminal idle"

    await component.async_register(connection, status)

iterm2.run_forever(main)
"#;
