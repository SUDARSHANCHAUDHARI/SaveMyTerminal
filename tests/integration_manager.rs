use savemyterminal::{
    integration::managed::{BlockState, Marker, insert_or_replace, inspect, remove},
    manifest::{IntegrationManifest, IntegrationRecord, load_manifest, save_manifest_atomic},
};
use std::path::PathBuf;

fn marker() -> Marker {
    Marker::new("example", "#").unwrap()
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
}
