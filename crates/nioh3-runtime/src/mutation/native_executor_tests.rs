//! The native executor's ownership rules, proven over the injected transport.

use crate::error::RuntimeError;
use crate::mutation::count::new_operation_id;
use crate::mutation::descriptor::new_assembly_record;
use crate::mutation::inventory::hex_decode;
use crate::mutation::live_add::LiveAddExecutor;
use crate::mutation::live_fakes::{assembly_record, InventoryFixture, FIXTURE_CREATION};
use crate::mutation::native_abi::{hex, PC_V201_LIVE_ADD};
use crate::mutation::native_executor::{
    DispatchBudget, NativeLiveAddExecutor, REQUIRED_DISPLAY_VERSION,
};
use crate::mutation::native_fakes::{FakeLiveAddTransport, NativeFaults};
use serde_json::json;
use std::path::PathBuf;

fn zero_pause() {}

fn budget() -> DispatchBudget {
    DispatchBudget {
        polls: 1,
        pause: zero_pause,
    }
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("nioh3-native-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn executor(
    fixture: &Fixture,
    faults: NativeFaults,
) -> Result<(NativeLiveAddExecutor<FakeLiveAddTransport>, Vec<u8>), RuntimeError> {
    let inventory = InventoryFixture::new(&[(4, 0x1234, 0xF00D)], 0x3345, 11);
    let transport = FakeLiveAddTransport::with_faults(&fixture.root, inventory, faults)?;
    let executor =
        NativeLiveAddExecutor::new(transport, PC_V201_LIVE_ADD, REQUIRED_DISPLAY_VERSION)
            .with_budget(budget());
    Ok((executor, assembly_record(0x1E82, 0x0BAD_F00D, 4)))
}

/// `LiveAddApplication.prepare` adds these three facts to the inspected plan.
fn prepared_plan(plan: &serde_json::Value, assembly: &[u8]) -> serde_json::Value {
    let mut plan = plan.clone();
    if let Some(object) = plan.as_object_mut() {
        object.insert(
            "operation_id".to_string(),
            json!(new_operation_id().unwrap_or_default()),
        );
        object.insert("expected_record_hex".to_string(), json!(hex(assembly)));
        object.insert(
            "source_save_path".to_string(),
            json!("76561198000000000/SAVEDATA00/SAVEDATA.BIN"),
        );
    }
    plan
}

#[test]
fn inspection_resolves_the_plan_the_executor_needs() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("inspect");
    let (mut executor, _assembly) = executor(&fixture, NativeFaults::default())?;
    let (plan, inventory, index) = executor.inspect()?;
    assert_eq!(plan.get("pid").and_then(|value| value.as_u64()), Some(4321));
    assert_eq!(
        plan.get("profile_id").and_then(|value| value.as_str()),
        Some("pc-v2.01-live-add-r1")
    );
    assert_eq!(
        plan.get("slot").and_then(|value| value.as_u64()),
        Some(0),
        "the first empty slot is the destination"
    );
    assert_eq!(
        plan.get("serial").and_then(|value| value.as_u64()),
        Some(0x3345)
    );
    assert_eq!(inventory.entries.len(), 1);
    assert_eq!(index.pid, inventory.pid);
    assert_eq!(
        plan.get("function_address")
            .and_then(|value| value.as_u64()),
        Some(0x7FF0_0000_0000 + PC_V201_LIVE_ADD.insertion_rva)
    );
    let container = hex_decode(
        plan.get("container_hex")
            .and_then(|value| value.as_str())
            .unwrap_or_default(),
    )?;
    assert_eq!(container.len(), 400 * 0xE8);
    Ok(())
}

#[test]
fn a_preview_completes_without_touching_the_inventory() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("preview");
    let (mut executor, assembly) = executor(&fixture, NativeFaults::default())?;
    let (plan, before, _index) = executor.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    let assembly_record = new_assembly_record(&assembly)?;
    let receipt = executor.preview(&plan, &assembly_record)?;
    assert_eq!(
        receipt.get("phase").and_then(|value| value.as_str()),
        Some("completed")
    );
    assert_eq!(
        receipt
            .get("redirect_count")
            .and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        receipt
            .get("breakpoints")
            .and_then(|value| value.as_array()),
        Some(&Vec::new())
    );
    let (after, _index) = executor.readback()?;
    assert_eq!(before, after, "a preview cannot change the inventory");
    Ok(())
}

#[test]
fn an_idle_miss_is_a_released_receipt_not_an_uncertain_one() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("idle");
    let (mut executor, assembly) = executor(
        &fixture,
        NativeFaults {
            idle_miss: true,
            ..NativeFaults::default()
        },
    )?;
    let (plan, before, _index) = executor.inspect()?;
    let mut plan = prepared_plan(&plan, &assembly);
    if let Some(object) = plan.as_object_mut() {
        object.insert("descriptor_hex".to_string(), json!("00"));
    }
    let receipt = executor.insert(&plan)?;
    assert_eq!(
        receipt
            .get("redirect_count")
            .and_then(|value| value.as_u64()),
        Some(0)
    );
    assert_eq!(
        receipt.get("error").and_then(|value| value.as_str()),
        Some("No accepted idle dispatch before timeout")
    );
    let (after, _index) = executor.readback()?;
    assert_eq!(before, after, "a missed window writes nothing");
    assert!(executor.safe_to_shutdown());
    Ok(())
}

