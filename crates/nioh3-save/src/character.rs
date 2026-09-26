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
    }
    Ok(patched)
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
}

impl CharacterEdit {
    fn describe(&self) -> String {
        match self {
            Self::Currency { currency, .. } => format!("currency {}", currency.label()),
            Self::Equipment { slot_index, .. } => format!("equipment slot {slot_index}"),
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
                },
                EquipmentEffectPatch {
                    index: 3,
                    effect_id: EMPTY_EFFECT_ID,
                    value: 9,
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
