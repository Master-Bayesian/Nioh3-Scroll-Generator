//! Natural-generation rules for owned equipment and soul-core records.
//!
//! PC v2.02 live evidence (`docs/knowledge/V082_LIVE_CHARACTER_EQUIPMENT_RESEARCH_20260926.md`):
//! the game builds equipment effects with the same `generate_effects` and value
//! normalization as scrolls, so an effect's legal raw values at a given level and
//! rarity are exactly the values the scroll formula yields over that rarity's
//! roll range. The slot layout, pools and exclusions below were read from the
//! tables and confirmed against every captured natural drop.
//!
//! This module only *describes* what natural generation can produce. It never
//! refuses a modded edit; callers decide whether to allow one.

use std::collections::{BTreeMap, BTreeSet};

use crate::effect::{
    f32_add, f32_div, f32_mul, trunc_f32, EffectDefinition, EffectError, EffectTableBytes,
    EffectTableIndex, ITEM_ROW_BYTES,
};

/// One owned-equipment record.
pub const EQUIPMENT_RECORD_BYTES: usize = 0xF0;
/// First effect entry; entries are 0x18 bytes, seven per record.
pub const EFFECT_ENTRY_OFFSET: usize = 0x34;
pub const EFFECT_ENTRY_BYTES: usize = 0x18;
pub const EFFECT_ENTRY_COUNT: usize = 7;
/// Effect-row flag carried only by effects the hell conversion draws.
pub const HELL_EFFECT_FLAG: u32 = 0x10;
/// Normalization flag of a star (green ✦) effect row.
pub const STAR_NORMALIZATION_FLAG: u32 = 0x08;
/// Entry byte `+0xE` bit the builder sets for a star effect.
pub const STAR_ENTRY_FLAG: u8 = 0x04;
/// Record byte `+0x1A` marking a hell weapon.
pub const HELL_RECORD_FLAG: u8 = 0x10;
/// Effects per natural record, by rarity 0..=5 (fixed, set and grace included).
pub const NATURAL_EFFECT_COUNT: [usize; 6] = [2, 3, 3, 4, 5, 6];
/// Lowest rarity whose items without a set carry a grace (恩宠) in the last slot.
pub const GRACE_MIN_RARITY: u8 = 4;
/// Draws per roll lottery operand (RVA 0x110A275: two `draw_int(46)` draws).
const ROLL_DRAW_SPAN: u32 = 46;
const MAX_ROLL_LOTTERY: usize = 101;

/// The item-table fields the equipment rules read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquipmentItem {
    pub item_id: u16,
    /// `+0xB0`: gates effect flags 0x04/0x08 and marks hell-capable weapons (0x02).
    pub item_flags: u32,
    /// `+0x15C` low half: the effect-row lottery weight column.
    pub weight_slot: u16,
    /// `+0x154`: the fixed set effect, placed in the last slot.
    pub set_effect: Option<u16>,
    /// `+0x158` and `+0x15A`: fixed innate effects, placed first (soul cores use both).
    pub innate_effects: [Option<u16>; 2],
    /// `+0x58`: the key the hell martial-skill table matches.
    pub weapon_type_key: u16,
    /// `+0x182`: 1 or 2 for weapons the hell conversion may pick.
    pub hell_class: u8,
}

impl EquipmentItem {
    fn parse(row: &[u8]) -> Option<Self> {
        if row.len() < ITEM_ROW_BYTES {
            return None;
        }
        let u16_at = |offset: usize| u16::from_le_bytes([row[offset], row[offset + 1]]);
        let u32_at = |offset: usize| {
            u32::from_le_bytes([
                row[offset],
                row[offset + 1],
                row[offset + 2],
                row[offset + 3],
            ])
        };
        let nonzero = |value: u16| (value != 0).then_some(value);
        Some(Self {
            item_id: u16_at(0x152),
            item_flags: u32_at(0xB0),
            weight_slot: u16_at(0x15C),
            set_effect: nonzero(u16_at(0x154)),
            innate_effects: [nonzero(u16_at(0x158)), nonzero(u16_at(0x15A))],
            weapon_type_key: u16_at(0x58),
            hell_class: row[0x182],
        })
    }

