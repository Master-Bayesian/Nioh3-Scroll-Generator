//! The native executor's ownership rules, proven over the injected transport.

use crate::error::RuntimeError;
use crate::mutation::count::new_operation_id;
use crate::mutation::descriptor::new_assembly_record;
use crate::mutation::evidence::{
    preview_owner_fingerprint, preview_rejection_complete, preview_rejection_decided,
    preview_rejection_receipt, PREVIEW_PHASE_AFTER, PREVIEW_PHASE_BEFORE,
};
use crate::mutation::inventory::{
    hex_decode, PC_V201_INVENTORY_LAYOUT, RECORD_SIZE, SERIAL_OFFSET,
};
use crate::mutation::live_add::LiveAddExecutor;
use crate::mutation::live_fakes::{assembly_record, InventoryFixture, FIXTURE_CREATION};
use crate::mutation::native_abi::{
    hex, LiveAddLayout, CANDIDATE_DISPLAY_VERSION, PC_V201_LIVE_ADD,
    PC_V202_CANDIDATE_EXECUTABLE_SHA256, PC_V202_LIVE_ADD_CANDIDATE,
};
use crate::mutation::native_executor::{
    settled, DispatchBudget, NativeLiveAddExecutor, REQUIRED_DISPLAY_VERSION,
    SAVE_GENERATION_SERIAL_MAX,
};
use crate::mutation::native_fakes::{FakeLiveAddTransport, NativeFaults};
use serde_json::json;
use std::path::PathBuf;

#[path = "native_owner_red_tests.rs"]
mod runtime_owner_contract_tests;

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
    executor_with_serial(fixture, faults, 0x3345)
}

/// The same executor over a fixture whose live serial counter is `serial`.
fn executor_with_serial(
    fixture: &Fixture,
    faults: NativeFaults,
    serial: u64,
) -> Result<(NativeLiveAddExecutor<FakeLiveAddTransport>, Vec<u8>), RuntimeError> {
    let inventory = InventoryFixture::new(&[(4, 0x1234, 0xF00D)], serial, 11);
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
    // A verified business outcome is evidence about the insertion, not about
    // the cleanup owner: recovery refuses while that owner is retained.
    let error = executor
        .recover(&operation_id, 4321, Some(&creation))
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "NATIVE_DISPATCH");
    assert!(
        error.message().contains("runtime owner still retained"),
        "the refusal names the retained owner: {error}"
    );
    let recovered = executor.transport().store.read(&operation_id)?;
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
    assert_eq!(recovered["business_outcome"], "committed");
    assert_eq!(recovered["released"], false);
    assert_eq!(recovered["breakpoint_count"], -1);
    assert!(
        !executor.safe_to_shutdown(),
        "business verification cannot release the original runtime owner"
    );
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
fn a_lost_reply_with_no_destination_is_proven_absent_and_still_owned() -> Result<(), RuntimeError> {
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
    let error = executor
        .recover(&operation_id, 4321, Some(&creation))
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "NATIVE_DISPATCH");
    assert!(
        error.message().contains("runtime owner still retained"),
        "proven absence is not release: {error}"
    );
    let recovered = executor.transport().store.read(&operation_id)?;
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
    assert_eq!(recovered["released"], false);
    assert!(!executor.safe_to_shutdown());
    Ok(())
}

/// R1 (P0). A durable receipt is a claim: the reaper that owns the target can
/// deregister its record before the owner finishes, so a record that reads
/// `released` outlives the process that still owns the allocation and the
/// debug session. Recovery must refuse, keep the adapter closed, and stay
/// diagnosable instead of calling that receipt a release.
#[test]
fn a_settled_receipt_cannot_release_a_live_owner() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("owner-live");
    let (mut executor, assembly) = executor(
        &fixture,
        NativeFaults {
            settled_receipt_with_live_owner: true,
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
    assert!(
        executor.insert(&plan).is_err(),
        "the acknowledgement is lost"
    );
    let stored = executor.transport().store.read(&operation_id)?;
    assert!(settled(&stored), "the durable record reads settled");
    assert!(
        executor.transport().live_owner.is_some(),
        "the owner is still alive although its record reads settled"
    );

    let creation = FIXTURE_CREATION.to_string();
    let error = executor
        .recover(&operation_id, 4321, Some(&creation))
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "NATIVE_DISPATCH");
    assert!(
        error.message().contains("runtime owner still retained"),
        "the refusal is stable: {error}"
    );
    assert!(
        error.message().contains(&operation_id),
        "the refusal names the owning operation: {error}"
    );
    assert!(
        !executor.safe_to_shutdown(),
        "a settled receipt cannot make the adapter safe to shut down"
    );

    // The automatic recovery path cannot admit a second dispatch.
    let second = prepared_plan(&plan, &assembly);
    assert!(
        executor.insert(&second).is_err(),
        "a retained owner refuses a second dispatch"
    );
    assert_eq!(
        executor.transport().submissions.len(),
        1,
        "recovery dispatched nothing and the refused admission never reached the target"
    );
    Ok(())
}

