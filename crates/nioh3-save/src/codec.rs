//! Write-side byte codec for the fixed scroll inventory region.
//!
//! Every function here mirrors the shipped Python reference one-for-one:
//! `emaki_exchange.patch_user_checksum` / `insert_scroll_record` and
//! `nioh3_scroll_editor/savegame.py` `allocate_scroll_inventory_keys`,
//! `allocate_scroll_generation_serials`, `write_scroll_inventory_key`,
//! `write_scroll_generation_serial`, `_clear_native_free_scroll_slot` and
//! `patch_user_checksum`. The reference stays authoritative; this module exists
//! so the Rust save transaction can produce the exact same bytes without
//! re-deriving a rule.
//!
//! Nothing here touches a file. Inputs are validated decrypted save blobs and
//! records, outputs are owned buffers.

use crate::error::SaveReadError;
use crate::layout::{
    slot_offset, RECORD_FLAG_WORD_OFFSET, RECORD_INVENTORY_KEY_OFFSET, SCROLL_RECORD_BYTES,
    SCROLL_SLOT_COUNT, USER_SAVE_BYTES, USER_SAVE_MAGIC,
};

/// Body window covered by the user-save checksum, mirroring
/// `USER_CHECKSUM_BODY_START`.
pub const USER_CHECKSUM_BODY_START: usize = 0x190;
/// Exclusive end of the checksum body window (`USER_CHECKSUM_BODY_END`).
pub const USER_CHECKSUM_BODY_END: usize = 0x90_0190;
/// Seed slot read by the checksum (`USER_CHECKSUM_SEED_OFFSET`).
pub const USER_CHECKSUM_SEED_OFFSET: usize = 0x90_0190;
/// Folded checksum slot rewritten by the checksum (`USER_CHECKSUM_VALUE_OFFSET`).
pub const USER_CHECKSUM_VALUE_OFFSET: usize = 0x90_0194;
/// Byte offset of the generation serial inside one record (`+0x28`).
pub const RECORD_GENERATION_SERIAL_OFFSET: usize = 0x28;
/// Highest allocation-compatible inventory key (`SCROLL_INVENTORY_KEY_MAX`).
pub const SCROLL_INVENTORY_KEY_MAX: u32 = 0xFFFF;
/// Highest allocation-compatible generation serial (`SCROLL_GENERATION_SERIAL_MAX`).
pub const SCROLL_GENERATION_SERIAL_MAX: u32 = 0xFFFF_FFFC;
/// Post-native-insertion lifecycle word for a newly installed scroll.
///
/// Mirrors `savegame.POST_INSERTION_FLAG_WORD`. The game's own pickup path ORs
/// the insertion bits `0x04000080` into the builder's `0x02800002` descriptor
/// state, so a freshly inserted record reads `0x06800082` before any reveal or
/// view. A direct save write must reproduce that inventory state instead of
/// inheriting the donor template's lifecycle/reveal flags.
pub const POST_INSERTION_FLAG_WORD: u32 = 0x0680_0082;

fn require_save_blob(decrypted: &[u8]) -> Result<(), SaveReadError> {
    if decrypted.len() != USER_SAVE_BYTES {
        return Err(SaveReadError::SaveLength {
            expected: USER_SAVE_BYTES,
            actual: decrypted.len(),
        });
    }
    if !decrypted.starts_with(USER_SAVE_MAGIC) {
        return Err(SaveReadError::SaveMagic);
    }
    Ok(())
}

fn record_window(decrypted: &[u8], slot_index: usize) -> Result<&[u8], SaveReadError> {
    let offset = slot_offset(slot_index).ok_or(SaveReadError::SlotIndex { index: slot_index })?;
    let end = offset + SCROLL_RECORD_BYTES;
    decrypted
        .get(offset..end)
        .ok_or(SaveReadError::InventoryRegionTruncated {
            needed: end,
            actual: decrypted.len(),
        })
}

fn record_window_mut(decrypted: &mut [u8], slot_index: usize) -> Result<&mut [u8], SaveReadError> {
    let offset = slot_offset(slot_index).ok_or(SaveReadError::SlotIndex { index: slot_index })?;
    let end = offset + SCROLL_RECORD_BYTES;
    let actual = decrypted.len();
    decrypted
        .get_mut(offset..end)
        .ok_or(SaveReadError::InventoryRegionTruncated {
            needed: end,
            actual,
        })
}

