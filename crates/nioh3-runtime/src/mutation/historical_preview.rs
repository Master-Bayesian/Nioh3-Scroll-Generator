//! One-time, explicitly restricted classification of a single historical
//! preview receipt.
//!
//! This is not a migration framework and not a general unlock. It exists for the
//! one receipt whose preview ran before the durable pre-dispatch baseline
//! existed, whose native record therefore stays `unknown`/unsettled, and whose
//! proof lives in same-run external snapshots plus the receipt's own terminal
//! cleanup. Everything it writes is a sidecar beside the receipt: the original
//! receipt bytes are never edited, moved or deleted.
//!
//! The classification is a *limited* decision. It may never claim that the
//! builder did not run, that a native pre-dispatch inventory baseline exists, or
//! that no engine-side effect occurred anywhere. It records exactly what the raw
//! evidence supports.

use crate::error::RuntimeError;
use crate::mutation::count::{
    canonical_json, is_canonical_uuid, new_operation_id, read_bytes, sha256_hex,
};
use crate::mutation::evidence::verify_dispatch_evidence;
use crate::mutation::inventory::{hex_decode, index_entries, RECORD_SIZE, SERIAL_OFFSET};
use crate::mutation::native_executor::{cleanup_state_complete, AdmissionLock, ReceiptStore};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The authorization document's schema. One schema, one decision.
pub const AUTHORIZATION_SCHEMA: &str =
    "nioh3-historical-preview-classification-authorization/v1";
/// The classification record's schema. Distinct from the native receipt's own
/// shape on purpose: this is a historical decision, not a native settlement.
pub const CLASSIFICATION_SCHEMA: &str = "nioh3-historical-preview-classification/v1";
/// The one decision an authorization may name.
pub const CLASSIFICATION_ACTION: &str = "classify_historical_preview_rejected";
/// The one terminal state the classification may project.
pub const CLASSIFICATION_STATE: &str = "historical_preview_rejected";

/// The bounded claim the evidence supports, verbatim.
pub const CLASSIFICATION_CLAIM: &str = "preview rejected at builder-output review gate; no change observed in the read-only container/serial/acquisition/index scope between the same-run before/after snapshots; the durable receipt records terminal cleanup with no retained allocation, debugger or thread owner";

/// What this classification explicitly does not claim.
pub const CLASSIFICATION_LIMITS: [&str; 4] = [
    "no_engine_side_effect_proof",
    "no_native_pre_dispatch_inventory_baseline",
    "builder_executed_in_scratch_memory",
    "no_claim_that_the_dispatch_never_ran",
];

const INDEX_SCHEMA: &str = "nioh3-native-serial-index/v1";

/// The five external evidence hashes one authorization pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalPreviewEvidenceHashes {
    pub inventory_before_sha256: String,
    pub inventory_after_sha256: String,
    pub runner_report_sha256: String,
    pub runner_request_sha256: String,
    pub runner_stdout_sha256: String,
}

/// The five external evidence files the verifier reads. Paths are operator input
/// and are never trusted from the authorization, which pins only hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalPreviewEvidencePaths {
    pub inventory_before: PathBuf,
    pub inventory_after: PathBuf,
    pub runner_report: PathBuf,
    pub runner_request: PathBuf,
    pub runner_stdout: PathBuf,
}

/// One explicit, narrowly restricted authorization to classify one receipt.
///
/// There is no wildcard and no force flag: the document names one decision, one
/// UUID, one raw receipt hash and the exact evidence hashes. Anything else is
/// refused before a single target file is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalPreviewAuthorization {
    pub operation_id: String,
    pub receipt_sha256: String,
    pub receipt_bytes: u64,
    pub pid: u32,
    pub process_creation_time: String,
    pub parent_operation_id: String,
    pub candidate_id: String,
    pub expected_error: String,
    pub evidence: HistoricalPreviewEvidenceHashes,
}

fn refused(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::LiveAddVerification {
        detail: detail.into(),
    }
}