/// R1 (P0) over the R2-A vocabulary. A record settled *because* Windows retired
/// a thread by reusing its numeric handle is still only a claim: the owner half
/// of the release proof must keep refusing while this process keeps the target.
#[test]
fn a_settled_handle_reused_receipt_still_cannot_release_a_live_owner() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("owner-live-reused");
    let (mut executor, assembly) = executor(
        &fixture,
        NativeFaults {
            settled_receipt_with_live_owner: true,
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
    assert!(
        executor.insert(&plan).is_err(),
        "the acknowledgement is lost"
    );

    // The durable record now reads settled through the reused-handle
    // vocabulary while the transport still owns the allocation and the session.
    let mut stored = executor.transport().store.read(&operation_id)?;
    stored["thread_cleanup"] = json!({
        "77:9": {
            "tid": 77,
            "instance": 9,
            "handle": 0x5000,
            "handle_provenance": "create_thread_event",
            "run_state": "exited",
            "cleanup_state": "handle_reused",
            "error": null,
        }
    });
    executor.transport().store.save(&stored)?;
    let stored = executor.transport().store.read(&operation_id)?;
    assert!(settled(&stored), "the reused-handle record reads settled");
    assert_eq!(
        stored["thread_cleanup"]["77:9"]["cleanup_state"], "handle_reused",
        "the reuse evidence is part of the durable record"
    );
    assert!(executor.transport().live_owner.is_some());

    let creation = FIXTURE_CREATION.to_string();
    let error = executor
        .recover(&operation_id, 4321, Some(&creation))
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "NATIVE_DISPATCH");
    assert!(
        error.message().contains("runtime owner still retained"),
        "the refusal is stable: {error}"
    );
    assert!(
        !executor.safe_to_shutdown(),
        "a settled reused-handle receipt cannot make the adapter safe to shut down"
    );
    Ok(())
}

/// R1 (P0), the cross-restart half. The next process builds a fresh adapter over
/// the same receipts; a settled record plus a transport that still knows it owns
/// the target must stay refused and unsafe to shut down.
#[test]
fn a_restarted_adapter_cannot_recover_past_a_live_owner() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("owner-restart");
    let (mut first, assembly) = executor(
        &fixture,
        NativeFaults {
            settled_receipt_with_live_owner: true,
            ..NativeFaults::default()
        },
    )?;
    let (plan, _before, _index) = first.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    let operation_id = plan
        .get("operation_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(first.insert(&plan).is_err());
    let stored = first.transport().store.read(&operation_id)?;
    assert!(settled(&stored));
    drop(first);

    let (mut second, _assembly) = executor(&fixture, NativeFaults::default())?;
    // The live owner survived the restart; only its durable record did not.
    second.transport_mut().live_owner = Some(operation_id.clone());
    let creation = FIXTURE_CREATION.to_string();
    let error = second
        .recover(&operation_id, 4321, Some(&creation))
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "NATIVE_DISPATCH");
    assert!(error.message().contains("runtime owner still retained"));
    assert!(
        !second.safe_to_shutdown(),
        "a settled record plus a live owner is not a safe shutdown"
    );
    let second_plan = prepared_plan(&plan, &assembly);
    assert!(second.insert(&second_plan).is_err());
    assert_eq!(second.transport().submissions.len(), 0);
    Ok(())
}

