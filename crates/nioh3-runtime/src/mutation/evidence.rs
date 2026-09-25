//! Independent verification of a preserved native dispatch and of live addition.
//!
//! Port of `dispatch_evidence.verify_dispatch` and `live_add_evidence.verify` /
//! `verify_persistence`. The point of both is that success is never published
//! from the receipt alone: the complete container, the native serial index, the
//! counters and the saved records are all re-derived and compared.

use crate::error::RuntimeError;
use crate::mutation::count::sha256_hex;
use crate::mutation::inventory::{
    hex_decode, index_entries, inventory_entries, Inventory, InventoryEntry, InventoryLayout,
    NativeIndex, RECORD_SIZE, SERIAL_OFFSET,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// `dispatch_evidence.REGISTERS`.
pub const REGISTERS: [&str; 15] = [
    "RAX", "RBX", "RCX", "RDX", "RSI", "RDI", "RBP", "R8", "R9", "R10", "R11", "R12", "R13", "R14",
    "R15",
];

/// `capture_phase` for the two fingerprints a live-add preview keeps: the
/// stopped owner before the redirect, and the acknowledged owner after it.
pub const PREVIEW_PHASE_BEFORE: &str = "stopped_before_redirect";
pub const PREVIEW_PHASE_AFTER: &str = "stopped_after_acknowledgement";

/// The phase and the settlement a preview mismatch leaves once every proof
/// exists. The phase is its own terminal word: the dispatch mechanics completed,
/// but the business result is a rejection, and neither is presented as the other.
pub const PREVIEW_PHASE_REJECTED_AFTER: &str = "rejected_after_preview";
pub const PREVIEW_SETTLEMENT_REJECTED: &str = "rejected_after_preview";

fn failed(detail: &str) -> RuntimeError {
    RuntimeError::LiveAddVerification {
        detail: detail.to_string(),
    }
}

fn field_u64(value: &Value, key: &str) -> Result<u64, RuntimeError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| failed("Insertion acknowledgement does not match the plan"))
}

fn text(value: &Value, key: &str, message: &str) -> Result<String, RuntimeError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| failed(message))
}

fn record_hex(value: &Value, key: &str, message: &str) -> Result<Vec<u8>, RuntimeError> {
    let text = text(value, key, message)?;
    let raw = hex_decode(&text).map_err(|_| failed(message))?;
    if raw.len() != RECORD_SIZE {
        return Err(failed(message));
    }
    Ok(raw)
}

/// The return, source and register proof of one acknowledged redirect.
///
/// This is [`verify_dispatch`] without its terminal-phase gate. A preview whose
/// dispatch completed and whose cleanup is terminal carries the same proof, so
/// a caller can verify it without rewriting the phase to satisfy a gate.
pub fn verify_dispatch_evidence(execution: &Value) -> Result<(), RuntimeError> {
    if execution.get("redirect_count").and_then(Value::as_u64) != Some(1) {
        return Err(failed("Exactly one acknowledged redirect is required"));
    }
    let released = execution.get("released").and_then(Value::as_bool) == Some(true);
    // The breakpoint axis is the same fact in two shapes: the stored receipt
    // carries `breakpoint_count`, and the adapter's normalized view derives the
    // empty `breakpoints` list from it. Either one proves the axis is clear.
    let breakpoints_clear = execution
        .get("breakpoints")
        .and_then(Value::as_array)
        .map(Vec::is_empty)
        == Some(true)
        || execution.get("breakpoint_count").and_then(Value::as_i64) == Some(0);
    if !released || !breakpoints_clear {
        return Err(failed("Allocation or breakpoint cleanup is not confirmed"));
    }
    let before = execution
        .get("before")
        .ok_or_else(|| failed("Exactly one acknowledged redirect is required"))?;
    let after = execution
        .get("after")
        .ok_or_else(|| failed("Exactly one acknowledged redirect is required"))?;
    for register in REGISTERS {
        if before.get(register) != after.get(register) {
            return Err(failed("A general register changed"));
        }
    }
    let before_rsp = field_u64(before, "RSP")?;
    let after_rsp = field_u64(after, "RSP")?;
    let before_rip = field_u64(before, "RIP")?;
    let after_rip = field_u64(after, "RIP")?;
    if before_rsp % 16 != 8 || after_rsp != before_rsp.wrapping_sub(0x48) {
        return Err(failed("Unexpected stack alignment or prologue delta"));
    }
    if after_rip != before_rip.wrapping_add(7) {
        return Err(failed(
            "Continuation is not the end of the replayed prologue",
        ));
    }
    // push rbx; push rdi; sub rsp,0x38. Only SUB changes arithmetic flags.
    let left = before_rsp.wrapping_sub(16);
    let result = left.wrapping_sub(0x38);
    let low = (result & 0xFF) as u8;
    let expected = u64::from(left < 0x38)
        | u64::from(low.count_ones().is_multiple_of(2)) << 2
        | u64::from((left ^ 0x38 ^ result) & 16 != 0) << 4
        | u64::from(result == 0) << 6
        | ((result >> 63) & 1) << 7
        | u64::from((left ^ 0x38) & (left ^ result) & (1u64 << 63) != 0) << 11;
    let before_eflags = field_u64(before, "EFLAGS")?;
    let after_eflags = field_u64(after, "EFLAGS")?;
    if after_eflags & 0x8D5 != expected {
        return Err(failed(
            "Arithmetic flags do not match the original prologue",
        ));
    }
    // Debugger RF/TF are not product flags and may differ at hardware stops.
    if (before_eflags ^ after_eflags) & !(0x8D5 | 0x1_0100) != 0 {
        return Err(failed("Non-arithmetic flags changed"));
    }
    Ok(())
}

