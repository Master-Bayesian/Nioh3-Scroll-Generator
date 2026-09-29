//! Equipment effect generation from a seed, as the game does it (PC v2.0.2.0).
//!
//! Mirrors `generate_effects` (+0x557F34) for weapons, armor, accessories and
//! soul cores on the reward route (all drop-source context fields zero), then
//! the record builder (+0x5515FC). Checked byte for byte against the game's
//! own code run offline on a snapshot of the running game, across every item
//! kind, rarity 0..=5, difficulties 1..=5 and every progress threshold.
//!
//! The player state the game reads is the current difficulty (the "type
//! class", context `+0x20`) and that difficulty's progress vector (context
//! `+0x24`). A grace-bias effect equipped by the player (key `0xB11A`) and the
//! two runtime predicates are taken as absent, which is one natural state.

use std::collections::HashMap;

use crate::effect::{EffectError, EffectResourceBytes, EffectTableIndex};
use crate::rng::LcgStream;
use crate::sequence::random_int;

/// Effect id of an empty entry.
pub const EMPTY_ENTRY: u32 = 0xFFFF_FFFF;
/// Seeds a generation can start from: the u16 stored at record `+0x22`.
pub const SEED_COUNT: u32 = 0x1_0000;
/// In-memory and save equipment records share this size and layout.
pub const GENERATED_RECORD_BYTES: usize = 0xF0;
/// Game param `0x98FE`: rarity 5 values use the level plus this.
pub const RARITY5_VALUE_LEVEL_BONUS: u16 = 10;
/// The highest stored level (+0x5515AC).
pub const STORED_LEVEL_CAP: u16 = 180;
/// Game param `0xA6D1`: difficulty 5 weight of an effect bound to another area.
const AREA_OTHER_MULTIPLIER: f32 = 0.5;
/// Item `+0x60` of the accessory kind whose values add `+0x550484`.
const SECONDARY_VALUE_KIND: u32 = 0x2D32;
const SOUL_CORE_MODE: u8 = 9;
const SCROLL_MODE: u8 = 0x12;

fn u16_at(row: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([row[offset], row[offset + 1]])
}

fn i16_at(row: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([row[offset], row[offset + 1]])
}

fn u32_at(row: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        row[offset],
        row[offset + 1],
        row[offset + 2],
        row[offset + 3],
    ])
}

fn f32_at(row: &[u8], offset: usize) -> f32 {
    f32::from_bits(u32_at(row, offset))
}

/// What the game reads from the player when it generates an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerState {
    /// Difficulty 1..=5.
    pub type_class: u8,
    /// Progress of that difficulty: three counters and their maximum.
    pub progress: [u32; 4],
}

/// One entry the generator leaves in the item descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedEntry {
    pub effect_id: u32,
    pub roll: u8,
    /// `+0x0D`: category in the low six bits, `0x40` fixed, `0x80` marked.
    pub marker: u8,
    /// `+0x0E`: `1` set, `2` grace, `4` star, `8` marked, `0x20` soul-core mark.
    pub flags: u8,
}

impl GeneratedEntry {
    pub const EMPTY: Self = Self {
        effect_id: EMPTY_ENTRY,
        roll: 0,
        marker: 0,
        flags: 0,
    };

