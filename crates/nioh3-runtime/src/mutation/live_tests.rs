//! Unit coverage for the inventory, live-add and batch slices.

// Tests assert on outcomes directly; the crate's production lints still keep
// `expect`, `unwrap` and `panic` out of non-test code.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::live_fakes::{
    assembly_record, candidate_payload, decrypted_save, FakeLiveAddExecutor, FakeSaveBackup,
    InventoryFixture, LiveAddFaults, NoCatalogPolicy,
};
use super::*;
use crate::mutation::inventory::capture_inventory;
use crate::mutation::inventory::RECORD_SIZE;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const CONTEXT_DIGEST: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "nioh3-runtime-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("scratch root");
    root
}

fn save_path(root: &Path) -> PathBuf {
    let directory = root.join("76561198000000000").join("SAVEDATA00");
    std::fs::create_dir_all(&directory).expect("save directory");
    let path = directory.join("SAVEDATA.BIN");
    std::fs::write(&path, b"fixture-save").expect("save file");
    path
}

fn fixture_application(
    root: &Path,
    executor: FakeLiveAddExecutor,
    backup: FakeSaveBackup,
) -> LiveAddApplication {
    LiveAddApplication::new(
        root,
        CONTEXT_DIGEST,
        Box::new(executor),
        Box::new(backup),
        Box::new(NoCatalogPolicy::default()),
    )
    .expect("application")
}

#[test]
fn the_inventory_capture_matches_the_shipped_gates() {
    let fixture = InventoryFixture::new(&[(0, 0x1122, 0x0BAD), (7, 0x3344, 0x0BEE)], 0x3345, 11);
    let (inventory, index) = fixture.capture().expect("capture");
    assert_eq!(inventory.entries.len(), 2);
    assert_eq!(inventory.entries[0].slot_index, 0);
    assert_eq!(inventory.entries[0].serial, "4386");
    assert_eq!(inventory.entries[0].seed, 0x0BAD);
    assert_eq!(inventory.serial_counter, "13125");
    assert_eq!(inventory.acquisition_order_counter, 11);
    assert!(inventory.duplicate_scroll_serials.is_empty());
    assert_eq!(index.node_count, 2);
    assert_eq!(index.entries.len(), 2);
    assert_eq!(index.slot_of("4386"), Some(0));
    assert_eq!(
        index_entries(&index.to_json()).expect("index"),
        index.entries_by_serial().expect("entries")
    );
}

#[test]
fn the_inventory_capture_fails_closed() {
    let mut fixture = InventoryFixture::new(&[(3, 0x77, 9)], 0x78, 1);
    fixture
        .memory
        .write(fixture.base + fixture.layout.insertion_rva, &[0u8; 16]);
    let mut view = super::live_fakes::fixture_view(&fixture);
    let error =
        capture_inventory(&mut view, &fixture.layout, "PC v2.01").expect_err("signature gate");
    assert_eq!(error.message(), "Inventory insertion signature mismatch");

    let fixture = InventoryFixture::new(&[(1, 0x55, 4)], 0x56, 2);
    let mut view = super::live_fakes::fixture_view(&fixture);
    let error = capture_inventory(&mut view, &fixture.layout, "PC v2.00").expect_err("version");
    assert_eq!(
        error.message(),
        "This inventory layout is validated only for PC v2.01"
    );
}

#[test]
fn a_duplicate_serial_is_refused_by_the_entry_filter() {
    let mut fixture = InventoryFixture::new(&[(0, 0x99, 1)], 0x9A, 0);
    let container = fixture.clone().container().expect("container");
    let data = fixture.data().expect("data");
    let start = data + fixture.layout.container_offset + (5 * RECORD_SIZE) as u64;
    fixture.memory.write(start, &container[..RECORD_SIZE]);
    let (inventory, _index) = fixture.capture().expect("capture");
    assert_eq!(inventory.duplicate_scroll_serials, vec!["153".to_string()]);
    assert_eq!(
        inventory_entries(&inventory)
            .expect_err("duplicate")
            .message(),
        "Invalid inventory capacity or duplicate serials"
    );
}

