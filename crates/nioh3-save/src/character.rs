//! Character currencies and owned equipment in a decrypted user save.
//!
//! The PC v2.02 character block is a tagged stream of `key u32 | width u32 |
//! value[width]` entries, so a currency is located by its key rather than by a
//! fixed offset; the key must occur exactly once with the expected width, or the
//! read fails closed. Owned equipment is a fixed array of `0xF0`-byte records
//! that is byte-identical to the live game's equipment container
//! (`docs/knowledge/V082_LIVE_CHARACTER_EQUIPMENT_RESEARCH_20260926.md`).

use crate::error::SaveReadError;
use serde::{Deserialize, Serialize};

/// First byte of the owned-equipment array.
pub const EQUIPMENT_GROUP_OFFSET: usize = 0x27_0066;
/// One equipment record.
pub const EQUIPMENT_RECORD_BYTES: usize = 0xF0;
/// Slots in the owned-equipment array.
pub const EQUIPMENT_SLOT_COUNT: usize = 2500;
/// End of the owned-equipment array.
pub const EQUIPMENT_REGION_END: usize =
    EQUIPMENT_GROUP_OFFSET + EQUIPMENT_SLOT_COUNT * EQUIPMENT_RECORD_BYTES;

/// A character currency stored as a tagged 64-bit value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Currency {
    /// 精华 (Amrita).
    Amrita,
    /// 持有金钱.
    Gold,
}

impl Currency {
    pub const ALL: [Self; 2] = [Self::Amrita, Self::Gold];

    /// The tagged-stream key.
    pub const fn key(self) -> u32 {
        match self {
            Self::Amrita => 0x13B4_3052,
            Self::Gold => 0x75AD_54DF,
        }
    }

    /// Stable wire name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Amrita => "amrita",
            Self::Gold => "gold",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|currency| currency.label() == label)
    }
}

/// Width of a currency value in the tagged stream.
pub const CURRENCY_WIDTH: usize = 8;

/// Offset of the value bytes of the only `key` entry with `width` bytes.
pub fn tagged_value_offset(plain: &[u8], key: u32, width: usize) -> Result<usize, SaveReadError> {
    let mut needle = [0u8; 8];
    needle[..4].copy_from_slice(&key.to_le_bytes());
    needle[4..].copy_from_slice(&(width as u32).to_le_bytes());
    let mut found = None;
    let mut start = 0;
    while let Some(position) = find(&plain[start..], &needle) {
        let entry = start + position;
        if found.is_some() {
            return Err(SaveReadError::InvalidTransform {
                message: format!("tagged key {key:#010x} occurs more than once"),
            });
        }
        found = Some(entry);
        start = entry + 1;
    }
    let entry = found.ok_or_else(|| SaveReadError::InvalidTransform {
        message: format!("tagged key {key:#010x} with width {width} is not present"),
    })?;
    let value = entry + needle.len();
    if value + width > plain.len() {
        return Err(SaveReadError::FieldOutOfRange {
            offset: value,
            width,
        });
    }
    Ok(value)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Offset of one currency value.
pub fn currency_offset(plain: &[u8], currency: Currency) -> Result<usize, SaveReadError> {
    tagged_value_offset(plain, currency.key(), CURRENCY_WIDTH)
}

/// The stored value of one currency.
pub fn read_currency(plain: &[u8], currency: Currency) -> Result<u64, SaveReadError> {
    let offset = currency_offset(plain, currency)?;
    let mut value = [0u8; CURRENCY_WIDTH];
    value.copy_from_slice(&plain[offset..offset + CURRENCY_WIDTH]);
    Ok(u64::from_le_bytes(value))
}

/// First byte of one equipment slot.
pub const fn equipment_offset(slot_index: usize) -> Option<usize> {
    if slot_index < EQUIPMENT_SLOT_COUNT {
        Some(EQUIPMENT_GROUP_OFFSET + slot_index * EQUIPMENT_RECORD_BYTES)
    } else {
        None
    }
}

/// One equipment record.
pub fn equipment_record(plain: &[u8], slot_index: usize) -> Result<&[u8], SaveReadError> {
    let offset =
        equipment_offset(slot_index).ok_or(SaveReadError::SlotIndex { index: slot_index })?;
    plain.get(offset..offset + EQUIPMENT_RECORD_BYTES).ok_or(
        SaveReadError::InventoryRegionTruncated {
            needed: offset + EQUIPMENT_RECORD_BYTES,
            actual: plain.len(),
        },
    )
}

/// A free equipment slot keeps its template tail but has item id zero.
pub fn equipment_slot_is_empty(record: &[u8]) -> bool {
    record.len() < 2 || u16::from_le_bytes([record[0], record[1]]) == 0
}

/// Record `+0xE8` and `+0xEC` (u32): the item's position in each of the two
/// equipment sets, or this value when that set does not wear it (PC v2.02,
/// live: swapping one helmet per set changed only these words).
pub const EQUIPMENT_NOT_WORN: u32 = 0x11;
/// The two equipment-set position words.
pub const EQUIPMENT_SET_OFFSETS: [usize; 2] = [0xE8, 0xEC];

/// Whether either equipment set wears this record.
pub fn equipment_is_worn(record: &[u8]) -> bool {
    record.len() >= EQUIPMENT_RECORD_BYTES
        && EQUIPMENT_SET_OFFSETS
            .iter()
            .any(|offset| u32_at(record, *offset) != EQUIPMENT_NOT_WORN)
}

/// Bytes of each effect entry a free slot keeps from its last item; the game
/// never clears them (1,094 free slots of a live PC v2.02 inventory differ
/// only there).
const FREE_SLOT_KEPT_ENTRY_BYTES: [usize; 6] = [0x2, 0x3, 0xE, 0xF, 0x12, 0x13];

/// The record as the game leaves a slot it frees: item id zero, every field
/// cleared to the free-slot values, each effect entry emptied and both
/// equipment-set words set to "not worn". A worn or already free record is
/// refused; the running game still shows a worn item in its equipment sets.
pub fn free_equipment_slot(record: &[u8]) -> Result<Vec<u8>, SaveReadError> {
    equipment_fields(record)?;
    if equipment_is_worn(record) {
        return Err(SaveReadError::InvalidTransform {
            message: "an equipped item cannot be removed".to_string(),
        });
    }
    free_equipment_slot_unequipping(record)
}

/// [`free_equipment_slot`] for a save file, where a worn item may go too.
///
/// Worn state lives only in the record's two set words (no other part of the
/// save names the item), so the freed slot simply leaves both sets. This is
/// how a record CE turned into an unequippable book gets out of a set.
pub fn free_equipment_slot_unequipping(record: &[u8]) -> Result<Vec<u8>, SaveReadError> {
    equipment_fields(record)?;
    if equipment_slot_is_empty(record) {
        return Err(SaveReadError::RecordTypeZero);
    }
    let mut freed = vec![0u8; EQUIPMENT_RECORD_BYTES];
    freed[0x0F] = 0x40;
    freed[0x18] = 0x02;
    freed[0x28..=0x30].fill(0xFF);
    for index in 0..EQUIPMENT_EFFECT_COUNT {
        let entry = EQUIPMENT_EFFECT_ID_OFFSET - 4 + index * EQUIPMENT_EFFECT_STRIDE;
        freed[entry + 4..entry + 8].fill(0xFF);
        for kept in FREE_SLOT_KEPT_ENTRY_BYTES {
            freed[entry + kept] = record[entry + kept];
        }
    }
    for offset in EQUIPMENT_SET_OFFSETS {
        freed[offset..offset + 4].copy_from_slice(&EQUIPMENT_NOT_WORN.to_le_bytes());
    }
    Ok(freed)
}

/// The save-wide counter of the next inventory key (`+0x1C`), a u32 whose
/// value stays within u16 (PC v2.02: one blacksmith purchase took `0xC86F`
/// and left `0xC870`).
pub const NEXT_INVENTORY_KEY_OFFSET: usize = 0x36_E226;
/// The save-wide counter of the next generation serial (`+0x28`); opening a
/// shop advances it past its whole stock.
pub const NEXT_GENERATION_SERIAL_OFFSET: usize = 0x36_E232;

/// The counters the game takes a new equipment record's key and serial from.
pub fn equipment_counters(plain: &[u8]) -> Result<(u32, u32), SaveReadError> {
    let read = |offset: usize| {
        plain
            .get(offset..offset + 4)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            .ok_or(SaveReadError::InventoryRegionTruncated {
                needed: offset + 4,
                actual: plain.len(),
            })
    };
    Ok((
        read(NEXT_INVENTORY_KEY_OFFSET)?,
        read(NEXT_GENERATION_SERIAL_OFFSET)?,
    ))
}

/// Where the game puts a new item: the first free slot after the occupied
/// tail (live purchase, PC v2.02), else the first free slot at all. Slots in
/// `taken` are treated as occupied.
pub fn next_free_equipment_slot(plain: &[u8], taken: &[usize]) -> Option<usize> {
    let free = |slot: usize| {
        !taken.contains(&slot) && equipment_record(plain, slot).is_ok_and(equipment_slot_is_empty)
    };
    let tail = (0..EQUIPMENT_SLOT_COUNT).rev().find(|slot| !free(*slot));
    let after = tail.map_or(0, |slot| slot + 1);
    (after..EQUIPMENT_SLOT_COUNT)
        .chain(0..after)
        .find(|slot| free(*slot))
}

/// The role an entry of a new record plays, which fixes its marker bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewEntryRole {
    Innate,
    Hell,
    Random,
    /// Entry `+0x0E` bit `0x02`; never carries a forge-material marker.
    Grace,
    /// Entry `+0x0E` bit `0x01` and the `+0x0D` marker `0x40`, as on every set
    /// entry of the owner's equipment.
    Set,
}