    /// Whether the hell conversion (`+0x2285E58`) may pick this item.
    pub fn hell_capable(&self) -> bool {
        matches!(self.hell_class, 1 | 2) && self.item_flags & 0x02 != 0
    }
}

/// One effect a random slot may naturally hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolEffect {
    pub effect_id: u16,
    /// A star row, reached by same-group promotion of a direct candidate.
    pub star: bool,
}

/// One raw value an effect can naturally take, with its roll probability.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegalValue {
    pub value: i32,
    pub roll_min: u8,
    pub roll_max: u8,
    /// Probability of this value among all rolls at this rarity.
    pub probability: f64,
    /// Probability of a value at least this good (1.0 for the worst value).
    pub top_fraction: f64,
}

/// What natural generation places in one slot of a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotRole {
    Innate,
    Hell,
    Random,
    Set,
    Grace,
}

/// One finding about a record. Every finding means "natural generation cannot
/// produce this"; none of them blocks an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    UnknownItem,
    UnsupportedRarity,
    EffectCount { expected: usize, actual: usize },
    UnknownEffect { slot: usize, effect_id: u32 },
    WrongInnate { slot: usize, expected: u16 },
    WrongSet { slot: usize, expected: u16 },
    NotGrace { slot: usize },
    NotInPool { slot: usize },
    HellEffectOutsideHellSlot { slot: usize },
    MissingHellEffect,
    HellOnIneligibleItem,
    StarBelowRarity { slot: usize },
    MultipleStars,
    StarFlagMismatch { slot: usize },
    GroupConflict { slot: usize, other: usize },
    ValueNotNatural { slot: usize, value: u32 },
    RollOutOfRange { slot: usize, roll: u8 },
    HellSkillNotNatural { skill: u16 },
}

/// The audit of one record against natural generation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecordAudit {
    pub findings: Vec<Finding>,
    /// The role natural generation gives each occupied slot, in slot order.
    pub roles: Vec<Option<SlotRole>>,
}