    fn category(&self) -> u16 {
        u16::from(self.marker & 0x3F)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenerationError {
    UnknownItem(u16),
    /// Scrolls use the scroll generator.
    NotEquipment(u16),
    UnsupportedRarity(u8),
    UnsupportedDifficulty(u8),
    Table(EffectError),
}

impl From<EffectError> for GenerationError {
    fn from(error: EffectError) -> Self {
        Self::Table(error)
    }
}

#[derive(Debug, Clone, Copy)]
struct Group {
    key: u16,
    category: u16,
    masks: [u32; 2],
    /// Group `+0x08`: sorts the entry after random ones (grace, set).
    trailing: bool,
}

impl Group {
    fn conflicts(&self, other: &Group) -> bool {
        self.key == other.key
            || self.masks[0] & other.masks[0] != 0
            || self.masks[1] & other.masks[1] != 0
    }

    fn masks_overlap(&self, other: &Group) -> bool {
        self.masks[0] & other.masks[0] != 0 || self.masks[1] & other.masks[1] != 0
    }
}

/// The tables the generator reads, indexed once.
pub struct EquipmentGenerator<'a> {
    resource: &'a EffectResourceBytes,
    index: &'a EffectTableIndex,
    items: HashMap<u16, usize>,
    effects: HashMap<u16, usize>,
    groups: HashMap<u16, Group>,
    count_multipliers: HashMap<u32, usize>,
}

impl<'a> EquipmentGenerator<'a> {
    pub fn new(resource: &'a EffectResourceBytes, index: &'a EffectTableIndex) -> Self {
        let mut items = HashMap::new();
        for row in 0..resource.item.row_count() {
            let raw = resource.item.row(row).unwrap_or_default();
            items.entry(u16_at(raw, 0x152)).or_insert(row);
        }
        let mut effects = HashMap::new();
        // Row 0 is never visited by the game's loops.
        for row in 1..resource.effect.row_count() {
            let raw = resource.effect.row(row).unwrap_or_default();
            effects.entry(u16_at(raw, 0)).or_insert(row);
        }
        let mut groups = HashMap::new();
        for row in 0..resource.effect_group.row_count() {
            let raw = resource.effect_group.row(row).unwrap_or_default();
            groups.entry(u16_at(raw, 0x0C)).or_insert(Group {
                key: u16_at(raw, 0x0C),
                category: u16_at(raw, 0x24),
                masks: [u32_at(raw, 0x54), u32_at(raw, 0x58)],
                trailing: u32_at(raw, 0x08) != 0,
            });
        }
        let mut count_multipliers = HashMap::new();
        for row in 0..resource.category_count_multiplier.row_count() {
            let raw = resource
                .category_count_multiplier
                .row(row)
                .unwrap_or_default();
            count_multipliers.entry(u32_at(raw, 0x1C)).or_insert(row);
        }
        Self {
            resource,
            index,
            items,
            effects,
            groups,
            count_multipliers,
        }
    }

    fn item_row(&self, item_id: u16) -> Option<&'a [u8]> {
        self.items
            .get(&item_id)
            .and_then(|row| self.resource.item.row(*row))
    }