fn conflicted(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::ReceiptConflict {
        detail: detail.into(),
    }
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

fn text<'a>(value: &'a Value, key: &str, what: &str) -> Result<&'a str, RuntimeError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| refused(format!("{what} needs a non-empty {key}")))
}

fn u64_field(value: &Value, key: &str, what: &str) -> Result<u64, RuntimeError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| refused(format!("{what} needs a numeric {key}")))
}

fn object<'a>(value: &'a Value, key: &str, what: &str) -> Result<&'a Value, RuntimeError> {
    value
        .get(key)
        .filter(|entry| entry.is_object())
        .ok_or_else(|| refused(format!("{what} needs an object {key}")))
}

fn json_value(path: &Path, what: &str) -> Result<Value, RuntimeError> {
    let raw = read_bytes(path)?;
    serde_json::from_slice(&raw)
        .map_err(|error| refused(format!("{what} {} is not valid JSON: {error}", path.display())))
}

/// Parse one authorization document. Unknown keys are refused: an authorization
/// is an explicitly restricted request, not a bag of options.
pub fn parse_authorization(
    value: &Value,
) -> Result<HistoricalPreviewAuthorization, RuntimeError> {
    const ALLOWED: [&str; 13] = [
        "schema",
        "action",
        "claim",
        "limits",
        "operation_id",
        "receipt_sha256",
        "receipt_bytes",
        "pid",
        "process_creation_time",
        "parent_operation_id",
        "candidate_id",
        "expected_error",
        "evidence",
    ];
    if let Some(map) = value.as_object() {
        for key in map.keys() {
            if !ALLOWED.contains(&key.as_str()) {
                return Err(refused(format!(
                    "Authorization carries an unsupported key {key}"
                )));
            }
        }
    }
    if text(value, "schema", "Authorization")? != AUTHORIZATION_SCHEMA {
        return Err(refused("Authorization schema differs"));
    }
    if text(value, "action", "Authorization")? != CLASSIFICATION_ACTION {
        return Err(refused("Authorization names a different decision"));
    }
    if text(value, "claim", "Authorization")? != CLASSIFICATION_CLAIM {
        return Err(refused("Authorization carries a different claim"));
    }
    let limits = value
        .get("limits")
        .and_then(Value::as_array)
        .ok_or_else(|| refused("Authorization needs its limits"))?;
    let limits: Vec<&str> = limits.iter().filter_map(Value::as_str).collect();
    if limits.len() != CLASSIFICATION_LIMITS.len()
        || CLASSIFICATION_LIMITS
            .iter()
            .any(|limit| !limits.contains(limit))
    {
        return Err(refused(
            "Authorization limits differ from the supported set",
        ));
    }
    let operation_id = text(value, "operation_id", "Authorization")?.to_string();
    if !is_canonical_uuid(&operation_id) {
        return Err(refused("Authorization needs one canonical operation UUID"));
    }
    let receipt_sha256 = text(value, "receipt_sha256", "Authorization")?.to_string();
    if !is_sha256(&receipt_sha256) {
        return Err(refused("Authorization needs the raw receipt SHA-256"));
    }
    let parent_operation_id = text(value, "parent_operation_id", "Authorization")?.to_string();
    if !is_canonical_uuid(&parent_operation_id) {
        return Err(refused("Authorization needs the canonical parent UUID"));
    }
    let candidate_id = text(value, "candidate_id", "Authorization")?.to_string();
    if !is_sha256(&candidate_id) {
        return Err(refused(
            "Authorization needs the candidate identity digest",
        ));
    }
    let evidence_value = object(value, "evidence", "Authorization")?;
    let evidence = HistoricalPreviewEvidenceHashes {
        inventory_before_sha256: text(
            evidence_value,
            "inventory_before_sha256",
            "Authorization evidence",
        )?
        .to_string(),
        inventory_after_sha256: text(
            evidence_value,
            "inventory_after_sha256",
            "Authorization evidence",
        )?
        .to_string(),
        runner_report_sha256: text(
            evidence_value,
            "runner_report_sha256",
            "Authorization evidence",
        )?
        .to_string(),
        runner_request_sha256: text(
            evidence_value,
            "runner_request_sha256",
            "Authorization evidence",
        )?
        .to_string(),
        runner_stdout_sha256: text(
            evidence_value,
            "runner_stdout_sha256",
            "Authorization evidence",
        )?
        .to_string(),
    };
    for digest in [
        &evidence.inventory_before_sha256,
        &evidence.inventory_after_sha256,
        &evidence.runner_report_sha256,
        &evidence.runner_request_sha256,
        &evidence.runner_stdout_sha256,
    ] {
        if !is_sha256(digest) {
            return Err(refused(
                "Authorization evidence needs five raw SHA-256 digests",
            ));
        }
    }
    Ok(HistoricalPreviewAuthorization {
        operation_id,
        receipt_sha256,
        receipt_bytes: u64_field(value, "receipt_bytes", "Authorization")?,
        pid: u32::try_from(u64_field(value, "pid", "Authorization")?)
            .map_err(|_| refused("Authorization pid is out of range"))?,
        process_creation_time: text(value, "process_creation_time", "Authorization")?
            .to_string(),
        parent_operation_id,
        candidate_id,
        expected_error: text(value, "expected_error", "Authorization")?.to_string(),
        evidence,
    })
}

