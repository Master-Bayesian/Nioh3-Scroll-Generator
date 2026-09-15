//! Offline PC v2.00.02 rarity-4 scroll completion finalizer.
//!
//! Direct port of `nioh3_scroll_editor/r4_finalizer_engine.py` (wrapper
//! RVA 0x22799A8, per-effect RVA 0x1109270, category assignment RVA 0x3DADC4,
//! acceptance loop RVA 0x10280BD). The port is deliberately restricted to the
//! captured playthrough-3, rarity-4, type-`0xE604` context: every other record
//! type, rarity or playthrough is rejected instead of guessed.
//!
//! The engine is standard-library only. Table bytes arrive through
//! [`EffectTableIndex`]; filesystem and hashing stay in the data adapter.

use crate::effect::{
    f32_mul, rng_float01, trunc_f32, CategoryDefinition, EffectError, EffectGroupDefinition,
    EffectTableIndex, NativeWeightContext, CATEGORY_CAPACITY_SLOTS,
};
use crate::record::{
    RecordError, ScrollRecord, ScrollRecordBytes, EFFECT_SLOT_COUNT, EMPTY_EFFECT_ID,
};
use crate::rng::{f32_of, LcgStream};
use crate::sequence::random_int;

/// Record type the captured finalizer is certified for.
pub const SUPPORTED_RECORD_TYPE: u16 = 0xE604;
/// Rarity the captured finalizer is certified for.
pub const SUPPORTED_RARITY: u8 = 4;
/// Playthrough the captured finalizer is certified for.
pub const SUPPORTED_PLAYTHROUGH: u8 = 3;
/// Zero-based index of the physical slot that carries the stage-one Grace.
pub const RARITY4_STAGE_ONE_INDEX: u8 = 4;
/// Native `total + 1` lottery bound is a uint32.
const LOTTERY_MASK: u32 = 0xFFFF_FFFF;
/// The recovered wrapper refuses conflict sets wider than 14 rows.
const MAX_CONFLICT_ROWS: usize = 14;
/// Native attempt count inside one per-effect completion call.
const FINALIZER_ATTEMPTS: u8 = 2;
/// Scoped-seed masks installed at RVA 0x1029204..0x102921E.
const AUXILIARY_MODE_SEED_MASK_LOW: u32 = 0x01E3_C78F;
const AUXILIARY_MODE_SEED_MASK_HIGH: u32 = 0x00E1_C387;
/// Optional-multiplier row key consumed by RVA 0x645DB0 for the mode lottery.
const AUXILIARY_MODE_THRESHOLD_KEY: u32 = 0x1E7D;

/// Fail-closed problems from the recovered R4 completion path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum R4FinalizerError {
    /// A record-codec problem surfaced while reading or writing slots.
    Record(RecordError),
    /// A table problem surfaced while resolving weights or groups.
    Table(EffectError),
    /// The source record is not the certified `0xE604` scroll type.
    UnsupportedRecordType { record_type: u16 },
    /// The source record is not rarity 4.
    UnsupportedRarity { rarity: u8 },
    /// A construction request asked for an uncertified playthrough.
    UnsupportedPlaythrough { playthrough: u8 },
    /// The requested effect index is outside `0..7`.
    InvalidTargetIndex { index: usize },
    /// An existing slot carried an effect id the effect table does not know.
    UnknownExistingEffect { effect_id: u32 },
    /// A wrapper-generated prior row carried an unknown effect id.
    UnknownPriorEffect { effect_id: u32 },
    /// The native wrapper rejects conflict sets wider than 14 rows.
    ConflictSetTooLarge { rows: usize },
    /// A live category count cannot index the native 32-entry vector.
    CategoryOutsideNativeVector { category_key: u16 },
    /// The native `total + 1` lottery bound wrapped to zero.
    LotteryWrapped,
}

impl From<RecordError> for R4FinalizerError {
    fn from(error: RecordError) -> Self {
        Self::Record(error)
    }
}

impl From<EffectError> for R4FinalizerError {
    fn from(error: EffectError) -> Self {
        Self::Table(error)
    }
}

/// Recovered auxiliary mode byte (`+0x1E`) for one displayed Seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxiliaryMode {
    /// Mode byte written to descriptor `+0x1E`.
    pub value: u8,
    /// Scoped seed installed before the lottery.
    pub scoped_seed: u32,
    /// Branch class selected before the row lottery.
    pub branch_class: u8,
    /// Draws consumed by the recovered path.
    pub random_draws: u32,
    /// `special_context` row the mode was read from, when one matched.
    pub selected_row_index: Option<usize>,
}

/// One per-effect completion attempt, mirroring `FinalizerAttemptTrace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinalizerAttemptTrace {
    pub target_index: u8,
    pub assigned_category: u8,
    pub weight_slot: u32,
    pub pool_size: u32,
    pub total_weight: u32,
    pub selected_effect_id: Option<u16>,
    pub roll_percent: Option<u8>,
    pub accepted: bool,
    pub final_rng_state: u32,
}

/// Result of the first-accepted-effect completion loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionResult {
    /// Fully mutated record: unmodified bytes are copied from the source.
    pub record: ScrollRecordBytes,
    /// Zero-based index of the accepted slot, or `None` when nothing accepted.
    pub accepted_index: Option<u8>,
    pub attempts: Vec<FinalizerAttemptTrace>,
}

/// One weighted pool row kept together with the identity the recovered
/// selection and conflict checks need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PoolEntry {
    effect_id: u16,
    weight: i64,
    group_key: u16,
    category_key: u16,
    conflict_mask_0: u32,
    conflict_mask_1: u32,
}

impl PoolEntry {
    fn conflicts_with(&self, existing: &EffectGroupDefinition) -> bool {
        self.group_key == existing.group_key
            || self.conflict_mask_0 & existing.conflict_mask_0 != 0
            || self.conflict_mask_1 & existing.conflict_mask_1 != 0
    }
}

/// Deterministic offline implementation of the captured R4 finalizer.
#[derive(Debug)]
pub struct R4FinalizerEngine<'a> {
    index: &'a EffectTableIndex,
    playthrough: u8,
    category_rows: Vec<&'a CategoryDefinition>,
}

impl<'a> R4FinalizerEngine<'a> {
    /// Build the engine for the certified playthrough-3 context.
    pub fn new(index: &'a EffectTableIndex) -> Result<Self, R4FinalizerError> {
        Self::with_playthrough(index, SUPPORTED_PLAYTHROUGH)
    }

    /// Build the engine while naming the requested playthrough explicitly.
    ///
    /// Anything except playthrough 3 is rejected, matching the reference's
    /// constructor. The required item row, progress vector and rarity row are
    /// resolved eagerly so a missing table fails closed at construction.
    pub fn with_playthrough(
        index: &'a EffectTableIndex,
        playthrough: u8,
    ) -> Result<Self, R4FinalizerError> {
        if playthrough != SUPPORTED_PLAYTHROUGH {
            return Err(R4FinalizerError::UnsupportedPlaythrough { playthrough });
        }
        index.item(SUPPORTED_RECORD_TYPE)?;
        index.playthrough_progress(playthrough)?;
        index.rarity_generation(SUPPORTED_RARITY)?;
        let mut category_rows: Vec<&CategoryDefinition> =
            index.categories_by_key.values().collect();
        category_rows.sort_by_key(|definition| definition.row_index);
        Ok(Self {
            index,
            playthrough,
            category_rows,
        })
    }

    /// Accepted playthrough selector.
    pub fn playthrough(&self) -> u8 {
        self.playthrough
    }

