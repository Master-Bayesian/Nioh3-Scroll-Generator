//! Ownership and fault-injection tests for the override sessions.
//!
//! Every case runs against the fault-injecting adapter: no game process, no
//! Cheat Engine, and no write to anything the product ships.

// Tests assert on outcomes directly; the crate's production lints still keep
// `expect`, `unwrap` and `panic` out of non-test code.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::fake::{FakeMemoryFactory, Faults, FAKE_MODULE_BASE};
use super::session::{
    ChallengeOverrideProfile, OverrideGroup, OverrideSession, RuntimeMutationHost, SessionSite,
    CAPACITY_RVA, CAPACITY_SIGNATURE, COUNTER_RESERVE, PC_V202_CAPACITY_RVA,
    REMOTE_ALLOCATION_SIZE,
};
use super::trampoline::OverrideProfile;
use crate::error::RuntimeError;
use crate::profile::default_pc_v2_00_02;

/// The site bytes the default PC v2.00.02 profile expects.
const SITE_RVA: u64 = 0x20DD558;
const SITE_BYTES: [u8; 5] = [0x48, 0x8B, 0x54, 0x24, 0x60];

fn seeded(faults: Faults) -> FakeMemoryFactory {
    let factory = FakeMemoryFactory::with_faults(faults);
    factory
        .state
        .borrow_mut()
        .seed(FAKE_MODULE_BASE + SITE_RVA, &SITE_BYTES);
    factory
}

fn factory_for(source: &FakeMemoryFactory) -> FakeMemoryFactory {
    FakeMemoryFactory {
        state: std::rc::Rc::clone(&source.state),
        faults: source.faults.clone(),
    }
}

fn auxiliary(
    source: &FakeMemoryFactory,
) -> Result<OverrideSession<FakeMemoryFactory>, RuntimeError> {
    OverrideSession::auxiliary(
        OverrideProfile {
            seed: 0x0102_0304,
            enemy_groups: Vec::new(),
            special_rule_keys: Some([9, 8, 7]),
            terrain_value: Some(2),
        },
        4242,
        default_pc_v2_00_02(),
        factory_for(source),
    )
}

fn challenge(
    source: &FakeMemoryFactory,
) -> Result<OverrideSession<FakeMemoryFactory>, RuntimeError> {
    let mut profile = default_pc_v2_00_02();
    profile.display_version = "PC v2.01".to_string();
    OverrideSession::challenge(
        ChallengeOverrideProfile {
            seed: 7,
            capacity: 5,
        },
        4242,
        profile,
        factory_for(source),
    )
}

#[test]
fn install_then_stop_restores_the_site_and_retires_the_trampoline() -> Result<(), RuntimeError> {
    let factory = seeded(Faults::default());
    let mut session = auxiliary(&factory)?;
    session.start()?;
    assert!(session.active());
    let hook = session.hook_address();
    let allocation = session.allocation();
    assert_eq!(hook, FAKE_MODULE_BASE + SITE_RVA);
    assert_eq!(
        session.counter_address(),
        allocation + (REMOTE_ALLOCATION_SIZE - COUNTER_RESERVE) as u64
    );
    let patch = session.patch().expect("a patch is installed").to_vec();
    {
        let state = factory.state.borrow();
        assert_eq!(state.bytes(hook, 5), patch);
        // The trampoline and the zeroed counter are written before the hook is
        // patched, and the trampoline goes out as code.
        assert_eq!(state.writes.len(), 2);
        assert_eq!(state.code_writes.len(), 1);
    }
    assert_eq!(session.hit_count()?, 0);
    session.stop()?;
    let state = factory.state.borrow();
    assert_eq!(state.bytes(hook, 5), SITE_BYTES.to_vec(), "site restored");
    assert!(state.freed.is_empty(), "a live trampoline is never freed");
    assert_eq!(state.closed, 1, "the write handle is closed");
    assert!(session.identity().is_some(), "identity was verified");
    Ok(())
}

#[test]
fn hit_count_tracks_the_counter_the_trampoline_owns() -> Result<(), RuntimeError> {
    let factory = seeded(Faults::default());
    let mut session = auxiliary(&factory)?;
    session.start()?;
    factory
        .state
        .borrow_mut()
        .seed(session.counter_address(), &42u64.to_le_bytes());
    assert_eq!(session.hit_count()?, 42);
    session.stop()?;
    assert_eq!(session.hit_count()?, 0, "no hook, no hits");
    Ok(())
}

#[test]
fn a_signature_mismatch_fails_closed_before_any_write() -> Result<(), RuntimeError> {
    let factory = FakeMemoryFactory::new();
    factory
        .state
        .borrow_mut()
        .seed(FAKE_MODULE_BASE + SITE_RVA, &[0u8; 5]);
    let mut session = auxiliary(&factory)?;
    let error = session.start().expect_err("a wrong site byte is refused");
    assert_eq!(
        error,
        RuntimeError::SignatureMismatch {
            site: "descriptor_complete".to_string(),
            rva: SITE_RVA,
        }
    );
    let state = factory.state.borrow();
    assert!(state.writes.is_empty() && state.code_writes.is_empty());
    assert!(state.allocated.is_empty());
    assert_eq!(state.closed, 1);
    Ok(())
}