/// Whether one value is the store's authoritative historical classification.
///
/// A structural check on the record alone: schema, the single decision state,
/// the bounded claim and limits, the receipt hash and the evidence hashes. The
/// store additionally re-binds it to the exact bytes on disk before a consumer
/// acts on it.
pub fn classification_is_terminal(value: &Value) -> bool {
    const KEYS: [&str; 17] = [
        "schema",
        "state",
        "decision",
        "business_outcome",
        "review",
        "inventory_effect",
        "claim",
        "limits",
        "operation_id",
        "parent_operation_id",
        "candidate_id",
        "receipt_sha256",
        "receipt_bytes",
        "identity",
        "evidence",
        "same_run",
        "evidence_kind",
    ];
    // An unknown key is a malformed sidecar, not a richer valid one.
    if !value.as_object().is_some_and(|map| {
        map.keys().all(|key| KEYS.contains(&key.as_str()))
    }) {
        return false;
    }
    let bounded = |limits: &Value| {
        limits.as_array().is_some_and(|limits| {
            let limits: Vec<&str> = limits.iter().filter_map(Value::as_str).collect();
            limits.len() == CLASSIFICATION_LIMITS.len()
                && CLASSIFICATION_LIMITS
                    .iter()
                    .all(|limit| limits.contains(limit))
        })
    };
    value.get("schema").and_then(Value::as_str) == Some(CLASSIFICATION_SCHEMA)
        && value.get("state").and_then(Value::as_str) == Some(CLASSIFICATION_STATE)
        && value.get("decision").and_then(Value::as_str) == Some(CLASSIFICATION_ACTION)
        && value.get("business_outcome").and_then(Value::as_str) == Some("rejected")
        && value.get("review").and_then(Value::as_str) == Some("mismatch")
        && value.get("inventory_effect").and_then(Value::as_str)
            == Some("unchanged_with_historical_external_evidence")
        && value.get("claim").and_then(Value::as_str) == Some(CLASSIFICATION_CLAIM)
        && value
            .get("operation_id")
            .and_then(Value::as_str)
            .is_some_and(is_canonical_uuid)
        && value
            .get("receipt_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
        && value.get("receipt_bytes").and_then(Value::as_u64).is_some()
        && value.get("limits").is_some_and(&bounded)
        && value.get("evidence").is_some_and(|evidence| {
            [
                "inventory_before_sha256",
                "inventory_after_sha256",
                "runner_report_sha256",
                "runner_request_sha256",
                "runner_stdout_sha256",
            ]
            .iter()
            .all(|key| {
                evidence
                    .get(*key)
                    .and_then(Value::as_str)
                    .is_some_and(is_sha256)
            })
        })
        && value.get("identity").is_some_and(|identity| {
            identity.get("pid").and_then(Value::as_u64).is_some()
                && identity
                    .get("process_creation_time")
                    .and_then(Value::as_str)
                    .is_some_and(|entry| !entry.is_empty())
                && identity.get("mode").and_then(Value::as_str) == Some("preview")
                && identity.get("redirect_count").and_then(Value::as_u64) == Some(1)
        })
}

/// The binding-relevant fields two classifications must share. The record's own
/// timestamp is deliberately excluded, so repeating the same decision is
/// idempotent instead of a conflict.
fn classification_binds_same(existing: &Value, proposed: &Value) -> bool {
    const BINDING: [&str; 10] = [
        "schema",
        "state",
        "decision",
        "operation_id",
        "receipt_sha256",
        "receipt_bytes",
        "claim",
        "limits",
        "evidence",
        "identity",
    ];
    BINDING
        .iter()
        .all(|key| existing.get(*key) == proposed.get(*key))
}

/// Read the classification sidecar, if one exists, without validating it.
pub fn read_classification(
    store: &ReceiptStore,
    operation_id: &str,
) -> Result<Option<Value>, RuntimeError> {
    if !is_canonical_uuid(operation_id) {
        return Err(conflicted("Use a canonical operation UUID"));
    }
    let path = store.classification_path(operation_id);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = read_bytes(&path)?;
    let value: Value = serde_json::from_slice(&raw).map_err(|error| {
        conflicted(format!(
            "Historical classification {} is not valid JSON: {error}",
            path.display()
        ))
    })?;
    Ok(Some(value))
}

/// The classification for one receipt, only while it still binds to the exact
/// bytes on disk and to the receipt's own identity.
///
/// A sidecar that exists but is malformed, mis-labelled, or bound to different
/// bytes is an error: it is never hidden as "not classified".
pub fn valid_classification(
    store: &ReceiptStore,
    operation_id: &str,
) -> Result<Option<Value>, RuntimeError> {
    let Some(value) = read_classification(store, operation_id)? else {
        return Ok(None);
    };
    if !classification_is_terminal(&value) {
        return Err(conflicted(format!(
            "Historical classification for {operation_id} is not a valid terminal record"
        )));
    }
    if value.get("operation_id").and_then(Value::as_str) != Some(operation_id) {
        return Err(conflicted(
            "Historical classification names a different operation",
        ));
    }
    let receipt_path = store.path(operation_id);
    if !receipt_path.is_file() {
        return Err(conflicted(
            "Historical classification has no receipt to bind to",
        ));
    }
    let raw = read_bytes(&receipt_path)?;
    let bound = value
        .get("receipt_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if sha256_hex(&raw) != bound {
        return Err(conflicted(
            "Historical classification no longer binds to the receipt bytes",
        ));
    }
    let receipt: Value = serde_json::from_slice(&raw).map_err(|error| {
        conflicted(format!(
            "Historical classification receipt {} is not valid JSON: {error}",
            receipt_path.display()
        ))
    })?;
    let identity = value.get("identity").unwrap_or(&Value::Null);
    if receipt.get("operation_id") != value.get("operation_id")
        || receipt.get("pid") != identity.get("pid")
        || receipt.get("process_creation_time") != identity.get("process_creation_time")
        || receipt.get("mode").and_then(Value::as_str) != Some("preview")
        || receipt.get("redirect_count").and_then(Value::as_u64) != Some(1)
    {
        return Err(conflicted(
            "Historical classification identity no longer matches its receipt",
        ));
    }
    Ok(Some(value))
}

/// Read one evidence file and prove its authorized raw bytes.
fn evidence_bytes(
    path: &Path,
    expected: &str,
    what: &str,
) -> Result<Vec<u8>, RuntimeError> {
    let raw = read_bytes(path).map_err(|error| {
        refused(format!(
            "{what} evidence is missing or unreadable ({}): {error}",
            path.display()
        ))
    })?;
    if sha256_hex(&raw) != expected {
        return Err(refused(format!(
            "{what} evidence does not match its authorized SHA-256"
        )));
    }
    Ok(raw)
}

fn records(value: &Value) -> Result<Vec<Value>, RuntimeError> {
    value
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| refused("the snapshot lacks its entries"))
}

