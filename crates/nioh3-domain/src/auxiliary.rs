//! Special-rule selection and terrain-display decoding for the offline NG3
//! preview slice.
//!
//! Direct port of `nioh3_scroll_editor/auxiliary_generation.py`
//! (`generate_special_rules`, `describe_special_rule`,
//! `terrain_display_effect_keys_for_row`) for PC v2.00.02. Resource loading
//! stays in `nioh3-data`; only header-stripped native rows cross this boundary.
//!
//! Contract notes:
//!
//! - The rule generator is a three-attempt weighted lottery over the native
//!   row order, with the running total kept in `u16` wrap-around. Selection
//!   keys come from the row's own `+0x20` field; `describe_special_rule`
//!   resolves a key through the captured native hash index, so a row whose two
//!   identities disagree fails closed exactly like the reference.
//! - Scratch keys carried by the selected enemy rows block a rule even though
//!   they are never displayed. Ignoring them would change later slots.
//! - Per-playthrough weights live at `+0x22 + 2 * playthrough` and a row is
//!   only eligible when `+0x36` bit 0 is set.
//! - `display_value` is an `f64`: the reference divides an unpacked `f32` by
//!   the `f64` literal `10.0`/`60.0`, and re-rounding to binary32 would drift.
//! - The generated terrain magnitude and the row-index helper deliberately
//!   differ: the native generator uses the filtered row's `+0x30` field when
//!   the context filters the pool and the hash key's low byte otherwise.

use crate::enemy::EnemyError;
use crate::rng::LcgStream;
use crate::sequence::random_int;

/// Display key emitted when a terrain row carries a nonzero `+0x2C` value.
pub const TERRAIN_DISPLAY_CRUCIBLE_KEY: u16 = 0x0024;
/// Native enum values that append a second hard-coded display key.
pub const TERRAIN_DISPLAY_SPECIAL_KEYS: [(u8, u16); 3] =
    [(0x2D, 0x0039), (0xD8, 0x0058), (0x08, 0x039F)];
/// Width of one `scroll_special_rule` row.
pub const SPECIAL_RULE_ROW_BYTES: usize = 56;
/// Width of one `auxiliary_rule_conflict` row.
pub const RULE_CONFLICT_ROW_BYTES: usize = 24;
/// The three rows tagged with this group key display as C, B, and A.
pub const SPECIAL_RULE_GRADE_GROUP_KEY: u16 = 0x2FC9;

const RULE_ENEMY_KEY_OFFSET: usize = 0x04;
const RULE_QUALIFIER_TEXT_OFFSET: usize = 0x08;
const RULE_COST_OFFSET: usize = 0x14;
const RULE_VALUE_OFFSET: usize = 0x18;
const RULE_PERCENT_VALUE_OFFSET: usize = 0x1C;
const RULE_KEY_OFFSET: usize = 0x20;
/// First per-playthrough weight; the reference reads `+0x20 + 2 * playthrough`.
const RULE_WEIGHT_BASE_OFFSET: usize = 0x20;
const RULE_GROUP_KEY_OFFSET: usize = 0x2C;
const RULE_GROUP_KEY_COUNT: usize = 2;
const RULE_ITEM_KEY_OFFSET: usize = 0x30;
const RULE_EFFECT_KEY_OFFSET: usize = 0x32;
const RULE_ENABLED_OFFSET: usize = 0x36;
const RULE_ENABLED_BIT: u8 = 0x01;

const CONFLICT_GROUP_IDENTITY_OFFSET: usize = 0x08;
const CONFLICT_GROUP_ENABLED_OFFSET: usize = 0x0C;
const CONFLICT_GROUP_ENABLED_BIT: u8 = 0x01;

/// Native `scroll_special_rule` rows plus their hash index and conflict rows.
#[derive(Debug, Clone)]
pub struct SpecialRuleTables {
    pub rules: Vec<[u8; SPECIAL_RULE_ROW_BYTES]>,
    pub rule_keys: Vec<u16>,
    pub conflicts: Vec<[u8; RULE_CONFLICT_ROW_BYTES]>,
    pub conflict_keys: Vec<u16>,
}

/// One selected rule with the scroll-detail display fields.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecialRuleEntry {
    pub key: u16,
    pub row_index: usize,
    pub raw_value: Option<f32>,
    pub display_value: Option<f64>,
    pub display_unit: Option<&'static str>,
    pub display_grade: Option<&'static str>,
    pub value_source_offset: Option<usize>,
    pub qualifier_kind: Option<&'static str>,
    pub qualifier_key: Option<u32>,
}