/// The native free-slot state for one record: the type word at `+0x00` is zero.
///
/// Mirrors `scroll_slot_is_empty`, including its deliberate tolerance for stale
/// payload bytes behind a cleared type word after an in-game deletion.
pub fn scroll_slot_is_empty(record: &[u8]) -> Result<bool, SaveReadError> {
    let type_word = read_u16(record, 0x00)?;
    Ok(type_word == 0)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, SaveReadError> {
    let window = bytes
        .get(offset..offset + 2)
        .ok_or(SaveReadError::FieldOutOfRange { offset, width: 2 })?;
    let mut value = [0u8; 2];
    value.copy_from_slice(window);
    Ok(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, SaveReadError> {
    let window = bytes
        .get(offset..offset + 4)
        .ok_or(SaveReadError::FieldOutOfRange { offset, width: 4 })?;
    let mut value = [0u8; 4];
    value.copy_from_slice(window);
    Ok(u32::from_le_bytes(value))
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), SaveReadError> {
    let window = bytes
        .get_mut(offset..offset + 4)
        .ok_or(SaveReadError::FieldOutOfRange { offset, width: 4 })?;
    window.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

/// Fold a save body into the shipped 32-bit user checksum.
///
/// Mirrors `emaki_exchange.compute_user_checksum`: 64-bit little-endian sums of
/// each `0x400` block's eight-byte groups, accumulated with an XOR against the
/// seed, then folded by `total // 0xFFFFFFFF + (total & 0xFFFFFFFF)`.
///
/// The body length is fixed by the format; a caller cannot reach this with a
/// different window because the public wrapper slices the constant range.
pub fn compute_user_checksum(body: &[u8], seed: u32) -> u32 {
    debug_assert_eq!(body.len(), 0x90_0000);
    let mut total: u64 = 0;
    let mut base = 0usize;
    while base + 0x400 <= body.len() {
        let mut block_sum: u64 = 0;
        let mut offset = base;
        while offset + 8 <= base + 0x400 {
            let mut group = [0u8; 8];
            group.copy_from_slice(&body[offset..offset + 8]);
            block_sum = block_sum.wrapping_add(u64::from_le_bytes(group));
            offset += 8;
        }
        // The reference masks the accumulator to 64 bits after each block,
        // which a `u64` already is.
        total = total.wrapping_add(block_sum) ^ u64::from(seed);
        base += 0x400;
    }
    let folded = (total / 0xFFFF_FFFF).wrapping_add(total & 0xFFFF_FFFF);
    (folded & 0xFFFF_FFFF) as u32
}

/// Rewrite the user checksum, returning `(old, new)` like `patch_user_checksum`.
pub fn patch_user_checksum(decrypted: &mut [u8]) -> Result<(u32, u32), SaveReadError> {
    require_save_blob(decrypted)?;
    let seed = read_u32(decrypted, USER_CHECKSUM_SEED_OFFSET)?;
    let old = read_u32(decrypted, USER_CHECKSUM_VALUE_OFFSET)?;
    let body = &decrypted[USER_CHECKSUM_BODY_START..USER_CHECKSUM_BODY_END];
    let new = compute_user_checksum(body, seed);
    write_u32(decrypted, USER_CHECKSUM_VALUE_OFFSET, new)?;
    Ok((old, new))
}

/// The result of allocating one unused scroll identity value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allocation {
    pub value: u32,
}

/// Allocate `count` conservative globally unused `+0x1C` inventory keys.
///
/// Mirrors `allocate_scroll_inventory_keys`, including the "value must not
/// appear anywhere in the save as a little-endian word" filter that the shipped
/// policy applies.
pub fn allocate_scroll_inventory_keys(
    decrypted: &[u8],
    count: usize,
) -> Result<Vec<u32>, SaveReadError> {
    require_save_blob(decrypted)?;
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut used: Vec<u32> = Vec::new();
    for slot_index in 0..SCROLL_SLOT_COUNT {
        let record = record_window(decrypted, slot_index)?;
        if scroll_slot_is_empty(record)? {
            continue;
        }
        let value = read_u32(record, RECORD_INVENTORY_KEY_OFFSET)?;
        if (1..=SCROLL_INVENTORY_KEY_MAX).contains(&value) && !used.contains(&value) {
            used.push(value);
        }
    }
    if count > (SCROLL_INVENTORY_KEY_MAX as usize).saturating_sub(used.len()) {
        return Err(SaveReadError::AllocationExhausted {
            kind: "inventory key",
        });
    }
    let largest = used.iter().copied().max().unwrap_or(0);
    let start = if used.is_empty() { 1 } else { largest + 1 };
    let mut allocated: Vec<u32> = Vec::with_capacity(count);
    for step in 0..SCROLL_INVENTORY_KEY_MAX {
        let value = ((start - 1 + step) % SCROLL_INVENTORY_KEY_MAX) + 1;
        if used.contains(&value) || allocated.contains(&value) {
            continue;
        }
        if contains_u32_word(decrypted, value) {
            continue;
        }
        allocated.push(value);
        if allocated.len() == count {
            return Ok(allocated);
        }
    }
    Err(SaveReadError::AllocationExhausted {
        kind: "inventory key",
    })
}

fn contains_u32_word(haystack: &[u8], value: u32) -> bool {
    let needle = value.to_le_bytes();
    if haystack.len() < 4 {
        return false;
    }
    haystack.windows(4).any(|window| window == needle)
}

/// Return one complete record with a validated native inventory key written.
pub fn write_scroll_inventory_key(
    record: &[u8],
    inventory_key: u32,
) -> Result<[u8; SCROLL_RECORD_BYTES], SaveReadError> {
    if record.len() != SCROLL_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: SCROLL_RECORD_BYTES,
            actual: record.len(),
        });
    }
    if !(1..=SCROLL_INVENTORY_KEY_MAX).contains(&inventory_key) {
        return Err(SaveReadError::AllocationValue {
            field: "inventory key",
        });
    }
    let mut owned = [0u8; SCROLL_RECORD_BYTES];
    owned.copy_from_slice(record);
    write_u32(&mut owned, RECORD_INVENTORY_KEY_OFFSET, inventory_key)?;
    Ok(owned)
}