fn index_mapping(value: &Value) -> Result<BTreeMap<String, u32>, RuntimeError> {
    index_entries(value).map_err(|error| refused(format!("index mapping: {error}")))
}

/// The same-run inventory equality one historical classification depends on: the
/// same process lifetime, the full record set, both counters, the container and
/// the canonical `serial -> slot` mapping.
fn verify_same_run_inventory(
    before: &Value,
    after: &Value,
    authorization: &HistoricalPreviewAuthorization,
) -> Result<Value, RuntimeError> {
    let before_inventory = object(before, "inventory", "before snapshot")?;
    let after_inventory = object(after, "inventory", "after snapshot")?;
    let before_index = object(before, "index", "before snapshot")?;
    let after_index = object(after, "index", "after snapshot")?;
    for (label, snapshot) in [("before", before_inventory), ("after", after_inventory)] {
        if u64_field(snapshot, "pid", label)?.try_into().ok() != Some(authorization.pid) {
            return Err(refused(format!(
                "the {label} snapshot belongs to another process"
            )));
        }
        if text(snapshot, "process_creation_time", label)?
            != authorization.process_creation_time
        {
            return Err(refused(format!(
                "the {label} snapshot belongs to another process lifetime"
            )));
        }
    }
    let before_records = records(before_inventory)?;
    let after_records = records(after_inventory)?;
    if before_records.is_empty() {
        return Err(refused("the before snapshot has no records"));
    }
    if before_records != after_records {
        return Err(refused("the same-run before/after records differ"));
    }
    let before_serial = text(before_inventory, "serial_counter", "before snapshot")?;
    if before_serial != text(after_inventory, "serial_counter", "after snapshot")? {
        return Err(refused("the serial counter changed across the run"));
    }
    let before_acquisition =
        u64_field(before_inventory, "acquisition_order_counter", "before snapshot")?;
    if before_acquisition
        != u64_field(after_inventory, "acquisition_order_counter", "after snapshot")?
    {
        return Err(refused("the acquisition counter changed across the run"));
    }
    let before_container = text(before_inventory, "container_sha256", "before snapshot")?;
    if !is_sha256(before_container)
        || before_container != text(after_inventory, "container_sha256", "after snapshot")?
    {
        return Err(refused("the container digest changed across the run"));
    }
    if before_inventory
        .get("duplicate_scroll_serials")
        .and_then(Value::as_array)
        .map(Vec::is_empty)
        != Some(true)
    {
        return Err(refused("the before snapshot carries duplicate serials"));
    }
    if text(before_index, "schema", "before index")? != INDEX_SCHEMA {
        return Err(refused("the before snapshot index schema differs"));
    }
    let before_nodes = u64_field(before_index, "node_count", "before index")?;
    let after_nodes = u64_field(after_index, "node_count", "after index")?;
    let before_buckets = u64_field(before_index, "bucket_count", "before index")?;
    if before_nodes != after_nodes
        || before_buckets != u64_field(after_index, "bucket_count", "after index")?
    {
        return Err(refused("the index shape changed across the run"));
    }
    let mapping = index_mapping(before_index)?;
    if mapping != index_mapping(after_index)? {
        return Err(refused("the canonical index mapping changed across the run"));
    }
    let mapping_digest = sha256_hex(canonical_json(&json!(mapping)).as_bytes());
    Ok(json!({
        "entries": before_records.len(),
        "serial_counter": before_serial,
        "acquisition_order_counter": before_acquisition,
        "container_sha256": before_container,
        "index_node_count": before_nodes,
        "index_bucket_count": before_buckets,
        "canonical_index_mapping_sha256": mapping_digest,
    }))
}

