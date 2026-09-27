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
    /// The hell-weapon marker (`+0x1A` bit `0x10`).
    pub hell: bool,
    /// The hell martial skill (`+0x10`).
    pub hell_skill: u16,
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
        effects: (0..EQUIPMENT_EFFECT_COUNT)
            .map(|index| {
                let offset = effect_offset(index);
                (u32_at(record, offset), u32_at(record, offset + 4))
            })
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
            Self::Equipment { slot_index, .. } => format!("equipment slot {slot_index}"),
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
/// record with a record that still names an item; creating into or clearing a
/// slot is a separate operation.
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
                },
                EquipmentEffectPatch {
                    index: 3,
                    effect_id: EMPTY_EFFECT_ID,
                    value: 9,
                    roll: None,
                    star: None,
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
        assert!(changed.iter().all(|offset| (0x0A..0x0C).contains(offset)
            || (0x38..0x40).contains(offset)
            || (0x80..0x88).contains(offset)));
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
    }
}