impl RecordAudit {
    pub fn natural(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Hell martial-skill rows read live from `[[Nioh3.exe+0x45B9E30]+0x5A8]` (PC v2.02):
/// `(skill id, weapon-type key, minimum level)`. Every row has weight 10.
pub const HELL_SKILLS_V202: [(u16, u16, u16); 39] = [
    (0xF9B9, 21589, 1),
    (0x5638, 28275, 1),
    (0xAA7C, 28275, 1),
    (0x9778, 28275, 1),
    (0xB623, 3375, 1),
    (0xD227, 3375, 1),
    (0x3881, 3375, 1),
    (0x0606, 24575, 1),
    (0x6F70, 24575, 1),
    (0xC0C1, 6409, 1),
    (0x3435, 6409, 1),
    (0xAC06, 6409, 1),
    (0xBB9F, 29361, 1),
    (0xC6EA, 29361, 1),
    (0x26D6, 29361, 1),
    (0x8D12, 20629, 1),
    (0x885C, 20629, 1),
    (0x29F4, 24091, 1),
    (0x3FB9, 24091, 1),
    (0x532E, 24091, 1),
    (0x409F, 636, 1),
    (0xE10B, 636, 1),
    (0x6EB6, 636, 1),
    (0x3A0F, 9554, 1),
    (0x7E6A, 9554, 1),
    (0x99C0, 1254, 1),
    (0x653C, 1254, 1),
    (0x2ABB, 6102, 1),
    (0x6122, 6102, 1),
    (0xD6F9, 6102, 1),
    (0x749E, 11583, 1),
    (0x59BF, 11583, 1),
    (0x410A, 11583, 1),
    (0x5841, 4866, 1),
    (0x60A3, 13257, 1),
    (0x99AF, 4866, 1),
    (0xC67A, 13257, 1),
    (0x490D, 4866, 1),
    (0x35C5, 13257, 1),
];

/// Equipment rules over one exact table bundle.
#[derive(Debug, Clone)]
pub struct EquipmentRules<'a> {
    index: &'a EffectTableIndex,
    items: BTreeMap<u16, EquipmentItem>,
    graces: BTreeSet<u16>,
    effects_by_group: BTreeMap<u16, Vec<u16>>,
    /// Probability of each roll lottery value 0..=100.
    lottery: [f64; MAX_ROLL_LOTTERY],
}

fn draw_distribution() -> [u32; ROLL_DRAW_SPAN as usize] {
    // Every u16 output of the LCG maps through the binary32 draw at RVA 0x56C6A8.
    // The same binary32 steps as `sequence::random_int`, applied to each u16.
    let mut counts = [0u32; ROLL_DRAW_SPAN as usize];
    for raw in 0..=u16::MAX {
        let random_float = f32::from(raw) * (1.0f64 / 65536.0) as f32;
        let scaled = random_float * ROLL_DRAW_SPAN as f32;
        let value = (scaled as u32).min(ROLL_DRAW_SPAN - 1);
        counts[value as usize] += 1;
    }
    counts
}

fn lottery_distribution() -> [f64; MAX_ROLL_LOTTERY] {
    let draws = draw_distribution();
    let total = 65_536f64 * 65_536f64;
    let mut lottery = [0f64; MAX_ROLL_LOTTERY];
    for (first, first_count) in draws.iter().enumerate() {
        for (second, second_count) in draws.iter().enumerate() {
            let value = first + second + if first == second { 10 } else { 0 };
            lottery[value] += f64::from(*first_count) * f64::from(*second_count) / total;
        }
    }
    lottery
}

/// The roll byte one lottery value yields for a rarity row (RVA 0x110A275).
fn roll_for_lottery(minimum: u32, maximum: u32, lottery: u32) -> Result<u8, EffectError> {
    if minimum >= maximum {
        return Ok((minimum & 0xFF) as u8);
    }
    let span = maximum.wrapping_sub(minimum);
    let scaled = f32_div(f32_mul(lottery as f32, span as f32), 100.0f32);
    let result = trunc_f32(f32_add(minimum as f32, scaled), "roll_percentile")?;
    Ok((result & 0xFF) as u8)
}

fn progress_bucket(threshold: u16) -> usize {
    match threshold {
        0..=6999 => 0,
        7000..=7999 => 1,
        8000..=8999 => 2,
        _ => 3,
    }
}

impl<'a> EquipmentRules<'a> {
    /// Build the rules from the table index, the full item table and the grace ids.
    pub fn new(
        index: &'a EffectTableIndex,
        item_table: &EffectTableBytes,
        grace_effects: impl IntoIterator<Item = u32>,
    ) -> Self {
        let mut items = BTreeMap::new();
        for row_index in 0..item_table.row_count() {
            if let Some(item) = item_table.row(row_index).and_then(EquipmentItem::parse) {
                items.entry(item.item_id).or_insert(item);
            }
        }
        let mut effects_by_group: BTreeMap<u16, Vec<u16>> = BTreeMap::new();
        for effect in &index.effects_in_row_order {
            effects_by_group
                .entry(effect.group_key)
                .or_default()
                .push(effect.effect_id);
        }
        Self {
            index,
            items,
            graces: grace_effects
                .into_iter()
                .filter_map(|id| u16::try_from(id).ok())
                .collect(),
            effects_by_group,
            lottery: lottery_distribution(),
        }
    }

    pub fn item(&self, item_id: u16) -> Option<&EquipmentItem> {
        self.items.get(&item_id)
    }

    pub fn graces(&self) -> impl Iterator<Item = u16> + '_ {
        self.graces.iter().copied()
    }

    pub fn effect(&self, effect_id: u16) -> Option<&EffectDefinition> {
        self.index.effects_by_id.get(&effect_id)
    }

    /// Whether any playthrough reaches the row's progress gate.
    fn progress_reachable(&self, effect: &EffectDefinition) -> bool {
        let bucket = progress_bucket(effect.progress_threshold);
        (1..=5u8).any(|playthrough| {
            self.index
                .playthrough_progress(playthrough)
                .map(|progress| progress[bucket] >= u32::from(effect.progress_threshold))
                .unwrap_or(false)
        })
    }

