//! Real Windows API proof for the batch oracle and the debug-session binding.
//!
//! The injected transports prove the ownership state machines; they cannot
//! prove the syscalls. This test drives both real paths against a helper process
//! the test itself spawns:
//!
//! * the batch oracle opens the helper with the shipped mask, verifies its
//!   signature sites, allocates the one code-plus-buffers region, writes the
//!   batch wrapper, starts it with a real `CreateRemoteThread`, waits, reads the
//!   exit code and reads the produced records back; a second call is deliberately
//!   timed out and must be retired to a waiter that frees only after the thread
//!   exits;
//! * the debug session attaches as a debugger, receives the create-process and
//!   create-thread events, consumes the initial breakpoint, reads a real
//!   `CONTEXT` through 16-byte aligned storage, writes and re-reads the debug
//!   registers, restores them and detaches.
//!
//! It never touches Nioh 3, a game, a save or a debugger session on any other
//! process. The helper is the only target, and it is the same owned binary the
//! mutation tests use.
#![cfg(all(feature = "test-helper", windows))]

use nioh3_runtime::mutation::native_abi::{
    LiveAddLayout, PC_V201_LIVE_ADD, RUNTIME_ACCESS, SCROLL_RECORD_SIZE,
};
use nioh3_runtime::mutation::oracle::NativeBatchOracle;
use nioh3_runtime::mutation::win_session::{
    DebugSession, WindowsDebugSession, WindowsRemoteSession, EXCEPTION_BREAKPOINT, WAIT_INFINITE,
};
use nioh3_runtime::profile::{NativeRuntimeProfile, ProfileSite, SIGNATURE_SITE_NAMES};
use nioh3_runtime::RuntimeError;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

/// The real Win32 helper cases exercise debugger and remote-thread ownership.
/// Keep those OS-level lifecycles sequential inside this integration binary so
/// one case cannot invalidate another case's inherited pipe or debug handles.
static LIVE_HELPER_TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock_live_helper_test() -> std::sync::MutexGuard<'static, ()> {
    LIVE_HELPER_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Helper {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pid: u32,
    module_base: u64,
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
        let stdout = BufReader::new(child.stdout.take().ok_or(RuntimeError::SessionNotOpen)?);
        let mut helper = Self {
            child,
            stdin,
            stdout,
            pid: 0,
            module_base: 0,
            page: 0,
        };
        let fields: Vec<String> = helper
            .line()?
            .trim()
            .split('\t')
            .map(str::to_string)
            .collect();
        if fields.first().map(String::as_str) != Some("ready") {
            return Err(RuntimeError::SessionNotOpen);
        }
        helper.pid = fields
            .get(1)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        helper.module_base = fields
            .get(2)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        helper.page = fields
            .get(3)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        Ok(helper)
    }

    fn line(&mut self) -> Result<String, RuntimeError> {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|error| RuntimeError::Io {
                path: "helper stdout".to_string(),
                detail: error.to_string(),
            })?;
        Ok(line)
    }

    fn command(&mut self, text: &str) -> Result<String, RuntimeError> {
        self.stdin
            .write_all(format!("{text}\n").as_bytes())
            .and_then(|()| self.stdin.flush())
            .map_err(|error| RuntimeError::Io {
                path: "helper stdin".to_string(),
                detail: error.to_string(),
            })?;
        self.line()
    }

    /// `oracle` / `oracle-slow`: the owned function the oracle calls, its
    /// record template and the exact bytes standing at the site.
    fn oracle(&mut self, slow: bool) -> Result<(u64, u64, Vec<u8>), RuntimeError> {
        let line = self.command(if slow { "oracle-slow" } else { "oracle" })?;
        let fields: Vec<&str> = line.trim().split('\t').collect();
        if fields.first() != Some(&"oracle") {
            return Err(RuntimeError::AllocationUnavailable {
                address: 0,
                size: 0,
            });
        }
        let page = fields
            .get(1)
            .and_then(|value| u64::from_str_radix(value, 16).ok())
            .unwrap_or(0);
        let template = fields
            .get(2)
            .and_then(|value| u64::from_str_radix(value, 16).ok())
            .unwrap_or(0);
        let code = fields
            .get(3)
            .and_then(|value| {
                value
                    .as_bytes()
                    .chunks(2)
                    .map(|pair| {
                        std::str::from_utf8(pair)
                            .ok()
                            .and_then(|text| u8::from_str_radix(text, 16).ok())
                    })
                    .collect::<Option<Vec<u8>>>()
            })
            .unwrap_or_default();
        Ok((page, template, code))
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

/// Every verified site points at the helper's own stand-in, which is the
/// test-helper-only factory the production path never takes.
fn helper_profile(page: u64, module_base: u64, signature: &[u8]) -> NativeRuntimeProfile {
    let site = |name: &'static str| ProfileSite {
        name,
        rva: page - module_base,
        signature: signature.to_vec(),
    };
    NativeRuntimeProfile {
        display_version: "PC v2.01".to_string(),
        canonicalize: site("canonicalize"),
        finalize_effect: site("finalize_effect"),
        descriptor_complete: site("descriptor_complete"),
        native_signatures: SIGNATURE_SITE_NAMES
            .iter()
            .map(|name| ProfileSite {
                name,
                rva: page - module_base,
                signature: signature.to_vec(),
            })
            .collect(),
        playthrough_selector_pointer_rva: 0,
    }
}