/// The three ordered rule keys the native descriptor builder emits.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecialRuleResult {
    pub keys: [u16; 3],
    pub entries: Vec<SpecialRuleEntry>,
    pub scoped_seed: u32,
    pub target_budget: u8,
    pub random_draws: u64,
}

/// Scoped rule seed installed at native RVA 0x10288A3..0x10288F2.
pub fn derive_special_rule_seed(displayed_seed: u32) -> u32 {
    displayed_seed & 0x0FFF_FFFF
}

/// `legal_special_rule_keys`: the rule keys the native generator can select.
///
/// The captured table keeps disabled placeholders and rows whose weight is zero
/// for one progression. Those rows stay research evidence, but offering them in
/// the product picker would promise a rule that can never be generated. Keys
/// come from the captured hash index, matching the reference, so every key this
/// returns is also resolvable by [`describe_special_rule`].
pub fn legal_special_rule_keys(
    playthrough: u8,
    tables: &SpecialRuleTables,
) -> Result<std::collections::BTreeSet<u16>, EnemyError> {
    if !(1..=5).contains(&playthrough) {
        return Err(EnemyError::InvalidInput(format!(
            "playthrough {playthrough} is outside 1..=5"
        )));
    }
    if tables.rule_keys.len() != tables.rules.len() {
        return Err(EnemyError::MissingData(
            "special-rule key index does not match row count".into(),
        ));
    }
    let weight_offset = RULE_WEIGHT_BASE_OFFSET + usize::from(playthrough) * 2;
    Ok(tables
        .rule_keys
        .iter()
        .zip(tables.rules.iter())
        .filter(|(_, row)| row[RULE_ENABLED_OFFSET] & RULE_ENABLED_BIT != 0)
        .filter(|(_, row)| read_u16(&row[..], weight_offset) > 0)
        .map(|(key, _)| *key)
        .collect())
}