    /// Underlying verified tables.
    pub fn index(&self) -> &EffectTableIndex {
        self.index
    }

    /// Native seed installed at RVA 0x1109659 for one effect call.
    pub fn derive_finalizer_rng_seed(
        source: &ScrollRecordBytes,
        target_index: u8,
    ) -> Result<u32, R4FinalizerError> {
        let target = usize::from(target_index);
        if target >= EFFECT_SLOT_COUNT {
            return Err(R4FinalizerError::InvalidTargetIndex { index: target });
        }
        let display_seed = i64::from(source.displayed_seed());
        let salt16 = i64::from(source.completion_salt());
        let rarity = i64::from(source.signed_rarity());
        let mut total = display_seed + salt16 + rarity * salt16 * (display_seed >> 16);
        total += 7 * (i64::from(target_index) << 16);
        for index in 0..EFFECT_SLOT_COUNT {
            let offset = ScrollRecordBytes::slot_offset(index)?;
            let raw_id_signed = i64::from(source.read_i32(offset + 0x04)?);
            let roll = i64::from(source.read_u8(offset + 0x0C)?.min(100));
            total += raw_id_signed * roll;
        }
        Ok(total as u32)
    }

    /// Exact discard count at RVA 0x1109680.
    pub fn finalizer_discard_count(
        source: &ScrollRecordBytes,
        target_index: u8,
    ) -> Result<u32, R4FinalizerError> {
        let target = usize::from(target_index);
        if target >= EFFECT_SLOT_COUNT {
            return Err(R4FinalizerError::InvalidTargetIndex { index: target });
        }
        Ok((u32::from(target_index) + u32::from(source.completion_salt())) & 0x1F)
    }

    /// Stream installed for one effect call.
    pub fn make_finalizer_rng(
        source: &ScrollRecordBytes,
        target_index: u8,
    ) -> Result<LcgStream, R4FinalizerError> {
        let seed = Self::derive_finalizer_rng_seed(source, target_index)?;
        let mut rng = LcgStream::new(seed);
        for _ in 0..Self::finalizer_discard_count(source, target_index)? {
            rng.u16();
        }
        Ok(rng)
    }

    /// Recovered auxiliary mode byte for one displayed Seed (RVA 0x10291F0).
    pub fn auxiliary_mode(&self, displayed_seed: u32) -> Result<AuxiliaryMode, R4FinalizerError> {
        let threshold_row = self
            .index
            .optional_multiplier_row(AUXILIARY_MODE_THRESHOLD_KEY)?;
        let threshold = trunc_f32(
            f32_mul(
                f32_of(f64::from(threshold_row.base_value)),
                threshold_row.multiplier,
            ),
            "auxiliary_mode_threshold",
        )?;

        let scoped_seed = derive_auxiliary_mode_seed(displayed_seed);
        let mut rng = LcgStream::new(scoped_seed);
        let first_roll = trunc_f32(
            f32_mul(rng_float01(&mut rng), f32_of(10000.0)),
            "auxiliary_mode_roll",
        )?
        .min(9999);
        let mut random_draws = 1u32;

        // EDI starts at 2. A missed threshold consumes one more draw and splits
        // the remaining path evenly into classes 1 and 0 (RVA 0x102928C).
        let mut branch_class = 2u8;
        if first_roll >= threshold {
            let second_roll = trunc_f32(
                f32_mul(rng_float01(&mut rng), f32_of(2.0)),
                "auxiliary_mode_split",
            )?;
            random_draws += 1;
            branch_class = if second_roll == 0 { 1 } else { 0 };
        }

        let matching: Vec<usize> = self
            .index
            .special_context
            .iter()
            .enumerate()
            .filter(|(_, row)| row.branch_class == branch_class)
            .map(|(index, _)| index)
            .collect();
        if matching.is_empty() {
            return Ok(AuxiliaryMode {
                value: 0,
                scoped_seed,
                branch_class,
                random_draws,
                selected_row_index: None,
            });
        }
        let row_count = u32::try_from(matching.len()).expect("context rows fit one uint32");
        let selected = random_int(&mut rng, row_count).expect("non-empty matching set draws a row");
        random_draws += 1;
        let row_index = matching[usize::try_from(selected).expect("draw is inside the set")];
        Ok(AuxiliaryMode {
            value: self.index.special_context[row_index].mode,
            scoped_seed,
            branch_class,
            random_draws,
            selected_row_index: Some(row_index),
        })
    }

    /// Run the first-accepted-effect completion loop (RVA 0x10280BD).
    pub fn finalize_completion(
        &self,
        source: &ScrollRecordBytes,
        reveal: bool,
    ) -> Result<CompletionResult, R4FinalizerError> {
        self.validate_source(source)?;
        let weight_slot = self.weight_slot(source, reveal)?;
        let mut attempts = Vec::new();
        for index in 0..EFFECT_SLOT_COUNT {
            if !source.slot(index)?.completion_loop_eligible() {
                continue;
            }
            let target_index = u8::try_from(index).expect("slot index fits one byte");
            let (candidate, trace) =
                self.build_completion_candidate(source, target_index, reveal, Some(weight_slot))?;
            let accepted = candidate.slot(index)?.completion_candidate_is_accepted();
            attempts.push(trace);
            if accepted {
                return Ok(CompletionResult {
                    record: candidate,
                    accepted_index: Some(target_index),
                    attempts,
                });
            }
        }
        Ok(CompletionResult {
            record: source.clone(),
            accepted_index: None,
            attempts,
        })
    }

    /// Wrapper RVA 0x22799A8, including the generated prior rows.
    pub fn build_completion_candidate(
        &self,
        source: &ScrollRecordBytes,
        target_index: u8,
        reveal: bool,
        resolved_weight_slot: Option<u32>,
    ) -> Result<(ScrollRecordBytes, FinalizerAttemptTrace), R4FinalizerError> {
        self.validate_source(source)?;
        let target = usize::from(target_index);
        if target >= EFFECT_SLOT_COUNT {
            return Err(R4FinalizerError::InvalidTargetIndex { index: target });
        }
        let weight_slot = match resolved_weight_slot {
            Some(slot) => slot,
            None => self.weight_slot(source, reveal)?,
        };

        let mut prior_effect_ids: Vec<u32> = Vec::new();
        for index in 0..target {
            if !source.slot(index)?.wrapper_prior_effect_eligible() {
                continue;
            }
            let prior_index = u8::try_from(index).expect("slot index fits one byte");
            let (candidate, _) = self.finalize_effect(
                source,
                prior_index,
                reveal,
                &prior_effect_ids,
                Some(weight_slot),
            )?;
            let generated = candidate.slot(index)?.raw_id;
            if generated != EMPTY_EFFECT_ID {
                prior_effect_ids.push(generated);
            }
        }
        self.finalize_effect(
            source,
            target_index,
            reveal,
            &prior_effect_ids,
            Some(weight_slot),
        )
    }

