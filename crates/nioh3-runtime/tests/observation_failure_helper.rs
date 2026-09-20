//! Real-binding acceptance for the RW05 observation-failure rule.
//!
//! The Pro RF03 finding is that a failed `WaitForSingleObject` was converted
//! into a thread-exited proof: `restore_threads` retired the record as `exited`
//! and cleared its original debug state, and the shared settled predicate then
//! accepted that as a completed cleanup.
//!
//! This test drives the *production* binding — the real `WindowsDebugSession`
//! over a helper process the test itself spawns — and makes one adopted handle
//! receive an injected failure return through the test-only observation seam.
//! Everything else is the real syscall path: real debug attach, real create events, real
//! `GetThreadId`, real `GetThreadContext`/`SetThreadContext`. The helper is the
//! only target; Nioh 3, a game, and any save are never touched.
//!
//! Run with:
//!   cargo test --features test-observation --test observation_failure_helper
#![cfg(all(feature = "test-observation", windows))]

use nioh3_runtime::mutation::win_session::{
    DebugSession, ObservationFault, WindowsDebugSession, EXCEPTION_BREAKPOINT,
};
use nioh3_runtime::RuntimeError;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

/// The test-only observation seams are process-global atomics. Serialize the
/// real-binding cases so parallel test execution cannot replace another case's
/// selected handle and turn the assertion into a scheduling race.
static OBSERVATION_TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock_observation_test() -> std::sync::MutexGuard<'static, ()> {
    OBSERVATION_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Helper {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pid: u32,
}

impl Helper {
    fn spawn() -> Result<Self, RuntimeError> {
        let path = env!("CARGO_BIN_EXE_runtime_mutation_helper");
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| RuntimeError::Io {
                path: path.to_string(),
                detail: error.to_string(),
            })?;
        let stdin = child.stdin.take().ok_or(RuntimeError::SessionNotOpen)?;
        let stdout = child.stdout.take().ok_or(RuntimeError::SessionNotOpen)?;
        let mut helper = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            pid: 0,
        };
        let ready = helper.line()?;
        if ready.trim().split('\t').next() != Some("ready") {
            return Err(RuntimeError::SessionNotOpen);
        }
        helper.pid = ready
            .trim()
            .split('\t')
            .nth(1)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        if helper.pid == 0 {
            return Err(RuntimeError::SessionNotOpen);
        }
        Ok(helper)
    }

    fn line(&mut self) -> Result<String, RuntimeError> {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|error| RuntimeError::Io {
                path: "observation helper stdout".to_string(),
                detail: error.to_string(),
            })?;
        Ok(line)
    }

    fn quit(&mut self) {
        let _ = self.stdin.write_all(b"quit\n");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }

    /// Close the command loop without waiting for the target to finish.
    fn request_quit(&mut self) {
        let _ = self.stdin.write_all(b"quit\n");
        let _ = self.stdin.flush();
    }

    /// Reap a target whose exit the debugger has already served.
    fn reap(&mut self) {
        let _ = self.child.wait();
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        ObservationFault::clear();
        // Never wait here. A retained, unresolved owner can leave the target
        // stopped inside a debug event the debugger never continues, and such a
        // target cannot finish dying until this process itself releases it.
        let _ = self.child.kill();
    }
}

/// Attach, adopt the helper's own thread from its create event, arm it, and
/// stop at a breakpoint so a real stopped-event barrier exists.
fn attach_and_arm(
    session: &mut WindowsDebugSession,
) -> Result<(u32, nioh3_runtime::mutation::win_session::DebugEvent), RuntimeError> {
    session.attach()?;
    let mut adopted_tid = None;
    for _ in 0..64 {
        let Some(event) = session.wait(1_000)? else {
            continue;
        };
        if event.is_create_thread() || event.is_create_process() {
            if let Some(handle) = event.thread_handle {
                session.adopt_thread(event.tid, handle)?;
                adopted_tid.get_or_insert(event.tid);
            }
            session.resume(&event, true)?;
            continue;
        }
        if event.exception_code == Some(EXCEPTION_BREAKPOINT) {
            let tid = adopted_tid.ok_or(RuntimeError::SessionNotOpen)?;
            session.arm_thread(tid, 0x1_0000, 0x1_0007)?;
            return Ok((tid, event));
        }
        session.resume(&event, event.exception_code.is_none())?;
    }
    Err(RuntimeError::SessionNotOpen)
}

/// The adopted handle value of the single owned record, read from the real
/// owner report.
fn adopted_handle(session: &WindowsDebugSession, tid: u32) -> Result<u64, RuntimeError> {
    session
        .observed_threads()
        .iter()
        .find(|thread| thread.tid == tid)
        .map(|thread| thread.handle)
        .ok_or(RuntimeError::SessionNotOpen)
}