/// `generate_special_rules`: native RVA 0x1028880.
///
/// `scratch_rule_keys` are the rule keys carried by the enemy rows that were
/// already accepted for this Seed; they stay blocked for every attempt.
pub fn generate_special_rules(
    displayed_seed: u32,
    playthrough: u8,
    scratch_rule_keys: &[u16],
    tables: &SpecialRuleTables,
) -> Result<SpecialRuleResult, EnemyError> {
    if !(1..=5).contains(&playthrough) {
        return Err(EnemyError::InvalidInput(format!(
            "playthrough {playthrough} is outside 1..=5"
        )));
    }
    if tables.rule_keys.len() != tables.rules.len() {
        return Err(EnemyError::MissingData(
            "special-rule key index does not match row count".into(),
        ));
    }
    if tables.conflict_keys.len() != tables.conflicts.len() {
        return Err(EnemyError::MissingData(
            "rule-conflict key index does not match row count".into(),
        ));
    }

    let mut conflicts: std::collections::BTreeMap<u16, &[u8; RULE_CONFLICT_ROW_BYTES]> =
        std::collections::BTreeMap::new();
    for (key, row) in tables.conflict_keys.iter().zip(tables.conflicts.iter()) {
        conflicts.insert(*key, row);
    }
    let blocked: std::collections::BTreeSet<u16> = scratch_rule_keys.iter().copied().collect();

    let scoped_seed = derive_special_rule_seed(displayed_seed);
    let mut rng = LcgStream::new(scoped_seed);
    let target_budget = random_int(&mut rng, 5)
        .ok_or_else(|| EnemyError::InvalidInput("rule budget span is zero".into()))?
        .wrapping_add(1) as u8;
    let mut draws = 1u64;
    let original_budget = target_budget as f32;
    let mut remaining = original_budget;
    let mut selected_keys: Vec<u16> = Vec::new();
    let mut selected_rows: Vec<&[u8; SPECIAL_RULE_ROW_BYTES]> = Vec::new();
    let mut zero_selected = false;
    let mut third_slot_best_abs = 0.0f32;

    for _attempt in 0..3 {
        let mut candidates: Vec<(u16, &[u8; SPECIAL_RULE_ROW_BYTES], u32)> = Vec::new();
        let mut total_weight: u32 = 0;
        let accepted_count = selected_keys.len();

        for row in tables.rules.iter() {
            if row[RULE_ENABLED_OFFSET] & RULE_ENABLED_BIT == 0 {
                continue;
            }
            // The native generator reads the key from the row itself; the
            // captured hash index is only a lookup index, so the two are kept
            // separate exactly like the reference. `describe_special_rule`
            // resolves through the index and therefore fails closed if a row
            // ever disagrees with it.
            let key = read_u16(row, RULE_KEY_OFFSET);
            if blocked.contains(&key) {
                continue;
            }

            if accepted_count > 0 {
                if key == 0 {
                    if zero_selected {
                        continue;
                    }
                } else {
                    if selected_keys.contains(&key) {
                        continue;
                    }
                    if selected_rows
                        .iter()
                        .any(|previous| rule_rows_conflict(row, previous, &conflicts))
                    {
                        continue;
                    }
                }
            }

            let cost = read_f32(row, RULE_COST_OFFSET);
            if accepted_count == 1 {
                let accumulated_delta = remaining - original_budget;
                if accumulated_delta < 0.0 && cost >= 0.0 {
                    continue;
                }
                if accumulated_delta > 0.0 && cost <= 0.0 {
                    continue;
                }
            } else if accepted_count == 2 {
                if remaining < 0.0 && cost > 0.0 {
                    continue;
                }
                if remaining > 0.0 && cost < 0.0 {
                    continue;
                }
                let absolute_cost = cost.abs();
                if absolute_cost > remaining.abs() {
                    continue;
                }
                if absolute_cost < third_slot_best_abs {
                    continue;
                }
                if absolute_cost > third_slot_best_abs {
                    candidates.clear();
                    total_weight = 0;
                    third_slot_best_abs = absolute_cost;
                }
            }

            let weight = u32::from(read_u16(
                row,
                RULE_WEIGHT_BASE_OFFSET + usize::from(playthrough) * 2,
            ));
            candidates.push((key, row, weight));
            total_weight = total_weight.wrapping_add(weight) & 0xFFFF;
        }

        if total_weight == 0 {
            break;
        }

        let mut ticket = random_int(&mut rng, total_weight)
            .ok_or_else(|| EnemyError::InvalidInput("rule lottery span is zero".into()))?;
        draws += 1;
        let mut chosen: Option<(u16, &[u8; SPECIAL_RULE_ROW_BYTES], f32)> = None;
        for (key, row, weight) in candidates {
            if ticket < weight {
                chosen = Some((key, row, read_f32(row, RULE_COST_OFFSET)));
                break;
            }
            ticket -= weight;
        }
        let (key, row, cost) = chosen.ok_or_else(|| {
            EnemyError::InvalidInput("special-rule weighted lottery had no winner".into())
        })?;

        selected_keys.push(key);
        selected_rows.push(row);
        if key == 0 {
            zero_selected = true;
        }
        remaining -= cost;
        if remaining == 0.0 {
            break;
        }
    }

    let compacted: Vec<u16> = selected_keys
        .iter()
        .copied()
        .filter(|key| *key != 0)
        .collect();
    let mut keys = [0u16; 3];
    for (index, key) in compacted.iter().take(3).enumerate() {
        keys[index] = *key;
    }
    let mut entries = Vec::new();
    for key in keys.iter().filter(|key| **key != 0) {
        entries.push(describe_special_rule(*key, tables)?);
    }

    Ok(SpecialRuleResult {
        keys,
        entries,
        scoped_seed,
        target_budget,
        random_draws: draws,
    })
}