    /// One native-equivalent per-effect completion attempt (RVA 0x1109270).
    pub fn finalize_effect(
        &self,
        source: &ScrollRecordBytes,
        target_index: u8,
        reveal: bool,
        prior_effect_ids: &[u32],
        resolved_weight_slot: Option<u32>,
    ) -> Result<(ScrollRecordBytes, FinalizerAttemptTrace), R4FinalizerError> {
        self.validate_source(source)?;
        let target = usize::from(target_index);
        if target >= EFFECT_SLOT_COUNT {
            return Err(R4FinalizerError::InvalidTargetIndex { index: target });
        }

        let mut local = source.clone();
        let slot_count = (0..EFFECT_SLOT_COUNT)
            .filter(|index| {
                source
                    .slot(*index)
                    .map(|slot| slot.prefix_id() != 0)
                    .unwrap_or(false)
            })
            .count();
        let mut rng = Self::make_finalizer_rng(source, target_index)?;
        clear_target_slot(&mut local, target_index)?;
        self.assign_categories(&mut local, slot_count, source.rarity(), &mut rng)?;
        let assigned_category = local.slot(target)?.category();
        let weight_slot = match resolved_weight_slot {
            Some(slot) => slot,
            None => self.weight_slot(source, reveal)?,
        };

        let mut selected: Option<u16> = None;
        let mut total_weight = 0u32;
        let mut pool_size = 0u32;
        for attempt in 0..FINALIZER_ATTEMPTS {
            let pool =
                self.candidate_pool(source, &local, target_index, prior_effect_ids, weight_slot)?;
            pool_size = u32::try_from(pool.len()).expect("pool fits one uint32");
            let (candidate, total) = select_candidate(&pool, &mut rng)?;
            total_weight = total;
            selected = candidate;
            if selected.is_some() {
                break;
            }
            let offset = ScrollRecordBytes::slot_offset(target)?;
            let flags = local.read_u8(offset + 0x0E)?;
            if attempt == 0 {
                local.write_u8(offset + 0x0E, flags & !0x04)?;
            } else {
                clear_target_slot(&mut local, target_index)?;
            }
        }

        let mut roll_percent = None;
        if let Some(effect_id) = selected {
            let roll = self
                .index
                .roll_effect_percentile(source.rarity(), &mut rng, false)?;
            roll_percent = Some(roll);
            self.write_selected_effect(&mut local, target_index, effect_id, roll)?;
        }

        let slot = local.slot(target)?;
        let trace = FinalizerAttemptTrace {
            target_index,
            assigned_category,
            weight_slot,
            pool_size,
            total_weight,
            selected_effect_id: selected,
            roll_percent,
            accepted: slot.completion_candidate_is_accepted(),
            final_rng_state: rng.state(),
        };
        Ok((local, trace))
    }

    /// Build the paired outputs for one stage-one record.
    ///
    /// `stage_one_rng_state` is the stage-one sequence's final state, used only
    /// when no slot was eligible for completion, mirroring the reference's
    /// fallback.
    pub fn build_rarity4_pair(
        &self,
        stage_one: &ScrollRecordBytes,
        stage_one_rng_state: u32,
    ) -> Result<crate::record::Rarity4RecordPair, R4FinalizerError> {
        let completion = self.finalize_completion(stage_one, true)?;
        let final_rng_state = completion
            .attempts
            .last()
            .map_or(stage_one_rng_state, |trace| trace.final_rng_state);
        let terminal_is_special = completion.accepted_index != Some(RARITY4_STAGE_ONE_INDEX);
        let preview_sequence = ScrollRecord::from_rarity4_final_record(
            &completion.record,
            final_rng_state,
            terminal_is_special,
        )?;
        Ok(crate::record::Rarity4RecordPair::new(
            stage_one.clone(),
            completion.record,
            preview_sequence,
            completion.accepted_index,
            completion.attempts,
        ))
    }

    /// The finalizer is certified only for type `0xE604`, rarity 4.
    fn validate_source(&self, source: &ScrollRecordBytes) -> Result<(), R4FinalizerError> {
        let record_type = source.record_type();
        if record_type != SUPPORTED_RECORD_TYPE {
            return Err(R4FinalizerError::UnsupportedRecordType { record_type });
        }
        let rarity = source.rarity();
        if rarity != SUPPORTED_RARITY {
            return Err(R4FinalizerError::UnsupportedRarity { rarity });
        }
        Ok(())
    }

    /// Weight slot resolved by the auxiliary mode and the reveal flag.
    fn weight_slot(
        &self,
        source: &ScrollRecordBytes,
        reveal: bool,
    ) -> Result<u32, R4FinalizerError> {
        let item = self.index.item(source.record_type())?;
        let mut weight_slot = if reveal { 0x3C } else { item.field_15c };
        let mode = self.auxiliary_mode(source.displayed_seed())?.value;
        if let Some(row) = self.index.special_context_row_for_mode(mode)? {
            if row.reveal_weight_flag {
                weight_slot = 0x3D + u32::from(reveal);
            }
        }
        Ok(weight_slot)
    }

    /// Mode-0x12 category assignment at RVA 0x3DADC4.
    fn assign_categories(
        &self,
        record: &mut ScrollRecordBytes,
        slot_count: usize,
        rarity: u8,
        rng: &mut LcgStream,
    ) -> Result<(), R4FinalizerError> {
        let mut counts = [0u32; CATEGORY_CAPACITY_SLOTS];
        for index in 0..slot_count.min(EFFECT_SLOT_COUNT) {
            let slot = record.slot(index)?;
            if slot.raw_id == EMPTY_EFFECT_ID {
                continue;
            }
            let category = self.index.group_for_effect_u32(slot.raw_id)?.category_key;
            let offset = ScrollRecordBytes::slot_offset(index)?;
            let current = record.read_u8(offset + 0x0D)?;
            record.write_u8(offset + 0x0D, (current & 0xC0) | (category as u8 & 0x3F))?;
            if slot.effect_flags & 0x03 == 0 {
                *count_slot(&mut counts, category)? += 1;
            }
        }

        for index in 0..slot_count.min(EFFECT_SLOT_COUNT) {
            let slot = record.slot(index)?;
            if slot.raw_id != EMPTY_EFFECT_ID {
                continue;
            }
            let offset = ScrollRecordBytes::slot_offset(index)?;
            if slot.effect_flags & 0x40 != 0 {
                let current = record.read_u8(offset + 0x0D)?;
                record.write_u8(offset + 0x0D, (current & 0xC0) | 0x1A)?;
                counts[0x1A] += 1;
                continue;
            }

            let mut candidates: Vec<(u16, i64)> = Vec::new();
            let mut total_weight = 0u32;
            for definition in &self.category_rows {
                let category = definition.category_key;
                let capacity = definition.rarity_capacities[usize::from(rarity)]
                    .min(definition.mode12_capacity);
                let count = count_at(&counts, category)?;
                if u32::from(capacity) <= count {
                    continue;
                }
                let mut multiplier = f32_of(1.0);
                if definition.mode12_count_multiplier_key != 0 {
                    multiplier = self.index.category_count_multiplier(
                        u32::from(definition.mode12_count_multiplier_key),
                        usize::try_from(count).unwrap_or(usize::MAX),
                    )?;
                }
                let weight = trunc_f32(
                    f32_mul(
                        f32_of(f64::from(definition.mode12_lottery_weight)),
                        multiplier,
                    ),
                    "category_weight",
                )?;
                candidates.push((category, weight));
                total_weight = total_weight.wrapping_add(weight as u32);
            }
            if candidates.is_empty() {
                continue;
            }

            let mut ticket = random_inclusive(rng, 0, total_weight);
            for (category, weight) in &candidates {
                if i64::from(ticket) <= *weight {
                    let current = record.read_u8(offset + 0x0D)?;
                    record.write_u8(offset + 0x0D, (current & 0xC0) | (*category as u8 & 0x3F))?;
                    let flags = record.read_u8(offset + 0x0E)?;
                    if flags & 0x03 == 0 {
                        *count_slot(&mut counts, *category)? += 1;
                    }
                    break;
                }
                ticket = (i64::from(ticket) - *weight) as u32;
            }
        }
        Ok(())
    }

