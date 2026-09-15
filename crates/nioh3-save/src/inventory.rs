//! Read-only inventory, template and identity view of a decrypted user save.
//!
//! Mirrors `nioh3_scroll_editor/savegame.py::SaveInventory`. Nothing here
//! mutates the blob: a "rebind" produces a new in-memory record and leaves the
//! save bytes untouched.
//!
//! Every index below is a fixed offset or slice of a record whose length is
//! proven by `ScrollRecordBytes`, so the module opts out of the indexing lint
//! once instead of at each constant offset.
#![allow(clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

use nioh3_domain::record::ScrollRecordBytes;

use crate::error::SaveReadError;
use crate::layout::{
    slot_offset, CATEGORY_TO_TYPE, RECORD_INVENTORY_KEY_OFFSET, SCROLL_RECORD_BYTES,
    SCROLL_SLOT_COUNT, TEMPLATE_RECORD_TYPE,
};
use crate::paths::account_id_from_save_path;
use crate::save::DecryptedSave;

/// One occupied scroll slot, without compacting or reordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollInventoryEntry {
    pub slot_index: usize,
    pub record_offset: usize,
    record: ScrollRecordBytes,
    pub mapped_category: Option<u8>,
}

impl ScrollInventoryEntry {
    /// The declared record type (`+0x00`).
    pub fn record_type(&self) -> u16 {
        self.record.record_type()
    }

    /// Mapped playthrough category, when the record type is a mapped scroll.
    pub fn playthrough(&self) -> Option<u8> {
        self.mapped_category
    }

    /// Displayed seed (`+0x20`).
    pub fn seed(&self) -> u32 {
        self.record.displayed_seed()
    }

    /// Rarity byte (`+0x30`).
    pub fn rarity(&self) -> u8 {
        self.record.rarity()
    }

    /// Transfer count (`+0xDC`).
    pub fn transfer_count(&self) -> u32 {
        self.record.transfer_count()
    }

    /// Generation serial (`+0x28`).
    pub fn generation_serial(&self) -> u32 {
        self.record.generation_serial()
    }

    /// Raw `0xE8` record bytes.
    pub fn record_bytes(&self) -> &[u8; SCROLL_RECORD_BYTES] {
        self.record.as_bytes()
    }

    /// Typed record codec view.
    pub fn record(&self) -> &ScrollRecordBytes {
        &self.record
    }

    /// Account id encoded in the record (`+0x02`/`+0x04`/`+0x14`).
    pub fn account_id(&self) -> u64 {
        account_id_from_record(&self.record)
    }

    /// Inventory key (`+0x1C`), zero when the slot carries no key.
    pub fn inventory_key(&self) -> u32 {
        read_u32(self.record.as_bytes(), RECORD_INVENTORY_KEY_OFFSET)
    }
}

/// One authentic template record and the record type it serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRecord {
    pub record_type: u16,
    pub record: ScrollRecordBytes,
}

/// Read-only inventory view of one validated decrypted save.
#[derive(Debug, Clone)]
pub struct SaveInventory {
    pub save_path: PathBuf,
    save: DecryptedSave,
    pub account_id: u64,
    pub template_record: Option<ScrollRecordBytes>,
    pub template_records: Vec<TemplateRecord>,
    pub empty_slots: Vec<usize>,
    pub next_slot_index: Option<usize>,
}

