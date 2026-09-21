//! Product-level record transforms layered on the write codec.
//!
//! These are the Rust equivalents of the shipped Python operations in
//! `save_application.prepare_edit` / `prepare_delete` / `prepare_install` /
//! `prepare_install_many` composed with `SaveInstaller.edit_many` /
//! `install_many`. The shape of the operation is preserved exactly:
//!
//! - an edit replaces existing occupied records byte-for-byte after the same
//!   "original record must still match" gate,
//! - a delete clears occupied records in place without compacting the array,
//! - an install searches for the next free native slot, allocates an inventory
//!   key and a save-wide generation serial, clears a stale free slot and inserts
//!   the prepared record,
//! - every mutation rewrites the user checksum through the same folded rule.
//!
//! The transforms never touch a file. [`SaveTransformHost`] decrypts exactly
//! once from a caller-supplied container and returns owned plaintext bytes that
//! the transaction layer is responsible for staging, replacing and verifying.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::codec::{
    allocate_scroll_generation_serials, allocate_scroll_inventory_keys,
    clear_native_free_scroll_slot, insert_scroll_record, patch_user_checksum,
    prepare_candidate_for_install, scroll_slot_is_empty, write_post_insertion_state,
    write_scroll_generation_serial, write_scroll_inventory_key, SCROLL_GENERATION_SERIAL_MAX,
};
use crate::crypto;
use crate::error::SaveReadError;
use crate::inventory::mapped_category;
use crate::layout::{CATEGORY_TO_TYPE, SCROLL_RECORD_BYTES, USER_SAVE_BYTES, USER_SAVE_MAGIC};
use crate::save::{sha256_hex, DecryptedSave};

const EFFECT_SLOT_BASE: usize = 0x34;
const EFFECT_SLOT_STRIDE: usize = 0x18;
const EFFECT_SLOT_COUNT: usize = 7;

/// One header patch, mirroring `patch_local_scroll_header`'s arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderPatch {
    pub playthrough: u8,
    pub level: u16,
    pub recommended_level: u16,
    pub seed: u32,
    pub rarity: u8,
    pub transfer_count: u32,
}

/// One local effect-slot patch, mirroring `LocalEffectEdit`.
///
/// `None` leaves the field untouched, so an all-`None` edit changes nothing and
/// the transform records no changed byte for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EffectPatch {
    pub slot_index: usize,
    pub prefix: Option<u32>,
    pub effect_id: Option<u32>,
    pub value: Option<u32>,
    pub metadata: Option<u32>,
    pub tail_0: Option<u32>,
    pub tail_1: Option<u32>,
}

/// One replacement of an occupied slot, gated on the original record bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotEdit {
    pub slot_index: usize,
    #[serde(with = "bytes_232")]
    pub expected_original: [u8; SCROLL_RECORD_BYTES],
    #[serde(with = "bytes_232")]
    pub replacement: [u8; SCROLL_RECORD_BYTES],
}

/// One prepared candidate install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRequest {
    #[serde(with = "bytes_232")]
    pub candidate_record: [u8; SCROLL_RECORD_BYTES],
    pub transfer_count: u32,
}

/// Serde support for the fixed `0xE8` record buffers carried by a stored plan.
mod bytes_232 {
    use serde::de::{Error, SeqAccess, Visitor};
    use serde::ser::SerializeTuple;
    use serde::{Deserializer, Serializer};
    use std::fmt;

    /// Records are serialized as a byte tuple, so a stored plan stays readable
    /// JSON and a truncated or oversized array fails to load.
    pub fn serialize<S: Serializer>(value: &[u8; 0xE8], serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(0xE8)?;
        for byte in value {
            tuple.serialize_element(byte)?;
        }
        tuple.end()
    }

    struct BytesVisitor;

