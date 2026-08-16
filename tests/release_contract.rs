use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).expect("release contract file should be readable")
}

#[test]
fn crate_is_the_v1_release_candidate() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "1.0.1");
}

#[test]
fn release_files_are_present() {
    let root = repository_root();
    for relative in [
        "CHANGELOG.md",
        "LICENSE",
        "docs/compatibility.md",
        "docs/release-checklist.md",
        "docs/verification-matrix.md",
        "scripts/install.ps1",
        "scripts/install.sh",
        "scripts/package.ps1",
        "scripts/package.sh",
        "scripts/verify-release.sh",
    ] {
        assert!(root.join(relative).is_file(), "missing {relative}");
    }
}

#[test]
fn package_scripts_build_locked_archives_with_checksums() {
    let root = repository_root();
    let unix = read(root.join("scripts/package.sh"));
    let windows = read(root.join("scripts/package.ps1"));

    for required in [
        "cargo build",
        "--locked",
        "README.md",
        "LICENSE",
        "CHANGELOG.md",
    ] {
        assert!(
            unix.contains(required),
            "package.sh must include {required}"
        );
        assert!(
            windows.contains(required),
            "package.ps1 must include {required}"
        );
    }

    assert!(
        unix.contains("sha256"),
        "package.sh must create SHA-256 output"
    );
    assert!(
        windows.contains("SHA256"),
        "package.ps1 must create SHA-256 output"
    );
    assert!(unix.contains("tar"), "package.sh must create a tar archive");
    assert!(
        windows.contains("Compress-Archive"),
        "package.ps1 must create a zip archive"
    );
}

#[test]
fn installers_do_not_require_rust_or_modify_startup_configuration() {
    let root = repository_root();
    for relative in ["scripts/install.sh", "scripts/install.ps1"] {
        let installer = read(root.join(relative));
        assert!(
            !installer.contains("rustc"),
            "{relative} must work without Rust"
        );
        for forbidden in [".bashrc", ".zshrc", "settings.json", "hooks.json"] {
            assert!(
                !installer.contains(forbidden),
                "{relative} must not modify {forbidden}"
            );
        }
    }
}

#[test]
fn unix_installer_refreshes_macos_code_signature_after_upgrade() {
    let installer = read(repository_root().join("scripts/install.sh"));
    assert!(installer.contains("uname -s"));
    assert!(installer.contains("codesign --force --sign -"));
}

#[test]
fn release_verification_checks_checksums_and_secret_shapes() {
    let script = read(repository_root().join("scripts/verify-release.sh"));
    assert!(
        script.contains("shasum -a 256 -c") && script.contains("sha256sum -c"),
        "release verification must independently check package checksums"
    );
    for required in [
        "git diff HEAD",
        "private_key_pattern",
        "github_token_pattern",
    ] {
        assert!(
            script.contains(required),
            "release verification must include secret scan marker {required}"
        );
    }
}

#[test]
fn repository_has_no_active_github_actions() {
    let workflows = repository_root().join(".github/workflows");
    if workflows.exists() {
        let entries = fs::read_dir(workflows)
            .expect("workflow directory should be readable")
            .collect::<Result<Vec<_>, _>>()
            .expect("workflow entries should be readable");
        assert!(
            entries.is_empty(),
            "GitHub Actions workflows are not allowed"
        );
    }
}

#[test]
fn runtime_source_contains_no_remote_urls() {
    let source = repository_root().join("src");
    let mut pending = vec![source];

    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).expect("source directory should be readable") {
            let entry = entry.expect("source entry should be readable");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                let contents = read(&path);
                assert!(
                    !contents.contains("https://"),
                    "remote URL found in {}",
                    path.display()
                );
                for line in contents.lines().filter(|line| line.contains("http://")) {
                    assert!(
                        line.contains("http://{address}"),
                        "non-loopback URL found in {}: {line}",
                        path.display()
                    );
                }
            }
        }
    }
}

#[test]
fn readme_documents_the_completed_product() {
    let readme = read(repository_root().join("README.md"));
    for section in [
        "## Installation",
        "## Run An Agent",
        "## Native Agent Hooks",
        "## Terminal Integrations",
        "## Snapshot",
        "## Uninstall",
    ] {
        assert!(readme.contains(section), "README is missing {section}");
    }
    assert!(!readme.contains("planned for later phases"));
    for required in [
        "savemyterminal run -- codex",
        "state-reactive black-hole",
        "Context pressure remains unavailable",
        "config.ghostty",
    ] {
        assert!(
            readme.contains(required),
            "README must document the verified renderer contract: {required}"
        );
    }
}
