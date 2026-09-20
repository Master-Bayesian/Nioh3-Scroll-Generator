//! Bounded Windows acceptance for the runtime-owner state protocol.
//!
//! The test owns and spawns `runtime_mutation_helper`; it never discovers,
//! starts or attaches to the game. Deterministic fake tests cover injected
//! partial-restore, detach and foreign-exception failures. This test adds the
//! real Win32 evidence that event handles may be retired and numerically
//! reused, contexts require a pending stop event, handled breakpoint events
//! resume the target, and detach leaves the helper alive.
#![cfg(all(feature = "test-helper", windows))]

use nioh3_runtime::mutation::win_session::{
    DebugSession, RemoteSession, WindowsDebugSession, WindowsRemoteSession, EXCEPTION_BREAKPOINT,
    WAIT_OBJECT_0,
};
use nioh3_runtime::RuntimeError;
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Helper {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pid: u32,
    page: u64,
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
            page: 0,
        };
        let ready = helper.line()?;
        let fields = ready.trim().split('\t').collect::<Vec<_>>();
        if fields.first().copied() != Some("ready") {
            return Err(RuntimeError::SessionNotOpen);
        }
        helper.pid = fields
            .get(1)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        helper.page = fields
            .get(3)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        if helper.pid == 0 || helper.page == 0 {
            return Err(RuntimeError::SessionNotOpen);
        }
        Ok(helper)
    }

    fn line(&mut self) -> Result<String, RuntimeError> {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|error| RuntimeError::Io {
                path: "runtime-owner helper stdout".to_string(),
                detail: error.to_string(),
            })?;
        Ok(line)
    }

    fn command(&mut self, command: &str) -> Result<String, RuntimeError> {
        self.stdin
            .write_all(format!("{command}\n").as_bytes())
            .and_then(|()| self.stdin.flush())
            .map_err(|error| RuntimeError::Io {
                path: "runtime-owner helper stdin".to_string(),
                detail: error.to_string(),
            })?;
        self.line()
    }

    fn quit(&mut self) {
        let _ = self.stdin.write_all(b"quit\n");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn drain_to_initial_breakpoint(
    session: &mut WindowsDebugSession,
) -> Result<(u32, nioh3_runtime::mutation::win_session::DebugEvent), RuntimeError> {
    for _ in 0..64 {
        let Some(event) = session.wait(1_000)? else {
            continue;
        };
        if event.is_create_process() || event.is_create_thread() {
            if let Some(handle) = event.thread_handle {
                session.adopt_thread(event.tid, handle)?;
            }
            session.resume(&event, true)?;
            continue;
        }
        if event.exception_code == Some(EXCEPTION_BREAKPOINT) {
            return Ok((event.tid, event));
        }
        session.resume(&event, false)?;
    }
    Err(RuntimeError::SessionNotOpen)
}

#[test]
fn controlled_helper_accepts_runtime_owner_lifecycle() -> Result<(), RuntimeError> {
    let mut helper = Helper::spawn()?;
    let image = std::path::Path::new(env!("CARGO_BIN_EXE_runtime_mutation_helper"))
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut session = WindowsDebugSession::open(helper.pid, &image)?;
    session.attach()?;

    let (main_tid, initial_breakpoint) = drain_to_initial_breakpoint(&mut session)?;
    assert_ne!(session.context(main_tid)?.rip, 0);
    session.resume(&initial_breakpoint, true)?;
    assert!(
        session.context(main_tid).is_err(),
        "a running thread has no stopped-event context authority"
    );

    let mut remote = WindowsRemoteSession::open(helper.pid)?;
    let mut event_handles = BTreeSet::new();
    let mut reused_handle = None;
    let mut retired = 0u32;
    for _ in 0..64 {
        let remote_handle = remote.create_remote_thread(helper.page)?;
        let mut created_tid = None;
        for _ in 0..64 {
            let Some(event) = session.wait(1_000)? else {
                continue;
            };
            if event.is_create_thread() || event.is_create_process() {
                if let Some(handle) = event.thread_handle {
                    if !event_handles.insert(handle) {
                        reused_handle = Some(handle);
                    }
                    session.adopt_thread(event.tid, handle)?;
                    assert!(
                        session.adopt_thread(event.tid, handle).is_err(),
                        "a live duplicate adoption of tid {} must be rejected",
                        event.tid
                    );
                    // The binding's identity probe must work on a real
                    // debug-event handle, or a stale record could never be
                    // reclaimed on the live path.
                    let observed_tid = unsafe {
                        windows_sys::Win32::System::Threading::GetThreadId(
                            handle as *mut std::ffi::c_void,
                        )
                    };
                    assert_eq!(
                        observed_tid, event.tid,
                        "GetThreadId must name the create event's own thread"
                    );
                    created_tid = Some(event.tid);
                }
                session.resume(&event, true)?;
                continue;
            }
            if event.is_exit_thread() && created_tid == Some(event.tid) {
                let tid = event.tid;
                session.resume(&event, true)?;
                assert!(
                    session.context(tid).is_err(),
                    "continued EXIT_THREAD retires the matching instance"
                );
                retired += 1;
                break;
            }
            if event.is_exception() {
                session.resume(&event, false)?;
            } else {
                session.resume(&event, true)?;
            }
        }
        assert_eq!(remote.wait_thread(remote_handle, 1_000)?, WAIT_OBJECT_0);
        remote.close_thread(remote_handle);
        if reused_handle.is_some() {
            break;
        }
    }
    assert!(retired >= 2, "at least two thread instances were retired");
    assert!(
        reused_handle.is_some(),
        "the bounded helper run must observe numeric debug-event handle reuse"
    );

    session.debug_break()?;
    let mut barrier = None;
    for _ in 0..64 {
        let Some(event) = session.wait(1_000)? else {
            continue;
        };
        if event.is_create_thread() || event.is_create_process() {
            if let Some(handle) = event.thread_handle {
                session.adopt_thread(event.tid, handle)?;
            }
            session.resume(&event, true)?;
            continue;
        }
        if event.is_exit_thread() {
            session.resume(&event, true)?;
            continue;
        }
        if event.exception_code == Some(EXCEPTION_BREAKPOINT) {
            assert_ne!(session.context(event.tid)?.rip, 0);
            barrier = Some(event);
            break;
        }
        session.resume(&event, event.exception_code.is_none())?;
    }
    let barrier = barrier.ok_or(RuntimeError::SessionNotOpen)?;
    session.restore_threads()?;
    session.resume(&barrier, true)?;
    assert!(session.context(barrier.tid).is_err());
    session.detach()?;

    assert!(
        helper.command("bytes")?.starts_with("bytes\t"),
        "handled breakpoint, cleanup and detach leave the target alive"
    );
    helper.quit();
    Ok(())
}
