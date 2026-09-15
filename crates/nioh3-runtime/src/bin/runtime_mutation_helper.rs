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
            "quit" => break,
            other => {
                println!("error\tunknown command {other}");
                let _ = std::io::stdout().flush();
            }
        }
    }
    std::process::ExitCode::from(0)
}