/// One effect entry of a new equipment record, markers already resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEquipmentEffect {
    pub role: NewEntryRole,
    pub effect_id: u32,
    pub value: u32,
    pub roll: u8,
    pub star: bool,
    pub group: u16,
    pub category: u8,
}

/// Everything a new equipment record is built from besides its identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEquipment {
    pub item_id: u16,
    pub level: u16,
    pub plus: u16,
    pub rarity: u8,
    pub hell: bool,
    pub hell_skill: u16,
    pub effects: Vec<NewEquipmentEffect>,
}

/// Build a new equipment record over the free slot it will occupy.
///
/// The layout follows a record the game wrote for a purchase: item and
/// appearance, count 1, level and pre-forge level, `+0x18` flags with the new
/// marker (`0x82`, as on a fresh drop; `+0x1A` bit `0x10` for a hell weapon),
/// key, `+0x20 = 1`, a seed, the serial, rarity, the entries and both set
/// words `0x11`. Unused entries carry the empty id. Bytes the game leaves
/// undefined (entry `+0x0F`, for example) keep what the free slot holds.
pub fn build_equipment_record(
    free: &[u8],
    new: &NewEquipment,
    key: u32,
    serial: u32,
) -> Result<Vec<u8>, SaveReadError> {
    if free.len() != EQUIPMENT_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: EQUIPMENT_RECORD_BYTES,
            actual: free.len(),
        });
    }
    if !equipment_slot_is_empty(free) {
        return Err(SaveReadError::InvalidTransform {
            message: "a new item needs a free equipment slot".to_string(),
        });
    }
    if new.item_id == 0 || new.effects.len() > EQUIPMENT_EFFECT_COUNT {
        return Err(SaveReadError::InvalidTransform {
            message: "a new item needs an item id and at most seven effects".to_string(),
        });
    }
    let mut record = free.to_vec();
    let put16 = |record: &mut Vec<u8>, offset: usize, value: u16| {
        record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    };
    let put32 = |record: &mut Vec<u8>, offset: usize, value: u32| {
        record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    };
    put16(&mut record, 0x00, new.item_id);
    put16(&mut record, 0x02, new.item_id);
    put16(&mut record, 0x04, 1);
    put16(&mut record, 0x06, new.level);
    put16(&mut record, 0x08, new.level);
    put16(&mut record, 0x0A, new.plus);
    put16(&mut record, 0x0C, 0);
    record[0x0E] = 0;
    record[0x0F] = 0x40;
    put16(&mut record, 0x10, if new.hell { new.hell_skill } else { 0 });
    put16(&mut record, 0x12, 0);
    put32(&mut record, 0x14, 0);
    put32(
        &mut record,
        0x18,
        0x82 | if new.hell { 0x10_0000 } else { 0 },
    );
    put32(&mut record, 0x1C, key);
    put16(&mut record, 0x20, 1);
    // Any value is natural; derive it from the serial so a plan is repeatable.
    put16(
        &mut record,
        0x22,
        (serial.wrapping_mul(0x9E37_79B1) >> 16) as u16,
    );
    put32(&mut record, 0x24, 0);
    put32(&mut record, 0x28, serial);
    put32(&mut record, 0x2C, 0);
    record[0x30] = new.rarity;
    record[0x31..0x34].fill(0);
    for index in 0..EQUIPMENT_EFFECT_COUNT {
        let entry = EQUIPMENT_EFFECT_ID_OFFSET - 4 + index * EQUIPMENT_EFFECT_STRIDE;
        put32(&mut record, entry, 0);
        put32(&mut record, entry + 4, EMPTY_EFFECT_ID);
        put32(&mut record, entry + 8, 0);
        record[entry + 0x0C..entry + 0x0F].fill(0);
        record[entry + 0x10..entry + EQUIPMENT_EFFECT_STRIDE].fill(0);
        let Some(effect) = new.effects.get(index) else {
            continue;
        };
        put16(&mut record, entry, effect.group);
        put32(&mut record, entry + 4, effect.effect_id);
        put32(&mut record, entry + 8, effect.value);
        record[entry + 0x0C] = effect.roll;
        let marker = if effect.role == NewEntryRole::Set {
            0x40
        } else {
            0
        };
        record[entry + 0x0D] = (effect.category & 0x3F) | marker;
        record[entry + 0x0E] = match effect.role {
            NewEntryRole::Set => 0x01,
            NewEntryRole::Grace => 0x02,
            _ => 0,
        } | if effect.star { 0x04 } else { 0 };
    }
    for offset in EQUIPMENT_SET_OFFSETS {
        put32(&mut record, offset, EQUIPMENT_NOT_WORN);
    }
    equipment_fields(&record)?;
    Ok(record)
}

/// Effect entries in one equipment record.
pub const EQUIPMENT_EFFECT_COUNT: usize = 7;
/// Distance between effect entries.
pub const EQUIPMENT_EFFECT_STRIDE: usize = 0x18;
/// Effect id of the first entry; its value follows at `+4`.
pub const EQUIPMENT_EFFECT_ID_OFFSET: usize = 0x38;
/// An unused effect entry.
pub const EMPTY_EFFECT_ID: u32 = u32::MAX;