    /// Remaining per-category capacity vector for a working record.
    fn remaining_category_capacities(
        &self,
        record: &ScrollRecordBytes,
    ) -> Result<[u16; CATEGORY_CAPACITY_SLOTS], R4FinalizerError> {
        let mut capacities = self
            .index
            .category_capacities(record.record_type(), record.rarity())?;
        for index in 0..EFFECT_SLOT_COUNT {
            let slot = record.slot(index)?;
            if slot.prefix_id() == 0 {
                continue;
            }
            let key = usize::from(slot.category());
            if key < capacities.len() && capacities[key] != 0 && slot.effect_flags & 0x03 == 0 {
                capacities[key] -= 1;
            }
        }
        Ok(capacities)
    }

    /// Ordered conflict set: existing slots first, then wrapper prior rows.
    fn conflict_effect_ids(
        &self,
        record: &ScrollRecordBytes,
        prior_effect_ids: &[u32],
    ) -> Result<Vec<u32>, R4FinalizerError> {
        let mut result = Vec::new();
        for index in 0..EFFECT_SLOT_COUNT {
            let slot = record.slot(index)?;
            if slot.prefix_id() == 0 || slot.raw_id == EMPTY_EFFECT_ID {
                continue;
            }
            if self.index.effect_u32(slot.raw_id).is_err() {
                return Err(R4FinalizerError::UnknownExistingEffect {
                    effect_id: slot.raw_id,
                });
            }
            result.push(slot.raw_id);
        }
        for effect_id in prior_effect_ids {
            if *effect_id == EMPTY_EFFECT_ID {
                continue;
            }
            if self.index.effect_u32(*effect_id).is_err() {
                return Err(R4FinalizerError::UnknownPriorEffect {
                    effect_id: *effect_id,
                });
            }
            result.push(*effect_id);
        }
        if result.len() > MAX_CONFLICT_ROWS {
            return Err(R4FinalizerError::ConflictSetTooLarge { rows: result.len() });
        }
        Ok(result)
    }

    /// Weighted candidate pool for one destination slot.
    fn candidate_pool(
        &self,
        source: &ScrollRecordBytes,
        local: &ScrollRecordBytes,
        target_index: u8,
        prior_effect_ids: &[u32],
        weight_slot: u32,
    ) -> Result<Vec<PoolEntry>, R4FinalizerError> {
        let record_type = source.record_type();
        let rarity = source.rarity();
        let source_effect_id = source.slot(usize::from(target_index))?.raw_id;
        let capacities = self.remaining_category_capacities(local)?;
        let conflicts = self.conflict_effect_ids(local, prior_effect_ids)?;
        let conflict_groups: Vec<&EffectGroupDefinition> = conflicts
            .iter()
            .map(|effect_id| self.index.group_for_effect_u32(*effect_id))
            .collect::<Result<_, EffectError>>()?;

        let context = NativeWeightContext {
            record_type,
            rarity,
            playthrough: self.playthrough,
            restricted_destination_slot: false,
            extra_selector: 0,
            rarity5_type_floor: 0,
        };
        let weight_slot = usize::try_from(weight_slot).unwrap_or(usize::MAX);

        let mut result = Vec::new();
        for effect in &self.index.effects_in_row_order {
            if effect.row_index == 0 {
                continue;
            }
            if !self
                .index
                .candidate_context_allowed(effect.effect_id, record_type, false)?
            {
                continue;
            }
            let weight = self.index.native_effect_weight_with_slot(
                effect.effect_id,
                context,
                weight_slot,
            )?;
            if weight == 0 {
                continue;
            }
            let group = self.index.groups_by_key.get(&effect.group_key).ok_or(
                EffectError::UnknownGroup {
                    group_key: effect.group_key,
                },
            )?;
            if u32::from(effect.effect_id) == source_effect_id {
                continue;
            }
            let category = usize::from(group.category_key);
            if category >= capacities.len() || capacities[category] == 0 {
                continue;
            }
            let entry = PoolEntry {
                effect_id: effect.effect_id,
                weight,
                group_key: group.group_key,
                category_key: group.category_key,
                conflict_mask_0: group.conflict_mask_0,
                conflict_mask_1: group.conflict_mask_1,
            };
            if conflict_groups
                .iter()
                .any(|existing| entry.conflicts_with(existing))
            {
                continue;
            }
            result.push(entry);
        }
        Ok(result)
    }

    /// Write the accepted effect into one slot.
    fn write_selected_effect(
        &self,
        record: &mut ScrollRecordBytes,
        target_index: u8,
        effect_id: u16,
        roll_percent: u8,
    ) -> Result<(), R4FinalizerError> {
        let offset = ScrollRecordBytes::slot_offset(usize::from(target_index))?;
        let group = self.index.group_for_effect_u32(u32::from(effect_id))?;
        let effect = self.index.effect_u32(u32::from(effect_id))?;
        let level = record.level();
        record.write_u16(offset, group.group_key)?;
        record.write_u32(offset + 0x04, u32::from(effect_id))?;
        let value = self
            .index
            .resolved_effect_value(u32::from(effect_id), roll_percent, level)?;
        record.write_i32(offset + 0x08, value)?;
        record.write_u8(offset + 0x0C, roll_percent)?;
        let current = record.read_u8(offset + 0x0D)?;
        record.write_u8(
            offset + 0x0D,
            (current & 0xC0) | (group.category_key as u8 & 0x3F),
        )?;
        let completion_flag = ((effect.normalization_flags >> 1) & 0x04) as u8;
        let flags = record.read_u8(offset + 0x0E)?;
        record.write_u8(offset + 0x0E, (flags & !0x04) | completion_flag)?;
        Ok(())
    }
}

/// Inclusive weighted selection at RVA 0x110A26F..0x110A349.
///
/// The reference does not drop zero-weight rows here, so the port keeps the
/// signed weights and reproduces the wrapping uint32 ticket arithmetic.
fn select_candidate(
    pool: &[PoolEntry],
    rng: &mut LcgStream,
) -> Result<(Option<u16>, u32), R4FinalizerError> {
    if pool.is_empty() {
        return Ok((None, 0));
    }
    let mut total: i64 = 0;
    for entry in pool {
        total = total.wrapping_add(entry.weight);
    }
    let total = total as u32;
    let upper_count = total.wrapping_add(1);
    if upper_count == 0 {
        return Err(R4FinalizerError::LotteryWrapped);
    }
    let mut ticket = random_int(rng, upper_count).ok_or(R4FinalizerError::LotteryWrapped)?;
    ticket = ticket.min(total);
    for entry in pool {
        if i64::from(ticket) <= entry.weight {
            return Ok((Some(entry.effect_id), total));
        }
        ticket = (i64::from(ticket) - entry.weight) as u32;
    }
    Ok((None, total))
}

/// Reference `random_inclusive`: an empty span consumes no draw.
fn random_inclusive(rng: &mut LcgStream, low: u32, high: u32) -> u32 {
    let low = low & LOTTERY_MASK;
    let high = high & LOTTERY_MASK;
    if low >= high {
        return low;
    }
    let span = high.wrapping_sub(low).wrapping_add(1);
    low.wrapping_add(random_int(rng, span).unwrap_or(0))
}

