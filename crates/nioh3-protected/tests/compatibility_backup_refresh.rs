#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Only owned synthetic files: an explicit backup retry must inspect current bytes.
use nioh3_protected::compatibility::{CompatibilitySession, ExecutableIdentity};
use serde_json::Value;
use std::path::PathBuf;

fn fixture(name: &str) -> (PathBuf, ExecutableIdentity) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "nioh3-backup-refresh-{name}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let identity = ExecutableIdentity {
        pid: 42,
        creation_filetime: 77,
        path: "D:/owned-fixture/Nioh3.exe".into(),
        version: "2.0.1.0".into(),
        sha256: "a".repeat(64),
    };
    (root, identity)
}

fn backup_path(report: &Value, index: usize) -> PathBuf {
    PathBuf::from(report["backup"]["paths"][index].as_str().unwrap())
}

#[test]
fn retry_copies_current_saves_and_preserves_the_previous_backup() {
    let (root, identity) = fixture("changed");
    let first_save = root.join("first-SAVEDATA.BIN");
    let added_save = root.join("added-SAVEDATA.BIN");
    std::fs::write(&first_save, b"original synthetic save").unwrap();
    let mut session = CompatibilitySession::default();
    let first = session.prepare(&identity, &root, std::slice::from_ref(&first_save));
    assert_eq!(first["backup"]["verified"], true);
    let previous = backup_path(&first, 0);

    std::fs::write(&first_save, b"updated synthetic save").unwrap();
    std::fs::write(&added_save, b"new character synthetic save").unwrap();
    let refreshed = session.prepare(&identity, &root, &[first_save.clone(), added_save.clone()]);
    assert_eq!(refreshed["backup"]["verified"], true);
    assert_eq!(
        std::fs::read(backup_path(&refreshed, 0)).unwrap(),
        b"updated synthetic save",
        "Retry must copy current bytes even after a successful backup"
    );
    assert_eq!(refreshed["backup"]["paths"].as_array().unwrap().len(), 2);
    assert_eq!(
        std::fs::read(backup_path(&refreshed, 1)).unwrap(),
        b"new character synthetic save"
    );
    assert_ne!(backup_path(&refreshed, 0), previous);
    assert_eq!(std::fs::read(previous).unwrap(), b"original synthetic save");
    assert_eq!(
        std::fs::read(first_save).unwrap(),
        b"updated synthetic save"
    );
    assert_eq!(
        std::fs::read(added_save).unwrap(),
        b"new character synthetic save"
    );
    assert_eq!(session.report(&identity)["backup"], refreshed["backup"]);
}

#[test]
fn unsuccessful_retry_does_not_report_the_previous_backup_as_current() {
    let (root, identity) = fixture("missing");
    let save = root.join("SAVEDATA.BIN");
    std::fs::write(&save, b"owned synthetic save").unwrap();
    let mut session = CompatibilitySession::default();
    let first = session.prepare(&identity, &root, std::slice::from_ref(&save));
    assert_eq!(first["backup"]["verified"], true);

    let missing = session.prepare(&identity, &root, &[]);
    assert_eq!(missing["backup"]["verified"], false);
    assert_eq!(missing["backup"]["paths"], serde_json::json!([]));
    assert!(missing["backup"]["error"]
        .as_str()
        .unwrap()
        .contains("manual confirmation"));
    assert_eq!(session.report(&identity)["backup"], missing["backup"]);
    assert_eq!(
        std::fs::read(backup_path(&first, 0)).unwrap(),
        b"owned synthetic save"
    );
    assert!(session.accept(&identity, "", true, false).is_err());
    let recovered = session.prepare(&identity, &root, std::slice::from_ref(&save));
    assert_eq!(recovered["backup"]["verified"], true);
    assert_ne!(backup_path(&recovered, 0), backup_path(&first, 0));
    assert!(session
        .accept(
            &identity,
            recovered["plan"]["plan_id"].as_str().unwrap(),
            true,
            true
        )
        .is_ok());
    assert_eq!(std::fs::read(save).unwrap(), b"owned synthetic save");
}