/// The decoded fields of one equipment record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquipmentFields {
    pub item_id: u16,
    pub appearance_id: u16,
    pub quantity: u16,
    pub level: u16,
    pub level_before_forge: u16,
    pub plus: u16,
    pub familiarity: u32,
    pub inventory_key: u16,
    pub seed: u16,
    pub rarity: u8,
    /// `(effect id, raw value)` per entry; an unused entry has id `u32::MAX`.
    pub effects: Vec<(u32, u32)>,
    /// The star (✦) marker of each entry, as the game shows it.
    pub stars: Vec<bool>,
    /// The hell-weapon marker (`+0x1A` bit `0x10`).
    pub hell: bool,
    /// The hell martial skill (`+0x10`).
    pub hell_skill: u16,
    /// Whether either equipment set wears the item.
    pub worn: bool,
}

fn u16_at(record: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([record[offset], record[offset + 1]])
}

fn u32_at(record: &[u8], offset: usize) -> u32 {
    let mut value = [0u8; 4];
    value.copy_from_slice(&record[offset..offset + 4]);
    u32::from_le_bytes(value)
}

fn effect_offset(index: usize) -> usize {
    EQUIPMENT_EFFECT_ID_OFFSET + index * EQUIPMENT_EFFECT_STRIDE
}

/// Decode one `0xF0`-byte equipment record.
pub fn equipment_fields(record: &[u8]) -> Result<EquipmentFields, SaveReadError> {
    if record.len() != EQUIPMENT_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: EQUIPMENT_RECORD_BYTES,
            actual: record.len(),
        });
    }
    Ok(EquipmentFields {
        item_id: u16_at(record, 0x00),
        appearance_id: u16_at(record, 0x02),
        quantity: u16_at(record, 0x04),
        level: u16_at(record, 0x06),
        level_before_forge: u16_at(record, 0x08),
        plus: u16_at(record, 0x0A),
        familiarity: u32_at(record, 0x14),
        inventory_key: u16_at(record, 0x1C),
        seed: u16_at(record, 0x22),
        rarity: record[0x30],
        hell: record[0x1A] & 0x10 != 0,
        hell_skill: u16_at(record, 0x10),
        worn: equipment_is_worn(record),
        effects: (0..EQUIPMENT_EFFECT_COUNT)
            .map(|index| {
                let offset = effect_offset(index);
                (u32_at(record, offset), u32_at(record, offset + 4))
            })
            .collect(),
        stars: (0..EQUIPMENT_EFFECT_COUNT)
            .map(|index| record[effect_offset(index) + 0xA] & 0x04 != 0)
            .collect(),
    })
}

/// One effect entry to overwrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentEffectPatch {
    pub index: usize,
    pub effect_id: u32,
    pub value: u32,
    /// Entry byte `+0xC`: the roll percent the value was derived from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roll: Option<u8>,
    /// Entry byte `+0xE` bit `0x04`: the star (✦) marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub star: Option<bool>,
    /// Entry `+0x00` (u16): the effect's group, which the game shows the icon
    /// from. Filled by the host from the effect tables, never by the caller.
    #[serde(skip)]
    pub group: Option<u16>,
    /// Entry byte `+0x0D` low six bits: the group's category.
    #[serde(skip)]
    pub category: Option<u8>,
}

/// Field overwrites for one equipment record. `None` leaves a field as stored.
///
/// These are raw writes: nothing here checks that the result could occur
/// naturally, so callers must present them as modded edits.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentPatch {
    pub level: Option<u16>,
    pub level_before_forge: Option<u16>,
    pub plus: Option<u16>,
    pub familiarity: Option<u32>,
    pub rarity: Option<u8>,
    #[serde(default)]
    pub effects: Vec<EquipmentEffectPatch>,
    /// Record byte `+0x1A` bit `0x10`: the hell-weapon marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hell: Option<bool>,
    /// Record `+0x10` (u16): the hell martial skill; 0 on a normal weapon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hell_skill: Option<u16>,
}