    impl<'de> Visitor<'de> for BytesVisitor {
        type Value = [u8; 0xE8];

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a 232-byte scroll record array")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut output = [0u8; 0xE8];
            for (index, slot) in output.iter_mut().enumerate() {
                *slot = seq
                    .next_element()?
                    .ok_or_else(|| Error::invalid_length(index, &self))?;
            }
            Ok(output)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 0xE8], D::Error> {
        deserializer.deserialize_tuple(0xE8, BytesVisitor)
    }
}

/// What one prepared product write contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWrite {
    /// Exact decrypted bytes the commit must install.
    pub plaintext: Vec<u8>,
    /// Digest of [`Self::plaintext`], for the receipt and the readback gate.
    pub plaintext_sha256: String,
    /// Slots the operation touched, in operation order.
    pub slot_indices: Vec<usize>,
    /// Old and new folded user checksum.
    pub checksum: (u32, u32),
    /// Identity values written into the installed records, in slot order. Empty
    /// for the edit/delete paths, which never re-identify a record.
    pub inventory_keys: Vec<u32>,
    /// Generation serials written into the installed records, in slot order.
    pub generation_serials: Vec<u32>,
    /// The records as installed, with their identity fields written.
    pub installed_records: Vec<[u8; SCROLL_RECORD_BYTES]>,
}

impl PlannedWrite {
    /// SHA-256 of the container this write will encrypt to.
    pub fn container_sha256(&self) -> Result<String, SaveReadError> {
        Ok(sha256_hex(&crypto::encrypt_container(&self.plaintext)?))
    }
}

/// The product transform host: one selected `SAVEDATA.BIN` and its account/slot
/// binding, mirroring `SaveApplication.register` plus `SaveInstaller`.
pub struct SaveTransformHost {
    save_path: PathBuf,
    account_id: u64,
    save_slot: usize,
    plaintext: Vec<u8>,
}

impl SaveTransformHost {
    /// Register one save path and decrypt it once.
    ///
    /// The account and slot identity are derived from the path with the same
    /// rules as the reader, so a caller cannot hand the host a foreign identity.
    pub fn register(save_path: &PathBuf) -> Result<Self, SaveReadError> {
        let account_id = crate::paths::account_id_from_save_path(save_path)?;
        let save_slot = usize::from(crate::paths::save_slot_index_from_path(save_path)?);
        let container = std::fs::read(save_path).map_err(|error| SaveReadError::Io {
            path: save_path.display().to_string(),
            message: error.to_string(),
        })?;
        let decrypted = DecryptedSave::from_container(&container)?;
        Ok(Self {
            save_path: save_path.clone(),
            account_id,
            save_slot,
            plaintext: decrypted.as_bytes().to_vec(),
        })
    }

    pub fn save_path(&self) -> &PathBuf {
        &self.save_path
    }

    pub fn account_id(&self) -> u64 {
        self.account_id
    }

    pub fn save_slot(&self) -> usize {
        self.save_slot
    }

    /// The decrypted bytes this host currently holds.
    pub fn plaintext(&self) -> &[u8] {
        &self.plaintext
    }

    fn require_slot(&self, slot_index: usize) -> Result<&[u8], SaveReadError> {
        let offset = crate::layout::slot_offset(slot_index)
            .ok_or(SaveReadError::SlotIndex { index: slot_index })?;
        self.plaintext
            .get(offset..offset + SCROLL_RECORD_BYTES)
            .ok_or(SaveReadError::InventoryRegionTruncated {
                needed: offset + SCROLL_RECORD_BYTES,
                actual: self.plaintext.len(),
            })
    }