/// Return one complete record carrying the post-insertion lifecycle word.
///
/// Mirrors `savegame.write_post_insertion_state`. Written only by the
/// installation boundary: generation bytes, the rarity/stage-one payload and
/// every donor-unrelated field are preserved.
pub fn write_post_insertion_state(
    record: &[u8],
) -> Result<[u8; SCROLL_RECORD_BYTES], SaveReadError> {
    if record.len() != SCROLL_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: SCROLL_RECORD_BYTES,
            actual: record.len(),
        });
    }
    let mut owned = [0u8; SCROLL_RECORD_BYTES];
    owned.copy_from_slice(record);
    write_u32(
        &mut owned,
        RECORD_FLAG_WORD_OFFSET,
        POST_INSERTION_FLAG_WORD,
    )?;
    Ok(owned)
}

/// Allocate `count` save-wide unique `+0x28` generation serials.
///
/// Mirrors `allocate_scroll_generation_serials`: mapped scroll serials and the
/// captured non-scroll equipment records share one identity namespace, so both
/// are excluded from the allocation. The non-scroll predicate is the strict
/// captured-equipment header rule (`_looks_like_non_scroll_item_record`).
///
/// Refuses with [`SaveReadError::AllocationExhausted`] when the observed
/// `+0x28` words leave no legal successor inside
/// `1..=SCROLL_GENERATION_SERIAL_MAX`.
pub fn allocate_scroll_generation_serials(
    decrypted: &[u8],
    count: usize,
) -> Result<Vec<u32>, SaveReadError> {
    require_save_blob(decrypted)?;
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut scroll_serials: Vec<u32> = Vec::new();
    for slot_index in 0..SCROLL_SLOT_COUNT {
        let record = record_window(decrypted, slot_index)?;
        if scroll_slot_is_empty(record)? {
            continue;
        }
        let record_type = read_u16(record, 0x00)?;
        if crate::inventory::mapped_category(record_type).is_some() {
            scroll_serials.push(read_u32(record, RECORD_GENERATION_SERIAL_OFFSET)?);
        }
    }
    let mut occupied = scroll_serials.clone();
    for value in non_scroll_item_generation_serials(decrypted)? {
        if !occupied.contains(&value) {
            occupied.push(value);
        }
    }
    let start = serial_allocation_start(scroll_serials.iter().copied().max().unwrap_or(0))?;
    let mut allocated: Vec<u32> = Vec::with_capacity(count);
    let mut value = start;
    // `start` is already inside the legal domain, so the increment below cannot
    // reach the u32 wrap.
    while value <= SCROLL_GENERATION_SERIAL_MAX {
        if !occupied.contains(&value) && !allocated.contains(&value) {
            allocated.push(value);
            if allocated.len() == count {
                return Ok(allocated);
            }
        }
        value += 1;
    }
    Err(SaveReadError::AllocationExhausted {
        kind: "generation serial",
    })
}