/// Apply `patch` to a copy of `record`.
pub fn patch_equipment(record: &[u8], patch: &EquipmentPatch) -> Result<Vec<u8>, SaveReadError> {
    equipment_fields(record)?;
    if equipment_slot_is_empty(record) {
        return Err(SaveReadError::RecordTypeZero);
    }
    let mut patched = record.to_vec();
    let mut put16 = |offset: usize, value: Option<u16>| {
        if let Some(value) = value {
            patched[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
    };
    put16(0x06, patch.level);
    put16(0x08, patch.level_before_forge);
    put16(0x0A, patch.plus);
    if let Some(familiarity) = patch.familiarity {
        patched[0x14..0x18].copy_from_slice(&familiarity.to_le_bytes());
    }
    if let Some(rarity) = patch.rarity {
        patched[0x30] = rarity;
    }
    if let Some(hell) = patch.hell {
        patched[0x1A] = (patched[0x1A] & !0x10) | if hell { 0x10 } else { 0 };
    }
    if let Some(skill) = patch.hell_skill {
        patched[0x10..0x12].copy_from_slice(&skill.to_le_bytes());
    }
    let mut seen = Vec::new();
    for effect in &patch.effects {
        if effect.index >= EQUIPMENT_EFFECT_COUNT || seen.contains(&effect.index) {
            return Err(SaveReadError::InvalidTransform {
                message: format!("effect entry {} is out of range or repeated", effect.index),
            });
        }
        seen.push(effect.index);
        let offset = effect_offset(effect.index);
        patched[offset..offset + 4].copy_from_slice(&effect.effect_id.to_le_bytes());
        let value = if effect.effect_id == EMPTY_EFFECT_ID {
            0
        } else {
            effect.value
        };
        patched[offset + 4..offset + 8].copy_from_slice(&value.to_le_bytes());
        if let Some(roll) = effect.roll {
            patched[offset + 8] = roll;
        }
        if let Some(star) = effect.star {
            patched[offset + 0xA] = (patched[offset + 0xA] & !0x04) | if star { 0x04 } else { 0 };
        }
        // Slot normalization clears the group of an unused entry.
        let group = if effect.effect_id == EMPTY_EFFECT_ID {
            Some(0)
        } else {
            effect.group
        };
        if let Some(group) = group {
            patched[offset - 4..offset - 2].copy_from_slice(&group.to_le_bytes());
        }
        if let Some(category) = effect.category {
            patched[offset + 9] = (patched[offset + 9] & !0x3F) | (category & 0x3F);
        }
    }
    Ok(patched)
}

/// One item record (consumables, materials, books, key items).
pub const ITEM_RECORD_BYTES: usize = 0xE8;
/// Each record array in the save is preceded by `tag u32 | size + 4 u32 | size u32`.
pub const CONTAINER_HEADER_BYTES: usize = 0xC;
/// A record whose count spans `+4..+8` (u32) instead of `+4..+6` (u16).
pub const ITEM_WIDE_COUNT_FLAG: u32 = 0x20_0000;
/// A record the game always counts as one.
pub const ITEM_SINGLE_FLAG: u32 = 0x80_0000;

/// The two item arrays that follow the owned equipment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemContainer {
    /// 持有: what the character carries.
    Held,
    /// 仓库: the storehouse.
    Storage,
}

impl ItemContainer {
    pub const ALL: [Self; 2] = [Self::Held, Self::Storage];

    pub const fn slots(self) -> usize {
        match self {
            Self::Held => 1500,
            Self::Storage => 400,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Held => "held",
            Self::Storage => "storage",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|container| container.label() == label)
    }

    /// First record of the array in the decrypted save.
    pub const fn save_offset(self) -> usize {
        let held = EQUIPMENT_REGION_END + CONTAINER_HEADER_BYTES;
        match self {
            Self::Held => held,
            Self::Storage => held + Self::Held.slots() * ITEM_RECORD_BYTES + CONTAINER_HEADER_BYTES,
        }
    }
}

/// Check the size words in front of an item array, then return its bytes.
pub fn item_region(plain: &[u8], container: ItemContainer) -> Result<&[u8], SaveReadError> {
    let start = container.save_offset();
    let size = container.slots() * ITEM_RECORD_BYTES;
    let region =
        plain
            .get(start - 8..start + size)
            .ok_or(SaveReadError::InventoryRegionTruncated {
                needed: start + size,
                actual: plain.len(),
            })?;
    let declared = u32_at(region, 0) as usize;
    let payload = u32_at(region, 4) as usize;
    if declared != size + 4 || payload != size {
        return Err(SaveReadError::InvalidTransform {
            message: format!(
                "the {} item array header declares {payload:#x} bytes, not {size:#x}",
                container.label()
            ),
        });
    }
    Ok(&region[8..])
}

/// One item record.
pub fn item_record(
    plain: &[u8],
    container: ItemContainer,
    slot_index: usize,
) -> Result<&[u8], SaveReadError> {
    if slot_index >= container.slots() {
        return Err(SaveReadError::SlotIndex { index: slot_index });
    }
    let offset = slot_index * ITEM_RECORD_BYTES;
    Ok(&item_region(plain, container)?[offset..offset + ITEM_RECORD_BYTES])
}

/// The count the game shows, or `None` for a record it always counts as one.
pub fn item_quantity(record: &[u8]) -> Option<u32> {
    let flags = u32_at(record, 0x18);
    if flags & ITEM_SINGLE_FLAG != 0 {
        None
    } else if flags & ITEM_WIDE_COUNT_FLAG != 0 {
        Some(u32_at(record, 0x04))
    } else {
        Some(u32::from(u16_at(record, 0x04)))
    }
}

/// The largest count one record can hold.
pub fn item_quantity_limit(record: &[u8]) -> u32 {
    if u32_at(record, 0x18) & ITEM_WIDE_COUNT_FLAG != 0 {
        u32::MAX
    } else {
        u32::from(u16::MAX)
    }
}

/// A copy of `record` with a new count; nothing else changes.
pub fn patch_item_quantity(record: &[u8], quantity: u32) -> Result<Vec<u8>, SaveReadError> {
    if record.len() != ITEM_RECORD_BYTES {
        return Err(SaveReadError::RecordLength {
            expected: ITEM_RECORD_BYTES,
            actual: record.len(),
        });
    }
    if equipment_slot_is_empty(record) {
        return Err(SaveReadError::RecordTypeZero);
    }
    if item_quantity(record).is_none() || quantity > item_quantity_limit(record) {
        return Err(SaveReadError::InvalidTransform {
            message: format!("this item cannot hold a count of {quantity}"),
        });
    }
    let mut patched = record.to_vec();
    if item_quantity_limit(record) == u32::MAX {
        patched[0x04..0x08].copy_from_slice(&quantity.to_le_bytes());
    } else {
        patched[0x04..0x06].copy_from_slice(&(quantity as u16).to_le_bytes());
    }
    Ok(patched)
}

/// True when two item records differ only in their count bytes.
pub fn only_count_differs(original: &[u8], replacement: &[u8]) -> bool {
    original.len() == ITEM_RECORD_BYTES
        && replacement.len() == ITEM_RECORD_BYTES
        && original[..0x04] == replacement[..0x04]
        && original[0x08..] == replacement[0x08..]
}

/// One change of the character block, gated on the stored original.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum CharacterEdit {
    Currency {
        currency: Currency,
        expected: u64,
        replacement: u64,
    },
    Equipment {
        slot_index: usize,
        expected_original: Vec<u8>,
        replacement: Vec<u8>,
    },
    /// Free one occupied equipment slot as the game frees one, taking a worn
    /// item out of its sets; the free slot is built from the record found.
    RemoveEquipment {
        slot_index: usize,
        expected_original: Vec<u8>,
    },
    /// Write a new equipment record into a free slot and advance the save-wide
    /// key and serial counters past it, as the game does for a new item. The
    /// record was built for exactly `expected_free` and the counters `key` and
    /// `serial`; adds in one plan chain their counters in order.
    AddEquipment {
        slot_index: usize,
        expected_free: Vec<u8>,
        record: Vec<u8>,
        key: u32,
        serial: u32,
    },
    /// A count change of one item record; every other byte must stay.
    Item {
        container: ItemContainer,
        slot_index: usize,
        expected_original: Vec<u8>,
        replacement: Vec<u8>,
    },
}

impl CharacterEdit {
    fn describe(&self) -> String {
        match self {
            Self::Currency { currency, .. } => format!("currency {}", currency.label()),
            Self::Equipment { slot_index, .. }
            | Self::RemoveEquipment { slot_index, .. }
            | Self::AddEquipment { slot_index, .. } => format!("equipment slot {slot_index}"),
            Self::Item {
                container,
                slot_index,
                ..
            } => format!("{} item slot {slot_index}", container.label()),
        }
    }
}