/// Port of `dispatch_evidence.verify_dispatch`.
///
/// Exactly one acknowledged redirect, a fully released allocation, no remaining
/// breakpoint, identical general registers, the expected stack prologue and
/// arithmetic flags that match a `push/push/sub` replay, and a terminal phase.
pub fn verify_dispatch(execution: &Value) -> Result<(), RuntimeError> {
    if execution.get("phase").and_then(Value::as_str) != Some("completed") {
        return Err(failed("Exactly one acknowledged redirect is required"));
    }
    verify_dispatch_evidence(execution)
}

/// `live_add_evidence.defined`.
pub fn defined(raw: &[u8]) -> Vec<u8> {
    let mut result = raw[..0x24].to_vec();
    result.extend_from_slice(&raw[0x28..0xE4]);
    result
}

/// One preview inventory fingerprint over a raw container image, the two
/// counters and the *actual* native serial index, all read in the same stopped
/// owner.
///
/// The index digest comes from the native index's own `serial -> slot` entries,
/// never from the container. A container and an index can disagree - the same
/// records with a remapped index is exactly the case a container-only proof
/// misses - so a container-derived mapping is not independent index evidence.
pub fn preview_inventory_fingerprint(
    container: &[u8],
    serial_counter: u64,
    acquisition_order_counter: u32,
    layout: &InventoryLayout,
    native_index: &Value,
) -> Value {
    let mapping = index_entries(native_index).ok();
    let native_index_digest = mapping.as_ref().map(|mapping| {
        let canonical = mapping
            .iter()
            .map(|(serial, slot)| format!("{serial}:{slot}"))
            .collect::<Vec<_>>()
            .join("\n");
        sha256_hex(canonical.as_bytes())
    });
    json!({
        "container_sha256": sha256_hex(container),
        "capacity": layout.capacity,
        "record_size": layout.record_size,
        "serial_counter": serial_counter.to_string(),
        "acquisition_order_counter": acquisition_order_counter,
        "native_index_digest": native_index_digest,
        "index_node_count": native_index.get("node_count").and_then(Value::as_u64),
        "index_bucket_count": native_index.get("bucket_count").and_then(Value::as_u64),
    })
}