#[test]
fn the_real_oracle_path_runs_a_batch_and_reads_it_back() -> Result<(), RuntimeError> {
    let _test_guard = lock_live_helper_test();
    let mut helper = Helper::spawn()?;
    let (page, template, code) = helper.oracle(false)?;
    assert!(page > 0 && template > 0);
    assert!(!code.is_empty(), "the helper reports the bytes at its site");
    let mut source = vec![0u8; SCROLL_RECORD_SIZE];
    source[0] = 0x82;
    source[1] = 0x1E;
    source[0x18] = 0x02;
    source[0x1A] = 0x80;
    source[0x20..0x24].copy_from_slice(&0x0BAD_F00Du32.to_le_bytes());
    source[0x28..0x30].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    source[0x30] = 4;
    source[0x31] = 4;

    let mut oracle = NativeBatchOracle::new(
        helper.pid,
        helper.module_base,
        helper_profile(page, helper.module_base, &code),
        4,
        false,
    )?;
    oracle.open(Box::new(WindowsRemoteSession::open(helper.pid)?))?;
    assert_eq!(
        nioh3_runtime::mutation::win_session::LIVE_ADD_ACCESS,
        RUNTIME_ACCESS,
        "the oracle declares the shipped mask"
    );
    let records = oracle.generate(&[source.clone(), source.clone()], 30_000)?;
    assert_eq!(records.len(), 2);
    for record in &records {
        assert_eq!(record.len(), SCROLL_RECORD_SIZE);
        assert_eq!(
            record, &source,
            "the owned function copied the source record"
        );
    }
    oracle.close();
    helper.quit();
    Ok(())
}

#[test]
fn a_timed_out_oracle_call_is_retired_before_more_work() -> Result<(), RuntimeError> {
    let _test_guard = lock_live_helper_test();
    let mut helper = Helper::spawn()?;
    let (page, _template, code) = helper.oracle(true)?;
    let mut oracle = NativeBatchOracle::new(
        helper.pid,
        helper.module_base,
        helper_profile(page, helper.module_base, &code),
        2,
        false,
    )?;
    oracle.open(Box::new(WindowsRemoteSession::open(helper.pid)?))?;
    let source = vec![0x01u8; SCROLL_RECORD_SIZE];
    let error = oracle
        .generate(&[source], 1)
        .err()
        .unwrap_or(RuntimeError::RuntimeBusy);
    assert_eq!(error.code(), "ORACLE_REJECTED");
    assert!(
        oracle.remote_call_pending(),
        "an unobserved call must be reported as pending"
    );
    // The waiter frees the allocation only after the thread exits.
    let mut waited = 0;
    while oracle.remote_call_pending() && waited < 200 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        waited += 1;
    }
    assert!(
        !oracle.remote_call_pending(),
        "retirement completes when the thread exit is observed"
    );
    oracle.close();
    helper.quit();
    Ok(())
}