    /// Replace existing occupied records under the exact-original gate.
    ///
    /// Mirrors `edit_many`: duplicates are refused, a slot must be occupied, and
    /// an occupied original must not be all zero.
    pub fn edit(&self, edits: &[SlotEdit]) -> Result<PlannedWrite, SaveReadError> {
        if edits.is_empty() {
            return Err(SaveReadError::InvalidTransform {
                message: "at least one slot edit is required".to_string(),
            });
        }
        let mut seen: Vec<usize> = Vec::new();
        for edit in edits {
            if seen.contains(&edit.slot_index) {
                return Err(SaveReadError::InvalidTransform {
                    message: format!(
                        "slot {} cannot be edited twice in one plan",
                        edit.slot_index
                    ),
                });
            }
            seen.push(edit.slot_index);
            let current = self.require_slot(edit.slot_index)?;
            if current.iter().all(|byte| *byte == 0) {
                return Err(SaveReadError::InvalidTransform {
                    message: format!("slot {} holds no record to replace", edit.slot_index),
                });
            }
            if current != edit.expected_original {
                return Err(SaveReadError::IntegrityMismatch {
                    path: format!("slot {} record", edit.slot_index),
                    expected: sha256_hex(&edit.expected_original),
                    actual: sha256_hex(current),
                });
            }
            if edit.replacement.iter().any(|byte| *byte != 0)
                && u16::from_le_bytes([edit.replacement[0], edit.replacement[1]]) == 0
            {
                return Err(SaveReadError::RecordTypeZero);
            }
        }
        let mut edited = self.plaintext.clone();
        let mut slots = Vec::with_capacity(edits.len());
        for edit in edits {
            let offset =
                crate::layout::slot_offset(edit.slot_index).ok_or(SaveReadError::SlotIndex {
                    index: edit.slot_index,
                })?;
            edited[offset..offset + SCROLL_RECORD_BYTES].copy_from_slice(&edit.replacement);
            slots.push(edit.slot_index);
        }
        let checksum = patch_user_checksum(&mut edited)?;
        Ok(self.finish(edited, slots, checksum))
    }

    /// Clear occupied records in place without compacting the array.
    ///
    /// Mirrors `prepare_delete` / `delete_many`.
    pub fn delete(&self, slots: &[usize]) -> Result<PlannedWrite, SaveReadError> {
        if slots.is_empty() {
            return Err(SaveReadError::InvalidTransform {
                message: "at least one slot is required to delete".to_string(),
            });
        }
        let mut seen: Vec<usize> = Vec::new();
        let mut edits: Vec<SlotEdit> = Vec::with_capacity(slots.len());
        for slot_index in slots {
            if seen.contains(slot_index) {
                return Err(SaveReadError::InvalidTransform {
                    message: format!("slot {slot_index} appears twice in one delete"),
                });
            }
            seen.push(*slot_index);
            let current = self.require_slot(*slot_index)?;
            if current.iter().all(|byte| *byte == 0) {
                return Err(SaveReadError::InvalidTransform {
                    message: format!("delete requires occupied slots; slot {slot_index} is empty"),
                });
            }
            let mut owned = [0u8; SCROLL_RECORD_BYTES];
            owned.copy_from_slice(current);
            edits.push(SlotEdit {
                slot_index: *slot_index,
                expected_original: owned,
                replacement: [0u8; SCROLL_RECORD_BYTES],
            });
        }
        self.edit(&edits)
    }

    /// Materialize one prepared record into the next free slot.
    ///
    /// Mirrors `SaveInstaller.install`: the next free slot is the first slot
    /// whose type word is zero, a stale free slot is normalized first, and the
    /// inventory key and generation serial are allocated save-wide.
    pub fn install(&self, request: &InstallRequest) -> Result<PlannedWrite, SaveReadError> {
        let prepared =
            prepare_candidate_for_install(&request.candidate_record, request.transfer_count)?;
        self.install_prepared(&[prepared])
    }

    /// Install several prepared records in one transaction, mirroring
    /// `install_many`.
    pub fn install_many(&self, requests: &[InstallRequest]) -> Result<PlannedWrite, SaveReadError> {
        if requests.is_empty() {
            return Err(SaveReadError::InvalidTransform {
                message: "at least one candidate record is required".to_string(),
            });
        }
        let mut prepared = Vec::with_capacity(requests.len());
        for request in requests {
            prepared.push(prepare_candidate_for_install(
                &request.candidate_record,
                request.transfer_count,
            )?);
        }
        self.install_prepared(&prepared)
    }

