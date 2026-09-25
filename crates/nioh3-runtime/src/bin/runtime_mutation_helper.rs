//! Owned helper process for the real Windows mutation-API probe.
//!
//! Built only with the `test-helper` feature. It allocates one executable page
//! of its own, writes a five-byte site with the auxiliary hook signature, and
//! then serves line commands so a test can install a real hook into it, change
//! its bytes behind the hook owner's back, and let it exit. It never touches
//! another process, a game, or a save.
//!
//! `layout` additionally commits a read/write region shaped like the PC v2.01
//! scroll inventory (signature site, manager and data pointers, counters and the
//! fixed 400 x 0xE8 container) so the read-only inventory capture and the count
//! adapter can be proven through real `ReadProcessMemory`/`WriteProcessMemory`
//! calls instead of an injected adapter.

use std::io::{BufRead, Write};

/// The auxiliary hook signature the page starts with.
const SITE_BYTES: [u8; 11] = [
    0x48, 0x89, 0x5C, 0x24, 0x08, // mov [rsp+8], rbx (the shipped signature)
    0xB8, 0x03, 0x00, 0x00, 0x00, // mov eax, 3
    0xC3, // ret
];

#[cfg(windows)]
fn allocate_page(module_base: u64) -> Result<u64, String> {
    use windows_sys::Win32::System::Memory::{
        VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
    };
    // The page must sit near the image, exactly like the trampoline the real
    // hook needs: a rel32 jump and `module_base + rva` both have to resolve.
    for offset in [0x1000_0000u64, 0x2000_0000, 0x4000_0000, 0x6000_0000] {
        if module_base == 0 {
            break;
        }
        let requested = (module_base + offset) as *const std::ffi::c_void;
        let page = unsafe {
            VirtualAlloc(
                requested,
                0x1000,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        };
        if !page.is_null() && page as u64 == requested as u64 {
            return Ok(page as u64);
        }
        if !page.is_null() {
            // Somebody else took the address; keep looking rather than leaking.
            let _ = unsafe {
                windows_sys::Win32::System::Memory::VirtualFree(
                    page,
                    0,
                    windows_sys::Win32::System::Memory::MEM_RELEASE,
                )
            };
        }
    }
    Err("VirtualAlloc could not place the helper page near its image".to_string())
}

#[cfg(not(windows))]
fn allocate_page(_module_base: u64) -> Result<u64, String> {
    Err("the helper is Windows-only".to_string())
}

fn write_page(page: u64, bytes: &[u8]) {
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), page as *mut u8, bytes.len());
    }
}