#[test]
fn the_real_debug_binding_attaches_reads_arms_and_restores() -> Result<(), RuntimeError> {
    let _test_guard = lock_live_helper_test();
    let mut helper = Helper::spawn()?;
    let image = std::path::Path::new(env!("CARGO_BIN_EXE_runtime_mutation_helper"))
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut session = WindowsDebugSession::open(helper.pid, &image)?;
    session.attach()?;
    assert!(session.attached());

    // Collect the create-process and create-thread events, adopt the thread and
    // stop at the initial breakpoint with every thread frozen.
    let mut tid = None;
    let mut armed_event = None;
    for _ in 0..64 {
        let Some(event) = session.wait(1000)? else {
            break;
        };
        if event.is_create_thread() || event.is_create_process() {
            if let Some(handle) = event.thread_handle {
                session.adopt_thread(event.tid, handle)?;
            }
            session.resume(&event, true)?;
            continue;
        }
        if event.is_exception() && event.exception_code == Some(EXCEPTION_BREAKPOINT) {
            tid = Some(event.tid);
            armed_event = Some(event);
            break;
        }
        session.resume(&event, true)?;
    }
    let tid = tid.ok_or(RuntimeError::SessionNotOpen)?;

    // A real CONTEXT read with the 16-byte aligned storage the binding uses.
    let context = session.context(tid)?;
    assert!(
        context.rip != 0 && context.rsp != 0,
        "the stopped thread has a readable CONTEXT"
    );
    assert_eq!(
        context.dr0, 0,
        "the helper starts with no hardware breakpoints"
    );

    // Arm, re-read, then restore: the whole debug-register round trip.
    session.arm_thread(tid, helper.page, helper.page + 7)?;
    let armed = session.context(tid)?;
    assert_eq!(armed.dr0, helper.page);
    assert_eq!(armed.dr1, helper.page + 7);
    assert_eq!(armed.dr7 & 0xFF, 5);
    session.restore_threads()?;
    let restored = session.context(tid)?;
    assert_eq!(restored.dr0, 0, "the debug registers are restored");
    assert_eq!(restored.dr7 & 0xFF, 0);

    if let Some(event) = armed_event {
        session.resume(&event, true)?;
    }
    session.detach()?;
    assert!(!session.attached());
    let _ = WAIT_INFINITE;
    helper.quit();
    Ok(())
}