    fn install_prepared(
        &self,
        prepared: &[[u8; SCROLL_RECORD_BYTES]],
    ) -> Result<PlannedWrite, SaveReadError> {
        for record in prepared {
            if u16::from_le_bytes([record[0], record[1]]) == 0 {
                return Err(SaveReadError::RecordTypeZero);
            }
        }
        let mut edited = self.plaintext.clone();

        // Mirror `SaveInstaller.install_many` exactly. It is not a repeat of the
        // single-install search:
        // 1. existing colliding generation serials refuse the whole batch,
        // 2. the target is one *contiguous* run starting at
        //    `inventory.next_slot_index`,
        // 3. a run that would cross the 400-slot boundary, or that contains any
        //    occupied slot, refuses before anything is written,
        // 4. every identity value is allocated for the whole batch up front.
        let mut next_serials = allocate_scroll_generation_serials(&edited, prepared.len())?;
        if has_duplicate_generation_serials(&edited)? {
            return Err(SaveReadError::AppendOnlyRepairRequired);
        }
        let first_slot = next_free_slot(&edited)?.ok_or(SaveReadError::AllocationExhausted {
            kind: "scroll slot",
        })?;
        let last_exclusive =
            first_slot
                .checked_add(prepared.len())
                .ok_or(SaveReadError::AllocationExhausted {
                    kind: "scroll slot",
                })?;
        if last_exclusive > crate::layout::SCROLL_SLOT_COUNT {
            return Err(SaveReadError::InsufficientContiguousSlots {
                first_slot,
                needed: prepared.len(),
            });
        }
        let slots: Vec<usize> = (first_slot..last_exclusive).collect();
        for slot_index in &slots {
            let offset = crate::layout::slot_offset(*slot_index)
                .ok_or(SaveReadError::SlotIndex { index: *slot_index })?;
            let record = edited.get(offset..offset + SCROLL_RECORD_BYTES).ok_or(
                SaveReadError::InventoryRegionTruncated {
                    needed: offset + SCROLL_RECORD_BYTES,
                    actual: edited.len(),
                },
            )?;
            if !scroll_slot_is_empty(record)? {
                return Err(SaveReadError::SlotOccupied {
                    slot_index: *slot_index,
                });
            }
        }

        let allocated_keys = allocate_scroll_inventory_keys(&edited, prepared.len())?;
        let allocated_serials = std::mem::take(&mut next_serials);
        let installed: Vec<[u8; SCROLL_RECORD_BYTES]> = prepared
            .iter()
            .zip(allocated_keys.iter())
            .zip(allocated_serials.iter())
            .map(|((record, key), serial)| {
                // The game's own insertion path leaves a new record in the
                // post-insertion lifecycle state; a direct save write must
                // reproduce it instead of inheriting the donor's flags.
                let inserted = write_post_insertion_state(record)?;
                let keyed = write_scroll_inventory_key(&inserted, *key)?;
                write_scroll_generation_serial(&keyed, *serial)
            })
            .collect::<Result<_, SaveReadError>>()?;

        let checksum_before = read_checksum(&edited)?;
        for (slot_index, record) in slots.iter().zip(installed.iter()) {
            edited = clear_native_free_scroll_slot(&edited, *slot_index)?;
            let (next, _report) = insert_scroll_record(&edited, *slot_index, record)?;
            edited = next;
        }
        let (_, new_checksum) = patch_user_checksum(&mut edited)?;
        let mut planned = self.finish(edited, slots, (checksum_before, new_checksum));
        planned.inventory_keys = allocated_keys;
        planned.generation_serials = allocated_serials;
        planned.installed_records = installed;
        Ok(planned)
    }

