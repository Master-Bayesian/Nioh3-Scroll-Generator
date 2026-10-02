//! Owned synthetic files only; none of these tests opens a process handle.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;

const ID: &str = "10000000-0000-4000-8000-000000000001";

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    backup: PathBuf,
    checkpoint: Value,
}
impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "nioh3-equipment-checkpoint-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("source.bin");
        let backup = root.join("backup.bin");
        let raw = b"owned synthetic save bytes";
        std::fs::write(&source, raw).unwrap();
        std::fs::write(&backup, raw).unwrap();
        let checkpoint = json!({
            "operation_id": ID,
            "source_save_path": source,
            "source_save_sha256": sha256_hex(raw),
            "backup_path": backup,
        });
        Self {
            root,
            source,
            backup,
            checkpoint,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn a_valid_checkpoint_is_read_only_and_bound_to_one_operation() {
    let fixture = Fixture::new();
    let source = std::fs::read(&fixture.source).unwrap();
    let backup = std::fs::read(&fixture.backup).unwrap();
    verify_equipment_checkpoint(ID, &fixture.checkpoint).unwrap();
    assert!(verify_equipment_checkpoint(
        "10000000-0000-4000-8000-000000000002",
        &fixture.checkpoint
    )
    .is_err());
    assert_eq!(std::fs::read(&fixture.source).unwrap(), source);
    assert_eq!(std::fs::read(&fixture.backup).unwrap(), backup);
}

#[test]
fn missing_or_changed_backup_and_source_are_never_accepted() {
    let fixture = Fixture::new();
    std::fs::write(&fixture.source, b"new source bytes").unwrap();
    let error = verify_equipment_checkpoint(ID, &fixture.checkpoint).unwrap_err();
    assert!(error.message().contains("Source save changed"));
    assert!(error.message().contains("prepare again"));
    std::fs::copy(&fixture.backup, &fixture.source).unwrap();
    std::fs::write(&fixture.backup, b"damaged backup").unwrap();
    assert!(verify_equipment_checkpoint(ID, &fixture.checkpoint)
        .unwrap_err()
        .message()
        .contains("Verified backup changed"));
    std::fs::remove_file(&fixture.backup).unwrap();
    assert!(verify_equipment_checkpoint(ID, &fixture.checkpoint).is_err());
}

#[test]
fn the_source_itself_cannot_stand_in_for_a_backup() {
    let fixture = Fixture::new();
    let mut checkpoint = fixture.checkpoint.clone();
    checkpoint["backup_path"] = json!(fixture.source);
    assert!(verify_equipment_checkpoint(ID, &checkpoint)
        .unwrap_err()
        .message()
        .contains("separate backup"));
}

#[test]
fn prepare_without_a_checkpoint_refuses_before_process_access_or_preview() {
    let fixture = Fixture::new();
    // Constructing this coordinator only opens its own journal directory.
    let mut app = EquipmentAddition::new(u32::MAX, &fixture.root).unwrap();
    let error = app
        .prepare(
            ID,
            &json!({
                "item_id": 1, "level": 1, "plus": 0, "rarity": 3, "seed": 7
            }),
        )
        .unwrap_err();
    assert!(error.message().contains("verified save checkpoint"));
    assert!(!app.request_path(ID).unwrap().exists());
    assert!(app.pending_preview().is_none());
}

#[test]
fn execute_rechecks_checkpoint_before_claim_or_process_access_and_cancel_stays_available() {
    let fixture = Fixture::new();
    let mut app = EquipmentAddition::new(u32::MAX, &fixture.root).unwrap();
    let plan = json!({
        "operation_id": ID, "kind": "equipment_native_add",
        "pid": u32::MAX,
        "save_checkpoint": fixture.checkpoint,
    });
    let snapshot = app.operations.prepare(ID, &plan).unwrap();
    std::fs::write(&fixture.source, b"changed after preparation").unwrap();
    let error = app.execute(ID, &snapshot.plan_digest).unwrap_err();
    assert!(error.message().contains("Source save changed"));
    assert!(app.operations.snapshot(ID).unwrap().can_dispatch);
    assert!(!app
        .operations
        .directory(ID)
        .unwrap()
        .join("claim.json")
        .exists());
    assert_eq!(app.cancel(ID).unwrap()["state"], "cancelled");
}

#[test]
fn checkpoint_metadata_is_covered_by_the_reviewed_plan_digest() {
    let fixture = Fixture::new();
    let operations = LiveAddOperations::new(&fixture.root.join("operations")).unwrap();
    let plan = json!({"operation_id": ID, "save_checkpoint": fixture.checkpoint});
    operations.prepare(ID, &plan).unwrap();
    let (before, _) = operations.plan(ID).unwrap();
    let mut changed = plan.clone();
    changed["save_checkpoint"]["source_save_sha256"] = json!("b".repeat(64));
    assert_ne!(
        before,
        sha256_hex(super::super::count::canonical_json(&changed).as_bytes())
    );
}