    fn weighted_for(&self, effect: &EffectDefinition, item: &EquipmentItem) -> bool {
        if effect.flags & 0x04 != 0 && item.item_flags & 0x0800 == 0 {
            return false;
        }
        if effect.flags & 0x08 != 0 && item.item_flags & 0x1000 == 0 {
            return false;
        }
        effect.slot_weight(usize::from(item.weight_slot)) != 0
            && effect.type_multipliers.iter().any(|value| *value > 0.0)
            && self.progress_reachable(effect)
    }

    fn direct_candidate(&self, effect: &EffectDefinition, item: &EquipmentItem) -> bool {
        effect.effect_id != 0
            && effect.flags & (0x40 | 0x80) != 0
            && effect.flags & HELL_EFFECT_FLAG == 0
            && effect.normalization_flags & STAR_NORMALIZATION_FLAG == 0
            && self.weighted_for(effect, item)
    }

    /// Effects a random slot of this item may hold at this rarity.
    pub fn random_pool(&self, item: &EquipmentItem, rarity: u8) -> Vec<PoolEffect> {
        let mut pool = Vec::new();
        let mut direct_groups = BTreeSet::new();
        for effect in &self.index.effects_in_row_order {
            if self.direct_candidate(effect, item) && effect.rarity_weight(rarity) != 0.0 {
                direct_groups.insert(effect.group_key);
                pool.push(PoolEffect {
                    effect_id: effect.effect_id,
                    star: false,
                });
            }
        }
        for effect in &self.index.effects_in_row_order {
            if effect.normalization_flags & STAR_NORMALIZATION_FLAG == 0
                || effect.rarity_weight(rarity) == 0.0
                || !direct_groups.contains(&effect.group_key)
            {
                continue;
            }
            pool.push(PoolEffect {
                effect_id: effect.effect_id,
                star: true,
            });
        }
        pool
    }

    /// Hell-only effects the conversion may place in a hell weapon's first slot.
    pub fn hell_pool(&self, item: &EquipmentItem) -> Vec<u16> {
        self.index
            .effects_in_row_order
            .iter()
            .filter(|effect| {
                effect.flags & HELL_EFFECT_FLAG != 0
                    && effect.flags & (0x40 | 0x80) != 0
                    && self.weighted_for(effect, item)
            })
            .map(|effect| effect.effect_id)
            .collect()
    }

    /// Hell martial skills natural conversion can give this item at this level.
    pub fn hell_skills(&self, item: &EquipmentItem, level: u16) -> Vec<u16> {
        HELL_SKILLS_V202
            .iter()
            .filter(|(_, key, minimum)| *key == item.weapon_type_key && level >= *minimum)
            .map(|(skill, _, _)| *skill)
            .collect()
    }

    /// The role natural generation gives each slot, in slot order.
    ///
    /// Innate effects come first and a set or grace effect last; every other
    /// slot is random. An item with neither innate nor set effects has one slot
    /// fewer. The hell conversion replaces the first innate effect with a
    /// hell-only effect; a hell weapon without innate effects has no hell slot.
    pub fn slot_roles(
        &self,
        item: &EquipmentItem,
        rarity: u8,
        hell: bool,
    ) -> Option<Vec<SlotRole>> {
        let natural = *NATURAL_EFFECT_COUNT.get(usize::from(rarity))?;
        let innate: Vec<u16> = item.innate_effects.iter().flatten().copied().collect();
        let count = if innate.is_empty() && item.set_effect.is_none() {
            natural - 1
        } else {
            natural
        };
        let mut front: Vec<SlotRole> = innate.iter().map(|_| SlotRole::Innate).collect();
        if hell {
            if let Some(first) = front.first_mut() {
                *first = SlotRole::Hell;
            }
        }
        let back = if item.set_effect.is_some() {
            Some(SlotRole::Set)
        } else if rarity >= GRACE_MIN_RARITY {
            Some(SlotRole::Grace)
        } else {
            None
        };
        let fixed = front.len() + usize::from(back.is_some());
        let mut roles = front;
        roles.extend(std::iter::repeat_n(
            SlotRole::Random,
            count.saturating_sub(fixed),
        ));
        roles.extend(back);
        Some(roles)
    }