    fn finish(
        &self,
        plaintext: Vec<u8>,
        slot_indices: Vec<usize>,
        checksum: (u32, u32),
    ) -> PlannedWrite {
        PlannedWrite {
            plaintext_sha256: sha256_hex(&plaintext),
            plaintext,
            slot_indices,
            checksum,
            inventory_keys: Vec::new(),
            generation_serials: Vec::new(),
            installed_records: Vec::new(),
        }
    }
}

/// The folded user checksum currently stored in one decrypted save.
pub fn read_checksum(decrypted: &[u8]) -> Result<u32, SaveReadError> {
    let offset = crate::codec::USER_CHECKSUM_VALUE_OFFSET;
    let window = decrypted
        .get(offset..offset + 4)
        .ok_or(SaveReadError::FieldOutOfRange { offset, width: 4 })?;
    let mut value = [0u8; 4];
    value.copy_from_slice(window);
    Ok(u32::from_le_bytes(value))
}

/// Whether two occupied mapped-scroll records in one save share a `+0x28` value.
///
/// Mirrors `repair_duplicate_scroll_generation_serials`: a collision means the
/// save's append-only identity namespace is already broken, so an insert must
/// refuse rather than silently rewrite records the application does not own.
pub fn has_duplicate_generation_serials(decrypted: &[u8]) -> Result<bool, SaveReadError> {
    let mut seen: Vec<u32> = Vec::new();
    for slot_index in 0..crate::layout::SCROLL_SLOT_COUNT {
        let offset = crate::layout::slot_offset(slot_index)
            .ok_or(SaveReadError::SlotIndex { index: slot_index })?;
        let record = decrypted.get(offset..offset + SCROLL_RECORD_BYTES).ok_or(
            SaveReadError::InventoryRegionTruncated {
                needed: offset + SCROLL_RECORD_BYTES,
                actual: decrypted.len(),
            },
        )?;
        if scroll_slot_is_empty(record)? {
            continue;
        }
        let record_type = u16::from_le_bytes([record[0], record[1]]);
        if mapped_category(record_type).is_none() {
            continue;
        }
        let serial = u32::from_le_bytes([record[0x28], record[0x29], record[0x2A], record[0x2B]]);
        if seen.contains(&serial) {
            return Ok(true);
        }
        seen.push(serial);
    }
    Ok(false)
}

/// The slot the shipped inventory would append to, mirroring
/// `SaveInventory.next_slot_index`.
///
/// The game appends after the occupied tail, so a hole left before the tail is
/// not the append target; only when no free slot follows the tail does the
/// reference reuse the first free hole.
fn next_free_slot(decrypted: &[u8]) -> Result<Option<usize>, SaveReadError> {
    let mut free: Vec<usize> = Vec::new();
    let mut occupied_tail: Option<usize> = None;
    for slot_index in 0..crate::layout::SCROLL_SLOT_COUNT {
        let offset = crate::layout::slot_offset(slot_index)
            .ok_or(SaveReadError::SlotIndex { index: slot_index })?;
        let record = decrypted.get(offset..offset + SCROLL_RECORD_BYTES).ok_or(
            SaveReadError::InventoryRegionTruncated {
                needed: offset + SCROLL_RECORD_BYTES,
                actual: decrypted.len(),
            },
        )?;
        if scroll_slot_is_empty(record)? {
            free.push(slot_index);
        } else {
            occupied_tail = Some(slot_index);
        }
    }
    let tail = occupied_tail;
    Ok(free
        .iter()
        .copied()
        .find(|index| tail.is_none_or(|tail| *index > tail))
        .or_else(|| free.first().copied()))
}