/// `start = largest observed serial + 1`, validated against the serial domain.
///
/// The record scan above copies `+0x28` without filtering it, so the largest
/// observation is an untrusted word: an empty namespace must still start at 1,
/// and only this helper knows where the namespace ends. `checked_add` is what
/// keeps a `u32::MAX` observation from wrapping to zero and handing the caller
/// a serial outside `1..=SCROLL_GENERATION_SERIAL_MAX`; the explicit ceiling
/// then refuses every value whose successor is already illegal. `largest` is
/// unsigned, so `checked_add` succeeding already establishes `start >= 1`.
fn serial_allocation_start(largest_serial: u32) -> Result<u32, SaveReadError> {
    let start = largest_serial
        .checked_add(1)
        .ok_or(SaveReadError::AllocationExhausted {
            kind: "generation serial",
        })?;
    if start > SCROLL_GENERATION_SERIAL_MAX {
        return Err(SaveReadError::AllocationExhausted {
            kind: "generation serial",
        });
    }
    Ok(start)
}

/// The strict captured-equipment header predicate used by the serial allocator.
fn looks_like_non_scroll_item_record(
    decrypted: &[u8],
    record_offset: usize,
) -> Result<bool, SaveReadError> {
    if record_offset + 0x2C > decrypted.len() {
        return Ok(false);
    }
    let record_type = read_u16(decrypted, record_offset)?;
    let mirrored_type = read_u16(decrypted, record_offset + 0x02)?;
    let item_count = read_u16(decrypted, record_offset + 0x04)?;
    let level = read_u16(decrypted, record_offset + 0x06)?;
    let mirrored_level = read_u16(decrypted, record_offset + 0x08)?;
    Ok(record_type != 0
        && record_type == mirrored_type
        && crate::inventory::mapped_category(record_type).is_none()
        && item_count == 1
        && level == mirrored_level)
}

/// Captured non-scroll item serials stored at record `+0x28`.
fn non_scroll_item_generation_serials(decrypted: &[u8]) -> Result<Vec<u32>, SaveReadError> {
    let mut serials: Vec<u32> = Vec::new();
    let mut search_offset = 4usize;
    while search_offset + 2 <= decrypted.len() {
        let Some(relative) = find_pair(&decrypted[search_offset..], 0x0001) else {
            break;
        };
        let count_offset = search_offset + relative;
        if count_offset < 4 {
            search_offset = count_offset + 1;
            continue;
        }
        let record_offset = count_offset - 4;
        if looks_like_non_scroll_item_record(decrypted, record_offset)?
            && record_offset + RECORD_GENERATION_SERIAL_OFFSET + 4 <= decrypted.len()
        {
            let value = read_u32(decrypted, record_offset + RECORD_GENERATION_SERIAL_OFFSET)?;
            if (1..=SCROLL_GENERATION_SERIAL_MAX).contains(&value) {
                serials.push(value);
            }
        }
        search_offset = count_offset + 1;
    }
    Ok(serials)
}

/// First window equal to the little-endian encoding of `value`.
fn find_pair(haystack: &[u8], value: u16) -> Option<usize> {
    let needle = value.to_le_bytes();
    haystack.windows(2).position(|window| window == needle)
}

/// Return one complete record with a validated generation serial written.
pub fn write_scroll_generation_serial(
    record: &[u8],
    generation_serial: u32,
) -> Result<[u8; SCROLL_RECORD_BYTES], SaveReadError> {
    if record.len() != SCROLL_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: SCROLL_RECORD_BYTES,
            actual: record.len(),
        });
    }
    if !(1..=SCROLL_GENERATION_SERIAL_MAX).contains(&generation_serial) {
        return Err(SaveReadError::AllocationValue {
            field: "generation serial",
        });
    }
    let mut owned = [0u8; SCROLL_RECORD_BYTES];
    owned.copy_from_slice(record);
    write_u32(
        &mut owned,
        RECORD_GENERATION_SERIAL_OFFSET,
        generation_serial,
    )?;
    Ok(owned)
}