#[test]
fn a_candidate_is_rejected_before_it_reaches_the_executor() {
    let root = scratch("candidate");
    let path = save_path(&root);
    let fixture = InventoryFixture::new(&[], 1, 0);
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let record = assembly_record(0x1E82, 0x0BAD, 4);

    let wrong_context = json!({
        "candidate_id": "00",
        "context_digest": "another",
        "record_stage": "final_record",
        "record_hex": hex(&record),
        "installation_record_hex": Value::Null,
        "effects": [],
        "level": 1,
        "seed": 0x0BAD,
        "playthrough": 2,
        "rarity": 4,
    });
    assert_eq!(
        application
            .validate_candidate(&wrong_context)
            .expect_err("context")
            .message(),
        "Candidate generation context has changed"
    );

    let stage_one = candidate_payload(
        CONTEXT_DIGEST,
        0x0BAD,
        4,
        &record,
        None,
        CandidateStage::NativeStageOne,
    )
    .expect("payload");
    let candidate =
        InstallationCandidate::from_payload(&stage_one, CONTEXT_DIGEST).expect("candidate");
    assert_eq!(
        candidate
            .require_search_candidate_ready()
            .expect_err("stage one")
            .message(),
        "稀有度4搜索结果仍是原生待揭露中间态，已拒绝加入候选列表"
    );

    let unresolved = candidate_payload(
        CONTEXT_DIGEST,
        0x0BAD,
        4,
        &record,
        None,
        CandidateStage::FinalRecord,
    )
    .expect("payload");
    let mut blocked = fixture_application(
        &root.join("blocked"),
        FakeLiveAddExecutor::new(InventoryFixture::new(&[], 1, 0)),
        FakeSaveBackup::new(&root.join("blocked")),
    );
    let blocker = "当前候选只包含离线词条序列，而且该周目/稀有度尚未通过完整记录原生一致性门禁，暂不允许写入。";
    let mut application_with_policy = LiveAddApplication::new(
        &root.join("blocked"),
        CONTEXT_DIGEST,
        Box::new(FakeLiveAddExecutor::new(InventoryFixture::new(&[], 1, 0))),
        Box::new(FakeSaveBackup::new(&root.join("blocked"))),
        Box::new(NoCatalogPolicy {
            blocker: Some(blocker.to_string()),
        }),
    )
    .expect("application");
    assert_eq!(
        application_with_policy
            .validate_candidate(&unresolved)
            .expect_err("catalog blocker")
            .message(),
        blocker
    );
    assert!(
        blocked
            .validate_candidate(&unresolved)
            .expect("no catalog blocker")
            .0
            .stage
            == CandidateStage::FinalRecord
    );
    let _ = path;
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn prepared(
    application: &mut LiveAddApplication,
    save: &Path,
    seed: u32,
) -> (PreparedLiveAdd, String) {
    let record = assembly_record(0x1E82, seed, 4);
    let payload = candidate_payload(
        CONTEXT_DIGEST,
        seed,
        4,
        &record,
        None,
        CandidateStage::FinalRecord,
    )
    .expect("payload");
    let prepared = application
        .prepare(&payload, save, None)
        .expect("prepared live add");
    let digest = prepared.snapshot.plan_digest.clone();
    (prepared, digest)
}

#[test]
fn a_reviewed_live_add_verifies_once_and_never_replays() {
    let root = scratch("live-add");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x1000, 0x11)], 0x1001, 5);
    let saved = decrypted_save(&fixture).expect("saved records");
    std::fs::write(&save, &saved).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let (prepared, digest) = prepared(&mut application, &save, 0x0BAD);
    assert_eq!(prepared.count_before, 1);
    assert_eq!(prepared.rarity, 4);
    assert_eq!(prepared.instance_serial, "4097");

    let first = application
        .execute(&prepared.snapshot.operation_id, &digest)
        .expect("execute");
    assert_eq!(first.state, OperationState::Verified);
    assert!(first
        .receipt
        .as_ref()
        .expect("receipt")
        .get("full_container_and_native_index_verified")
        .and_then(Value::as_bool)
        .expect("flag"));
    assert_eq!(
        first.receipt.as_ref().expect("receipt")["count_after"],
        json!(2)
    );

    let second = application
        .execute(&prepared.snapshot.operation_id, &digest)
        .expect("no replay");
    assert_eq!(second.state, OperationState::Verified);
    assert!(application.safe_to_shutdown());
}