    /// Every raw value natural generation gives this effect, best value last.
    pub fn legal_values(
        &self,
        effect_id: u16,
        rarity: u8,
        level: u16,
    ) -> Result<Vec<LegalValue>, EffectError> {
        let row = self.index.rarity_generation(rarity)?;
        let mut by_value: BTreeMap<i32, (u8, u8, f64)> = BTreeMap::new();
        for (lottery, probability) in self.lottery.iter().enumerate() {
            if *probability == 0.0 {
                continue;
            }
            let roll = roll_for_lottery(
                row.minimum_roll_percent,
                row.maximum_roll_percent,
                lottery as u32,
            )?;
            let value = self
                .index
                .resolved_effect_value(u32::from(effect_id), roll, level)?;
            let entry = by_value.entry(value).or_insert((roll, roll, 0.0));
            entry.0 = entry.0.min(roll);
            entry.1 = entry.1.max(roll);
            entry.2 += probability;
        }
        let mut values: Vec<LegalValue> = by_value
            .into_iter()
            .map(|(value, (roll_min, roll_max, probability))| LegalValue {
                value,
                roll_min,
                roll_max,
                probability,
                top_fraction: 0.0,
            })
            .collect();
        // Higher rolls give better values; order by roll so "best" is last even
        // for effects whose raw value falls as the roll rises.
        values.sort_by_key(|value| value.roll_max);
        let mut remaining = values.iter().map(|value| value.probability).sum::<f64>();
        for value in &mut values {
            value.top_fraction = remaining;
            remaining -= value.probability;
        }
        Ok(values)
    }