/// Apply the reference header patch to one record.
///
/// Mirrors `patch_local_scroll_header`, including the mirrored level and
/// recommended-level words and the synchronized `+0x30`/`+0x31` rarity bytes.
pub fn patch_local_scroll_header(
    record: &[u8],
    patch: &HeaderPatch,
) -> Result<[u8; SCROLL_RECORD_BYTES], SaveReadError> {
    if record.len() != SCROLL_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: SCROLL_RECORD_BYTES,
            actual: record.len(),
        });
    }
    if !(1..=5).contains(&patch.playthrough) {
        return Err(SaveReadError::InvalidTransform {
            message: "playthrough must be in 1..=5".to_string(),
        });
    }
    if !(3..=5).contains(&patch.rarity) {
        return Err(SaveReadError::InvalidTransform {
            message: "rarity must be 3, 4, or 5".to_string(),
        });
    }
    let record_type = CATEGORY_TO_TYPE[usize::from(patch.playthrough)];
    let mut owned = [0u8; SCROLL_RECORD_BYTES];
    owned.copy_from_slice(record);
    owned[0x00..0x02].copy_from_slice(&record_type.to_le_bytes());
    owned[0x06..0x08].copy_from_slice(&patch.level.to_le_bytes());
    owned[0x08..0x0A].copy_from_slice(&patch.level.to_le_bytes());
    owned[0x10..0x12].copy_from_slice(&patch.recommended_level.to_le_bytes());
    owned[0x12..0x14].copy_from_slice(&patch.recommended_level.to_le_bytes());
    owned[0x20..0x24].copy_from_slice(&patch.seed.to_le_bytes());
    owned[0x30] = patch.rarity;
    owned[0x31] = patch.rarity;
    owned[0xDC..0xE0].copy_from_slice(&patch.transfer_count.to_le_bytes());
    Ok(owned)
}

/// Apply local effect-slot field replacements, mirroring
/// `patch_local_scroll_record`.
pub fn patch_local_scroll_record(
    record: &[u8],
    edits: &[EffectPatch],
) -> Result<[u8; SCROLL_RECORD_BYTES], SaveReadError> {
    if record.len() != SCROLL_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: SCROLL_RECORD_BYTES,
            actual: record.len(),
        });
    }
    if edits.is_empty() {
        return Err(SaveReadError::InvalidTransform {
            message: "at least one local effect edit is required".to_string(),
        });
    }
    let mut seen: Vec<usize> = Vec::new();
    for edit in edits {
        if edit.slot_index >= EFFECT_SLOT_COUNT {
            return Err(SaveReadError::InvalidTransform {
                message: format!("effect slot {} is outside 0..6", edit.slot_index),
            });
        }
        if seen.contains(&edit.slot_index) {
            return Err(SaveReadError::InvalidTransform {
                message: format!("effect slot {} cannot be edited twice", edit.slot_index),
            });
        }
        seen.push(edit.slot_index);
    }
    let mut owned = [0u8; SCROLL_RECORD_BYTES];
    owned.copy_from_slice(record);
    for edit in edits {
        let base = EFFECT_SLOT_BASE + edit.slot_index * EFFECT_SLOT_STRIDE;
        for (offset, value) in [
            (0x00usize, edit.prefix),
            (0x04, edit.effect_id),
            (0x08, edit.value),
            (0x0C, edit.metadata),
            (0x10, edit.tail_0),
            (0x14, edit.tail_1),
        ] {
            if let Some(field) = value {
                owned[base + offset..base + offset + 4].copy_from_slice(&field.to_le_bytes());
            }
        }
    }
    Ok(owned)
}

/// One decoded effect slot, mirroring `read_local_effect_slots`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalEffectSlot {
    pub slot_index: usize,
    pub prefix: u32,
    pub effect_id: u32,
    pub value: u32,
    pub metadata: u32,
    pub tail_0: u32,
    pub tail_1: u32,
}