#[test]
fn a_lost_native_reply_stays_uncertain_until_its_receipt_is_read() {
    let root = scratch("live-add-lost");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x2000, 0x22)], 0x2001, 6);
    std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
    let faults = LiveAddFaults {
        reply_lost: true,
        ..LiveAddFaults::default()
    };
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::with_faults(fixture, faults),
        FakeSaveBackup::new(&root),
    );
    let (prepared, digest) = prepared(&mut application, &save, 0x0BEE);
    let operation_id = prepared.snapshot.operation_id.clone();

    application
        .execute(&operation_id, &digest)
        .expect_err("lost reply");
    assert_eq!(
        application.status(&operation_id).expect("status").state,
        OperationState::Uncertain
    );
    assert_eq!(
        application
            .execute(&operation_id, &digest)
            .expect_err("no replay")
            .message(),
        "Operation is cancelled or uncertain; do not replay it"
    );
    let recovered = application.recover(&operation_id).expect("recover");
    assert_eq!(recovered.state, OperationState::Verified);
}

#[test]
fn a_changed_readback_keeps_the_operation_uncertain() {
    let root = scratch("live-add-readback");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x3000, 0x33)], 0x3001, 7);
    std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
    let faults = LiveAddFaults {
        readback_changed: true,
        ..LiveAddFaults::default()
    };
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::with_faults(fixture, faults),
        FakeSaveBackup::new(&root),
    );
    let (prepared, digest) = prepared(&mut application, &save, 0x0C00);
    let operation_id = prepared.snapshot.operation_id.clone();
    let error = application
        .execute(&operation_id, &digest)
        .expect_err("verification");
    assert_eq!(
        error.message(),
        "Expected exactly one newly allocated serial"
    );
    assert_eq!(
        application.status(&operation_id).expect("status").state,
        OperationState::Uncertain
    );
    assert!(application.recover(&operation_id).is_err());
}

#[test]
fn a_recycled_process_instance_refuses_an_old_receipt() {
    let root = scratch("live-add-pid");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x4000, 0x44)], 0x4001, 8);
    std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
    let faults = LiveAddFaults {
        pid_reuse: true,
        ..LiveAddFaults::default()
    };
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::with_faults(fixture, faults),
        FakeSaveBackup::new(&root),
    );
    let (prepared, digest) = prepared(&mut application, &save, 0x0C11);
    let operation_id = prepared.snapshot.operation_id.clone();
    assert_eq!(
        application
            .execute(&operation_id, &digest)
            .expect_err("pid reuse")
            .message(),
        "PROCESS_INSTANCE_CHANGED: do not verify an old receipt against a new game"
    );
    assert_eq!(
        application.status(&operation_id).expect("status").state,
        OperationState::Uncertain
    );
}

#[test]
fn a_proven_absence_and_an_idle_miss_are_rejections_not_uncertainty() {
    for (name, absent, idle) in [("absent", true, false), ("idle", false, true)] {
        let root = scratch(&format!("live-add-{name}"));
        let save = save_path(&root);
        let fixture = InventoryFixture::new(&[(0, 0x5000, 0x55)], 0x5001, 9);
        std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
        let faults = LiveAddFaults {
            submission_absent: absent,
            idle_miss: idle,
            ..LiveAddFaults::default()
        };
        let mut application = fixture_application(
            &root,
            FakeLiveAddExecutor::with_faults(fixture, faults),
            FakeSaveBackup::new(&root),
        );
        let (prepared, digest) = prepared(&mut application, &save, 0x0C22);
        let snapshot = application
            .execute(&prepared.snapshot.operation_id, &digest)
            .expect("terminal rejection");
        assert_eq!(snapshot.state, OperationState::RejectedBeforeDispatch);
        assert_eq!(
            snapshot.receipt.as_ref().expect("receipt")["redirect_count"],
            json!(0)
        );
    }
}

