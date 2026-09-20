//! T3a runtime-owner model controls and expected-red production contracts.
//!
//! The `owner_model_*` controls prove that the offline fake can represent the
//! Win32 lifecycle required by the Pro review. The `red_*` tests deliberately
//! fail against the current production/fake contract; T3 must make them green
//! without weakening fail-closed ownership.

use super::{executor, prepared_plan, Fixture};
use crate::error::RuntimeError;
use crate::mutation::live_add::LiveAddExecutor;
use crate::mutation::live_fakes::InventoryFixture;
use crate::mutation::native_fakes::{
    FakeDebugSession, FakeRuntimeOwner, FakeThreadState, NativeFaults, RuntimeOwnerFaults,
};
use crate::mutation::win_session::{DebugEvent, DebugSession, ThreadContext};

fn create_thread(tid: u32, handle: u64) -> DebugEvent {
    DebugEvent {
        code: 2,
        tid,
        thread_handle: Some(handle),
        ..DebugEvent::default()
    }
}

fn exit_thread(tid: u32) -> DebugEvent {
    DebugEvent {
        code: 4,
        tid,
        exit_code: Some(0),
        ..DebugEvent::default()
    }
}

fn exception(tid: u32, code: i32) -> DebugEvent {
    DebugEvent {
        code: 1,
        tid,
        exception_code: Some(code),
        ..DebugEvent::default()
    }
}

fn registers(seed: u64) -> ThreadContext {
    ThreadContext {
        dr0: seed,
        dr1: seed + 1,
        dr2: seed + 2,
        dr3: seed + 3,
        dr6: seed + 4,
        dr7: seed + 5,
        ..ThreadContext::default()
    }
}

#[test]
fn owner_model_requires_exit_retirement_before_numeric_handle_reuse() -> Result<(), RuntimeError> {
    let mut owner = FakeRuntimeOwner::default();
    owner.attach();
    owner.begin_event(create_thread(10, 0x44))?;
    let first = owner.adopt_thread(10, 0x44, registers(10))?;
    owner.continue_event(true)?;

    owner.begin_event(exit_thread(10))?;
    owner.continue_event(true)?;
    assert_eq!(owner.threads[&10].state, FakeThreadState::Exited);
    assert!(
        !owner.threads[&10].handle_open,
        "continuing EXIT_THREAD automatically closes the event-supplied handle"
    );
    assert!(
        owner.adopt_thread(11, 0x44, registers(20)).is_err(),
        "an automatically closed numeric handle remains ambiguous until the exact EXIT_THREAD instance is retired"
    );

    let retired = owner.retire_exit_thread(10)?;
    assert_eq!(retired.instance, first);
    let second = owner.adopt_thread(11, 0x44, registers(20))?;
    assert_ne!(first, second, "numeric handle reuse never reuses identity");
    Ok(())
}

#[test]
fn owner_model_refuses_running_context_and_requires_a_stopped_event() -> Result<(), RuntimeError> {
    let mut owner = FakeRuntimeOwner::default();
    owner.adopt_thread(20, 0x55, registers(30))?;
    assert!(owner.context(20).is_err(), "a running context is illegal");
    assert!(
        owner.set_context(20, &registers(90)).is_err(),
        "a running context cannot be written"
    );
    owner.begin_event(exception(20, 0x8000_0004u32 as i32))?;
    assert_eq!(owner.context(20)?.dr7, 35);
    owner.set_context(20, &registers(90))?;
    assert_eq!(owner.context(20)?.dr7, 95);
    owner.continue_event(true)?;
    assert!(
        owner.context(20).is_err(),
        "continue returns the thread to running"
    );
    assert!(owner.set_context(20, &registers(100)).is_err());
    Ok(())
}

#[test]
fn owner_model_exposes_partial_restore_and_detach_failure() -> Result<(), RuntimeError> {
    let mut owner = FakeRuntimeOwner::with_faults(RuntimeOwnerFaults {
        restore_tid: Some(31),
        detach: true,
    });
    owner.attach();
    owner.adopt_thread(30, 0x60, registers(40))?;
    owner.adopt_thread(31, 0x61, registers(50))?;
    owner.begin_event(exception(30, 0x8000_0004u32 as i32))?;
    let Some(second) = owner.threads.get_mut(&31) else {
        return Err(RuntimeError::RuntimeBusy);
    };
    second.state = FakeThreadState::Stopped;
    assert!(owner.restore_threads().is_err());
    assert!(
        owner.threads[&30].restored,
        "the first restore remains recorded"
    );
    assert!(
        !owner.threads[&31].restored,
        "the injected failure remains owned"
    );
    assert_eq!(owner.restore_attempts, vec![(30, 1), (31, 2)]);
    assert!(owner.detach().is_err());
    assert!(
        owner.attached,
        "detach failure cannot erase debugger ownership"
    );
    assert_eq!(owner.detach_attempts, 1);
    Ok(())
}

