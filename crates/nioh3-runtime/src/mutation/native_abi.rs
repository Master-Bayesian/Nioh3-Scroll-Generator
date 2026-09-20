//! Byte-exact port of the shipped remote machine code and the live-add shim.
//!
//! Two shipped emitters are reproduced here without reinterpretation:
//!
//! * `native.py`'s batch wrappers (`build_batch_wrapper`,
//!   `build_seed_range_wrapper`, `build_effect_finalizer_wrapper`,
//!   `build_effect_finalizer_batch_wrapper`,
//!   `build_explicit_playthrough_seed_range_wrapper`) and `build_source_record`;
//! * `live_add_dispatch_code.build_dispatch_code`, the one x64 shim the live-add
//!   executor runs inside a stopped mission thread.
//!
//! The module is pure: it computes bytes from values and never opens, reads,
//! writes or allocates anything. Both cross-language gates compare its output
//! byte for byte with the shipped Python on identical inputs, so the executor
//! and the oracle can be reviewed as data before any process is touched.

use crate::error::RuntimeError;
use crate::profile::NativeRuntimeProfile;

/// `emaki_exchange.SCROLL_RECORD_SIZE`.
pub const SCROLL_RECORD_SIZE: usize = 0xE8;
/// `emaki_exchange.EFFECT_START`.
pub const EFFECT_START: usize = 0x34;
/// `emaki_exchange.EFFECT_STRIDE`.
pub const EFFECT_STRIDE: usize = 0x18;
/// `live_add_descriptor.assembly_descriptor` output size.
pub const DESCRIPTOR_SIZE: usize = 0xCC;
/// `native.REMOTE_CODE_SIZE`.
pub const REMOTE_CODE_SIZE: u64 = 0x400;
/// `native.NativeBatchOracle` batch bound.
pub const ORACLE_BATCH_LIMIT: usize = 4096;

/// `native.PROCESS_ACCESS` and `windows_debug_session.WindowsDebug`'s
/// `OpenProcess(0x043A, ...)`: one mask, two shipped callers.
///
/// `PROCESS_CREATE_THREAD | PROCESS_VM_OPERATION | PROCESS_VM_READ |
/// PROCESS_VM_WRITE | PROCESS_QUERY_INFORMATION`. `PROCESS_CREATE_THREAD` is the
/// only right either shipped path uses beyond the override mask, and it exists
/// there for `CreateRemoteThread` (oracle) and `DebugBreakProcess` (live add).
/// `PROCESS_ALL_ACCESS`, `PROCESS_TERMINATE`, `PROCESS_SUSPEND_RESUME` and
/// `DebugActiveProcess` are still never requested.
pub const RUNTIME_ACCESS: u32 = 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0400;

/// `MEM_COMMIT | MEM_RESERVE`.
pub const MEM_COMMIT_RESERVE: u32 = 0x1000 | 0x2000;
/// `MEM_RELEASE`.
pub const MEM_RELEASE: u32 = 0x8000;
/// `PAGE_EXECUTE_READWRITE`.
pub const PAGE_EXECUTE_READWRITE: u32 = 0x40;

/// `live_add_profile.PC_V201`; the only live-add layout the product validates.
///
/// ABI/shim offsets belong to the executor and the game RVAs belong to the
/// version profile, exactly as the shipped comment says. This type carries both
/// halves for one accepted build, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveAddLayout {
    pub profile_id: &'static str,
    pub dispatch_rva: u64,
    pub dispatch_return_rva: u64,
    pub dispatch_signature: [u8; 7],
    pub builder_rva: u64,
    pub builder_size: u64,
    pub insertion_rva: u64,
    pub insertion_size: u64,
    pub slot_lookup_rva: u64,
    pub manager_pointer_rva: u64,
    pub scheduler_pointer_rva: u64,
    pub container_offset: u64,
    pub capacity_offset: u64,
    pub serial_counter_offset: u64,
    pub serial_index_offset: u64,
    pub scheduler_pending_offset: u64,
    pub scheduler_ready_offset: u64,
    pub queue_begin_offset: u64,
    pub queue_end_offset: u64,
    pub record_size: usize,
    pub descriptor_size: usize,
    pub capacity: u32,
}

/// `live_add_profile.PC_V201`.
pub const PC_V201_LIVE_ADD: LiveAddLayout = LiveAddLayout {
    profile_id: "pc-v2.01-live-add-r1",
    dispatch_rva: 0x12E6840,
    dispatch_return_rva: 0x20BB2C,
    dispatch_signature: [0x40, 0x53, 0x57, 0x48, 0x83, 0xEC, 0x38],
    builder_rva: 0x227C4CC,
    builder_size: 0x27B,
    insertion_rva: 0x54D294,
    insertion_size: 0xE17,
    slot_lookup_rva: 0x552FBC,
    manager_pointer_rva: 0x474D4E0,
    scheduler_pointer_rva: 0x47412F8,
    container_offset: 0x224A60,
    capacity_offset: 0x16A80,
    serial_counter_offset: 8,
    serial_index_offset: 0x23B5E8,
    scheduler_pending_offset: 0x1408,
    scheduler_ready_offset: 0x1629,
    queue_begin_offset: 0x60,
    queue_end_offset: 0x68,
    record_size: SCROLL_RECORD_SIZE,
    descriptor_size: DESCRIPTOR_SIZE,
    capacity: 400,
};