/// `describe_special_rule`: decode the scroll-detail UI value path for a key.
pub fn describe_special_rule(
    key: u16,
    tables: &SpecialRuleTables,
) -> Result<SpecialRuleEntry, EnemyError> {
    let mut matches = tables
        .rule_keys
        .iter()
        .zip(tables.rules.iter())
        .enumerate()
        .filter(|(_, (row_key, _))| **row_key == key);
    let (row_index, (_, row)) = matches.next().ok_or_else(|| {
        EnemyError::MissingData(format!("special-rule key 0x{key:04X} is unknown"))
    })?;
    if matches.next().is_some() {
        return Err(EnemyError::MissingData(format!(
            "special-rule key 0x{key:04X} resolved to multiple rows"
        )));
    }

    let qualifier_text_id = read_u32(row, RULE_QUALIFIER_TEXT_OFFSET);
    let enemy_key = read_u32(row, RULE_ENEMY_KEY_OFFSET);
    let item_key = read_u16(row, RULE_ITEM_KEY_OFFSET);
    let effect_key = read_u16(row, RULE_EFFECT_KEY_OFFSET);
    let (qualifier_kind, qualifier_key) = if qualifier_text_id != 0 {
        (Some("text"), Some(qualifier_text_id))
    } else if item_key != 0 {
        (Some("item"), Some(u32::from(item_key)))
    } else if enemy_key != 0 {
        (Some("enemy"), Some(enemy_key))
    } else if effect_key != 0 {
        (Some("effect"), Some(u32::from(effect_key)))
    } else {
        (None, None)
    };

    let group_key = read_u16(row, RULE_GROUP_KEY_OFFSET);
    if group_key == SPECIAL_RULE_GRADE_GROUP_KEY {
        let mut ordered_values: Vec<f32> = tables
            .rules
            .iter()
            .filter(|candidate| read_u16(*candidate, RULE_GROUP_KEY_OFFSET) == group_key)
            .map(|candidate| read_f32(candidate, RULE_VALUE_OFFSET))
            .collect();
        ordered_values
            .sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        let raw_value = read_f32(row, RULE_VALUE_OFFSET);
        let rank = ordered_values
            .iter()
            .position(|value| *value == raw_value)
            .ok_or_else(|| {
                EnemyError::MissingData(format!(
                    "graded special-rule key 0x{key:04X} has no ordered rank"
                ))
            })?;
        let grade = match rank {
            1 => "B",
            2 => "A",
            _ => "C",
        };
        return Ok(SpecialRuleEntry {
            key,
            row_index,
            raw_value: Some(raw_value),
            display_value: None,
            display_unit: Some("grade"),
            display_grade: Some(grade),
            value_source_offset: Some(RULE_VALUE_OFFSET),
            qualifier_kind,
            qualifier_key,
        });
    }

    let (source_offset, raw_value, display_value, display_unit) = if qualifier_text_id != 0 {
        let raw = read_f32(row, RULE_PERCENT_VALUE_OFFSET);
        (
            Some(RULE_PERCENT_VALUE_OFFSET),
            Some(raw),
            Some(f64::from(raw) / 10.0),
            Some("percent"),
        )
    } else if item_key != 0 {
        let raw = read_f32(row, RULE_VALUE_OFFSET);
        (
            Some(RULE_VALUE_OFFSET),
            Some(raw),
            Some(f64::from(raw) / 60.0),
            Some("seconds"),
        )
    } else if enemy_key != 0 {
        (None, None, None, None)
    } else {
        let raw = read_f32(row, RULE_VALUE_OFFSET);
        if raw == 0.0 {
            (None, None, None, None)
        } else {
            (
                Some(RULE_VALUE_OFFSET),
                Some(raw),
                Some(f64::from(raw) / 10.0),
                Some("percent"),
            )
        }
    };

    Ok(SpecialRuleEntry {
        key,
        row_index,
        raw_value,
        display_value,
        display_unit,
        display_grade: None,
        value_source_offset: source_offset,
        qualifier_kind,
        qualifier_key,
    })
}

/// Player-visible terrain effects for one row and one generated magnitude.
///
/// The generated path passes the magnitude the terrain phase resolved
/// (`+0x30` for a filtered pool, the hash key's low byte otherwise).
pub fn terrain_display_effect_keys(row: &[u8; 52], terrain_value: u8) -> Vec<u16> {
    let mut keys = Vec::new();
    if read_u16(row, TERRAIN_EFFECT_BLOCK_OFFSET) != 0 {
        keys.push(TERRAIN_DISPLAY_CRUCIBLE_KEY);
    }
    if let Some((_, key)) = TERRAIN_DISPLAY_SPECIAL_KEYS
        .iter()
        .find(|(value, _)| *value == terrain_value)
    {
        keys.push(*key);
    }
    keys
}

/// `terrain_display_effect_keys_for_row`: the criteria-side helper, which
/// always derives the magnitude from the hash key's low byte.
pub fn terrain_display_effect_keys_for_row(
    row_index: usize,
    rows: &[[u8; 52]],
    terrain_keys: &[u16],
) -> Result<Vec<u16>, EnemyError> {
    let row = rows.get(row_index).ok_or_else(|| {
        EnemyError::InvalidInput(format!("terrain row {row_index} is outside the table"))
    })?;
    let key = terrain_keys.get(row_index).ok_or_else(|| {
        EnemyError::MissingData(format!("terrain row {row_index} has no hash key"))
    })?;
    Ok(terrain_display_effect_keys(row, (*key & 0xFF) as u8))
}

/// Byte offset of the terrain-effect block inside a 52-byte terrain row.
const TERRAIN_EFFECT_BLOCK_OFFSET: usize = 0x2C;