/// One fingerprint that names the exact owner, layout and code identity it was
/// read from.
#[allow(clippy::too_many_arguments)]
pub fn preview_owner_fingerprint(
    container: &[u8],
    serial_counter: u64,
    acquisition_order_counter: u32,
    layout: &InventoryLayout,
    profile_id: &str,
    pid: u32,
    process_creation_time: &str,
    manager: u64,
    data: u64,
    module_base: u64,
    native_index: &Value,
    capture_phase: &str,
) -> Value {
    let mut fingerprint = preview_inventory_fingerprint(
        container,
        serial_counter,
        acquisition_order_counter,
        layout,
        native_index,
    );
    if let Some(object) = fingerprint.as_object_mut() {
        object.insert("capture_phase".to_string(), json!(capture_phase));
        object.insert("pid".to_string(), json!(pid));
        object.insert(
            "process_creation_time".to_string(),
            json!(process_creation_time),
        );
        object.insert("profile_id".to_string(), json!(profile_id));
        object.insert("manager".to_string(), json!(manager));
        object.insert("data".to_string(), json!(data));
        object.insert("module_base".to_string(), json!(module_base));
    }
    fingerprint
}

/// The exact terminal receipt a preview mismatch leaves.
///
/// The fault-injecting transports use it so an offline run can present the shape
/// the settlement writes without a game process; the production path builds the
/// same shape incrementally inside `run_dispatch`. The helper refuses to hand
/// back a receipt its own predicate does not accept, so the two cannot drift.
#[cfg(any(test, feature = "test-fake"))]
#[allow(clippy::too_many_arguments)]
pub fn preview_rejection_receipt(
    operation_id: &str,
    parent_operation_id: Option<&str>,
    pid: u32,
    process_creation_time: &str,
    descriptor_hex: &str,
    expected_record_hex: &str,
    builder_code_hex: &str,
    source: &[u8],
    before: Value,
    after: Value,
) -> Result<Value, RuntimeError> {
    let frame = crate::mutation::native_fakes::frame_for(0, 0);
    let builder_code_sha256 = sha256_hex(&hex_decode(builder_code_hex).unwrap_or_default());
    let mut receipt = json!({
        "operation_id": operation_id,
        "pid": pid,
        "process_creation_time": process_creation_time,
        "phase": PREVIEW_PHASE_REJECTED_AFTER,
        "settlement": PREVIEW_SETTLEMENT_REJECTED,
        "active": false,
        "released": true,
        "redirect_count": 1,
        "breakpoint_count": 0,
        "business_outcome": "rejected",
        "remote_execution": "quiescent",
        "allocation_state": "freed",
        "debugger_state": "detached",
        "thread_cleanup": {},
        "executor": "windows-native",
        "mode": "preview",
        "parent_operation_id": parent_operation_id,
        "source_save_path": Value::Null,
        "candidate_id": Value::Null,
        "expected_record_hex": expected_record_hex,
        "serial": Value::Null,
        "slot": Value::Null,
        "status": 3,
        "source_hex": crate::mutation::native_abi::hex(source),
        "preview_intent": {
            "mode": "preview",
            "descriptor_sha256": sha256_hex(descriptor_hex.as_bytes()),
            "expected_record_sha256": sha256_hex(expected_record_hex.as_bytes()),
            "builder_code_sha256": builder_code_sha256,
            "allocate_serial": false,
            "insertion_args_present": false,
        },
        "preview_target": {"builder_code_sha256": builder_code_sha256},
        "preview_before": before,
        "preview_after": after,
        "preview_review": {"outcome": "mismatch", "source_serial_sentinel": true},
        "preview_dispatch_proof": {
            "return_and_register_verified": true,
            "inventory_fingerprints_agree": true,
        },
    });
    if let (Some(object), Some(frame_object)) = (receipt.as_object_mut(), frame.as_object()) {
        for (key, value) in frame_object {
            object.insert(key.clone(), value.clone());
        }
    }
    if !preview_rejection_complete(&receipt) {
        return Err(failed("Preview rejection fixture is not self-consistent"));
    }
    Ok(receipt)
}