/// Decode all seven editable slots without applying generation semantics.
pub fn read_local_effect_slots(record: &[u8]) -> Result<Vec<LocalEffectSlot>, SaveReadError> {
    if record.len() != SCROLL_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: SCROLL_RECORD_BYTES,
            actual: record.len(),
        });
    }
    let mut slots = Vec::with_capacity(EFFECT_SLOT_COUNT);
    for slot_index in 0..EFFECT_SLOT_COUNT {
        let base = EFFECT_SLOT_BASE + slot_index * EFFECT_SLOT_STRIDE;
        let mut fields = [0u32; 6];
        for (index, chunk) in record[base..base + 0x18].chunks_exact(4).enumerate() {
            let mut window = [0u8; 4];
            window.copy_from_slice(chunk);
            fields[index] = u32::from_le_bytes(window);
        }
        slots.push(LocalEffectSlot {
            slot_index,
            prefix: fields[0],
            effect_id: fields[1],
            value: fields[2],
            metadata: fields[3],
            tail_0: fields[4],
            tail_1: fields[5],
        });
    }
    Ok(slots)
}

/// The mapped playthrough of a record type, or `None` when unmapped.
pub fn playthrough_of(record_type: u16) -> Option<u8> {
    mapped_category(record_type).filter(|category| (1..=5).contains(category))
}