/// Apply `edits` to a copy of `plain`, each gated on its expected original.
///
/// Every target may appear once. An equipment edit must replace an occupied
/// record with a record that still names an item; clearing a slot is
/// [`CharacterEdit::RemoveEquipment`] and creating into one is a separate
/// operation.
pub fn apply_character_edits(
    plain: &[u8],
    edits: &[CharacterEdit],
) -> Result<(Vec<u8>, Vec<usize>), SaveReadError> {
    if edits.is_empty() {
        return Err(SaveReadError::InvalidTransform {
            message: "at least one character edit is required".to_string(),
        });
    }
    let mut seen: Vec<String> = Vec::new();
    let mut edited = plain.to_vec();
    let mut slots = Vec::new();
    for edit in edits {
        let target = edit.describe();
        if seen.contains(&target) {
            return Err(SaveReadError::InvalidTransform {
                message: format!("{target} cannot be edited twice in one plan"),
            });
        }
        seen.push(target.clone());
        match edit {
            CharacterEdit::Currency {
                currency,
                expected,
                replacement,
            } => {
                let offset = currency_offset(plain, *currency)?;
                let current = read_currency(plain, *currency)?;
                if current != *expected {
                    return Err(SaveReadError::IntegrityMismatch {
                        path: target,
                        expected: expected.to_string(),
                        actual: current.to_string(),
                    });
                }
                edited[offset..offset + CURRENCY_WIDTH].copy_from_slice(&replacement.to_le_bytes());
            }
            CharacterEdit::Equipment {
                slot_index,
                expected_original,
                replacement,
            } => {
                if expected_original.len() != EQUIPMENT_RECORD_BYTES
                    || replacement.len() != EQUIPMENT_RECORD_BYTES
                {
                    return Err(SaveReadError::RecordLength {
                        expected: EQUIPMENT_RECORD_BYTES,
                        actual: expected_original.len().min(replacement.len()),
                    });
                }
                let current = equipment_record(plain, *slot_index)?;
                if equipment_slot_is_empty(current) {
                    return Err(SaveReadError::EmptyRecord {
                        slot_index: *slot_index,
                    });
                }
                if current != expected_original.as_slice() {
                    return Err(SaveReadError::IntegrityMismatch {
                        path: target,
                        expected: crate::save::sha256_hex(expected_original),
                        actual: crate::save::sha256_hex(current),
                    });
                }
                if equipment_slot_is_empty(replacement) {
                    return Err(SaveReadError::RecordTypeZero);
                }
                let offset = equipment_offset(*slot_index)
                    .ok_or(SaveReadError::SlotIndex { index: *slot_index })?;
                edited[offset..offset + EQUIPMENT_RECORD_BYTES].copy_from_slice(replacement);
                slots.push(*slot_index);
            }
            CharacterEdit::RemoveEquipment {
                slot_index,
                expected_original,
            } => {
                let current = equipment_record(plain, *slot_index)?;
                if equipment_slot_is_empty(current) {
                    return Err(SaveReadError::EmptyRecord {
                        slot_index: *slot_index,
                    });
                }
                if current != expected_original.as_slice() {
                    return Err(SaveReadError::IntegrityMismatch {
                        path: target,
                        expected: crate::save::sha256_hex(expected_original),
                        actual: crate::save::sha256_hex(current),
                    });
                }
                let freed = free_equipment_slot_unequipping(current)?;
                let offset = equipment_offset(*slot_index)
                    .ok_or(SaveReadError::SlotIndex { index: *slot_index })?;
                edited[offset..offset + EQUIPMENT_RECORD_BYTES].copy_from_slice(&freed);
                slots.push(*slot_index);
            }
            CharacterEdit::AddEquipment {
                slot_index,
                expected_free,
                record,
                key,
                serial,
            } => {
                // The slot and the counters are read from the plan so far, so
                // several adds take consecutive keys and serials.
                let current = equipment_record(&edited, *slot_index)?;
                if !equipment_slot_is_empty(current) {
                    return Err(SaveReadError::SlotOccupied {
                        slot_index: *slot_index,
                    });
                }
                if current != expected_free.as_slice() {
                    return Err(SaveReadError::IntegrityMismatch {
                        path: target,
                        expected: crate::save::sha256_hex(expected_free),
                        actual: crate::save::sha256_hex(current),
                    });
                }
                let counters = equipment_counters(&edited)?;
                if counters != (*key, *serial) {
                    return Err(SaveReadError::IntegrityMismatch {
                        path: "equipment key and serial counters".to_string(),
                        expected: format!("{key:#x}/{serial:#x}"),
                        actual: format!("{:#x}/{:#x}", counters.0, counters.1),
                    });
                }
                let stored = |offset: usize| {
                    record
                        .get(offset..offset + 4)
                        .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                };
                if record.len() != EQUIPMENT_RECORD_BYTES
                    || equipment_slot_is_empty(record)
                    || stored(0x1C) != Some(*key)
                    || stored(0x28) != Some(*serial)
                {
                    return Err(SaveReadError::InvalidTransform {
                        message: format!(
                            "{target}: the new record does not carry its key and serial"
                        ),
                    });
                }
                // The key counter holds a u16; never step it past what the game
                // itself can store.
                if *key == 0 || *key >= u32::from(u16::MAX) {
                    return Err(SaveReadError::AllocationExhausted {
                        kind: "inventory key",
                    });
                }
                let next_serial =
                    serial
                        .checked_add(1)
                        .ok_or(SaveReadError::AllocationExhausted {
                            kind: "generation serial",
                        })?;
                let offset = equipment_offset(*slot_index)
                    .ok_or(SaveReadError::SlotIndex { index: *slot_index })?;
                edited[offset..offset + EQUIPMENT_RECORD_BYTES].copy_from_slice(record);
                edited[NEXT_INVENTORY_KEY_OFFSET..NEXT_INVENTORY_KEY_OFFSET + 4]
                    .copy_from_slice(&(key + 1).to_le_bytes());
                edited[NEXT_GENERATION_SERIAL_OFFSET..NEXT_GENERATION_SERIAL_OFFSET + 4]
                    .copy_from_slice(&next_serial.to_le_bytes());
                slots.push(*slot_index);
            }
            CharacterEdit::Item {
                container,
                slot_index,
                expected_original,
                replacement,
            } => {
                let current = item_record(plain, *container, *slot_index)?;
                if equipment_slot_is_empty(current) {
                    return Err(SaveReadError::EmptyRecord {
                        slot_index: *slot_index,
                    });
                }
                if current != expected_original.as_slice() {
                    return Err(SaveReadError::IntegrityMismatch {
                        path: target,
                        expected: crate::save::sha256_hex(expected_original),
                        actual: crate::save::sha256_hex(current),
                    });
                }
                if !only_count_differs(current, replacement) {
                    return Err(SaveReadError::InvalidTransform {
                        message: format!("{target}: only the count may change"),
                    });
                }
                let offset = container.save_offset() + slot_index * ITEM_RECORD_BYTES;
                edited[offset..offset + ITEM_RECORD_BYTES].copy_from_slice(replacement);
            }
        }
    }
    Ok((edited, slots))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn plain_with(amrita: u64, gold: u64) -> Vec<u8> {
        let mut plain = vec![0u8; EQUIPMENT_REGION_END + 0x100];
        let mut at = EQUIPMENT_REGION_END + 0x10;
        for (key, value) in [
            (Currency::Amrita.key(), amrita),
            (Currency::Gold.key(), gold),
        ] {
            plain[at..at + 4].copy_from_slice(&key.to_le_bytes());
            plain[at + 4..at + 8].copy_from_slice(&8u32.to_le_bytes());
            plain[at + 8..at + 16].copy_from_slice(&value.to_le_bytes());
            at += 16 + 1;
        }
        let slot = equipment_offset(3).unwrap();
        plain[slot..slot + 2].copy_from_slice(&0x8D5Bu16.to_le_bytes());
        plain[slot + 6] = 175;
        plain
    }

    /// A forged 甲斐国江 captured live (PC v2.02); its UI showed 近距离攻击精力伤害
    /// （地狱） +1.5%, 陷入水状态时精华槽增加 D+, 赋予造成伤害增加 and 风林火山.
    fn forged_record() -> Vec<u8> {
        let hex = [
            "5b8d5b8d0100af00af00000000000040000000000000000082010000d7c70000",
            "0100c3dc0000000017802600000000000300000046548c3f66a100000f000000",
            "4092003f0000803f00000000b6a70000e302000002000000551900be00000000",
            "000000008a520000763500003a0000004e0300d10000000000000000a64f0000",
            "24d5000000000000004c0154000000000000000000000000ffffffff00000000",
            "00000054000000000000000000000000ffffffff00000000000000ba00000000",
            "0000000000000000ffffffff000000000000002d000000000000000000000000",
            "0000000000000000ffffffff03000000",
        ]
        .concat();
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn a_live_equipment_record_decodes_to_its_ui_fields() {
        let fields = equipment_fields(&forged_record()).unwrap();
        assert_eq!(fields.item_id, 0x8D5B);
        assert_eq!(fields.level, 175);
        assert_eq!(fields.plus, 0);
        assert_eq!(fields.rarity, 3);
        assert_eq!(fields.seed, 0xDCC3);
        assert_eq!(
            fields.effects[..4],
            [(0xA166, 15), (0x02E3, 2), (0x3576, 58), (0xD524, 0)]
        );
        assert!(fields.effects[4..]
            .iter()
            .all(|(id, _)| *id == EMPTY_EFFECT_ID));
    }

    #[test]
    fn an_equipment_patch_writes_only_the_named_fields() {
        let record = forged_record();
        let patch = EquipmentPatch {
            plus: Some(20),
            effects: vec![
                EquipmentEffectPatch {
                    index: 0,
                    effect_id: 0x8D2B,
                    value: 4,
                    roll: None,
                    star: None,
                    group: None,
                    category: None,
                },
                EquipmentEffectPatch {
                    index: 3,
                    effect_id: EMPTY_EFFECT_ID,
                    value: 9,
                    roll: None,
                    star: None,
                    group: None,
                    category: None,
                },
            ],
            ..EquipmentPatch::default()
        };
        let patched = patch_equipment(&record, &patch).unwrap();
        let fields = equipment_fields(&patched).unwrap();
        assert_eq!(fields.plus, 20);
        assert_eq!(fields.level, 175);
        assert_eq!(fields.effects[0], (0x8D2B, 4));
        assert_eq!(fields.effects[3], (EMPTY_EFFECT_ID, 0));
        let changed: Vec<usize> = (0..record.len())
            .filter(|index| record[*index] != patched[*index])
            .collect();
        // Clearing entry 3 also clears its group marker at 0x7C.
        assert!(changed.iter().all(|offset| (0x0A..0x0C).contains(offset)
            || (0x38..0x40).contains(offset)
            || (0x7C..0x7E).contains(offset)
            || (0x80..0x88).contains(offset)));
        assert_eq!(&patched[0x7C..0x7E], &[0, 0]);
        let repeated = EquipmentPatch {
            effects: vec![patch.effects[0], patch.effects[0]],
            ..EquipmentPatch::default()
        };
        assert!(patch_equipment(&record, &repeated).is_err());
    }

    #[test]
    fn a_patch_can_toggle_the_hell_marker_and_skill_only() {
        let record = forged_record();
        assert!(!equipment_fields(&record).unwrap().hell);
        let to_hell = EquipmentPatch {
            hell: Some(true),
            hell_skill: Some(0xC0C1),
            ..EquipmentPatch::default()
        };
        let hell = patch_equipment(&record, &to_hell).unwrap();
        let fields = equipment_fields(&hell).unwrap();
        assert!(fields.hell);
        assert_eq!(fields.hell_skill, 0xC0C1);
        let changed: Vec<usize> = (0..record.len())
            .filter(|i| record[*i] != hell[*i])
            .collect();
        assert!(changed
            .iter()
            .all(|offset| matches!(offset, 0x10 | 0x11 | 0x1A)));
        let back = EquipmentPatch {
            hell: Some(false),
            hell_skill: Some(0),
            ..EquipmentPatch::default()
        };
        assert_eq!(patch_equipment(&hell, &back).unwrap(), record);
    }

    #[test]
    fn an_effect_patch_can_set_the_roll_and_star_marker() {
        let record = forged_record();
        let with = |star: bool| EquipmentPatch {
            effects: vec![EquipmentEffectPatch {
                index: 1,
                effect_id: 0x010A,
                value: 180,
                roll: Some(84),
                star: Some(star),
                group: None,
                category: None,
            }],
            ..EquipmentPatch::default()
        };
        let starred = patch_equipment(&record, &with(true)).unwrap();
        // Entry 1 starts at 0x4C: id +4, value +8, roll +0xC, flags +0xE.
        assert_eq!(starred[0x4C + 0xC], 84);
        assert_eq!(starred[0x4C + 0xE] & 0x04, 0x04);
        assert_eq!(starred[0x4C + 0xE] & !0x04, record[0x4C + 0xE] & !0x04);
        let plain = patch_equipment(&starred, &with(false)).unwrap();
        assert_eq!(plain[0x4C + 0xE] & 0x04, 0);
    }

    #[test]
    fn an_effect_patch_writes_the_group_marker_and_category_bits() {
        let record = forged_record();
        let patch = EquipmentPatch {
            effects: vec![EquipmentEffectPatch {
                index: 1,
                effect_id: 0xD4F0,
                value: 126,
                roll: Some(86),
                star: Some(true),
                group: Some(0x766B),
                category: Some(0x1F),
            }],
            ..EquipmentPatch::default()
        };
        let patched = patch_equipment(&record, &patch).unwrap();
        // Entry 1 starts at 0x4C: group +0, category bits in +0xD.
        assert_eq!(&patched[0x4C..0x4E], &0x766Bu16.to_le_bytes());
        assert_eq!(patched[0x4C + 0xD] & 0x3F, 0x1F);
        assert_eq!(patched[0x4C + 0xD] & 0xC0, record[0x4C + 0xD] & 0xC0);
    }

    fn plain_with_items() -> Vec<u8> {
        let end = ItemContainer::Storage.save_offset()
            + ItemContainer::Storage.slots() * ITEM_RECORD_BYTES;
        let mut plain = vec![0u8; end + 0x10];
        for container in ItemContainer::ALL {
            let start = container.save_offset();
            let size = (container.slots() * ITEM_RECORD_BYTES) as u32;
            plain[start - 8..start - 4].copy_from_slice(&(size + 4).to_le_bytes());
            plain[start - 4..start].copy_from_slice(&size.to_le_bytes());
        }
        // 仙药 held x8 (wide count), 铁甲片 held 623127, 仙药 stored x6734.
        let put = |plain: &mut Vec<u8>,
                   container: ItemContainer,
                   slot: usize,
                   id: u16,
                   count: u32,
                   flags: u32| {
            let at = container.save_offset() + slot * ITEM_RECORD_BYTES;
            plain[at..at + 2].copy_from_slice(&id.to_le_bytes());
            plain[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
            plain[at + 0x18..at + 0x1C].copy_from_slice(&flags.to_le_bytes());
            plain[at + 0x40] = 0x5A;
        };
        put(&mut plain, ItemContainer::Held, 0, 0x05E7, 8, 0x20_0000);
        put(
            &mut plain,
            ItemContainer::Held,
            86,
            0xCE16,
            623_127,
            0x20_0000,
        );
        put(
            &mut plain,
            ItemContainer::Storage,
            2,
            0x05E7,
            6734,
            0x20_0002,
        );
        put(&mut plain, ItemContainer::Held, 5, 0x1234, 1, 0x80_0000);
        plain
    }

    #[test]
    fn item_arrays_follow_the_equipment_and_carry_counts() {
        assert_eq!(ItemContainer::Held.save_offset(), 0x30_2832);
        assert_eq!(ItemContainer::Storage.save_offset(), 0x35_779E);
        let plain = plain_with_items();
        let held = item_record(&plain, ItemContainer::Held, 86).unwrap();
        assert_eq!(item_quantity(held), Some(623_127));
        let stored = item_record(&plain, ItemContainer::Storage, 2).unwrap();
        assert_eq!(item_quantity(stored), Some(6734));
        let single = item_record(&plain, ItemContainer::Held, 5).unwrap();
        assert_eq!(item_quantity(single), None);
        assert!(patch_item_quantity(single, 3).is_err());
        let mut broken = plain.clone();
        let start = ItemContainer::Storage.save_offset();
        broken[start - 4] ^= 1;
        assert!(item_record(&broken, ItemContainer::Storage, 2).is_err());
    }

    #[test]
    fn item_edits_change_only_the_count() {
        let plain = plain_with_items();
        let original = item_record(&plain, ItemContainer::Storage, 2)
            .unwrap()
            .to_vec();
        let replacement = patch_item_quantity(&original, 9999).unwrap();
        let edit = CharacterEdit::Item {
            container: ItemContainer::Storage,
            slot_index: 2,
            expected_original: original.clone(),
            replacement: replacement.clone(),
        };
        let (edited, _) = apply_character_edits(&plain, &[edit]).unwrap();
        let after = item_record(&edited, ItemContainer::Storage, 2).unwrap();
        assert_eq!(item_quantity(after), Some(9999));
        assert_eq!(
            item_record(&edited, ItemContainer::Held, 0).unwrap(),
            item_record(&plain, ItemContainer::Held, 0).unwrap()
        );

        let mut sneaky = replacement.clone();
        sneaky[0] = 0x99;
        let rejected = CharacterEdit::Item {
            container: ItemContainer::Storage,
            slot_index: 2,
            expected_original: original.clone(),
            replacement: sneaky,
        };
        assert!(apply_character_edits(&plain, &[rejected]).is_err());
        let stale = CharacterEdit::Item {
            container: ItemContainer::Storage,
            slot_index: 2,
            expected_original: replacement.clone(),
            replacement,
        };
        assert!(apply_character_edits(&plain, &[stale]).is_err());
    }

    #[test]
    fn currencies_are_read_by_key() {
        let plain = plain_with(0, 19_072_714);
        assert_eq!(read_currency(&plain, Currency::Amrita).unwrap(), 0);
        assert_eq!(read_currency(&plain, Currency::Gold).unwrap(), 19_072_714);
    }

    #[test]
    fn a_duplicated_or_missing_key_fails_closed() {
        let mut plain = plain_with(1, 2);
        assert!(tagged_value_offset(&plain, 0xDEAD_BEEF, 8).is_err());
        let copy = EQUIPMENT_REGION_END + 0x60;
        plain[copy..copy + 4].copy_from_slice(&Currency::Gold.key().to_le_bytes());
        plain[copy + 4..copy + 8].copy_from_slice(&8u32.to_le_bytes());
        assert!(read_currency(&plain, Currency::Gold).is_err());
        assert_eq!(read_currency(&plain, Currency::Amrita).unwrap(), 1);
    }

    #[test]
    fn edits_are_gated_on_the_stored_original() {
        let plain = plain_with(0, 5);
        let stale = CharacterEdit::Currency {
            currency: Currency::Gold,
            expected: 4,
            replacement: 9,
        };
        assert!(apply_character_edits(&plain, &[stale]).is_err());
        let fresh = CharacterEdit::Currency {
            currency: Currency::Gold,
            expected: 5,
            replacement: 9,
        };
        let (edited, slots) = apply_character_edits(&plain, &[fresh]).unwrap();
        assert_eq!(read_currency(&edited, Currency::Gold).unwrap(), 9);
        assert!(slots.is_empty());
        let changed = plain.iter().zip(&edited).filter(|(a, b)| a != b).count();
        assert!(changed <= 8);
    }

    #[test]
    fn equipment_edits_replace_only_occupied_records() {
        let plain = plain_with(0, 0);
        let original = equipment_record(&plain, 3).unwrap().to_vec();
        let mut replacement = original.clone();
        replacement[0x0A] = 20;
        let edit = CharacterEdit::Equipment {
            slot_index: 3,
            expected_original: original.clone(),
            replacement: replacement.clone(),
        };
        let (edited, slots) = apply_character_edits(&plain, std::slice::from_ref(&edit)).unwrap();
        assert_eq!(
            equipment_record(&edited, 3).unwrap(),
            replacement.as_slice()
        );
        assert_eq!(slots, vec![3]);
        assert!(apply_character_edits(&plain, &[edit.clone(), edit]).is_err());

        let empty = CharacterEdit::Equipment {
            slot_index: 4,
            expected_original: equipment_record(&plain, 4).unwrap().to_vec(),
            replacement: replacement.clone(),
        };
        assert!(apply_character_edits(&plain, &[empty]).is_err());
        let mut cleared = replacement;
        cleared[0] = 0;
        cleared[1] = 0;
        let clearing = CharacterEdit::Equipment {
            slot_index: 3,
            expected_original: original,
            replacement: cleared,
        };
        assert!(apply_character_edits(&plain, &[clearing]).is_err());

        let removal = CharacterEdit::RemoveEquipment {
            slot_index: 3,
            expected_original: equipment_record(&plain, 3).unwrap().to_vec(),
        };
        let (removed, slots) =
            apply_character_edits(&plain, std::slice::from_ref(&removal)).unwrap();
        assert!(equipment_slot_is_empty(
            equipment_record(&removed, 3).unwrap()
        ));
        assert_eq!(slots, vec![3]);
        let stale = CharacterEdit::RemoveEquipment {
            slot_index: 3,
            expected_original: vec![0; EQUIPMENT_RECORD_BYTES],
        };
        assert!(apply_character_edits(&plain, &[stale]).is_err());
        let free = CharacterEdit::RemoveEquipment {
            slot_index: 4,
            expected_original: equipment_record(&plain, 4).unwrap().to_vec(),
        };
        assert!(apply_character_edits(&plain, &[free]).is_err());
        assert!(apply_character_edits(&plain, &[removal, edit_again(&plain)]).is_err());
    }

    fn edit_again(plain: &[u8]) -> CharacterEdit {
        let original = equipment_record(plain, 3).unwrap().to_vec();
        let mut replacement = original.clone();
        replacement[0x0A] = 7;
        CharacterEdit::Equipment {
            slot_index: 3,
            expected_original: original,
            replacement,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod free_slot_tests {
    use super::*;

    /// A free slot of a live PC v2.02 inventory, as the game left it.
    const LIVE_FREE_SLOT: &str = "00000000000000000000000000000040000000000000000002000000000000000000000000000000ffffffffffffffffff00000000000000ffffffff000000000000000f000000000000000000000000ffffffff0000000000000000000000000000000000000000ffffffff000000000000000f000000000000000000000000ffffffff0000000000000041000000000000000000000000ffffffff00000000000000ef000000000000000000000000ffffffff000000000000000f000000000000000000000000ffffffff000000000000003f0000a63f000000000000000000000000000000001100000011000000";

    fn live_free_slot() -> Vec<u8> {
        (0..LIVE_FREE_SLOT.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&LIVE_FREE_SLOT[index..index + 2], 16).unwrap())
            .collect()
    }

    /// The slot with an item in it: every byte the game clears is filled.
    fn occupied_from(free: &[u8]) -> Vec<u8> {
        let mut record = free.to_vec();
        record[0..0x28].copy_from_slice(&[0x5A; 0x28]);
        record[0x28..0x34].copy_from_slice(&[0x07; 0x0C]);
        for index in 0..EQUIPMENT_EFFECT_COUNT {
            let entry = 0x34 + index * EQUIPMENT_EFFECT_STRIDE;
            record[entry..entry + 2].copy_from_slice(&[0x12, 0x34]);
            record[entry + 4..entry + 0xE].copy_from_slice(&[0x21; 0x0A]);
            record[entry + 0x10..entry + 0x12].copy_from_slice(&[0x43, 0x65]);
            record[entry + 0x14..entry + 0x18].copy_from_slice(&[0x66; 4]);
        }
        record[0xDC..0xE8].copy_from_slice(&[0x77; 0x0C]);
        record
    }

    #[test]
    fn a_freed_record_is_the_slot_the_game_leaves() {
        let free = live_free_slot();
        let occupied = occupied_from(&free);
        assert!(!equipment_slot_is_empty(&occupied));
        assert!(!equipment_is_worn(&occupied));
        assert_eq!(free_equipment_slot(&occupied).unwrap(), free);
        assert!(
            free_equipment_slot(&free).is_err(),
            "a free slot is not freed again"
        );
    }

    #[test]
    fn a_worn_record_is_refused() {
        for offset in EQUIPMENT_SET_OFFSETS {
            let mut worn = occupied_from(&live_free_slot());
            worn[offset..offset + 4].copy_from_slice(&4u32.to_le_bytes());
            assert!(equipment_is_worn(&worn));
            assert!(equipment_fields(&worn).unwrap().worn);
            assert!(free_equipment_slot(&worn).is_err());
        }
    }

    #[test]
    fn a_save_removal_takes_a_worn_record_out_of_its_sets() {
        let free = live_free_slot();
        let mut worn = occupied_from(&free);
        worn[0xE8..0xEC].copy_from_slice(&4u32.to_le_bytes());
        assert_eq!(free_equipment_slot_unequipping(&worn).unwrap(), free);
        assert!(!equipment_is_worn(
            &free_equipment_slot_unequipping(&worn).unwrap()
        ));
        assert!(free_equipment_slot_unequipping(&free).is_err());
    }

    /// PC v2.02, owner's save: slot 1464 before and after one blacksmith
    /// purchase of 木刀 (`0x27BF`), key `0xC86F`, serial `0x26B007`.
    const PURCHASE_FREE: &str = "00000000000000000000000000000040000000000000000002000000000000000000000000000000ffffffffffffffffff00000000000000ffffffff000000000000000f000000000000000000000000ffffffff0000000000000000000000000000000000000000ffffffff000000000000000f000000000000000000000000ffffffff0000000000000041000000000000000000000000ffffffff00000000000000ef000000000000000000000000ffffffff000000000000000f000000000000000000000000ffffffff000000000000003f0000a63f000000000000000000000000000000001100000011000000";
    const PURCHASE_RECORD: &str = "bf27bf270100af00af000000000000400000000000000000800100006fc800000100affc0000000007b026000000000003000000093c0000f16800002a0000004f8300d50000000000000000d0a78c3f8915000050000000419f003f0000803f00000000a9610000e51600000b000000530900a0000000000000000000000000ffffffff00000000000000f4000000000000000000000000ffffffff00000000000000d5000000000000000000000000ffffffff000000000000009c000000000000000000000000ffffffff00000000000000ae00000000000000000000000000000000000000001100000011000000";

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
            .collect()
    }

    fn purchased_item() -> NewEquipment {
        let random =
            |group: u16, effect_id: u32, value: u32, roll: u8, category: u8| NewEquipmentEffect {
                role: NewEntryRole::Random,
                effect_id,
                value,
                roll,
                star: false,
                group,
                category,
            };
        NewEquipment {
            item_id: 0x27BF,
            level: 0xAF,
            plus: 0,
            rarity: 3,
            hell: false,
            hell_skill: 0,
            effects: vec![
                random(0x3C09, 0x68F1, 42, 79, 0x03),
                random(0xA7D0, 0x1589, 80, 65, 0x1F),
                random(0x61A9, 0x16E5, 11, 83, 0x09),
            ],
        }
    }

    #[test]
    fn a_new_record_matches_the_one_the_game_wrote_for_a_purchase() {
        let free = unhex(PURCHASE_FREE);
        let game = unhex(PURCHASE_RECORD);
        let built = build_equipment_record(&free, &purchased_item(), 0xC86F, 0x26B007).unwrap();
        // Bytes the game fills from state this tool does not model: the
        // purchase flag (0x180 against a drop's 0x82), the seed, and per entry
        // the second group word, the forge-material marker bits, the
        // undefined +0x0F byte and the optional scaled fields +0x10..+0x17.
        let mut unmodelled = vec![0x18, 0x19, 0x22, 0x23];
        for index in 0..EQUIPMENT_EFFECT_COUNT {
            let entry = EQUIPMENT_EFFECT_ID_OFFSET - 4 + index * EQUIPMENT_EFFECT_STRIDE;
            unmodelled.extend([entry + 2, entry + 3, entry + 0x0F]);
            unmodelled.extend(entry + 0x10..entry + EQUIPMENT_EFFECT_STRIDE);
        }
        for (offset, (ours, theirs)) in built.iter().zip(&game).enumerate() {
            if unmodelled.contains(&offset) {
                continue;
            }
            let mask =
                if (0x34..0x34 + 7 * 0x18).contains(&offset) && (offset - 0x34) % 0x18 == 0x0D {
                    0x3F
                } else {
                    0xFF
                };
            assert_eq!(ours & mask, theirs & mask, "byte {offset:#x}");
        }
        let fields = equipment_fields(&built).unwrap();
        assert_eq!(
            (
                fields.item_id,
                fields.level,
                fields.rarity,
                fields.inventory_key
            ),
            (0x27BF, 0xAF, 3, 0xC86F)
        );
        assert!(!fields.worn);
    }

    fn plain_with_counters(key: u32, serial: u32) -> Vec<u8> {
        let mut plain = vec![0u8; NEXT_GENERATION_SERIAL_OFFSET + 0x10];
        let free = unhex(PURCHASE_FREE);
        for slot in 0..4 {
            let at = equipment_offset(slot).unwrap();
            plain[at..at + EQUIPMENT_RECORD_BYTES].copy_from_slice(&free);
        }
        let at = equipment_offset(1).unwrap();
        plain[at..at + EQUIPMENT_RECORD_BYTES].copy_from_slice(&unhex(PURCHASE_RECORD));
        plain[NEXT_INVENTORY_KEY_OFFSET..NEXT_INVENTORY_KEY_OFFSET + 4]
            .copy_from_slice(&key.to_le_bytes());
        plain[NEXT_GENERATION_SERIAL_OFFSET..NEXT_GENERATION_SERIAL_OFFSET + 4]
            .copy_from_slice(&serial.to_le_bytes());
        plain
    }

    #[test]
    fn new_items_go_after_the_occupied_tail_and_take_consecutive_counters() {
        let plain = plain_with_counters(0x100, 0x5000);
        assert_eq!(equipment_counters(&plain).unwrap(), (0x100, 0x5000));
        let first = next_free_equipment_slot(&plain, &[]).unwrap();
        assert_eq!(first, 2, "the first free slot after the occupied tail");
        let second = next_free_equipment_slot(&plain, &[first]).unwrap();
        assert_eq!(second, 3);
        let add = |slot: usize, key: u32, serial: u32| CharacterEdit::AddEquipment {
            slot_index: slot,
            expected_free: equipment_record(&plain, slot).unwrap().to_vec(),
            record: build_equipment_record(&unhex(PURCHASE_FREE), &purchased_item(), key, serial)
                .unwrap(),
            key,
            serial,
        };
        let (edited, slots) = apply_character_edits(
            &plain,
            &[add(first, 0x100, 0x5000), add(second, 0x101, 0x5001)],
        )
        .unwrap();
        assert_eq!(slots, vec![first, second]);
        assert_eq!(equipment_counters(&edited).unwrap(), (0x102, 0x5002));
        assert_eq!(
            equipment_fields(equipment_record(&edited, second).unwrap())
                .unwrap()
                .inventory_key,
            0x101
        );
        // Nothing outside the two slots and the counters changed.
        for (offset, (before, after)) in plain.iter().zip(&edited).enumerate() {
            let inside = [first, second].iter().any(|slot| {
                let at = equipment_offset(*slot).unwrap();
                (at..at + EQUIPMENT_RECORD_BYTES).contains(&offset)
            }) || (NEXT_INVENTORY_KEY_OFFSET..NEXT_INVENTORY_KEY_OFFSET + 4)
                .contains(&offset)
                || (NEXT_GENERATION_SERIAL_OFFSET..NEXT_GENERATION_SERIAL_OFFSET + 4)
                    .contains(&offset);
            if !inside {
                assert_eq!(before, after, "byte {offset:#x}");
            }
        }
        // A plan built for other counters, an occupied slot, or a repeat is refused.
        assert!(apply_character_edits(&plain, &[add(first, 0x101, 0x5001)]).is_err());
        assert!(apply_character_edits(&plain, &[add(1, 0x100, 0x5000)]).is_err());
        assert!(apply_character_edits(
            &plain,
            &[add(first, 0x100, 0x5000), add(first, 0x101, 0x5001)]
        )
        .is_err());
        assert!(
            apply_character_edits(&plain_with_counters(0xFFFF, 1), &[add(first, 0xFFFF, 1)])
                .is_err()
        );
    }
}