impl SaveInventory {
    /// Build the view for one decrypted save.
    ///
    /// `allow_empty` mirrors the shipped loader: an inventory with no mapped
    /// scroll may be accepted for a read-only surface, but a save that is too
    /// small, has the wrong magic, or has no template fails closed.
    pub fn load(
        save_path: &Path,
        save: DecryptedSave,
        allow_empty: bool,
    ) -> Result<Self, SaveReadError> {
        let account_id = account_id_from_save_path(save_path)?;
        let mut own_records: Vec<ScrollRecordBytes> = Vec::new();
        let mut mapped_records: Vec<ScrollRecordBytes> = Vec::new();
        let mut empty_slots: Vec<usize> = Vec::new();
        let mut occupied_slots: Vec<usize> = Vec::new();
        for slot_index in 0..SCROLL_SLOT_COUNT {
            let raw = save.raw_record(slot_index)?;
            let record = ScrollRecordBytes::from_slice(raw)?;
            if record.record_type() == 0 {
                // A deletion clears only the type; the tail may hold stale bytes.
                empty_slots.push(slot_index);
                continue;
            }
            occupied_slots.push(slot_index);
            if mapped_category(record.record_type()).is_none() {
                continue;
            }
            let owned_by_account = account_id_from_record(&record) == account_id;
            if owned_by_account {
                own_records.push(record.clone());
            }
            mapped_records.push(record);
        }

        if mapped_records.is_empty() {
            if allow_empty {
                return Ok(Self {
                    save_path: save_path.to_path_buf(),
                    save,
                    account_id,
                    template_record: None,
                    template_records: Vec::new(),
                    next_slot_index: empty_slots.first().copied(),
                    empty_slots,
                });
            }
            return Err(SaveReadError::TemplateUnavailable { playthrough: 3 });
        }

        let own_e604 = own_records
            .iter()
            .find(|record| record.record_type() == TEMPLATE_RECORD_TYPE);
        let any_e604 = mapped_records
            .iter()
            .find(|record| record.record_type() == TEMPLATE_RECORD_TYPE);
        let selected = own_e604
            .or(any_e604)
            .or_else(|| own_records.first())
            .or_else(|| mapped_records.first())
            .ok_or(SaveReadError::TemplateUnavailable { playthrough: 3 })?;
        let template_record = rebind_account_id(selected, account_id);

        let mut template_records = Vec::new();
        for record_type in CATEGORY_TO_TYPE.iter().skip(1) {
            let selected = own_records
                .iter()
                .find(|record| record.record_type() == *record_type)
                .or_else(|| {
                    mapped_records
                        .iter()
                        .find(|record| record.record_type() == *record_type)
                });
            let Some(selected) = selected else {
                continue;
            };
            template_records.push(TemplateRecord {
                record_type: *record_type,
                record: rebind_account_id(selected, account_id),
            });
        }

        let occupied_tail = occupied_slots.iter().copied().max().unwrap_or(0);
        let next_slot_index = empty_slots
            .iter()
            .copied()
            .find(|index| *index > occupied_tail)
            .or_else(|| empty_slots.first().copied());

        Ok(Self {
            save_path: save_path.to_path_buf(),
            save,
            account_id,
            template_record: Some(template_record),
            template_records,
            empty_slots,
            next_slot_index,
        })
    }

    /// The validated decrypted blob.
    pub fn decrypted(&self) -> &DecryptedSave {
        &self.save
    }

    /// SHA-256 of the decrypted blob.
    pub fn source_sha256(&self) -> String {
        self.save.sha256()
    }

    /// Mapped scroll entries in physical slot order, never compacted.
    pub fn scroll_entries(&self, include_unmapped: bool) -> Vec<ScrollInventoryEntry> {
        let mut entries = Vec::new();
        for slot_index in 0..SCROLL_SLOT_COUNT {
            let Some(offset) = slot_offset(slot_index) else {
                continue;
            };
            let Ok(raw) = self.save.raw_record(slot_index) else {
                continue;
            };
            let Ok(record) = ScrollRecordBytes::from_slice(raw) else {
                continue;
            };
            if record.record_type() == 0 {
                continue;
            }
            let category = mapped_category(record.record_type());
            if category.is_none() && !include_unmapped {
                continue;
            }
            entries.push(ScrollInventoryEntry {
                slot_index,
                record_offset: offset,
                record,
                mapped_category: category,
            });
        }
        entries
    }

    /// One occupied entry by slot index.
    pub fn entry(&self, slot_index: usize) -> Result<ScrollInventoryEntry, SaveReadError> {
        let offset =
            slot_offset(slot_index).ok_or(SaveReadError::SlotIndex { index: slot_index })?;
        let raw = self.save.raw_record(slot_index)?;
        let record = ScrollRecordBytes::from_slice(raw)?;
        if record.record_type() == 0 {
            return Err(SaveReadError::EmptyRecord { slot_index });
        }
        Ok(ScrollInventoryEntry {
            slot_index,
            record_offset: offset,
            mapped_category: mapped_category(record.record_type()),
            record,
        })
    }

    /// The template record that serves `playthrough`, mirroring the shipped
    /// fallback: an authentic record when present, otherwise an in-memory clone
    /// of the highest available genuine template with only its type changed.
    pub fn template_record_for_playthrough(
        &self,
        playthrough: u8,
    ) -> Result<ScrollRecordBytes, SaveReadError> {
        let Some(base) = self.template_record.as_ref() else {
            return Err(SaveReadError::TemplateUnavailable { playthrough });
        };
        if !(1..=5).contains(&playthrough) {
            return Err(SaveReadError::Playthrough { playthrough });
        }
        let record_type = CATEGORY_TO_TYPE[playthrough as usize];
        if let Some(found) = self
            .template_records
            .iter()
            .find(|template| template.record_type == record_type)
        {
            return Ok(found.record.clone());
        }
        if playthrough >= 4 {
            let mut synthetic: [u8; SCROLL_RECORD_BYTES] = *base.as_bytes();
            synthetic[0..2].copy_from_slice(&record_type.to_le_bytes());
            return ScrollRecordBytes::from_slice(&synthetic).map_err(SaveReadError::from);
        }
        Err(SaveReadError::TemplateUnavailable { playthrough })
    }

