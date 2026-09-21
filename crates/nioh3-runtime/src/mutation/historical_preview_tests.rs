//! Acceptance coverage for the one-time historical preview classification.
//!
//! Every fixture and every evidence file is synthetic, and each authorization is
//! built from the fixture's own hashes. The tests reuse the case's non-secret
//! identifiers (the operation, parent and candidate UUIDs, the pid and the
//! process birth) so the shape matches the real case; they carry no real receipt
//! hash, no real evidence hash and no force flag. The real pinned values live
//! only in the operator's authorization document.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use crate::mutation::count::sha256_hex;
use crate::mutation::historical_preview::{
    classification_is_terminal, classify_historical_preview, parse_authorization,
    read_classification, valid_classification, verify_historical_preview,
    HistoricalPreviewEvidencePaths, AUTHORIZATION_SCHEMA, CLASSIFICATION_ACTION,
    CLASSIFICATION_CLAIM, CLASSIFICATION_LIMITS,
};
use crate::mutation::native_executor::ReceiptStore;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const PID: u32 = 40936;
const CREATION: &str = "134344509389235036";
const OPERATION: &str = "178625a6-1f46-4edc-9c45-6e61f6b7f38f";
const PARENT: &str = "f27a475f-4603-4150-bdd0-73d92d39bbec";
const CANDIDATE: &str = "59985fc8ace4ea907cae4ffbdc26c42954c0d416c3a1da663b44b00dd0286ecf";
const ERROR: &str = "Native builder output differs from reviewed record";
const CONTAINER: &str = "c61b6b93b92d02595b470f50e9fababd58f4f63d7a2628d328f66df0f124e7d3";

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "nioh3-historical-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("state").join("live-add").join("native-executor"))
        .expect("store directory");
    root
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec(value).expect("json")).expect("write");
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The synthetic native receipt: a terminal-cleanup, non-allocating preview whose
/// return frame and recorded mismatch satisfy the shipped proof.
fn receipt() -> Value {
    let frame = crate::mutation::native_fakes::frame_for(0x1000, 0x1000);
    let mut expected = vec![0x11u8; 0xE8];
    expected[0] = 0x82;
    let mut source = expected.clone();
    source[0x28..0x30].copy_from_slice(&[0xFF; 8]);
    source[0x20] ^= 0xFF;
    json!({
        "operation_id": OPERATION,
        "parent_operation_id": PARENT,
        "candidate_id": CANDIDATE,
        "pid": PID,
        "process_creation_time": CREATION,
        "mode": "preview",
        "executor": "windows-native",
        "phase": "uncertain",
        "business_outcome": "unknown",
        "error": ERROR,
        "redirect_count": 1,
        "breakpoint_count": 0,
        "released": true,
        "active": false,
        "allocation": Value::Null,
        "allocation_state": "freed",
        "debugger_state": "detached",
        "remote_execution": "quiescent",
        "serial": Value::Null,
        "slot": Value::Null,
        "status": 0,
        "expected_record_hex": hex(&expected),
        "source_hex": hex(&source),
        "before": frame["before"].clone(),
        "after": frame["after"].clone(),
        "thread_cleanup": {
            "10168:163": {"cleanup_state": "original_restored", "error": null},
            "10380:87": {"cleanup_state": "exited", "error": null},
        },
    })
}

fn records() -> Value {
    json!([
        {"slot_index": 0, "serial": "2253799", "seed": 170827512, "record_hex": "04e6"},
        {"slot_index": 4, "serial": "2253801", "seed": 170827514, "record_hex": "04e7"},
    ])
}

fn index_mapping() -> Value {
    json!([{"serial": "2253799", "slot": 0}, {"serial": "2253801", "slot": 4}])
}

fn snapshot(serial_counter: &str, entries: Value, index_entries: Value) -> Value {
    json!({
        "inventory": {
            "pid": PID,
            "process_creation_time": CREATION,
            "capacity": 400,
            "game_version": "PC v2.02",
            "entries": entries,
            "duplicate_scroll_serials": [],
            "serial_counter": serial_counter,
            "acquisition_order_counter": 51151,
            "container_sha256": CONTAINER,
        },
        "index": {
            "schema": "nioh3-native-serial-index/v1",
            "pid": PID,
            "process_creation_time": CREATION,
            "node_count": index_entries.as_array().map(Vec::len).unwrap_or(0),
            "bucket_count": 4096,
            "entries": index_entries,
        },
    })
}

