use savemyterminal::{
    integration::{
        PlanAction, TextDescriptor, Validator, apply_json_plan, apply_json_uninstall, apply_plan,
        apply_uninstall,
        json::{plan_install as plan_json_install, plan_uninstall as plan_json_uninstall},
        managed::{BlockState, Marker, insert_or_replace, inspect, remove},
        plan_install, plan_uninstall,
    },
    manifest::{IntegrationManifest, IntegrationRecord, load_manifest, save_manifest_atomic},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

fn marker() -> Marker {
    Marker::new("example", "#").unwrap()
}

#[test]
fn agent_json_plans_preserve_unrelated_settings_and_remove_only_owned_hooks() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let descriptor = savemyterminal::agents::descriptors(&home)
        .into_iter()
        .find(|descriptor| descriptor.id == "claude")
        .unwrap();
    std::fs::create_dir_all(descriptor.target.parent().unwrap()).unwrap();
    std::fs::write(
        &descriptor.target,
        r#"{"theme":"dark","hooks":{"Stop":[{"matcher":"*","hooks":[{"type":"command","command":"user-hook"}]}]}}"#,
    )
    .unwrap();
    let manifest = temp.path().join("manifest.json");
    let backups = temp.path().join("backups");

    let install = plan_json_install(&descriptor).unwrap();
    apply_json_plan(&install, &descriptor, &manifest, &backups).unwrap();
    let installed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&descriptor.target).unwrap()).unwrap();
    assert_eq!(installed["theme"], "dark");
    let encoded = installed.to_string();
    assert!(encoded.contains("user-hook"));
    assert!(encoded.contains("smt hook claude"));

    let repeated = plan_json_install(&descriptor).unwrap();
    assert_eq!(repeated.action, PlanAction::NoChange);

    let uninstall = plan_json_uninstall(&descriptor).unwrap();
    apply_json_uninstall(&uninstall, &descriptor, &manifest, &backups).unwrap();
    let removed = std::fs::read_to_string(&descriptor.target).unwrap();
    assert!(removed.contains("user-hook"));
    assert!(removed.contains("\"theme\": \"dark\""));
    assert!(!removed.contains("smt hook claude"));
}

#[test]
fn every_agent_descriptor_uses_the_documented_user_file_and_events() {
    let temp = tempfile::tempdir().unwrap();
    for descriptor in savemyterminal::agents::descriptors(temp.path()) {
        let plan = plan_json_install(&descriptor).unwrap();
        let value: serde_json::Value = serde_json::from_str(&plan.preview).unwrap();
        let hooks = value["hooks"].as_object().unwrap();
        match descriptor.id.as_str() {
            "codex" => {
                assert!(descriptor.target.ends_with(".codex/hooks.json"));
                assert!(hooks.contains_key("UserPromptSubmit"));
                assert!(hooks.contains_key("Stop"));
            }
            "claude" => {
                assert!(descriptor.target.ends_with(".claude/settings.json"));
                assert!(hooks.contains_key("SessionEnd"));
            }
            "gemini" => {
                assert!(descriptor.target.ends_with(".gemini/settings.json"));
                assert!(hooks.contains_key("BeforeAgent"));
                assert!(hooks.contains_key("AfterAgent"));
            }
            other => panic!("unexpected descriptor {other}"),
        }
    }
}

#[test]
fn managed_block_insertion_and_replacement_preserve_unrelated_bytes() {
    let original = "user-before\nuser-after\n";
    let inserted = insert_or_replace(original, &marker(), "managed line").unwrap();
    assert_eq!(
        inserted,
        "user-before\nuser-after\n# >>> SaveMyTerminal:example >>>\nmanaged line\n# <<< SaveMyTerminal:example <<<\n"
    );
    assert_eq!(inspect(&inserted, &marker()).unwrap(), BlockState::Present);

    let replaced = insert_or_replace(&inserted, &marker(), "new managed line\n").unwrap();
    assert_eq!(
        replaced,
        "user-before\nuser-after\n# >>> SaveMyTerminal:example >>>\nnew managed line\n# <<< SaveMyTerminal:example <<<\n"
    );
}

#[test]
fn managed_block_removal_restores_surrounding_content() {
    let original = "before\n# >>> SaveMyTerminal:example >>>\nowned\n# <<< SaveMyTerminal:example <<<\nafter\n";

    assert_eq!(remove(original, &marker()).unwrap(), "before\nafter\n");
    assert_eq!(
        inspect("before\nafter\n", &marker()).unwrap(),
        BlockState::Missing
    );
}