/// The other half of the same rule: recovery does clear the pending owner, and
/// reports the receipt, as soon as the transport proves the release.
#[test]
fn recovery_reports_success_only_with_a_proven_release() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("owner-proof");
    let (mut executor, assembly) = executor(&fixture, NativeFaults::default())?;
    let (plan, _before, _index) = executor.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    let operation_id = plan
        .get("operation_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let receipt = executor.insert(&plan)?;
    assert_eq!(receipt["released"], true);
    assert!(executor.transport().live_owner.is_none());
    let creation = FIXTURE_CREATION.to_string();
    let recovered = executor.recover(&operation_id, 4321, Some(&creation))?;
    assert_eq!(recovered["released"], true);
    assert_eq!(recovered["operation_id"], operation_id);
    assert!(executor.safe_to_shutdown());
    assert_eq!(
        executor.transport().submissions.len(),
        1,
        "recovery dispatched nothing"
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
    let operation_id = plan
        .get("operation_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    assert!(executor.insert(&plan).is_err());
    // A dispatch that owns an unresolved receipt also blocks inspection, and a
    // replayed plan is refused before it reaches the transport. The refusal names
    // the exact unsettled operation instead of a generic busy fact, so a caller
    // can recover it rather than guess.
    let refused = executor
        .inspect()
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(refused.code(), "LIVE_ADD_UNCERTAIN");
    assert!(
        refused.message().contains(&operation_id),
        "the refusal names the unsettled operation: {refused}"
    );
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

#[test]
fn the_top_allowed_serial_still_plans_and_inserts() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("serial-top");
    let (mut executor, assembly) = executor_with_serial(
        &fixture,
        NativeFaults::default(),
        SAVE_GENERATION_SERIAL_MAX - 1,
    )?;
    let (plan, before, _index) = executor.inspect()?;
    assert_eq!(
        plan.get("serial").and_then(|value| value.as_u64()),
        Some(SAVE_GENERATION_SERIAL_MAX - 1),
        "the last serial whose successor is still a save serial is planned"
    );
    let plan = prepared_plan(&plan, &assembly);
    let receipt = executor.insert(&plan)?;
    assert_eq!(
        receipt.get("phase").and_then(|value| value.as_str()),
        Some("completed")
    );
    let (after, _index) = executor.readback()?;
    assert_eq!(after.entries.len(), before.entries.len() + 1);
    assert_eq!(
        after.serial_counter,
        SAVE_GENERATION_SERIAL_MAX.to_string(),
        "the counter advances to the top representable serial"
    );
    Ok(())
}

#[test]
fn a_serial_whose_successor_cannot_be_saved_is_refused_before_dispatch() -> Result<(), RuntimeError>
{
    for serial in [
        // Below the save domain: the codec's serial range starts at 1.
        0,
        // The top representable serial itself: its successor is not a serial.
        SAVE_GENERATION_SERIAL_MAX,
        SAVE_GENERATION_SERIAL_MAX + 1,
        // High 32 bits set: the live value could never round-trip into a save.
        u64::from(u32::MAX) + 1,
    ] {
        let fixture = Fixture::new(&format!("serial-refuse-{serial}"));
        let (mut executor, _assembly) =
            executor_with_serial(&fixture, NativeFaults::default(), serial)?;
        let error = executor
            .inspect()
            .err()
            .unwrap_or(RuntimeError::RuntimeBusy);
        assert_eq!(error.code(), "LIVE_ADD_REJECTED", "serial {serial:#x}");
        assert_eq!(
            executor.transport().submissions.len(),
            0,
            "a refused serial never reaches the transport"
        );
        assert!(executor.safe_to_shutdown());
    }
    Ok(())
}

/// An injected transport over one fixture built for `layout`, claiming the
/// exact executable identity the caller supplies.
fn transport_for_layout(
    fixture: &Fixture,
    layout: LiveAddLayout,
    executable_sha256: Option<&str>,
) -> Result<FakeLiveAddTransport, RuntimeError> {
    let inventory_layout = if layout == PC_V202_LIVE_ADD_CANDIDATE {
        crate::mutation::inventory::PC_V202_INVENTORY_LAYOUT_CANDIDATE
    } else {
        crate::mutation::inventory::PC_V201_INVENTORY_LAYOUT
    };
    let inventory =
        InventoryFixture::new_for_layout(inventory_layout, &[(4, 0x1234, 0xF00D)], 0x3345, 11);
    FakeLiveAddTransport::with_layout_faults(
        &fixture.root,
        inventory,
        layout,
        executable_sha256,
        NativeFaults::default(),
    )
}

/// The opt-in research candidate: the pinned layout, the pinned version and the
/// exact executable are all required, and the run then plans and inserts over
/// synthetic memory exactly like the product pair does.
#[test]
fn the_pinned_candidate_binding_plans_and_inserts() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("candidate-binding");
    let transport = transport_for_layout(
        &fixture,
        PC_V202_LIVE_ADD_CANDIDATE,
        Some(PC_V202_CANDIDATE_EXECUTABLE_SHA256),
    )?;
    let mut executor = NativeLiveAddExecutor::candidate(transport).with_budget(budget());
    let assembly = assembly_record(0x1E82, 0x0BAD_F00D, 4);
    let (plan, before, _index) = executor.inspect()?;
    assert_eq!(
        plan.get("profile_id").and_then(serde_json::Value::as_str),
        Some("pc-v2.02-live-add-candidate")
    );
    assert_eq!(
        plan.get("function_address")
            .and_then(serde_json::Value::as_u64),
        Some(0x7FF0_0000_0000 + PC_V202_LIVE_ADD_CANDIDATE.insertion_rva)
    );
    // No online session in the fixture: the builder identity reads as offline.
    assert_eq!(plan["builder_ambient_identity"], json!("0"));
    assert_eq!(
        executor.display_version(),
        CANDIDATE_DISPLAY_VERSION,
        "the bound version is the one the run proved"
    );
    let plan = prepared_plan(&plan, &assembly);
    let receipt = executor.insert(&plan)?;
    assert_eq!(
        receipt.get("phase").and_then(serde_json::Value::as_str),
        Some("completed")
    );
    let (after, _index) = executor.readback()?;
    assert_eq!(after.entries.len(), before.entries.len() + 1);
    assert_eq!(after.serial_counter, (0x3345u64 + 1).to_string());
    assert_eq!(executor.transport().submissions.len(), 1);
    assert!(executor.safe_to_shutdown());
    Ok(())
}

/// The pinned identity is a hex digest, so a transport may render it in either
/// case: the real `NativeDebugTransport` returns `sha256_hex` (lower case) while
/// the pinned constant is upper case. A different digest and a missing proof must
/// still be refused before any target read.
#[test]
fn the_pinned_executable_identity_compares_hex_case_insensitively() -> Result<(), RuntimeError> {
    let pinned = PC_V202_CANDIDATE_EXECUTABLE_SHA256;
    let lower = pinned.to_lowercase();
    assert_ne!(lower, pinned, "the two renderings differ in case");

    let fixture = Fixture::new("candidate-identity-lower-case");
    let transport = transport_for_layout(&fixture, PC_V202_LIVE_ADD_CANDIDATE, Some(&lower))?;
    let mut executor = NativeLiveAddExecutor::candidate(transport).with_budget(budget());
    let (plan, _before, _index) = executor.inspect()?;
    assert_eq!(
        plan.get("profile_id").and_then(serde_json::Value::as_str),
        Some("pc-v2.02-live-add-candidate"),
        "a lower-case rendering of the pinned digest is the same identity"
    );

    let mut other = lower.clone();
    other.replace_range(0..1, if lower.starts_with('e') { "f" } else { "e" });
    assert_ne!(other, lower, "the probed digest really differs");
    let fixture = Fixture::new("candidate-identity-other-digest");
    let transport = transport_for_layout(&fixture, PC_V202_LIVE_ADD_CANDIDATE, Some(&other))?;
    let mut executor = NativeLiveAddExecutor::candidate(transport).with_budget(budget());
    let error = executor
        .inspect()
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(
        error.code(),
        "NATIVE_DISPATCH",
        "a different digest refuses"
    );
    assert_eq!(
        executor.transport().reads,
        0,
        "a different digest is refused before any target read"
    );

    let fixture = Fixture::new("candidate-identity-missing-proof");
    let transport = transport_for_layout(&fixture, PC_V202_LIVE_ADD_CANDIDATE, None)?;
    let mut executor = NativeLiveAddExecutor::candidate(transport).with_budget(budget());
    let error = executor
        .inspect()
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "NATIVE_DISPATCH", "missing proof refuses");
    assert_eq!(
        executor.transport().reads,
        0,
        "missing proof is refused before any target read"
    );
    Ok(())
}