/// PC v2.02 candidate live-add layout. Not accepted: nothing selects it.
///
/// Additive and disabled. [`PC_V201_LIVE_ADD`] stays the only layout the
/// product validates, `profile_for_game_version` still refuses `2.0.2.0`, and
/// neither the protected host nor any JSON parameter can name this constant. It
/// exists so the version-owned RVAs the PC v2.02 lanes derived can be reviewed
/// as data and pinned by tests before one controlled validation through the
/// injected transport seam.
///
/// `profile_id` carries the candidate marker on purpose: the executor's
/// identity gate compares the transport's claimed profile id with the layout's
/// own id, so this layout can never be mistaken for the shipped
/// `pc-v2.01-live-add-r1` identity, and any run that used it would be an
/// explicit research claim rather than product selection.
pub const PC_V202_LIVE_ADD_CANDIDATE: LiveAddLayout = LiveAddLayout {
    profile_id: "pc-v2.02-live-add-candidate",
    dispatch_rva: 0x12E9E50,
    dispatch_return_rva: 0x20BB1C,
    dispatch_signature: [0x40, 0x53, 0x57, 0x48, 0x83, 0xEC, 0x38],
    builder_rva: 0x227FC5C,
    builder_size: 0x27B,
    insertion_rva: 0x54D324,
    insertion_size: 0xE17,
    slot_lookup_rva: 0x55308C,
    manager_pointer_rva: 0x4751530,
    scheduler_pointer_rva: 0x4745348,
    container_offset: 0x224A60,
    capacity_offset: 0x16A80,
    serial_counter_offset: 8,
    serial_index_offset: 0x23B5E8,
    scheduler_pending_offset: 0x1408,
    scheduler_ready_offset: 0x1629,
    queue_begin_offset: 0x60,
    queue_end_offset: 0x68,
    // Carried from PC v2.01 and confirmed by the v2.02 static and live lanes;
    // not re-derived here.
    record_size: SCROLL_RECORD_SIZE,
    descriptor_size: DESCRIPTOR_SIZE,
    capacity: 400,
};

/// The executable the PC v2.02 candidate RVAs were read from.
pub const PC_V202_CANDIDATE_EXECUTABLE_SHA256: &str =
    "E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130";

/// The captured `.text` image of that executable, the lanes' decode input.
pub const PC_V202_CANDIDATE_TEXT_SHA256: &str =
    "4CEC8FB6AD867417A76DF8201C1D4F54172443884910463C1953ACAD91AE6C29";

/// Display version of the accepted product build.
///
/// One accepted layout pairs with one display version: a version string alone
/// never authorizes a layout, and a layout alone never authorizes a version.
pub const PRODUCT_DISPLAY_VERSION: &str = "PC v2.01";

/// Display version of the accepted research candidate build.
///
/// The candidate is opt-in research only. Naming this version without
/// [`PC_V202_LIVE_ADD_CANDIDATE`] and the pinned
/// [`PC_V202_CANDIDATE_EXECUTABLE_SHA256`] is refused by the executor's binding
/// gate, and no product path names it.
pub const CANDIDATE_DISPLAY_VERSION: &str = "PC v2.02";

/// Owning lane for every version-owned value of
/// [`PC_V202_LIVE_ADD_CANDIDATE`].
///
/// The three record/descriptor constants carried unchanged from PC v2.01 and
/// the authored `profile_id` have no lane of their own and are deliberately
/// absent. `inventory_global_mode` records the ABI choice this layout fixes
/// rather than a field of [`LiveAddLayout`]; the Python mirror
/// (`live_add_profile.PC_V202_EVIDENCE`) names the same lanes and spells the
/// signature field `dispatch_signature_hex`.
pub const PC_V202_CANDIDATE_EVIDENCE: [(&str, &str); 19] = [
    (
        "manager_pointer_rva",
        "deepseek-v202-inventory-live-verify + go-v202-acquisition-contract",
    ),
    ("scheduler_pointer_rva", "deepseek-v202-scheduler-recovery"),
    (
        "scheduler_pending_offset",
        "deepseek-v202-scheduler-recovery",
    ),
    ("scheduler_ready_offset", "deepseek-v202-scheduler-recovery"),
    ("queue_begin_offset", "deepseek-v202-scheduler-recovery"),
    ("queue_end_offset", "deepseek-v202-scheduler-recovery"),
    ("dispatch_rva", "deepseek-v202-scheduler-recovery"),
    ("dispatch_return_rva", "deepseek-v202-scheduler-recovery"),
    ("dispatch_signature", "deepseek-v202-scheduler-recovery"),
    ("builder_rva", "deepseek-v202-scheduler-recovery"),
    ("builder_size", "deepseek-v202-scheduler-recovery"),
    ("insertion_rva", "deepseek-v202-scheduler-recovery"),
    ("insertion_size", "deepseek-v202-scheduler-recovery"),
    ("slot_lookup_rva", "deepseek-v202-layout-static-offsets"),
    ("container_offset", "deepseek-v202-layout-static-offsets"),
    ("capacity_offset", "deepseek-v202-layout-static-offsets"),
    (
        "serial_counter_offset",
        "deepseek-v202-layout-static-offsets",
    ),
    ("serial_index_offset", "deepseek-v202-layout-static-offsets"),
    ("inventory_global_mode", "deepseek-v202-layout-acceptance"),
];

/// Live-add dispatch page offsets (`memory + <offset>` in the shipped shim).
pub const DISPATCH_MARKER_OFFSET: u64 = 0x300;
pub const BUILDER_RESULT_OFFSET: u64 = 0x308;
pub const DISPATCH_STATUS_OFFSET: u64 = 0x318;
pub const DISPATCH_SLOT_OFFSET: u64 = 0x320;
pub const INSERTION_RESULT_OFFSET: u64 = 0x328;
pub const DISPATCH_DESCRIPTOR_OFFSET: u64 = 0x400;
pub const DISPATCH_SOURCE_OFFSET: u64 = 0x600;
pub const DISPATCH_REMAINDER_OFFSET: u64 = 0x800;
/// The four 16-byte canaries the shim must leave untouched.
pub const DISPATCH_CANARIES: [u64; 4] = [0x5F0, 0x6E8, 0x7F0, 0x8E8];

/// The insertion branch's own arguments, all absolute addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InsertionArgs {
    pub serial: u64,
    pub data: u64,
    pub manager: u64,
    pub function_address: u64,
    pub serial_counter_offset: u64,
}

fn abi(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::NativeAbi {
        detail: detail.into(),
    }
}