/// Native `+0x2C`/`+0x2E` group keys of the current row against one previous
/// row: RVA 0x1026680 semantics, keyed through the conflict-row manager.
fn rule_rows_conflict(
    current: &[u8; SPECIAL_RULE_ROW_BYTES],
    previous: &[u8; SPECIAL_RULE_ROW_BYTES],
    conflicts: &std::collections::BTreeMap<u16, &[u8; RULE_CONFLICT_ROW_BYTES]>,
) -> bool {
    let current_groups: [u16; RULE_GROUP_KEY_COUNT] =
        std::array::from_fn(|index| read_u16(current, RULE_GROUP_KEY_OFFSET + index * 2));
    let previous_groups: [u16; RULE_GROUP_KEY_COUNT] =
        std::array::from_fn(|index| read_u16(previous, RULE_GROUP_KEY_OFFSET + index * 2));
    for current_key in current_groups {
        let Some(current_group) = conflicts.get(&current_key).copied() else {
            continue;
        };
        if current_group[CONFLICT_GROUP_ENABLED_OFFSET] & CONFLICT_GROUP_ENABLED_BIT == 0 {
            continue;
        }
        let current_identity = read_u16(current_group, CONFLICT_GROUP_IDENTITY_OFFSET);
        for previous_key in previous_groups {
            let Some(previous_group) = conflicts.get(&previous_key).copied() else {
                continue;
            };
            let previous_identity = read_u16(previous_group, CONFLICT_GROUP_IDENTITY_OFFSET);
            if current_identity == previous_identity {
                return true;
            }
        }
    }
    false
}

fn read_u16(row: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([row[offset], row[offset + 1]])
}

fn read_u32(row: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        row[offset],
        row[offset + 1],
        row[offset + 2],
        row[offset + 3],
    ])
}

