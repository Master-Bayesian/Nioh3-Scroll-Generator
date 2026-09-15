//! Independent verification of a preserved native dispatch and of live addition.
//!
//! Port of `dispatch_evidence.verify_dispatch` and `live_add_evidence.verify` /
//! `verify_persistence`. The point of both is that success is never published
//! from the receipt alone: the complete container, the native serial index, the
//! counters and the saved records are all re-derived and compared.

use crate::error::RuntimeError;
use crate::mutation::count::sha256_hex;
use crate::mutation::inventory::{
    hex_decode, index_entries, inventory_entries, Inventory, InventoryEntry, NativeIndex,
    RECORD_SIZE,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// `dispatch_evidence.REGISTERS`.
pub const REGISTERS: [&str; 15] = [
    "RAX", "RBX", "RCX", "RDX", "RSI", "RDI", "RBP", "R8", "R9", "R10", "R11", "R12", "R13", "R14",
    "R15",
];

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

/// Port of `dispatch_evidence.verify_dispatch`.
///
/// Exactly one acknowledged redirect, a fully released allocation, no remaining
/// breakpoint, identical general registers, the expected stack prologue and
/// arithmetic flags that match a `push/push/sub` replay.
pub fn verify_dispatch(execution: &Value) -> Result<(), RuntimeError> {
    if execution.get("phase").and_then(Value::as_str) != Some("completed")
        || execution.get("redirect_count").and_then(Value::as_u64) != Some(1)
    {
        return Err(failed("Exactly one acknowledged redirect is required"));
    }
    let released = execution.get("released").and_then(Value::as_bool) == Some(true);
    let breakpoints = execution
        .get("breakpoints")
        .and_then(Value::as_array)
        .map(Vec::len);
    if !released || breakpoints != Some(0) {
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

/// `live_add_evidence.defined`.
pub fn defined(raw: &[u8]) -> Vec<u8> {
    let mut result = raw[..0x24].to_vec();
    result.extend_from_slice(&raw[0x28..0xE4]);
    result
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
