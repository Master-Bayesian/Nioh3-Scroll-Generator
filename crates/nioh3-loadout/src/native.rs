//! Retained game machine code on a replay image, with two explicitly scoped
//! adapters (D/TLS getter and one multiplier call site). Removing an adapter
//! does not certify unknown memory, host TLS or reconstructed candidate inputs.
//!
//! The memory sits at fixed addresses, so a process holds one `Native` and runs one evaluation at a time. Watched
//! chunks stay read-only until first written; from then on write watching finds the pages a run writes, and they are
//! restored from the template before the next run.

#![cfg(windows)]

use crate::oracle_contract::{OracleEnvironment, SnapshotKnowledge};
use std::arch::global_asm;
use std::ffi::c_void;
use std::path::Path;

#[repr(C)]
struct Ctx {
    gp: [u64; 16],
    xmm: [[u8; 16]; 16],
    entry: u64,
    host_rsp: u64,
    out_rax: u64,
    faulted: u64,
    fault_code: u64,
    fault_rip: u64,
    fault_addr: u64,
    out_xmm0: [u8; 16],
}

#[no_mangle]
static mut NIOH3_NATIVE_CTX: *mut Ctx = std::ptr::null_mut();
#[no_mangle]
static mut NIOH3_NATIVE_ACTIVE: u64 = 0;
/// Nonzero: enter MAIN with the trap flag set and record every instruction it executes inside [TRACE_LO, TRACE_HI).
#[no_mangle]
static mut NIOH3_NATIVE_TRACE: u64 = 0;
static mut TRACE_LO: u64 = 0;
static mut TRACE_HI: u64 = 0;
static mut TRACE_BUF: *mut u64 = std::ptr::null_mut();
static mut TRACE_LEN: usize = 0;
static mut TRACE_CAP: usize = 0;

/// The guarded chunks (sorted), the state of each of their pages (0 read-only, 1 writable), and the pages the fault
/// handler has made writable. Set once by `Native::open`; the handler reads them.
static mut OWNER_THREAD: u32 = 0;
static mut COLD: *const u64 = std::ptr::null();
static mut COLD_STATE: *mut u8 = std::ptr::null_mut();
static mut COLD_LEN: usize = 0;
static mut PROMOTED: *mut u64 = std::ptr::null_mut();
static mut PROMOTED_LEN: usize = 0;

/// A write to a read-only guarded page: make the page writable and record it, then retry the write.
unsafe fn promote(record: *mut u8) -> bool {
    let code = *(record as *const u32);
    if code != 0xC000_0005
        || *(record.add(0x18) as *const u32) < 2
        || *(record.add(0x20) as *const u64) != 1
    {
        return false;
    }
    let page = *(record.add(0x28) as *const u64) & !(PAGE - 1);
    let chunk = page & !(CHUNK - 1);
    let cold = std::slice::from_raw_parts(COLD, COLD_LEN);
    let Ok(i) = cold.binary_search(&chunk) else {
        return false;
    };
    let at = i * PAGES_PER_CHUNK + ((page - chunk) / PAGE) as usize;
    if *COLD_STATE.add(at) != 0 {
        return false;
    }
    let mut old = 0u32;
    if VirtualProtect(
        page as *mut c_void,
        PAGE as usize,
        PAGE_EXECUTE_READWRITE,
        &mut old,
    ) == 0
    {
        return false;
    }
    *COLD_STATE.add(at) = 1;
    *PROMOTED.add(PROMOTED_LEN) = page;
    PROMOTED_LEN += 1;
    true
}

extern "C" {
    fn nioh3_native_call(ctx: *mut Ctx);
    fn nioh3_native_restore();
}

