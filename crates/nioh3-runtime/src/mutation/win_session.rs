//! The two real Windows process views the native executor and the batch oracle
//! drive, behind neutral traits so the state machines stay portable.
//!
//! Port of `windows_debug_session.WindowsDebug` (the live-add path) and of the
//! `NativeBatchOracle` process surface (the batch path). Both open the target
//! with [`crate::mutation::native_abi::RUNTIME_ACCESS`], which is the exact mask
//! the shipped code uses for them: `PROCESS_CREATE_THREAD` is present only
//! because `DebugBreakProcess` and `CreateRemoteThread` need it, and neither
//! `PROCESS_ALL_ACCESS` nor any lifecycle right is ever requested.
//!
//! Nothing here decides *whether* to act. The executor and the oracle own the
//! admission, identity and receipt rules; this module is the syscall binding and
//! refuses to invent policy.

use crate::error::RuntimeError;

/// One neutral debug event, decoded from `DEBUG_EVENT`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DebugEvent {
    /// `dwDebugEventCode`.
    pub code: u32,
    pub pid: u32,
    pub tid: u32,
    /// `EXCEPTION_DEBUG_EVENT`: the exception code, e.g. `0x80000003`.
    pub exception_code: Option<i32>,
    /// `CREATE_THREAD_DEBUG_EVENT` / `CREATE_PROCESS_DEBUG_EVENT`.
    pub thread_handle: Option<u64>,
    /// `CREATE_PROCESS_DEBUG_EVENT` / `LOAD_DLL_DEBUG_EVENT`.
    pub file_handle: Option<u64>,
    /// `CREATE_PROCESS_DEBUG_EVENT`.
    pub process_handle: Option<u64>,
    /// `EXIT_PROCESS_DEBUG_EVENT` / `EXIT_THREAD_DEBUG_EVENT`.
    pub exit_code: Option<u32>,
}

impl DebugEvent {
    /// `windows_debug_session`'s `event.code == 1`.
    pub fn is_exception(&self) -> bool {
        self.code == 1
    }

    pub fn is_create_thread(&self) -> bool {
        self.code == 2
    }

    pub fn is_create_process(&self) -> bool {
        self.code == 3
    }

    pub fn is_exit_thread(&self) -> bool {
        self.code == 4
    }

    pub fn is_exit_process(&self) -> bool {
        self.code == 5
    }

    pub fn is_load_dll(&self) -> bool {
        self.code == 6
    }
}

/// `STATUS_BREAKPOINT` / `STATUS_SINGLE_STEP`: the two hardware-breakpoint
/// stops the live-add shim arms and acknowledges.
pub const EXCEPTION_BREAKPOINT: i32 = 0x8000_0003u32 as i32;
pub const EXCEPTION_SINGLE_STEP: i32 = 0x8000_0004u32 as i32;
/// `DBG_CONTINUE`.
pub const DBG_CONTINUE: u32 = 0x0001_0002;
/// `DBG_EXCEPTION_NOT_HANDLED`.
pub const DBG_EXCEPTION_NOT_HANDLED: u32 = 0x8001_0001;

/// The general-purpose and debug state the shim reads and restores.
///
/// Field order and names follow the shipped `registers(context)` view so the
/// ported `dispatch_evidence.verify_dispatch` consumes exactly the same keys.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ThreadContext {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rsp: u64,
    pub rip: u64,
    pub eflags: u64,
    pub dr0: u64,
    pub dr1: u64,
    pub dr2: u64,
    pub dr3: u64,
    pub dr6: u64,
    pub dr7: u64,
}

impl ThreadContext {
    /// The `registers(context)` mapping the dispatch evidence verifier reads.
    pub fn registers(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut map = serde_json::Map::new();
        for (name, value) in [
            ("RAX", self.rax),
            ("RBX", self.rbx),
            ("RCX", self.rcx),
            ("RDX", self.rdx),
            ("RSI", self.rsi),
            ("RDI", self.rdi),
            ("RBP", self.rbp),
            ("R8", self.r8),
            ("R9", self.r9),
            ("R10", self.r10),
            ("R11", self.r11),
            ("R12", self.r12),
            ("R13", self.r13),
            ("R14", self.r14),
            ("R15", self.r15),
            ("RSP", self.rsp),
            ("RIP", self.rip),
            ("EFLAGS", self.eflags),
        ] {
            map.insert(name.to_string(), serde_json::json!(value));
        }
        map
    }
}

/// The live-add debugger view `NativeLiveAddTransport._run` drives.
///
/// `arm_thread` is the only place a hardware breakpoint is written, and it only
/// ever writes the entry/acknowledgement pair the caller already resolved.
pub trait DebugSession {
    fn pid(&self) -> u32;

    fn creation_time(&mut self) -> Result<String, RuntimeError>;

    fn module_base(&mut self) -> Result<u64, RuntimeError>;

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError>;

    fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError>;

    fn allocate(&mut self, size: usize) -> Result<u64, RuntimeError>;

    fn free(&mut self, address: u64) -> Result<(), RuntimeError>;

    /// `FlushInstructionCache` over the bytes just written.
    fn flush_instruction_cache(&mut self, address: u64, size: usize) -> Result<(), RuntimeError>;

    /// `DebugActiveProcess` plus `DebugSetProcessKillOnExit(false)`.
    fn attach(&mut self) -> Result<(), RuntimeError>;

    fn attached(&self) -> bool;

    /// `DebugActiveProcessStop`; a no-op when this view is not attached.
    fn detach(&mut self) -> Result<(), RuntimeError>;

    fn wait(&mut self, milliseconds: u32) -> Result<Option<DebugEvent>, RuntimeError>;

    fn resume(&mut self, event: &DebugEvent, handled: bool) -> Result<(), RuntimeError>;

    fn context(&mut self, tid: u32) -> Result<ThreadContext, RuntimeError>;

    fn set_context(&mut self, tid: u32, context: &ThreadContext) -> Result<(), RuntimeError>;

    /// Register a thread handle delivered by a create event.
    fn adopt_thread(&mut self, tid: u32, handle: u64) -> Result<(), RuntimeError>;

    /// `arm_thread`: Dr0 = entry, Dr1 = acknowledgement, `Dr7 = (Dr7 &
    /// ~0xFFFF00FF) | 5`.
    fn arm_thread(
        &mut self,
        tid: u32,
        entry: u64,
        acknowledgement: u64,
    ) -> Result<(), RuntimeError>;

    /// Restore every adopted thread's debug registers and drop the record.
    fn restore_threads(&mut self) -> Result<(), RuntimeError>;

    /// `Ok(true)` only when every adopted thread handle is signalled.
    fn all_threads_exited(&mut self) -> Result<bool, RuntimeError>;

    /// A thread terminated without an exit event must not be written again.
    fn thread_signalled(&mut self, tid: u32) -> Result<bool, RuntimeError>;

    fn debug_break(&mut self) -> Result<(), RuntimeError>;
}

/// One test/report view of a debugger-owned thread instance.
///
/// Crate-private so the public `DebugSession` ABI stays unchanged while the
/// executor and its injected fake share one explicit ownership protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnerThreadSnapshot {
    pub tid: u32,
    pub instance: u64,
    pub handle: u64,
    pub handle_provenance: &'static str,
    pub run_state: &'static str,
    pub cleanup_state: &'static str,
    pub error: Option<String>,
}

/// Independent debugger and per-thread cleanup facts for one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeOwnerSnapshot {
    pub debugger_state: &'static str,
    pub threads: Vec<OwnerThreadSnapshot>,
}

/// Internal extension implemented by the real Windows binding and strict fake.
/// It adds reporting only; event/context operations remain on the unchanged
/// public `DebugSession` trait.
pub(crate) trait RuntimeOwnerSession: DebugSession {
    /// Request the explicit stop event used by cleanup. This is separate from
    /// operational nudges so fakes can model both without inventing barriers.
    fn begin_cleanup_barrier(&mut self) -> Result<(), RuntimeError>;

    fn runtime_owner_snapshot(&self) -> RuntimeOwnerSnapshot;
}