/// Whether a decrypted blob is a structurally valid user save.
pub fn validate_user_save(decrypted: &[u8]) -> Result<(), SaveReadError> {
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

/// Highest serial the allocator can produce, re-exported for the plan gate.
pub const SERIAL_CEILING: u32 = SCROLL_GENERATION_SERIAL_MAX;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::layout::slot_offset;

    fn blob(records: &[(usize, [u8; SCROLL_RECORD_BYTES])]) -> Vec<u8> {
        let mut save = vec![0u8; USER_SAVE_BYTES];
        save[..6].copy_from_slice(USER_SAVE_MAGIC);
        for (slot_index, record) in records {
            let offset = slot_offset(*slot_index).expect("in range");
            save[offset..offset + SCROLL_RECORD_BYTES].copy_from_slice(record);
        }
        save
    }

    fn record(record_type: u16, seed: u32) -> [u8; SCROLL_RECORD_BYTES] {
        let mut value = [0u8; SCROLL_RECORD_BYTES];
        value[..2].copy_from_slice(&record_type.to_le_bytes());
        value[0x06..0x08].copy_from_slice(&180u16.to_le_bytes());
        value[0x20..0x24].copy_from_slice(&seed.to_le_bytes());
        value[0x30] = 5;
        value
    }

    fn host_over(blob: Vec<u8>) -> SaveTransformHost {
        SaveTransformHost {
            save_path: PathBuf::from("virt"),
            account_id: 1,
            save_slot: 0,
            plaintext: blob,
        }
    }

    #[test]
    fn header_patch_mirrors_level_and_rarity() {
        let patched = patch_local_scroll_header(
            &record(0xE604, 1),
            &HeaderPatch {
                playthrough: 4,
                level: 200,
                recommended_level: 210,
                seed: 0xDEADBEEF,
                rarity: 5,
                transfer_count: 2,
            },
        )
        .expect("patch");
        assert_eq!(u16::from_le_bytes([patched[0x00], patched[0x01]]), 0xDD82);
        assert_eq!(u16::from_le_bytes([patched[0x08], patched[0x09]]), 200);
        assert_eq!(u16::from_le_bytes([patched[0x12], patched[0x13]]), 210);
        assert_eq!(patched[0x30], 5);
        assert_eq!(patched[0x31], 5);
    }

    #[test]
    fn install_uses_the_next_free_slot() {
        let host = host_over(blob(&[(0, record(0xE604, 1))]));
        let planned = host
            .install(&InstallRequest {
                candidate_record: record(0xE604, 0x2222),
                transfer_count: 1,
            })
            .expect("install");
        assert_eq!(planned.slot_indices, vec![1]);
        let offset = slot_offset(1).expect("in range");
        assert_eq!(
            u16::from_le_bytes([planned.plaintext[offset], planned.plaintext[offset + 1]]),
            0xE604
        );
        assert_ne!(planned.checksum.0, u32::MAX);
    }

    /// A newly installed scroll must read like the game's own insertion result.
    ///
    /// The donor's lifecycle word (`+0x18`..`+0x1B`) is never inherited: the
    /// builder state `0x02800002` plus the engine insertion bits `0x04000080`
    /// is what a pickup leaves behind, so the installed record must read
    /// `0x06800082` with its generated effect bytes untouched.
    #[test]
    fn install_writes_the_post_insertion_lifecycle_word() {
        let mut donor = record(0xE604, 1);
        donor[0x18..0x1C].copy_from_slice(&0x0F80_0080u32.to_le_bytes());
        donor[0x28..0x2C].copy_from_slice(&40u32.to_le_bytes());

        let mut candidate = record(0xE604, 0x2222);
        candidate[0x18..0x1C].copy_from_slice(&0x0F80_0080u32.to_le_bytes());
        for index in 0..SCROLL_RECORD_BYTES - 0x34 {
            candidate[0x34 + index] = (index % 251) as u8;
        }
        let candidate_effects = candidate[0x34..0xDC].to_vec();

        let host = host_over(blob(&[(0, donor)]));
        let planned = host
            .install(&InstallRequest {
                candidate_record: candidate,
                transfer_count: 7,
            })
            .expect("install");
        assert_eq!(planned.slot_indices, vec![1]);
        let offset = slot_offset(1).expect("in range");
        let installed = &planned.plaintext[offset..offset + SCROLL_RECORD_BYTES];

        let word = u32::from_le_bytes([
            installed[0x18],
            installed[0x19],
            installed[0x1A],
            installed[0x1B],
        ]);
        assert_eq!(word, crate::codec::POST_INSERTION_FLAG_WORD);
        assert_eq!(installed[0x18] & 0x02, 0x02, "the new-item marker is set");
        assert_eq!(
            word & 0x0400_0080,
            0x0400_0080,
            "the engine insertion bits are set"
        );
        assert_eq!(
            word & 0x0900_0000,
            0,
            "the installed record is not revealed"
        );
        assert_eq!(
            &installed[0x34..0xDC],
            candidate_effects.as_slice(),
            "generated effect bytes must survive the install"
        );
        assert_eq!(
            u32::from_le_bytes([
                installed[0xDC],
                installed[0xDD],
                installed[0xDE],
                installed[0xDF],
            ]),
            7,
            "the transfer count is still written"
        );
        let donor_offset = slot_offset(0).expect("in range");
        assert_eq!(
            &planned.plaintext[donor_offset + 0x18..donor_offset + 0x1C],
            &0x0F80_0080u32.to_le_bytes(),
            "the donor record must not change"
        );
    }

    #[test]
    fn delete_clears_in_place_and_refuses_an_empty_slot() {
        let host = host_over(blob(&[(0, record(0xE604, 1))]));
        let planned = host.delete(&[0]).expect("delete");
        let offset = slot_offset(0).expect("in range");
        assert!(planned.plaintext[offset..offset + SCROLL_RECORD_BYTES]
            .iter()
            .all(|byte| *byte == 0));
        assert!(
            host.delete(&[2]).is_err(),
            "an empty slot cannot be deleted"
        );
    }

    #[test]
    fn edit_refuses_a_stale_original() {
        let host = host_over(blob(&[(0, record(0xE604, 1))]));
        let stale = record(0xE604, 0x99);
        let error = host
            .edit(&[SlotEdit {
                slot_index: 0,
                expected_original: stale,
                replacement: record(0xE604, 5),
            }])
            .expect_err("stale original");
        assert!(matches!(error, SaveReadError::IntegrityMismatch { .. }));
    }
}