/// Neither half of the binding authorizes the other, and the candidate half
/// additionally requires the exact executable: every unbound combination is
/// refused before a single target read or any dispatch.
#[test]
fn an_unbound_live_add_pair_is_refused_before_any_read_or_dispatch() -> Result<(), RuntimeError> {
    let cases: [(&str, LiveAddLayout, &str, LiveAddLayout, Option<&str>); 5] = [
        (
            "candidate-layout-product-version",
            PC_V202_LIVE_ADD_CANDIDATE,
            REQUIRED_DISPLAY_VERSION,
            PC_V202_LIVE_ADD_CANDIDATE,
            Some(PC_V202_CANDIDATE_EXECUTABLE_SHA256),
        ),
        (
            "product-layout-candidate-version",
            PC_V201_LIVE_ADD,
            CANDIDATE_DISPLAY_VERSION,
            PC_V201_LIVE_ADD,
            Some(PC_V202_CANDIDATE_EXECUTABLE_SHA256),
        ),
        (
            "candidate-layout-product-identity",
            PC_V202_LIVE_ADD_CANDIDATE,
            CANDIDATE_DISPLAY_VERSION,
            PC_V201_LIVE_ADD,
            Some(PC_V202_CANDIDATE_EXECUTABLE_SHA256),
        ),
        (
            "candidate-layout-other-executable",
            PC_V202_LIVE_ADD_CANDIDATE,
            CANDIDATE_DISPLAY_VERSION,
            PC_V202_LIVE_ADD_CANDIDATE,
            Some("00"),
        ),
        (
            "candidate-layout-no-executable-proof",
            PC_V202_LIVE_ADD_CANDIDATE,
            CANDIDATE_DISPLAY_VERSION,
            PC_V202_LIVE_ADD_CANDIDATE,
            None,
        ),
    ];
    for (label, layout, version, transport_layout, identity) in cases {
        let fixture = Fixture::new(label);
        let transport = transport_for_layout(&fixture, transport_layout, identity)?;
        let mut executor =
            NativeLiveAddExecutor::new(transport, layout, version).with_budget(budget());
        let error = executor
            .inspect()
            .err()
            .unwrap_or(RuntimeError::RuntimeBusy);
        assert_eq!(error.code(), "NATIVE_DISPATCH", "{label}");
        assert_eq!(
            executor.transport().reads,
            0,
            "{label} must be refused before any target read"
        );
        // A dispatch cannot outrun the same gate, even without inspection.
        let plan = json!({"operation_id": "unbound", "pid": 4321});
        assert!(executor.insert(&plan).is_err(), "{label}");
        assert_eq!(
            executor.transport().reads,
            0,
            "{label} must not read while refusing a dispatch"
        );
        assert!(
            executor.transport().submissions.is_empty(),
            "{label} must never reach the transport"
        );
        assert!(executor.safe_to_shutdown(), "{label}");
    }
    Ok(())
}