    /// Whether `roll` is reachable at this rarity.
    pub fn roll_reachable(&self, rarity: u8, roll: u8) -> Result<bool, EffectError> {
        let row = self.index.rarity_generation(rarity)?;
        for (lottery, probability) in self.lottery.iter().enumerate() {
            if *probability != 0.0
                && roll_for_lottery(
                    row.minimum_roll_percent,
                    row.maximum_roll_percent,
                    lottery as u32,
                )? == roll
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The value one roll yields; the value a legal edit writes for a chosen roll.
    pub fn value_for_roll(&self, effect_id: u16, roll: u8, level: u16) -> Result<i32, EffectError> {
        self.index
            .resolved_effect_value(u32::from(effect_id), roll, level)
    }

    /// Group peers of an effect (its ordinary, star and hell variants).
    pub fn group_peers(&self, effect_id: u16) -> &[u16] {
        self.effect(effect_id)
            .and_then(|effect| self.effects_by_group.get(&effect.group_key))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Audit one 0xF0 record against natural generation.
    pub fn audit(&self, record: &[u8]) -> RecordAudit {
        let mut audit = RecordAudit::default();
        if record.len() < EQUIPMENT_RECORD_BYTES {
            audit.findings.push(Finding::UnknownItem);
            return audit;
        }
        let item_id = u16::from_le_bytes([record[0], record[1]]);
        let level = u16::from_le_bytes([record[6], record[7]]);
        let rarity = record[0x30];
        let hell = record[0x1A] & HELL_RECORD_FLAG != 0;
        let Some(item) = self.item(item_id).copied() else {
            audit.findings.push(Finding::UnknownItem);
            return audit;
        };
        if hell {
            if !item.hell_capable() {
                audit.findings.push(Finding::HellOnIneligibleItem);
            }
            let skill = u16::from_le_bytes([record[0x10], record[0x11]]);
            if !self.hell_skills(&item, level).contains(&skill) {
                audit.findings.push(Finding::HellSkillNotNatural { skill });
            }
        }
        let entries: Vec<(usize, u32, u32, u8, u8)> = (0..EFFECT_ENTRY_COUNT)
            .filter_map(|slot| {
                let entry = &record[EFFECT_ENTRY_OFFSET + slot * EFFECT_ENTRY_BYTES..]
                    [..EFFECT_ENTRY_BYTES];
                let effect_id = u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]);
                (effect_id != u32::MAX).then(|| {
                    let value = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]);
                    (slot, effect_id, value, entry[0xC], entry[0xE])
                })
            })
            .collect();
        let Some(roles) = self.slot_roles(&item, rarity, hell) else {
            audit.findings.push(Finding::UnsupportedRarity);
            return audit;
        };
        if roles.len() != entries.len() {
            audit.findings.push(Finding::EffectCount {
                expected: roles.len(),
                actual: entries.len(),
            });
        }
        let pool = self.random_pool(&item, rarity);
        let hell_pool = self.hell_pool(&item);
        let mut stars = 0;
        for (position, (slot, effect_id, value, roll, flags)) in entries.iter().copied().enumerate()
        {
            let role = roles.get(position).copied();
            audit.roles.push(role);
            let Some(effect) = u16::try_from(effect_id).ok().and_then(|id| self.effect(id)) else {
                audit
                    .findings
                    .push(Finding::UnknownEffect { slot, effect_id });
                continue;
            };
            let id = effect.effect_id;
            let is_star = effect.normalization_flags & STAR_NORMALIZATION_FLAG != 0;
            if is_star != (flags & STAR_ENTRY_FLAG != 0) {
                audit.findings.push(Finding::StarFlagMismatch { slot });
            }
            if is_star {
                stars += 1;
                if self.index.rarity_generation(rarity).is_err()
                    || effect.rarity_weight(rarity) == 0.0
                {
                    audit.findings.push(Finding::StarBelowRarity { slot });
                }
            }
            if effect.flags & HELL_EFFECT_FLAG != 0 && role != Some(SlotRole::Hell) {
                audit
                    .findings
                    .push(Finding::HellEffectOutsideHellSlot { slot });
            }
            let valued = match role {
                Some(SlotRole::Innate) => {
                    let expected = item.innate_effects[position.min(1)];
                    if Some(id) != expected {
                        audit.findings.push(Finding::WrongInnate {
                            slot,
                            expected: expected.unwrap_or(0),
                        });
                    }
                    true
                }
                Some(SlotRole::Set) => {
                    if Some(id) != item.set_effect {
                        audit.findings.push(Finding::WrongSet {
                            slot,
                            expected: item.set_effect.unwrap_or(0),
                        });
                    }
                    false
                }
                Some(SlotRole::Grace) => {
                    if !self.graces.contains(&id) {
                        audit.findings.push(Finding::NotGrace { slot });
                    }
                    false
                }
                Some(SlotRole::Hell) => {
                    if !hell_pool.contains(&id) {
                        audit.findings.push(Finding::MissingHellEffect);
                    }
                    true
                }
                Some(SlotRole::Random) | None => {
                    if !pool.iter().any(|candidate| candidate.effect_id == id) {
                        audit.findings.push(Finding::NotInPool { slot });
                    }
                    true
                }
            };
            if valued {
                match self.roll_reachable(rarity, roll) {
                    Ok(true) => {}
                    _ => audit.findings.push(Finding::RollOutOfRange { slot, roll }),
                }
                let natural = self
                    .legal_values(id, rarity, level)
                    .map(|values| {
                        values
                            .iter()
                            .any(|legal| i64::from(legal.value) == i64::from(value))
                    })
                    .unwrap_or(false);
                if !natural {
                    audit
                        .findings
                        .push(Finding::ValueNotNatural { slot, value });
                }
            } else if value != 0 {
                audit
                    .findings
                    .push(Finding::ValueNotNatural { slot, value });
            }
        }
        if stars > 1 {
            audit.findings.push(Finding::MultipleStars);
        }
        for (left_position, (left_slot, left, ..)) in entries.iter().enumerate() {
            for (right_slot, right, ..) in entries.iter().skip(left_position + 1) {
                let (Ok(left), Ok(right)) = (u16::try_from(*left), u16::try_from(*right)) else {
                    continue;
                };
                if self.effect(left).is_none() || self.effect(right).is_none() {
                    continue;
                }
                if !self
                    .index
                    .is_compatible(left, &[u32::from(right)], None)
                    .unwrap_or(true)
                {
                    audit.findings.push(Finding::GroupConflict {
                        slot: *right_slot,
                        other: *left_slot,
                    });
                }
            }
        }
        audit
    }
}
