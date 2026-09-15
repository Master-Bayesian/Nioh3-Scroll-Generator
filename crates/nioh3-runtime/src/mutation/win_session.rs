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

#[cfg(windows)]
mod windows_impl {
    use super::{
        DebugEvent, DebugSession, RemoteSession, ThreadContext, DBG_CONTINUE,
        DBG_EXCEPTION_NOT_HANDLED,
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
        CreateRemoteThread, GetExitCodeThread, OpenProcess, WaitForSingleObject,
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

    pub struct WindowsDebugSession {
        pid: u32,
        process: HANDLE,
        attached: bool,
        threads: Vec<(u32, HANDLE)>,
        /// The six debug registers exactly as the shipped code stores them.
        original_debug: Vec<(u32, [u64; 6])>,
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
                threads: Vec::new(),
                original_debug: Vec::new(),
                module_name: module_name.to_string(),
            })
        }

        fn thread(&self, tid: u32) -> Option<HANDLE> {
            self.threads
                .iter()
                .find(|(id, _)| *id == tid)
                .map(|(_, handle)| *handle)
        }
    }

    impl Drop for WindowsDebugSession {
        fn drop(&mut self) {
            self.detach().ok();
            for (_, handle) in self.threads.drain(..) {
                unsafe { CloseHandle(handle) };
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
            if unsafe { DebugActiveProcess(self.pid) } == 0 {
                return Err(failed("DebugActiveProcess", last_error()));
            }
            self.attached = true;
            if unsafe { DebugSetProcessKillOnExit(0) } == 0 {
                return Err(failed("DebugSetProcessKillOnExit", last_error()));
            }
            Ok(())
        }

        fn attached(&self) -> bool {
            self.attached
        }

        fn detach(&mut self) -> Result<(), RuntimeError> {
            if !self.original_debug.is_empty() {
                return Err(RuntimeError::HookRestoreUnverified {
                    detail: "Debug-register restoration is not confirmed".to_string(),
                });
            }
            if self.attached {
                if unsafe { DebugActiveProcessStop(self.pid) } == 0 {
                    return Err(failed("DebugActiveProcessStop", last_error()));
                }
                self.attached = false;
            }
            Ok(())
        }

        fn wait(&mut self, milliseconds: u32) -> Result<Option<DebugEvent>, RuntimeError> {
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
            Ok(Some(decoded))
        }

        fn resume(&mut self, event: &DebugEvent, handled: bool) -> Result<(), RuntimeError> {
            let status: NTSTATUS = if handled {
                DBG_CONTINUE as NTSTATUS
            } else {
                DBG_EXCEPTION_NOT_HANDLED as NTSTATUS
            };
            if unsafe { ContinueDebugEvent(event.pid, event.tid, status) } == 0 {
                return Err(failed("ContinueDebugEvent", last_error()));
            }
            Ok(())
        }

        fn context(&mut self, tid: u32) -> Result<ThreadContext, RuntimeError> {
            let handle = self.thread(tid).ok_or(RuntimeError::SessionNotOpen)?;
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
            let handle = self.thread(tid).ok_or(RuntimeError::SessionNotOpen)?;
            let mut storage = AlignedContext::new();
            let raw = storage.raw_ptr();
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
            self.threads.push((tid, handle as HANDLE));
            Ok(())
        }

        fn arm_thread(
            &mut self,
            tid: u32,
            entry: u64,
            acknowledgement: u64,
        ) -> Result<(), RuntimeError> {
            let mut context = self.context(tid)?;
            if context.dr7 & 0xFF != 0 {
                return Err(failed(
                    "a thread already has active hardware breakpoints",
                    0,
                ));
            }
            self.original_debug.push((
                tid,
                [
                    context.dr0,
                    context.dr1,
                    context.dr2,
                    context.dr3,
                    context.dr6,
                    context.dr7,
                ],
            ));
            context.dr0 = entry;
            context.dr1 = acknowledgement;
            context.dr6 = 0;
            context.dr7 = (context.dr7 & !0xFFFF_00FF) | 5;
            self.set_context(tid, &context)
        }

        fn restore_threads(&mut self) -> Result<(), RuntimeError> {
            for (tid, saved) in self.original_debug.clone() {
                if self.thread(tid).is_none() {
                    continue;
                }
                // A terminated thread has no live debug-register state.
                if self.thread_signalled(tid)? {
                    continue;
                }
                let mut context = self.context(tid)?;
                context.dr0 = saved[0];
                context.dr1 = saved[1];
                context.dr2 = saved[2];
                context.dr3 = saved[3];
                context.dr6 = saved[4];
                context.dr7 = saved[5];
                self.set_context(tid, &context)?;
            }
            self.original_debug.clear();
            Ok(())
        }

        fn all_threads_exited(&mut self) -> Result<bool, RuntimeError> {
            if self.threads.is_empty() {
                return Ok(false);
            }
            for (_, handle) in self.threads.clone() {
                if unsafe { WaitForSingleObject(handle, 0) } != super::WAIT_OBJECT_0 {
                    return Ok(false);
                }
            }
            Ok(true)
        }

        fn thread_signalled(&mut self, tid: u32) -> Result<bool, RuntimeError> {
            match self.thread(tid) {
                Some(handle) => {
                    Ok(unsafe { WaitForSingleObject(handle, 0) } == super::WAIT_OBJECT_0)
                }
                None => Ok(true),
            }
        }

        fn debug_break(&mut self) -> Result<(), RuntimeError> {
            if unsafe { DebugBreakProcess(self.process) } == 0 {
                return Err(failed("DebugBreakProcess", last_error()));
            }
            Ok(())
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
}

#[cfg(windows)]
pub use windows_impl::{WindowsDebugSession, WindowsRemoteSession, LIVE_ADD_ACCESS};
