//! Serialize a canonical installation record into the native assembly input.
//!
//! Port of `live_add_descriptor.py`. It copies inputs and never reimplements RNG
//! or finalization: the isolated native preview must still match the installation
//! record before a serial is allocated.

use crate::error::RuntimeError;
use serde_json::Value;

/// `SCROLL_RECORD_SIZE`.
pub const RECORD_SIZE: usize = 0xE8;
/// `live_add_descriptor.assembly_descriptor` output size.
pub const DESCRIPTOR_SIZE: usize = 0xCC;
/// The builder metadata `new_assembly_record` installs: `0x02800002`, `0`.
pub const ASSEMBLY_FLAGS: u32 = 0x0280_0002;

fn u16_at(raw: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([raw[offset], raw[offset + 1]])
}

fn u32_at(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        raw[offset],
        raw[offset + 1],
        raw[offset + 2],
        raw[offset + 3],
    ])
}

fn rejected(detail: &str) -> RuntimeError {
    RuntimeError::CandidateRejected {
        detail: detail.to_string(),
    }
}

/// Port of `new_assembly_record`.
///
/// PC v2.01 builder metadata, never a template's inventory state. Acquisition
/// order and owned/seen/equipped flags are not descriptor inputs; native
/// insertion assigns their destination values.
pub fn new_assembly_record(record: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    assembly_descriptor(record, false)?;
    let mut result = record.to_vec();
    result[0x18..0x1C].copy_from_slice(&ASSEMBLY_FLAGS.to_le_bytes());
    result[0x1C..0x20].copy_from_slice(&0u32.to_le_bytes());
    Ok(result)
}

/// Record flag bit 25, which the PC v2.02 builder sets only when the effective
/// descriptor identity equals the game's ambient identity.
pub const BUILDER_OWN_IDENTITY_FLAG: u32 = 0x0200_0000;

/// The plan field carrying the ambient identity the executor read, as a
/// decimal string (the value is a 64-bit account id).
pub const BUILDER_AMBIENT_IDENTITY_FIELD: &str = "builder_ambient_identity";

/// Port of `new_assembly_record_for_ambient`: the builder metadata one
/// descriptor produces against one ambient identity.
///
/// The builder takes `J` from descriptor `+0x18` (falling back to `A` only
/// when `J == 0` and descriptor `+0x20` is clear) and sets bit 25 exactly when
/// `J == A`; every other metadata bit is the reviewed [`ASSEMBLY_FLAGS`].
pub fn new_assembly_record_for_ambient(
    record: &[u8],
    ambient: u64,
) -> Result<Vec<u8>, RuntimeError> {
    let descriptor = assembly_descriptor(record, false)?;
    let mut identity = u64::from_le_bytes(
        descriptor[0x18..0x20]
            .try_into()
            .map_err(|_| rejected("Expected one canonical scroll installation record"))?,
    );
    if identity == 0 && descriptor[0x20] == 0 {
        identity = ambient;
    }
    let flags = if identity == ambient {
        ASSEMBLY_FLAGS
    } else {
        ASSEMBLY_FLAGS & !BUILDER_OWN_IDENTITY_FLAG
    };
    let mut result = new_assembly_record(record)?;
    result[0x18..0x1C].copy_from_slice(&flags.to_le_bytes());
    Ok(result)
}

/// Port of `assembly_record_in_context`: the record the native builder is
/// expected to produce for this inspected process.
///
/// A plan without [`BUILDER_AMBIENT_IDENTITY_FIELD`] comes from a layout whose
/// builder has no reviewed identity chain and keeps the fixed metadata. A
/// present field must be a decimal `u64`; anything else is refused.
pub fn assembly_record_in_context(record: &[u8], context: &Value) -> Result<Vec<u8>, RuntimeError> {
    match context.get(BUILDER_AMBIENT_IDENTITY_FIELD) {
        None => new_assembly_record(record),
        Some(value) => {
            let ambient = value
                .as_str()
                .filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
                .and_then(|text| text.parse::<u64>().ok())
                .ok_or_else(|| rejected("Prepared live-add plan expired; prepare a new plan"))?;
            new_assembly_record_for_ambient(record, ambient)
        }
    }
}

/// Port of `assembly_descriptor`.
pub fn assembly_descriptor(record: &[u8], allocate_serial: bool) -> Result<Vec<u8>, RuntimeError> {
    if record.len() != RECORD_SIZE {
        return Err(rejected(
            "Expected one canonical scroll installation record",
        ));
    }
    let record_type = u16_at(record, 0);
    let seed = u32_at(record, 0x20);
    let flags = u32_at(record, 0x18);
    if !matches!(record_type, 0x1E82 | 0x516D | 0xE604) || seed == 0 || flags & 0x80_0000 == 0 {
        return Err(rejected(
            "Live assembly requires a seeded nonstackable NG1-NG3 scroll",
        ));
    }
    if !(3..=5).contains(&record[0x30]) {
        return Err(rejected("Unsupported assembly rarity"));
    }
    let mut descriptor = vec![0u8; DESCRIPTOR_SIZE];
    descriptor[0..2].copy_from_slice(&record_type.to_le_bytes());
    descriptor[4..8].copy_from_slice(&(u16_at(record, 6) as u32).to_le_bytes());
    descriptor[8..12].copy_from_slice(&(u16_at(record, 0x10) as u32).to_le_bytes());
    descriptor[0x0C] = record[0x30];
    descriptor[0x10..0x14].copy_from_slice(&record[0x20..0x24]);
    descriptor[0x14..0x18].copy_from_slice(&record[0xDC..0xE0]);
    let identity = u32_at(record, 0x14) as u64
        | ((u16_at(record, 4) as u64) << 32)
        | ((u16_at(record, 2) as u64) << 48);
    descriptor[0x18..0x20].copy_from_slice(&identity.to_le_bytes());
    descriptor[0x21] = if allocate_serial { 0 } else { 1 };
    descriptor[0x22] = record[0x0F];
    descriptor[0x24..DESCRIPTOR_SIZE].copy_from_slice(&record[0x34..0xDC]);
    Ok(descriptor)
}

/// Port of `verify_assembly_preview`.
///
/// The preview must not allocate or reuse an instance serial, and the native
/// builder may not change the generated content of the installation record.
pub fn verify_assembly_preview(expected: &[u8], actual: &[u8]) -> Result<(), RuntimeError> {
    let failed = |detail: &str| {
        Err(RuntimeError::LiveAddVerification {
            detail: detail.to_string(),
        })
    };
    if expected.len() != RECORD_SIZE || actual.len() != RECORD_SIZE {
        return failed("Partial native assembly preview");
    }
    if actual[0x28..0x30] != [0xFF; 8] {
        return failed("Preview allocated or reused an instance serial");
    }
    if expected[..0x24] != actual[..0x24] || expected[0x30..0xE4] != actual[0x30..0xE4] {
        return failed("Native assembly differs from the expected installation record");
    }
    Ok(())
}