    fn effect_row(&self, effect_id: u32) -> Option<&'a [u8]> {
        let id = u16::try_from(effect_id).ok()?;
        self.effects
            .get(&id)
            .and_then(|row| self.resource.effect.row(*row))
    }

    fn group_of(&self, effect_row: &[u8]) -> Option<Group> {
        self.groups.get(&u16_at(effect_row, 2)).copied()
    }

    /// Whether the item exists and generation supports it (not a scroll).
    pub fn supports(&self, item_id: u16) -> bool {
        self.item_row(item_id)
            .is_some_and(|row| row[0x182] != SCROLL_MODE)
    }

    /// Everything about one item, rarity and player state that does not
    /// depend on the seed, ready for many seeds.
    pub fn prepare(
        &self,
        item_id: u16,
        rarity: u8,
        state: PlayerState,
    ) -> Result<PreparedItem<'_, 'a>, GenerationError> {
        let item = self
            .item_row(item_id)
            .ok_or(GenerationError::UnknownItem(item_id))?;
        let mode = item[0x182];
        if mode == SCROLL_MODE {
            return Err(GenerationError::NotEquipment(item_id));
        }
        if rarity > 5 {
            return Err(GenerationError::UnsupportedRarity(rarity));
        }
        if !(1..=5).contains(&state.type_class) {
            return Err(GenerationError::UnsupportedDifficulty(state.type_class));
        }
        let rarity_row = self
            .resource
            .rarity_roll
            .row(usize::from(rarity))
            .ok_or(GenerationError::UnsupportedRarity(rarity))?;
        let soul_core = mode == SOUL_CORE_MODE;
        let column = capacity_column(item);
        let mut categories = Vec::new();
        let mut caps = [0u8; 32];
        let mut max_key = 0usize;
        for row in 0..self.resource.category.row_count() {
            let raw = self.resource.category.row(row).unwrap_or_default();
            let key = u16_at(raw, 8);
            let rarity_cap = u16_at(raw, 0x18 + 2 * usize::from(rarity));
            let column_cap = u16_at(raw, 0x24 + 6 * column + 2);
            let cap = rarity_cap.min(column_cap);
            if usize::from(key) < caps.len() {
                caps[usize::from(key)] = if key == 0x1A {
                    rarity_cap as u8
                } else {
                    cap as u8
                };
            }
            max_key = max_key.max(usize::from(key));
            let multipliers = self
                .count_multipliers
                .get(&u32::from(u16_at(raw, 0x24 + 6 * column + 4)))
                .and_then(|row| self.resource.category_count_multiplier.row(*row))
                .map(|row| std::array::from_fn(|index| f32_at(row, 4 * index)));
            categories.push(CategoryChoice {
                key,
                cap,
                weight: u16_at(raw, 0x24 + 6 * column),
                multipliers,
            });
        }
        let weight_column = usize::from(u16_at(item, 0x15C));
        let mut pool = Vec::new();
        let mut graces = Vec::new();
        for row in 1..self.resource.effect.row_count() {
            let raw = self.resource.effect.row(row).unwrap_or_default();
            let flags = u32_at(raw, 0x1C);
            if flags & 2 != 0 {
                let weight = (base_weight(raw, state, rarity) * 1000.0) as u32;
                if weight != 0 {
                    graces.push((u32::from(u16_at(raw, 0)), weight));
                }
            }
            let Some(group) = self.group_of(raw) else {
                continue;
            };
            if group.category >= 32 || !context_allowed(raw, item) {
                continue;
            }
            let slot = if weight_column < 0x40 {
                u16_at(raw, 0x58 + 2 * weight_column)
            } else {
                0
            };
            let weight = (f32::from(slot) * 100.0 * base_weight(raw, state, rarity)) as u32;
            if weight == 0 {
                continue;
            }
            pool.push(Candidate {
                effect_id: u16_at(raw, 0),
                group,
                star: u32_at(raw, 0x20) & 8 != 0,
                weight,
            });
        }
        let mut innates = Vec::new();
        let innate_count = if soul_core {
            u32_at(rarity_row, 0x38)
        } else {
            2
        };
        for (position, offset) in [0x158usize, 0x15A].into_iter().enumerate() {
            if position as u32 >= innate_count {
                break;
            }
            let id = u16_at(item, offset);
            if id != 0 {
                let group = self
                    .effect_row(u32::from(id))
                    .map(|row| (row, self.group_of(row)));
                innates.push((id, group.map(|(_, group)| group)));
            }
        }
        let set_effect = u16_at(item, 0x154);
        let special_group = if set_effect != 0 {
            self.effect_row(u32::from(set_effect))
                .and_then(|row| self.group_of(row))
        } else {
            None
        };
        let promotion = if soul_core {
            (0x54, 0xD8)
        } else {
            (0x50, 0xD4)
        };
        Ok(PreparedItem {
            generator: self,
            item_id,
            item,
            rarity,
            state,
            soul_core,
            rarity_row,
            slot_count: u32_at(rarity_row, if soul_core { 0x40 } else { 0x34 }),
            marked: u32_at(rarity_row, if soul_core { 0x3C } else { 0x28 }),
            maybe_marked: if soul_core {
                0
            } else {
                u32_at(rarity_row, 0x2C)
            },
            marker_percent: u32_at(rarity_row, 0x5C),
            promotion_trials: u32_at(rarity_row, promotion.0),
            promotion_threshold: (f32_at(rarity_row, promotion.1) * 100.0) as i32,
            categories,
            count_slots: max_key + 1,
            caps,
            pool,
            graces,
            innates,
            set_effect,
            special_group,
        })
    }
}