/// The batch-oracle process surface (`NativeBatchOracle`).
pub trait RemoteSession {
    fn pid(&self) -> u32;

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError>;

    fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError>;

    fn allocate(&mut self, size: usize) -> Result<u64, RuntimeError>;

    fn free(&mut self, address: u64) -> Result<(), RuntimeError>;

    /// `CreateRemoteThread` at `start`, returning the thread handle.
    fn create_remote_thread(&mut self, start: u64) -> Result<u64, RuntimeError>;

    /// `WaitForSingleObject(thread, milliseconds)`.
    fn wait_thread(&mut self, thread: u64, milliseconds: u32) -> Result<u32, RuntimeError>;

    fn thread_exit_code(&mut self, thread: u64) -> Result<u32, RuntimeError>;

    fn close_thread(&mut self, thread: u64);

    /// Release this view's own process handle without touching the target.
    fn close(&mut self);
}

/// `WAIT_OBJECT_0`.
pub const WAIT_OBJECT_0: u32 = 0;
/// `WAIT_INFINITE`.
pub const WAIT_INFINITE: u32 = 0xFFFF_FFFF;
/// `WAIT_FAILED`.
pub const WAIT_FAILED: u32 = 0xFFFF_FFFF;

/// The two OS observations that decide whether a recorded handle still owns
/// its thread instance.
///
/// A numeric Win32 handle value is not thread identity. The system closes the
/// handle it supplied with `EXIT_THREAD` once the debugger continues that
/// event, and it may hand the same value to a later thread, so nothing here may
/// be concluded from the value. Ownership is therefore decided from the
/// instance the value names now, and only from an observation that actually
/// succeeded.
///
/// A failed wait is an *observation* failure, not a lifecycle fact. Microsoft
/// documents `WAIT_FAILED` as the wait itself failing and requiring
/// `GetLastError`; it is explicitly not the thread terminated
/// (`WaitForSingleObject`, "Return value"). Because the wait failed there is no
/// signal evidence at all, so the caller may conclude neither "still owns" nor
/// "no longer owns" and the record must stay unresolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordedHandleObservation {
    /// `WaitForSingleObject(recorded_handle, 0)`.
    pub wait_result: u32,
    /// Whether the wait succeeded and produced a real signal observation. Only
    /// `WAIT_OBJECT_0` (signalled) and `WAIT_TIMEOUT` (still running) are
    /// observations; a failed wait carries none of the documented wait values
    /// and must never be compared as if one of them.
    pub wait_observed: bool,
    /// `GetThreadId(recorded_handle)` when it was queried, and `None` when the
    /// query failed or could not be run on this observation.
    pub handle_thread_id: Option<u32>,
    /// `GetLastError()` read immediately after a failed wait, and `0` for an
    /// observation that produced a real signal. The failure code is part of the
    /// evidence, and the thread's last-error slot is not stable: any later
    /// successful call may leave a different value there, so the code has to be
    /// captured at the observation rather than re-read when it is reported.
    pub wait_last_error: u32,
    /// `GetLastError()` read immediately after `GetThreadId` returned zero.
    /// A running wait result does not restore handle authority when this query
    /// fails: liveness and instance identity are separate facts.
    pub identity_last_error: u32,
}

/// Whether a recorded thread instance still owns its numeric handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordedInstanceState {
    /// The handle still names the recorded thread, which is still alive: a
    /// genuine duplicate adoption.
    Live,
    /// The handle is closed, the thread terminated, or the value now names a
    /// different thread. The record no longer owns the value and must never be
    /// written through it again.
    Stale,
    /// The OS could not be observed: the wait failed, so there is neither a
    /// signal nor a liveness answer. The record keeps ownership and stays
    /// unresolved: a failed observation is not a termination proof, is not
    /// evidence that the record no longer owns its value, and must never settle
    /// its cleanup obligation.
    Unobserved,
}

/// Decide whether one recorded instance still owns its numeric handle.
///
/// The numeric value is deliberately not an input: it can be reused, so only
/// the instance the handle names can prove ownership, and that proof exists
/// only for an observation the OS actually produced.
pub(crate) fn recorded_instance_state(
    recorded_tid: u32,
    observation: RecordedHandleObservation,
) -> RecordedInstanceState {
    if !observation.wait_observed {
        // `WAIT_FAILED`: the wait itself failed. This is an observation
        // failure, so the record is unresolved rather than retired.
        return RecordedInstanceState::Unobserved;
    }
    match observation.wait_result {
        // The thread terminated. A terminated thread has no live debug state
        // and the handle it left behind is not evidence of ownership.
        WAIT_OBJECT_0 => RecordedInstanceState::Stale,
        // `WAIT_TIMEOUT` (and any other documented running value): the thread
        // is still running. Only a *successful* `GetThreadId` adds identity:
        // the same thread keeps ownership, a different thread proves the value
        // was reused, and a failed query proves nothing either way.
        _ => match observation.handle_thread_id {
            None => RecordedInstanceState::Unobserved,
            Some(handle_tid) if handle_tid == recorded_tid => RecordedInstanceState::Live,
            // The numeric value was handed to a different thread.
            Some(_) => RecordedInstanceState::Stale,
        },
    }
}

/// One recorded instance as the create-event admission rule sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordedInstance {
    pub tid: u32,
    pub instance: u64,
    pub handle_value: u64,
    pub state: RecordedInstanceState,
}

/// The admission decision for one create event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Adoption {
    /// Adopt the event after retiring these record indexes.
    Adopt { stale: Vec<usize> },
    /// A live instance already owns the tid; the event is a duplicate.
    Duplicate { tid: u32, instance: u64 },
    /// A record the OS could not be observed for might still own the tid or the
    /// value. The event is refused and the record keeps its ownership, because
    /// retirement requires evidence and a failed wait is not evidence.
    Unresolved { tid: u32, instance: u64 },
}

/// Resolve one create event against the recorded instances.
///
/// A record only blocks the event when the OS still reports it as the live
/// thread that owns its tid. Every other conflicting record is stale: its
/// numeric value was closed or reused, so it is retired and the value is free
/// for the new instance.
pub(crate) fn resolve_adoption(records: &[RecordedInstance], tid: u32, handle: u64) -> Adoption {
    let mut stale = Vec::new();
    for (index, record) in records.iter().enumerate() {
        if record.tid != tid && record.handle_value != handle {
            continue;
        }
        if record.state == RecordedInstanceState::Live {
            return Adoption::Duplicate {
                tid: record.tid,
                instance: record.instance,
            };
        }
        if record.state == RecordedInstanceState::Unobserved {
            // A failed observation is not a stale value. The record's ownership
            // is unknown, so the new event may not take the value it might
            // still hold. The event is refused rather than the record retired,
            // which keeps every unresolved thread instance visible to the
            // caller instead of silently dropping a live one.
            return Adoption::Unresolved {
                tid: record.tid,
                instance: record.instance,
            };
        }
        stale.push(index);
    }
    Adoption::Adopt { stale }
}

#[cfg(windows)]
mod windows_impl {
    use super::{
        recorded_instance_state, resolve_adoption, Adoption, DebugEvent, DebugSession,
        OwnerThreadSnapshot, RecordedHandleObservation, RecordedInstance, RecordedInstanceState,
        RemoteSession, RuntimeOwnerSession, RuntimeOwnerSnapshot, ThreadContext, DBG_CONTINUE,
        DBG_EXCEPTION_NOT_HANDLED, WAIT_FAILED,
    };
    use crate::error::RuntimeError;
    use crate::mutation::native_abi::{
        MEM_COMMIT_RESERVE, MEM_RELEASE, PAGE_EXECUTE_READWRITE, RUNTIME_ACCESS,
    };
    // The module-level access constant is imported so the audit can see which
    // module claims it, and re-exported for the executor's own use.
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, NTSTATUS,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{
        ContinueDebugEvent, DebugActiveProcess, DebugActiveProcessStop, DebugBreakProcess,
        DebugSetProcessKillOnExit, FlushInstructionCache, GetThreadContext, ReadProcessMemory,
        SetThreadContext, WaitForDebugEvent, CONTEXT, DEBUG_EVENT, EXCEPTION_DEBUG_EVENT,
    };
    use windows_sys::Win32::System::Memory::{
        VirtualAllocEx, VirtualFreeEx, MEMORY_BASIC_INFORMATION,
    };
    use windows_sys::Win32::System::Threading::{
        CreateRemoteThread, GetExitCodeThread, GetThreadId, OpenProcess, WaitForSingleObject,
    };