#[test]
fn a_recycled_pid_is_refused_before_any_write() -> Result<(), RuntimeError> {
    let factory = seeded(Faults {
        creation_filetime: 100,
        creation_filetime_override: Some(200),
        ..Faults::default()
    });
    let mut session = auxiliary(&factory)?;
    let error = session.start().expect_err("a replaced process is refused");
    assert_eq!(error, RuntimeError::ProcessInstanceChanged { pid: 4242 });
    let state = factory.state.borrow();
    assert!(state.writes.is_empty() && state.allocated.is_empty());
    Ok(())
}

#[test]
fn a_failed_trampoline_write_releases_the_allocation_and_leaves_the_site(
) -> Result<(), RuntimeError> {
    let factory = seeded(Faults {
        fail_write_call: Some(1),
        ..Faults::default()
    });
    let mut session = auxiliary(&factory)?;
    let error = session.start().expect_err("the code write fails");
    assert!(matches!(error, RuntimeError::MemoryWrite { .. }));
    let state = factory.state.borrow();
    assert_eq!(
        state.bytes(FAKE_MODULE_BASE + SITE_RVA, 5),
        SITE_BYTES.to_vec()
    );
    assert_eq!(state.freed.len(), 1, "an unpatched trampoline is released");
    assert_eq!(state.closed, 1);
    Ok(())
}

#[test]
fn a_failed_hook_write_keeps_the_trampoline_and_leaves_the_site() -> Result<(), RuntimeError> {
    // The patch is the first code write, so failing it fails the install.
    let factory = seeded(Faults {
        fail_write_code_call: Some(1),
        ..Faults::default()
    });
    let mut session = auxiliary(&factory)?;
    let error = session.start().expect_err("the hook write fails");
    assert!(matches!(error, RuntimeError::MemoryWrite { .. }));
    let state = factory.state.borrow();
    assert_eq!(
        state.bytes(FAKE_MODULE_BASE + SITE_RVA, 5),
        SITE_BYTES.to_vec()
    );
    assert!(
        state.freed.is_empty(),
        "a trampoline that was written is never freed"
    );
    assert_eq!(state.closed, 1);
    Ok(())
}

#[test]
fn an_external_rewrite_of_the_hook_is_never_overwritten() -> Result<(), RuntimeError> {
    let factory = seeded(Faults::default());
    let mut session = auxiliary(&factory)?;
    session.start()?;
    let hook = session.hook_address();
    let foreign = [0xCC, 0xCC, 0xCC, 0xCC, 0xCC];
    factory.state.borrow_mut().seed(hook, &foreign);
    let error = session.stop().expect_err("a foreign hook is refused");
    assert_eq!(error, RuntimeError::HookModified { address: hook });
    let state = factory.state.borrow();
    assert_eq!(
        state.bytes(hook, 5),
        foreign.to_vec(),
        "nothing overwritten"
    );
    assert!(state.freed.is_empty(), "nothing freed");
    Ok(())
}

#[test]
fn a_read_failure_while_the_process_lives_retains_the_owner() -> Result<(), RuntimeError> {
    // Read 1 is the signature check; read 2 is stop's first read.
    let factory = seeded(Faults {
        fail_read_call: Some(2),
        ..Faults::default()
    });
    let mut session = auxiliary(&factory)?;
    session.start()?;
    let error = session
        .stop()
        .expect_err("an unreadable hook is not a clean stop");
    assert!(matches!(error, RuntimeError::HookRestoreUnverified { .. }));
    let state = factory.state.borrow();
    assert_eq!(
        state.closed, 0,
        "the handle stays with the retained ownership"
    );
    assert!(state.freed.is_empty());
    Ok(())
}

#[test]
fn a_confirmed_process_exit_releases_the_owner() -> Result<(), RuntimeError> {
    let factory = seeded(Faults {
        fail_read_call: Some(2),
        exited: true,
        ..Faults::default()
    });
    let mut session = auxiliary(&factory)?;
    session.start()?;
    session.stop()?;
    let state = factory.state.borrow();
    assert_eq!(state.closed, 1);
    assert!(state.freed.is_empty(), "the target released its own memory");
    Ok(())
}

#[test]
fn an_unreachable_exit_query_is_not_an_exit() -> Result<(), RuntimeError> {
    let factory = seeded(Faults {
        fail_read_call: Some(2),
        exit_query_fails: true,
        ..Faults::default()
    });
    let mut session = auxiliary(&factory)?;
    session.start()?;
    assert!(session.stop().is_err(), "unknown is not exited");
    assert_eq!(factory.state.borrow().closed, 0);
    Ok(())
}