/// Overwrite the transfer-count field at `+0xDC`, mirroring
/// `prepare_candidate_for_install`.
pub fn prepare_candidate_for_install(
    record: &[u8],
    transfer_count: u32,
) -> Result<[u8; SCROLL_RECORD_BYTES], SaveReadError> {
    if record.len() != SCROLL_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: SCROLL_RECORD_BYTES,
            actual: record.len(),
        });
    }
    let mut owned = [0u8; SCROLL_RECORD_BYTES];
    owned.copy_from_slice(record);
    write_u32(&mut owned, 0xDC, transfer_count)?;
    Ok(owned)
}

/// Normalize one already-verified native free slot to all zero bytes.
///
/// Mirrors `_clear_native_free_scroll_slot`: a normal in-game delete clears the
/// type word but can leave stale payload bytes, and the strict insert helper
/// only accepts an all-zero destination. A slot whose type word is set is
/// refused, so this can never erase an occupied record.
pub fn clear_native_free_scroll_slot(
    decrypted: &[u8],
    slot_index: usize,
) -> Result<Vec<u8>, SaveReadError> {
    let record = record_window(decrypted, slot_index)?;
    if !scroll_slot_is_empty(record)? {
        return Err(SaveReadError::SlotOccupied { slot_index });
    }
    if record.iter().all(|byte| *byte == 0) {
        return Ok(decrypted.to_vec());
    }
    let mut edited = decrypted.to_vec();
    let window = record_window_mut(&mut edited, slot_index)?;
    window.fill(0);
    Ok(edited)
}

/// Insert one complete record into a fully zeroed fixed inventory slot.
///
/// Mirrors `emaki_exchange.insert_scroll_record`, including the type-word and
/// all-zero-destination gates and the checksum rewrite. The returned report
/// carries the same offsets and checksum pair as the reference.
pub fn insert_scroll_record(
    decrypted: &[u8],
    slot_index: usize,
    record: &[u8],
) -> Result<(Vec<u8>, InsertReport), SaveReadError> {
    require_save_blob(decrypted)?;
    if record.len() != SCROLL_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: SCROLL_RECORD_BYTES,
            actual: record.len(),
        });
    }
    if read_u16(record, 0x00)? == 0 {
        return Err(SaveReadError::RecordTypeZero);
    }
    let offset = slot_offset(slot_index).ok_or(SaveReadError::SlotIndex { index: slot_index })?;
    let existing = record_window(decrypted, slot_index)?;
    if existing.iter().any(|byte| *byte != 0) {
        return Err(SaveReadError::SlotNotZeroed { slot_index });
    }
    let mut edited = decrypted.to_vec();
    let window = record_window_mut(&mut edited, slot_index)?;
    window.copy_from_slice(record);
    let (old_checksum, new_checksum) = patch_user_checksum(&mut edited)?;
    Ok((
        edited,
        InsertReport {
            slot_index,
            record_offset: offset,
            old_checksum,
            new_checksum,
        },
    ))
}