    /// `DebugActiveProcess` is not a right, but the mask the shipped debug view
    /// opens with is the same one the oracle uses.
    pub const LIVE_ADD_ACCESS: u32 = RUNTIME_ACCESS;

    fn last_error() -> u32 {
        unsafe { GetLastError() }
    }

    fn failed(detail: &'static str, code: u32) -> RuntimeError {
        RuntimeError::ProcessQuery {
            pid: 0,
            code,
            detail,
        }
    }

    /// The observation seam: which numeric handle value the wait must be made to
    /// fail for, so a test can create a genuine `WAIT_FAILED` from the real
    /// binding instead of injecting a finished snapshot.
    ///
    /// Empty in the product build and in every shipped test, so the real
    /// `WaitForSingleObject` result is the only input. Under `test-observation`
    /// exactly one handle value can have its wait redirected. The independent
    /// identity-query seam below can redirect `GetThreadId`; context calls and
    /// retire decisions still run on the real handle.
    #[cfg(feature = "test-observation")]
    static UNOBSERVABLE_HANDLE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    #[cfg(feature = "test-observation")]
    static UNIDENTIFIABLE_HANDLE: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0);

    /// `WaitForSingleObject` as the binding observes it.
    fn wait_for_single_object(handle: HANDLE, milliseconds: u32) -> u32 {
        #[cfg(feature = "test-observation")]
        {
            let forced = UNOBSERVABLE_HANDLE.load(std::sync::atomic::Ordering::SeqCst);
            if forced != 0 && forced == handle as u64 {
                // A real failure result with a real error code. The mismatch is
                // exactly the shape the product must not read as a termination.
                unsafe {
                    windows_sys::Win32::Foundation::SetLastError(
                        windows_sys::Win32::Foundation::ERROR_INVALID_HANDLE,
                    )
                };
                return WAIT_FAILED;
            }
        }
        unsafe { WaitForSingleObject(handle, milliseconds) }
    }

    /// `GetThreadId` as the binding observes it.
    fn get_thread_id(handle: HANDLE) -> u32 {
        #[cfg(feature = "test-observation")]
        {
            let forced = UNIDENTIFIABLE_HANDLE.load(std::sync::atomic::Ordering::SeqCst);
            if forced != 0 && forced == handle as u64 {
                unsafe {
                    windows_sys::Win32::Foundation::SetLastError(
                        windows_sys::Win32::Foundation::ERROR_INVALID_HANDLE,
                    )
                };
                return 0;
            }
        }
        unsafe { GetThreadId(handle) }
    }

    /// A 64-byte aligned `CONTEXT` slot: Windows requires 16-byte alignment for
    /// `GetThreadContext`, which the natural `repr(C)` alignment does not give.
    #[repr(align(64))]
    struct AlignedContext([u8; 1280]);

    impl AlignedContext {
        fn new() -> Box<Self> {
            Box::new(Self([0u8; 1280]))
        }

        fn raw_ptr(&mut self) -> *mut CONTEXT {
            self.0.as_mut_ptr() as *mut CONTEXT
        }
    }

    /// `CONTEXT_ALL` for x86-64: control, integer, floating point and debug.
    const CONTEXT_ALL: u32 = 0x0010_003Bu32;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ThreadRunState {
        Running,
        Stopped,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum HandleProvenance {
        CreateProcessEvent,
        CreateThreadEvent,
    }

    impl HandleProvenance {
        fn label(self) -> &'static str {
            match self {
                Self::CreateProcessEvent => "create_process_event",
                Self::CreateThreadEvent => "create_thread_event",
            }
        }
    }

    struct OwnedThread {
        tid: u32,
        instance: u64,
        handle_value: u64,
        handle: HANDLE,
        provenance: HandleProvenance,
        run_state: ThreadRunState,
        original_debug: Option<[u64; 6]>,
        cleanup_state: &'static str,
        /// Whether the OS observation behind this record still authorizes
        /// reading or writing through its numeric handle.
        ///
        /// A create event authorizes the handle. A failed observation withdraws
        /// that authorization, because the value may already name a different
        /// thread; only a later *successful* observation of the same instance
        /// restores it.
        handle_trusted: bool,
        error: Option<String>,
    }

    pub struct WindowsDebugSession {
        pid: u32,
        process: HANDLE,
        attached: bool,
        owner_thread: Option<std::thread::ThreadId>,
        pending_event: Option<DebugEvent>,
        next_thread_instance: u64,
        threads: Vec<OwnedThread>,
        retired_threads: Vec<OwnerThreadSnapshot>,
        debugger_state: &'static str,
        module_name: String,
    }

    // The handle is owned exclusively by this session and only passed to the
    // documented thread-safe Win32 calls.
    unsafe impl Send for WindowsDebugSession {}

    impl WindowsDebugSession {
        pub fn open(pid: u32, module_name: &str) -> Result<Self, RuntimeError> {
            let process = unsafe { OpenProcess(LIVE_ADD_ACCESS, 0, pid) };
            if process.is_null() || process == INVALID_HANDLE_VALUE {
                return Err(RuntimeError::OpenProcess {
                    pid,
                    code: last_error(),
                });
            }
            Ok(Self {
                pid,
                process,
                attached: false,
                owner_thread: None,
                pending_event: None,
                next_thread_instance: 0,
                threads: Vec::new(),
                retired_threads: Vec::new(),
                debugger_state: "not_attached",
                module_name: module_name.to_string(),
            })
        }

        fn thread(&self, tid: u32) -> Option<&OwnedThread> {
            self.threads.iter().find(|thread| thread.tid == tid)
        }

        fn thread_mut(&mut self, tid: u32) -> Option<&mut OwnedThread> {
            self.threads.iter_mut().find(|thread| thread.tid == tid)
        }

        fn ensure_owner_thread(&self, operation: &str) -> Result<(), RuntimeError> {
            if let Some(owner) = self.owner_thread {
                if owner != std::thread::current().id() {
                    return Err(RuntimeError::NativeDispatch {
                        detail: format!(
                            "RuntimeOwner {operation} must run on the debugger owner thread"
                        ),
                    });
                }
            }
            Ok(())
        }

        fn require_stopped_thread(&self, tid: u32) -> Result<HANDLE, RuntimeError> {
            if self.pending_event.is_none() {
                return Err(RuntimeError::NativeDispatch {
                    detail: format!(
                        "Thread context for tid {tid} requires a pending stopped debug event"
                    ),
                });
            }
            let thread = self.thread(tid).ok_or(RuntimeError::SessionNotOpen)?;
            if thread.run_state != ThreadRunState::Stopped {
                return Err(RuntimeError::NativeDispatch {
                    detail: format!(
                        "Thread context for tid {tid} instance {} is not stopped",
                        thread.instance
                    ),
                });
            }
            if !thread.handle_trusted {
                // The record's OS observation failed, so this numeric value is
                // not proven to name this instance any more. Reading or writing
                // through it could reach a different thread, so the handle is
                // refused until a successful observation authorizes it again.
                return Err(RuntimeError::NativeDispatch {
                    detail: format!(
                        "Thread context for tid {tid} instance {} is refused: its handle is \
                         unverified after a failed OS observation",
                        thread.instance
                    ),
                });
            }
            Ok(thread.handle)
        }

        fn snapshot_thread(thread: &OwnedThread) -> OwnerThreadSnapshot {
            OwnerThreadSnapshot {
                tid: thread.tid,
                instance: thread.instance,
                handle: thread.handle_value,
                handle_provenance: thread.provenance.label(),
                run_state: match thread.run_state {
                    ThreadRunState::Running => "running",
                    ThreadRunState::Stopped => "stopped",
                },
                cleanup_state: thread.cleanup_state,
                error: thread.error.clone(),
            }
        }

        /// The OS facts that decide whether a recorded instance still owns its
        /// numeric handle. The value itself is never consulted.
        ///
        /// `WaitForSingleObject` returns `WAIT_FAILED` when the wait *failed*,
        /// which is an observation failure and not a thread-terminated result.
        /// The failure is therefore recorded as `wait_observed = false` with
        /// its `GetLastError`, and no signal conclusion is drawn. `GetThreadId`
        /// is only queried for a successful wait, because a failed wait has no
        /// handle authority to ask about.
        fn observe_recorded_handle(thread: &OwnedThread) -> RecordedHandleObservation {
            let wait_result = wait_for_single_object(thread.handle, 0);
            if wait_result == WAIT_FAILED {
                // Read the failure code here, at the observation, so the
                // recorded category belongs to this failed wait.
                let wait_last_error = last_error();
                return RecordedHandleObservation {
                    wait_result,
                    wait_observed: false,
                    handle_thread_id: None,
                    wait_last_error,
                    identity_last_error: 0,
                };
            }
            let (handle_thread_id, identity_last_error) = if wait_result != super::WAIT_OBJECT_0 {
                // A running thread still has a thread identity; only a genuine
                // query answers it, and a failed query stays `None`.
                match get_thread_id(thread.handle) {
                    0 => (None, last_error()),
                    tid => (Some(tid), 0),
                }
            } else {
                (None, 0)
            };
            RecordedHandleObservation {
                wait_result,
                wait_observed: true,
                handle_thread_id,
                wait_last_error: 0,
                identity_last_error,
            }
        }

        /// Move one record out of the live set and into the retired report.
        ///
        /// The numeric handle is only preserved as provenance: the system
        /// already closed it, or handed the value to another thread, so it is
        /// never closed here and never written through again.
        fn retire_thread_at(&mut self, index: usize, cleanup_state: &'static str) {
            let mut retired = self.threads.remove(index);
            retired.original_debug = None;
            retired.cleanup_state = cleanup_state;
            retired.error = None;
            let mut snapshot = Self::snapshot_thread(&retired);
            snapshot.run_state = "exited";
            self.retired_threads.push(snapshot);
        }

        /// Retire one record addressed by identity instead of by index.
        ///
        /// A stored index names a slot, not a record: removing an earlier
        /// record shifts every later one, so a decision list captured before
        /// the removals must be resolved again by `(tid, instance)`.
        fn retire_thread_named(&mut self, tid: u32, instance: u64, cleanup_state: &'static str) {
            if let Some(index) = self
                .threads
                .iter()
                .position(|thread| thread.tid == tid && thread.instance == instance)
            {
                self.retire_thread_at(index, cleanup_state);
            }
        }

        /// Keep one record whose OS observation failed without retiring it.
        ///
        /// `WAIT_FAILED` is not an exit and not proof that the value was reused,
        /// so the record keeps its live slot, its original debug state and its
        /// numeric handle, and is marked unresolved. The earlier error text and
        /// the failing `last_error` are preserved, and the record is never
        /// written through again because `recorded_instance_state` reports it as
        /// `Unobserved` until a real observation resolves it.
        ///
        /// The record is addressed by identity rather than by a stored index:
        /// retiring an earlier record shifts every later slot, so a captured
        /// index is not a stable name for the record it came from.
        fn retain_unobserved_thread_at(
            &mut self,
            tid: u32,
            instance: u64,
            failed_api: &'static str,
            last_error: u32,
        ) {
            if let Some(thread) = self
                .threads
                .iter_mut()
                .find(|thread| thread.tid == tid && thread.instance == instance)
            {
                // Ownership is retained: no debug registers are dropped, no
                // handle is closed, and the record stays in the live set.
                let retained_cleanup = thread.cleanup_state;
                thread.cleanup_state = "unknown";
                thread.handle_trusted = false;
                thread.error = Some(format!(
                    "{failed_api} could not establish the identity of tid {} instance {} handle {:#x}; \
                     last_error={last_error}; observation is unresolved, the retained \
                     structural cleanup state was {retained_cleanup}",
                    thread.tid, thread.instance, thread.handle_value
                ));
            }
        }
    }

    impl Drop for WindowsDebugSession {
        fn drop(&mut self) {
            self.detach().ok();
            for thread in self.threads.drain(..) {
                unsafe { CloseHandle(thread.handle) };
            }
            if !self.process.is_null() && self.process != INVALID_HANDLE_VALUE {
                unsafe { CloseHandle(self.process) };
                self.process = std::ptr::null_mut();
            }
        }
    }

    impl DebugSession for WindowsDebugSession {
        fn pid(&self) -> u32 {
            self.pid
        }

        fn creation_time(&mut self) -> Result<String, RuntimeError> {
            crate::platform::process_creation_filetime(self.pid)?
                .map(|value| value.to_string())
                .ok_or(RuntimeError::ProcessGone { pid: self.pid })
        }

        fn module_base(&mut self) -> Result<u64, RuntimeError> {
            crate::platform::module_range(self.pid, &self.module_name).map(|range| range.base)
        }

        fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            let mut buffer = vec![0u8; size];
            let mut transferred: usize = 0;
            let ok = unsafe {
                ReadProcessMemory(
                    self.process,
                    address as *const c_void,
                    buffer.as_mut_ptr() as *mut c_void,
                    size,
                    &mut transferred,
                )
            };
            if ok == 0 {
                return Err(RuntimeError::MemoryRead {
                    address,
                    size,
                    code: last_error(),
                });
            }
            if transferred != size {
                return Err(RuntimeError::ShortRead {
                    address,
                    expected: size,
                    actual: transferred,
                });
            }
            Ok(buffer)
        }

        fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
            let mut transferred: usize = 0;
            let ok = unsafe {
                windows_sys::Win32::System::Diagnostics::Debug::WriteProcessMemory(
                    self.process,
                    address as *mut c_void,
                    data.as_ptr() as *const c_void,
                    data.len(),
                    &mut transferred,
                )
            };
            if ok == 0 {
                return Err(RuntimeError::MemoryWrite {
                    address,
                    size: data.len(),
                    code: last_error(),
                });
            }
            if transferred != data.len() {
                return Err(RuntimeError::ShortWrite {
                    address,
                    expected: data.len(),
                    actual: transferred,
                });
            }
            Ok(())
        }

        fn allocate(&mut self, size: usize) -> Result<u64, RuntimeError> {
            let allocation = unsafe {
                VirtualAllocEx(
                    self.process,
                    std::ptr::null(),
                    size,
                    MEM_COMMIT_RESERVE,
                    PAGE_EXECUTE_READWRITE,
                )
            };
            if allocation.is_null() {
                return Err(RuntimeError::AllocationUnavailable {
                    address: 0,
                    size: size as u64,
                });
            }
            Ok(allocation as u64)
        }

        fn free(&mut self, address: u64) -> Result<(), RuntimeError> {
            let freed =
                unsafe { VirtualFreeEx(self.process, address as *mut c_void, 0, MEM_RELEASE) };
            if freed == 0 {
                return Err(RuntimeError::AllocationRelease {
                    address,
                    code: last_error(),
                });
            }
            Ok(())
        }

        fn flush_instruction_cache(
            &mut self,
            address: u64,
            size: usize,
        ) -> Result<(), RuntimeError> {
            let flushed =
                unsafe { FlushInstructionCache(self.process, address as *const c_void, size) };
            if flushed == 0 {
                return Err(RuntimeError::InstructionCacheFlush {
                    address,
                    size,
                    code: last_error(),
                });
            }
            Ok(())
        }

        fn attach(&mut self) -> Result<(), RuntimeError> {
            if let Some(owner) = self.owner_thread {
                if owner != std::thread::current().id() {
                    return Err(RuntimeError::NativeDispatch {
                        detail: "RuntimeOwner attach must run on its owner thread".to_string(),
                    });
                }
            } else {
                self.owner_thread = Some(std::thread::current().id());
            }
            if unsafe { DebugActiveProcess(self.pid) } == 0 {
                self.debugger_state = "attach_failed";
                return Err(failed("DebugActiveProcess", last_error()));
            }
            self.attached = true;
            self.debugger_state = "attached";
            if unsafe { DebugSetProcessKillOnExit(0) } == 0 {
                return Err(failed("DebugSetProcessKillOnExit", last_error()));
            }
            Ok(())
        }

        fn attached(&self) -> bool {
            self.attached
        }

        fn detach(&mut self) -> Result<(), RuntimeError> {
            self.ensure_owner_thread("detach")?;
            if self.pending_event.is_some() {
                self.debugger_state = "detach_blocked_pending_event";
                return Err(RuntimeError::HookRestoreUnverified {
                    detail: "A debug event is still pending continuation".to_string(),
                });
            }
            if self
                .threads
                .iter()
                .any(|thread| thread.original_debug.is_some())
            {
                self.debugger_state = "detach_blocked_thread_cleanup";
                return Err(RuntimeError::HookRestoreUnverified {
                    detail: "Debug-register restoration is not confirmed".to_string(),
                });
            }
            if self.attached {
                if unsafe { DebugActiveProcessStop(self.pid) } == 0 {
                    self.debugger_state = "detach_failed";
                    return Err(failed("DebugActiveProcessStop", last_error()));
                }
                self.attached = false;
                self.debugger_state = "detached";
            }
            Ok(())
        }

        fn wait(&mut self, milliseconds: u32) -> Result<Option<DebugEvent>, RuntimeError> {
            self.ensure_owner_thread("wait")?;
            if self.pending_event.is_some() {
                return Err(RuntimeError::NativeDispatch {
                    detail: "RuntimeOwner cannot wait while a debug event is pending".to_string(),
                });
            }
            let mut event: DEBUG_EVENT = unsafe { std::mem::zeroed() };
            let ok = unsafe { WaitForDebugEvent(&mut event, milliseconds) };
            if ok == 0 {
                let code = last_error();
                if code == 121 || code == 258 {
                    return Ok(None);
                }
                return Err(failed("WaitForDebugEvent", code));
            }
            let mut decoded = DebugEvent {
                code: event.dwDebugEventCode,
                pid: event.dwProcessId,
                tid: event.dwThreadId,
                ..DebugEvent::default()
            };
            if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT {
                decoded.exception_code =
                    Some(unsafe { event.u.Exception.ExceptionRecord.ExceptionCode });
            } else if decoded.is_create_thread() {
                decoded.thread_handle = Some(unsafe { event.u.CreateThread.hThread as u64 });
            } else if decoded.is_create_process() {
                let info = unsafe { event.u.CreateProcessInfo };
                decoded.thread_handle = Some(info.hThread as u64);
                decoded.process_handle = Some(info.hProcess as u64);
                decoded.file_handle = Some(info.hFile as u64);
            } else if decoded.is_exit_process() {
                decoded.exit_code = Some(unsafe { event.u.ExitProcess.dwExitCode });
            } else if decoded.is_exit_thread() {
                decoded.exit_code = Some(unsafe { event.u.ExitThread.dwExitCode });
            } else if decoded.is_load_dll() {
                decoded.file_handle = Some(unsafe { event.u.LoadDll.hFile as u64 });
            }
            for thread in &mut self.threads {
                thread.run_state = ThreadRunState::Stopped;
            }
            self.pending_event = Some(decoded);
            Ok(Some(decoded))
        }

        fn resume(&mut self, event: &DebugEvent, handled: bool) -> Result<(), RuntimeError> {
            self.ensure_owner_thread("continue")?;
            if self.pending_event.as_ref() != Some(event) {
                return Err(RuntimeError::NativeDispatch {
                    detail: format!(
                        "RuntimeOwner continuation does not match pending event tid {} code {}",
                        event.tid, event.code
                    ),
                });
            }
            let status: NTSTATUS = if handled {
                DBG_CONTINUE as NTSTATUS
            } else {
                DBG_EXCEPTION_NOT_HANDLED as NTSTATUS
            };
            if unsafe { ContinueDebugEvent(event.pid, event.tid, status) } == 0 {
                return Err(failed("ContinueDebugEvent", last_error()));
            }
            self.pending_event = None;
            if event.is_exit_thread() {
                if let Some(index) = self
                    .threads
                    .iter()
                    .position(|thread| thread.tid == event.tid)
                {
                    // Windows closes the event-supplied handle after the
                    // EXIT_THREAD event is continued. Preserve only its numeric
                    // provenance in the retired report; never CloseHandle it.
                    self.retire_thread_at(index, "exited");
                }
            }
            for thread in &mut self.threads {
                thread.run_state = ThreadRunState::Running;
            }
            Ok(())
        }

        fn context(&mut self, tid: u32) -> Result<ThreadContext, RuntimeError> {
            self.ensure_owner_thread("GetThreadContext")?;
            let handle = self.require_stopped_thread(tid)?;
            let mut storage = AlignedContext::new();
            let raw = storage.raw_ptr();
            unsafe { (*raw).ContextFlags = CONTEXT_ALL };
            if unsafe { GetThreadContext(handle, raw) } == 0 {
                return Err(failed("GetThreadContext", last_error()));
            }
            let context = unsafe { *raw };
            Ok(ThreadContext {
                rax: context.Rax,
                rbx: context.Rbx,
                rcx: context.Rcx,
                rdx: context.Rdx,
                rsi: context.Rsi,
                rdi: context.Rdi,
                rbp: context.Rbp,
                r8: context.R8,
                r9: context.R9,
                r10: context.R10,
                r11: context.R11,
                r12: context.R12,
                r13: context.R13,
                r14: context.R14,
                r15: context.R15,
                rsp: context.Rsp,
                rip: context.Rip,
                eflags: u64::from(context.EFlags),
                dr0: context.Dr0,
                dr1: context.Dr1,
                dr2: context.Dr2,
                dr3: context.Dr3,
                dr6: context.Dr6,
                dr7: context.Dr7,
            })
        }

        fn set_context(&mut self, tid: u32, context: &ThreadContext) -> Result<(), RuntimeError> {
            self.ensure_owner_thread("SetThreadContext")?;
            let handle = self.require_stopped_thread(tid)?;
            let mut storage = AlignedContext::new();
            let raw = storage.raw_ptr();
            // `GetThreadContext` populates only the groups its `ContextFlags`
            // selects. The buffer starts zeroed, so without this the fetch
            // below returns none of the groups this call then writes back and
            // every non-modelled register (XMM, floating point, control,
            // segment and debug state) would be written back as zeros.
            unsafe { (*raw).ContextFlags = CONTEXT_ALL };
            if unsafe { GetThreadContext(handle, raw) } == 0 {
                return Err(failed("GetThreadContext", last_error()));
            }
            unsafe {
                (*raw).Rax = context.rax;
                (*raw).Rbx = context.rbx;
                (*raw).Rcx = context.rcx;
                (*raw).Rdx = context.rdx;
                (*raw).Rsi = context.rsi;
                (*raw).Rdi = context.rdi;
                (*raw).Rbp = context.rbp;
                (*raw).R8 = context.r8;
                (*raw).R9 = context.r9;
                (*raw).R10 = context.r10;
                (*raw).R11 = context.r11;
                (*raw).R12 = context.r12;
                (*raw).R13 = context.r13;
                (*raw).R14 = context.r14;
                (*raw).R15 = context.r15;
                (*raw).Rsp = context.rsp;
                (*raw).Rip = context.rip;
                (*raw).EFlags = context.eflags as u32;
                (*raw).Dr0 = context.dr0;
                (*raw).Dr1 = context.dr1;
                (*raw).Dr2 = context.dr2;
                (*raw).Dr3 = context.dr3;
                (*raw).Dr6 = context.dr6;
                (*raw).Dr7 = context.dr7;
                (*raw).ContextFlags = CONTEXT_ALL;
            }
            if unsafe { SetThreadContext(handle, raw) } == 0 {
                return Err(failed("SetThreadContext", last_error()));
            }
            Ok(())
        }

        fn adopt_thread(&mut self, tid: u32, handle: u64) -> Result<(), RuntimeError> {
            self.ensure_owner_thread("adopt_thread")?;
            let event = self
                .pending_event
                .ok_or_else(|| RuntimeError::NativeDispatch {
                    detail: "Thread adoption requires its pending create event".to_string(),
                })?;
            let provenance = if event.is_create_process() {
                HandleProvenance::CreateProcessEvent
            } else if event.is_create_thread() {
                HandleProvenance::CreateThreadEvent
            } else {
                return Err(RuntimeError::NativeDispatch {
                    detail: "Only create events may supply an owned thread handle".to_string(),
                });
            };
            if event.tid != tid {
                return Err(RuntimeError::NativeDispatch {
                    detail: format!(
                        "Create event tid {} cannot adopt thread tid {tid}",
                        event.tid
                    ),
                });
            }
            // A numeric handle is not identity. The conflict rule asks the OS
            // whether each conflicting record still owns its own instance: an
            // instance whose handle was closed or handed to another thread is
            // retired, and only a live instance still refuses the adoption.
            let recorded = self
                .threads
                .iter()
                .map(|thread| RecordedInstance {
                    tid: thread.tid,
                    instance: thread.instance,
                    handle_value: thread.handle_value,
                    state: recorded_instance_state(
                        thread.tid,
                        Self::observe_recorded_handle(thread),
                    ),
                })
                .collect::<Vec<_>>();
            match resolve_adoption(&recorded, tid, handle) {
                Adoption::Duplicate {
                    tid: owner_tid,
                    instance,
                } => {
                    return Err(RuntimeError::NativeDispatch {
                        detail: format!(
                            "Create event tid {tid} handle {handle:#x} conflicts with live owned \
                             thread tid {owner_tid} instance {instance}"
                        ),
                    });
                }
                Adoption::Unresolved {
                    tid: owner_tid,
                    instance,
                } => {
                    // A record whose observation failed may still own the tid or
                    // the value, and a failed wait proves neither death nor
                    // reuse. The new event is refused and the unresolved record
                    // keeps its ownership.
                    return Err(RuntimeError::NativeDispatch {
                        detail: format!(
                            "Create event tid {tid} handle {handle:#x} cannot be resolved against \
                             unresolved owned thread tid {owner_tid} instance {instance}: its OS \
                             observation failed and is not a termination proof"
                        ),
                    });
                }
                Adoption::Adopt { stale } => {
                    for index in stale.into_iter().rev() {
                        self.retire_thread_at(index, "handle_reused");
                    }
                }
            }
            self.next_thread_instance += 1;
            self.threads.push(OwnedThread {
                tid,
                instance: self.next_thread_instance,
                handle_value: handle,
                handle: handle as HANDLE,
                provenance,
                run_state: ThreadRunState::Stopped,
                original_debug: None,
                cleanup_state: "not_armed",
                handle_trusted: true,
                error: None,
            });
            Ok(())
        }

        fn arm_thread(
            &mut self,
            tid: u32,
            entry: u64,
            acknowledgement: u64,
        ) -> Result<(), RuntimeError> {
            self.ensure_owner_thread("arm_thread")?;
            let mut context = self.context(tid)?;
            if context.dr7 & 0xFF != 0 {
                return Err(failed(
                    "a thread already has active hardware breakpoints",
                    0,
                ));
            }
            let saved = [
                context.dr0,
                context.dr1,
                context.dr2,
                context.dr3,
                context.dr6,
                context.dr7,
            ];
            let thread = self.thread_mut(tid).ok_or(RuntimeError::SessionNotOpen)?;
            thread.original_debug = Some(saved);
            thread.cleanup_state = "armed";
            context.dr0 = entry;
            context.dr1 = acknowledgement;
            context.dr6 = 0;
            context.dr7 = (context.dr7 & !0xFFFF_00FF) | 5;
            self.set_context(tid, &context)
        }

        fn restore_threads(&mut self) -> Result<(), RuntimeError> {
            self.ensure_owner_thread("restore_threads")?;
            let pending = self
                .pending_event
                .ok_or_else(|| RuntimeError::NativeDispatch {
                    detail: "Debug-register restoration requires a stopped-event barrier"
                        .to_string(),
                })?;
            // One fresh observation per record decides that record's class for
            // this pass, and the three outcomes are never collapsed into one.
            // `Stale` is a termination or reuse proof, `Unobserved` is an
            // observation failure that keeps the owner, and `Live` is the only
            // class this loop may write debug registers through.
            //
            // A thread that terminated without a continued EXIT_THREAD has no
            // live debug state, and its numeric handle may already belong to a
            // different thread, so a stale record is retired instead of being
            // read or written through that value.
            let mut terminated = Vec::new();
            let mut unobserved = Vec::new();
            let mut restorable = Vec::new();
            for thread in self.threads.iter() {
                let Some(saved) = thread.original_debug else {
                    continue;
                };
                let observation = Self::observe_recorded_handle(thread);
                match recorded_instance_state(thread.tid, observation) {
                    RecordedInstanceState::Stale => {
                        terminated.push((thread.tid, thread.instance, "exited"))
                    }
                    RecordedInstanceState::Unobserved => {
                        let (failed_api, last_error) = if observation.wait_observed {
                            ("GetThreadId", observation.identity_last_error)
                        } else {
                            (
                                "WaitForSingleObject(WAIT_FAILED)",
                                observation.wait_last_error,
                            )
                        };
                        unobserved.push((thread.tid, thread.instance, failed_api, last_error))
                    }
                    RecordedInstanceState::Live => {
                        restorable.push((thread.tid, thread.instance, saved))
                    }
                }
            }
            for (tid, instance, cleanup_state) in terminated {
                self.retire_thread_named(tid, instance, cleanup_state);
            }
            // A record whose observation failed keeps its slot, its original
            // debug state and its numeric handle, and loses its handle trust:
            // the cleanup obligation stays owed and nothing may be read or
            // written through a value the OS did not confirm for this instance.
            for (tid, instance, failed_api, last_error) in &unobserved {
                self.retain_unobserved_thread_at(*tid, *instance, failed_api, *last_error);
            }
            for (tid, instance, saved) in restorable {
                if let Some(thread) = self
                    .threads
                    .iter_mut()
                    .find(|thread| thread.tid == tid && thread.instance == instance)
                {
                    // A fresh successful observation is the only evidence that
                    // re-authorizes the handle of a record that lost trust.
                    thread.handle_trusted = true;
                }
                if self
                    .thread(tid)
                    .map(|thread| thread.run_state != ThreadRunState::Stopped)
                    .unwrap_or(true)
                {
                    return Err(RuntimeError::NativeDispatch {
                        detail: format!(
                            "Thread tid {tid} instance {instance} is not stopped at barrier tid {}",
                            pending.tid
                        ),
                    });
                }
                let mut context = match self.context(tid) {
                    Ok(context) => context,
                    Err(error) => {
                        if let Some(thread) = self.thread_mut(tid) {
                            thread.cleanup_state = "restore_failed";
                            thread.error = Some(error.message());
                        }
                        return Err(error);
                    }
                };
                context.dr0 = saved[0];
                context.dr1 = saved[1];
                context.dr2 = saved[2];
                context.dr3 = saved[3];
                context.dr6 = saved[4];
                context.dr7 = saved[5];
                if let Err(error) = self.set_context(tid, &context) {
                    if let Some(thread) = self.thread_mut(tid) {
                        thread.cleanup_state = "restore_failed";
                        thread.error = Some(error.message());
                    }
                    return Err(error);
                }
                if let Some(thread) = self.thread_mut(tid) {
                    thread.original_debug = None;
                    thread.cleanup_state = "original_restored";
                    thread.error = None;
                }
            }
            // Every unresolved record still owes its cleanup, so this pass can
            // only report success when none is left. Refusing here is what keeps
            // an observation failure out of the completed vocabulary: the owner
            // stays retained with its debug state, handle and error category
            // until a later observation actually resolves the instance.
            if let Some((tid, instance, failed_api, last_error)) = unobserved.first() {
                return Err(RuntimeError::NativeDispatch {
                    detail: format!(
                        "Thread tid {tid} instance {instance} could not be identified during \
                         debug-register restoration because {failed_api} failed; \
                         last_error={last_error}; the owner is retained and unresolved"
                    ),
                });
            }
            Ok(())
        }

        fn all_threads_exited(&mut self) -> Result<bool, RuntimeError> {
            if self.threads.is_empty() {
                return Ok(false);
            }
            for thread in &self.threads {
                let result = wait_for_single_object(thread.handle, 0);
                match result {
                    super::WAIT_OBJECT_0 => {}
                    0xFFFF_FFFF => {
                        return Err(RuntimeError::NativeDispatch {
                            detail: format!(
                                "WaitForSingleObject returned WAIT_FAILED for tid {} instance {} handle {:#x}; last_error={}",
                                thread.tid,
                                thread.instance,
                                thread.handle_value,
                                last_error()
                            ),
                        });
                    }
                    _ => return Ok(false),
                }
            }
            Ok(true)
        }

        fn thread_signalled(&mut self, tid: u32) -> Result<bool, RuntimeError> {
            match self.thread(tid) {
                Some(thread) => match wait_for_single_object(thread.handle, 0) {
                    super::WAIT_OBJECT_0 => Ok(true),
                    0xFFFF_FFFF => Err(RuntimeError::NativeDispatch {
                        detail: format!(
                            "WaitForSingleObject returned WAIT_FAILED for tid {} instance {} handle {:#x}; last_error={}",
                            thread.tid,
                            thread.instance,
                            thread.handle_value,
                            last_error()
                        ),
                    }),
                    _ => Ok(false),
                },
                None => Ok(true),
            }
        }

        fn debug_break(&mut self) -> Result<(), RuntimeError> {
            self.ensure_owner_thread("debug_break")?;
            if self.pending_event.is_some() {
                return Err(RuntimeError::NativeDispatch {
                    detail: "DebugBreakProcess requires no pending debug event".to_string(),
                });
            }
            if unsafe { DebugBreakProcess(self.process) } == 0 {
                return Err(failed("DebugBreakProcess", last_error()));
            }
            Ok(())
        }
    }

    impl RuntimeOwnerSession for WindowsDebugSession {
        fn begin_cleanup_barrier(&mut self) -> Result<(), RuntimeError> {
            <Self as DebugSession>::debug_break(self)
        }

        fn runtime_owner_snapshot(&self) -> RuntimeOwnerSnapshot {
            let mut threads = self.retired_threads.clone();
            threads.extend(self.threads.iter().map(Self::snapshot_thread));
            RuntimeOwnerSnapshot {
                debugger_state: self.debugger_state,
                threads,
            }
        }
    }

    pub struct WindowsRemoteSession {
        pid: u32,
        process: HANDLE,
    }

    unsafe impl Send for WindowsRemoteSession {}

    impl WindowsRemoteSession {
        pub fn open(pid: u32) -> Result<Self, RuntimeError> {
            let process = unsafe { OpenProcess(RUNTIME_ACCESS, 0, pid) };
            if process.is_null() || process == INVALID_HANDLE_VALUE {
                return Err(RuntimeError::OpenProcess {
                    pid,
                    code: last_error(),
                });
            }
            Ok(Self { pid, process })
        }
    }

    impl Drop for WindowsRemoteSession {
        fn drop(&mut self) {
            self.close();
        }
    }

    impl RemoteSession for WindowsRemoteSession {
        fn pid(&self) -> u32 {
            self.pid
        }

        fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            let mut buffer = vec![0u8; size];
            let mut transferred: usize = 0;
            let ok = unsafe {
                ReadProcessMemory(
                    self.process,
                    address as *const c_void,
                    buffer.as_mut_ptr() as *mut c_void,
                    size,
                    &mut transferred,
                )
            };
            if ok == 0 {
                return Err(RuntimeError::MemoryRead {
                    address,
                    size,
                    code: last_error(),
                });
            }
            if transferred != size {
                return Err(RuntimeError::ShortRead {
                    address,
                    expected: size,
                    actual: transferred,
                });
            }
            Ok(buffer)
        }

        fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
            let mut transferred: usize = 0;
            let ok = unsafe {
                windows_sys::Win32::System::Diagnostics::Debug::WriteProcessMemory(
                    self.process,
                    address as *mut c_void,
                    data.as_ptr() as *const c_void,
                    data.len(),
                    &mut transferred,
                )
            };
            if ok == 0 {
                return Err(RuntimeError::MemoryWrite {
                    address,
                    size: data.len(),
                    code: last_error(),
                });
            }
            if transferred != data.len() {
                return Err(RuntimeError::ShortWrite {
                    address,
                    expected: data.len(),
                    actual: transferred,
                });
            }
            Ok(())
        }

        fn allocate(&mut self, size: usize) -> Result<u64, RuntimeError> {
            let allocation = unsafe {
                VirtualAllocEx(
                    self.process,
                    std::ptr::null(),
                    size,
                    MEM_COMMIT_RESERVE,
                    PAGE_EXECUTE_READWRITE,
                )
            };
            if allocation.is_null() {
                return Err(RuntimeError::AllocationUnavailable {
                    address: 0,
                    size: size as u64,
                });
            }
            Ok(allocation as u64)
        }

        fn free(&mut self, address: u64) -> Result<(), RuntimeError> {
            let freed =
                unsafe { VirtualFreeEx(self.process, address as *mut c_void, 0, MEM_RELEASE) };
            if freed == 0 {
                return Err(RuntimeError::AllocationRelease {
                    address,
                    code: last_error(),
                });
            }
            Ok(())
        }

        fn create_remote_thread(&mut self, start: u64) -> Result<u64, RuntimeError> {
            let mut thread_id: u32 = 0;
            let thread = unsafe {
                CreateRemoteThread(
                    self.process,
                    std::ptr::null(),
                    0,
                    Some(std::mem::transmute::<
                        u64,
                        unsafe extern "system" fn(*mut c_void) -> u32,
                    >(start)),
                    std::ptr::null(),
                    0,
                    &mut thread_id,
                )
            };
            if thread.is_null() || thread == INVALID_HANDLE_VALUE {
                return Err(failed("CreateRemoteThread", last_error()));
            }
            Ok(thread as u64)
        }

        fn wait_thread(&mut self, thread: u64, milliseconds: u32) -> Result<u32, RuntimeError> {
            Ok(unsafe { WaitForSingleObject(thread as HANDLE, milliseconds) })
        }

        fn thread_exit_code(&mut self, thread: u64) -> Result<u32, RuntimeError> {
            let mut code: u32 = 0;
            if unsafe { GetExitCodeThread(thread as HANDLE, &mut code) } == 0 {
                return Err(failed("GetExitCodeThread", last_error()));
            }
            Ok(code)
        }

        fn close_thread(&mut self, thread: u64) {
            unsafe { CloseHandle(thread as HANDLE) };
        }

        fn close(&mut self) {
            if !self.process.is_null() && self.process != INVALID_HANDLE_VALUE {
                unsafe { CloseHandle(self.process) };
                self.process = std::ptr::null_mut();
            }
        }
    }

    // `MEMORY_BASIC_INFORMATION` is imported for parity with the shipped query
    // path; the oracle allocates at a null base and needs no region walk.
    #[allow(dead_code)]
    fn _unused_signature_probe() -> usize {
        std::mem::size_of::<MEMORY_BASIC_INFORMATION>()
    }

    /// Test-only control over the observation seam.
    ///
    /// Compiled only under `test-observation`, so the shipped binding cannot
    /// select an unobservable handle. The selected value makes the *real*
    /// `WaitForSingleObject` fail on that one handle while every other call, and
    /// every other decision, still runs on the real OS handle.
    #[cfg(feature = "test-observation")]
    pub struct ObservationFault;

    #[cfg(feature = "test-observation")]
    impl ObservationFault {
        /// Make the wait fail for exactly this numeric handle value.
        #[cfg(feature = "test-observation")]
        pub fn make_unobservable(handle: u64) {
            UNOBSERVABLE_HANDLE.store(handle, std::sync::atomic::Ordering::SeqCst);
        }

        /// Make the identity query fail after a successful running wait.
        #[cfg(feature = "test-observation")]
        pub fn make_identity_unreadable(handle: u64) {
            UNIDENTIFIABLE_HANDLE.store(handle, std::sync::atomic::Ordering::SeqCst);
        }

        /// Clear the fault so a later observation runs against the real OS.
        #[cfg(feature = "test-observation")]
        pub fn clear() {
            UNOBSERVABLE_HANDLE.store(0, std::sync::atomic::Ordering::SeqCst);
            UNIDENTIFIABLE_HANDLE.store(0, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// One public, read-only view of an owned thread record, published only for
    /// the `test-observation` acceptance build so an integration test can assert
    /// on the state the real binding recorded.
    #[cfg(feature = "test-observation")]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ObservedThreadRecord {
        pub tid: u32,
        pub instance: u64,
        pub handle: u64,
        pub run_state: &'static str,
        pub cleanup_state: &'static str,
        pub error: Option<String>,
    }

    impl WindowsDebugSession {
        /// The real owner report as a public, read-only view.
        ///
        /// Compiled only under `test-observation`; the product keeps the
        /// crate-private reporting trait.
        #[cfg(feature = "test-observation")]
        pub fn observed_threads(&self) -> Vec<ObservedThreadRecord> {
            self.runtime_owner_snapshot()
                .threads
                .into_iter()
                .map(|thread| ObservedThreadRecord {
                    tid: thread.tid,
                    instance: thread.instance,
                    handle: thread.handle,
                    run_state: thread.run_state,
                    cleanup_state: thread.cleanup_state,
                    error: thread.error,
                })
                .collect()
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{WindowsDebugSession, WindowsRemoteSession, LIVE_ADD_ACCESS};

/// The observation-failure seam, exported only for the `test-observation`
/// acceptance build. The product never compiles it.
#[cfg(all(windows, feature = "test-observation"))]
pub use windows_impl::{ObservationFault, ObservedThreadRecord};

#[cfg(test)]
mod identity_tests {
    use super::*;

    /// One observation whose `WaitForSingleObject` succeeded and produced a real
    /// signal value. `handle_thread_id` is `None` when the identity query
    /// failed, which is a different fact than the wait failing.
    fn observed(wait_result: u32, handle_thread_id: Option<u32>) -> RecordedHandleObservation {
        RecordedHandleObservation {
            wait_result,
            wait_observed: true,
            handle_thread_id,
            wait_last_error: 0,
            identity_last_error: 0,
        }
    }

    /// One observation whose `WaitForSingleObject` itself failed: `WAIT_FAILED`
    /// carries the failure code and no signal information at all.
    fn wait_failed() -> RecordedHandleObservation {
        // The code the seam raises. The oracle only needs a real, non-zero
        // category to be carried with the failure.
        const ERROR_INVALID_HANDLE: u32 = 6;
        RecordedHandleObservation {
            wait_result: WAIT_FAILED,
            wait_observed: false,
            handle_thread_id: None,
            wait_last_error: ERROR_INVALID_HANDLE,
            identity_last_error: 0,
        }
    }

    fn record(
        tid: u32,
        instance: u64,
        handle_value: u64,
        state: RecordedInstanceState,
    ) -> RecordedInstance {
        RecordedInstance {
            tid,
            instance,
            handle_value,
            state,
        }
    }

    #[test]
    fn a_terminated_thread_never_keeps_owning_its_numeric_handle() {
        assert_eq!(
            recorded_instance_state(10, observed(WAIT_OBJECT_0, None)),
            RecordedInstanceState::Stale,
            "WAIT_OBJECT_0 means the thread terminated; the handle is not ownership"
        );
    }

    #[test]
    fn a_failed_wait_is_an_observation_failure_not_a_termination_proof() {
        let failed = wait_failed();
        assert_ne!(
            failed.wait_last_error, 0,
            "the failing wait carries the GetLastError category it was observed with"
        );
        assert_eq!(
            recorded_instance_state(10, failed),
            RecordedInstanceState::Unobserved,
            "WAIT_FAILED is the wait failing, so the record stays unresolved and never settles"
        );
        assert_eq!(
            observed(WAIT_OBJECT_0, None).wait_last_error,
            0,
            "a successful observation carries no failure category"
        );
    }

    #[test]
    fn a_numeric_value_that_names_another_thread_is_not_this_instance() {
        assert_eq!(
            recorded_instance_state(10, observed(258, Some(11))),
            RecordedInstanceState::Stale,
            "the value was handed to tid 11, so instance (10) no longer owns it"
        );
    }

    #[test]
    fn only_a_matching_thread_identity_authorizes_the_handle() {
        assert_eq!(
            recorded_instance_state(10, observed(258, Some(10))),
            RecordedInstanceState::Live
        );
        assert_eq!(
            recorded_instance_state(10, observed(258, None)),
            RecordedInstanceState::Unobserved,
            "an unreadable id retains the owner but cannot authorize handle access"
        );
    }

    #[test]
    fn an_unobserved_record_is_never_retired_to_let_a_new_event_adopt() {
        // A wait failure is not a reuse proof. Retiring this record would let a
        // later create event take a value the record may still own, so the event
        // is refused and the unresolved record keeps its ownership instead.
        let records = [record(10, 1, 0x44, RecordedInstanceState::Unobserved)];
        assert_eq!(
            resolve_adoption(&records, 11, 0x44),
            Adoption::Unresolved {
                tid: 10,
                instance: 1
            },
            "an unobserved record blocks the numeric collision instead of being reclaimed"
        );
        assert_eq!(
            resolve_adoption(&records, 10, 0x44),
            Adoption::Unresolved {
                tid: 10,
                instance: 1
            },
            "the same tid is blocked by the unobserved record too"
        );
    }

    #[test]
    fn a_reused_numeric_handle_is_adopted_as_a_new_instance() {
        // The R2 sequence: instance 1 (tid 10, handle 0x44) terminated, so the
        // kernel may hand 0x44 to tid 11. The record must be retired rather
        // than blocking the adoption, and the value must never be identity.
        let records = [record(10, 1, 0x44, RecordedInstanceState::Stale)];
        assert_eq!(
            resolve_adoption(&records, 11, 0x44),
            Adoption::Adopt { stale: vec![0] }
        );
    }

    #[test]
    fn a_live_instance_still_refuses_a_duplicate_adoption() {
        let records = [record(10, 1, 0x44, RecordedInstanceState::Live)];
        assert_eq!(
            resolve_adoption(&records, 10, 0x44),
            Adoption::Duplicate {
                tid: 10,
                instance: 1
            }
        );
    }

    #[test]
    fn an_unrelated_live_record_is_never_retired() {
        let records = [record(7, 1, 0x99, RecordedInstanceState::Live)];
        assert_eq!(
            resolve_adoption(&records, 11, 0x44),
            Adoption::Adopt { stale: vec![] }
        );
    }

    #[test]
    fn a_collision_resolves_by_instance_liveness_not_by_the_numeric_value() {
        // Two records, one numeric collision each. Only the record the OS no
        // longer reports is reclaimed; a live record keeps blocking the event
        // instead of losing its value to the newer event.
        let records = [
            record(10, 1, 0x44, RecordedInstanceState::Stale),
            record(20, 2, 0x99, RecordedInstanceState::Live),
        ];
        assert_eq!(
            resolve_adoption(&records, 11, 0x44),
            Adoption::Adopt { stale: vec![0] },
            "a stale numeric collision is reclaimed, and the unrelated live record is kept"
        );
        assert_eq!(
            resolve_adoption(&records, 30, 0x99),
            Adoption::Duplicate {
                tid: 20,
                instance: 2
            },
            "a live record blocks any event that would share its tid"
        );
    }
}