#[test]
fn an_unresolved_insertion_blocks_the_next_preparation() {
    let root = scratch("live-add-unresolved");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x6000, 0x66)], 0x6001, 10);
    std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
    let faults = LiveAddFaults {
        reply_lost: true,
        ..LiveAddFaults::default()
    };
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::with_faults(fixture.clone(), faults),
        FakeSaveBackup::new(&root),
    );
    let (prepared, digest) = prepared(&mut application, &save, 0x0C33);
    let operation_id = prepared.snapshot.operation_id.clone();
    application
        .execute(&operation_id, &digest)
        .expect_err("lost reply");

    let mut second = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let error = second
        .prepare(
            &candidate_payload(
                CONTEXT_DIGEST,
                0x0C44,
                4,
                &assembly_record(0x1E82, 0x0C44, 4),
                None,
                CandidateStage::FinalRecord,
            )
            .expect("payload"),
            &save,
            None,
        )
        .expect_err("uncertain insertion");
    assert_eq!(error.code(), "LIVE_ADD_UNCERTAIN");
    assert!(error.message().contains(&operation_id));
}

fn batch_candidates(count: usize) -> Vec<Value> {
    (0..count)
        .map(|index| {
            candidate_payload(
                CONTEXT_DIGEST,
                0x0D00 + index as u32,
                4,
                &assembly_record(0x1E82, 0x0D00 + index as u32, 4),
                None,
                CandidateStage::FinalRecord,
            )
            .expect("payload")
        })
        .collect()
}

#[test]
fn a_batch_verifies_every_item_and_keeps_a_child_receipt() {
    let root = scratch("live-batch");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x7000, 0x77)], 0x7001, 3);
    std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let candidates = batch_candidates(2);
    let prepared = LiveAddBatch::prepare(&mut application, &candidates, &save).expect("prepared");
    let batch_id = prepared["batch_id"].as_str().expect("batch id").to_string();
    let digest = prepared["plan_digest"]
        .as_str()
        .expect("digest")
        .to_string();
    let mut progress = Vec::new();
    let receipt = LiveAddBatch::execute(
        &mut application,
        &batch_id,
        &digest,
        &mut || false,
        &mut |value| progress.push(value),
    )
    .expect("executed");
    assert_eq!(receipt["state"], json!("complete"));
    assert_eq!(receipt["verified_count"], json!(2));
    assert_eq!(progress.len(), 3);
    let status = LiveAddBatch::status(&application, &batch_id).expect("status");
    assert_eq!(status["state"], json!("complete"));
    assert_eq!(status["requested_count"], json!(2));
    assert_eq!(status["children"].as_array().expect("children").len(), 2);
}

#[test]
fn a_batch_cancel_between_items_leaves_only_verified_children() {
    let root = scratch("live-batch-cancel");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x8000, 0x88)], 0x8001, 4);
    std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let candidates = batch_candidates(3);
    let prepared = LiveAddBatch::prepare(&mut application, &candidates, &save).expect("prepared");
    let batch_id = prepared["batch_id"].as_str().expect("batch id").to_string();
    let digest = prepared["plan_digest"]
        .as_str()
        .expect("digest")
        .to_string();
    let mut seen = 0;
    let receipt = LiveAddBatch::execute(
        &mut application,
        &batch_id,
        &digest,
        &mut || {
            seen += 1;
            seen > 1
        },
        &mut |_value| {},
    )
    .expect("executed");
    assert_eq!(receipt["state"], json!("partial"));
    assert_eq!(receipt["verified_count"], json!(1));
    let status = LiveAddBatch::status(&application, &batch_id).expect("status");
    assert_eq!(status["state"], json!("partial"));
    assert_eq!(status["claimed"], json!(true));
    assert_eq!(status["children"].as_array().expect("children").len(), 1);
}

#[test]
fn a_batch_cancel_at_the_first_item_cancels_that_operation() {
    let root = scratch("live-batch-first");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x9000, 0x99)], 0x9001, 2);
    std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let candidates = batch_candidates(2);
    let prepared = LiveAddBatch::prepare(&mut application, &candidates, &save).expect("prepared");
    let batch_id = prepared["batch_id"].as_str().expect("batch id").to_string();
    let digest = prepared["plan_digest"]
        .as_str()
        .expect("digest")
        .to_string();
    let receipt = LiveAddBatch::execute(
        &mut application,
        &batch_id,
        &digest,
        &mut || true,
        &mut |_value| {},
    )
    .expect("executed");
    assert_eq!(receipt["state"], json!("partial"));
    assert_eq!(receipt["verified_count"], json!(0));
    let status = LiveAddBatch::status(&application, &batch_id).expect("status");
    assert_eq!(status["children"].as_array().expect("children").len(), 0);
}