/// +0x2FAB8C: the category-table column of the item's kind.
fn capacity_column(item: &[u8]) -> usize {
    match item[0x182] {
        1 => 0,
        2 => 1,
        3 => match u32_at(item, 0x60) {
            0x7B7 => 4,
            0x9A9 => 6,
            0xDF9 => 2,
            0x2B2F => 3,
            0x2D32 => 7,
            0x403B => 5,
            _ => 0,
        },
        9 => 8,
        0x12 => 9,
        _ => 0,
    }
}

/// +0x558ECC with both runtime predicates false.
fn context_allowed(effect: &[u8], item: &[u8]) -> bool {
    let flags = u32_at(effect, 0x1C);
    let item_flags = u32_at(item, 0xB0);
    flags & 0x40 != 0
        && (flags & 4 == 0 || item_flags & 0x800 != 0)
        && (flags & 8 == 0 || item_flags & 0x1000 != 0)
}

/// The float +0x558F3C and +0x5590E0 share: progress gate x difficulty x
/// rarity x (difficulty 5 only) area multiplier, in the game's order.
fn base_weight(effect: &[u8], state: PlayerState, rarity: u8) -> f32 {
    let gate = u16_at(effect, 0x54);
    let bucket = match gate {
        0..=0x1B57 => 0,
        0x1B58..=0x1F3F => 1,
        0x1F40..=0x2327 => 2,
        _ => 3,
    };
    let mut weight = if state.progress[bucket] >= u32::from(gate) {
        1.0f32
    } else {
        0.0
    };
    let kind = match state.type_class {
        3 => 0x48,
        4 => 0x4C,
        5 => 0x50,
        _ => 0x44,
    };
    weight *= f32_at(effect, kind);
    weight *= f32_at(effect, 0x28 + 4 * usize::from(rarity));
    if state.type_class >= 5 && u16_at(effect, 0x56) != 0 {
        // Game param 0x415 (2.0) when the drop area is the effect's own, else
        // 0xA6D1; the reward route has no drop area, so it is never its own.
        weight *= AREA_OTHER_MULTIPLIER;
    }
    weight
}

struct CategoryChoice {
    key: u16,
    cap: u16,
    weight: u16,
    multipliers: Option<[f32; 7]>,
}

#[derive(Clone, Copy)]
struct Candidate {
    effect_id: u16,
    group: Group,
    star: bool,
    weight: u32,
}

fn next_float(rng: &mut LcgStream) -> f32 {
    f32::from(rng.u16()) * (1.0 / 65536.0)
}

fn draw(rng: &mut LcgStream, count: u32) -> u32 {
    random_int(rng, count).unwrap_or(0)
}

/// +0x623660(0, total) over the weights in order; `None` when nothing fits.
fn lottery(rng: &mut LcgStream, weights: impl Iterator<Item = u32> + Clone) -> Option<usize> {
    let total = weights.clone().fold(0u32, u32::wrapping_add);
    let mut ticket = if total > 0 {
        draw(rng, total.wrapping_add(1))
    } else {
        0
    };
    for (position, weight) in weights.enumerate() {
        if ticket <= weight {
            return Some(position);
        }
        ticket -= weight;
    }
    None
}

/// One item at one rarity for one player state.
pub struct PreparedItem<'g, 'a> {
    generator: &'g EquipmentGenerator<'a>,
    item_id: u16,
    item: &'a [u8],
    rarity: u8,
    state: PlayerState,
    soul_core: bool,
    rarity_row: &'a [u8],
    slot_count: u32,
    marked: u32,
    maybe_marked: u32,
    marker_percent: u32,
    promotion_trials: u32,
    promotion_threshold: i32,
    categories: Vec<CategoryChoice>,
    count_slots: usize,
    caps: [u8; 32],
    pool: Vec<Candidate>,
    graces: Vec<(u32, u32)>,
    innates: Vec<(u16, Option<Option<Group>>)>,
    set_effect: u16,
    special_group: Option<Group>,
}