// Enter MAIN with the context's registers on its stack and return here at its RET; a fault resumes at the restore.
global_asm!(
    ".globl nioh3_native_call",
    "nioh3_native_call:",
    "push rbx",
    "push rbp",
    "push rsi",
    "push rdi",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "sub rsp, 0xB8",
    "movdqu [rsp+0x00], xmm6",
    "movdqu [rsp+0x10], xmm7",
    "movdqu [rsp+0x20], xmm8",
    "movdqu [rsp+0x30], xmm9",
    "movdqu [rsp+0x40], xmm10",
    "movdqu [rsp+0x50], xmm11",
    "movdqu [rsp+0x60], xmm12",
    "movdqu [rsp+0x70], xmm13",
    "movdqu [rsp+0x80], xmm14",
    "movdqu [rsp+0x90], xmm15",
    "stmxcsr [rsp+0xA0]",
    "mov dword ptr [rsp+0xA4], 0x1F80",
    "ldmxcsr [rsp+0xA4]",
    "mov [rip+NIOH3_NATIVE_CTX], rcx",
    "mov [rcx+0x188], rsp",
    "mov qword ptr [rip+NIOH3_NATIVE_ACTIVE], 1",
    "movdqu xmm0, [rcx+0x80]",
    "movdqu xmm1, [rcx+0x90]",
    "movdqu xmm2, [rcx+0xA0]",
    "movdqu xmm3, [rcx+0xB0]",
    "movdqu xmm4, [rcx+0xC0]",
    "movdqu xmm5, [rcx+0xD0]",
    "movdqu xmm6, [rcx+0xE0]",
    "movdqu xmm7, [rcx+0xF0]",
    "movdqu xmm8, [rcx+0x100]",
    "movdqu xmm9, [rcx+0x110]",
    "movdqu xmm10, [rcx+0x120]",
    "movdqu xmm11, [rcx+0x130]",
    "movdqu xmm12, [rcx+0x140]",
    "movdqu xmm13, [rcx+0x150]",
    "movdqu xmm14, [rcx+0x160]",
    "movdqu xmm15, [rcx+0x170]",
    "mov rsp, [rcx+0x38]",
    "lea rax, [rip+nioh3_native_return]",
    "mov [rsp], rax",
    "mov rax, [rcx+0x00]",
    "mov rbx, [rcx+0x08]",
    "mov rdx, [rcx+0x18]",
    "mov rsi, [rcx+0x20]",
    "mov rdi, [rcx+0x28]",
    "mov rbp, [rcx+0x30]",
    "mov r8, [rcx+0x40]",
    "mov r9, [rcx+0x48]",
    "mov r10, [rcx+0x50]",
    "mov r11, [rcx+0x58]",
    "mov r12, [rcx+0x60]",
    "mov r13, [rcx+0x68]",
    "mov r14, [rcx+0x70]",
    "mov r15, [rcx+0x78]",
    "cld",
    "push qword ptr [rcx+0x180]",
    "cmp qword ptr [rip+NIOH3_NATIVE_TRACE], 0",
    "mov rcx, [rcx+0x10]",
    "jz 2f",
    "pushfq",
    "or qword ptr [rsp], 0x100",
    "popfq",
    "2:",
    "ret",
    "nioh3_native_return:",
    "mov rcx, [rip+NIOH3_NATIVE_CTX]",
    "mov [rcx+0x190], rax",
    "movdqu [rcx+0x1B8], xmm0",
    ".globl nioh3_native_restore",
    "nioh3_native_restore:",
    "mov qword ptr [rip+NIOH3_NATIVE_ACTIVE], 0",
    "mov rcx, [rip+NIOH3_NATIVE_CTX]",
    "mov rsp, [rcx+0x188]",
    "ldmxcsr [rsp+0xA0]",
    "movdqu xmm6, [rsp+0x00]",
    "movdqu xmm7, [rsp+0x10]",
    "movdqu xmm8, [rsp+0x20]",
    "movdqu xmm9, [rsp+0x30]",
    "movdqu xmm10, [rsp+0x40]",
    "movdqu xmm11, [rsp+0x50]",
    "movdqu xmm12, [rsp+0x60]",
    "movdqu xmm13, [rsp+0x70]",
    "movdqu xmm14, [rsp+0x80]",
    "movdqu xmm15, [rsp+0x90]",
    "add rsp, 0xB8",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rdi",
    "pop rsi",
    "pop rbp",
    "pop rbx",
    "ret",
);

const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const MEM_WRITE_WATCH: u32 = 0x200000;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;
const WRITE_WATCH_FLAG_RESET: u32 = 1;
const PAGE_EXECUTE_READ: u32 = 0x20;
/// Watched chunks this close to the entry stack pointer stay writable.
const STACK_SPAN: u64 = 1 << 20;
const CHUNK: u64 = 0x10000;
const PAGE: u64 = 0x1000;
const PAGES_PER_CHUNK: usize = (CHUNK / PAGE) as usize;

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn GetCurrentThreadId() -> u32;
    fn FlushInstructionCache(process: *mut c_void, base: *const c_void, size: usize) -> i32;
    fn VirtualAlloc(address: *mut c_void, size: usize, kind: u32, protect: u32) -> *mut c_void;
    fn GetWriteWatch(
        flags: u32,
        base: *mut c_void,
        size: usize,
        addresses: *mut *mut c_void,
        count: *mut usize,
        granularity: *mut u32,
    ) -> u32;
    fn ResetWriteWatch(base: *mut c_void, size: usize) -> u32;
    fn VirtualProtect(address: *mut c_void, size: usize, protect: u32, old: *mut u32) -> u32;
    fn AddVectoredExceptionHandler(
        first: u32,
        handler: unsafe extern "system" fn(*mut [*mut u8; 2]) -> i32,
    ) -> *mut c_void;
}

/// A fault while MAIN runs: record it and resume in the restore path with the host stack.
unsafe extern "system" fn on_fault(info: *mut [*mut u8; 2]) -> i32 {
    // Vectored handlers run process-wide; a foreign thread's exception must
    // never resume on this evaluator's saved host stack.
    if GetCurrentThreadId() != OWNER_THREAD {
        return 0;
    }
    if promote((*info)[0]) {
        return -1;
    }
    if NIOH3_NATIVE_TRACE != 0 && *((*info)[0] as *const u32) == 0x8000_0004 {
        let context = (*info)[1];
        let rip = *(context.add(0xF8) as *const u64);
        let flags = context.add(0x44) as *mut u32;
        if (TRACE_LO..TRACE_HI).contains(&rip) {
            if TRACE_LEN < TRACE_CAP {
                *TRACE_BUF.add(TRACE_LEN) = rip;
                TRACE_LEN += 1;
            }
            *flags |= 0x100;
        } else {
            *flags &= !0x100;
        }
        return -1;
    }
    if NIOH3_NATIVE_ACTIVE == 0 {
        return 0;
    }
    let record = (*info)[0];
    let context = (*info)[1];
    let ctx = &mut *NIOH3_NATIVE_CTX;
    ctx.faulted = 1;
    ctx.fault_code = *(record as *const u32) as u64;
    ctx.fault_rip = *(context.add(0xF8) as *const u64);
    ctx.fault_addr = if *(record.add(0x18) as *const u32) >= 2 {
        *(record.add(0x28) as *const u64)
    } else {
        0
    };
    *(context.add(0x98) as *mut u64) = ctx.host_rsp;
    *(context.add(0xF8) as *mut u64) = nioh3_native_restore as *const () as u64;
    -1
}

