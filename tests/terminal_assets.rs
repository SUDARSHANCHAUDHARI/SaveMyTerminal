use savemyterminal::{
    detection::OsId,
    paths::AppPaths,
    terminal_assets::{GHOSTTY_SHADER, ambient_png},
    terminals::{COMPATIBILITY, descriptors},
};

#[test]
fn generated_ambient_asset_is_a_deterministic_rgba_png() {
    let first = ambient_png().unwrap();
    let second = ambient_png().unwrap();
    assert_eq!(first, second);
    assert_eq!(&first[..8], b"\x89PNG\r\n\x1a\n");
    assert!(first.len() > 1_000);
}

#[test]
fn shader_uses_only_terminal_uniforms_and_contains_no_captured_content_fields() {
    for required in ["iChannel0", "iCurrentCursorColor", "iResolution", "iTime"] {
        assert!(GHOSTTY_SHADER.contains(required));
    }
    for prohibited in ["prompt", "response", "command", "transcript", "cwd"] {
        assert!(!GHOSTTY_SHADER.contains(prohibited));
    }
}

#[test]
fn terminal_descriptors_target_documented_user_locations() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths {
        config_dir: temp.path().join("smt"),
        runtime_dir: temp.path().join("run"),
        data_dir: temp.path().join("data"),
    };
    let descriptors = descriptors(temp.path(), &paths, OsId::Macos);
    assert!(
        descriptors
            .iter()
            .any(|item| item.id == "ghostty"
                && item.target.ends_with("com.mitchellh.ghostty/config"))
    );
    assert!(
        descriptors
            .iter()
            .any(|item| item.id == "kitty" && item.target.ends_with("kitty/kitty.conf"))
    );
    assert!(
        descriptors
            .iter()
            .any(|item| item.id == "wezterm" && item.target.ends_with(".wezterm.lua"))
    );
    assert!(
        descriptors
            .iter()
            .any(|item| item.id == "iterm2" && item.target.ends_with("savemyterminal.py"))
    );
    assert_eq!(COMPATIBILITY.len(), 4);
}

#[test]
fn wezterm_managed_block_is_prepended_before_existing_return() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths {
        config_dir: temp.path().join("smt"),
        runtime_dir: temp.path().join("run"),
        data_dir: temp.path().join("data"),
    };
    let descriptor = descriptors(temp.path(), &paths, OsId::Linux)
        .into_iter()
        .find(|item| item.id == "wezterm")
        .unwrap();
    std::fs::write(&descriptor.target, "local config = {}\nreturn config\n").unwrap();
    let plan = savemyterminal::integration::plan_install(&descriptor).unwrap();
    assert!(
        plan.preview
            .starts_with("-- >>> SaveMyTerminal:wezterm >>>")
    );
    assert!(plan.preview.ends_with("return config\n"));
}