/// Verify the external runner's own record of the same attempt.
fn verify_runner_record(
    request: &Value,
    report: &Value,
    paths: &HistoricalPreviewEvidencePaths,
    store: &ReceiptStore,
    authorization: &HistoricalPreviewAuthorization,
) -> Result<(), RuntimeError> {
    if u64_field(request, "pid", "runner request")? != u64::from(authorization.pid)
        || text(request, "process_creation_time", "runner request")?
            != authorization.process_creation_time
    {
        return Err(refused(
            "the runner request names another process lifetime",
        ));
    }
    let store_root = store
        .directory
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| refused("the receipt store has no state root"))?;
    if Path::new(text(request, "state_root", "runner request")?) != store_root {
        return Err(refused("the runner request names a different state root"));
    }
    for (key, expected) in [
        ("inventory_before", &paths.inventory_before),
        ("inventory_after", &paths.inventory_after),
        ("report", &paths.runner_report),
    ] {
        if Path::new(text(request, key, "runner request")?) != expected.as_path() {
            return Err(refused(format!(
                "the runner request does not bind the {key} evidence file"
            )));
        }
    }
    if u64_field(report, "pid", "runner report")?.try_into().ok() != Some(authorization.pid)
        || text(report, "creation_filetime", "runner report")?
            != authorization.process_creation_time
    {
        return Err(refused("the runner report names another process lifetime"));
    }
    if text(report, "failure", "runner report")? != authorization.expected_error {
        return Err(refused("the runner report records a different failure"));
    }
    if u64_field(report, "records_added", "runner report")? != 0
        || report.get("serial_advanced").and_then(Value::as_bool) != Some(false)
        || report.get("execution").is_some_and(|entry| !entry.is_null())
        || report.get("receipt_settled").and_then(Value::as_bool) != Some(false)
    {
        return Err(refused(
            "the runner report does not record an unexecuted, unsettled attempt",
        ));
    }
    if text(report, "before_container_sha256", "runner report")?
        != text(report, "after_container_sha256", "runner report")?
    {
        return Err(refused("the runner report records a container change"));
    }
    Ok(())
}