#[test]
fn the_challenge_session_hooks_the_capacity_site() -> Result<(), RuntimeError> {
    let factory = FakeMemoryFactory::new();
    factory
        .state
        .borrow_mut()
        .seed(FAKE_MODULE_BASE + CAPACITY_RVA, &CAPACITY_SIGNATURE);
    let mut session = challenge(&factory)?;
    assert_eq!(session.site(), SessionSite::ChallengeCapacity);
    session.start()?;
    assert_eq!(session.hook_address(), FAKE_MODULE_BASE + CAPACITY_RVA);
    session.stop()?;
    assert_eq!(
        factory
            .state
            .borrow()
            .bytes(FAKE_MODULE_BASE + CAPACITY_RVA, 5),
        CAPACITY_SIGNATURE.to_vec()
    );
    Ok(())
}

/// The PC v2.02 getter body is the PC v2.01 body relocated, so the session
/// hooks the v2.02 address with the same verified entry bytes.
#[test]
fn the_pc_v2_02_challenge_session_hooks_the_relocated_getter() -> Result<(), RuntimeError> {
    let factory = FakeMemoryFactory::new();
    factory
        .state
        .borrow_mut()
        .seed(FAKE_MODULE_BASE + PC_V202_CAPACITY_RVA, &CAPACITY_SIGNATURE);
    let mut profile = default_pc_v2_00_02();
    profile.display_version = "PC v2.02".to_string();
    let mut session = OverrideSession::challenge(
        ChallengeOverrideProfile {
            seed: 7,
            capacity: 5,
        },
        4242,
        profile,
        factory_for(&factory),
    )?;
    session.start()?;
    assert_eq!(
        session.hook_address(),
        FAKE_MODULE_BASE + PC_V202_CAPACITY_RVA
    );
    session.stop()?;
    assert_eq!(
        factory
            .state
            .borrow()
            .bytes(FAKE_MODULE_BASE + PC_V202_CAPACITY_RVA, 5),
        CAPACITY_SIGNATURE.to_vec()
    );
    Ok(())
}

#[test]
fn a_challenge_session_requires_verified_pc_v2_01() {
    let outcome = OverrideSession::challenge(
        ChallengeOverrideProfile {
            seed: 1,
            capacity: 4,
        },
        4242,
        default_pc_v2_00_02(),
        FakeMemoryFactory::new(),
    );
    assert!(
        outcome.is_err(),
        "the capacity getter is only verified on PC v2.01 and PC v2.02"
    );
    let error = outcome.err().unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(
        error,
        RuntimeError::InvalidOverrideProfile {
            detail: "Challenge capacity override requires verified PC v2.01 or PC v2.02"
                .to_string(),
        }
    );
}

#[test]
fn a_group_stops_every_session_and_closes_each_owner() -> Result<(), RuntimeError> {
    // The shipped host installs one session per verified site, so the group
    // pairs the descriptor hook with the challenge capacity getter.
    let factory = seeded(Faults::default());
    factory
        .state
        .borrow_mut()
        .seed(FAKE_MODULE_BASE + CAPACITY_RVA, &CAPACITY_SIGNATURE);
    let mut aux = auxiliary(&factory)?;
    let mut challenge = challenge(&factory)?;
    challenge.start()?;
    aux.start()?;
    let mut group = OverrideGroup::new(vec![aux, challenge]);
    assert_eq!(group.hit_count()?, 0);
    group.stop()?;
    let state = factory.state.borrow();
    assert_eq!(state.closed, 2);
    assert_eq!(
        state.bytes(FAKE_MODULE_BASE + SITE_RVA, 5),
        SITE_BYTES.to_vec()
    );
    Ok(())
}

#[test]
fn the_host_retains_ownership_when_a_rollback_cannot_be_confirmed() -> Result<(), RuntimeError> {
    // The hook write fails, so the site is untouched; the rollback read then
    // fails too, so the host must keep the owner instead of reporting clean.
    let factory = seeded(Faults {
        fail_write_code_call: Some(1),
        fail_reads_after: Some(2),
        ..Faults::default()
    });
    let mut host = RuntimeMutationHost::new();
    let session = auxiliary(&factory)?;
    let outcome = host.start_override(vec![session]);
    assert!(outcome.is_err(), "install must fail");
    let error = outcome.err().unwrap_or(RuntimeError::RuntimeBusy);
    assert!(matches!(error, RuntimeError::MemoryWrite { .. }));
    let status = host.status();
    assert!(
        status.override_state == "armed_no_hit" || status.override_state == "unknown",
        "ambiguous ownership is retained, got {}",
        status.override_state
    );
    assert!(!status.safe_to_shutdown, "ownership retained");
    Ok(())
}

#[test]
fn the_host_publishes_hits_and_shutdown_safety() -> Result<(), RuntimeError> {
    let factory = seeded(Faults::default());
    let mut host = RuntimeMutationHost::new();
    let session = auxiliary(&factory)?;
    let started = host.start_override(vec![session])?;
    assert_eq!(started.override_state, "armed_no_hit");
    assert_eq!(started.hit_count, 0);
    assert!(!started.safe_to_shutdown);
    assert!(
        host.start_override(Vec::new()).is_err(),
        "an empty selection has nothing to install"
    );
    let stopped = host.stop_override()?;
    assert_eq!(stopped.override_state, "stopped");
    assert!(stopped.safe_to_shutdown);
    Ok(())
}
