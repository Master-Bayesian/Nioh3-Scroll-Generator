//! Fixed byte layout of the decrypted user save and one scroll record.
//!
//! Values are copied from `emaki_exchange.py` (`SCROLL_RECORD_SIZE`,
//! `USER_SAVE_SIZE`, `CATEGORY_TO_TYPE`) and
//! `nioh3_scroll_editor/savegame.py` (`SCROLL_GROUP_OFFSET`,
//! `SCROLL_SLOT_COUNT`). The record-internal layout lives in `nioh3-domain`.

/// Total bytes of one decrypted Nioh 3 user save (`RNNUSR` blob).
pub const USER_SAVE_BYTES: usize = 0x90_01B0;
/// Magic every decrypted user save starts with.
pub const USER_SAVE_MAGIC: &[u8; 6] = b"RNNUSR";
/// Total bytes of one scroll record (`0xE8`).
pub const SCROLL_RECORD_BYTES: usize = 0xE8;
/// Byte offset of the first inventory slot inside the decrypted save.
pub const SCROLL_GROUP_OFFSET: usize = 0x17_6CCE;
/// Fixed number of inventory slots in the scroll region.
pub const SCROLL_SLOT_COUNT: usize = 400;
/// Native record type per mapped category; index 0 is the empty category.
pub const CATEGORY_TO_TYPE: [u16; 6] = [0x0000, 0x1E82, 0x516D, 0xE604, 0xDD82, 0xD523];
/// The category whose record carries the verified current-NG3 Grace context.
pub const TEMPLATE_CATEGORY: u8 = 3;
/// Record type of [`TEMPLATE_CATEGORY`] (`0xE604`).
pub const TEMPLATE_RECORD_TYPE: u16 = 0xE604;

/// Byte offset of the inventory key inside one record (`+0x1C`).
pub const RECORD_INVENTORY_KEY_OFFSET: usize = 0x1C;

/// Byte offset of the little-endian lifecycle flag word inside one record
/// (`+0x18`..`+0x1B`): new-item marker, insertion bits, reveal state.
pub const RECORD_FLAG_WORD_OFFSET: usize = 0x18;

/// Flag-word bit of the game's protection (lock): a protected item cannot be
/// sold or discarded. Scrolls and equipment share it (PC v2.02, 2026-10-11
/// live capture: toggling protection in game changed only this bit, in memory
/// at once and in the save on the next shrine rest).
pub const RECORD_LOCK_FLAG: u32 = 0x0000_0004;

/// Flag-word bit of the game's 喜好项目 (favourites). Favouriting in game also
/// sets [`RECORD_LOCK_FLAG`]; removing the favourite keeps the lock, and the
/// lock can be removed while the favourite stays (same capture).
pub const RECORD_FAVORITE_FLAG: u32 = 0x0002_0000;

/// The protection and favourite marks of one scroll or equipment record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RecordMarks {
    pub locked: bool,
    pub favorite: bool,
}

/// Set or clear the marks in a record's flag word, leaving every other bit.
///
/// Mirrors what the game writes: only the two bits change. A caller that
/// favourites an item should also lock it, as the game does.
pub fn set_record_marks(
    record: &mut [u8],
    locked: Option<bool>,
    favorite: Option<bool>,
) -> Option<()> {
    let word = record.get_mut(RECORD_FLAG_WORD_OFFSET..RECORD_FLAG_WORD_OFFSET + 4)?;
    let mut flags = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
    for (bit, wanted) in [(RECORD_LOCK_FLAG, locked), (RECORD_FAVORITE_FLAG, favorite)] {
        match wanted {
            Some(true) => flags |= bit,
            Some(false) => flags &= !bit,
            None => {}
        }
    }
    word.copy_from_slice(&flags.to_le_bytes());
    Some(())
}

/// Read the marks from a record's flag word; `None` when the record is too short.
pub fn record_marks(record: &[u8]) -> Option<RecordMarks> {
    let word = record.get(RECORD_FLAG_WORD_OFFSET..RECORD_FLAG_WORD_OFFSET + 4)?;
    let flags = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
    Some(RecordMarks {
        locked: flags & RECORD_LOCK_FLAG != 0,
        favorite: flags & RECORD_FAVORITE_FLAG != 0,
    })
}

/// Byte offset of one inventory slot inside the decrypted save.
///
/// Returns `None` when the index is outside `0..SCROLL_SLOT_COUNT`, so a caller
/// cannot compute an out-of-bounds window from untrusted input.
pub const fn slot_offset(slot_index: usize) -> Option<usize> {
    if slot_index >= SCROLL_SLOT_COUNT {
        return None;
    }
    Some(SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_BYTES)
}

/// End offset of the whole fixed inventory region, exclusive.
pub const SCROLL_REGION_END: usize = SCROLL_GROUP_OFFSET + SCROLL_SLOT_COUNT * SCROLL_RECORD_BYTES;