/// Continue every pending event until the target runs freely again.
///
/// A retained, unresolved owner keeps the debugger attached, and a target left
/// stopped inside an event nobody continues can never be released, so the test
/// has to keep serving the debugger's own event loop.
fn serve_until_idle(session: &mut WindowsDebugSession) -> Result<(), RuntimeError> {
    for _ in 0..256 {
        let Some(event) = session.wait(300)? else {
            return Ok(());
        };
        let handled = event.exception_code.is_none();
        session.resume(&event, handled)?;
    }
    Err(RuntimeError::SessionNotOpen)
}

/// Close the target's command loop and serve its exit events, so both sides are
/// released without pretending the retained cleanup completed.
fn release_target(
    session: &mut WindowsDebugSession,
    helper: &mut Helper,
) -> Result<(), RuntimeError> {
    helper.request_quit();
    for _ in 0..256 {
        let Some(event) = session.wait(500)? else {
            continue;
        };
        let exit_process = event.is_exit_process();
        let handled = event.exception_code.is_none();
        session.resume(&event, handled)?;
        if exit_process {
            helper.reap();
            return Ok(());
        }
    }
    Err(RuntimeError::SessionNotOpen)
}

/// RW05 regression. A failed wait on an armed thread must keep the record
/// unresolved and retain its cleanup obligation, instead of retiring it as
/// `exited` and letting the settled predicate accept a completed cleanup.
#[test]
fn a_failed_wait_keeps_the_thread_owner_and_never_settles_cleanup() -> Result<(), RuntimeError> {
    let _test_guard = lock_observation_test();
    ObservationFault::clear();
    let mut helper = Helper::spawn()?;
    let image = std::path::Path::new(env!("CARGO_BIN_EXE_runtime_mutation_helper"))
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut session = WindowsDebugSession::open(helper.pid, &image)?;
    let (tid, barrier) = attach_and_arm(&mut session)?;
    let handle = adopted_handle(&session, tid)?;
    assert_ne!(handle, 0, "the create event supplied a real thread handle");

    // Inject WAIT_FAILED for this real handle. Every other call still runs
    // against the real OS handle; the OS did not independently produce this failure.
    ObservationFault::make_unobservable(handle);
    let error = match session.restore_threads() {
        Ok(()) => {
            return Err(RuntimeError::NativeDispatch {
                detail: "a failed observation was reported as a completed restore".to_string(),
            })
        }
        Err(error) => error,
    };
    ObservationFault::clear();
    assert_eq!(error.code(), "NATIVE_DISPATCH");
    assert!(
        error.message().contains("could not be identified")
            && error.message().contains("WaitForSingleObject"),
        "the refusal names the observation failure: {error}"
    );
    assert!(
        error.message().contains("last_error="),
        "the refusal carries the failing GetLastError category: {error}"
    );

    let snapshot = session.observed_threads();
    let record = match snapshot.iter().find(|thread| thread.tid == tid) {
        Some(record) => record.clone(),
        None => {
            let _ = session.resume(&barrier, true);
            session.detach()?;
            return Err(RuntimeError::SessionNotOpen);
        }
    };

    // The failed observation is now recorded, and the record lost the authority
    // of its numeric handle: a context read through it is refused.
    let context_refusal = match session.context(tid) {
        Ok(_) => {
            return Err(RuntimeError::NativeDispatch {
                detail: "a context read through an unobserved handle was allowed".to_string(),
            })
        }
        Err(error) => error,
    };

    // The pending barrier event is still owned, so release it explicitly. The
    // target stays attached because the retained record still owes its cleanup:
    // refusing to detach is the fail-closed half of this rule.
    let resume = session.resume(&barrier, true);
    let drained = serve_until_idle(&mut session);
    // A clean detach is refused while the retained record still owes its debug
    // registers, so both sides are released through the target's own exit.
    let detach = session.detach();
    let released = release_target(&mut session, &mut helper);
    drop(session);

    resume?;
    drained?;
    released?;
    let detach_refusal = match detach {
        Ok(()) => {
            return Err(RuntimeError::NativeDispatch {
                detail: "a clean detach was allowed with an unresolved cleanup".to_string(),
            })
        }
        Err(error) => error,
    };
    assert_eq!(
        detach_refusal.code(),
        "HOOK_RESTORE_UNVERIFIED",
        "the refusal names the unrestored debug registers: {detach_refusal}"
    );

    assert_eq!(
        record.cleanup_state, "unknown",
        "the failed observation is recorded as unresolved, not as an exit"
    );
    assert_ne!(
        record.cleanup_state, "exited",
        "WAIT_FAILED must never be converted into a thread-exited proof"
    );
    assert_eq!(
        record.handle, handle,
        "the numeric handle is still retained as the unresolved owner's provenance"
    );
    let recorded_error = record
        .error
        .as_deref()
        .ok_or(RuntimeError::SessionNotOpen)?;
    assert!(
        recorded_error.contains("WAIT_FAILED") && recorded_error.contains("last_error="),
        "the error category and last_error are preserved: {recorded_error}"
    );
    assert_eq!(
        context_refusal.code(),
        "NATIVE_DISPATCH",
        "the retained record is still present: {context_refusal}"
    );
    assert!(
        context_refusal.message().contains("unverified"),
        "the refusal names the untrusted handle: {context_refusal}"
    );
    Ok(())
}