#[test]
fn the_selected_batch_is_gated_before_any_native_work() {
    let root = scratch("live-batch-gates");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0xA000, 0xAA)], 0xA001, 1);
    std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let mut duplicate = batch_candidates(2);
    duplicate[1]["candidate_id"] = duplicate[0]["candidate_id"].clone();
    assert_eq!(
        LiveAddBatch::prepare(&mut application, &duplicate, &save)
            .expect_err("duplicate")
            .message(),
        "Duplicate candidate identity"
    );
    assert_eq!(
        LiveAddBatch::prepare(&mut application, &[], &save)
            .expect_err("empty")
            .message(),
        "Batch size must be 1-200"
    );
    let too_many = batch_candidates(201);
    assert_eq!(
        LiveAddBatch::prepare(&mut application, &too_many, &save)
            .expect_err("too many")
            .message(),
        "Batch size must be 1-200"
    );
    let full = (0..399usize)
        .map(|slot| (slot, 0x1000 + slot as u64, 1u32))
        .collect::<Vec<_>>();
    let crowded_fixture = InventoryFixture::new(&full, 0x2000, 1);
    let crowded_save = save_path(&root.join("crowded"));
    std::fs::write(
        &crowded_save,
        decrypted_save(&crowded_fixture).expect("crowded saved"),
    )
    .expect("crowded save");
    let mut crowded = fixture_application(
        &root.join("crowded"),
        FakeLiveAddExecutor::new(crowded_fixture),
        FakeSaveBackup::new(&root.join("crowded")),
    );
    let over_capacity = batch_candidates(2);
    assert_eq!(
        LiveAddBatch::prepare(&mut crowded, &over_capacity, &crowded_save)
            .expect_err("capacity")
            .message(),
        "Insufficient scroll capacity"
    );
}

#[test]
fn the_count_adapter_resolves_its_record_through_the_inventory_gate() {
    use crate::mutation::count::{CountLayout, CountMemory, WindowsCountMemory};
    use crate::mutation::live_fakes::FixtureProcesses;
    let fixture = InventoryFixture::new(&[(4, 0x1234, 0xF00D)], 0x1235, 12);
    let layout = CountLayout {
        manager_pointer_rva: fixture.layout.manager_pointer_rva,
        container_offset: fixture.layout.container_offset,
        capacity_offset: fixture.layout.capacity_offset,
        insertion_rva: fixture.layout.insertion_rva,
        capacity: 400,
        record_size: RECORD_SIZE,
        count_offset: 0x33,
        display_version: "PC v2.01",
    };
    let processes = FixtureProcesses::new(&fixture);
    let mut memory = WindowsCountMemory::new(4321, fixture.base, layout, processes);
    let capture = memory.capture(0x1234).expect("capture");
    assert_eq!(capture.serial, 0x1234);
    assert_eq!(capture.record_hex[0x33 * 2..0x33 * 2 + 2], *"02");
    let after = memory.write(&capture, 5).expect("write");
    assert_eq!(after[0x33], 5);
    assert_eq!(
        memory.capture(0x9999).expect_err("missing serial").code(),
        "COUNT_INSTANCE_UNAVAILABLE"
    );
}

/// The preview children one application owns, read from its state root.
fn preview_children(application: &LiveAddApplication) -> Vec<String> {
    let mut children: Vec<String> = std::fs::read_dir(application.operations().root())
        .expect("operations root")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.join("preview-child.json").is_file())
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .collect();
    children.sort();
    children
}

/// The application records the reviewed facts one live-addition payload names.
fn preview_payload(seed: u32) -> Value {
    let record = assembly_record(0x1E82, seed, 4);
    candidate_payload(
        CONTEXT_DIGEST,
        seed,
        4,
        &record,
        None,
        CandidateStage::FinalRecord,
    )
    .expect("payload")
}