/// One complete, valid fixture: the store, the receipt, the five evidence files
/// and the authorization document the tests may mutate.
struct Fixture {
    root: PathBuf,
    store: ReceiptStore,
    paths: HistoricalPreviewEvidencePaths,
    authorization: Value,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = scratch(name);
        let directory = root.join("state").join("live-add").join("native-executor");
        let store = ReceiptStore::new(&directory).expect("store");
        let receipt_bytes = serde_json::to_vec(&receipt()).expect("receipt json");
        std::fs::write(store.path(OPERATION), &receipt_bytes).expect("receipt");
        let evidence = root.join("evidence");
        std::fs::create_dir_all(&evidence).expect("evidence dir");
        let paths = HistoricalPreviewEvidencePaths {
            inventory_before: evidence.join("insert3-inventory-before.json"),
            inventory_after: evidence.join("insert3-inventory-after.json"),
            runner_report: evidence.join("insert3-report.json"),
            runner_request: evidence.join("insert3-request.json"),
            runner_stdout: evidence.join("insert3-stdout.txt"),
        };
        write_json(
            &paths.inventory_before,
            &snapshot("2501562", records(), index_mapping()),
        );
        write_json(
            &paths.inventory_after,
            &snapshot("2501562", records(), index_mapping()),
        );
        write_json(
            &paths.runner_request,
            &json!({
                "pid": PID,
                "process_creation_time": CREATION,
                "state_root": root.join("state").display().to_string(),
                "inventory_before": paths.inventory_before.display().to_string(),
                "inventory_after": paths.inventory_after.display().to_string(),
                "report": paths.runner_report.display().to_string(),
                "arm": "insert",
            }),
        );
        write_json(
            &paths.runner_report,
            &json!({
                "pid": PID,
                "creation_filetime": CREATION,
                "failure": ERROR,
                "records_added": 0,
                "serial_advanced": false,
                "execution": Value::Null,
                "receipt_settled": false,
                "before_container_sha256": CONTAINER,
                "after_container_sha256": CONTAINER,
            }),
        );
        std::fs::write(&paths.runner_stdout, b"pid\t40936\n").expect("stdout");
        let digest = |path: &Path| sha256_hex(&std::fs::read(path).expect("evidence bytes"));
        let authorization = json!({
            "schema": AUTHORIZATION_SCHEMA,
            "action": CLASSIFICATION_ACTION,
            "claim": CLASSIFICATION_CLAIM,
            "limits": CLASSIFICATION_LIMITS,
            "operation_id": OPERATION,
            "receipt_sha256": sha256_hex(&receipt_bytes),
            "receipt_bytes": receipt_bytes.len(),
            "pid": PID,
            "process_creation_time": CREATION,
            "parent_operation_id": PARENT,
            "candidate_id": CANDIDATE,
            "expected_error": ERROR,
            "evidence": {
                "inventory_before_sha256": digest(&paths.inventory_before),
                "inventory_after_sha256": digest(&paths.inventory_after),
                "runner_report_sha256": digest(&paths.runner_report),
                "runner_request_sha256": digest(&paths.runner_request),
                "runner_stdout_sha256": digest(&paths.runner_stdout),
            },
        });
        Fixture {
            root,
            store,
            paths,
            authorization,
        }
    }

    fn parsed(&self) -> crate::mutation::historical_preview::HistoricalPreviewAuthorization {
        parse_authorization(&self.authorization).expect("authorization")
    }

    fn classified(&self) -> Value {
        classify_historical_preview(&self.store, &self.parsed(), &self.paths).expect("classify")
    }

    fn receipt_bytes(&self) -> Vec<u8> {
        std::fs::read(self.store.path(OPERATION)).expect("receipt bytes")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn a_valid_historical_fixture_classifies_without_touching_the_receipt() {
    let fixture = Fixture::new("valid");
    let before_bytes = fixture.receipt_bytes();
    let proposed =
        verify_historical_preview(&fixture.store, &fixture.parsed(), &fixture.paths).expect("dry run");
    assert!(classification_is_terminal(&proposed), "{proposed}");
    assert_eq!(proposed["claim"], CLASSIFICATION_CLAIM);
    assert!(
        !fixture.store.classification_path(OPERATION).exists(),
        "a dry run writes nothing"
    );
    assert_eq!(
        fixture.store.unresolved_owner().expect("owner"),
        Some(OPERATION.to_string()),
        "the fence holds before the decision"
    );
    let stored = fixture.classified();
    assert_eq!(stored["state"], "historical_preview_rejected");
    assert_eq!(stored["business_outcome"], "rejected");
    assert_eq!(
        stored["evidence_kind"], "historical_external_snapshots_not_a_native_durable_baseline",
        "the record never claims a native baseline"
    );
    assert_eq!(fixture.receipt_bytes(), before_bytes, "the receipt is immutable");
    assert_eq!(
        fixture.store.unresolved_owner().expect("owner"),
        None,
        "the classified operation no longer owns the target"
    );
    assert!(valid_classification(&fixture.store, OPERATION)
        .expect("valid")
        .is_some());
    let raw = fixture.store.read(OPERATION).expect("raw receipt");
    assert_eq!(
        fixture.store.authoritative_state(&raw).expect("state"),
        Some(stored.clone()),
        "the store's one authoritative reading returns the classification"
    );
}

#[test]
fn the_classified_operation_is_never_replayable() {
    let fixture = Fixture::new("no-replay");
    fixture.classified();
    assert!(fixture.store.exists(OPERATION), "the receipt stays on disk");
    let first = std::fs::read(fixture.store.classification_path(OPERATION)).expect("record");
    let again = fixture.classified();
    assert!(classification_is_terminal(&again));
    assert_eq!(
        std::fs::read(fixture.store.classification_path(OPERATION)).expect("record"),
        first,
        "an identical decision rewrites nothing"
    );
}

#[test]
fn wrong_identity_and_hash_evidence_is_refused() {
    let fixture = Fixture::new("wrong-identity");
    let mut wrong_hash = fixture.authorization.clone();
    wrong_hash["receipt_sha256"] = json!("00".repeat(32));
    let parsed = parse_authorization(&wrong_hash).expect("shape");
    assert!(verify_historical_preview(&fixture.store, &parsed, &fixture.paths).is_err());
    let mut wrong_uuid = fixture.authorization.clone();
    wrong_uuid["operation_id"] = json!("11111111-1111-4111-8111-111111111111");
    let parsed = parse_authorization(&wrong_uuid).expect("shape");
    assert!(verify_historical_preview(&fixture.store, &parsed, &fixture.paths).is_err());
    let mut wrong_pid = fixture.authorization.clone();
    wrong_pid["pid"] = json!(1);
    let parsed = parse_authorization(&wrong_pid).expect("shape");
    assert!(verify_historical_preview(&fixture.store, &parsed, &fixture.paths).is_err());
}

#[test]
fn a_mutated_receipt_voids_a_recorded_classification() {
    let fixture = Fixture::new("mutated");
    fixture.classified();
    let mut receipt = fixture.store.read(OPERATION).expect("receipt");
    receipt["error"] = json!("something else");
    std::fs::write(
        fixture.store.path(OPERATION),
        serde_json::to_vec(&receipt).expect("json"),
    )
    .expect("rewrite");
    let error = valid_classification(&fixture.store, OPERATION).expect_err("binding");
    assert_eq!(error.code(), "RECEIPT_CONFLICT", "{error:?}");
    assert!(
        fixture.store.unresolved_owner().is_err(),
        "a record that no longer binds fails closed; it cannot release the fence"
    );
}

#[test]
fn missing_evidence_is_refused() {
    let fixture = Fixture::new("missing-evidence");
    std::fs::remove_file(&fixture.paths.inventory_after).expect("remove");
    assert!(verify_historical_preview(&fixture.store, &fixture.parsed(), &fixture.paths).is_err());
    assert!(classify_historical_preview(&fixture.store, &fixture.parsed(), &fixture.paths).is_err());
    assert_eq!(
        fixture.store.unresolved_owner().expect("owner"),
        Some(OPERATION.to_string()),
        "a refused classification keeps the fence"
    );
}

#[test]
fn a_changed_index_or_counter_is_refused() {
    let fixture = Fixture::new("changed-index");
    let mut moved = index_mapping();
    moved[0]["slot"] = json!(7);
    write_json(
        &fixture.paths.inventory_after,
        &snapshot("2501562", records(), moved),
    );
    assert!(verify_historical_preview(&fixture.store, &fixture.parsed(), &fixture.paths).is_err());
    let fixture = Fixture::new("changed-counter");
    write_json(
        &fixture.paths.inventory_after,
        &snapshot("2501563", records(), index_mapping()),
    );
    assert!(verify_historical_preview(&fixture.store, &fixture.parsed(), &fixture.paths).is_err());
}

#[test]
fn an_unresolved_cleanup_or_a_non_preview_receipt_is_refused() {
    let fixture = Fixture::new("cleanup-unknown");
    let mut receipt = fixture.store.read(OPERATION).expect("receipt");
    receipt["thread_cleanup"] = json!({
        "10168:163": {"cleanup_state": "armed", "error": null},
    });
    let bytes = serde_json::to_vec(&receipt).expect("json");
    std::fs::write(fixture.store.path(OPERATION), &bytes).expect("write");
    let mut authorization = fixture.authorization.clone();
    authorization["receipt_sha256"] = json!(sha256_hex(&bytes));
    authorization["receipt_bytes"] = json!(bytes.len());
    let parsed = parse_authorization(&authorization).expect("shape");
    assert!(verify_historical_preview(&fixture.store, &parsed, &fixture.paths).is_err());
    let fixture = Fixture::new("not-preview");
    let mut receipt = fixture.store.read(OPERATION).expect("receipt");
    receipt["mode"] = json!("single_native_insertion");
    let bytes = serde_json::to_vec(&receipt).expect("json");
    std::fs::write(fixture.store.path(OPERATION), &bytes).expect("write");
    let mut authorization = fixture.authorization.clone();
    authorization["receipt_sha256"] = json!(sha256_hex(&bytes));
    authorization["receipt_bytes"] = json!(bytes.len());
    let parsed = parse_authorization(&authorization).expect("shape");
    assert!(verify_historical_preview(&fixture.store, &parsed, &fixture.paths).is_err());
}

#[test]
fn an_unauthorized_or_widened_request_is_refused() {
    let fixture = Fixture::new("no-authorization");
    let mut widened = fixture.authorization.clone();
    widened["force"] = json!(true);
    assert!(parse_authorization(&widened).is_err(), "no force flag");
    let mut wrong_action = fixture.authorization.clone();
    wrong_action["action"] = json!("classify_anything");
    assert!(parse_authorization(&wrong_action).is_err());
    let mut wrong_claim = fixture.authorization.clone();
    wrong_claim["claim"] = json!("no side effects occurred anywhere");
    assert!(parse_authorization(&wrong_claim).is_err());
    let mut wrong_limits = fixture.authorization.clone();
    wrong_limits["limits"] = json!(["anything_goes"]);
    assert!(parse_authorization(&wrong_limits).is_err());
    let mut missing_action = fixture.authorization.clone();
    missing_action
        .as_object_mut()
        .expect("object")
        .remove("action");
    assert!(parse_authorization(&missing_action).is_err());
    assert_eq!(
        fixture.store.unresolved_owner().expect("owner"),
        Some(OPERATION.to_string()),
        "nothing above reached the store"
    );
}

#[test]
fn a_different_decision_for_the_same_receipt_is_a_conflict() {
    let fixture = Fixture::new("conflict");
    let mut record = fixture.classified();
    // A structurally valid record that binds to different evidence: the same
    // receipt may not carry two different decisions.
    record["evidence"]["runner_stdout_sha256"] = json!("11".repeat(32));
    std::fs::write(
        fixture.store.classification_path(OPERATION),
        serde_json::to_vec(&record).expect("json"),
    )
    .expect("write");
    assert!(classification_is_terminal(&record));
    let error = classify_historical_preview(&fixture.store, &fixture.parsed(), &fixture.paths)
        .expect_err("a second, different decision");
    assert_eq!(error.code(), "RECEIPT_CONFLICT", "{error:?}");
    assert_eq!(
        fixture
            .store
            .read(OPERATION)
            .expect("receipt")
            .get("operation_id")
            .and_then(Value::as_str),
        Some(OPERATION),
        "the original receipt is untouched by the conflict"
    );
}

#[test]
fn a_malformed_or_extra_sidecar_is_an_error_not_a_hidden_owner() {
    let fixture = Fixture::new("malformed");
    std::fs::write(fixture.store.classification_path(OPERATION), b"{ not json").expect("write");
    assert_eq!(
        read_classification(&fixture.store, OPERATION)
            .expect_err("malformed")
            .code(),
        "RECEIPT_CONFLICT"
    );
    assert!(
        fixture.store.unresolved_owner().is_err(),
        "a malformed sidecar fails closed, it does not release the fence"
    );
    let fixture = Fixture::new("extra-key");
    let mut record = fixture.classified();
    record["unexpected"] = json!(true);
    std::fs::write(
        fixture.store.classification_path(OPERATION),
        serde_json::to_vec(&record).expect("json"),
    )
    .expect("write");
    assert!(!classification_is_terminal(&record));
    assert_eq!(
        valid_classification(&fixture.store, OPERATION)
            .expect_err("extra key")
            .code(),
        "RECEIPT_CONFLICT"
    );
}

#[test]
fn a_failed_persist_keeps_the_fence() {
    let fixture = Fixture::new("atomic");
    std::fs::create_dir_all(fixture.store.classification_path(OPERATION)).expect("block");
    assert!(classify_historical_preview(&fixture.store, &fixture.parsed(), &fixture.paths).is_err());
    assert_eq!(
        fixture.store.unresolved_owner().expect("owner"),
        Some(OPERATION.to_string()),
        "an atomic failure leaves the operation blocked"
    );
}