/// Build the executor over one layout, with the injected faults the caller
/// chooses and the fixture the preview tests read.
fn preview_executor(
    fixture: &Fixture,
    faults: NativeFaults,
) -> Result<NativeLiveAddExecutor<FakeLiveAddTransport>, RuntimeError> {
    let inventory = InventoryFixture::new(&[(4, 0x1234, 0xF00D)], 0x3345, 11);
    let transport = FakeLiveAddTransport::with_faults(&fixture.root, inventory, faults)?;
    Ok(
        NativeLiveAddExecutor::new(transport, PC_V201_LIVE_ADD, REQUIRED_DISPLAY_VERSION)
            .with_budget(budget()),
    )
}

/// The durable preview receipt the transport left, whatever its identity.
fn preview_receipt(
    executor: &NativeLiveAddExecutor<FakeLiveAddTransport>,
) -> Result<serde_json::Value, RuntimeError> {
    let receipts = executor.transport().store.all()?;
    assert_eq!(receipts.len(), 1, "exactly one durable preview receipt");
    Ok(receipts[0].clone())
}

/// A preview whose builder output differs settles as a formal rejection over the
/// shipped adapter and recovery settles it read-only, twice, without replay.
#[test]
fn a_preview_mismatch_settles_and_recovers_read_only() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("preview-rejection");
    let mut executor = preview_executor(
        &fixture,
        NativeFaults {
            preview_mismatch: true,
            ..NativeFaults::default()
        },
    )?;
    let assembly = assembly_record(0x1E82, 0x0BAD_F00D, 4);
    let (plan, before, _index) = executor.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    let native = new_assembly_record(&assembly)?;
    let error = executor
        .preview(&plan, &native)
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "LIVE_ADD_VERIFICATION", "{error:?}");

    let receipt = preview_receipt(&executor)?;
    assert!(preview_rejection_complete(&receipt), "{receipt}");
    assert!(settled(&receipt), "the rejection is a terminal receipt");
    assert_eq!(receipt["redirect_count"], 1);
    assert_eq!(receipt["business_outcome"], "rejected");
    assert_eq!(receipt["phase"], "rejected_after_preview");
    let child = receipt["operation_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let creation = FIXTURE_CREATION.to_string();
    let recovered = executor.recover(&child, 4321, Some(&creation))?;
    assert_eq!(recovered["settlement"], "rejected_after_preview");
    assert_eq!(recovered["redirect_count"], 1);
    // The same id recovers again and dispatches nothing at all.
    let _ = executor.recover(&child, 4321, Some(&creation))?;
    assert_eq!(
        executor.transport().submissions.len(),
        1,
        "recovery never submits: one preview, no replay"
    );
    let (after, _index) = executor.readback()?;
    assert_eq!(before, after, "a rejected preview wrote nothing");
    assert!(executor.safe_to_shutdown());
    Ok(())
}

