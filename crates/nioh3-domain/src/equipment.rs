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
/// Random effects per natural record, by rarity 0..=5 (innate, set and grace excluded).
///
/// Read from 1,406 records of the owner's PC v2.02 inventory; rarity 5 follows
/// the pattern but has no sample yet.
pub const RANDOM_EFFECT_COUNT: [usize; 6] = [1, 2, 2, 3, 3, 4];
/// Weight column the game substitutes for restricted slots (`NativeWeightContext`).
pub const RESTRICTED_WEIGHT_SLOT: usize = 0x29;
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

    /// Soul cores (weight columns 54..=57) follow their own slot layout.
    pub fn soul_core(&self) -> bool {
        (54..=57).contains(&self.weight_slot)
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

/// One finding about a record. A finding means "natural drop generation does
/// not produce this"; none of them blocks an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    UnknownItem,
    UnsupportedRarity,
    EffectCount {
        expected: usize,
        actual: usize,
    },
    UnknownEffect {
        slot: usize,
        effect_id: u32,
    },
    MissingInnate {
        expected: u16,
    },
    MissingSet {
        expected: u16,
    },
    MissingGrace,
    /// A set, grace or innate effect the item does not naturally carry.
    UnexpectedFixed {
        slot: usize,
    },
    NotInPool {
        slot: usize,
    },
    HellEffectOnNormal {
        slot: usize,
    },
    MissingHellEffect,
    HellOnIneligibleItem,
    StarBelowRarity {
        slot: usize,
    },
    MultipleStars,
    StarFlagMismatch {
        slot: usize,
    },
    GroupConflict {
        slot: usize,
        other: usize,
    },
    ValueNotNatural {
        slot: usize,
        value: u32,
    },
    /// A star value above the base formula: the unmodelled optional addition
    /// (`+0x5712D8`) can explain it, so it is reported as unverified.
    ValueAboveFormula {
        slot: usize,
        value: u32,
    },
    RollOutOfRange {
        slot: usize,
        roll: u8,
    },
    /// The entry's group marker (`+0x00`) or category bits (`+0x0D`) belong to
    /// another effect: only the id was replaced, which slot normalization never
    /// leaves behind. `original` is an effect of the recorded group.
    ReplacedEffect {
        slot: usize,
        recorded_group: u16,
        original: Option<u16>,
    },
}

/// Record flag bits at `+0x18` whose items follow rules not modelled here:
/// `0x20000` (processed at the blacksmith) and `0x400000` (unique items, which
/// the hell conversion also skips).
pub const SPECIAL_RECORD_FLAGS: u32 = 0x0002_0000 | 0x0040_0000;

/// The audit of one record against natural drop generation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecordAudit {
    /// Differences natural generation cannot explain.
    pub findings: Vec<Finding>,
    /// Differences the modelled rules cannot decide (special items, optional
    /// value additions); neither natural nor unnatural.
    pub unverified: Vec<Finding>,
    /// The role each occupied slot plays, in slot order.
    pub roles: Vec<Option<SlotRole>>,
}

impl RecordAudit {
    pub fn natural(&self) -> bool {
        self.findings.is_empty() && self.unverified.is_empty()
    }