/// `live_add_dispatch_code.build_dispatch_code`.
///
/// `memory` is the executor's own allocation, `resume` the address the shim
/// returns to and `original` the bytes it replays. `leaf` is the game's builder
/// (or `None` for `noop`), `argument`/`second_argument` are the `rcx`/`rdx` it
/// receives, and `insertion` selects the insertion branch.
#[allow(clippy::too_many_arguments)]
pub fn build_dispatch_code(
    memory: u64,
    resume: u64,
    original: &[u8],
    leaf: Option<u64>,
    argument: u64,
    second_argument: Option<u64>,
    insertion: Option<InsertionArgs>,
    preserve_rarity5: bool,
) -> Result<Vec<u8>, RuntimeError> {
    let mut code: Vec<u8> = Vec::with_capacity(0x300);
    let mut jumps: Vec<usize> = Vec::new();

    if let Some(leaf) = leaf {
        emit_hex(&mut code, "9C 50 51 52 41 50 41 51 41 52 41 53");
        let (stack_size, xmm_offset) = if insertion.is_some() {
            (0x98u32, 0x30u32)
        } else {
            (0x88u32, 0x20u32)
        };
        emit_hex(&mut code, "48 81 EC");
        code.extend_from_slice(&stack_size.to_le_bytes());
        save_xmm(&mut code, 0x7F, xmm_offset);
        emit_hex(&mut code, "48 B9");
        code.extend_from_slice(&argument.to_le_bytes());
        code.extend_from_slice(&[0x48, 0xB8]);
        code.extend_from_slice(&leaf.to_le_bytes());
        match second_argument {
            Some(value) => {
                code.extend_from_slice(&[0x48, 0xBA]);
                code.extend_from_slice(&value.to_le_bytes());
            }
            None => emit_hex(&mut code, "33 D2"),
        }
        emit_hex(&mut code, "FF D0 48 A3");
        code.extend_from_slice(&(memory + BUILDER_RESULT_OFFSET).to_le_bytes());
        if preserve_rarity5 {
            // `NativeBatchOracle`'s v2.01 raw-R5 header preservation, applied
            // to our returned scratch record before the insertion branch.
            emit_hex(&mut code, "49 BA");
            code.extend_from_slice(&(memory + DISPATCH_SOURCE_OFFSET).to_le_bytes());
            emit_hex(&mut code, "4C 39 D0");
            reject_unless_equal(&mut code, &mut jumps);
            emit_hex(&mut code, "66 81 78 30 04 04 75 06 66 C7 40 30 05 05");
            emit_hex(&mut code, "66 81 78 30 05 05");
            reject_unless_equal(&mut code, &mut jumps);
        }
        if let Some(insertion) = insertion {
            marker(&mut code, 0x318, 1);
            emit_hex(&mut code, "49 BA");
            code.extend_from_slice(&(memory + DISPATCH_SOURCE_OFFSET).to_le_bytes());
            emit_hex(&mut code, "4C 39 D0");
            reject_unless_equal(&mut code, &mut jumps);
            emit_hex(&mut code, "49 BB");
            code.extend_from_slice(&insertion.serial.to_le_bytes());
            emit_hex(&mut code, "4D 39 5A 28");
            reject_unless_equal(&mut code, &mut jumps);
            emit_hex(&mut code, "49 BA");
            code.extend_from_slice(
                &(insertion.data + insertion.serial_counter_offset).to_le_bytes(),
            );
            emit_hex(&mut code, "49 BB");
            code.extend_from_slice(&(insertion.serial + 1).to_le_bytes());
            emit_hex(&mut code, "4D 39 1A");
            reject_unless_equal(&mut code, &mut jumps);
            marker(&mut code, 0x318, 2);
            emit_hex(&mut code, "48 B9");
            code.extend_from_slice(&insertion.manager.to_le_bytes());
            emit_hex(&mut code, "48 BA");
            code.extend_from_slice(&(memory + DISPATCH_REMAINDER_OFFSET).to_le_bytes());
            emit_hex(&mut code, "49 B8");
            code.extend_from_slice(&(memory + DISPATCH_SOURCE_OFFSET).to_le_bytes());
            emit_hex(&mut code, "49 B9");
            code.extend_from_slice(&(memory + DISPATCH_SLOT_OFFSET).to_le_bytes());
            emit_hex(&mut code, "C7 44 24 20 00 00 00 00 48 B8");
            code.extend_from_slice(&insertion.function_address.to_le_bytes());
            emit_hex(&mut code, "FF D0 48 A3");
            code.extend_from_slice(&(memory + INSERTION_RESULT_OFFSET).to_le_bytes());
            marker(&mut code, 0x318, 3);
        }
        for offset in &jumps {
            let displacement = code.len() as i64 - *offset as i64 - 4;
            let displacement =
                i32::try_from(displacement).map_err(|_| abi("Dispatch code overlaps its data"))?;
            code[*offset..*offset + 4].copy_from_slice(&displacement.to_le_bytes());
        }
        save_xmm(&mut code, 0x6F, xmm_offset);
        emit_hex(&mut code, "48 81 C4");
        code.extend_from_slice(&stack_size.to_le_bytes());
        emit_hex(&mut code, "41 5B 41 5A 41 59 41 58 5A 59 58 9D");
    }

    marker(&mut code, 0x300, 1);
    code.extend_from_slice(original);
    emit_hex(&mut code, "FF 25 00 00 00 00");
    code.extend_from_slice(&resume.to_le_bytes());
    if code.len() >= 0x300 {
        return Err(abi("Dispatch code overlaps its data"));
    }
    Ok(code)
}

fn save_xmm(code: &mut Vec<u8>, opcode: u8, xmm_offset: u32) {
    for index in 0..6u32 {
        let offset = xmm_offset + index * 16;
        let prefix = if offset < 128 { 0x44u32 } else { 0x84 };
        code.extend_from_slice(&[0xF3, 0x0F, opcode, (prefix + index * 8) as u8, 0x24]);
        if offset < 128 {
            code.push(offset as u8);
        } else {
            code.extend_from_slice(&offset.to_le_bytes());
        }
    }
}