fn read_f32(row: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        row[offset],
        row[offset + 1],
        row[offset + 2],
        row[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Terrain-effect block offset inside a 52-byte terrain row.
    const TERRAIN_EFFECT_BLOCK: usize = 0x2C;
    /// Generated terrain magnitude inside a 52-byte terrain row.
    const TERRAIN_VALUE: usize = 0x30;

    fn terrain_row(block: u16, value: u8) -> [u8; 52] {
        let mut row = [0u8; 52];
        row[TERRAIN_EFFECT_BLOCK..TERRAIN_EFFECT_BLOCK + 2].copy_from_slice(&block.to_le_bytes());
        row[TERRAIN_VALUE] = value;
        row
    }

    #[allow(clippy::too_many_arguments)]
    fn rule_row(
        key: u16,
        weights: [u16; 5],
        cost: f32,
        value: f32,
        percent_value: f32,
        group_key: u16,
        enabled: bool,
        item_key: u16,
        enemy_key: u32,
        qualifier_text: u32,
    ) -> [u8; SPECIAL_RULE_ROW_BYTES] {
        let mut row = [0u8; SPECIAL_RULE_ROW_BYTES];
        row[RULE_ENEMY_KEY_OFFSET..RULE_ENEMY_KEY_OFFSET + 4]
            .copy_from_slice(&enemy_key.to_le_bytes());
        row[RULE_QUALIFIER_TEXT_OFFSET..RULE_QUALIFIER_TEXT_OFFSET + 4]
            .copy_from_slice(&qualifier_text.to_le_bytes());
        row[RULE_COST_OFFSET..RULE_COST_OFFSET + 4].copy_from_slice(&cost.to_le_bytes());
        row[RULE_VALUE_OFFSET..RULE_VALUE_OFFSET + 4].copy_from_slice(&value.to_le_bytes());
        row[RULE_PERCENT_VALUE_OFFSET..RULE_PERCENT_VALUE_OFFSET + 4]
            .copy_from_slice(&percent_value.to_le_bytes());
        row[RULE_KEY_OFFSET..RULE_KEY_OFFSET + 2].copy_from_slice(&key.to_le_bytes());
        for (index, weight) in weights.iter().enumerate() {
            // Playthrough 1 lives at +0x22, so the first weight is one word
            // above the rule key at +0x20.
            let offset = RULE_WEIGHT_BASE_OFFSET + (index + 1) * 2;
            row[offset..offset + 2].copy_from_slice(&weight.to_le_bytes());
        }
        row[RULE_GROUP_KEY_OFFSET..RULE_GROUP_KEY_OFFSET + 2]
            .copy_from_slice(&group_key.to_le_bytes());
        row[RULE_ITEM_KEY_OFFSET..RULE_ITEM_KEY_OFFSET + 2]
            .copy_from_slice(&item_key.to_le_bytes());
        row[RULE_ENABLED_OFFSET] = u8::from(enabled);
        row
    }

    fn plain_rule(
        key: u16,
        weights: [u16; 5],
        cost: f32,
        value: f32,
    ) -> [u8; SPECIAL_RULE_ROW_BYTES] {
        rule_row(key, weights, cost, value, value, 0, true, 0, 0, 0)
    }

    fn conflict_row(identity: u16, enabled: bool) -> [u8; RULE_CONFLICT_ROW_BYTES] {
        let mut row = [0u8; RULE_CONFLICT_ROW_BYTES];
        row[CONFLICT_GROUP_IDENTITY_OFFSET..CONFLICT_GROUP_IDENTITY_OFFSET + 2]
            .copy_from_slice(&identity.to_le_bytes());
        row[CONFLICT_GROUP_ENABLED_OFFSET] = u8::from(enabled);
        row
    }

    fn tables(
        rules: Vec<[u8; SPECIAL_RULE_ROW_BYTES]>,
        conflicts: Vec<[u8; RULE_CONFLICT_ROW_BYTES]>,
        conflict_keys: Vec<u16>,
    ) -> SpecialRuleTables {
        let rule_keys = rules
            .iter()
            .map(|row| read_u16(row, RULE_KEY_OFFSET))
            .collect();
        SpecialRuleTables {
            rules,
            rule_keys,
            conflicts,
            conflict_keys,
        }
    }

    #[test]
    fn terrain_display_keys_cover_the_crucible_and_special_enums() {
        assert_eq!(
            terrain_display_effect_keys(&terrain_row(1, 0x2D), 0x2D),
            vec![TERRAIN_DISPLAY_CRUCIBLE_KEY, 0x0039]
        );
        assert_eq!(
            terrain_display_effect_keys(&terrain_row(0, 0x2D), 0x2D),
            vec![0x0039]
        );
        assert_eq!(
            terrain_display_effect_keys(&terrain_row(1, 0xD8), 0xD8),
            vec![TERRAIN_DISPLAY_CRUCIBLE_KEY, 0x0058]
        );
        assert_eq!(
            terrain_display_effect_keys(&terrain_row(1, 0x08), 0x08),
            vec![TERRAIN_DISPLAY_CRUCIBLE_KEY, 0x039F]
        );
        // A row without the display block and without a mapped magnitude
        // reports no player-visible terrain effect at all.
        assert!(terrain_display_effect_keys(&terrain_row(0, 0x99), 0x99).is_empty());
    }

    #[test]
    fn terrain_row_helper_uses_the_hash_key_low_byte() {
        let rows = vec![terrain_row(0, 0x2D)];
        let keys = vec![0xD82Du16];
        assert_eq!(
            terrain_display_effect_keys_for_row(0, &rows, &keys).unwrap(),
            vec![0x0039],
            "the helper derives the magnitude from the hash key's low byte"
        );
        assert!(terrain_display_effect_keys_for_row(1, &rows, &keys).is_err());
        assert!(terrain_display_effect_keys_for_row(0, &rows, &[]).is_err());
    }

    #[test]
    fn per_playthrough_weights_decide_whether_a_rule_is_selectable() {
        let table = tables(
            vec![plain_rule(0x0100, [0, 5, 0, 0, 0], 1.0, 0.0)],
            vec![],
            vec![],
        );
        let blocked = generate_special_rules(0, 1, &[], &table).unwrap();
        assert_eq!(blocked.keys, [0, 0, 0]);
        assert!(blocked.entries.is_empty());

        let selected = generate_special_rules(0, 2, &[], &table).unwrap();
        assert_eq!(selected.keys, [0x0100, 0, 0]);
        assert_eq!(selected.entries.len(), 1);
        assert_eq!(selected.entries[0].key, 0x0100);
        assert_eq!(selected.random_draws, 2);
    }

    #[test]
    fn scratch_rule_keys_block_a_rule_that_would_otherwise_be_selected() {
        let table = tables(
            vec![plain_rule(0x0100, [0, 5, 0, 0, 0], 1.0, 0.0)],
            vec![],
            vec![],
        );
        let blocked = generate_special_rules(0, 2, &[0x0100], &table).unwrap();
        assert_eq!(blocked.keys, [0, 0, 0]);
        assert_eq!(blocked.random_draws, 1, "no lottery happens without a pool");
    }

    #[test]
    fn conflicting_rule_groups_cannot_fill_a_second_slot() {
        let mut first = plain_rule(0x0101, [0, 1, 0, 0, 0], 0.0, 0.0);
        let mut second = plain_rule(0x0102, [0, 1, 0, 0, 0], 0.0, 0.0);
        first[RULE_GROUP_KEY_OFFSET..RULE_GROUP_KEY_OFFSET + 2]
            .copy_from_slice(&0x0010u16.to_le_bytes());
        second[RULE_GROUP_KEY_OFFSET..RULE_GROUP_KEY_OFFSET + 2]
            .copy_from_slice(&0x0010u16.to_le_bytes());

        let conflicting = tables(
            vec![first, second],
            vec![conflict_row(0x0007, true)],
            vec![0x0010],
        );
        let result = generate_special_rules(0, 2, &[], &conflicting).unwrap();
        assert_eq!(
            result.entries.len(),
            1,
            "a shared conflict identity blocks the slot"
        );

        let independent = tables(
            vec![first, second],
            vec![conflict_row(0x0007, false)],
            vec![0x0010],
        );
        let result = generate_special_rules(0, 2, &[], &independent).unwrap();
        assert_eq!(
            result.entries.len(),
            2,
            "a disabled conflict row must not block anything"
        );
        assert_ne!(result.keys[0], result.keys[1]);
    }

    #[test]
    fn duplicate_rule_keys_fail_closed_instead_of_filling_two_slots() {
        // The captured hash index maps one key to one row. A synthetic table
        // that breaks that promise must be rejected during descriptor
        // composition exactly like the reference, not silently collapsed.
        let table = tables(
            vec![
                plain_rule(0x0103, [0, 1, 0, 0, 0], 0.0, 0.0),
                plain_rule(0x0103, [0, 1, 0, 0, 0], 0.0, 0.0),
            ],
            vec![],
            vec![],
        );
        let error = generate_special_rules(0, 2, &[], &table).expect_err("ambiguous table");
        assert!(error.to_string().contains("multiple rows"), "{error}");
    }

    #[test]
    fn graded_rules_display_letters_instead_of_numbers() {
        let table = tables(
            vec![
                rule_row(
                    0x0200,
                    [0; 5],
                    0.0,
                    10.0,
                    0.0,
                    SPECIAL_RULE_GRADE_GROUP_KEY,
                    true,
                    0,
                    0,
                    0,
                ),
                rule_row(
                    0x0201,
                    [0; 5],
                    0.0,
                    20.0,
                    0.0,
                    SPECIAL_RULE_GRADE_GROUP_KEY,
                    true,
                    0,
                    0,
                    0,
                ),
                rule_row(
                    0x0202,
                    [0; 5],
                    0.0,
                    30.0,
                    0.0,
                    SPECIAL_RULE_GRADE_GROUP_KEY,
                    true,
                    0,
                    0,
                    0,
                ),
            ],
            vec![],
            vec![],
        );
        for (key, grade) in [(0x0200u16, "C"), (0x0201, "B"), (0x0202, "A")] {
            let entry = describe_special_rule(key, &table).unwrap();
            assert_eq!(entry.display_grade, Some(grade), "key 0x{key:04X}");
            assert_eq!(entry.display_unit, Some("grade"));
            assert_eq!(entry.display_value, None);
            assert_eq!(entry.value_source_offset, Some(RULE_VALUE_OFFSET));
            assert!(entry.raw_value.is_some());
        }
        assert_eq!(
            describe_special_rule(0x0200, &table).unwrap().raw_value,
            Some(10.0)
        );
        assert_eq!(
            describe_special_rule(0x0202, &table).unwrap().raw_value,
            Some(30.0)
        );
    }

    #[test]
    fn describe_special_rule_decodes_every_display_family() {
        let percent = rule_row(0x0300, [0; 5], 0.0, 7.0, 1234.0, 1, true, 0, 0, 0x1234);
        let seconds = rule_row(0x0301, [0; 5], 0.0, 90.0, 0.0, 2, true, 0x0011, 0, 0);
        let enemy = rule_row(0x0302, [0; 5], 0.0, 0.0, 0.0, 3, true, 0, 0xABCD, 0);
        let plain = rule_row(0x0303, [0; 5], 0.0, 55.0, 0.0, 4, true, 0, 0, 0);
        let empty = rule_row(0x0304, [0; 5], 0.0, 0.0, 0.0, 5, true, 0, 0, 0);
        let table = tables(vec![percent, seconds, enemy, plain, empty], vec![], vec![]);

        let entry = describe_special_rule(0x0300, &table).unwrap();
        assert_eq!(entry.qualifier_kind, Some("text"));
        assert_eq!(entry.qualifier_key, Some(0x1234));
        assert_eq!(entry.display_unit, Some("percent"));
        assert_eq!(entry.display_value, Some(123.4));
        assert_eq!(entry.value_source_offset, Some(RULE_PERCENT_VALUE_OFFSET));

        let entry = describe_special_rule(0x0301, &table).unwrap();
        assert_eq!(entry.qualifier_kind, Some("item"));
        assert_eq!(entry.qualifier_key, Some(0x0011));
        assert_eq!(entry.display_unit, Some("seconds"));
        assert_eq!(entry.display_value, Some(1.5));

        let entry = describe_special_rule(0x0302, &table).unwrap();
        assert_eq!(entry.qualifier_kind, Some("enemy"));
        assert_eq!(entry.qualifier_key, Some(0xABCD));
        assert_eq!(entry.raw_value, None);
        assert_eq!(entry.display_value, None);
        assert_eq!(entry.display_unit, None);
        assert_eq!(entry.value_source_offset, None);

        let entry = describe_special_rule(0x0303, &table).unwrap();
        assert_eq!(entry.qualifier_kind, None);
        assert_eq!(entry.display_unit, Some("percent"));
        assert_eq!(entry.display_value, Some(5.5));

        let entry = describe_special_rule(0x0304, &table).unwrap();
        assert_eq!(entry.raw_value, None);
        assert_eq!(entry.display_value, None);
        assert_eq!(entry.value_source_offset, None);
    }

    #[test]
    fn effect_qualifiers_are_reported_and_ambiguous_keys_fail_closed() {
        let plain = rule_row(0x0400, [0; 5], 0.0, 0.0, 0.0, 6, true, 0, 0, 0);
        let mut effect = rule_row(0x0401, [0; 5], 0.0, 0.0, 0.0, 7, true, 0, 0, 0);
        effect[RULE_EFFECT_KEY_OFFSET..RULE_EFFECT_KEY_OFFSET + 2]
            .copy_from_slice(&0x0022u16.to_le_bytes());
        let table = tables(vec![plain, effect], vec![], vec![]);
        let entry = describe_special_rule(0x0400, &table).unwrap();
        assert_eq!(entry.qualifier_kind, None);
        assert_eq!(entry.qualifier_key, None);
        let entry = describe_special_rule(0x0401, &table).unwrap();
        assert_eq!(entry.qualifier_kind, Some("effect"));
        assert_eq!(entry.qualifier_key, Some(0x0022));

        // The hash index is authoritative: two rows sharing a key are ambiguous.
        let mut duplicate = table.clone();
        duplicate.rule_keys = vec![0x0400, 0x0400];
        let error = describe_special_rule(0x0400, &duplicate).expect_err("ambiguous key");
        assert!(error.to_string().contains("multiple rows"), "{error}");
        let error = describe_special_rule(0x9999, &table).expect_err("unknown key");
        assert!(error.to_string().contains("unknown"), "{error}");
    }

    #[test]
    fn unsupported_progressions_and_incomplete_tables_fail_closed() {
        let table = tables(
            vec![plain_rule(0x0100, [1, 1, 1, 1, 1], 1.0, 0.0)],
            vec![],
            vec![],
        );
        assert!(generate_special_rules(0, 0, &[], &table).is_err());
        assert!(generate_special_rules(0, 6, &[], &table).is_err());

        let mut mismatched = table.clone();
        mismatched.rule_keys.clear();
        assert!(generate_special_rules(0, 3, &[], &mismatched).is_err());
        assert!(describe_special_rule(0x0100, &mismatched).is_err());

        let mut conflicts = table;
        conflicts.conflicts = vec![conflict_row(1, true)];
        assert!(generate_special_rules(0, 3, &[], &conflicts).is_err());
    }

    #[test]
    fn legal_rule_keys_keep_enabled_rows_with_a_positive_progression_weight() {
        // Playthrough 3 reads the third weight word at +0x26.
        let enabled = rule_row(0x0200, [1, 1, 1, 1, 1], 0.0, 0.0, 0.0, 0, true, 0, 0, 0);
        let zero_weight = rule_row(0x0201, [1, 1, 0, 1, 1], 0.0, 0.0, 0.0, 0, true, 0, 0, 0);
        let disabled = rule_row(0x0202, [1, 1, 1, 1, 1], 0.0, 0.0, 0.0, 0, false, 0, 0, 0);
        let table = tables(vec![enabled, zero_weight, disabled], vec![], vec![]);

        let legal = legal_special_rule_keys(3, &table).unwrap();
        assert_eq!(legal.iter().copied().collect::<Vec<_>>(), vec![0x0200]);
        // The same rows stay legal for a progression whose weight is non-zero.
        assert_eq!(
            legal_special_rule_keys(1, &table)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![0x0200, 0x0201]
        );
        assert!(legal_special_rule_keys(0, &table).is_err());
        assert!(legal_special_rule_keys(6, &table).is_err());

        let mut mismatched = table;
        mismatched.rule_keys.clear();
        let error = legal_special_rule_keys(3, &mismatched).expect_err("key index mismatch");
        assert!(
            error.to_string().contains("does not match row count"),
            "{error}"
        );
    }
}