/// A rejected preview leaves a durable, non-dispatchable child an operator can
/// find and recover, and never publishes the insertion plan.
#[test]
fn a_rejected_preview_leaves_a_discoverable_child() {
    let root = scratch("live-add-preview-rejected");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x1000, 0x11)], 0x1001, 5);
    let saved = decrypted_save(&fixture).expect("saved records");
    std::fs::write(&save, &saved).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::with_faults(
            fixture,
            LiveAddFaults {
                preview_mismatch: true,
                ..LiveAddFaults::default()
            },
        ),
        FakeSaveBackup::new(&root),
    );

    let error = application
        .prepare(&preview_payload(0x0BAD), &save, None)
        .expect_err("the reviewed output is not reproduced");
    assert_eq!(error.code(), "LIVE_ADD_REJECTED", "{error:?}");

    let children = preview_children(&application);
    assert_eq!(children.len(), 1, "one durable preview child");
    let child = &children[0];
    assert!(
        error.message().contains(child.as_str()),
        "the failure names the child it left behind: {error}"
    );
    // No insertion plan was published: the child is the only operation record.
    assert_eq!(
        std::fs::read_dir(application.operations().root())
            .expect("operations root")
            .count(),
        1
    );

    let snapshot = application.status(child).expect("child status");
    assert_eq!(snapshot.state, OperationState::RejectedAfterPreview);
    assert!(
        !snapshot.can_dispatch,
        "a preview child is never dispatchable"
    );
    assert!(!snapshot.can_cancel);

    // Recovering the child is read-only and idempotent, and the child can never
    // be claimed or executed as an insertion.
    let recovered = application.recover(child).expect("read-only recovery");
    assert_eq!(recovered.state, OperationState::RejectedAfterPreview);
    let again = application.recover(child).expect("idempotent recovery");
    assert_eq!(again.state, OperationState::RejectedAfterPreview);
    assert!(application.operations().claim(child, "digest").is_err());
    assert!(application.execute(child, "digest").is_err());
}

/// A preview that matched leaves its own terminal, non-dispatchable child: the
/// review is durable and it never fences the next preparation.
#[test]
fn a_matched_preview_leaves_a_terminal_child() {
    let root = scratch("live-add-preview-completed");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x1000, 0x11)], 0x1001, 5);
    let saved = decrypted_save(&fixture).expect("saved records");
    std::fs::write(&save, &saved).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let (_prepared, _digest) = prepared(&mut application, &save, 0x0BAD);

    let children = preview_children(&application);
    assert_eq!(children.len(), 1, "the attempt's child is durable");
    let snapshot = application.status(&children[0]).expect("child status");
    assert_eq!(snapshot.state, OperationState::PreviewCompleted);
    assert!(!snapshot.can_dispatch);
    assert!(
        application
            .operations()
            .unresolved_ids()
            .expect("unresolved ids")
            .is_empty(),
        "a terminal preview child never fences the next preparation"
    );
}

/// The window between the pre-registration and the durable native receipt: the
/// child owns no plan, so the refusal must name it instead of failing to read a
/// plan that never existed.
#[test]
fn a_crash_after_preview_registration_still_names_the_child() {
    let root = scratch("live-add-preview-orphan");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x1000, 0x11)], 0x1001, 5);
    let saved = decrypted_save(&fixture).expect("saved records");
    std::fs::write(&save, &saved).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let child = crate::mutation::count::new_operation_id().expect("child id");
    application
        .operations()
        .register_preview_child(
            &child,
            &json!({
                "parent_operation_id": "11111111-1111-4111-8111-111111111111",
                "mode": "preview",
                "pid": 4321,
                "process_creation_time": crate::mutation::live_fakes::FIXTURE_CREATION.to_string(),
            }),
        )
        .expect("child record");
    assert!(
        application
            .operations()
            .unresolved_ids()
            .expect("ids")
            .contains(&child),
        "the orphan child fences admission"
    );
    assert!(!application
        .operations()
        .directory(&child)
        .expect("directory")
        .join("plan.json")
        .is_file());

    let error = application
        .prepare(&preview_payload(0x0BAD), &save, None)
        .expect_err("the orphan child blocks the next preparation");
    assert_eq!(error.code(), "LIVE_ADD_UNCERTAIN", "{error:?}");
    assert!(
        error.message().contains(&child),
        "the refusal names the plan-less child: {error}"
    );
    assert_eq!(
        application.status(&child).expect("child status").state,
        OperationState::PreparingPreview
    );
    assert!(
        application.recover(&child).is_err(),
        "no native receipt means no settlement"
    );
}