/// `C7 05 <rel32> <imm32>` pointing forward at the marker cell.
fn marker(code: &mut Vec<u8>, offset: i64, value: u32) {
    let displacement = offset - (code.len() as i64 + 10);
    code.extend_from_slice(&[0xC7, 0x05]);
    code.extend_from_slice(&(displacement as i32).to_le_bytes());
    code.extend_from_slice(&value.to_le_bytes());
}

/// `0F 85 <rel32>`, fixed up to the end of the guarded block.
fn reject_unless_equal(code: &mut Vec<u8>, jumps: &mut Vec<usize>) {
    code.extend_from_slice(&[0x0F, 0x85]);
    jumps.push(code.len());
    code.extend_from_slice(&[0, 0, 0, 0]);
}

/// `native.build_batch_wrapper`.
pub fn build_batch_wrapper(
    source: u64,
    destination: u64,
    function: u64,
    count: u32,
) -> Result<Vec<u8>, RuntimeError> {
    if count == 0 {
        return Err(abi("count must fit in uint32 and be nonzero"));
    }
    let mut code = hex_decode("53 56 57 41 54 48 83 EC 28").unwrap_or_default();
    code.extend_from_slice(&[0x48, 0xBB]);
    code.extend_from_slice(&source.to_le_bytes());
    code.extend_from_slice(&[0x48, 0xBE]);
    code.extend_from_slice(&destination.to_le_bytes());
    code.push(0xBF);
    code.extend_from_slice(&count.to_le_bytes());
    code.extend_from_slice(&[0x49, 0xBC]);
    code.extend_from_slice(&function.to_le_bytes());
    let loop_body =
        hex_decode("48 89 F1 48 89 DA 41 FF D4 48 81 C3 E8 00 00 00 48 81 C6 E8 00 00 00 FF CF")
            .unwrap_or_default();
    let loop_len = loop_body.len();
    let jump_back = -(loop_len as i32 + 2);
    if !(-128..=127).contains(&jump_back) {
        return Err(abi("batch wrapper loop no longer fits a short jump"));
    }
    code.extend_from_slice(&loop_body);
    code.push(0x75);
    code.push(jump_back as i8 as u8);
    code.extend_from_slice(&hex_decode("31 C0 48 83 C4 28 41 5C 5F 5E 5B C3").unwrap_or_default());
    Ok(code)
}

/// `native.build_seed_range_wrapper`.
pub fn build_seed_range_wrapper(
    source: u64,
    destination: u64,
    function: u64,
    start_seed: u32,
    seed_step: u32,
    count: u32,
) -> Result<Vec<u8>, RuntimeError> {
    check_seed_range(seed_step, count)?;
    let mut code = hex_decode("53 56 57 41 54 41 55 48 83 EC 20").unwrap_or_default();
    code.extend_from_slice(&[0x48, 0xBB]);
    code.extend_from_slice(&source.to_le_bytes());
    code.extend_from_slice(&[0x48, 0xBE]);
    code.extend_from_slice(&destination.to_le_bytes());
    code.push(0xBF);
    code.extend_from_slice(&count.to_le_bytes());
    code.extend_from_slice(&[0x41, 0xBD]);
    code.extend_from_slice(&start_seed.to_le_bytes());
    code.extend_from_slice(&[0x49, 0xBC]);
    code.extend_from_slice(&function.to_le_bytes());
    let mut loop_body = hex_decode("44 89 6B 20 48 89 F1 48 89 DA 41 FF D4 48 81 C6 E8 00 00 00")
        .unwrap_or_default();
    loop_body.extend_from_slice(&[0x41, 0x81, 0xC5]);
    loop_body.extend_from_slice(&seed_step.to_le_bytes());
    loop_body.extend_from_slice(&[0xFF, 0xCF]);
    let loop_len = loop_body.len();
    let jump_back = -(loop_len as i32 + 2);
    if !(-128..=127).contains(&jump_back) {
        return Err(abi("seed range wrapper loop no longer fits a short jump"));
    }
    code.extend_from_slice(&loop_body);
    code.push(0x75);
    code.push(jump_back as i8 as u8);
    code.extend_from_slice(
        &hex_decode("31 C0 48 83 C4 20 41 5D 41 5C 5F 5E 5B C3").unwrap_or_default(),
    );
    Ok(code)
}

fn check_seed_range(seed_step: u32, count: u32) -> Result<(), RuntimeError> {
    if seed_step == 0 {
        return Err(abi("seed_step must be between 1 and 0xFFFFFFFF"));
    }
    if count == 0 {
        return Err(abi("count must fit in uint32 and be nonzero"));
    }
    Ok(())
}

/// `native.build_effect_finalizer_wrapper`.
pub fn build_effect_finalizer_wrapper(
    source: u64,
    destination: u64,
    function: u64,
    effect_index: u32,
    reveal: bool,
) -> Result<Vec<u8>, RuntimeError> {
    if effect_index >= 7 {
        return Err(abi("effect_index must be between 0 and 6"));
    }
    let mut code = hex_decode("48 83 EC 28").unwrap_or_default();
    code.extend_from_slice(&[0x48, 0xB9]);
    code.extend_from_slice(&destination.to_le_bytes());
    code.extend_from_slice(&[0x48, 0xBA]);
    code.extend_from_slice(&source.to_le_bytes());
    code.extend_from_slice(&[0x41, 0xB8]);
    code.extend_from_slice(&effect_index.to_le_bytes());
    code.extend_from_slice(&[0x41, 0xB9]);
    code.extend_from_slice(&u32::from(reveal).to_le_bytes());
    code.extend_from_slice(&[0x48, 0xB8]);
    code.extend_from_slice(&function.to_le_bytes());
    code.extend_from_slice(&hex_decode("FF D0 31 C0 48 83 C4 28 C3").unwrap_or_default());
    Ok(code)
}