/// `set_context` must fetch the full CONTEXT before writing the modelled
/// fields back, or every group its `ContextFlags` did not select is written
/// back as zeros. The observable proof is a non-modelled group: `MxCsr` and an
/// `Xmm` register must survive a Get -> modify-modelled-fields -> Set round
/// trip unchanged. Disposable helper child only; the game is never touched.
#[test]
fn a_context_write_back_preserves_the_groups_it_does_not_model() -> Result<(), RuntimeError> {
    let _test_guard = lock_live_helper_test();
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::Debug::{GetThreadContext, CONTEXT};
    use windows_sys::Win32::System::Threading::{
        OpenThread, THREAD_GET_CONTEXT, THREAD_QUERY_INFORMATION, THREAD_SET_CONTEXT,
    };
    /// `CONTEXT_ALL` for x86-64: control, integer, floating point and debug.
    const CONTEXT_ALL: u32 = 0x0010_003B;
    /// One `CONTEXT` slot with the 16-byte alignment the API requires.
    #[repr(align(64))]
    struct Aligned([u8; 1280]);

    /// Read one full `CONTEXT` through aligned storage.
    unsafe fn read_context(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<CONTEXT> {
        let mut storage = Box::new(Aligned([0u8; 1280]));
        let raw = storage.0.as_mut_ptr() as *mut CONTEXT;
        (*raw).ContextFlags = CONTEXT_ALL;
        if GetThreadContext(handle, raw) == 0 {
            return None;
        }
        Some(*raw)
    }

    let mut helper = Helper::spawn()?;
    let image = std::path::Path::new(env!("CARGO_BIN_EXE_runtime_mutation_helper"))
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut session = WindowsDebugSession::open(helper.pid, &image)?;
    session.attach()?;

    let outcome = (|| -> Result<(), RuntimeError> {
        // Stop at the initial debugger breakpoint with the helper's own thread.
        let mut tid = None;
        let mut armed_event = None;
        for _ in 0..64 {
            let Some(event) = session.wait(1000)? else {
                break;
            };
            if event.is_create_thread() || event.is_create_process() {
                if let Some(handle) = event.thread_handle {
                    session.adopt_thread(event.tid, handle)?;
                }
                session.resume(&event, true)?;
                continue;
            }
            if event.is_exception() && event.exception_code == Some(EXCEPTION_BREAKPOINT) {
                tid = Some(event.tid);
                armed_event = Some(event);
                break;
            }
            session.resume(&event, true)?;
        }
        let tid = tid.ok_or(RuntimeError::SessionNotOpen)?;

        // A thread handle for the raw CONTEXT reads below. The session's adopted
        // handle stays private, so this test opens its own with exactly the
        // rights a context round trip needs.
        let handle = unsafe {
            OpenThread(
                THREAD_GET_CONTEXT | THREAD_SET_CONTEXT | THREAD_QUERY_INFORMATION,
                0,
                tid,
            )
        };
        if handle.is_null() {
            return Err(RuntimeError::SessionNotOpen);
        }
        // Both the handle and the debug session's ownership are released on
        // every path, including an assertion failure below.
        let result = (|| -> Result<(), RuntimeError> {
            let before = unsafe { read_context(handle) }.ok_or(RuntimeError::SessionNotOpen)?;
            // `MxCsr` is a direct field; the XMM registers live in a union, so
            // read the first 128-bit register through its struct view.
            let (mxcsr_before, xmm_before) =
                unsafe { (before.MxCsr, before.Anonymous.Anonymous.Xmm0) };
            let original = session.context(tid)?;

            // Write back a context that differs only in modelled fields.
            let mut modelled = original;
            modelled.rax = modelled.rax.wrapping_add(0x1234_5678);
            modelled.rbx = modelled.rbx.wrapping_add(0x9ABC_DEF0);
            session.set_context(tid, &modelled)?;

            let after = unsafe { read_context(handle) }.ok_or(RuntimeError::SessionNotOpen)?;
            assert_eq!(
                after.MxCsr, mxcsr_before,
                "the non-modelled MxCsr control state must survive the write back"
            );
            let xmm_after = unsafe { after.Anonymous.Anonymous.Xmm0 };
            assert_eq!(
                (xmm_after.Low, xmm_after.High),
                (xmm_before.Low, xmm_before.High),
                "the non-modelled Xmm0 register must survive the write back"
            );
            assert_eq!(
                after.Rip, modelled.rip,
                "the modelled instruction pointer is still what was written"
            );
            assert_eq!(after.Rax, modelled.rax, "the modelled Rax landed");

            // Put the modelled registers back so the helper continues cleanly.
            session.set_context(tid, &original)?;
            let restored = unsafe { read_context(handle) }.ok_or(RuntimeError::SessionNotOpen)?;
            assert_eq!(restored.Rax, original.rax, "Rax is restored");
            assert_eq!(restored.MxCsr, mxcsr_before, "MxCsr is still untouched");
            Ok(())
        })();

        // Cleanup runs whatever the assertions decided.
        unsafe { CloseHandle(handle) };
        if let Some(event) = armed_event {
            session.resume(&event, true)?;
        }
        session.detach()?;
        assert!(!session.attached());
        result
    })();

    helper.quit();
    outcome
}

#[test]
fn the_layout_the_executor_uses_is_the_shipped_one() {
    let layout: LiveAddLayout = PC_V201_LIVE_ADD;
    assert_eq!(layout.profile_id, "pc-v2.01-live-add-r1");
    assert_eq!(layout.capacity, 400);
    assert_eq!(layout.record_size, 0xE8);
    assert_eq!(layout.descriptor_size, 0xCC);
    assert_eq!(layout.insertion_rva, 0x54D294);
}