/// Field-specific slot clear at RVA 0x110985E.
fn clear_target_slot(
    record: &mut ScrollRecordBytes,
    target_index: u8,
) -> Result<(), R4FinalizerError> {
    let offset = ScrollRecordBytes::slot_offset(usize::from(target_index))?;
    record.write_u16(offset, 0)?;
    record.write_u32(offset + 0x04, EMPTY_EFFECT_ID)?;
    record.write_i32(offset + 0x08, 0)?;
    record.write_u16(offset + 0x0C, 0)?;
    record.write_u8(offset + 0x0E, 0)?;
    record.update_u32(offset + 0x10, |value| value & 0xFFFE_0000)?;
    record.write_u32(offset + 0x14, 0)?;
    Ok(())
}

fn count_slot(
    counts: &mut [u32; CATEGORY_CAPACITY_SLOTS],
    category: u16,
) -> Result<&mut u32, R4FinalizerError> {
    let key = usize::from(category);
    counts
        .get_mut(key)
        .ok_or(R4FinalizerError::CategoryOutsideNativeVector {
            category_key: category,
        })
}

fn count_at(
    counts: &[u32; CATEGORY_CAPACITY_SLOTS],
    category: u16,
) -> Result<u32, R4FinalizerError> {
    let key = usize::from(category);
    counts
        .get(key)
        .copied()
        .ok_or(R4FinalizerError::CategoryOutsideNativeVector {
            category_key: category,
        })
}