impl PreparedItem<'_, '_> {
    pub fn item_id(&self) -> u16 {
        self.item_id
    }

    pub fn rarity(&self) -> u8 {
        self.rarity
    }

    fn group_of_entry(&self, entry: &GeneratedEntry) -> Option<Group> {
        if entry.effect_id == EMPTY_ENTRY {
            return None;
        }
        self.generator
            .effect_row(entry.effect_id)
            .and_then(|row| self.generator.group_of(row))
    }

    /// +0x2F9AB4.
    fn allocate_categories(
        &self,
        rng: &mut LcgStream,
        entries: &mut [GeneratedEntry; 7],
        count: usize,
        counts: &mut Vec<u32>,
    ) {
        counts.clear();
        counts.resize(self.count_slots.max(64), 0);
        for entry in entries.iter_mut().take(count) {
            let Some(group) = self.group_of_entry(entry) else {
                continue;
            };
            entry.marker = (entry.marker & 0xC0) | (group.category as u8 & 0x3F);
            if entry.flags & 3 == 0 {
                counts[usize::from(group.category)] += 1;
            }
        }
        for entry in entries.iter_mut().take(count) {
            if entry.effect_id != EMPTY_ENTRY {
                continue;
            }
            let mut keys = Vec::with_capacity(self.categories.len());
            let mut weights = Vec::with_capacity(self.categories.len());
            for category in &self.categories {
                let current = counts[usize::from(category.key)];
                if u32::from(category.cap) <= current {
                    continue;
                }
                let multiplier = category.multipliers.map_or(1.0, |values| {
                    values[if current < 7 { current as usize } else { 0 }]
                });
                keys.push(category.key);
                weights.push((f32::from(category.weight) * multiplier) as u32);
            }
            if let Some(position) = lottery(rng, weights.iter().copied()) {
                let key = keys[position];
                entry.marker = (entry.marker & 0xC0) | (key as u8 & 0x3F);
                if entry.flags & 3 == 0 {
                    counts[usize::from(key)] += 1;
                }
            }
        }
    }

    /// +0x1114750 on the reward route.
    fn promote(&self, rng: &mut LcgStream, entries: &mut [GeneratedEntry; 7], count: usize) {
        let mut promoted = 0u32;
        for _ in 0..self.promotion_trials {
            let ticket = ((next_float(rng) * 10000.0) as i32).min(9999);
            if ticket < self.promotion_threshold {
                promoted += 1;
            }
        }
        if self.state.type_class < 3 || promoted == 0 {
            return;
        }
        let mut order = [0usize, 1, 2, 3, 4, 5, 6];
        for position in 0..7 {
            let swap = ((next_float(rng) * 7.0) as usize).min(6);
            order.swap(position, swap);
        }
        let mut done = 0;
        for index in order {
            if index >= count {
                continue;
            }
            let entry = &mut entries[index];
            if entry.marker & 0x40 != 0 || entry.flags & 3 != 0 {
                continue;
            }
            entry.flags |= 4;
            done += 1;
            if done == promoted {
                return;
            }
        }
    }

    /// +0x983E28.
    fn roll(&self, rng: &mut LcgStream) -> u8 {
        let low = u32_at(self.rarity_row, 0x1C);
        let high = u32_at(self.rarity_row, 0x20);
        if low >= high {
            return high as u8;
        }
        let first = draw(rng, 46);
        let second = draw(rng, 46);
        let lottery = first + second + if first == second { 10 } else { 0 };
        ((high - low) as f32 * lottery as f32 / 100.0 + low as f32) as u32 as u8
    }