/// Verify the native receipt itself, from its raw bytes.
fn verify_receipt(
    raw: &[u8],
    authorization: &HistoricalPreviewAuthorization,
) -> Result<(), RuntimeError> {
    if raw.len() as u64 != authorization.receipt_bytes {
        return Err(refused("the receipt byte length differs"));
    }
    if sha256_hex(raw) != authorization.receipt_sha256 {
        return Err(refused(
            "the receipt bytes do not match the authorized SHA-256",
        ));
    }
    let receipt: Value = serde_json::from_slice(raw)
        .map_err(|error| refused(format!("the receipt is not valid JSON: {error}")))?;
    if receipt.get("operation_id").and_then(Value::as_str) != Some(&authorization.operation_id)
        || receipt.get("parent_operation_id").and_then(Value::as_str)
            != Some(&authorization.parent_operation_id)
        || receipt.get("candidate_id").and_then(Value::as_str)
            != Some(&authorization.candidate_id)
        || u64_field(&receipt, "pid", "receipt")? != u64::from(authorization.pid)
        || text(&receipt, "process_creation_time", "receipt")?
            != authorization.process_creation_time
    {
        return Err(refused(
            "the receipt identity differs from the authorization",
        ));
    }
    if text(&receipt, "mode", "receipt")? != "preview"
        || text(&receipt, "executor", "receipt")? != "windows-native"
    {
        return Err(refused("the receipt is not a native preview"));
    }
    if text(&receipt, "error", "receipt")? != authorization.expected_error {
        return Err(refused("the receipt records a different failure"));
    }
    if u64_field(&receipt, "redirect_count", "receipt")? != 1
        || receipt.get("breakpoint_count").and_then(Value::as_i64) != Some(0)
        || receipt.get("released").and_then(Value::as_bool) != Some(true)
        || receipt.get("active").and_then(Value::as_bool) != Some(false)
        || text(&receipt, "debugger_state", "receipt")? != "detached"
        || text(&receipt, "remote_execution", "receipt")? != "quiescent"
        || text(&receipt, "allocation_state", "receipt")? != "freed"
        || !receipt.get("allocation").is_some_and(Value::is_null)
    {
        return Err(refused("the receipt does not record a terminal cleanup"));
    }
    if !receipt.get("serial").is_some_and(Value::is_null)
        || !receipt.get("slot").is_some_and(Value::is_null)
    {
        return Err(refused("the receipt records a serial or a slot"));
    }
    if text(&receipt, "business_outcome", "receipt")? != "unknown"
        || text(&receipt, "phase", "receipt")? != "uncertain"
    {
        return Err(refused("the receipt is not the historical unsettled shape"));
    }
    // The dispatch return/source/register proof is re-derived from the receipt,
    // not taken from a caller flag or the runner's transcript.
    verify_dispatch_evidence(&receipt)
        .map_err(|error| refused(format!("the receipt return proof failed: {error}")))?;
    let expected = hex_decode(text(&receipt, "expected_record_hex", "receipt")?)
        .map_err(|error| refused(format!("the reviewed record is unreadable: {error}")))?;
    let source = hex_decode(text(&receipt, "source_hex", "receipt")?)
        .map_err(|error| refused(format!("the builder output is unreadable: {error}")))?;
    if expected.len() != RECORD_SIZE || source.len() != RECORD_SIZE {
        return Err(refused("the receipt record lengths differ"));
    }
    let mut sentinel = [0u8; 8];
    sentinel.copy_from_slice(&source[SERIAL_OFFSET..SERIAL_OFFSET + 8]);
    if sentinel != [0xFF; 8] {
        return Err(refused(
            "the preview output allocated or reused an instance serial",
        ));
    }
    if source == expected {
        return Err(refused(
            "the receipt records no builder-output difference",
        ));
    }
    let threads = receipt
        .get("thread_cleanup")
        .and_then(Value::as_object)
        .ok_or_else(|| refused("the receipt has no thread cleanup map"))?;
    if threads.is_empty() {
        return Err(refused("the receipt thread cleanup map is empty"));
    }
    for (name, thread) in threads {
        if !cleanup_state_complete(text(thread, "cleanup_state", "thread cleanup")?) {
            return Err(refused(format!("thread {name} is not terminally retired")));
        }
        if thread.get("error").is_some_and(|error| !error.is_null()) {
            return Err(refused(format!("thread {name} records a cleanup error")));
        }
    }
    Ok(())
}