/// `native.build_effect_finalizer_batch_wrapper`.
pub fn build_effect_finalizer_batch_wrapper(
    source: u64,
    destination: u64,
    function: u64,
    count: u32,
    effect_index: u32,
    reveal: bool,
) -> Result<Vec<u8>, RuntimeError> {
    if count == 0 {
        return Err(abi("count must fit in uint32 and be nonzero"));
    }
    if effect_index >= 7 {
        return Err(abi("effect_index must be between 0 and 6"));
    }
    let mut code = hex_decode("53 56 57 41 54 48 83 EC 28").unwrap_or_default();
    code.extend_from_slice(&[0x48, 0xBB]);
    code.extend_from_slice(&source.to_le_bytes());
    code.extend_from_slice(&[0x48, 0xBE]);
    code.extend_from_slice(&destination.to_le_bytes());
    code.push(0xBF);
    code.extend_from_slice(&count.to_le_bytes());
    code.extend_from_slice(&[0x49, 0xBC]);
    code.extend_from_slice(&function.to_le_bytes());
    let mut loop_body = hex_decode("48 89 F1 48 89 DA").unwrap_or_default();
    loop_body.extend_from_slice(&[0x41, 0xB8]);
    loop_body.extend_from_slice(&effect_index.to_le_bytes());
    loop_body.extend_from_slice(&[0x41, 0xB9]);
    loop_body.extend_from_slice(&u32::from(reveal).to_le_bytes());
    loop_body.extend_from_slice(
        &hex_decode("41 FF D4 48 81 C3 E8 00 00 00 48 81 C6 E8 00 00 00 FF CF").unwrap_or_default(),
    );
    let loop_len = loop_body.len();
    let jump_back = -(loop_len as i32 + 2);
    if !(-128..=127).contains(&jump_back) {
        return Err(abi(
            "effect finalizer batch loop no longer fits a short jump",
        ));
    }
    code.extend_from_slice(&loop_body);
    code.push(0x75);
    code.push(jump_back as i8 as u8);
    code.extend_from_slice(&hex_decode("31 C0 48 83 C4 28 41 5C 5F 5E 5B C3").unwrap_or_default());
    Ok(code)
}

/// The nine module-relative addresses
/// `native.build_explicit_playthrough_seed_range_wrapper` calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaythroughChainRvas {
    pub init_compact: u64,
    pub reset_compact: u64,
    pub effective_level: u64,
    pub init_generation_context: u64,
    pub incomplete_record: u64,
    pub generate_effects: u64,
    pub assemble_scroll: u64,
    pub playthrough_vector: u64,
    pub playthrough_manager_pointer: u64,
}

impl PlaythroughChainRvas {
    /// Resolve the chain by site name from a validated runtime profile.
    pub fn from_profile(profile: &NativeRuntimeProfile) -> Result<Self, RuntimeError> {
        let site = |name: &str| -> Result<u64, RuntimeError> {
            profile
                .site(name)
                .map(|site| site.rva)
                .ok_or_else(|| RuntimeError::ProfileUnresolved {
                    site: name.to_string(),
                })
        };
        Ok(Self {
            init_compact: site("init_compact")?,
            reset_compact: site("reset_compact")?,
            effective_level: site("effective_level")?,
            init_generation_context: site("init_generation_context")?,
            incomplete_record: site("incomplete_record")?,
            generate_effects: site("generate_effects")?,
            assemble_scroll: site("assemble_scroll")?,
            playthrough_vector: site("playthrough_vector")?,
            playthrough_manager_pointer: profile.playthrough_selector_pointer_rva,
        })
    }
}

