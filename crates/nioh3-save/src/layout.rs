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