#[test]
fn an_insertion_lands_and_settles_once() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("insert");
    let (mut executor, assembly) = executor(&fixture, NativeFaults::default())?;
    let (plan, before, _index) = executor.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    let receipt = executor.insert(&plan)?;
    assert_eq!(
        receipt.get("phase").and_then(|value| value.as_str()),
        Some("completed")
    );
    // The engine's own insertion assigns the live flags at 0x18 and the
    // acquisition order at 0x1C; every other byte is the accepted source.
    let destination = hex_decode(
        receipt
            .get("destination_hex")
            .and_then(|value| value.as_str())
            .unwrap_or_default(),
    )?;
    let source = hex_decode(
        receipt
            .get("source_hex")
            .and_then(|value| value.as_str())
            .unwrap_or_default(),
    )?;
    assert_eq!(destination[..0x18], source[..0x18]);
    assert_eq!(destination[0x20..], source[0x20..]);
    assert_ne!(
        destination[0x18..0x1C],
        source[0x18..0x1C],
        "the insertion marks the record as owned"
    );
    let (after, _index) = executor.readback()?;
    assert_eq!(after.entries.len(), before.entries.len() + 1);
    assert!(executor.safe_to_shutdown());
    // The same operation may not be submitted twice.
    assert_eq!(
        executor.transport().submissions.len(),
        1,
        "one submission, one insertion"
    );
    Ok(())
}

#[test]
fn a_lost_reply_stays_uncertain_and_recovery_only_verifies() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("lost");
    let (mut executor, assembly) = executor(
        &fixture,
        NativeFaults {
            reply_lost: true,
            ..NativeFaults::default()
        },
    )?;
    let (plan, before, _index) = executor.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    let operation_id = plan
        .get("operation_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let error = executor
        .insert(&plan)
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "NATIVE_DISPATCH");
    assert!(!executor.safe_to_shutdown(), "ownership is retained");
    assert_eq!(
        executor.transport().submissions.len(),
        1,
        "the insertion is never replayed"
    );
    let creation = FIXTURE_CREATION.to_string();
    let recovered = executor.recover(&operation_id, 4321, Some(&creation))?;
    assert_eq!(
        recovered.get("phase").and_then(|value| value.as_str()),
        Some("completed")
    );
    assert_eq!(
        recovered
            .get("recovered_by")
            .and_then(|value| value.as_str()),
        Some("destination_record_and_serial")
    );
    assert!(executor.safe_to_shutdown());
    let (after, _index) = executor.readback()?;
    assert_eq!(after.entries.len(), before.entries.len() + 1);
    assert_eq!(
        executor.transport().submissions.len(),
        1,
        "recovery dispatched nothing"
    );
    Ok(())
}

#[test]
fn a_lost_reply_with_no_destination_resolves_as_proven_absence() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("absent");
    let (mut executor, assembly) = executor(
        &fixture,
        NativeFaults {
            reply_lost: true,
            skip_destination: true,
            ..NativeFaults::default()
        },
    )?;
    let (plan, _before, _index) = executor.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    let operation_id = plan
        .get("operation_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(executor.insert(&plan).is_err());
    let creation = FIXTURE_CREATION.to_string();
    let recovered = executor.recover(&operation_id, 4321, Some(&creation))?;
    assert_eq!(
        recovered.get("phase").and_then(|value| value.as_str()),
        Some("rejected")
    );
    assert_eq!(
        recovered
            .get("recovered_by")
            .and_then(|value| value.as_str()),
        Some("proven_absence")
    );
    Ok(())
}

#[test]
fn a_rejected_submission_has_no_native_ownership() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("reject");
    let (mut executor, assembly) = executor(
        &fixture,
        NativeFaults {
            reject_before_dispatch: true,
            ..NativeFaults::default()
        },
    )?;
    let (plan, _before, _index) = executor.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    let operation_id = plan
        .get("operation_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(executor.insert(&plan).is_err());
    assert!(
        executor.submission_absent(&operation_id),
        "a provably absent submission releases ownership"
    );
    assert!(executor.safe_to_shutdown());
    Ok(())
}

#[test]
fn a_second_operation_is_refused_while_one_receipt_is_unresolved() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("busy");
    let (mut executor, assembly) = executor(
        &fixture,
        NativeFaults {
            reply_lost: true,
            ..NativeFaults::default()
        },
    )?;
    let (plan, _before, _index) = executor.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    assert!(executor.insert(&plan).is_err());
    // A dispatch that owns an unresolved receipt also blocks inspection, and a
    // replayed plan is refused before it reaches the transport.
    let refused = executor
        .inspect()
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(refused.code(), "NATIVE_DISPATCH");
    let error = executor
        .insert(&plan)
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert!(matches!(
        error.code(),
        "NATIVE_DISPATCH" | "LIVE_ADD_REJECTED"
    ));
    assert_eq!(
        executor.transport().submissions.len(),
        1,
        "the unresolved operation is never replayed"
    );
    Ok(())
}

#[test]
fn an_unapproved_version_is_refused_before_any_read() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("version");
    let inventory = InventoryFixture::new(&[(4, 0x1234, 0xF00D)], 0x3345, 11);
    let transport = FakeLiveAddTransport::new(&fixture.root, inventory)?;
    let mut executor = NativeLiveAddExecutor::new(transport, PC_V201_LIVE_ADD, "PC v2.00.02")
        .with_budget(budget());
    let error = executor
        .inspect()
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "NATIVE_DISPATCH");
    Ok(())
}