/// `native.build_explicit_playthrough_seed_range_wrapper`.
#[allow(clippy::too_many_arguments)]
pub fn build_explicit_playthrough_seed_range_wrapper(
    source: u64,
    destination: u64,
    module_base: u64,
    start_seed: u32,
    seed_step: u32,
    count: u32,
    playthrough: u32,
    generation_mode: u32,
    chain: &PlaythroughChainRvas,
) -> Result<Vec<u8>, RuntimeError> {
    check_seed_range(seed_step, count)?;
    if !(1..=5).contains(&playthrough) {
        return Err(abi("playthrough must be between 1 and 5"));
    }
    if generation_mode > 1 {
        return Err(abi("generation_mode must be 0 or 1"));
    }

    let mut builder = CodeBuilder::default();
    builder.emit_hex("53 56 57 41 54 41 55 41 56 41 57");
    builder.emit_hex("48 81 EC A0 01 00 00");
    builder.emit_imm64(0x48, 0xBB, source);
    builder.emit_imm64(0x48, 0xBE, destination);
    builder.emit_imm32(0x41, 0xBC, count);
    builder.emit_imm32(0x41, 0xBD, start_seed);
    builder.emit_imm32(0x41, 0xBE, seed_step);
    builder.emit_imm32(0x41, 0xBF, playthrough);

    builder.mark("loop");
    builder.emit_hex("44 89 6B 20");

    builder.emit_hex("48 8D 4C 24 60");
    builder.emit_call(module_base + chain.init_compact);
    builder.emit_hex("48 8D 4C 24 60");
    builder.emit_call(module_base + chain.reset_compact);

    builder.emit_hex("0F B7 03");
    builder.emit_hex("66 89 44 24 60");
    builder.emit_hex("48 89 D9");
    builder.emit_call(module_base + chain.effective_level);
    builder.emit_hex("0F B7 C0");
    builder.emit_hex("89 44 24 64");
    builder.emit_hex("0F B7 43 10");
    builder.emit_hex("89 44 24 68");

    builder.emit_hex("44 0F B6 43 31");
    builder.emit_hex("41 80 F8 03");
    builder.jump32("0F 83", "rarity_ready");
    builder.emit_hex("41 B8 03 00 00 00");
    builder.mark("rarity_ready");
    builder.emit_hex("44 88 44 24 6C");

    builder.emit_hex("44 8B 4B 20");
    builder.emit_hex("44 89 4C 24 70");
    builder.emit_hex("8B 83 DC 00 00 00");
    builder.emit_hex("89 44 24 74");

    builder.emit_hex("44 0F B7 53 02");
    builder.emit_hex("0F B7 43 04");
    builder.emit_hex("49 C1 E2 10");
    builder.emit_hex("49 09 C2");
    builder.emit_hex("8B 43 14");
    builder.emit_hex("49 C1 E2 20");
    builder.emit_hex("49 09 C2");
    builder.emit_hex("4C 89 54 24 78");
    builder.emit_hex("8A 43 0F");
    builder.emit_hex("88 84 24 82 00 00 00");

    builder.emit_hex("48 8D 4C 24 20");
    builder.emit_hex("0F B7 13");
    builder.emit_call(module_base + chain.init_generation_context);

    builder.emit_hex("31 C0");
    builder.emit_hex("48 89 84 24 40 01 00 00");
    builder.emit_hex("48 89 84 24 48 01 00 00");
    builder.emit_hex("44 88 BC 24 40 01 00 00");
    builder.emit_imm64(0x48, 0xB8, module_base + chain.playthrough_manager_pointer);
    builder.emit_hex("48 8B 00");
    builder.emit_hex("48 8B 40 08");
    builder.emit_hex("48 89 84 24 48 01 00 00");
    builder.emit_hex("48 8D 8C 24 40 01 00 00");
    builder.emit_hex("48 8D 94 24 60 01 00 00");
    builder.emit_call(module_base + chain.playthrough_vector);
    builder.emit_hex("0F 10 00");
    builder.emit_hex("0F 11 44 24 44");
    builder.emit_hex("44 88 7C 24 40");

    builder.emit_hex("48 89 D9");
    builder.emit_call(module_base + chain.incomplete_record);
    builder.emit_hex("C7 44 24 3C 00 00 00 00");
    builder.emit_hex("84 C0");
    builder.jump32("0F 84", "incomplete_ready");
    builder.emit_hex("C7 44 24 3C 00 00 80 3F");
    builder.mark("incomplete_ready");

    builder.emit_hex("48 8D 4C 24 70");
    builder.emit_hex("48 8D 94 24 84 00 00 00");
    builder.emit_hex("4C 8D 44 24 20");
    if generation_mode == 0 {
        builder.emit_hex("45 31 C9");
    } else {
        builder.emit_hex("41 B1 01");
    }
    builder.emit_call(module_base + chain.generate_effects);

    // An isolated search preview, not an inventory acquisition: the compact
    // descriptor's +0x21 flag suppresses the global serial allocation.
    builder.emit_hex("C6 84 24 81 00 00 00 01");
    builder.emit_hex("48 89 F1");
    builder.emit_hex("48 8D 54 24 60");
    builder.emit_call(module_base + chain.assemble_scroll);

    builder.emit_hex("48 81 C6 E8 00 00 00");
    builder.emit_hex("45 01 F5");
    builder.emit_hex("41 FF CC");
    builder.jump32("0F 85", "loop");

    builder.emit_hex("31 C0");
    builder.emit_hex("48 81 C4 A0 01 00 00");
    builder.emit_hex("41 5F 41 5E 41 5D 41 5C 5F 5E 5B C3");
    let wrapper = builder.finish()?;
    if wrapper.len() as u64 > REMOTE_CODE_SIZE {
        return Err(abi("explicit playthrough wrapper exceeds the code region"));
    }
    Ok(wrapper)
}

/// `native.build_source_record`: a template with the generation inputs applied.
pub fn build_source_record(
    template: &[u8],
    seed: u32,
    rarity: u8,
    level: u16,
    recommended_level: u16,
    transfer_count: u32,
) -> Result<Vec<u8>, RuntimeError> {
    if template.len() != SCROLL_RECORD_SIZE {
        return Err(abi("template must be exactly 0xE8 bytes"));
    }
    if rarity > 0x0F {
        return Err(abi("rarity is outside its supported range"));
    }
    let mut record = template.to_vec();
    record[0x06..0x08].copy_from_slice(&level.to_le_bytes());
    record[0x08..0x0A].copy_from_slice(&level.to_le_bytes());
    record[0x10..0x12].copy_from_slice(&recommended_level.to_le_bytes());
    record[0x12..0x14].copy_from_slice(&recommended_level.to_le_bytes());
    record[0x20..0x24].copy_from_slice(&seed.to_le_bytes());
    record[0x30] = rarity;
    record[0x31] = rarity;
    record[0xDC..0xE0].copy_from_slice(&transfer_count.to_le_bytes());
    Ok(record)
}