/// Two preview fingerprints that agree: the same process lifetime, profile,
/// inventory owner and layout, and an unchanged container, mapping and counters.
pub fn preview_fingerprints_agree(receipt: &Value) -> bool {
    let (Some(before), Some(after)) = (receipt.get("preview_before"), receipt.get("preview_after"))
    else {
        return false;
    };
    if before.get("pid").is_none()
        || before.get("process_creation_time").is_none()
        || before.get("pid") != after.get("pid")
        || before.get("pid") != receipt.get("pid")
        || before.get("process_creation_time") != after.get("process_creation_time")
        || before.get("process_creation_time") != receipt.get("process_creation_time")
        || before.get("profile_id").is_none()
        || before.get("profile_id") != after.get("profile_id")
        || before.get("manager").is_none()
        || before.get("manager") != after.get("manager")
        || before.get("data").is_none()
        || before.get("data") != after.get("data")
        || before.get("module_base").is_none()
        || before.get("module_base") != after.get("module_base")
        || before.get("capture_phase").and_then(Value::as_str) != Some(PREVIEW_PHASE_BEFORE)
        || after.get("capture_phase").and_then(Value::as_str) != Some(PREVIEW_PHASE_AFTER)
    {
        return false;
    }
    [
        "container_sha256",
        "capacity",
        "record_size",
        "serial_counter",
        "acquisition_order_counter",
        "index_node_count",
        "index_bucket_count",
    ]
    .iter()
    .all(|key| {
        before.get(*key).is_some_and(|value| !value.is_null())
            && before.get(*key) == after.get(*key)
    }) && before
        .get("native_index_digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| {
            digest.len() == 64
                && before.get("native_index_digest") == after.get("native_index_digest")
        })
}

/// The evidence that decides a preview rejection, derived from the receipt
/// alone: the one acknowledged redirect, the dispatch's own return/source and
/// register frame, the non-allocating serial sentinel, two fingerprints that
/// agree, and a released owner.
///
/// The settlement words are deliberately absent so a producer can decide from
/// the same facts a reader re-derives.
pub fn preview_rejection_decided(receipt: &Value) -> bool {
    receipt.get("redirect_count").and_then(Value::as_u64) == Some(1)
        && receipt.get("released").and_then(Value::as_bool) == Some(true)
        && receipt
            .get("preview_review")
            .and_then(|value| value.get("outcome"))
            .and_then(Value::as_str)
            == Some("mismatch")
        && receipt
            .get("source_hex")
            .and_then(Value::as_str)
            .map(|text| {
                hex_decode(text).is_ok_and(|raw| {
                    raw.len() == RECORD_SIZE && raw[SERIAL_OFFSET..SERIAL_OFFSET + 8] == [0xFFu8; 8]
                })
            })
            .unwrap_or(false)
        && receipt
            .get("preview_dispatch_proof")
            .and_then(|value| value.get("return_and_register_verified"))
            .and_then(Value::as_bool)
            == Some(true)
        && receipt
            .get("preview_dispatch_proof")
            .and_then(|value| value.get("inventory_fingerprints_agree"))
            .and_then(Value::as_bool)
            == Some(true)
        // The code identity the gate verified at the target must be the reviewed
        // one the intent recorded, not a caller-supplied string.
        && receipt
            .get("preview_target")
            .and_then(|value| value.get("builder_code_sha256"))
            .and_then(Value::as_str)
            .is_some_and(|target| {
                target.len() == 64
                    && receipt
                        .get("preview_intent")
                        .and_then(|value| value.get("builder_code_sha256"))
                        .and_then(Value::as_str)
                        == Some(target)
            })
        && preview_fingerprints_agree(receipt)
        && verify_dispatch_evidence(receipt).is_ok()
}

/// The complete, immutable evidence a preview mismatch must leave behind before
/// the operation may be settled as a formal rejection.
///
/// This is [`preview_rejection_decided`] plus the recorded mode and terminal
/// words, so a stored receipt says both what was proved and what was concluded.
pub fn preview_rejection_complete(receipt: &Value) -> bool {
    receipt.get("mode").and_then(Value::as_str) == Some("preview")
        && receipt.get("phase").and_then(Value::as_str) == Some(PREVIEW_PHASE_REJECTED_AFTER)
        && receipt.get("settlement").and_then(Value::as_str) == Some(PREVIEW_SETTLEMENT_REJECTED)
        && receipt.get("business_outcome").and_then(Value::as_str) == Some("rejected")
        && preview_rejection_decided(receipt)
}

fn u16_at(raw: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([raw[offset], raw[offset + 1]])
}

fn u32_at(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        raw[offset],
        raw[offset + 1],
        raw[offset + 2],
        raw[offset + 3],
    ])
}