/// A preview without the complete proof keeps the owner and never claims a
/// rejection, while the read-only recovery still refuses it.
#[test]
fn a_preview_without_complete_evidence_stays_blocked() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("preview-incomplete");
    let mut executor = preview_executor(
        &fixture,
        NativeFaults {
            preview_incomplete: true,
            ..NativeFaults::default()
        },
    )?;
    let assembly = assembly_record(0x1E82, 0x0BAD_F00D, 4);
    let (plan, _before, _index) = executor.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    let native = new_assembly_record(&assembly)?;
    assert!(executor.preview(&plan, &native).is_err());

    let receipt = preview_receipt(&executor)?;
    assert!(
        !preview_rejection_complete(&receipt),
        "an incomplete proof is never a formal rejection"
    );
    assert!(
        !preview_rejection_decided(&receipt),
        "the decision is derived from the evidence, not the words"
    );
    assert!(!settled(&receipt));
    let child = receipt["operation_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let creation = FIXTURE_CREATION.to_string();
    assert!(
        executor.recover(&child, 4321, Some(&creation)).is_err(),
        "the retained owner refuses recovery"
    );
    assert_eq!(
        executor.transport().submissions.len(),
        1,
        "the blocked preview was never replayed"
    );
    assert!(!executor.safe_to_shutdown());
    Ok(())
}