#[test]
fn owner_model_records_late_non_owned_exception_as_not_handled() -> Result<(), RuntimeError> {
    let mut owner = FakeRuntimeOwner::default();
    let late = exception(99, 0xC000_0005u32 as i32);
    owner.begin_event(late)?;
    owner.continue_event(false)?;
    assert_eq!(owner.resumes, vec![(late, false)]);
    assert!(
        owner.threads.is_empty(),
        "a late foreign exception creates no owner"
    );
    Ok(())
}

/// EXPECTED RED: the current fake returns a context merely because a map entry
/// exists. The real contract requires proof that a debug event currently stops
/// this thread instance.
#[test]
fn t3a_red_current_debug_fake_refuses_context_for_a_running_thread() -> Result<(), RuntimeError> {
    let fixture = InventoryFixture::new(&[], 1, 1);
    let mut session = FakeDebugSession::new(&fixture);
    session.contexts.insert(70, ThreadContext::default());
    let create = create_thread(70, 0x6f);
    session.events.push_back(create);
    let delivered = DebugSession::wait(&mut session, 0)?.ok_or(RuntimeError::SessionNotOpen)?;
    DebugSession::adopt_thread(&mut session, 70, 0x6f)?;
    DebugSession::resume(&mut session, &delivered, true)?;
    assert!(
        DebugSession::context(&mut session, 70).is_err(),
        "EXPECTED RED: context access is currently allowed without a stopped-event barrier"
    );
    Ok(())
}

/// EXPECTED RED: continuing EXIT_THREAD closes the event-supplied handle. A
/// cached context/instance must be retired before the numeric value is reused.
#[test]
fn t3a_red_current_debug_fake_retires_exit_thread_state() -> Result<(), RuntimeError> {
    let fixture = InventoryFixture::new(&[], 1, 1);
    let mut session = FakeDebugSession::new(&fixture);
    session.contexts.insert(71, registers(60));
    let create = create_thread(71, 0x70);
    session.events.push_back(create);
    let delivered = DebugSession::wait(&mut session, 0)?.ok_or(RuntimeError::SessionNotOpen)?;
    DebugSession::adopt_thread(&mut session, 71, 0x70)?;
    DebugSession::resume(&mut session, &delivered, true)?;
    let exit = exit_thread(71);
    session.events.push_back(exit);
    let delivered = DebugSession::wait(&mut session, 0)?.ok_or(RuntimeError::SessionNotOpen)?;
    DebugSession::resume(&mut session, &delivered, true)?;
    assert!(
        DebugSession::context(&mut session, 71).is_err(),
        "EXPECTED RED: EXIT_THREAD currently leaves stale fake context usable"
    );
    Ok(())
}

/// EXPECTED RED: a successful business observation cannot establish that the
/// old debugger session restored registers, freed its allocation and detached.
#[test]
fn t3a_red_recovery_keeps_cleanup_ownership_separate_from_business_success(
) -> Result<(), RuntimeError> {
    let fixture = Fixture::new("owner-recovery-red");
    let (mut executor, assembly) = executor(
        &fixture,
        NativeFaults {
            reply_lost: true,
            ..NativeFaults::default()
        },
    )?;
    let (plan, _before, _index) = executor.inspect()?;
    let plan = prepared_plan(&plan, &assembly);
    let operation_id = plan["operation_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(executor.insert(&plan).is_err());
    // R1: a verified business outcome is evidence, not a release, so the
    // recovery is refused and the receipt is read back for that evidence.
    let error = executor
        .recover(
            &operation_id,
            4321,
            Some(&crate::mutation::live_fakes::FIXTURE_CREATION.to_string()),
        )
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "NATIVE_DISPATCH");
    assert!(error.message().contains("runtime owner still retained"));
    let receipt = executor.transport().store.read(&operation_id)?;
    assert_eq!(receipt["phase"], "completed", "business outcome is known");
    assert_eq!(
        receipt["released"], false,
        "EXPECTED RED: verification in a new session cannot release the old allocation/debug/thread owner"
    );
    assert_eq!(receipt["breakpoint_count"], -1);
    assert!(!executor.safe_to_shutdown());
    Ok(())
}

/// EXPECTED RED: the current success receipt collapses independent ownership
/// axes into `released=true` and zero breakpoints.
#[test]
fn t3a_red_completed_receipt_reports_each_cleanup_axis_independently() -> Result<(), RuntimeError> {
    let fixture = Fixture::new("owner-fields-red");
    let (mut executor, assembly) = executor(&fixture, NativeFaults::default())?;
    let (plan, _before, _index) = executor.inspect()?;
    let receipt = executor.insert(&prepared_plan(&plan, &assembly))?;
    assert_eq!(receipt["phase"], "completed");
    let missing = [
        "business_outcome",
        "remote_execution",
        "allocation_state",
        "debugger_state",
        "thread_cleanup",
    ]
    .into_iter()
    .filter(|key| receipt.get(*key).is_none())
    .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "EXPECTED RED: independent receipt axes are absent: {missing:?}"
    );
    assert_eq!(receipt["business_outcome"], "committed");
    assert_eq!(receipt["remote_execution"], "quiescent");
    assert_eq!(receipt["allocation_state"], "freed");
    assert_eq!(receipt["debugger_state"], "detached");
    assert!(receipt["thread_cleanup"].is_object());
    Ok(())
}