/// The minimal label-and-fixup builder `native._MachineCodeBuilder` uses.
#[derive(Default)]
struct CodeBuilder {
    code: Vec<u8>,
    labels: Vec<(&'static str, usize)>,
    fixups: Vec<(usize, &'static str)>,
}

impl CodeBuilder {
    fn emit_hex(&mut self, text: &str) {
        self.code.extend(hex_decode(text).unwrap_or_default());
    }

    fn emit_imm64(&mut self, prefix_a: u8, prefix_b: u8, value: u64) {
        self.code.extend_from_slice(&[prefix_a, prefix_b]);
        self.code.extend_from_slice(&value.to_le_bytes());
    }

    fn emit_imm32(&mut self, prefix_a: u8, prefix_b: u8, value: u32) {
        self.code.extend_from_slice(&[prefix_a, prefix_b]);
        self.code.extend_from_slice(&value.to_le_bytes());
    }

    fn emit_call(&mut self, address: u64) {
        self.emit_imm64(0x48, 0xB8, address);
        self.code.extend_from_slice(&[0xFF, 0xD0]);
    }

    fn mark(&mut self, label: &'static str) {
        self.labels.push((label, self.code.len()));
    }

    fn jump32(&mut self, opcode: &str, label: &'static str) {
        self.emit_hex(opcode);
        let offset = self.code.len();
        self.code.extend_from_slice(&[0, 0, 0, 0]);
        self.fixups.push((offset, label));
    }

    fn finish(mut self) -> Result<Vec<u8>, RuntimeError> {
        for (offset, label) in self.fixups.clone() {
            let target = self
                .labels
                .iter()
                .find(|(name, _)| *name == label)
                .map(|(_, position)| *position)
                .ok_or_else(|| abi(format!("missing machine-code label: {label}")))?;
            let displacement = target as i64 - (offset as i64 + 4);
            let displacement = i32::try_from(displacement)
                .map_err(|_| abi("machine-code displacement out of range"))?;
            self.code[offset..offset + 4].copy_from_slice(&displacement.to_le_bytes());
        }
        Ok(self.code)
    }
}

/// Lenient hex decode used by the emitters; these literals are programming
/// input, not operator input.
pub(crate) fn hex_decode(text: &str) -> Option<Vec<u8>> {
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

fn emit_hex(code: &mut Vec<u8>, text: &str) {
    code.extend(hex_decode(text).unwrap_or_default());
}

/// Lowercase hex, the shape every emitted-byte receipt uses.
pub fn hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_batch_wrapper_loops_back_to_its_own_body() -> Result<(), RuntimeError> {
        let wrapper = build_batch_wrapper(0x1000_0000, 0x1001_0000, 0x7FF0_1234, 3)?;
        let epilogue = hex_decode("31 C0 48 83 C4 28 41 5C 5F 5E 5B C3").unwrap_or_default();
        assert_eq!(&wrapper[wrapper.len() - epilogue.len()..], &epilogue[..]);
        let jump = wrapper.len() - epilogue.len() - 2;
        assert_eq!(wrapper[jump], 0x75);
        let displacement = wrapper[jump + 1] as i8 as i64;
        // The loop body starts after the fixed prefix: nine bytes of prologue,
        // then the source, destination, count and function loads.
        assert_eq!(44i64, (jump + 2) as i64 + displacement);
        Ok(())
    }

    #[test]
    fn a_zero_count_is_refused_by_every_wrapper() {
        assert!(build_batch_wrapper(1, 2, 3, 0).is_err());
        assert!(build_seed_range_wrapper(1, 2, 3, 0, 1, 0).is_err());
        assert!(build_seed_range_wrapper(1, 2, 3, 0, 0, 1).is_err());
        assert!(build_effect_finalizer_batch_wrapper(1, 2, 3, 0, 0, true).is_err());
        assert!(build_effect_finalizer_wrapper(1, 2, 3, 7, true).is_err());
    }

    #[test]
    fn an_effect_index_outside_the_seven_slots_is_refused() {
        assert!(build_effect_finalizer_wrapper(1, 2, 3, 7, true).is_err());
        assert!(build_effect_finalizer_batch_wrapper(1, 2, 3, 1, 7, true).is_err());
        assert!(build_effect_finalizer_wrapper(1, 2, 3, 6, false).is_ok());
    }

    #[test]
    fn a_preview_shim_ends_with_the_six_byte_indirect_jump() -> Result<(), RuntimeError> {
        let code = build_dispatch_code(
            0x1000_0000,
            0x7FF0_0000,
            &[0x90; 7],
            None,
            0,
            None,
            None,
            false,
        )?;
        assert!(code.len() < 0x300);
        assert_eq!(
            &code[code.len() - 14..code.len() - 8],
            &[0xFF, 0x25, 0, 0, 0, 0]
        );
        Ok(())
    }

    #[test]
    fn the_marker_points_forward_at_its_own_cell() -> Result<(), RuntimeError> {
        let code = build_dispatch_code(
            0x1000_0000,
            0x7FF0_0000,
            &[0x90; 7],
            None,
            0,
            None,
            None,
            false,
        )?;
        let marker = code
            .windows(2)
            .position(|pair| pair == [0xC7, 0x05])
            .ok_or_else(|| abi("no marker emitted"))?;
        let displacement =
            i32::from_le_bytes(code[marker + 2..marker + 6].try_into().unwrap_or_default());
        let value =
            u32::from_le_bytes(code[marker + 6..marker + 10].try_into().unwrap_or_default());
        assert_eq!(marker as i64 + 10 + displacement as i64, 0x300);
        assert_eq!(value, 1);
        Ok(())
    }

    #[test]
    fn an_insertion_shim_needs_the_builder_and_the_contract_bytes() -> Result<(), RuntimeError> {
        let code = build_dispatch_code(
            0x1000_0000,
            0x7FF0_0000,
            &[0x90; 7],
            Some(0x7FF0_1000),
            0x1000_0600,
            Some(0x1000_0400),
            Some(InsertionArgs {
                serial: 0x44,
                data: 0x2000_0000,
                manager: 0x2000_1000,
                function_address: 0x7FF0_2000,
                serial_counter_offset: 8,
            }),
            true,
        )?;
        assert!(code.len() < 0x300);
        // The three status markers are 1, 2 and 3.
        let mut seen = Vec::new();
        for (index, window) in code.windows(2).enumerate() {
            if window == [0xC7, 0x05] {
                seen.push(u32::from_le_bytes(
                    code[index + 6..index + 10].try_into().unwrap_or_default(),
                ));
            }
        }
        // The three guarded insertion steps, then the page marker the shim
        // always writes before replaying the original prologue.
        assert_eq!(seen, vec![1, 2, 3, 1]);
        Ok(())
    }

    #[test]
    fn a_source_record_only_touches_the_generation_inputs() -> Result<(), RuntimeError> {
        let template = vec![0x5Au8; SCROLL_RECORD_SIZE];
        let record = build_source_record(&template, 0x1122_3344, 4, 180, 183, 2)?;
        assert_eq!(&record[0x06..0x08], &180u16.to_le_bytes());
        assert_eq!(&record[0x08..0x0A], &180u16.to_le_bytes());
        assert_eq!(&record[0x10..0x12], &183u16.to_le_bytes());
        assert_eq!(&record[0x12..0x14], &183u16.to_le_bytes());
        assert_eq!(&record[0x20..0x24], &0x1122_3344u32.to_le_bytes());
        assert_eq!(record[0x30], 4);
        assert_eq!(record[0x31], 4);
        assert_eq!(&record[0xDC..0xE0], &2u32.to_le_bytes());
        assert_eq!(record[0x00], 0x5A);
        assert_eq!(record[0xE7], 0x5A);
        Ok(())
    }

    /// The PC v2.02 candidate is review data: every value is pinned to the
    /// report its owning lane published, and the three record/descriptor
    /// constants are shared with the shipped layout rather than restated.
    #[test]
    fn the_v2_02_candidate_layout_pins_the_lane_values() {
        let candidate = PC_V202_LIVE_ADD_CANDIDATE;
        assert_eq!(candidate.profile_id, "pc-v2.02-live-add-candidate");
        assert_eq!(candidate.dispatch_rva, 0x12E9E50);
        assert_eq!(candidate.dispatch_return_rva, 0x20BB1C);
        assert_eq!(
            candidate.dispatch_signature,
            [0x40, 0x53, 0x57, 0x48, 0x83, 0xEC, 0x38]
        );
        assert_eq!(candidate.builder_rva, 0x227FC5C);
        assert_eq!(candidate.builder_size, 0x27B);
        assert_eq!(candidate.insertion_rva, 0x54D324);
        assert_eq!(candidate.insertion_size, 0xE17);
        assert_eq!(candidate.slot_lookup_rva, 0x55308C);
        assert_eq!(candidate.manager_pointer_rva, 0x4751530);
        assert_eq!(candidate.scheduler_pointer_rva, 0x4745348);
        assert_eq!(candidate.container_offset, 0x224A60);
        assert_eq!(candidate.capacity_offset, 0x16A80);
        assert_eq!(candidate.serial_counter_offset, 8);
        assert_eq!(candidate.serial_index_offset, 0x23B5E8);
        assert_eq!(candidate.scheduler_pending_offset, 0x1408);
        assert_eq!(candidate.scheduler_ready_offset, 0x1629);
        assert_eq!(candidate.queue_begin_offset, 0x60);
        assert_eq!(candidate.queue_end_offset, 0x68);
        assert_eq!(candidate.record_size, PC_V201_LIVE_ADD.record_size);
        assert_eq!(candidate.descriptor_size, PC_V201_LIVE_ADD.descriptor_size);
        assert_eq!(candidate.capacity, PC_V201_LIVE_ADD.capacity);
        // A real relocation, not a copy of the shipped layout.
        assert_ne!(candidate.insertion_rva, PC_V201_LIVE_ADD.insertion_rva);
        assert_ne!(
            candidate.manager_pointer_rva,
            PC_V201_LIVE_ADD.manager_pointer_rva
        );
    }

    /// A candidate value may not be added without naming the lane that owns it.
    #[test]
    fn every_version_owned_candidate_field_names_its_lane() {
        let expected = [
            "dispatch_rva",
            "dispatch_return_rva",
            "dispatch_signature",
            "builder_rva",
            "builder_size",
            "insertion_rva",
            "insertion_size",
            "slot_lookup_rva",
            "manager_pointer_rva",
            "scheduler_pointer_rva",
            "container_offset",
            "capacity_offset",
            "serial_counter_offset",
            "serial_index_offset",
            "scheduler_pending_offset",
            "scheduler_ready_offset",
            "queue_begin_offset",
            "queue_end_offset",
            "inventory_global_mode",
        ];
        let mut named: Vec<&str> = PC_V202_CANDIDATE_EVIDENCE
            .iter()
            .map(|(field, _)| *field)
            .collect();
        named.sort_unstable();
        let mut expected = Vec::from(expected);
        expected.sort_unstable();
        assert_eq!(named, expected);
        for (field, lane) in PC_V202_CANDIDATE_EVIDENCE {
            assert!(!lane.is_empty(), "{field} names no owning lane");
        }
        // The authored identity of the candidate is not a version-owned field.
        assert!(!named.contains(&"profile_id"));
    }

    /// The disabled candidate leaves the product's version gates exactly as
    /// they were: `2.0.2.0` resolves nothing and the shipped layout is
    /// byte-for-byte the accepted PC v2.01 profile.
    #[test]
    fn the_v2_02_candidate_stays_unreachable_from_product_selection() {
        use crate::platform::FileVersion;

        let v2_02 = FileVersion::new(2, 0, 2, 0);
        assert_eq!(crate::profile::supported_display_version(v2_02), None);
        assert_eq!(
            crate::profile::profile_for_game_version(v2_02, std::path::Path::new(".")).err(),
            Some(RuntimeError::UnsupportedGameVersion {
                display: "2.0.2.0".to_string(),
            })
        );
        let shipped = PC_V201_LIVE_ADD;
        assert_eq!(shipped.profile_id, "pc-v2.01-live-add-r1");
        assert_eq!(shipped.dispatch_rva, 0x12E6840);
        assert_eq!(shipped.dispatch_return_rva, 0x20BB2C);
        assert_eq!(shipped.builder_rva, 0x227C4CC);
        assert_eq!(shipped.builder_size, 0x27B);
        assert_eq!(shipped.insertion_rva, 0x54D294);
        assert_eq!(shipped.insertion_size, 0xE17);
        assert_eq!(shipped.slot_lookup_rva, 0x552FBC);
        assert_eq!(shipped.manager_pointer_rva, 0x474D4E0);
        assert_eq!(shipped.scheduler_pointer_rva, 0x47412F8);
        assert_eq!(shipped.scheduler_pending_offset, 0x1408);
        assert_eq!(shipped.scheduler_ready_offset, 0x1629);
        assert_eq!(shipped.queue_begin_offset, 0x60);
        assert_eq!(shipped.queue_end_offset, 0x68);
        assert_eq!(shipped.record_size, SCROLL_RECORD_SIZE);
        assert_eq!(shipped.descriptor_size, DESCRIPTOR_SIZE);
        assert_eq!(shipped.capacity, 400);
        // The executor's only accepted version string is still PC v2.01, and
        // the candidate advertises a different identity, so selecting it would
        // take an explicit code change rather than a value change.
        assert_eq!(
            crate::mutation::live_add::LIVE_ADD_DISPLAY_VERSION,
            "PC v2.01"
        );
        assert_ne!(PC_V202_LIVE_ADD_CANDIDATE.profile_id, shipped.profile_id);
    }
}