/// The offsets and checksum transition of one insert, mirroring the reference
/// report fields the transaction journals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InsertReport {
    pub slot_index: usize,
    pub record_offset: usize,
    pub old_checksum: u32,
    pub new_checksum: u32,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn save_with(records: &[(usize, [u8; SCROLL_RECORD_BYTES])]) -> Vec<u8> {
        let mut save = vec![0u8; USER_SAVE_BYTES];
        save[..6].copy_from_slice(USER_SAVE_MAGIC);
        for (slot_index, record) in records {
            let offset = slot_offset(*slot_index).expect("in range");
            save[offset..offset + SCROLL_RECORD_BYTES].copy_from_slice(record);
        }
        save
    }

    fn record(record_type: u16, key: u32, serial: u32) -> [u8; SCROLL_RECORD_BYTES] {
        let mut value = [0u8; SCROLL_RECORD_BYTES];
        value[..2].copy_from_slice(&record_type.to_le_bytes());
        value[RECORD_INVENTORY_KEY_OFFSET..RECORD_INVENTORY_KEY_OFFSET + 4]
            .copy_from_slice(&key.to_le_bytes());
        value[RECORD_GENERATION_SERIAL_OFFSET..RECORD_GENERATION_SERIAL_OFFSET + 4]
            .copy_from_slice(&serial.to_le_bytes());
        value
    }

    #[test]
    fn checksum_is_stable_for_a_zero_body() {
        let mut save = save_with(&[]);
        let (old, new) = patch_user_checksum(&mut save).expect("checksum");
        assert_eq!(old, 0);
        // A zero body sums to zero, so the folded value is zero.
        assert_eq!(new, 0);
    }

    /// `write_post_insertion_state` rewrites one word and nothing else.
    #[test]
    fn post_insertion_state_preserves_every_other_byte() {
        let mut candidate = record(0xE604, 5, 6);
        for (index, byte) in candidate.iter_mut().enumerate() {
            *byte = byte.wrapping_add((index % 97) as u8);
        }
        candidate[RECORD_FLAG_WORD_OFFSET..RECORD_FLAG_WORD_OFFSET + 4]
            .copy_from_slice(&0x0F80_0080u32.to_le_bytes());

        let written = write_post_insertion_state(&candidate).expect("state");
        assert_eq!(
            &written[RECORD_FLAG_WORD_OFFSET..RECORD_FLAG_WORD_OFFSET + 4],
            &POST_INSERTION_FLAG_WORD.to_le_bytes()
        );
        assert_eq!(
            &written[..RECORD_FLAG_WORD_OFFSET],
            &candidate[..RECORD_FLAG_WORD_OFFSET]
        );
        assert_eq!(
            &written[RECORD_FLAG_WORD_OFFSET + 4..],
            &candidate[RECORD_FLAG_WORD_OFFSET + 4..]
        );
        assert!(write_post_insertion_state(&candidate[..SCROLL_RECORD_BYTES - 1]).is_err());
    }

    #[test]
    fn checksum_matches_the_reference_fold_and_survives_a_body_change() {
        // A body with a known nonzero word gives the fold something to work on,
        // so the value is not trivially zero.
        let mut save = save_with(&[]);
        save[USER_CHECKSUM_SEED_OFFSET..USER_CHECKSUM_SEED_OFFSET + 4]
            .copy_from_slice(&0x1234_5678u32.to_le_bytes());
        let body_start = USER_CHECKSUM_BODY_START;
        save[body_start..body_start + 8].copy_from_slice(&1u64.to_le_bytes());
        let (_, first) = patch_user_checksum(&mut save).expect("checksum");
        assert_ne!(first, 0, "a non-zero body must not fold to zero");

        // The stored value must at all times be the fold of the current body.
        let seed = u32::from_le_bytes(
            save[USER_CHECKSUM_SEED_OFFSET..USER_CHECKSUM_SEED_OFFSET + 4]
                .try_into()
                .expect("four bytes"),
        );
        let expected = compute_user_checksum(
            &save[USER_CHECKSUM_BODY_START..USER_CHECKSUM_BODY_END],
            seed,
        );
        assert_eq!(first, expected);
        assert_eq!(
            u32::from_le_bytes(
                save[USER_CHECKSUM_VALUE_OFFSET..USER_CHECKSUM_VALUE_OFFSET + 4]
                    .try_into()
                    .expect("four bytes")
            ),
            expected,
            "the stored field must hold the derived value"
        );

        // A second, different body must produce a different folded value, so a
        // hard-coded or ignored checksum fails.
        save[body_start..body_start + 8].copy_from_slice(&7u64.to_le_bytes());
        let (old, second) = patch_user_checksum(&mut save).expect("checksum");
        assert_eq!(old, first, "the previous stored value is reported");
        assert_ne!(second, first, "changing the body must change the checksum");
    }

    #[test]
    fn inventory_keys_start_after_the_largest_existing_key() {
        let save = save_with(&[(0, record(0xE604, 7, 11)), (1, record(0xE604, 9, 12))]);
        let allocated = allocate_scroll_inventory_keys(&save, 1).expect("allocation");
        assert_eq!(allocated.len(), 1);
        assert!(allocated[0] > 9, "must start after the largest key");
        assert!(![7u32, 9].contains(&allocated[0]));
    }

    #[test]
    fn generation_serials_skip_reserved_scroll_values() {
        let save = save_with(&[(0, record(0xE604, 7, 11)), (1, record(0xE604, 9, 12))]);
        let allocated = allocate_scroll_generation_serials(&save, 2).expect("allocation");
        assert_eq!(allocated, vec![13, 14]);
    }

    #[test]
    fn generation_serials_allocate_the_maximum_legal_serial() {
        // The largest observed serial leaves exactly one legal successor, so the
        // domain ceiling itself must be allocatable rather than refused.
        let save = save_with(&[(0, record(0xE604, 7, SCROLL_GENERATION_SERIAL_MAX - 1))]);
        let allocated = allocate_scroll_generation_serials(&save, 1).expect("allocation");
        assert_eq!(allocated, vec![SCROLL_GENERATION_SERIAL_MAX]);
        assert!(
            write_scroll_generation_serial(&record(0xE604, 7, 1), allocated[0]).is_ok(),
            "the allocator's ceiling must be a serial the writer accepts"
        );

        // One more value would step past the ceiling, so the loop terminates by
        // refusing instead of incrementing out of the domain.
        let error = allocate_scroll_generation_serials(&save, 2).expect_err("ceiling");
        assert!(matches!(
            error,
            SaveReadError::AllocationExhausted {
                kind: "generation serial"
            }
        ));
    }

    #[test]
    fn generation_serial_allocation_refuses_a_u32_max_observation() {
        // `+0x28` is read as a raw word, so a corrupt or reserved record can
        // hold `u32::MAX`. `max + 1` would wrap to zero and hand the caller a
        // serial below the legal domain; the start must refuse instead, and it
        // must refuse before anything observable changes.
        let save = save_with(&[(0, record(0xE604, 7, u32::MAX))]);
        let before = save.clone();
        let error = allocate_scroll_generation_serials(&save, 1).expect_err("wrapped start");
        assert!(matches!(
            error,
            SaveReadError::AllocationExhausted {
                kind: "generation serial"
            }
        ));
        assert_eq!(
            save, before,
            "a refused allocation must not mutate its input"
        );
    }

    #[test]
    fn serial_allocation_start_refuses_every_value_without_a_legal_successor() {
        assert_eq!(serial_allocation_start(0).expect("empty namespace"), 1);
        assert_eq!(
            serial_allocation_start(SCROLL_GENERATION_SERIAL_MAX - 1).expect("successor"),
            SCROLL_GENERATION_SERIAL_MAX
        );
        for largest_serial in [
            SCROLL_GENERATION_SERIAL_MAX,
            SCROLL_GENERATION_SERIAL_MAX + 1,
            u32::MAX - 1,
            u32::MAX,
        ] {
            let error = serial_allocation_start(largest_serial).expect_err("no successor");
            assert!(
                matches!(
                    error,
                    SaveReadError::AllocationExhausted {
                        kind: "generation serial"
                    }
                ),
                "{largest_serial:#010X} must refuse"
            );
        }
    }

    #[test]
    fn insert_refuses_an_occupied_slot() {
        let save = save_with(&[(0, record(0xE604, 7, 11))]);
        let error = insert_scroll_record(&save, 0, &record(0xE604, 8, 12)).expect_err("occupied");
        assert!(matches!(error, SaveReadError::SlotNotZeroed { .. }));
    }

    #[test]
    fn insert_refuses_a_zero_type_record() {
        let save = save_with(&[]);
        let error = insert_scroll_record(&save, 0, &record(0x0000, 1, 1)).expect_err("zero type");
        assert!(matches!(error, SaveReadError::RecordTypeZero));
    }

    #[test]
    fn a_stale_free_slot_is_cleared_then_accepted() {
        let mut stale = record(0x0000, 0, 0);
        stale[0x40] = 0xAA;
        let save = save_with(&[(3, stale)]);
        let cleared = clear_native_free_scroll_slot(&save, 3).expect("clear");
        let offset = slot_offset(3).expect("in range");
        assert!(cleared[offset..offset + SCROLL_RECORD_BYTES]
            .iter()
            .all(|byte| *byte == 0));
        let inserted = insert_scroll_record(&cleared, 3, &record(0xE604, 5, 20)).expect("insert");
        assert_eq!(
            u16::from_le_bytes([inserted.0[offset], inserted.0[offset + 1]]),
            0xE604
        );
    }

    #[test]
    fn clear_refuses_an_occupied_slot() {
        let save = save_with(&[(2, record(0xE604, 7, 11))]);
        let error = clear_native_free_scroll_slot(&save, 2).expect_err("occupied");
        assert!(matches!(error, SaveReadError::SlotOccupied { .. }));
    }
}