/// Verify the authorization and the raw evidence without writing anything.
///
/// This is the dry run: it returns the exact classification record
/// [`classify_historical_preview`] would persist.
pub fn verify_historical_preview(
    store: &ReceiptStore,
    authorization: &HistoricalPreviewAuthorization,
    paths: &HistoricalPreviewEvidencePaths,
) -> Result<Value, RuntimeError> {
    let receipt_path = store.path(&authorization.operation_id);
    if !receipt_path.is_file() {
        return Err(refused(
            "the authorized receipt does not exist in this store",
        ));
    }
    let raw = read_bytes(&receipt_path)?;
    verify_receipt(&raw, authorization)?;

    let before_raw = evidence_bytes(
        &paths.inventory_before,
        &authorization.evidence.inventory_before_sha256,
        "before inventory",
    )?;
    let after_raw = evidence_bytes(
        &paths.inventory_after,
        &authorization.evidence.inventory_after_sha256,
        "after inventory",
    )?;
    evidence_bytes(
        &paths.runner_report,
        &authorization.evidence.runner_report_sha256,
        "runner report",
    )?;
    evidence_bytes(
        &paths.runner_request,
        &authorization.evidence.runner_request_sha256,
        "runner request",
    )?;
    evidence_bytes(
        &paths.runner_stdout,
        &authorization.evidence.runner_stdout_sha256,
        "runner transcript",
    )?;
    let before: Value = serde_json::from_slice(&before_raw)
        .map_err(|error| refused(format!("the before snapshot is not valid JSON: {error}")))?;
    let after: Value = serde_json::from_slice(&after_raw)
        .map_err(|error| refused(format!("the after snapshot is not valid JSON: {error}")))?;
    let runner_request = json_value(&paths.runner_request, "runner request")?;
    let runner_report = json_value(&paths.runner_report, "runner report")?;
    let same_run = verify_same_run_inventory(&before, &after, authorization)?;
    verify_runner_record(
        &runner_request,
        &runner_report,
        paths,
        store,
        authorization,
    )?;

    let record = json!({
        "schema": CLASSIFICATION_SCHEMA,
        "state": CLASSIFICATION_STATE,
        "decision": CLASSIFICATION_ACTION,
        "business_outcome": "rejected",
        "review": "mismatch",
        "inventory_effect": "unchanged_with_historical_external_evidence",
        "claim": CLASSIFICATION_CLAIM,
        "limits": CLASSIFICATION_LIMITS,
        "operation_id": authorization.operation_id,
        "parent_operation_id": authorization.parent_operation_id,
        "candidate_id": authorization.candidate_id,
        "receipt_sha256": authorization.receipt_sha256,
        "receipt_bytes": authorization.receipt_bytes,
        "identity": {
            "pid": authorization.pid,
            "process_creation_time": authorization.process_creation_time,
            "mode": "preview",
            "redirect_count": 1,
        },
        "evidence": {
            "inventory_before_sha256": authorization.evidence.inventory_before_sha256,
            "inventory_after_sha256": authorization.evidence.inventory_after_sha256,
            "runner_report_sha256": authorization.evidence.runner_report_sha256,
            "runner_request_sha256": authorization.evidence.runner_request_sha256,
            "runner_stdout_sha256": authorization.evidence.runner_stdout_sha256,
        },
        "same_run": same_run,
        "evidence_kind": "historical_external_snapshots_not_a_native_durable_baseline",
    });
    if !classification_is_terminal(&record) {
        return Err(refused(
            "the classification record is not self-consistent",
        ));
    }
    Ok(record)
}