/// Every clause of the rejection predicate has to hold: an absent baseline, a
/// changed mapping with an unchanged node count, another process lifetime, an
/// allocated serial and a redirect of zero each refuse it.
#[test]
fn a_preview_rejection_requires_every_proof_clause() -> Result<(), RuntimeError> {
    let layout = PC_V201_INVENTORY_LAYOUT;
    let mut container = vec![0u8; layout.capacity as usize * layout.record_size];
    for (slot, serial) in [(4usize, 0x1234u64), (5, 0x4321)] {
        let start = slot * layout.record_size;
        container[start] = 0x82;
        container[start + 1] = 0x1E;
        container[start + SERIAL_OFFSET..start + SERIAL_OFFSET + 8]
            .copy_from_slice(&serial.to_le_bytes());
    }
    let mut source = vec![0x11u8; RECORD_SIZE];
    source[0x28..0x30].copy_from_slice(&[0xFF; 8]);
    // The *actual* native index at the same stop: two nodes in one bucket. The
    // fingerprint's index evidence is this mapping, not the container.
    let native_index = json!({
        "schema": "nioh3-native-serial-index/v1",
        "pid": 4321,
        "process_creation_time": "134338049984156850",
        "node_count": 2,
        "bucket_count": 1,
        "entries": [
            {"serial": "4660", "slot": 4},
            {"serial": "17185", "slot": 5},
        ],
    });
    let native_index_before = native_index.clone();
    let fingerprint = |phase: &str| {
        preview_owner_fingerprint(
            &container,
            0x3345,
            11,
            &layout,
            "pc-v2.01-live-add-r1",
            4321,
            "134338049984156850",
            0x7FF0_0000_0001_0000,
            0x7FF0_0000_0002_0000,
            0x7FF0_0000_0000,
            &native_index_before,
            phase,
        )
    };
    let receipt = preview_rejection_receipt(
        "child-1",
        Some("parent-1"),
        4321,
        "134338049984156850",
        "descriptor",
        "record",
        "builder",
        &source,
        fingerprint(PREVIEW_PHASE_BEFORE),
        fingerprint(PREVIEW_PHASE_AFTER),
    )?;
    assert!(preview_rejection_complete(&receipt), "{receipt}");
    assert_eq!(
        receipt["preview_before"]["index_node_count"],
        receipt["preview_after"]["index_node_count"]
    );
    assert_eq!(
        receipt["preview_before"]["index_bucket_count"],
        receipt["preview_after"]["index_bucket_count"]
    );
    assert_eq!(
        receipt["preview_before"]["container_sha256"],
        receipt["preview_after"]["container_sha256"]
    );
    assert!(
        receipt["preview_before"]["native_index_digest"]
            .as_str()
            .is_some_and(|digest| digest.len() == 64),
        "the native index digest is recorded: {receipt}"
    );

    // A remapped *index* with the same container and the same node count is still
    // a change: the index is read from its own graph, not derived from records.
    let mut remapped = receipt.clone();
    remapped["preview_after"]["native_index_digest"] = json!("00".repeat(32));
    assert_eq!(
        remapped["preview_before"]["index_node_count"],
        remapped["preview_after"]["index_node_count"]
    );
    assert_eq!(
        remapped["preview_before"]["container_sha256"],
        remapped["preview_after"]["container_sha256"]
    );
    assert!(!preview_rejection_decided(&remapped));

    // The same index with a changed node count is a change too.
    let mut resized = receipt.clone();
    resized["preview_after"]["index_node_count"] = json!(3);
    assert!(!preview_rejection_decided(&resized));

    // An index that could not be read at all leaves no digest to agree on.
    let mut absent_index = receipt.clone();
    absent_index["preview_after"]["native_index_digest"] = serde_json::Value::Null;
    assert!(!preview_rejection_decided(&absent_index));

    let mut other_lifetime = receipt.clone();
    other_lifetime["preview_after"]["process_creation_time"] = json!("other");
    assert!(!preview_rejection_decided(&other_lifetime));

    let mut absent_baseline = receipt.clone();
    if let Some(object) = absent_baseline.as_object_mut() {
        object.remove("preview_before");
    }
    assert!(!preview_rejection_decided(&absent_baseline));

    let mut allocated = receipt.clone();
    let mut allocated_source = source.clone();
    allocated_source[0x28..0x30].copy_from_slice(&7u64.to_le_bytes());
    allocated["source_hex"] = json!(hex(&allocated_source));
    assert!(!preview_rejection_decided(&allocated));

    let mut before_dispatch = receipt.clone();
    before_dispatch["redirect_count"] = json!(0);
    assert!(!preview_rejection_decided(&before_dispatch));
    assert!(!preview_rejection_complete(&before_dispatch));

    let mut unproved = receipt.clone();
    unproved["preview_dispatch_proof"]["return_and_register_verified"] = json!(false);
    assert!(!preview_rejection_decided(&unproved));
    Ok(())
}