    /// The seven entries the game generates from `seed` (record `+0x22`).
    pub fn generate(&self, seed: u16) -> [GeneratedEntry; 7] {
        let mut rng = LcgStream::new(u32::from(seed) << 16 | 1);
        let mut entries = [GeneratedEntry::EMPTY; 7];
        let mut counts = Vec::new();
        let mut next = 0usize;
        let mut slot_count = self.slot_count as usize;
        let mut special = None;
        if self.set_effect != 0 {
            entries[next] = GeneratedEntry {
                effect_id: u32::from(self.set_effect),
                roll: 0,
                marker: 0x40,
                flags: 1,
            };
            next += 1;
            slot_count += 1;
            special = self.special_group;
        } else if self.rarity >= 4 && !self.soul_core {
            if let Some(position) = lottery(&mut rng, self.graces.iter().map(|grace| grace.1)) {
                let grace = self.graces[position].0;
                entries[next] = GeneratedEntry {
                    effect_id: grace,
                    roll: 0,
                    marker: 0,
                    flags: 2,
                };
                next += 1;
                special = self
                    .generator
                    .effect_row(grace)
                    .and_then(|row| self.generator.group_of(row));
            }
        }
        let mut accepted: Vec<Group> = Vec::with_capacity(8);
        for (id, row) in &self.innates {
            if let Some(group) = row {
                entries[next] = GeneratedEntry {
                    effect_id: u32::from(*id),
                    roll: u32_at(self.rarity_row, 0x20) as u8,
                    marker: 0x40,
                    flags: 0,
                };
                next += 1;
                accepted.extend(*group);
            }
            if !self.soul_core {
                slot_count += 1;
            }
        }
        for _ in 0..self.marked {
            if next >= 7 {
                break;
            }
            entries[next].marker |= 0x80;
            next += 1;
        }
        for _ in 0..self.maybe_marked {
            if next >= 7 {
                break;
            }
            if draw(&mut rng, 100) < self.marker_percent {
                entries[next].marker |= 0x80;
            }
            next += 1;
        }
        let slot_count = slot_count.min(7);
        self.allocate_categories(&mut rng, &mut entries, slot_count, &mut counts);
        self.promote(&mut rng, &mut entries, slot_count);
        let mut caps = self.caps;
        let mut retry = false;
        let mut wiped = false;
        let mut picks: Vec<usize> = Vec::with_capacity(self.pool.len());
        let mut index = 0;
        while index < slot_count {
            if entries[index].effect_id == EMPTY_ENTRY {
                let entry = entries[index];
                picks.clear();
                for (position, candidate) in self.pool.iter().enumerate() {
                    let category = candidate.group.category;
                    if caps[usize::from(category)] == 0 {
                        continue;
                    }
                    if entry.flags & 4 != 0 {
                        if !candidate.star {
                            continue;
                        }
                    } else if (!retry && entry.category() != 0 && entry.category() != category)
                        || candidate.star
                    {
                        continue;
                    }
                    if accepted
                        .iter()
                        .any(|other| other.conflicts(&candidate.group))
                    {
                        continue;
                    }
                    if special.is_some_and(|group: Group| group.masks_overlap(&candidate.group)) {
                        continue;
                    }
                    picks.push(position);
                }
                let chosen = lottery(
                    &mut rng,
                    picks.iter().map(|position| self.pool[*position].weight),
                )
                .map(|position| self.pool[picks[position]]);
                match chosen {
                    None if !retry => {
                        entries[index].flags &= !4;
                        retry = true;
                        continue;
                    }
                    None => {
                        entries[index] = GeneratedEntry {
                            effect_id: EMPTY_ENTRY,
                            ..GeneratedEntry::EMPTY
                        };
                        wiped = true;
                    }
                    Some(candidate) => {
                        let roll = self.roll(&mut rng);
                        let entry = &mut entries[index];
                        entry.roll = roll;
                        entry.effect_id = u32::from(candidate.effect_id);
                        accepted.push(candidate.group);
                        let category = candidate.group.category;
                        if entry.category() != category {
                            entry.marker = (entry.marker & 0xC0) | (category as u8 & 0x3F);
                            for later in index + 1..slot_count {
                                if entries[later].category() == category {
                                    entries[later].marker &= 0xC0;
                                    self.allocate_categories(
                                        &mut rng,
                                        &mut entries,
                                        slot_count,
                                        &mut counts,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            let entry = entries[index];
            let category = usize::from(entry.category());
            if category < 0x20 && caps[category] != 0 && entry.flags & 3 == 0 {
                caps[category] -= 1;
            }
            retry = false;
            index += 1;
        }
        if wiped {
            // The game means to drop the emptied entries, but its test
            // (+0x558B80) keeps entries whose first word is nonzero, and that
            // word is always zero here: one failed slot empties the item.
            entries = [GeneratedEntry::EMPTY; 7];
        }
        // +0x627ADC: mark one unfixed entry. The game marks entry `pick`
        // itself, not the pick-th eligible entry.
        let percent = u32_at(self.rarity_row, 0x60) * 100;
        if draw(&mut rng, 10000) < percent {
            let eligible = entries
                .iter()
                .filter(|entry| entry.marker & 0x40 == 0 && entry.flags & 3 == 0)
                .count() as u32;
            if eligible > 0 {
                let pick = if eligible > 1 {
                    draw(&mut rng, eligible)
                } else {
                    0
                };
                entries[pick as usize].flags |= 8;
            }
        }
        if self.soul_core {
            // +0x2286FAC: rarity +0x68 is zero in every row, so it only draws.
            let percent = u32_at(self.rarity_row, 0x68) * 100;
            if draw(&mut rng, 10000) < percent {
                let eligible: Vec<usize> = (0..7)
                    .filter(|index| {
                        let entry = entries[*index];
                        entry.marker & 0x40 == 0
                            && entry.flags & 3 == 0
                            && self
                                .generator
                                .effect_row(entry.effect_id)
                                .is_some_and(|row| u32_at(row, 0x20) & 2 != 0)
                    })
                    .collect();
                if !eligible.is_empty() {
                    let pick = if eligible.len() > 1 {
                        draw(&mut rng, eligible.len() as u32)
                    } else {
                        0
                    };
                    entries[eligible[pick as usize]].flags |= 0x20;
                }
            }
        }
        entries
    }

    /// +0x5515FC without a serial: the in-memory record of a generated item.
    /// Save records share the layout (the inventory adds key, serial, the new
    /// marker and the set words).
    pub fn build_record(
        &self,
        entries: &[GeneratedEntry; 7],
        seed: u16,
        level: u16,
        plus: u16,
    ) -> Result<[u8; GENERATED_RECORD_BYTES], GenerationError> {
        let generator = self.generator;
        let item = self.item;
        let mode = item[0x182];
        let mut record = [0u8; GENERATED_RECORD_BYTES];
        record[0..2].copy_from_slice(&self.item_id.to_le_bytes());
        record[2..4].copy_from_slice(&self.item_id.to_le_bytes());
        record[4..6].copy_from_slice(&1u16.to_le_bytes());
        let flags: u32 = 2 | if matches!(mode, 1 | 2 | 3 | 9) {
            0
        } else {
            0x20_0000
        };
        record[0x18..0x1C].copy_from_slice(&flags.to_le_bytes());
        let stored_level = if flags & 0x20_0000 != 0 {
            level
        } else {
            level.min(STORED_LEVEL_CAP)
        };
        record[6..8].copy_from_slice(&stored_level.to_le_bytes());
        record[8..0x0A].copy_from_slice(&level.to_le_bytes());
        let plus = if self.rarity > 2 { plus } else { 0 };
        record[0x0A..0x0C].copy_from_slice(&plus.to_le_bytes());
        record[0x0F] = 0x40;
        record[0x20..0x24].copy_from_slice(&(u32::from(seed) << 16 | 1).to_le_bytes());
        record[0x28..0x30].fill(0xFF);
        record[0x30] = self.rarity;
        let value_level = stored_level
            + if self.rarity >= 5 {
                RARITY5_VALUE_LEVEL_BONUS
            } else {
                0
            };
        let secondary = mode == 3 && u32_at(item, 0x60) == SECONDARY_VALUE_KIND;
        let mut built: Vec<([u8; 0x18], u32)> = Vec::with_capacity(7);
        for entry in entries {
            let mut raw = [0u8; 0x18];
            raw[4..8].copy_from_slice(&entry.effect_id.to_le_bytes());
            raw[0x0C] = entry.roll;
            raw[0x0D] = entry.marker;
            raw[0x0E] = entry.flags;
            let row = if entry.effect_id == EMPTY_ENTRY {
                None
            } else {
                generator.effect_row(entry.effect_id)
            };
            if let Some(row) = row {
                let roll = entry.roll.min(100);
                raw[0x0C] = roll;
                raw[0..2].copy_from_slice(&u16_at(row, 2).to_le_bytes());
                if let Some(group) = generator.group_of(row) {
                    raw[0x0D] = (raw[0x0D] & 0xC0) | (group.category as u8 & 0x3F);
                }
                let mut value =
                    generator
                        .index
                        .resolved_effect_value(entry.effect_id, roll, value_level)?;
                raw[0x0E] &= 0xEF;
                if secondary {
                    value += self.secondary_value(row, roll, value_level)?;
                }
                raw[8..0x0C].copy_from_slice(&value.to_le_bytes());
                raw[0x0E] = (raw[0x0E] & !4) | ((u32_at(row, 0x20) >> 1) as u8 & 4);
            }
            built.push((raw, self.sort_key(entry.effect_id)));
        }
        // +0x551C54: exchange sort by key, repeated until nothing moves.
        let mut swapped = true;
        while swapped {
            swapped = false;
            for first in 0..7 {
                for second in first + 1..7 {
                    if built[second].1 < built[first].1 {
                        built.swap(first, second);
                        swapped = true;
                    }
                }
            }
        }
        for (position, (raw, _)) in built.iter().enumerate() {
            let at = 0x34 + position * 0x18;
            record[at..at + 0x18].copy_from_slice(raw);
        }
        Ok(record)
    }

    /// +0x550484.
    fn secondary_value(&self, row: &[u8], roll: u8, level: u16) -> Result<i32, GenerationError> {
        let scale = self
            .generator
            .index
            .curve_scale(level.min(500), u16_at(row, 0x14))?;
        let low = i32::from(i16_at(row, 0x18));
        let high = i32::from(i16_at(row, 0x1A));
        let base = i32::from(i16_at(row, 0x16));
        let t = f32::from(roll) * 0.01;
        let span = (high - low) as f32 * t + low as f32;
        Ok((scale as f32 * 0.001 * span + base as f32) as i32)
    }

    /// Keys of +0x551C54: innate first, random next, grace and set after,
    /// empty last.
    fn sort_key(&self, effect_id: u32) -> u32 {
        let Some(group) = self
            .generator
            .effect_row(effect_id)
            .and_then(|row| self.generator.group_of(row))
        else {
            return u32::MAX;
        };
        if effect_id == u32::from(u16_at(self.item, 0x158)) {
            1
        } else if effect_id == u32::from(u16_at(self.item, 0x15A)) {
            2
        } else if group.trailing || effect_id == 1 {
            u32::MAX - 1
        } else {
            3
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_entries_sort_last() {
        assert!(GeneratedEntry::EMPTY.effect_id == EMPTY_ENTRY);
        assert_eq!(GeneratedEntry::EMPTY.category(), 0);
    }
}