    /// `natural`, `unverified` or `unnatural`.
    pub fn verdict(&self) -> &'static str {
        if !self.findings.is_empty() {
            "unnatural"
        } else if !self.unverified.is_empty() {
            "unverified"
        } else {
            "natural"
        }
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
        (effect.slot_weight(usize::from(item.weight_slot)) != 0
            || effect.slot_weight(RESTRICTED_WEIGHT_SLOT) != 0)
            && effect.type_multipliers.iter().any(|value| *value > 0.0)
            && self.progress_reachable(effect)
    }

    fn direct_candidate(&self, effect: &EffectDefinition, item: &EquipmentItem) -> bool {
        effect.effect_id != 0
            && effect.flags & (0x40 | 0x80) != 0
            && effect.flags & HELL_EFFECT_FLAG == 0
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
                    star: effect.normalization_flags & STAR_NORMALIZATION_FLAG != 0,
                });
            }
        }
        for effect in &self.index.effects_in_row_order {
            if effect.normalization_flags & STAR_NORMALIZATION_FLAG == 0
                || effect.rarity_weight(rarity) == 0.0
                || !direct_groups.contains(&effect.group_key)
                || pool.iter().any(|entry| entry.effect_id == effect.effect_id)
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
    /// Innate effects first, then [`RANDOM_EFFECT_COUNT`] random effects; from
    /// rarity [`GRACE_MIN_RARITY`] one more slot holds a grace, or a random
    /// effect when the item has a set; a set effect comes last. Soul cores carry
    /// `min(2, rarity + 1)` innate and `rarity - 1` random effects. The hell
    /// conversion replaces the first innate effect with a hell-only effect; a
    /// hell weapon without innate effects has no hell slot.
    pub fn slot_roles(
        &self,
        item: &EquipmentItem,
        rarity: u8,
        hell: bool,
    ) -> Option<Vec<SlotRole>> {
        let random = *RANDOM_EFFECT_COUNT.get(usize::from(rarity))?;
        let innate: Vec<u16> = item.innate_effects.iter().flatten().copied().collect();
        let mut roles = Vec::new();
        if item.soul_core() {
            let innate_count = innate.len().min(usize::from(rarity) + 1);
            roles.extend(std::iter::repeat_n(SlotRole::Innate, innate_count));
            roles.extend(std::iter::repeat_n(
                SlotRole::Random,
                usize::from(rarity).saturating_sub(1),
            ));
            return Some(roles);
        }
        roles.extend(innate.iter().map(|_| SlotRole::Innate));
        if hell {
            if let Some(first) = roles.first_mut() {
                *first = SlotRole::Hell;
            }
        }
        roles.extend(std::iter::repeat_n(SlotRole::Random, random));
        if rarity >= GRACE_MIN_RARITY {
            roles.push(if item.set_effect.is_some() {
                SlotRole::Random
            } else {
                SlotRole::Grace
            });
        }
        if item.set_effect.is_some() {
            roles.push(SlotRole::Set);
        }
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

    /// An effect's group and conflict masks: two drawn effects exclude each
    /// other when they share a group or any mask bit (`is_compatible`).
    pub fn conflict_key(&self, effect_id: u16) -> Option<(u16, [u32; 2])> {
        let group = self.index.group_for_effect(effect_id)?;
        Some((
            group.group_key,
            [group.conflict_mask_0, group.conflict_mask_1],
        ))
    }

    /// The group and category bits slot normalization writes for an effect
    /// (entry `+0x00` and the low six bits of `+0x0D`).
    pub fn effect_marker(&self, effect_id: u16) -> Option<(u16, u8)> {
        let group = self.effect(effect_id)?.group_key;
        let category = self.index.groups_by_key.get(&group)?.category_key;
        Some((group, (category & 0x3F) as u8))
    }

    /// Group peers of an effect (its ordinary, star and hell variants).
    pub fn group_peers(&self, effect_id: u16) -> &[u16] {
        self.effect(effect_id)
            .and_then(|effect| self.effects_by_group.get(&effect.group_key))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Audit one 0xF0 record against natural drop generation.
    ///
    /// Slots are classified by content, not position: the game keeps every
    /// effect when a record is reordered, so only the counts of innate, hell,
    /// set, grace and random effects and each effect's pool and value matter.
    pub fn audit(&self, record: &[u8]) -> RecordAudit {
        let mut audit = RecordAudit::default();
        if record.len() < EQUIPMENT_RECORD_BYTES {
            audit.findings.push(Finding::UnknownItem);
            return audit;
        }
        let item_id = u16::from_le_bytes([record[0], record[1]]);
        let level = u16::from_le_bytes([record[6], record[7]]);
        let rarity = record[0x30];
        let flags = u32::from_le_bytes([record[0x18], record[0x19], record[0x1A], record[0x1B]]);
        let hell = record[0x1A] & HELL_RECORD_FLAG != 0;
        let special = flags & SPECIAL_RECORD_FLAGS != 0;
        let Some(item) = self.item(item_id).copied() else {
            audit.findings.push(Finding::UnknownItem);
            return audit;
        };
        let Some(expected_roles) = self.slot_roles(&item, rarity, hell) else {
            audit.findings.push(Finding::UnsupportedRarity);
            return audit;
        };
        let mut structural: Vec<Finding> = Vec::new();
        let mut values: Vec<Finding> = Vec::new();
        let mut unverified: Vec<Finding> = Vec::new();
        // Findings no special route explains: the game rewrites an entry's group
        // marker whenever it writes the entry, blacksmith replacements included.
        let mut definite: Vec<Finding> = Vec::new();
        if hell && !item.hell_capable() {
            structural.push(Finding::HellOnIneligibleItem);
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
        if expected_roles.len() != entries.len() {
            structural.push(Finding::EffectCount {
                expected: expected_roles.len(),
                actual: entries.len(),
            });
        }
        let count = |role: SlotRole| expected_roles.iter().filter(|r| **r == role).count();
        let innate: Vec<u16> = item.innate_effects.iter().flatten().copied().collect();
        let expected_innate: Vec<u16> = innate
            .iter()
            .copied()
            .skip(count(SlotRole::Hell))
            .take(count(SlotRole::Innate))
            .collect();
        let pool = self.random_pool(&item, rarity);
        let hell_pool = self.hell_pool(&item);
        let mut innate_seen: Vec<u16> = Vec::new();
        let (mut hell_seen, mut set_seen, mut grace_seen, mut stars) = (0, 0, 0, 0);
        for (slot, effect_id, value, roll, entry_flags) in entries.iter().copied() {
            let Some(effect) = u16::try_from(effect_id).ok().and_then(|id| self.effect(id)) else {
                structural.push(Finding::UnknownEffect { slot, effect_id });
                audit.roles.push(None);
                continue;
            };
            let id = effect.effect_id;
            let role = if expected_innate.contains(&id) && !innate_seen.contains(&id) {
                innate_seen.push(id);
                SlotRole::Innate
            } else if Some(id) == item.set_effect {
                set_seen += 1;
                SlotRole::Set
            } else if self.graces.contains(&id) {
                grace_seen += 1;
                SlotRole::Grace
            } else if effect.flags & HELL_EFFECT_FLAG != 0 {
                hell_seen += 1;
                SlotRole::Hell
            } else {
                SlotRole::Random
            };
            audit.roles.push(Some(role));
            let entry =
                &record[EFFECT_ENTRY_OFFSET + slot * EFFECT_ENTRY_BYTES..][..EFFECT_ENTRY_BYTES];
            let recorded_group = u16::from_le_bytes([entry[0], entry[1]]);
            let category_matches = self
                .index
                .groups_by_key
                .get(&effect.group_key)
                .is_none_or(|group| entry[0xD] & 0x3F == (group.category_key & 0x3F) as u8);
            if recorded_group != effect.group_key || !category_matches {
                definite.push(Finding::ReplacedEffect {
                    slot,
                    recorded_group,
                    original: self
                        .index
                        .effects_by_id
                        .values()
                        .find(|other| other.group_key == recorded_group)
                        .map(|other| other.effect_id),
                });
            }
            let is_star = effect.normalization_flags & STAR_NORMALIZATION_FLAG != 0;
            if is_star != (entry_flags & STAR_ENTRY_FLAG != 0) {
                structural.push(Finding::StarFlagMismatch { slot });
            }
            if is_star {
                stars += 1;
                if effect.rarity_weight(rarity) == 0.0 {
                    structural.push(Finding::StarBelowRarity { slot });
                }
            }
            match role {
                SlotRole::Hell if !hell || !hell_pool.contains(&id) => {
                    structural.push(Finding::HellEffectOnNormal { slot });
                }
                SlotRole::Random if !pool.iter().any(|entry| entry.effect_id == id) => {
                    structural.push(Finding::NotInPool { slot });
                }
                _ => {}
            }
            if matches!(role, SlotRole::Set | SlotRole::Grace) {
                if value != 0 {
                    values.push(Finding::ValueNotNatural { slot, value });
                }
                continue;
            }
            if !matches!(self.roll_reachable(rarity, roll), Ok(true)) {
                values.push(Finding::RollOutOfRange { slot, roll });
            }
            let legal = self.legal_values(id, rarity, level).unwrap_or_default();
            if !legal
                .iter()
                .any(|entry| i64::from(entry.value) == i64::from(value))
            {
                let above = legal
                    .iter()
                    .map(|entry| i64::from(entry.value))
                    .max()
                    .is_some_and(|max| i64::from(value) > max);
                // Only an effect with optional addition fields can exceed the base formula.
                if is_star && above && effect.addition_fields.iter().any(|field| *field != 0) {
                    unverified.push(Finding::ValueAboveFormula { slot, value });
                } else {
                    values.push(Finding::ValueNotNatural { slot, value });
                }
            }
        }
        for expected in &expected_innate {
            if !innate_seen.contains(expected) {
                structural.push(Finding::MissingInnate {
                    expected: *expected,
                });
            }
        }
        if count(SlotRole::Hell) > hell_seen {
            structural.push(Finding::MissingHellEffect);
        }
        if let Some(set) = item.set_effect {
            if set_seen == 0 {
                structural.push(Finding::MissingSet { expected: set });
            }
        }
        if count(SlotRole::Grace) > grace_seen {
            structural.push(Finding::MissingGrace);
        }
        for (position, role) in audit.roles.iter().enumerate() {
            let excess = match role {
                Some(SlotRole::Set) => set_seen > count(SlotRole::Set),
                Some(SlotRole::Grace) => grace_seen > count(SlotRole::Grace),
                _ => false,
            };
            if excess {
                structural.push(Finding::UnexpectedFixed {
                    slot: entries[position].0,
                });
            }
        }
        // A drop carries at most one star, but re-rolling a soul core's random
        // effect offers star candidates whatever the other slots hold (player
        // report, PC v2.02: 伤害反映（心） plus a re-rolled 水属性伤害 0x4AE3).
        if stars > 1 && !item.soul_core() {
            structural.push(Finding::MultipleStars);
        }
        // Innate, set and grace effects are fixed by the item; only drawn
        // effects are subject to group exclusion.
        let drawn: Vec<(usize, u16)> = entries
            .iter()
            .zip(&audit.roles)
            .filter(|(_, role)| matches!(role, Some(SlotRole::Random | SlotRole::Hell)))
            .filter_map(|((slot, id, ..), _)| u16::try_from(*id).ok().map(|id| (*slot, id)))
            .collect();
        for (position, (left_slot, left)) in drawn.iter().enumerate() {
            for (right_slot, right) in drawn.iter().skip(position + 1) {
                if !self
                    .index
                    .is_compatible(*left, &[u32::from(*right)], None)
                    .unwrap_or(true)
                {
                    structural.push(Finding::GroupConflict {
                        slot: *right_slot,
                        other: *left_slot,
                    });
                }
            }
        }
        audit.findings.extend(definite);
        if special {
            unverified.extend(structural);
            unverified.extend(values);
        } else {
            audit.findings.extend(structural);
            audit.findings.extend(values);
        }
        audit.unverified = unverified;
        audit
    }
}