/// Fixture memory for the PC v2.02 ambient-identity chain.
#[allow(clippy::unwrap_used)]
fn identity_memory(
    session_state: Option<u8>,
    gate: u8,
    ready: u8,
    kind: u16,
    class: u8,
) -> std::collections::BTreeMap<u64, u8> {
    use crate::mutation::native_abi::PC_V202_BUILDER_IDENTITY as LAYOUT;
    const BASE: u64 = 0x7FF6_0000_0000;
    const SESSION: u64 = 0x2_0000_0000;
    let mut memory = std::collections::BTreeMap::new();
    let mut put = |address: u64, bytes: &[u8]| {
        for (offset, byte) in bytes.iter().enumerate() {
            memory.insert(address + offset as u64, *byte);
        }
    };
    for (rva, code) in LAYOUT.code {
        put(BASE + rva, &hex_decode(code).unwrap());
    }
    let session = if session_state.is_some() { SESSION } else { 0 };
    put(BASE + LAYOUT.session_pointer_rva, &session.to_le_bytes());
    put(
        SESSION + LAYOUT.session_state_offset,
        &[session_state.unwrap_or(0)],
    );
    put(BASE + LAYOUT.online_gate_rva, &[gate]);
    put(BASE + LAYOUT.identity_ready_rva, &[ready]);
    let mut identity = 0x0110_0001_1234_5678u64.to_le_bytes().to_vec();
    identity.extend_from_slice(&[0; 8]);
    identity.extend_from_slice(&kind.to_le_bytes());
    identity.push(class);
    put(BASE + LAYOUT.identity_rva, &identity);
    memory
}

fn ambient_of(memory: &std::collections::BTreeMap<u64, u8>) -> Result<u64, RuntimeError> {
    use crate::mutation::native_abi::PC_V202_BUILDER_IDENTITY;
    use crate::mutation::native_executor::read_builder_ambient_identity;
    read_builder_ambient_identity(
        &mut |address, size| {
            (address..address + size as u64)
                .map(|at| {
                    memory
                        .get(&at)
                        .copied()
                        .ok_or_else(|| RuntimeError::NativeDispatch {
                            detail: format!("unmapped {at:#x}"),
                        })
                })
                .collect()
        },
        0x7FF6_0000_0000,
        &PC_V202_BUILDER_IDENTITY,
    )
}

#[test]
#[allow(clippy::unwrap_used)]
fn the_ambient_identity_follows_the_decoded_chain() {
    let account = 0x0110_0001_1234_5678u64;
    assert_eq!(
        ambient_of(&identity_memory(Some(2), 1, 1, 0x0100, 1)).unwrap(),
        account
    );
    assert_eq!(
        ambient_of(&identity_memory(Some(4), 1, 1, 0x01AB, 2)).unwrap(),
        account
    );
    // Every branch the game takes to `A = 0`.
    for memory in [
        identity_memory(None, 1, 1, 0x0100, 1),
        identity_memory(Some(3), 1, 1, 0x0100, 1),
        identity_memory(Some(0), 1, 1, 0x0100, 1),
        identity_memory(Some(2), 0, 1, 0x0100, 1),
        identity_memory(Some(2), 1, 1, 0x0200, 1),
        identity_memory(Some(2), 1, 1, 0x0100, 3),
        identity_memory(Some(2), 1, 1, 0x0100, 0),
    ] {
        assert_eq!(ambient_of(&memory).unwrap(), 0);
    }
    // An identity the game has not initialized is refused, not guessed.
    assert!(ambient_of(&identity_memory(Some(2), 1, 0, 0x0100, 1)).is_err());
    // A different function body on the chain is refused before any data read.
    let mut patched = identity_memory(Some(2), 1, 1, 0x0100, 1);
    patched.insert(0x7FF6_0000_0000 + 0x654E94, 0x90);
    assert!(ambient_of(&patched).is_err());
}

/// Both accepted builders carry the bit-25 guard, so both bind a chain.
#[test]
fn every_accepted_builder_binds_its_own_identity_chain() {
    use crate::mutation::native_abi::{
        builder_identity_for, PC_V201_BUILDER_IDENTITY, PC_V202_BUILDER_IDENTITY,
    };
    assert_eq!(
        builder_identity_for(&PC_V201_LIVE_ADD),
        Some(&PC_V201_BUILDER_IDENTITY)
    );
    assert_eq!(
        builder_identity_for(&PC_V202_LIVE_ADD_CANDIDATE),
        Some(&PC_V202_BUILDER_IDENTITY)
    );
    // The reviewed bodies differ only in their relocated displacements.
    for (old, new) in PC_V201_BUILDER_IDENTITY
        .code
        .iter()
        .zip(PC_V202_BUILDER_IDENTITY.code)
    {
        assert_eq!(old.1.len(), new.1.len());
    }
}