/// Commit the inventory region. Returns `(region, container)`.
///
/// The layout mirrors `live_fakes::InventoryFixture`: signature at `+0x1000`,
/// manager pointer at `+0x2000` to `+0x10000`, data and counters at `+0x20000`,
/// the container at `data + 0x1000`, its capacity cell at `container + 0x16A80`,
/// records in slots 4 and 100, acquisition order 11 and serial counter 0x3345.
#[cfg(windows)]
fn allocate_layout() -> Result<(u64, u64), String> {
    use windows_sys::Win32::System::Memory::{
        VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
    };
    let size = 0x8_0000usize;
    let region = unsafe {
        VirtualAlloc(
            std::ptr::null(),
            size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    if region.is_null() {
        return Err("VirtualAlloc could not commit the inventory region".to_string());
    }
    let base = region as u64;
    let manager = base + 0x1_0000;
    let data = base + 0x2_0000;
    let container = data + 0x1000;
    // The shipped insertion signature guards the whole capture.
    write_page(
        base + 0x1000,
        &bytes_from_hex("40555356574154415541564157488DAC"),
    );
    write_page(base + 0x2000, &manager.to_le_bytes());
    write_page(manager, &data.to_le_bytes());
    write_page(data, &11u32.to_le_bytes());
    write_page(data + 8, &0x3345u64.to_le_bytes());
    write_page(container + 0x16A80, &400u64.to_le_bytes());
    for (slot, serial, seed) in [(4usize, 0x1234u64, 0xF00Du32), (100, 0x5678, 0x2222)] {
        let mut record = [0u8; 0xE8];
        record[0] = 0x82;
        record[1] = 0x1E;
        record[0x18] = 0x02;
        record[0x1A] = 0x80;
        record[0x1C..0x20].copy_from_slice(&11u32.to_le_bytes());
        record[0x20..0x24].copy_from_slice(&seed.to_le_bytes());
        record[0x28..0x30].copy_from_slice(&serial.to_le_bytes());
        record[0x30] = 4;
        record[0x33] = 2;
        write_page(container + (slot * 0xE8) as u64, &record);
    }
    Ok((base, container))
}

#[cfg(not(windows))]
fn allocate_layout() -> Result<(u64, u64), String> {
    Err("the helper is Windows-only".to_string())
}

fn bytes_from_hex(text: &str) -> Vec<u8> {
    parse_hex(text).unwrap_or_default()
}

/// The batch-oracle stand-in: `rcx` is the destination record, `rdx` the source
/// record. It copies the source to the destination, preserves the batch
/// wrapper's own cursors (`rbx`, `rsi`, `rdi`) and returns the destination
/// pointer, which is the calling convention the shipped wrapper uses
/// (`mov rcx,rsi` / `mov rdx,rbx` / `call r12` / `add rbx,0xE8`).
///
/// `53 56 57`        push rbx; push rsi; push rdi
/// `48 89 C8`        mov rax, rcx
/// `48 89 D6`        mov rsi, rdx
/// `48 89 CF`        mov rdi, rcx
/// `B9 E8 00 00 00`  mov ecx, 0xE8
/// `F3 A4`           rep movsb
/// `5F 5E 5B`        pop rdi; pop rsi; pop rbx
/// `C3`              ret
const ORACLE_STUB: [u8; 23] = [
    0x53, 0x56, 0x57, 0x48, 0x89, 0xC8, 0x48, 0x89, 0xD6, 0x48, 0x89, 0xCF, 0xB9, 0xE8, 0x00, 0x00,
    0x00, 0xF3, 0xA4, 0x5F, 0x5E, 0x5B, 0xC3,
];

/// A deliberately long call, so a short oracle timeout is observable:
/// `53 56 57` push rbx; push rsi; push rdi
/// `B8 <imm32>` mov eax, imm
/// `FF C8` dec eax; `75 FB` jnz -5
/// `5F 5E 5B` pop rdi; pop rsi; pop rbx
/// `31 C0` xor eax, eax; `C3` ret
fn oracle_slow_stub() -> Vec<u8> {
    let mut code = vec![0x53u8, 0x56, 0x57, 0xB8];
    code.extend_from_slice(&0x0800_0000u32.to_le_bytes());
    code.extend_from_slice(&[0xFF, 0xC8, 0x75, 0xFB, 0x5F, 0x5E, 0x5B, 0x31, 0xC0, 0xC3]);
    code
}

/// The canonical record bytes the oracle stand-in copies: type `0x1E82`,
/// the seeded nonstackable flag, a serial and a rarity.
fn oracle_template() -> [u8; 0xE8] {
    let mut record = [0u8; 0xE8];
    record[0] = 0x82;
    record[1] = 0x1E;
    record[0x18] = 0x02;
    record[0x1A] = 0x80;
    record[0x20..0x24].copy_from_slice(&0x0BADF00Du32.to_le_bytes());
    record[0x28..0x30].copy_from_slice(&0x1122334455667788u64.to_le_bytes());
    record[0x30] = 4;
    record[0x31] = 4;
    record
}

/// `oracle` / `oracle-slow`: one owned function the batch oracle can call.
#[cfg(windows)]
fn allocate_oracle_page(module_base: u64, slow: bool) -> Result<(u64, u64), String> {
    let page = allocate_page(module_base)?;
    let code = if slow {
        oracle_slow_stub()
    } else {
        ORACLE_STUB.to_vec()
    };
    write_page(page, &code);
    let template = page + 0x100;
    write_page(template, &oracle_template());
    // The caller needs the exact bytes it must expect at the site.
    LAST_ORACLE_CODE.with(|cell| *cell.borrow_mut() = code);
    Ok((page, template))
}

thread_local! {
    static LAST_ORACLE_CODE: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

// ---------------------------------------------------------------------------
// Bounded live-add dispatch target for the disposable debug acceptance.
//
// This presents the *accepted PC v2.01 live-add layout* over the helper's own
// image so a real `NativeDebugTransport` debug session can run one preview
// dispatch against it: the executor arms Dr0 at the dispatch entry and Dr1 at
// its acknowledgement, the shim it allocates calls the builder below, and the
// worker thread executes the entry whenever a debugger is attached. It never
// touches Nioh 3, a game, or a save, and it is compiled only with `test-helper`.

/// RVAs taken verbatim from the accepted PC v2.01 live-add layout.
const LIVE_ADD_DISPATCH_RVA: u64 = 0x12E6840;
const LIVE_ADD_BUILDER_RVA: u64 = 0x227C4CC;
const LIVE_ADD_BUILDER_SIZE: usize = 0x27B;
const LIVE_ADD_MANAGER_RVA: u64 = 0x474D4E0;
const LIVE_ADD_INSERTION_RVA: u64 = 0x54D294;
const LIVE_ADD_CAPACITY_OFFSET: u64 = 0x16A80;
const LIVE_ADD_CAPACITY: u64 = 400;
/// The shipped insertion signature every product inventory capture verifies.
const LIVE_ADD_INSERTION_SIGNATURE: [u8; 16] = [
    0x40, 0x55, 0x53, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57, 0x48, 0x8D, 0xAC,
];
const LIVE_ADD_DATA_SIZE: usize = 0x24_0000;
const LIVE_ADD_TEMPLATE_OFFSET: u64 = 0x1000;
const LIVE_ADD_COUNTER_OFFSET: u64 = 0x2000;
const LIVE_ADD_CONTAINER_OFFSET: u64 = 0x224A60;
const LIVE_ADD_SERIAL_INDEX_OFFSET: u64 = 0x23B5E8;
const LIVE_ADD_RECORD_SIZE: usize = 0xE8;
/// `40 53 57 48 83 EC 38` - the executor verifies exactly these seven bytes.
const LIVE_ADD_PROLOGUE: [u8; 7] = [0x40, 0x53, 0x57, 0x48, 0x83, 0xEC, 0x38];
/// `48 83 C4 38 5F 5B C3` - add rsp,0x38; pop rdi; pop rbx; ret.
const LIVE_ADD_EPILOGUE: [u8; 7] = [0x48, 0x83, 0xC4, 0x38, 0x5F, 0x5B, 0xC3];

struct LiveAddRuntime {
    entry: u64,
    data: u64,
    counter: u64,
    builder_hex: String,
    creation: String,
}

static LIVE_ADD: std::sync::Mutex<Option<LiveAddRuntime>> = std::sync::Mutex::new(None);

/// Commit one 64 KiB region covering `address`, which need not be aligned.
#[cfg(windows)]
fn commit_fixed(address: u64) -> Result<(), String> {
    use windows_sys::Win32::System::Memory::{
        VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
    };
    let aligned = address & !0xFFFF;
    let page = unsafe {
        VirtualAlloc(
            aligned as *const std::ffi::c_void,
            0x1_0000,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        )
    };
    if page.is_null() || page as u64 != aligned {
        return Err(format!("VirtualAlloc could not commit {aligned:#x}"));
    }
    Ok(())
}

#[cfg(not(windows))]
fn commit_fixed(_address: u64) -> Result<(), String> {
    Err("the helper is Windows-only".to_string())
}

#[cfg(windows)]
fn commit_read_write(size: usize) -> Result<u64, String> {
    use windows_sys::Win32::System::Memory::{
        VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
    };
    let region = unsafe {
        VirtualAlloc(
            std::ptr::null(),
            size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    if region.is_null() {
        return Err("VirtualAlloc could not commit the live-add region".to_string());
    }
    Ok(region as u64)
}

#[cfg(not(windows))]
fn commit_read_write(_size: usize) -> Result<u64, String> {
    Err("the helper is Windows-only".to_string())
}

/// A builder stand-in with the game builder's calling convention: `rcx` is the
/// output record, `rdx` the descriptor. It copies the recorded template into the
/// output, increments the call counter, and returns the output pointer. The
/// `hang` variant stops after the copy so the shim can never reach its
/// acknowledgement - the bounded "no acknowledgement" negative.
fn live_add_builder_code(template: u64, counter: u64, hang: bool) -> Vec<u8> {
    let mut code: Vec<u8> = Vec::new();
    code.extend_from_slice(&[0x48, 0x89, 0xCA]); // mov rdx, rcx
    code.push(0x48);
    code.push(0xB8);
    code.extend_from_slice(&template.to_le_bytes()); // mov rax, template
    code.extend_from_slice(&[0x31, 0xC9]); // xor ecx, ecx
    let loop_at = code.len();
    code.extend_from_slice(&[0x44, 0x8A, 0x04, 0x08]); // mov r8b, [rax+rcx]
    code.extend_from_slice(&[0x44, 0x88, 0x04, 0x0A]); // mov [rdx+rcx], r8b
    code.extend_from_slice(&[0x48, 0xFF, 0xC1]); // inc rcx
    code.extend_from_slice(&[0x48, 0x81, 0xF9]);
    code.extend_from_slice(&(LIVE_ADD_RECORD_SIZE as u32).to_le_bytes()); // cmp rcx, 0xE8
    let jne_at = code.len();
    code.extend_from_slice(&[0x75, 0x00]); // jne loop
    let delta = loop_at as i64 - (jne_at as i64 + 2);
    code[jne_at + 1] = delta as i8 as u8;
    code.push(0x48);
    code.push(0xB8);
    code.extend_from_slice(&counter.to_le_bytes()); // mov rax, counter
    code.extend_from_slice(&[0xF0, 0x48, 0xFF, 0x00]); // lock inc qword [rax]
    if hang {
        code.extend_from_slice(&[0xEB, 0xFE]); // jmp $
    } else {
        code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx
        code.push(0xC3); // ret
    }
    code
}

/// Write `bytes` at `address` in this process.
fn write_bytes(address: u64, bytes: &[u8]) {
    write_page(address, bytes);
}

/// The smallest native serial-index shape `live_inventory.inspect` accepts: one
/// FNV bucket holding the whole doubly-linked list, mirroring
/// `live_fakes::InventoryFixture::seed_index` field for field, including the
/// header, bucket, sentinel and node offsets the product's own traversal reads.
fn seed_live_add_index(header: u64, records: &[(u64, u64)]) {
    let count = records.len() as u64;
    let nodes: Vec<u64> = (0..count)
        .map(|index| header + 0x1000 + index * 0x40)
        .collect();
    let sentinel = header + 0x100;
    write_page(header, &[0u8; 0x40]);
    write_page(header + 0x40, &[0u8; 16]);
    write_page(sentinel, &[0u8; 0x20]);
    for node in &nodes {
        write_page(*node, &[0u8; 0x20]);
    }
    write_u64_at(header + 8, sentinel);
    write_u64_at(header + 16, count);
    write_u64_at(header + 24, header + 0x40);
    write_u64_at(header + 0x30, 0);
    write_u64_at(header + 0x38, 1);
    let first = nodes.first().copied();
    let last = nodes.last().copied();
    let (Some(first), Some(last)) = (first, last) else {
        // The empty shape: the sentinel is its own predecessor and successor.
        write_u64_at(sentinel, sentinel);
        write_u64_at(sentinel + 8, sentinel);
        return;
    };
    write_u64_at(sentinel, first);
    write_u64_at(sentinel + 8, last);
    for (index, ((slot, serial), node)) in records.iter().zip(nodes.iter()).enumerate() {
        let next = nodes.get(index + 1).copied().unwrap_or(sentinel);
        let previous = if index == 0 {
            sentinel
        } else {
            nodes
                .get(index.wrapping_sub(1))
                .copied()
                .unwrap_or(sentinel)
        };
        write_u64_at(*node, next);
        write_u64_at(*node + 8, previous);
        write_u64_at(*node + 0x10, *serial);
        write_u32_at(*node + 0x18, *slot as u32);
    }
    // The bucket walks the backward (`previous`) chain, so `first` is the
    // chain's end in list order and `current` is its newest node.
    write_u64_at(header + 0x40, first);
    write_u64_at(header + 0x48, last);
}

fn read_u64_at(address: u64) -> u64 {
    unsafe { std::ptr::read_unaligned(address as *const u64) }
}

fn write_u32_at(address: u64, value: u32) {
    write_page(address, &value.to_le_bytes());
}

fn write_u64_at(address: u64, value: u64) {
    write_page(address, &value.to_le_bytes());
}

/// Present the accepted PC v2.01 live-add layout over this helper's own image.
#[cfg(windows)]
fn live_add_setup(module_base: u64, mode: &str, source: &[u8]) -> Result<LiveAddRuntime, String> {
    if module_base == 0 {
        return Err("the helper has no module base".to_string());
    }
    if source.len() != LIVE_ADD_RECORD_SIZE {
        return Err("the live-add source must be exactly one 232-byte record".to_string());
    }
    let entry = module_base + LIVE_ADD_DISPATCH_RVA;
    let builder_address = module_base + LIVE_ADD_BUILDER_RVA;
    let manager_slot = module_base + LIVE_ADD_MANAGER_RVA;
    let insertion_site = module_base + LIVE_ADD_INSERTION_RVA;
    commit_fixed(entry)?;
    commit_fixed(builder_address)?;
    commit_fixed(manager_slot)?;
    commit_fixed(insertion_site)?;
    write_bytes(insertion_site, &LIVE_ADD_INSERTION_SIGNATURE);

    let data = commit_read_write(LIVE_ADD_DATA_SIZE)?;
    // The product resolves two hops: the manager slot names a manager object,
    // and the manager object's first eight bytes name the data object. Collapsing
    // the two would make the data address the counter value instead.
    let manager = commit_read_write(0x1000)?;
    write_u64_at(manager, data);
    write_u32_at(data, 11);
    write_u64_at(data + 8, 0x3345);
    write_u64_at(
        data + LIVE_ADD_CONTAINER_OFFSET + LIVE_ADD_CAPACITY_OFFSET,
        LIVE_ADD_CAPACITY,
    );
    for (slot, serial, seed) in [(4usize, 0x1234u64, 0xF00Du32), (100, 0x5678, 0x2222)] {
        let mut record = [0u8; LIVE_ADD_RECORD_SIZE];
        record[0] = 0x82;
        record[1] = 0x1E;
        record[0x18] = 0x02;
        record[0x1A] = 0x80;
        record[0x1C..0x20].copy_from_slice(&11u32.to_le_bytes());
        record[0x20..0x24].copy_from_slice(&seed.to_le_bytes());
        record[0x28..0x30].copy_from_slice(&serial.to_le_bytes());
        record[0x30] = 4;
        record[0x33] = 2;
        write_bytes(
            data + LIVE_ADD_CONTAINER_OFFSET + (slot * LIVE_ADD_RECORD_SIZE) as u64,
            &record,
        );
    }
    // The real native serial index the product's capture_index walks, with the
    // same two records as the container (decimal serial forms are 4660 and
    // 22136).
    seed_live_add_index(
        data + LIVE_ADD_SERIAL_INDEX_OFFSET,
        &[(4, 0x1234), (100, 0x5678)],
    );
    write_u64_at(manager_slot, manager);

    let template = data + LIVE_ADD_TEMPLATE_OFFSET;
    let counter = data + LIVE_ADD_COUNTER_OFFSET;
    write_bytes(template, source);
    write_u64_at(counter, 0);

    let hang = mode == "noack";
    let builder = live_add_builder_code(template, counter, hang);
    if builder.len() > LIVE_ADD_BUILDER_SIZE {
        return Err("the builder stand-in does not fit the reviewed region".to_string());
    }
    let mut padded = vec![0xCCu8; LIVE_ADD_BUILDER_SIZE];
    padded[..builder.len()].copy_from_slice(&builder);
    write_bytes(builder_address, &padded);
    write_bytes(entry, &LIVE_ADD_PROLOGUE);
    write_bytes(entry + LIVE_ADD_PROLOGUE.len() as u64, &LIVE_ADD_EPILOGUE);

    // One owned worker thread calls the entry whenever a debugger is attached.
    // It only enters once Dr0 can intercept it, so the entry's own fall-through
    // is never executed while the acknowledgement breakpoint is armed.
    std::thread::spawn(move || {
        use windows_sys::Win32::System::Diagnostics::Debug::IsDebuggerPresent;
        while unsafe { IsDebuggerPresent() } == 0 {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let call: extern "C" fn() = unsafe { std::mem::transmute(entry as usize) };
        for _ in 0..4_000 {
            call();
            if read_u64_at(counter) > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });

    let creation = nioh3_runtime::platform::process_creation_filetime(std::process::id())
        .map_err(|error| error.message().to_string())?
        .ok_or("the helper has no creation time")?
        .to_string();
    Ok(LiveAddRuntime {
        entry,
        data,
        counter,
        builder_hex: hex(&padded),
        creation,
    })
}

#[cfg(not(windows))]
fn live_add_setup(
    _module_base: u64,
    _mode: &str,
    _source: &[u8],
) -> Result<LiveAddRuntime, String> {
    Err("the helper is Windows-only".to_string())
}

#[cfg(not(windows))]
fn allocate_oracle_page(_module_base: u64, _slow: bool) -> Result<(u64, u64), String> {
    Err("the helper is Windows-only".to_string())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_hex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(digits.len() / 2);
    for pair in digits.chunks(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        out.push((high * 16 + low) as u8);
    }
    Some(out)
}

fn main() -> std::process::ExitCode {
    let pid = std::process::id();
    let image = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    let module_base = nioh3_runtime::module_range(pid, &image)
        .map(|range| range.base)
        .unwrap_or(0);
    let page = match allocate_page(module_base) {
        Ok(page) => page,
        Err(error) => {
            println!("error\t{error}");
            return std::process::ExitCode::from(3);
        }
    };
    write_page(page, &SITE_BYTES);
    println!("ready\t{pid}\t{module_base}\t{page}\t{}", hex(&SITE_BYTES));
    let _ = std::io::stdout().flush();

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let mut parts = line.trim().splitn(2, ' ');
        match parts.next().unwrap_or_default() {
            "bytes" => {
                // Report what the page currently holds, so a test can see a
                // patch written by another process and a later restore.
                let current =
                    unsafe { std::slice::from_raw_parts(page as *const u8, SITE_BYTES.len()) };
                println!("bytes\t{}", hex(current));
                let _ = std::io::stdout().flush();
            }
            "poke" => {
                let Some(bytes) = parts.next().and_then(parse_hex) else {
                    println!("error\tpoke needs hex bytes");
                    let _ = std::io::stdout().flush();
                    continue;
                };
                write_page(page, &bytes);
                println!("poked\t{}", hex(&bytes));
                let _ = std::io::stdout().flush();
            }
            "layout" => match allocate_layout() {
                Ok((base, container)) => {
                    println!("layout\t{base:x}\t{container:x}");
                }
                Err(error) => println!("error\t{error}"),
            },
            "oracle" | "oracle-slow" => {
                // One owned function the batch oracle calls through a real
                // CreateRemoteThread, plus the record it copies.
                match allocate_oracle_page(module_base, line.trim() == "oracle-slow") {
                    Ok((page, template)) => {
                        let code = LAST_ORACLE_CODE.with(|cell| cell.borrow().clone());
                        println!("oracle\t{page:x}\t{template:x}\t{}", hex(&code));
                    }
                    Err(error) => println!("error\t{error}"),
                }
                let _ = std::io::stdout().flush();
            }
            "poke-at" => {
                // The external writer: the test uses this to change a byte the
                // mutation owner is reading, so a real inconsistency is visible.
                let rest = parts.next().unwrap_or_default();
                let mut tokens = rest.split_whitespace();
                let address = tokens
                    .next()
                    .and_then(|text| u64::from_str_radix(text.trim_start_matches("0x"), 16).ok());
                let bytes = tokens.next().and_then(parse_hex);
                match (address, bytes) {
                    (Some(address), Some(bytes)) => {
                        write_page(address, &bytes);
                        println!("poked\t{address:x}\t{}", hex(&bytes));
                    }
                    _ => println!("error\tpoke-at needs an address and hex bytes"),
                }
                let _ = std::io::stdout().flush();
            }
            "live-add" => {
                // `live-add <matched|mismatch|noack> <232-byte source record>`:
                // present the accepted PC v2.01 live-add layout over this owned
                // helper so a real debug session can run one preview against it.
                let rest = parts.next().unwrap_or_default();
                let mut tokens = rest.split_whitespace();
                let mode = tokens.next().unwrap_or_default().to_string();
                let source = tokens.next().and_then(parse_hex);
                match (mode.as_str(), source) {
                    ("matched" | "mismatch" | "noack", Some(source)) => {
                        match live_add_setup(module_base, &mode, &source) {
                            Ok(runtime) => {
                                println!(
                                    "live-add\t{:x}\t{}\t{:x}\t{}\t{:x}",
                                    runtime.entry,
                                    runtime.builder_hex,
                                    runtime.counter,
                                    runtime.creation,
                                    runtime.data
                                );
                                let mut guard = LIVE_ADD
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                                *guard = Some(runtime);
                            }
                            Err(error) => println!("error\t{error}"),
                        }
                    }
                    _ => println!(
                        "error\tlive-add needs <matched|mismatch|noack> and a 232-byte hex record"
                    ),
                }
                let _ = std::io::stdout().flush();
            }
            "live-add-calls" => {
                // The builder's own execution count, read from this process.
                let counter = LIVE_ADD
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .as_ref()
                    .map(|runtime| runtime.counter);
                match counter {
                    Some(counter) => println!("live-add-calls\t{}", read_u64_at(counter)),
                    None => println!("error\tno live-add target"),
                }
                let _ = std::io::stdout().flush();
            }
            "quit" => break,
            other => {
                println!("error\tunknown command {other}");
                let _ = std::io::stdout().flush();
            }
        }
    }
    std::process::ExitCode::from(0)
}