/// Verify, then persist the one-time classification beside the receipt.
///
/// Idempotent: repeating the identical decision returns the stored record and
/// writes nothing. A different classification for the same receipt is a
/// conflict. The original receipt is never written.
pub fn classify_historical_preview(
    store: &ReceiptStore,
    authorization: &HistoricalPreviewAuthorization,
    paths: &HistoricalPreviewEvidencePaths,
) -> Result<Value, RuntimeError> {
    // The same admission serialization every native dispatch uses.
    let _admission = AdmissionLock::acquire(&store.directory)?;
    let proposed = verify_historical_preview(store, authorization, paths)?;
    if let Some(existing) = read_classification(store, &authorization.operation_id)? {
        if classification_binds_same(&existing, &proposed) {
            return Ok(existing);
        }
        return Err(conflicted(
            "Another historical classification is already recorded for this receipt",
        ));
    }
    let path = store.classification_path(&authorization.operation_id);
    let staging = store
        .directory
        .join(format!(".{}.classification.tmp", new_operation_id()?));
    let mut file = std::fs::File::create(&staging).map_err(|error| RuntimeError::Io {
        path: staging.display().to_string(),
        detail: error.to_string(),
    })?;
    file.write_all(canonical_json(&proposed).as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| RuntimeError::Io {
            path: staging.display().to_string(),
            detail: error.to_string(),
        })?;
    drop(file);
    std::fs::rename(&staging, &path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    // Confirm the record is readable and still binds before reporting it.
    let stored = valid_classification(store, &authorization.operation_id)?
        .ok_or_else(|| conflicted("the historical classification did not persist"))?;
    if !classification_binds_same(&stored, &proposed) {
        return Err(conflicted(
            "the persisted historical classification differs from the decision",
        ));
    }
    Ok(stored)
}