    /// The next generation serial that avoids every occupied record.
    ///
    /// This is a *read* of the current allocation state, used by the shipped
    /// count/edit source path to describe what a later transaction would take.
    pub fn next_generation_serial(&self) -> u32 {
        let highest = self
            .occupied_generation_serials()
            .into_iter()
            .max()
            .unwrap_or(0);
        highest.saturating_add(1)
    }

    /// Every nonzero generation serial currently present in the region.
    pub fn occupied_generation_serials(&self) -> Vec<u32> {
        let mut serials = Vec::new();
        for slot_index in 0..SCROLL_SLOT_COUNT {
            let Ok(raw) = self.save.raw_record(slot_index) else {
                continue;
            };
            let Ok(record) = ScrollRecordBytes::from_slice(raw) else {
                continue;
            };
            if record.record_type() == 0 {
                continue;
            }
            let serial = record.generation_serial();
            if serial != 0 {
                serials.push(serial);
            }
        }
        serials
    }

    /// Every inventory key currently in use (mapped or not), in slot order.
    pub fn occupied_inventory_keys(&self) -> Vec<u32> {
        let mut keys = Vec::new();
        for slot_index in 0..SCROLL_SLOT_COUNT {
            let Ok(raw) = self.save.raw_record(slot_index) else {
                continue;
            };
            let Ok(record) = ScrollRecordBytes::from_slice(raw) else {
                continue;
            };
            if record.record_type() == 0 {
                continue;
            }
            let key = read_u32(record.as_bytes(), RECORD_INVENTORY_KEY_OFFSET);
            if key != 0 {
                keys.push(key);
            }
        }
        keys
    }
}

/// Byte offset and width of the fields a record-level accessor does not expose.
fn read_u32(record: &[u8; SCROLL_RECORD_BYTES], offset: usize) -> u32 {
    u32::from_le_bytes([
        record[offset],
        record[offset + 1],
        record[offset + 2],
        record[offset + 3],
    ])
}

/// Reverse lookup of [`CATEGORY_TO_TYPE`], mirroring `TYPE_TO_CATEGORY`.
pub fn mapped_category(record_type: u16) -> Option<u8> {
    CATEGORY_TO_TYPE
        .iter()
        .position(|candidate| *candidate == record_type)
        .filter(|category| *category != 0)
        .map(|category| category as u8)
}

/// Account id encoded at `+0x02`/`+0x04`/`+0x14`, mirroring
/// `emaki_exchange.account_id_from_record`.
pub fn account_id_from_record(record: &ScrollRecordBytes) -> u64 {
    let bytes = record.as_bytes();
    let high = u16::from_le_bytes([bytes[0x02], bytes[0x03]]) as u64;
    let middle = u16::from_le_bytes([bytes[0x04], bytes[0x05]]) as u64;
    let low = u32::from_le_bytes([bytes[0x14], bytes[0x15], bytes[0x16], bytes[0x17]]) as u64;
    (high << 48) | (middle << 32) | low
}

/// Rebind the account id inside an in-memory template copy.
///
/// Mirrors `write_account_id`: the save bytes are never touched, only the
/// returned copy.
#[allow(clippy::expect_used)]
fn rebind_account_id(record: &ScrollRecordBytes, account_id: u64) -> ScrollRecordBytes {
    if account_id_from_record(record) == account_id {
        return record.clone();
    }
    let mut bytes: [u8; SCROLL_RECORD_BYTES] = *record.as_bytes();
    bytes[0x02..0x04].copy_from_slice(&((account_id >> 48) as u16).to_le_bytes());
    bytes[0x04..0x06].copy_from_slice(&((account_id >> 32) as u16).to_le_bytes());
    bytes[0x14..0x18].copy_from_slice(&((account_id & 0xFFFF_FFFF) as u32).to_le_bytes());
    // The buffer is unchanged in length, so the fixed-size codec cannot fail.
    ScrollRecordBytes::from_slice(&bytes).expect("a rebound record keeps its fixed length")
}