#[test]
fn malformed_duplicate_or_nested_markers_are_conflicts() {
    let duplicate = "# >>> SaveMyTerminal:example >>>\none\n# <<< SaveMyTerminal:example <<<\n# >>> SaveMyTerminal:example >>>\ntwo\n# <<< SaveMyTerminal:example <<<\n";
    let missing_end = "# >>> SaveMyTerminal:example >>>\none\n";
    let nested = "# >>> SaveMyTerminal:example >>>\n# >>> SaveMyTerminal:example >>>\n# <<< SaveMyTerminal:example <<<\n# <<< SaveMyTerminal:example <<<\n";

    assert!(inspect(duplicate, &marker()).is_err());
    assert!(inspect(missing_end, &marker()).is_err());
    assert!(inspect(nested, &marker()).is_err());
}

#[test]
fn manifest_round_trip_is_sorted_and_contains_no_copied_user_content() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("integrations.json");
    let manifest = IntegrationManifest {
        version: 1,
        integrations: vec![
            IntegrationRecord {
                id: "zeta".to_owned(),
                descriptor_version: 1,
                target_path: PathBuf::from("/tmp/zeta.conf"),
                marker_id: "zeta".to_owned(),
                backup_path: None,
                post_write_sha256: "aa".repeat(32),
                applied_at_unix_ms: 2,
            },
            IntegrationRecord {
                id: "alpha".to_owned(),
                descriptor_version: 1,
                target_path: PathBuf::from("/tmp/alpha.conf"),
                marker_id: "alpha".to_owned(),
                backup_path: Some(PathBuf::from("/tmp/backup")),
                post_write_sha256: "bb".repeat(32),
                applied_at_unix_ms: 1,
            },
        ],
    };

    save_manifest_atomic(&path, &manifest).unwrap();
    let loaded = load_manifest(&path).unwrap();
    assert_eq!(loaded.integrations[0].id, "alpha");
    assert_eq!(loaded.integrations[1].id, "zeta");

    let encoded = std::fs::read_to_string(path).unwrap();
    assert!(!encoded.contains("managed line"));
    assert!(!encoded.contains("user-before"));
    assert!(!encoded.contains("file_contents"));
}

#[test]
fn manifest_rejects_unknown_fields_duplicate_ids_and_newer_versions() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("integrations.json");
    std::fs::write(&path, r#"{"version":2,"integrations":[]}"#).unwrap();
    assert!(load_manifest(&path).is_err());

    std::fs::write(&path, r#"{"version":1,"integrations":[],"unknown":true}"#).unwrap();
    assert!(load_manifest(&path).is_err());

    let record = IntegrationRecord {
        id: "duplicate".to_owned(),
        descriptor_version: 1,
        target_path: PathBuf::from("/tmp/one.conf"),
        marker_id: "one".to_owned(),
        backup_path: None,
        post_write_sha256: "cc".repeat(32),
        applied_at_unix_ms: 1,
    };
    let duplicate = IntegrationManifest {
        version: 1,
        integrations: vec![
            record.clone(),
            IntegrationRecord {
                target_path: PathBuf::from("/tmp/two.conf"),
                marker_id: "two".to_owned(),
                ..record
            },
        ],
    };
    assert!(save_manifest_atomic(&path, &duplicate).is_err());

    let unsafe_identifier = IntegrationManifest {
        version: 1,
        integrations: vec![IntegrationRecord {
            id: "unsafe\nidentifier".to_owned(),
            descriptor_version: 1,
            target_path: PathBuf::from("/tmp/tool.conf"),
            marker_id: "unsafe\nidentifier".to_owned(),
            backup_path: None,
            post_write_sha256: "dd".repeat(32),
            applied_at_unix_ms: 1,
        }],
    };
    assert!(save_manifest_atomic(&path, &unsafe_identifier).is_err());
}

struct ContentValidator {
    required: &'static str,
}

impl Validator for ContentValidator {
    fn validate(&self, target: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(target).map_err(|error| error.to_string())?;
        content
            .contains(self.required)
            .then_some(())
            .ok_or_else(|| format!("missing {}", self.required))
    }
}

fn descriptor(
    target: PathBuf,
    body: &str,
    validator: Option<Arc<dyn Validator>>,
) -> TextDescriptor {
    TextDescriptor::new("example", 1, target, "#", body, validator).unwrap()
}

#[test]
fn planning_is_read_only_and_bounds_the_preview() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("tool.conf");
    let original = format!("{}\n", "user-line".repeat(400));
    std::fs::write(&target, &original).unwrap();

    let plan = plan_install(&descriptor(target.clone(), "managed line", None)).unwrap();

    assert_eq!(plan.action, PlanAction::Update);
    assert_eq!(std::fs::read_to_string(target).unwrap(), original);
    assert!(plan.preview.len() <= 4096);
    assert!(plan.preview.contains("SaveMyTerminal:example"));
}