/// The row shortcut is a continuation inside MAIN, not a function replacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowLookupMode {
    MainContinuation,
    OriginalFunction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LookupCodeState {
    Ready(RowLookupMode),
    Unsynchronized(RowLookupMode),
}

struct RowLookupCode {
    address: u64,
    original: Vec<u8>,
    main_continuation: Vec<u8>,
    state: LookupCodeState,
}

impl RowLookupCode {
    fn mode(&self) -> RowLookupMode {
        match self.state {
            LookupCodeState::Ready(mode) | LookupCodeState::Unsynchronized(mode) => mode,
        }
    }

    fn select(&mut self, mode: RowLookupMode) -> Result<(), String> {
        let bytes = match mode {
            RowLookupMode::MainContinuation => &self.main_continuation,
            RowLookupMode::OriginalFunction => &self.original,
        };
        self.state = LookupCodeState::Unsynchronized(mode);
        // SAFETY: Native::open committed this code range; this process owns one Native.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.address as *mut u8, bytes.len())
        };
        // SAFETY: current-process code mapping above; no other Native executes concurrently.
        if unsafe {
            FlushInstructionCache(
                GetCurrentProcess(),
                self.address as *const c_void,
                bytes.len(),
            )
        } == 0
        {
            return Err(format!(
                "Native code synchronization failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        self.state = LookupCodeState::Ready(mode);
        Ok(())
    }
}
pub struct Native {
    ctx: Box<Ctx>,
    module: u64,
    score: u64,
    /// Candidate domain: (start, end) address ranges a write may touch.
    domain: Vec<(u64, u64)>,
    /// Stack chunks, write-watched and queried at each reset (start, length), and the template copy of every chunk.
    watched: Vec<(u64, u64)>,
    template: std::collections::HashMap<u64, Box<[u8]>>,
    dirty: Vec<*mut c_void>,
    /// Guarded pages written at least once, with their template bytes; every reset copies them all back.
    written: Vec<(u64, *const u8)>,
    /// Chunks holding only the game's sections, never written.
    code: Vec<u64>,
    row_lookup: RowLookupCode,
    knowledge: SnapshotKnowledge,
    reset_error: Option<String>,
    owner_thread: u32,
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
    /// Profiling: pages restored and write-watch queries so far.
    pub restored: u64,
    pub queries: u64,
}

fn hex_bytes(s: &str) -> Result<Vec<u8>, String> {
    if !s.is_ascii() || !s.len().is_multiple_of(2) {
        return Err("invalid hex bytes".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| "invalid hex bytes".into()))
        .collect()
}

static OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Native {
    /// Map an export (native_export.py: scene.txt, memory.bin) into this process. Only one per process.
    pub fn open(dir: &Path) -> Result<Native, String> {
        let scene = std::fs::read_to_string(dir.join("scene.txt")).map_err(|e| e.to_string())?;
        let (mut module, mut entry, mut main, mut rsp) = (0u64, 0u64, 0u64, 0u64);
        let mut sections = Vec::new();
        let mut domain = Vec::new();
        let mut gp = [0u64; 16];
        let mut xmm = [[0u8; 16]; 16];
        const GP: [&str; 16] = [
            "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15",
        ];
        for line in scene.lines() {
            let w: Vec<&str> = line.trim().splitn(3, ' ').collect();
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let needed = if matches!(w[0], "section" | "domain" | "reg" | "xmm") {
                3
            } else {
                2
            };
            if w.len() < needed {
                return Err(format!("incomplete scene directive: {}", w[0]));
            }
            let h = |s: &str| u64::from_str_radix(s, 16).map_err(|e| e.to_string());
            match w[0] {
                "module" => module = h(w[1])?,
                "entry" => entry = h(w[1])?,
                "main" => main = h(w[1])?,
                "rsp" => rsp = h(w[1])?,
                "section" => sections.push((h(w[1])?, w[2].to_string())),
                "domain" => domain.push((h(w[1])?, h(w[2])?)),
                "reg" => {
                    let b = hex_bytes(w[2])?;
                    let i = GP
                        .iter()
                        .position(|n| *n == w[1])
                        .ok_or("unknown register")?;
                    gp[i] = u64::from_le_bytes(
                        b.as_slice()
                            .try_into()
                            .map_err(|_| "register must contain eight bytes")?,
                    );
                }
                "xmm" => {
                    let i: usize = w[1].parse().map_err(|_| "bad xmm")?;
                    let b = hex_bytes(w[2])?;
                    if i >= 16 || b.len() != 16 {
                        return Err("invalid XMM register".into());
                    }
                    xmm[i].copy_from_slice(&b);
                }
                _ => {}
            }
        }
        let raw = std::fs::read(dir.join("memory.bin")).map_err(|e| e.to_string())?;
        let mut regions: Vec<(u64, Vec<u8>, Vec<u8>)> = Vec::new();
        let mut o = 0;
        while o < raw.len() {
            if raw.len() - o < 13 {
                return Err("truncated memory header".into());
            }
            let start = u64::from_le_bytes(raw[o..o + 8].try_into().unwrap());
            let n = u32::from_le_bytes(raw[o + 8..o + 12].try_into().unwrap()) as usize;
            o += 13;
            if n > (raw.len() - o) / 2 || start.checked_add(n as u64).is_none() {
                return Err("truncated or overflowing memory region".into());
            }
            regions.push((
                start,
                raw[o..o + n].to_vec(),
                raw[o + n..o + 2 * n].to_vec(),
            ));
            o += 2 * n;
        }
        let sections: Vec<(u64, Vec<u8>)> = sections
            .iter()
            .map(|(rva, f)| {
                let address = module.checked_add(*rva).ok_or("section address overflow")?;
                let path = if Path::new(f).is_absolute() {
                    Path::new(f).to_path_buf()
                } else {
                    dir.join(f)
                };
                std::fs::read(path)
                    .and_then(|d| {
                        if address.checked_add(d.len() as u64).is_some() {
                            Ok((address, d))
                        } else {
                            Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "section extent overflow",
                            ))
                        }
                    })
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<_, _>>()?;
        let chunks_of = |start: u64, len: u64| {
            let mut out = Vec::new();
            if len == 0 {
                return out;
            }
            let Some(last) = start.checked_add(len - 1) else {
                return out;
            };
            let last = last & !(CHUNK - 1);
            let mut c = start & !(CHUNK - 1);
            loop {
                out.push(c);
                if c == last {
                    break;
                }
                c += CHUNK;
            }
            out
        };
        let mut watched: Vec<u64> = regions
            .iter()
            .flat_map(|r| chunks_of(r.0, r.1.len() as u64))
            .collect();
        watched.sort_unstable();
        watched.dedup();
        let mut code: Vec<u64> = sections
            .iter()
            .flat_map(|s| chunks_of(s.0, s.1.len() as u64))
            .filter(|c| watched.binary_search(c).is_err())
            .collect();
        code.sort_unstable();
        code.dedup();
        if code.first() == Some(&0) || watched.first() == Some(&0) {
            return Err("a native scene cannot map the null chunk".into());
        }
        let mapped = |start: u64, len: u64| -> bool {
            start.checked_add(len).is_some()
                && chunks_of(start, len)
                    .iter()
                    .all(|c| code.binary_search(c).is_ok() || watched.binary_search(c).is_ok())
        };
        for (rva, len) in [(0x62451E, 8), (0x9F7CA2, 30)] {
            if !module.checked_add(rva).is_some_and(|a| mapped(a, len)) {
                return Err("adapter location is outside mapped sections".into());
            }
        }
        if !module.checked_add(entry).is_some_and(|a| mapped(a, 1))
            || !main.checked_add(0x218).is_some_and(|a| mapped(a, 4))
            || !mapped(rsp.saturating_sub(8), 0x30)
            || domain.iter().any(|&(s, e)| e <= s || !mapped(s, e - s))
        {
            return Err("invalid native entry, stack, score or candidate-domain mapping".into());
        }
        if OPENED.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err("a native image is already mapped or its initialization failed; use a fresh process".into());
        }
        let owner_thread = unsafe { GetCurrentThreadId() };
        let mut knowledge = SnapshotKnowledge::default();
        for (c, watch) in code
            .iter()
            .map(|&c| (c, false))
            .chain(watched.iter().map(|&c| (c, true)))
        {
            let kind = MEM_RESERVE | MEM_COMMIT | if watch { MEM_WRITE_WATCH } else { 0 };
            // SAFETY: a fixed-address allocation; a taken range returns null and is refused.
            let p = unsafe {
                VirtualAlloc(
                    c as *mut c_void,
                    CHUNK as usize,
                    kind,
                    PAGE_EXECUTE_READWRITE,
                )
            };
            if p.is_null() || p as u64 != c {
                return Err(format!("address {c:#x} is taken in this process"));
            }
        }
        for (a, d) in &sections {
            knowledge.declare_bytes(*a, d.len());
            // SAFETY: the range was committed above.
            unsafe { std::ptr::copy_nonoverlapping(d.as_ptr(), *a as *mut u8, d.len()) };
        }
        let in_section = |a: u64| {
            sections
                .iter()
                .any(|(s, d)| *s <= a && a < s + d.len() as u64)
        };
        for (start, data, known) in &regions {
            for (i, (&v, &k)) in data.iter().zip(known).enumerate() {
                let a = start + i as u64;
                if k != 0 || !in_section(a) {
                    // SAFETY: inside a committed chunk.
                    unsafe { *(a as *mut u8) = v & k };
                    // A partial bit mask cannot certify a complete byte.
                    knowledge.set_byte(a, k == 0xFF);
                }
            }
        }
        // Pin the PC202 originals before any patch. A readable allocation or
        // zero padding is not permission to call an unknown original function.
        for (rva, signature) in [
            (0x624518u64, "40534883ec2065488b042558000000488bd9b918000000488b108b04113905652158040f8f23842f01488b83c00700004885c07507488d055c2158044883c4205bc3"),
            (0x9f7c78u64, "48894c24084883ec28488b054898d5034c8bcac74424300000803fc74424340000803f488b48088a5110e86ddd68ff33d24885c0741e413851010f85e2300601413851050f85e2300601413851030f85f0300601f30f10442434f30f594424304883c428c3"),
            (0x85a14u64, "488b0515445304440fb6c2488b4810488b11443b4204730f4969c8dc000000488d42084803c1c333c0c3"),
        ] {
            let expected = hex_bytes(signature)?;
            knowledge.require_range(module + rva, expected.len())
                .map_err(|address| format!("original code byte is unknown at {address:#x}"))?;
            // SAFETY: known bytes originate from the mapped sections/regions.
            let actual = unsafe { std::slice::from_raw_parts((module + rva) as *const u8, expected.len()) };
            if actual != expected { return Err(format!("PC202 original code signature differs at {rva:#x}")); }
        }
        let patch = |rva: u64, bytes: &[u8]| unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), (module + rva) as *mut u8, bytes.len())
        };
        let rel = |from_end: u64, to: u64| ((to as i64 - from_end as i64) as i32).to_le_bytes();
        // D getter cut: at 62451E RCX is the STAT; the VM resumes at 624541 with RBX = STAT.
        let mut getter = vec![0x48, 0x89, 0xCB, 0xE9];
        getter.extend(rel(0x62451E + 8, 0x624541));
        patch(0x62451E, &getter);
        // Caller-local cut at9F7CA2. GLOBAL85A14 always remains callable.
        // These checks replace the wrapper's CALL and resume its own stack frame;
        // there is no synthetic POP and no jump into an unrelated caller.
        let cut_rva = 0x9F7CA2;
        let mut row: Vec<u8> = vec![
            0x41, 0x80, 0x79, 0x01, 0x00, 0x75, 0x00, 0x41, 0x80, 0x79, 0x03, 0x00, 0x75, 0x00,
            0x41, 0x80, 0x79, 0x05, 0x00, 0x75, 0x00, 0x31, 0xD2, 0xE9, 0, 0, 0, 0, 0x0F, 0x0B,
        ];
        let fail_at = row.len() - 2;
        for jne in [5usize, 12, 19] {
            row[jne + 1] = (fail_at - (jne + 2)) as u8;
        }
        row[fail_at - 4..fail_at].copy_from_slice(&rel(cut_rva + fail_at as u64, 0x9F7CCC));
        // Retain both implementations. A helper call cannot use MAIN's continuation.
        let row_lookup = RowLookupCode {
            address: module + cut_rva,
            // SAFETY: original section bytes have been mapped and overlaid above.
            original: unsafe {
                std::slice::from_raw_parts((module + cut_rva) as *const u8, row.len())
            }
            .to_vec(),
            main_continuation: row.clone(),
            state: LookupCodeState::Ready(RowLookupMode::MainContinuation),
        };
        patch(cut_rva, &row);
        knowledge.declare_bytes(module + cut_rva, row.len());
        knowledge.declare_bytes(module + 0x62451E, getter.len());
        // SAFETY: the complete current-process executable image and both adapters are now installed.
        if unsafe { FlushInstructionCache(GetCurrentProcess(), std::ptr::null(), 0) } == 0 {
            return Err(format!(
                "Native image synchronization failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        // Template copies of watched chunks, after every write above, then a clean watch.
        let mut template = std::collections::HashMap::new();
        for &c in &watched {
            let mut copy = vec![0u8; CHUNK as usize].into_boxed_slice();
            // SAFETY: committed chunk.
            unsafe {
                std::ptr::copy_nonoverlapping(c as *const u8, copy.as_mut_ptr(), CHUNK as usize)
            };
            template.insert(c, copy);
        }
        // Every chunk off the stack is guarded: its pages start read-only, a page's first write faults once and the
        // handler makes it writable, and from then on every reset copies that page back. Pages never written cost
        // nothing and need no query. The stack stays writable, since a fault is dispatched on it, and is write-watched.
        let near_stack = |c: u64| c + STACK_SPAN > rsp && c < rsp + STACK_SPAN;
        let cold: &'static mut [u64] = Box::leak(watched.clone().into_boxed_slice());
        let state: &'static mut [u8] =
            Box::leak(vec![0u8; cold.len() * PAGES_PER_CHUNK].into_boxed_slice());
        for (i, &c) in cold.iter().enumerate() {
            // SAFETY: committed chunks of this process.
            if unsafe { ResetWriteWatch(c as *mut c_void, CHUNK as usize) } != 0 {
                return Err(format!("initial write-watch reset failed at {c:#x}"));
            }
            if near_stack(c) {
                state[i * PAGES_PER_CHUNK..(i + 1) * PAGES_PER_CHUNK].fill(1);
                continue;
            }
            let mut old = 0u32;
            // SAFETY: as above.
            if unsafe {
                VirtualProtect(
                    c as *mut c_void,
                    CHUNK as usize,
                    PAGE_EXECUTE_READ,
                    &mut old,
                )
            } == 0
            {
                return Err(format!("chunk {c:#x} could not be protected"));
            }
        }
        let promoted: &'static mut [u64] =
            Box::leak(vec![0u64; cold.len() * PAGES_PER_CHUNK].into_boxed_slice());
        // SAFETY: set once, before the handler is registered; OPENED keeps it to one Native per process.
        unsafe {
            OWNER_THREAD = owner_thread;
            COLD = cold.as_ptr();
            COLD_LEN = cold.len();
            COLD_STATE = state.as_mut_ptr();
            PROMOTED = promoted.as_mut_ptr();
            PROMOTED_LEN = 0;
        }
        // SAFETY: registering a handler that only acts while MAIN runs.
        if unsafe { AddVectoredExceptionHandler(1, on_fault) }.is_null() {
            return Err("native exception handler registration failed; use a fresh process".into());
        }
        let mut ctx = Box::new(Ctx {
            gp,
            xmm,
            entry: module + entry,
            host_rsp: 0,
            out_rax: 0,
            faulted: 0,
            fault_code: 0,
            fault_rip: 0,
            fault_addr: 0,
            out_xmm0: [0; 16],
        });
        ctx.gp[2] = main;
        ctx.gp[7] = rsp;
        let mut native = Native {
            ctx,
            module,
            score: main + 0x218,
            domain,
            watched: watched
                .iter()
                .filter(|&&c| near_stack(c))
                .map(|&c| (c, CHUNK))
                .collect(),
            template,
            dirty: vec![std::ptr::null_mut(); 4096],
            written: Vec::new(),
            code,
            row_lookup,
            knowledge,
            reset_error: None,
            owner_thread,
            _thread_bound: std::marker::PhantomData,
            restored: 0,
            queries: 0,
        };
        native.try_reset()?;
        Ok(native)
    }

    /// Put every page written since the last reset back to the template.
    pub fn reset(&mut self) {
        if let Err(error) = self.try_reset() {
            self.reset_error = Some(error);
        }
    }

    /// Fallible reset; a failure poisons execution even for legacy void-reset callers.
    pub fn try_reset(&mut self) -> Result<(), String> {
        if let Some(error) = &self.reset_error {
            return Err(error.clone());
        }
        if unsafe { GetCurrentThreadId() } != self.owner_thread {
            return Err("native reset attempted on a different thread".into());
        }
        let mode = self.row_lookup.mode();
        let restored_before = self.restored;
        // Reset can restore a watched executable page to MAIN's template.
        self.row_lookup.state = LookupCodeState::Unsynchronized(mode);
        // SAFETY: the handler only appends; it runs on this thread, inside a write.
        let promoted = unsafe { std::slice::from_raw_parts(PROMOTED, PROMOTED_LEN) };
        for &page in &promoted[self.written.len()..] {
            let chunk = page & !(CHUNK - 1);
            if let Some(t) = self.template.get(&chunk) {
                self.written
                    .push((page, t[(page - chunk) as usize..].as_ptr()));
            }
        }
        for &(page, from) in &self.written {
            // SAFETY: a writable guarded page and its template copy.
            unsafe { std::ptr::copy_nonoverlapping(from, page as *mut u8, PAGE as usize) };
        }
        self.restored += self.written.len() as u64;
        for i in 0..self.watched.len() {
            let (start, len) = self.watched[i];
            let mut count = self.dirty.len();
            let mut granularity = 0u32;
            // SAFETY: a watched range of this process; the buffer holds `count` entries.
            let failed = unsafe {
                GetWriteWatch(
                    WRITE_WATCH_FLAG_RESET,
                    start as *mut c_void,
                    len as usize,
                    self.dirty.as_mut_ptr(),
                    &mut count,
                    &mut granularity,
                )
            };
            self.queries += 1;
            if failed != 0 || granularity as u64 != PAGE || count > self.dirty.len() {
                let error = format!("native write-watch query failed at {start:#x}: status={failed}, granularity={granularity}, count={count}");
                self.reset_error = Some(error.clone());
                return Err(error);
            }
            self.restored += count as u64;
            for &page in &self.dirty[..count] {
                let page = page as u64;
                let chunk = page & !(CHUNK - 1);
                if let Some(t) = self.template.get(&chunk) {
                    let at = (page - chunk) as usize;
                    // SAFETY: a committed page and its template copy.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            t[at..].as_ptr(),
                            page as *mut u8,
                            PAGE as usize,
                        )
                    };
                }
            }
            if count > 0 {
                // The copies are writes too; clear their marks.
                // SAFETY: a watched allocation of this process.
                if unsafe { ResetWriteWatch(start as *mut c_void, len as usize) } != 0 {
                    let error = format!("native write-watch reset failed at {start:#x}");
                    self.reset_error = Some(error.clone());
                    return Err(error);
                }
            }
        }
        let result = self.row_lookup.select(mode);
        if let Err(error) = &result {
            self.reset_error = Some(error.clone());
        }
        result?;
        // Watched allocations are executable. A helper can dirty code outside
        // the wrapper cut; restoring those pages must synchronize that code too.
        if self.restored != restored_before
            && unsafe { FlushInstructionCache(GetCurrentProcess(), std::ptr::null(), 0) } == 0
        {
            let error = format!(
                "native reset code synchronization failed: {}",
                std::io::Error::last_os_error()
            );
            self.row_lookup.state = LookupCodeState::Unsynchronized(mode);
            self.reset_error = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }

    /// Write the bytes of `words` inside the candidate domain and return the byte count.
    pub fn write_words(&mut self, words: &[(u64, u32)]) -> usize {
        let mut written = 0;
        for &(address, value) in words {
            let Some(&(s, e)) = self
                .domain
                .iter()
                .find(|&&(s, e)| s < address + 4 && address < e)
            else {
                continue;
            };
            for (k, b) in value.to_le_bytes().into_iter().enumerate() {
                let a = address + k as u64;
                if s <= a && a < e || self.domain.iter().any(|&(s, e)| s <= a && a < e) {
                    // SAFETY: domain bytes lie in mapped regions.
                    unsafe { *(a as *mut u8) = b };
                    written += 1;
                }
            }
        }
        written
    }

    /// Write one byte at an offset of the concatenated domain, as the research vectors number it.
    #[doc(hidden)]
    pub fn write_domain_byte(&mut self, offset: u64, byte: u8) {
        let mut at = 0;
        for &(s, e) in &self.domain {
            if offset < at + (e - s) {
                // SAFETY: domain bytes lie in mapped regions.
                unsafe { *((s + offset - at) as *mut u8) = byte };
                return;
            }
            at += e - s;
        }
    }

    /// Write the memory MAIN starts from, patches included, as records of (address u64, 64 KiB) per chunk: the
    /// image a translation of MAIN reads.
    #[doc(hidden)]
    pub fn dump_image(&self, path: &Path) -> std::io::Result<()> {
        let mut chunks: Vec<u64> = self
            .code
            .iter()
            .copied()
            .chain(self.template.keys().copied())
            .collect();
        chunks.sort_unstable();
        let mut out = Vec::with_capacity(chunks.len() * (8 + CHUNK as usize));
        for &c in &chunks {
            out.extend_from_slice(&c.to_le_bytes());
            match self.template.get(&c) {
                Some(t) => out.extend_from_slice(t),
                // SAFETY: a committed code chunk, never written after open.
                None => out.extend_from_slice(unsafe {
                    std::slice::from_raw_parts(c as *const u8, CHUNK as usize)
                }),
            }
        }
        std::fs::write(path, out)?;
        // Exact-byte knownness accompanies every newly exported image. Padding
        // and absent TLS state remain unknown despite readable native pages.
        std::fs::write(
            path.with_file_name("image_known.bin"),
            self.knowledge.encode(&chunks),
        )
    }

    pub fn oracle_environment(&self) -> OracleEnvironment {
        OracleEnvironment {
            tls_guard_captured: self
                .knowledge
                .require_range(self.module + 0x4BA66A0, 4)
                .is_ok(),
            tls_epoch_captured: false, // The scene format carries no captured TEB/TLS binding.
            native_reads_checked: false, // Hardware page validity is not per-byte provenance.
            source_admission_derived: false, // The runtime does not certify its input producer.
        }
    }

    /// The captured module base the game code is mapped at.
    pub fn module(&self) -> u64 {
        self.module
    }

    /// Run MAIN single-stepped and return its score bits with the address of every instruction it executed in the
    /// game's code, in order (at most `cap`).
    #[doc(hidden)]
    pub fn run_traced(&mut self, cap: usize) -> Result<(u32, Vec<u64>), String> {
        let mut buf = vec![0u64; cap];
        // SAFETY: the handler writes inside `buf` only while tracing, on this thread.
        unsafe {
            TRACE_LO = self.module;
            TRACE_HI = self.module + 0x1000_0000;
            TRACE_BUF = buf.as_mut_ptr();
            TRACE_CAP = cap;
            TRACE_LEN = 0;
            NIOH3_NATIVE_TRACE = 1;
        }
        let r = self.run();
        // SAFETY: as above.
        let n = unsafe {
            NIOH3_NATIVE_TRACE = 0;
            TRACE_BUF = std::ptr::null_mut();
            TRACE_CAP = 0;
            TRACE_LEN
        };
        buf.truncate(n);
        r.map(|bits| (bits, buf))
    }

    /// Execute a control helper with the original row lookup and the fault-contained trampoline.
    /// The captured nonnull-D/TLS getter binding remains installed; this is not a raw-game loader.
    /// This research entry point restores the MAIN register/entry configuration even on failure.
    ///
    /// # Safety
    /// The caller must establish the original function ABI, valid argument lifetimes,
    /// and that the helper cannot mutate host-owned memory unexpectedly. Missing captured
    /// memory is reported through the native fault handler. Arguments are integer/pointer registers.
    #[doc(hidden)]
    pub unsafe fn call_original(&mut self, rva: u64, args: [u64; 4]) -> Result<u64, String> {
        let entry = self.ctx.entry;
        let gp = self.ctx.gp;
        let row_mode = self.row_lookup.mode();
        if let Err(error) = self.row_lookup.select(RowLookupMode::OriginalFunction) {
            let recovery = self.row_lookup.select(row_mode);
            return Err(format!("{error}; code recovery: {recovery:?}"));
        }
        self.ctx.entry = self.module + rva;
        for (index, value) in [2, 3, 8, 9].into_iter().zip(args) {
            self.ctx.gp[index] = value;
        }
        let outcome = self.execute().map(|_| self.ctx.out_rax);
        let restored = self.row_lookup.select(row_mode);
        self.ctx.entry = entry;
        self.ctx.gp = gp;
        restored?;
        outcome
    }
    /// Capture an original helper's scalar binary32 return in XMM0.
    ///
    /// # Safety
    /// The caller must establish the ABI and all `call_original` requirements.
    #[doc(hidden)]
    pub unsafe fn call_original_f32(&mut self, rva: u64, args: [u64; 4]) -> Result<u32, String> {
        // SAFETY: forwarded from the caller; the trampoline owns the return snapshot.
        unsafe { self.call_original(rva, args) }?;
        Ok(u32::from_le_bytes(
            self.ctx.out_xmm0[..4].try_into().unwrap(),
        ))
    }
    /// Execute an original helper with explicit Windows x64 stack arguments.
    /// The temporary stack is owned by this call; MAIN's captured stack is restored.
    ///
    /// # Safety
    /// The caller must establish the original ABI and the same argument/mutation
    /// requirements as `call_original`. The helper must fit its stack use in 60 KiB.
    #[doc(hidden)]
    pub unsafe fn call_original_with_stack(
        &mut self,
        rva: u64,
        args: [u64; 4],
        stack_args: &[u64],
    ) -> Result<u64, String> {
        if stack_args.len() > 16 {
            return Err("Too many native helper stack arguments".into());
        }
        let mut stack = vec![0u128; 4096];
        let rsp = stack.as_mut_ptr() as u64 + 0xff08;
        for (index, value) in stack_args.iter().enumerate() {
            // SAFETY: this address lies in the owned, 16-byte-aligned stack allocation.
            unsafe { ((rsp + 0x28 + index as u64 * 8) as *mut u64).write(*value) };
        }
        let captured_rsp = self.ctx.gp[7];
        self.ctx.gp[7] = rsp;
        // SAFETY: supplied by the caller; the temporary stack remains alive through return/fault.
        let result = unsafe { self.call_original(rva, args) };
        self.ctx.gp[7] = captured_rsp;
        result
    }
    /// Run MAIN with the original row lookup, retaining only the declared TLS getter binding.
    /// Static row resources must have been bound to the captured manager by the research caller.
    #[doc(hidden)]
    pub fn run_with_original_row_lookup(&mut self) -> Result<u32, String> {
        let mode = self.row_lookup.mode();
        if let Err(error) = self.row_lookup.select(RowLookupMode::OriginalFunction) {
            let recovery = self.row_lookup.select(mode);
            return Err(format!("{error}; code recovery: {recovery:?}"));
        }
        let result = self.run();
        let restored = self.row_lookup.select(mode);
        restored?;
        result
    }
    /// Run the original instructions and read their independent final channels.
    pub fn run_components(&mut self) -> Result<crate::damage::DamageWords, String> {
        let start = self
            .score
            .checked_add(0xC)
            .ok_or("component address overflow")?;
        let end = start.checked_add(7).ok_or("component address overflow")?;
        if [start & !0xFFFFu64, end & !0xFFFFu64]
            .iter()
            .any(|chunk| !self.code.contains(chunk) && !self.template.contains_key(chunk))
        {
            return Err("native component outputs are outside the mapped image".into());
        }
        self.execute()?;
        // SAFETY: score was validated by open; both component words were checked
        // against owned mapped chunks above. The executed game writes them.
        Ok(unsafe {
            crate::damage::DamageWords {
                total: *(self.score as *const u32),
                physical: *(start as *const u32),
                elemental: *((start + 4) as *const u32),
                reserved: 0,
            }
        })
    }

    /// Run MAIN on the current memory and return its score bits at MAIN+0x218.
    pub fn run(&mut self) -> Result<u32, String> {
        self.execute()?;
        // SAFETY: the score word lies in the validated mapped scene domain.
        Ok(unsafe { *(self.score as *const u32) })
    }

    fn execute(&mut self) -> Result<(), String> {
        if unsafe { GetCurrentThreadId() } != self.owner_thread {
            return Err("native execution attempted on a different thread".into());
        }
        if let Some(error) = &self.reset_error {
            return Err(error.clone());
        }
        if matches!(self.row_lookup.state, LookupCodeState::Unsynchronized(_)) {
            return Err("Native code is not synchronized after an adapter write failure".into());
        }
        self.ctx.faulted = 0;
        // SAFETY: the trampoline saves and restores every host register; faults return through the handler.
        unsafe { nioh3_native_call(&mut *self.ctx) };
        if self.ctx.faulted != 0 {
            return Err(format!(
                "fault {:#x} at rva {:#x} reading {:#x}",
                self.ctx.fault_code,
                self.ctx.fault_rip.wrapping_sub(self.module),
                self.ctx.fault_addr
            ));
        }
        Ok(())
    }
}