/// Scoped RNG seed installed at RVA 0x1029204..0x102921E.
pub fn derive_auxiliary_mode_seed(displayed_seed: u32) -> u32 {
    ((displayed_seed & AUXILIARY_MODE_SEED_MASK_LOW) << 3)
        | ((displayed_seed >> 4) & AUXILIARY_MODE_SEED_MASK_HIGH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::test_support::synthetic_resource_with;
    use crate::effect::{
        EffectResourceBytes, EffectTableBytes, GraceMap, GraceRange,
        CATEGORY_COUNT_MULTIPLIER_ROW_BYTES, CATEGORY_ROW_BYTES, EFFECT_GROUP_ROW_BYTES,
        EFFECT_ROW_BYTES, ITEM_ROW_BYTES, LEVEL_CURVE_ROW_BYTES, OPTIONAL_MULTIPLIER_ROW_BYTES,
        RARITY_ROLL_ROW_BYTES, SCROLL_ITEM_MODE, SCROLL_RECORD_TYPES, SPECIAL_CONTEXT_ROW_BYTES,
    };
    use crate::record::{RecordError, EFFECT_SLOT_BASE, EFFECT_SLOT_STRIDE, SCROLL_RECORD_BYTES};
    use crate::sequence::{
        materialize_ng3_rarity4_final_record, materialize_ng3_rarity4_stage_one_record,
        NG3_RECORD_TYPE,
    };

    /// Effect the recovered write path marks as an accepted completion.
    const ACCEPTED_ID: u16 = 0x2001;
    /// Non-promotable effects. A stage-one ordinary slot cannot offer the Grace
    /// it shares a group with and cannot repeat a group, so five rows are the
    /// minimum that can fill four ordinary slots.
    const PLAIN_IDS: [u16; 5] = [0x2002, 0x2003, 0x2004, 0x2005, 0x2006];
    /// Effect without a completion flag, used as a selectable non-accepting row.
    const PLAIN_ID: u16 = PLAIN_IDS[0];
    const CATEGORY_KEY: u16 = 5;
    /// Threshold base that keeps branch class 2 and resolves auxiliary mode 0.
    const MODE_THRESHOLD_BASE: i32 = 9999;

    struct TableBuilder {
        row_size: usize,
        rows: Vec<u8>,
    }

    impl TableBuilder {
        fn new(row_size: usize) -> Self {
            Self {
                row_size,
                rows: Vec::new(),
            }
        }

        fn row(&mut self) -> &mut [u8] {
            let start = self.rows.len();
            self.rows.resize(start + self.row_size, 0);
            &mut self.rows[start..]
        }

        fn finish(self, name: &'static str) -> EffectTableBytes {
            EffectTableBytes::new(name, self.row_size, self.rows).expect("whole rows")
        }
    }

    fn put_f32(row: &mut [u8], offset: usize, value: f32) {
        row[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    /// Group key the fixture writes for one effect id (index + 1 in table order
    /// beyond the sentinel row).
    fn group_key_for(effect_id: u16) -> u16 {
        match PLAIN_IDS
            .iter()
            .position(|candidate| *candidate == effect_id)
        {
            Some(index) => 0x0101 + index as u16,
            None if effect_id == ACCEPTED_ID => 0x0100,
            None => 0x0101,
        }
    }

    /// Minimal complete resource for the R4 finalizer and materializer paths.
    ///
    /// `canonical_effects` selects the effect rows; the finalizer-only tests use
    /// two rows and the materializer test uses the same two. `reveal_flag` sets
    /// `special_context[+0x2F]` bit 0, which moves the reveal weight slot to
    /// `0x3E`. `roll_maximum` equal to the minimum 90 removes the rarity-roll
    /// draws, which keeps the finalizer assertions deterministic.
    fn finalizer_resource(
        reveal_flag: bool,
        roll_minimum: u32,
        roll_maximum: u32,
        effects: &[(u16, u32)],
    ) -> EffectResourceBytes {
        let item = {
            let mut table = TableBuilder::new(ITEM_ROW_BYTES);
            for record_type in SCROLL_RECORD_TYPES {
                let row = table.row();
                row[0x152..0x154].copy_from_slice(&record_type.to_le_bytes());
                row[0xB0..0xB4].copy_from_slice(&0x1800u32.to_le_bytes());
                row[0x182] = SCROLL_ITEM_MODE;
            }
            table.finish("item")
        };

        let effect_group = {
            let mut table = TableBuilder::new(EFFECT_GROUP_ROW_BYTES);
            for (index, _) in effects.iter().enumerate() {
                let row = table.row();
                row[0x0C..0x0E].copy_from_slice(&(0x0100u16 + index as u16).to_le_bytes());
                row[0x24..0x26].copy_from_slice(&CATEGORY_KEY.to_le_bytes());
            }
            table.finish("effect_group")
        };

        let category = {
            let mut table = TableBuilder::new(CATEGORY_ROW_BYTES);
            let row = table.row();
            row[0x08..0x0A].copy_from_slice(&CATEGORY_KEY.to_le_bytes());
            row[0x20..0x22].copy_from_slice(&12u16.to_le_bytes());
            row[0x5A..0x5C].copy_from_slice(&12u16.to_le_bytes());
            row[0x5C..0x5E].copy_from_slice(&12u16.to_le_bytes());
            table.finish("category")
        };

        let category_count_multiplier = {
            let mut table = TableBuilder::new(CATEGORY_COUNT_MULTIPLIER_ROW_BYTES);
            let row = table.row();
            for slot in 0..7 {
                put_f32(row, slot * 4, 1.0);
            }
            table.finish("category_count_multiplier")
        };

        let effect = {
            let mut table = TableBuilder::new(EFFECT_ROW_BYTES);
            let sentinel = table.row();
            sentinel[0x02..0x04].copy_from_slice(&0x0100u16.to_le_bytes());
            for (index, (effect_id, normalization_flags)) in effects.iter().enumerate() {
                let row = table.row();
                row[0x00..0x02].copy_from_slice(&effect_id.to_le_bytes());
                row[0x02..0x04].copy_from_slice(&(0x0100u16 + index as u16).to_le_bytes());
                row[0x08..0x0A].copy_from_slice(&0i16.to_le_bytes());
                row[0x0A..0x0C].copy_from_slice(&0i16.to_le_bytes());
                row[0x0C..0x0E].copy_from_slice(&100i16.to_le_bytes());
                row[0x1C..0x20].copy_from_slice(&0x40u32.to_le_bytes());
                row[0x20..0x24].copy_from_slice(&normalization_flags.to_le_bytes());
                put_f32(row, 0x38, 1.0);
                for slot in (0x44..0x54).step_by(4) {
                    put_f32(row, slot, 1.0);
                }
                for slot in 0..64 {
                    row[0x58 + slot * 2..0x5A + slot * 2].copy_from_slice(&1u16.to_le_bytes());
                }
            }
            table.finish("effect")
        };

        let level_curve = {
            let mut table = TableBuilder::new(LEVEL_CURVE_ROW_BYTES);
            for level in 0..=700u16 {
                let row = table.row();
                let value: u16 = match level {
                    0..=180 => 1000,
                    500 => 5000,
                    _ => 9000,
                };
                for slot in 0..3 {
                    row[slot * 2..slot * 2 + 2].copy_from_slice(&value.to_le_bytes());
                }
            }
            table.finish("level_curve")
        };

        let optional_multiplier = {
            let mut table = TableBuilder::new(OPTIONAL_MULTIPLIER_ROW_BYTES);
            let row = table.row();
            row[0x10..0x14].copy_from_slice(&MODE_THRESHOLD_BASE.to_le_bytes());
            row[0x14..0x18].copy_from_slice(&AUXILIARY_MODE_THRESHOLD_KEY.to_le_bytes());
            put_f32(row, 0x18, 1.0);
            table.finish("optional_multiplier")
        };

        let rarity_roll = {
            let mut table = TableBuilder::new(RARITY_ROLL_ROW_BYTES);
            for _ in 0..6 {
                let row = table.row();
                row[0x1C..0x20].copy_from_slice(&roll_minimum.to_le_bytes());
                row[0x20..0x24].copy_from_slice(&roll_maximum.to_le_bytes());
                row[0x44..0x48].copy_from_slice(&4u32.to_le_bytes());
                row[0x4C..0x50].copy_from_slice(&6u32.to_le_bytes());
                row[0x58..0x5C].copy_from_slice(&1u32.to_le_bytes());
                put_f32(row, 0xDC, 0.0);
            }
            table.finish("rarity_roll")
        };

        let special_context = {
            let mut table = TableBuilder::new(SPECIAL_CONTEXT_ROW_BYTES);
            let row = table.row();
            row[0x28] = 0;
            row[0x29] = 2;
            row[0x2F] = u8::from(reveal_flag);
            table.finish("special_context")
        };

        let mut playthrough_progress = Vec::new();
        for selector in 1..=5u32 {
            for value in [selector, 0, 0, 0] {
                playthrough_progress.extend_from_slice(&value.to_le_bytes());
            }
        }

        EffectResourceBytes {
            schema: "nioh3-r4-finalizer-resource/v1".to_string(),
            item,
            effect_group,
            category,
            category_count_multiplier,
            effect,
            level_curve,
            optional_multiplier,
            rarity_roll,
            special_context,
            bonus_curve_rows: Vec::new(),
            bonus_curve_index: Vec::new(),
            playthrough_progress,
            grace_maps: Vec::new(),
        }
    }

    fn two_effect_resource(reveal_flag: bool, roll_maximum: u32) -> EffectResourceBytes {
        finalizer_resource(
            reveal_flag,
            90,
            roll_maximum,
            &[(ACCEPTED_ID, 0x08), (PLAIN_ID, 0)],
        )
    }

    fn finalizer_index(reveal_flag: bool, roll_maximum: u32) -> EffectTableIndex {
        EffectTableIndex::from_resource(&two_effect_resource(reveal_flag, roll_maximum))
            .expect("synthetic finalizer resource indexes")
    }

    /// Deterministic index whose rarity roll consumes no draw.
    fn deterministic_index() -> EffectTableIndex {
        finalizer_index(false, 90)
    }

    /// One record fixture with the requested slots populated.
    fn stage_fixture(level: u16, slots: &[(u8, u16, u8)]) -> ScrollRecordBytes {
        let mut record = ScrollRecordBytes::zeroed();
        record.write_u16(0x00, NG3_RECORD_TYPE).unwrap();
        record.write_u16(0x06, level).unwrap();
        record.write_u16(0x08, level).unwrap();
        record.write_u32(0x20, 1).unwrap();
        record.write_u8(0x30, SUPPORTED_RARITY).unwrap();
        record.write_u8(0x31, SUPPORTED_RARITY).unwrap();
        for (index, effect_id, effect_flags) in slots {
            let offset = ScrollRecordBytes::slot_offset(usize::from(*index)).unwrap();
            record.write_u16(offset, group_key_for(*effect_id)).unwrap();
            record
                .write_u32(offset + 0x04, u32::from(*effect_id))
                .unwrap();
            record.write_u8(offset + 0x0C, 90).unwrap();
            record.write_u8(offset + 0x0D, CATEGORY_KEY as u8).unwrap();
            record.write_u8(offset + 0x0E, *effect_flags).unwrap();
        }
        record
    }

    /// Dense Grace map whose ids exist in the synthetic effect table.
    ///
    /// The ids are the two non-promotable rows, because a stage-one ordinary
    /// slot cannot offer a Grace that shares its own group.
    fn stage_map(first_id: u16, second_id: u16) -> GraceMap {
        GraceMap {
            format: "nioh3-grace-first-u16-map-v2".to_string(),
            game_version: "2.00.02".to_string(),
            record_type: u32::from(NG3_RECORD_TYPE),
            rarity: SUPPORTED_RARITY,
            capture_state: crate::effect::GRACE_MAP_CAPTURE_STATE.to_string(),
            effect_slot: 5,
            ranges: vec![
                GraceRange {
                    start: 0,
                    end: 0x7FFF,
                    effect_id: u32::from(first_id),
                },
                GraceRange {
                    start: 0x8000,
                    end: 0xFFFF,
                    effect_id: u32::from(second_id),
                },
            ],
        }
    }

    #[test]
    fn finalizer_rng_seed_and_discard_count_match_the_recovered_formula() {
        let source = stage_fixture(180, &[(0, PLAIN_ID, 0)]);
        // display_seed 1 + salt 0 + rarity 4 * salt 0 + 7 * slot 0 +
        // raw_id 0x2002 * roll 90.
        assert_eq!(
            R4FinalizerEngine::derive_finalizer_rng_seed(&source, 0).unwrap(),
            1 + 0x2002 * 90
        );
        assert_eq!(
            R4FinalizerEngine::derive_finalizer_rng_seed(&source, 6).unwrap(),
            1 + 0x2002 * 90 + 7 * (6 << 16)
        );
        // Untouched slots read as zero id and zero roll, so they add nothing.
        let empty = stage_fixture(180, &[]);
        assert_eq!(
            R4FinalizerEngine::derive_finalizer_rng_seed(&empty, 0).unwrap(),
            1
        );
        // The completion salt at +0x0C participates in both formulas.
        let mut salted = stage_fixture(180, &[]);
        salted.write_u16(0x0C, 3).unwrap();
        assert_eq!(
            R4FinalizerEngine::derive_finalizer_rng_seed(&salted, 2).unwrap(),
            1 + 3 + 7 * (2 << 16)
        );
        assert_eq!(
            R4FinalizerEngine::finalizer_discard_count(&salted, 2).unwrap(),
            5
        );
        assert_eq!(
            R4FinalizerEngine::derive_finalizer_rng_seed(&source, 7).unwrap_err(),
            R4FinalizerError::InvalidTargetIndex { index: 7 }
        );
    }

    #[test]
    fn completion_accepts_the_first_eligible_slot_and_preserves_other_bytes() {
        let index = deterministic_index();
        let engine = R4FinalizerEngine::new(&index).unwrap();
        let source = stage_fixture(180, &[(0, PLAIN_ID, 0)]);
        let result = engine.finalize_completion(&source, true).unwrap();
        assert_eq!(result.accepted_index, Some(0));
        assert_eq!(result.attempts.len(), 1);
        assert_eq!(result.attempts[0].selected_effect_id, Some(ACCEPTED_ID));
        assert_eq!(result.attempts[0].roll_percent, Some(90));
        assert_eq!(result.attempts[0].assigned_category, CATEGORY_KEY as u8);
        assert_eq!(result.attempts[0].weight_slot, 0x3C);
        assert!(result.attempts[0].accepted);

        // Everything outside the accepted slot is copied verbatim.
        let slot_end = EFFECT_SLOT_BASE + EFFECT_SLOT_STRIDE;
        assert_eq!(
            result.record.as_bytes()[slot_end..],
            source.as_bytes()[slot_end..]
        );
        assert_eq!(
            result.record.as_bytes()[..EFFECT_SLOT_BASE],
            source.as_bytes()[..EFFECT_SLOT_BASE]
        );
        // The recovered write path installs the group key and the accept flag.
        let slot = result.record.slot(0).unwrap();
        assert_eq!(slot.raw_id, u32::from(ACCEPTED_ID));
        assert_eq!(slot.prefix_id(), group_key_for(ACCEPTED_ID));
        assert_eq!(slot.effect_flags & 0x04, 0x04);
    }

    #[test]
    fn non_accepting_completion_returns_the_source_and_reports_the_attempt() {
        let index = deterministic_index();
        let engine = R4FinalizerEngine::new(&index).unwrap();
        // The destination holds the only accepting row, and the pool excludes the
        // source id, so the non-accepting row is selected and rejected.
        let source = stage_fixture(180, &[(0, ACCEPTED_ID, 0)]);
        let result = engine.finalize_completion(&source, true).unwrap();
        assert_eq!(result.accepted_index, None);
        assert_eq!(result.record, source);
        assert_eq!(result.attempts.len(), 1);
        assert_eq!(result.attempts[0].selected_effect_id, Some(PLAIN_ID));
        assert_eq!(result.attempts[0].pool_size, 1);
        assert_eq!(result.attempts[0].total_weight, 100);
        assert!(!result.attempts[0].accepted);

        // A table with no other candidate reports the empty pool instead.
        let single = finalizer_resource(false, 90, 90, &[(ACCEPTED_ID, 0x08)]);
        let single_index = EffectTableIndex::from_resource(&single).unwrap();
        let single_engine = R4FinalizerEngine::new(&single_index).unwrap();
        let empty_attempt = single_engine
            .finalize_effect(&source, 0, true, &[], None)
            .unwrap();
        assert_eq!(empty_attempt.1.selected_effect_id, None);
        assert_eq!(empty_attempt.1.pool_size, 0);
        assert_eq!(empty_attempt.1.total_weight, 0);
        // The reference leaves the destination cleared when both attempts find
        // no candidate, so the target slot is emptied and nothing else moves.
        let cleared = empty_attempt.0.slot(0).unwrap();
        assert_eq!(cleared.raw_id, EMPTY_EFFECT_ID);
        assert_eq!(cleared.prefix_id(), 0);
        assert_eq!(cleared.roll_percent, 0);
        assert_eq!(
            empty_attempt.0.as_bytes()[EFFECT_SLOT_BASE + EFFECT_SLOT_STRIDE..],
            source.as_bytes()[EFFECT_SLOT_BASE + EFFECT_SLOT_STRIDE..]
        );
    }

    #[test]
    fn no_change_pair_keeps_both_sides_identical_and_independent() {
        let index = deterministic_index();
        let engine = R4FinalizerEngine::new(&index).unwrap();
        let source = stage_fixture(180, &[(0, ACCEPTED_ID, 0)]);
        let pair = engine.build_rarity4_pair(&source, 0x1234_5678).unwrap();
        assert_eq!(pair.accepted_index(), None);
        assert!(!pair.completion_changed_record());
        assert_eq!(pair.install_record(), pair.preview_record());
        // An attempt ran and was rejected, so the preview state is the last
        // attempt's state rather than the stage-one fallback.
        assert_eq!(pair.attempts().len(), 1);
        assert_eq!(
            pair.preview_sequence().final_rng_state,
            pair.attempts()[0].final_rng_state
        );
        assert_ne!(pair.preview_sequence().final_rng_state, 0x1234_5678);
        // The pair owns two buffers, so neither side can alias the other.
        let mut mutated = pair.preview_record().clone();
        mutated.write_u8(0x00, 0xFF).unwrap();
        assert_eq!(
            pair.install_record().as_bytes()[0x00],
            NG3_RECORD_TYPE as u8
        );
        assert_ne!(pair.install_record(), &mutated);
    }

    #[test]
    fn changed_pair_separates_install_and_preview() {
        let index = deterministic_index();
        let engine = R4FinalizerEngine::new(&index).unwrap();
        let source = stage_fixture(180, &[(0, PLAIN_ID, 0)]);
        let pair = engine.build_rarity4_pair(&source, 0).unwrap();
        assert_eq!(pair.accepted_index(), Some(0));
        assert!(pair.completion_changed_record());
        assert_eq!(pair.install_record(), &source);
        assert_ne!(pair.preview_record(), &source);
        assert_eq!(pair.preview_sequence().effects.len(), 5);
        assert_eq!(
            pair.preview_sequence().effects[0].effect_id,
            u32::from(ACCEPTED_ID)
        );
        assert_eq!(pair.preview_sequence().seed, source.displayed_seed());
        assert_eq!(pair.preview_sequence().level, source.level());
    }

    #[test]
    fn weight_slot_follows_the_auxiliary_reveal_flag() {
        let plain = deterministic_index();
        let engine = R4FinalizerEngine::new(&plain).unwrap();
        let source = stage_fixture(180, &[(0, PLAIN_ID, 0)]);
        // Mode 0 resolves to a `special_context` row whose flag is clear, so the
        // reveal path keeps 0x3C and the non-reveal path keeps the item selector.
        assert_eq!(engine.auxiliary_mode(1).unwrap().value, 0);
        assert_eq!(
            engine
                .finalize_effect(&source, 0, true, &[], None)
                .unwrap()
                .1
                .weight_slot,
            0x3C
        );
        assert_eq!(
            engine
                .finalize_effect(&source, 0, false, &[], None)
                .unwrap()
                .1
                .weight_slot,
            0x00
        );

        let revealed = finalizer_index(true, 90);
        let engine = R4FinalizerEngine::new(&revealed).unwrap();
        assert_eq!(
            engine
                .finalize_effect(&source, 0, true, &[], None)
                .unwrap()
                .1
                .weight_slot,
            0x3E
        );
        assert_eq!(
            engine
                .finalize_effect(&source, 0, false, &[], None)
                .unwrap()
                .1
                .weight_slot,
            0x3D
        );
    }

    #[test]
    fn level_flows_through_the_write_path_and_clamps_at_500() {
        let index = deterministic_index();
        let engine = R4FinalizerEngine::new(&index).unwrap();
        let written_value = |level: u16| {
            let source = stage_fixture(level, &[(0, PLAIN_ID, 0)]);
            engine
                .finalize_completion(&source, true)
                .unwrap()
                .record
                .slot(0)
                .unwrap()
                .value
        };
        let low = written_value(1);
        let high = written_value(500);
        assert_eq!(written_value(180), low);
        assert!(
            low < high,
            "level must move the written value: {low} !< {high}"
        );
        assert_eq!(written_value(700), high, "the curve clamps at level 500");
    }

    #[test]
    fn unsupported_contexts_and_records_fail_closed() {
        let index = deterministic_index();
        assert_eq!(
            R4FinalizerEngine::with_playthrough(&index, 4).unwrap_err(),
            R4FinalizerError::UnsupportedPlaythrough { playthrough: 4 }
        );
        let engine = R4FinalizerEngine::new(&index).unwrap();

        let short = [0u8; SCROLL_RECORD_BYTES - 1];
        assert_eq!(
            ScrollRecordBytes::from_slice(&short).unwrap_err(),
            RecordError::Length {
                expected: SCROLL_RECORD_BYTES,
                actual: SCROLL_RECORD_BYTES - 1
            }
        );
        assert!(ScrollRecordBytes::from_slice(&[0u8; SCROLL_RECORD_BYTES + 1]).is_err());

        let mut wrong_type = stage_fixture(180, &[(0, PLAIN_ID, 0)]);
        wrong_type.write_u16(0x00, 0x1E82).unwrap();
        assert_eq!(
            engine.finalize_completion(&wrong_type, true).unwrap_err(),
            R4FinalizerError::UnsupportedRecordType {
                record_type: 0x1E82
            }
        );

        let mut wrong_rarity = stage_fixture(180, &[(0, PLAIN_ID, 0)]);
        wrong_rarity.write_u8(0x30, 3).unwrap();
        assert_eq!(
            engine.finalize_completion(&wrong_rarity, true).unwrap_err(),
            R4FinalizerError::UnsupportedRarity { rarity: 3 }
        );

        let source = stage_fixture(180, &[(0, PLAIN_ID, 0)]);
        assert_eq!(
            engine
                .finalize_effect(&source, 7, true, &[], None)
                .unwrap_err(),
            R4FinalizerError::InvalidTargetIndex { index: 7 }
        );
        assert_eq!(
            engine
                .build_completion_candidate(&source, 9, true, None)
                .unwrap_err(),
            R4FinalizerError::InvalidTargetIndex { index: 9 }
        );

        // A populated slot outside the processed prefix range whose id is absent
        // from the table fails during the conflict walk.
        let unknown = stage_fixture(180, &[(0, PLAIN_ID, 0), (5, 0xBEEF, 0)]);
        assert_eq!(
            engine.finalize_completion(&unknown, true).unwrap_err(),
            R4FinalizerError::UnknownExistingEffect { effect_id: 0xBEEF }
        );

        // A processed populated slot with an unknown id fails earlier, while the
        // category pass resolves its group.
        let unknown_in_prefix = stage_fixture(180, &[(0, PLAIN_ID, 0), (1, 0xBEEF, 0)]);
        assert_eq!(
            engine
                .finalize_completion(&unknown_in_prefix, true)
                .unwrap_err(),
            R4FinalizerError::Table(EffectError::UnknownEffect { effect_id: 0xBEEF })
        );

        // A missing progress vector fails at construction.
        let mut table_only = two_effect_resource(false, 90);
        table_only.playthrough_progress.clear();
        let table_only = EffectTableIndex::from_resource(&table_only).unwrap();
        assert_eq!(
            R4FinalizerEngine::new(&table_only).unwrap_err(),
            R4FinalizerError::Table(EffectError::PlaythroughSelector { selector: 3 })
        );
    }

    #[test]
    fn materializers_bind_the_sequence_to_the_template() {
        // Stage-one ordinary slots cannot offer a promotion-capable row, so the
        // materializer needs a non-promotable row per ordinary slot.
        let mut effects = vec![(ACCEPTED_ID, 0x08u32)];
        effects.extend(PLAIN_IDS.iter().map(|effect_id| (*effect_id, 0u32)));
        let resource = finalizer_resource(false, 90, 94, &effects);
        let index =
            EffectTableIndex::from_resource(&resource).expect("synthetic finalizer resource");
        let engine = R4FinalizerEngine::new(&index).unwrap();
        let map = stage_map(PLAIN_IDS[0], PLAIN_IDS[1]);
        let mut template = ScrollRecordBytes::zeroed();
        template.write_u16(0x00, NG3_RECORD_TYPE).unwrap();
        let (stage_one, sequence) =
            materialize_ng3_rarity4_stage_one_record(&index, &map, &template, 7, 180, 3, 5, 9)
                .unwrap();
        assert_eq!(stage_one.record_type(), NG3_RECORD_TYPE);
        assert_eq!(stage_one.displayed_seed(), 7);
        assert_eq!(stage_one.level(), 180);
        assert_eq!(stage_one.recommended_level(), 3);
        assert_eq!(stage_one.generation_serial(), 5);
        assert_eq!(stage_one.transfer_count(), 9);
        assert_eq!(stage_one.completion_salt(), 0);
        assert_eq!(sequence.effects.len(), 5);
        assert_eq!(
            &stage_one.as_bytes()[EFFECT_SLOT_BASE..EFFECT_SLOT_BASE + 5 * EFFECT_SLOT_STRIDE],
            &sequence.serialize_rarity4_stage_one_slots().unwrap()[..5 * EFFECT_SLOT_STRIDE]
        );

        let pair =
            materialize_ng3_rarity4_final_record(&index, &map, &template, 7, 180, 3, 5, 9).unwrap();
        assert_eq!(pair.install_record(), &stage_one);
        assert_eq!(
            pair.preview_record(),
            &engine.finalize_completion(&stage_one, true).unwrap().record
        );
        assert_eq!(
            pair,
            engine
                .build_rarity4_pair(&stage_one, sequence.final_rng_state)
                .unwrap()
        );

        // A template of another scroll type is refused before any table use.
        let mut foreign = ScrollRecordBytes::zeroed();
        foreign.write_u16(0x00, 0x516D).unwrap();
        assert_eq!(
            materialize_ng3_rarity4_stage_one_record(&index, &map, &foreign, 7, 180, 3, 5, 9)
                .unwrap_err(),
            crate::sequence::SequenceError::TemplateRecordType {
                record_type: 0x516D
            }
        );
    }

    #[test]
    fn serializer_guards_the_supported_context() {
        let index = EffectTableIndex::from_resource(&synthetic_resource_with(0.0))
            .expect("synthetic sequence resource indexes");
        let record = crate::sequence::generate_ng3_rarity4_stage_one_effect_sequence(
            &index,
            &crate::effect::test_support::dense_map(u32::from(NG3_RECORD_TYPE), 4, 5),
            11,
            180,
        )
        .unwrap();
        assert!(record.serialize_rarity4_stage_one_slots().is_ok());
        let mut rarity3 = record.clone();
        rarity3.rarity = 3;
        assert_eq!(
            rarity3.serialize_rarity4_stage_one_slots().unwrap_err(),
            RecordError::UnsupportedEffectContext
        );
        let mut same_type = record;
        same_type.record_type = 0x1E82;
        assert_eq!(
            same_type.serialize_rarity4_stage_one_slots().unwrap_err(),
            RecordError::UnsupportedEffectContext
        );
    }
}