fn u64_at(raw: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&raw[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

/// Port of `live_add_evidence.verify`.
///
/// `plan` is the stored plan JSON; the inventories and indexes are the captures
/// taken around the dispatch.
pub fn verify(
    plan: &Value,
    execution: &Value,
    before: &Inventory,
    after: &Inventory,
    index_before: &NativeIndex,
    index_after: &NativeIndex,
) -> Result<Value, RuntimeError> {
    verify_dispatch(execution)?;
    let mode = execution.get("mode").and_then(Value::as_str);
    let operation_id = text(
        plan,
        "operation_id",
        "Insertion acknowledgement does not match the plan",
    )?;
    let plan_slot = field_u64(plan, "slot")?;
    if mode != Some("single_native_insertion")
        || execution.get("operation_id").and_then(Value::as_str) != Some(operation_id.as_str())
        || execution.get("status").and_then(Value::as_u64) != Some(3)
        || execution.get("slot").and_then(Value::as_u64) != Some(plan_slot)
    {
        return Err(failed("Insertion acknowledgement does not match the plan"));
    }
    let plan_pid = field_u64(plan, "pid")?;
    let execution_pid = execution
        .get("pid")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX);
    if [
        execution_pid,
        before.pid as u64,
        after.pid as u64,
        index_before.pid as u64,
        index_after.pid as u64,
    ]
    .iter()
    .any(|pid| *pid != plan_pid)
    {
        return Err(failed("Process identity differs"));
    }
    let old = inventory_entries(before)?;
    let new = inventory_entries(after)?;
    let plan_serial = field_u64(plan, "serial")?;
    let serial = plan_serial.to_string();
    if old.contains_key(&serial) || set_of(&new) != union_of(&old, &serial) {
        return Err(failed("Expected exactly one newly allocated serial"));
    }
    if old
        .iter()
        .any(|(key, entry)| new.get(key).is_none_or(|other| other != entry))
    {
        return Err(failed("An existing record changed"));
    }
    let added = new
        .get(&serial)
        .ok_or_else(|| failed("Expected exactly one newly allocated serial"))?;
    if added.slot_index as u64 != plan_slot {
        return Err(failed("The added record occupies another slot"));
    }
    let baseline = hex_decode(&text(
        plan,
        "container_hex",
        "Planned container does not match the before capture",
    )?)
    .map_err(|_| failed("Planned container does not match the before capture"))?;
    if baseline.len() != 400 * RECORD_SIZE || sha256_hex(&baseline) != before.container_sha256 {
        return Err(failed(
            "Planned container does not match the before capture",
        ));
    }
    let mut occupied: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    for slot in 0..400usize {
        let start = slot * RECORD_SIZE;
        if baseline[start] != 0 || baseline[start + 1] != 0 {
            occupied.insert(slot, baseline[start..start + RECORD_SIZE].to_vec());
        }
    }
    let mut planned: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    for entry in old.values() {
        let raw = hex_decode(&entry.record_hex)?;
        if raw.len() != RECORD_SIZE {
            return Err(failed("Invalid scroll record length"));
        }
        planned.insert(entry.slot_index, raw);
    }
    if occupied != planned {
        return Err(failed(
            "Before records do not match the complete planned container",
        ));
    }
    let start = plan_slot as usize * RECORD_SIZE;
    if baseline[start] != 0 || baseline[start + 1] != 0 {
        return Err(failed("Planned slot was not empty"));
    }
    let destination = record_hex(execution, "destination_hex", "Partial insertion receipt")?;
    let source = record_hex(execution, "source_hex", "Partial insertion receipt")?;
    let remainder = record_hex(execution, "remainder_hex", "Partial insertion receipt")?;
    let added_raw = hex_decode(&added.record_hex)?;
    if destination != added_raw || remainder[0] != 0 || remainder[1] != 0 {
        return Err(failed(
            "Destination or remainder disagrees with the receipt",
        ));
    }
    if u64_at(&source, 0x28) != plan_serial {
        return Err(failed("Builder source serial differs"));
    }
    if u32_at(&destination, 0x18) != (u32_at(&source, 0x18) | 0x0400_0080) {
        return Err(failed(
            "Native insertion flags differ from the accepted scroll path",
        ));
    }
    if source[..0x18] != destination[..0x18]
        || source[0x20..0x24] != destination[0x20..0x24]
        || source[0x28..0xE4] != destination[0x28..0xE4]
    {
        return Err(failed("Native insertion changed generated content"));
    }
    let mut predicted = baseline[..start].to_vec();
    predicted.extend_from_slice(&destination);
    predicted.extend_from_slice(&baseline[start + RECORD_SIZE..]);
    if sha256_hex(&predicted) != after.container_sha256 {
        return Err(failed("Another container byte changed"));
    }
    let before_serial: u64 = before
        .serial_counter
        .parse()
        .map_err(|_| failed("Native counters or acquisition order differ"))?;
    let after_serial: u64 = after
        .serial_counter
        .parse()
        .map_err(|_| failed("Native counters or acquisition order differ"))?;
    if before_serial != plan_serial
        || after_serial != plan_serial.wrapping_add(1)
        || after.acquisition_order_counter != before.acquisition_order_counter.wrapping_add(1)
        || u32_at(&destination, 0x1C) != before.acquisition_order_counter
    {
        return Err(failed("Native counters or acquisition order differ"));
    }
    let old_index = index_entries(&index_before.to_json())?;
    let new_index = index_entries(&index_after.to_json())?;
    let mut expected_index = old_index.clone();
    expected_index.insert(serial.clone(), plan_slot as u32);
    if new_index != expected_index {
        return Err(failed(
            "Native index change is not exactly the new full serial",
        ));
    }
    if new
        .iter()
        .any(|(key, item)| new_index.get(key).copied() != Some(item.slot_index as u32))
    {
        return Err(failed("Native index does not resolve occupied records"));
    }
    Ok(json!({
        "schema": "nioh3-live-add-verification/v1",
        "operation_id": operation_id,
        "serial": serial,
        "seed": added.seed,
        "slot": plan_slot,
        "previous_records_preserved": old.len(),
        "count_after": new.len(),
        "full_container_and_native_index_verified": true,
        "dispatch_and_cleanup_verified": true,
        "persistence_verified": false,
        "limits": [
            "One observed mission-thread insertion; not all-state concurrency acceptance.",
            "Persistence requires independent normal save and reload evidence.",
        ],
    }))
}

fn set_of(entries: &BTreeMap<String, InventoryEntry>) -> Vec<String> {
    let mut keys: Vec<String> = entries.keys().cloned().collect();
    keys.sort();
    keys
}

fn union_of(entries: &BTreeMap<String, InventoryEntry>, serial: &str) -> Vec<String> {
    let mut keys = set_of(entries);
    keys.push(serial.to_string());
    keys.sort();
    keys
}

/// Port of `live_add_evidence.verify_persistence`.
pub fn verify_persistence(
    after: &Inventory,
    saved_records: &[Vec<u8>],
    allow_new_marker_clear: bool,
) -> Result<Value, RuntimeError> {
    let expected = inventory_entries(after)?;
    let mut actual: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for raw in saved_records {
        if raw.len() != RECORD_SIZE {
            return Err(failed("Partial saved record"));
        }
        if raw[0] == 0 && raw[1] == 0 {
            continue;
        }
        let serial = u64_at(raw, 0x28).to_string();
        if actual.insert(serial, raw.clone()).is_some() {
            return Err(failed("Duplicate saved serial"));
        }
    }
    if actual.keys().collect::<Vec<_>>() != expected.keys().collect::<Vec<_>>() {
        return Err(failed("Saved inventory serial set differs"));
    }
    let mut cleared: Vec<String> = Vec::new();
    for (key, entry) in &expected {
        let mut previous = hex_decode(&entry.record_hex)?;
        let current = actual
            .get(key)
            .ok_or_else(|| failed("Saved inventory serial set differs"))?;
        if allow_new_marker_clear && previous[0x18] & 2 != 0 && current[0x18] == previous[0x18] & !2
        {
            previous[0x18] &= !2;
            cleared.push(key.clone());
        }
        if defined(current) != defined(&previous) {
            return Err(failed("Saved defined record fields differ"));
        }
    }
    Ok(json!({
        "records_verified": actual.len(),
        "serials_and_defined_fields_match": true,
        "new_marker_cleared_serials": cleared,
    }))
}

/// `u16` of one record, exposed because the count and live-add slices both
/// filter on the "occupied" marker.
pub fn record_type(raw: &[u8]) -> u16 {
    u16_at(raw, 0)
}