/// The retained owner is not a wedge, and it never settles on the failed wait.
///
/// Once the OS answers an observation of the same instance again, the pass can
/// finish the cleanup it refused before. A create event that would take the
/// unresolved record's numeric value or tid needs a real collision the bounded
/// helper cannot construct, so that branch is owned by the in-crate
/// `resolve_adoption` oracle instead of by this test.
#[test]
fn a_retained_record_settles_only_through_a_later_real_observation() -> Result<(), RuntimeError> {
    let _test_guard = lock_observation_test();
    ObservationFault::clear();
    let mut helper = Helper::spawn()?;
    let image = std::path::Path::new(env!("CARGO_BIN_EXE_runtime_mutation_helper"))
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut session = WindowsDebugSession::open(helper.pid, &image)?;
    let (tid, barrier) = attach_and_arm(&mut session)?;
    let handle = adopted_handle(&session, tid)?;

    ObservationFault::make_unobservable(handle);
    let restore = session.restore_threads();
    ObservationFault::clear();
    assert!(restore.is_err(), "the failed observation is not a restore");

    let unresolved = session.observed_threads();
    assert_eq!(
        unresolved
            .iter()
            .find(|thread| thread.tid == tid)
            .map(|thread| thread.cleanup_state),
        Some("unknown"),
        "the failed observation leaves the record unresolved"
    );

    // A later successful wait proves liveness, but an injected GetThreadId
    // failure still cannot prove that this numeric handle names the recorded
    // instance. The cleanup owner remains, while context access stays denied.
    ObservationFault::make_identity_unreadable(handle);
    let identity_failure = session.restore_threads();
    assert!(
        identity_failure.is_err(),
        "unknown identity is not an authorization"
    );
    let identity_error = identity_failure.err().ok_or(RuntimeError::SessionNotOpen)?;
    assert!(
        identity_error.message().contains("GetThreadId"),
        "the refusal preserves the failed identity query: {identity_error}"
    );
    let context_refusal = session.context(tid);
    assert!(
        context_refusal.is_err(),
        "an identity-query failure must not restore context access"
    );
    let still_unresolved = session.observed_threads();
    let unresolved_record = still_unresolved
        .iter()
        .find(|thread| thread.tid == tid)
        .ok_or(RuntimeError::SessionNotOpen)?;
    assert_eq!(unresolved_record.cleanup_state, "unknown");
    assert!(
        unresolved_record
            .error
            .as_deref()
            .is_some_and(|error| error.contains("GetThreadId")),
        "the owner report preserves the identity-query failure category"
    );
    ObservationFault::clear();

    // The seam is clear now, so the next observation of this instance is a real
    // one. The same pending barrier still authorizes the pass, and the verified
    // live thread lets it finish the cleanup it refused before.
    session.restore_threads()?;
    let settled = session.observed_threads();
    assert_eq!(
        settled
            .iter()
            .find(|thread| thread.tid == tid)
            .map(|thread| thread.cleanup_state),
        Some("original_restored"),
        "a real observation completes the cleanup the failed one refused"
    );

    // The cleanup is complete, so the debugger can release the target cleanly.
    session.resume(&barrier, true)?;
    session.detach()?;
    drop(session);
    helper.quit();
    Ok(())
}

/// Negative control. A real unobstructed observation of the same live instance
/// still permits restoring its original debug registers. Without this control
/// the previous test would also pass if the binding refused every restore.
#[test]
fn a_verified_live_instance_still_restores_its_original_debug_state() -> Result<(), RuntimeError> {
    let _test_guard = lock_observation_test();
    ObservationFault::clear();
    let mut helper = Helper::spawn()?;
    let image = std::path::Path::new(env!("CARGO_BIN_EXE_runtime_mutation_helper"))
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut session = WindowsDebugSession::open(helper.pid, &image)?;
    let (tid, barrier) = attach_and_arm(&mut session)?;
    let handle = adopted_handle(&session, tid)?;

    // The observation seam is deliberately left clear, so this run uses the
    // real `WaitForSingleObject` result for the real handle. The control proves
    // the refusal in the previous test is specific to a failed observation and
    // not a blanket refusal to restore anything.
    let owner_report = session.observed_threads();
    assert_eq!(
        owner_report
            .iter()
            .find(|thread| thread.tid == tid)
            .map(|thread| thread.cleanup_state),
        Some("armed"),
        "a live armed thread keeps its retained cleanup state before restoration"
    );
    assert_ne!(handle, 0);

    // A real, unobstructed restore succeeds and settles this one record.
    session.restore_threads()?;
    let after = session.observed_threads();
    assert_eq!(
        after
            .iter()
            .find(|thread| thread.tid == tid)
            .map(|thread| thread.cleanup_state),
        Some("original_restored"),
        "a real restore is the only thing that completes this record's cleanup"
    );

    session.resume(&barrier, true)?;
    session.detach()?;
    drop(session);
    helper.quit();
    Ok(())
}
