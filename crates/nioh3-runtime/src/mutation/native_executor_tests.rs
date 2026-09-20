//! The native executor's ownership rules, proven over the injected transport.

use crate::error::RuntimeError;
use crate::mutation::count::new_operation_id;
use crate::mutation::descriptor::new_assembly_record;
use crate::mutation::inventory::hex_decode;
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
