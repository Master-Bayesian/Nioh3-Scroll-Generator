//! Ordinary-completion replacement and extra-painting prediction.
//!
//! Certified context: PC 2.0.2.0 NG3 rarity 4, ordinary completion. Revelation
//! trigger/slot selection is deliberately not inferred. Branch records are
//! simulation inputs, never installation records or proof of complete byte parity.

use crate::effect::{EffectError, EffectTableIndex, NativeWeightContext, WeightedEffectCandidate};
use crate::record::{RecordError, ScrollRecordBytes};
use crate::rng::{lottery_10000, LcgStream};

#[derive(Debug)]
pub enum CompletionError {
    UnsupportedContext,
    UnknownRecordSemantics,
    NoAttempts,
    CounterOverflow,
    EmptyPool,
    Table(EffectError),
    Record(RecordError),
}
impl From<EffectError> for CompletionError {
    fn from(error: EffectError) -> Self {
        Self::Table(error)
    }
}
impl From<RecordError> for CompletionError {
    fn from(error: RecordError) -> Self {
        Self::Record(error)
    }
}

#[derive(Debug, Clone)]
pub struct Replacement {
    pub slot: usize,
    pub effect_id: u16,
    pub roll: u8,
    pub value: i32,
}
#[derive(Debug)]
pub struct Painting {
    pub eligible: bool,
    pub draw: u32,
    pub threshold: u32,
    pub success: bool,
}
#[derive(Debug)]
pub struct Branch {
    pub choice: Option<usize>,
    pub painting_effect: Option<Replacement>,
    pub record: ScrollRecordBytes,
}
#[derive(Debug)]
pub struct CompletionPrediction {
    pub candidates: Vec<Replacement>,
    pub painting: Painting,
    pub branches: Vec<Branch>,
}

fn seed(record: &ScrollRecordBytes, slot: usize) -> Result<u32, CompletionError> {
    let displayed = record.displayed_seed();
    let counter = u32::from(record.completion_salt());
    let mut state = displayed
        .wrapping_add(counter)
        .wrapping_add((4 * counter).wrapping_mul(displayed >> 16))
        .wrapping_add(((slot as u32) << 16).wrapping_mul(7));
    for i in 0..7 {
        let e = record.slot(i)?;
        // Low32 wrapping is identical to summing the signed native operands.
        state = state.wrapping_add(e.raw_id.wrapping_mul(u32::from(e.roll_percent.min(100))));
    }
    Ok(state)
}

fn generate(
    tables: &EffectTableIndex,
    record: &ScrollRecordBytes,
    slot: usize,
    prior: &[u32],
    painting: bool,
) -> Result<Replacement, CompletionError> {
    let selected = record.slot(slot)?;
    let mut caps = tables.category_capacities(record.record_type(), 4)?;
    let mut existing = prior.to_vec();
    for i in 0..7 {
        if i == slot {
            continue;
        }
        let e = record.slot(i)?;
        if e.prefix_id() == 0 {
            continue;
        }
        existing.push(e.raw_id);
        let category = usize::from(e.category());
        if category < 32 && caps[category] > 0 && e.effect_flags & 3 == 0 {
            caps[category] -= 1;
        }
    }
    let context = NativeWeightContext {
        record_type: 0xE604,
        rarity: 4,
        playthrough: 3,
        restricted_destination_slot: false,
        extra_selector: 0,
        rarity5_type_floor: 5,
    };
    let mut pool = Vec::new();
    for effect in &tables.effects_in_row_order {
        if effect.row_index == 0 || effect.effect_id as u32 == selected.raw_id {
            continue;
        }
        let Some(group) = tables.groups_by_key.get(&effect.group_key) else {
            continue;
        };
        if group.group_key == 0
            || group.category_key >= 32
            || caps[group.category_key as usize] == 0
            || !tables.candidate_context_allowed(effect.effect_id, 0xE604, false)?
            || !tables.is_compatible(effect.effect_id, &existing, None)?
        {
            continue;
        }
        let weight = tables.native_effect_weight_with_slot(
            effect.effect_id,
            context,
            if painting { 0x3D } else { 0x3E },
        )?;
        if weight > 0 {
            pool.push(WeightedEffectCandidate {
                effect_id: effect.effect_id,
                weight,
            });
        }
    }
    let mut rng = LcgStream::new(seed(record, slot)?);
    for _ in 0..(((slot + usize::from(record.completion_salt())) & 31) + usize::from(!painting)) {
        rng.u16();
    }
    let chosen = tables
        .select_weighted_candidate(&pool, &mut rng)?
        .ok_or(CompletionError::EmptyPool)?;
    let roll = tables.roll_effect_percentile(4, &mut rng, false)?;
    Ok(Replacement {
        slot,
        effect_id: chosen.effect_id,
        roll,
        value: tables.resolved_effect_value(u32::from(chosen.effect_id), roll, record.level())?,
    })
}