/// An uncertain preview child — the preview ran but left no complete proof —
/// blocks the next preparation by naming itself, not by failing on a plan.
#[test]
fn an_uncertain_preview_child_blocks_the_next_prepare_by_id() {
    let root = scratch("live-add-preview-uncertain");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x1000, 0x11)], 0x1001, 5);
    let saved = decrypted_save(&fixture).expect("saved records");
    std::fs::write(&save, &saved).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::with_faults(
            fixture,
            LiveAddFaults {
                preview_incomplete: true,
                ..LiveAddFaults::default()
            },
        ),
        FakeSaveBackup::new(&root),
    );
    let first = application
        .prepare(&preview_payload(0x0BAD), &save, None)
        .expect_err("the preview leaves no complete proof");
    assert_eq!(first.code(), "LIVE_ADD_VERIFICATION", "{first:?}");

    let children = preview_children(&application);
    assert_eq!(children.len(), 1);
    let child = &children[0];
    let snapshot = application.status(child).expect("child status");
    assert_eq!(snapshot.state, OperationState::Uncertain);
    assert!(!snapshot.can_dispatch);

    let second = application
        .prepare(&preview_payload(0x0BAD), &save, None)
        .expect_err("the uncertain child blocks the next preparation");
    assert_eq!(second.code(), "LIVE_ADD_UNCERTAIN", "{second:?}");
    assert!(
        second.message().contains(child.as_str()),
        "the refusal names the child: {second}"
    );
}

/// The live inventory with its slot-0 record's rarity bytes rewritten on disk,
/// the shape the PC v2.02 diagnosis observed (disk 5/5, live 4/4).
fn diverged_disk(fixture: &InventoryFixture) -> Vec<u8> {
    let mut saved = decrypted_save(fixture).expect("saved records");
    let start = SCROLL_GROUP_OFFSET;
    saved[start + 0x30] = 5;
    saved[start + 0x31] = 5;
    saved
}

/// Prepare records the actual disk checkpoint (`D0`) and no longer requires it
/// to equal the live-before inventory (`Li`); execution still verifies exactly
/// `Li` plus the one planned record.
#[test]
fn a_disk_checkpoint_that_differs_from_live_does_not_block_prepare() {
    let root = scratch("live-add-disk-live");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x1000, 0x11), (3, 0x1002, 0x12)], 0x1003, 5);
    let saved = diverged_disk(&fixture);
    std::fs::write(&save, &saved).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let (prepared, digest) = prepared(&mut application, &save, 0x0BAD);
    let (_digest, plan) = application
        .operations()
        .plan(&prepared.snapshot.operation_id)
        .expect("stored plan");
    let expected = disk_persistence_baseline(&saved_scroll_records(&saved).expect("records"));
    assert_eq!(plan[DISK_PERSISTENCE_BASELINE_FIELD], expected);
    assert_eq!(expected["slots"].as_array().expect("slots").len(), 2);
    assert_eq!(expected["slots"][1]["slot_index"], json!(3));
    // The live-before snapshot is no longer mislabelled as a disk baseline.
    assert!(plan.get("persistence_baseline").is_none());

    let receipt = application
        .execute(&prepared.snapshot.operation_id, &digest)
        .expect("execute");
    assert_eq!(receipt.state, OperationState::Verified);
}