#[test]
fn successful_apply_creates_backup_and_manifest_record() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("tool.conf");
    let manifest_path = temp.path().join("integrations.json");
    let backup_dir = temp.path().join("backups");
    std::fs::write(&target, "user-content\n").unwrap();
    let descriptor = descriptor(
        target.clone(),
        "managed line",
        Some(Arc::new(ContentValidator {
            required: "managed line",
        })),
    );
    let plan = plan_install(&descriptor).unwrap();

    let record = apply_plan(&plan, &descriptor, &manifest_path, &backup_dir).unwrap();

    assert!(
        std::fs::read_to_string(&target)
            .unwrap()
            .contains("managed line")
    );
    let backup = record.backup_path.unwrap();
    assert_eq!(std::fs::read_to_string(backup).unwrap(), "user-content\n");
    let manifest = load_manifest(&manifest_path).unwrap();
    assert_eq!(manifest.integrations.len(), 1);
    assert_eq!(manifest.integrations[0].marker_id, "example");
    assert_eq!(
        manifest.integrations[0].post_write_sha256,
        plan.after_sha256
    );
}

#[test]
fn apply_rejects_a_stale_precondition_without_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("tool.conf");
    let manifest_path = temp.path().join("integrations.json");
    std::fs::write(&target, "before\n").unwrap();
    let descriptor = descriptor(target.clone(), "managed line", None);
    let plan = plan_install(&descriptor).unwrap();
    std::fs::write(&target, "changed-after-preview\n").unwrap();

    assert!(
        apply_plan(
            &plan,
            &descriptor,
            &manifest_path,
            &temp.path().join("backups")
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(target).unwrap(),
        "changed-after-preview\n"
    );
    assert!(!manifest_path.exists());
}

#[test]
fn validator_failure_rolls_back_target_and_preserves_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("tool.conf");
    let manifest_path = temp.path().join("integrations.json");
    std::fs::write(&target, "original\n").unwrap();
    save_manifest_atomic(&manifest_path, &IntegrationManifest::default()).unwrap();
    let manifest_before = std::fs::read(&manifest_path).unwrap();
    let descriptor = descriptor(
        target.clone(),
        "managed line",
        Some(Arc::new(ContentValidator {
            required: "impossible-value",
        })),
    );
    let plan = plan_install(&descriptor).unwrap();

    assert!(
        apply_plan(
            &plan,
            &descriptor,
            &manifest_path,
            &temp.path().join("backups")
        )
        .is_err()
    );
    assert_eq!(std::fs::read_to_string(target).unwrap(), "original\n");
    assert_eq!(std::fs::read(manifest_path).unwrap(), manifest_before);
}

#[test]
fn uninstall_plan_removes_only_managed_content_and_manifest_record() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("tool.conf");
    let manifest_path = temp.path().join("integrations.json");
    let backup_dir = temp.path().join("backups");
    let descriptor = descriptor(target.clone(), "managed line", None);
    std::fs::write(&target, "user-before\n").unwrap();
    let install = plan_install(&descriptor).unwrap();
    apply_plan(&install, &descriptor, &manifest_path, &backup_dir).unwrap();

    let uninstall = plan_uninstall(&descriptor).unwrap();
    assert_eq!(uninstall.action, PlanAction::Update);
    assert!(
        std::fs::read_to_string(&target)
            .unwrap()
            .contains("managed line")
    );
    apply_uninstall(&uninstall, &descriptor, &manifest_path, &backup_dir).unwrap();

    assert_eq!(std::fs::read_to_string(target).unwrap(), "user-before\n");
    assert!(
        load_manifest(&manifest_path)
            .unwrap()
            .integrations
            .is_empty()
    );
}

#[cfg(unix)]
#[test]
fn apply_and_uninstall_preserve_existing_target_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("tool.conf");
    let manifest_path = temp.path().join("integrations.json");
    let backup_dir = temp.path().join("backups");
    std::fs::write(&target, "user-content\n").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
    let descriptor = descriptor(target.clone(), "managed line", None);

    let install = plan_install(&descriptor).unwrap();
    apply_plan(&install, &descriptor, &manifest_path, &backup_dir).unwrap();
    assert_eq!(
        std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o644
    );

    let uninstall = plan_uninstall(&descriptor).unwrap();
    apply_uninstall(&uninstall, &descriptor, &manifest_path, &backup_dir).unwrap();
    assert_eq!(
        std::fs::metadata(target).unwrap().permissions().mode() & 0o777,
        0o644
    );
}