fn apply(
    tables: &EffectTableIndex,
    record: &mut ScrollRecordBytes,
    replacement: &Replacement,
) -> Result<(), CompletionError> {
    let group = tables
        .group_for_effect(replacement.effect_id)
        .ok_or(CompletionError::UnknownRecordSemantics)?;
    let old = record.slot(replacement.slot)?;
    let at = ScrollRecordBytes::slot_offset(replacement.slot)?;
    record.write_u32(at, u32::from(group.group_key))?;
    record.write_u32(at + 4, u32::from(replacement.effect_id))?;
    record.write_i32(at + 8, replacement.value)?;
    record.write_u8(at + 12, replacement.roll)?;
    record.write_u8(
        at + 13,
        (old.category_and_flags & 0xC0) | (group.category_key as u8 & 0x3F),
    )?;
    Ok(())
}

pub fn predict(
    tables: &EffectTableIndex,
    record: &ScrollRecordBytes,
) -> Result<CompletionPrediction, CompletionError> {
    if record.record_type() != 0xE604 || record.read_u8(0x30)? != 4 {
        return Err(CompletionError::UnsupportedContext);
    }
    if record.read_u8(0x33)? == 0 {
        return Err(CompletionError::NoAttempts);
    }
    if record.completion_salt() == u16::MAX {
        return Err(CompletionError::CounterOverflow);
    }
    if record.level() > 180 {
        return Err(CompletionError::UnknownRecordSemantics);
    }
    let mut occupied = 0;
    let mut slots = Vec::new();
    for i in 0..7 {
        let e = record.slot(i)?;
        if e.prefix_id() == 0 {
            if !e.is_empty() {
                return Err(CompletionError::UnknownRecordSemantics);
            }
            continue;
        }
        occupied += 1;
        let effect = tables.effect_u32(e.raw_id)?;
        if effect.group_key != e.prefix_id() {
            return Err(CompletionError::UnknownRecordSemantics);
        }
        if e.category_and_flags & 0x40 == 0 && e.effect_flags & 2 == 0 && e.raw_id != 1 {
            slots.push(i);
        }
    }
    let mut prior = Vec::new();
    let mut candidates = Vec::new();
    for slot in slots {
        let candidate = generate(tables, record, slot, &prior, false)?;
        prior.push(u32::from(candidate.effect_id));
        candidates.push(candidate);
    }
    let pity = record.read_u8(0x32)?;
    let param = |key| -> Result<f32, CompletionError> {
        let row = tables.optional_multiplier_row(key)?;
        Ok((row.base_value as f32 * row.multiplier) * 0.01_f32)
    };
    let threshold = ((param(0xD56F)? + param(0xAA65)? * f32::from(pity)).min(param(0x3472)?)
        * 10000.0_f32) as u32;
    let mut rng = LcgStream::new(!record.displayed_seed());
    let mut draw = 0;
    for _ in 0..=pity {
        draw = lottery_10000(rng.u16()) as u32;
    }
    let eligible = occupied < 6
        && record.slot(5)?.prefix_id() == 0
        && record.read_u32(0x18)? & 0x20000000 == 0;
    let painting = Painting {
        eligible,
        draw,
        threshold,
        success: eligible && draw < threshold,
    };
    let mut branches = Vec::new();
    for choice in std::iter::once(None).chain(candidates.iter().map(|v| Some(v.slot))) {
        let mut after = record.clone();
        if let Some(slot) = choice {
            let replacement = candidates
                .iter()
                .find(|v| v.slot == slot)
                .ok_or(CompletionError::UnknownRecordSemantics)?;
            apply(tables, &mut after, replacement)?;
        }
        after.write_u16(0x0C, record.completion_salt() + 1)?;
        after.write_u8(0x33, record.read_u8(0x33)? - 1)?;
        let painting_effect = if painting.success {
            let mut effect = generate(tables, &after, 5, &[], true)?;
            apply(tables, &mut after, &effect)?;
            // Native insertion sorts the new random effect before the grace.
            if after.slot(4)?.effect_flags & 2 != 0 {
                let grace = after.as_bytes()[0x94..0xAC].to_vec();
                let added = after.as_bytes()[0xAC..0xC4].to_vec();
                after.write_bytes(0x94, &added)?;
                after.write_bytes(0xAC, &grace)?;
                effect.slot = 4;
            }
            after.update_u32(0x18, |flags| flags | 0x20000000)?;
            Some(effect)
        } else {
            if painting.eligible {
                after.write_u8(0x32, pity.saturating_add(1))?;
            }
            None
        };
        branches.push(Branch {
            choice,
            painting_effect,
            record: after,
        });
    }
    Ok(CompletionPrediction {
        candidates,
        painting,
        branches,
    })
}