/// Every batch child inherits the first checkpoint: the source path and raw
/// hash pin the bytes, and the recorded `D0` must match the new capture.
#[test]
fn a_batch_over_a_diverged_disk_checkpoint_verifies_every_item() {
    let root = scratch("live-batch-disk-live");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x7000, 0x77)], 0x7001, 3);
    std::fs::write(&save, diverged_disk(&fixture)).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let candidates = batch_candidates(3);
    let prepared = LiveAddBatch::prepare(&mut application, &candidates, &save).expect("prepared");
    let batch_id = prepared["batch_id"].as_str().expect("batch id").to_string();
    let digest = prepared["plan_digest"]
        .as_str()
        .expect("digest")
        .to_string();
    let receipt = LiveAddBatch::execute(
        &mut application,
        &batch_id,
        &digest,
        &mut || false,
        &mut |_| {},
    )
    .expect("executed");
    assert_eq!(receipt["state"], json!("complete"));
    assert_eq!(receipt["verified_count"], json!(3));
}

/// Each version's count layout reads the inventory through exactly the
/// `(layout, version)` pair the inventory gate accepts, so PC v2.02 is not
/// refused by a hard-coded PC v2.01 name and cannot borrow another build's.
#[test]
fn each_count_layout_reads_its_own_accepted_inventory_pair() {
    use crate::mutation::count::{WindowsCountMemory, PC_V201_COUNT_LAYOUT, PC_V202_COUNT_LAYOUT};
    use crate::mutation::inventory::{
        accepted_inventory_version, PC_V201_INVENTORY_LAYOUT, PC_V202_INVENTORY_LAYOUT_CANDIDATE,
    };
    use crate::mutation::live_fakes::FixtureProcesses;
    let fixture = InventoryFixture::new(&[(4, 0x1234, 0xF00D)], 0x1235, 12);
    for (layout, inventory, version) in [
        (PC_V201_COUNT_LAYOUT, PC_V201_INVENTORY_LAYOUT, "PC v2.01"),
        (
            PC_V202_COUNT_LAYOUT,
            PC_V202_INVENTORY_LAYOUT_CANDIDATE,
            "PC v2.02",
        ),
    ] {
        let memory = WindowsCountMemory::new(1, 0, layout, FixtureProcesses::new(&fixture));
        assert_eq!(memory.inventory_layout(), inventory);
        assert_eq!(layout.display_version, version);
        assert_eq!(accepted_inventory_version(&inventory), Some(version));
    }
}

/// Saves in the wild carry scroll records that share a `+0x28` serial, and the
/// game loads them. Live addition keys old records by slot, so such an
/// inventory is prepared and verified record for record; only the new serial
/// must be unused.
#[test]
fn an_inventory_with_duplicate_serials_accepts_one_live_addition() {
    let root = scratch("live-add-duplicates");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(
        &[(0, 0x1000, 0x11), (2, 0x1000, 0x12), (5, 0x1001, 0x13)],
        0x1002,
        5,
    );
    let (inventory, _index) = fixture.capture().expect("capture");
    assert_eq!(inventory.duplicate_scroll_serials, vec!["4096".to_string()]);
    std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let (prepared, digest) = prepared(&mut application, &save, 0x0BAD);
    assert_eq!(prepared.count_before, 3);
    let receipt = application
        .execute(&prepared.snapshot.operation_id, &digest)
        .expect("execute");
    assert_eq!(receipt.state, OperationState::Verified);
    let receipt = receipt.receipt.expect("receipt");
    assert_eq!(receipt["previous_records_preserved"], json!(3));
    assert_eq!(receipt["count_after"], json!(4));
}

/// The serial the game would allocate next must not already name a record.
#[test]
fn a_next_serial_that_an_existing_record_uses_is_refused() {
    let root = scratch("live-add-serial-taken");
    let save = save_path(&root);
    let fixture = InventoryFixture::new(&[(0, 0x1000, 0x11), (2, 0x1000, 0x12)], 0x1000, 5);
    std::fs::write(&save, decrypted_save(&fixture).expect("saved")).expect("save");
    let mut application = fixture_application(
        &root,
        FakeLiveAddExecutor::new(fixture),
        FakeSaveBackup::new(&root),
    );
    let record = assembly_record(0x1E82, 0x0BAD, 4);
    let payload = candidate_payload(
        CONTEXT_DIGEST,
        0x0BAD,
        4,
        &record,
        None,
        CandidateStage::FinalRecord,
    )
    .expect("payload");
    let error = application
        .prepare(&payload, &save, None)
        .expect_err("a taken serial is refused");
    assert!(error.message().contains("Native serial index differs"));
}
