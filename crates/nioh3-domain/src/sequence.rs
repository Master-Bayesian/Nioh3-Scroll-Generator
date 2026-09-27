//! NG3 effect-sequence generation with exact RNG replay.
//!
//! This is a direct port of `nioh3_scroll_editor/effect_sequence.py` for the
//! ordinary NG3 paths: the rarity-3 growing sequence, the rarity-4 stage-one
//! sequence, and the rarity-5 Grace sequence (playthrough 3 with the shipped
//! maps; NG4/NG5 need a matching captured map that the shipped resource does
//! not contain, so they fail closed). The R4 finalized-preview/stage-one
//! pairing, candidate identity and worker transport stay outside this slice.

use crate::effect::{
    CandidatePoolRequest, EffectError, EffectTableIndex, GraceMap, NativeWeightContext,
    PromotedSlotRequest, CATEGORY_CAPACITY_SLOTS, EFFECT_FLAG_PRIMARY, EFFECT_FLAG_PROMOTED,
};
use crate::record::{Rarity4RecordPair, RecordError, ScrollRecordBytes};
use crate::record::{ScrollEffect, ScrollRecord};
use crate::rng::{f32_of, LcgStream};

/// NG3 scroll record type used by every ordinary sequence path.
pub const NG3_RECORD_TYPE: u16 = 0xE604;
/// Growing (rarity 3) records.
pub const RARITY_GROWING: u8 = 3;
/// Finalizable (rarity 4) records.
pub const RARITY_FINALIZABLE: u8 = 4;
/// Divine (rarity 5) records.
pub const RARITY_DIVINE: u8 = 5;
pub const CHALLENGE_COUNT_SEED_MASK: u32 = 0x001F_C07F;
pub const MIN_CHALLENGE_ATTEMPTS: i32 = 4;
pub const MAX_CHALLENGE_ATTEMPTS: i32 = 7;
/// Serialized effect slots in one scroll record.
pub const EFFECT_SLOT_COUNT: u8 = 7;
/// `CATEGORY_TO_TYPE` from `emaki_exchange`, indexed by category/playthrough.
pub const CATEGORY_TO_TYPE: [u16; 6] = [0x0000, 0x1E82, 0x516D, 0xE604, 0xDD82, 0xD523];
/// Physical slot carrying the rarity-4 stage-one Grace.
pub const RARITY4_STAGE_ONE_SLOT: u8 = 5;
/// Physical slot carrying the rarity-5 Grace.
pub const RARITY5_GRACE_SLOT: u8 = 6;
/// Fixed growing token occupying serialized slot 5 of a rarity-3 record.
pub const RARITY3_GROWING_TOKEN: u32 = 0x0001;
/// Effect flags carried by the fixed growing token.
pub const RARITY3_TOKEN_EFFECT_FLAGS: u8 = 0x84;
/// Effect flags marked on source slot 0 of both Grace paths.
pub const SOURCE_GRACE_EFFECT_FLAGS: u8 = 0x02;
/// NG3 playthrough selector used by the ordinary scroll paths.
pub const NG3_PLAYTHROUGH: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceError {
    /// `displayed_seed` did not fit the uint32 contract.
    DisplayedSeedOutOfRange(u64),
    /// A table problem surfaced while preparing the sequence.
    Table(EffectError),
    /// A sequence requested a playthrough outside 1..=5.
    UnsupportedPlaythrough(u8),
    /// The supplied Grace map is not the context the path requires.
    GraceMapContext { field: &'static str },
    /// The native candidate pool became empty before a slot could be filled.
    EmptyCandidatePool { rarity: u8 },
    /// A category capacity would underflow while accepting an effect.
    CategoryCapacityUnderflow { category: u8 },
    /// The claimed draw count disagrees with the draws actually consumed.
    DrawCountMismatch { expected: u32, actual: u64 },
    /// A recovered record codec or context guard rejected the input.
    Record(RecordError),
    /// A record of another scroll type was handed to an NG3 materializer.
    TemplateRecordType { record_type: u16 },
    /// The R4 finalizer rejected the stage-one record it was given.
    Finalizer(crate::r4_finalizer::R4FinalizerError),
}

impl From<EffectError> for SequenceError {
    fn from(error: EffectError) -> Self {
        Self::Table(error)
    }
}

impl From<RecordError> for SequenceError {
    fn from(error: RecordError) -> Self {
        Self::Record(error)
    }
}

impl From<crate::r4_finalizer::R4FinalizerError> for SequenceError {
    fn from(error: crate::r4_finalizer::R4FinalizerError) -> Self {
        Self::Finalizer(error)
    }
}

/// Scroll record type of playthrough 1..=5 (`CATEGORY_TO_TYPE`).
pub fn record_type_for_playthrough(playthrough: u8) -> Result<u16, SequenceError> {
    match playthrough {
        1..=5 => Ok(CATEGORY_TO_TYPE[usize::from(playthrough)]),
        other => Err(SequenceError::UnsupportedPlaythrough(other)),
    }
}

/// Scoped Seed transformation at native RVA 0x10283FA.
pub fn derive_challenge_count_seed(displayed_seed: u32) -> u32 {
    (displayed_seed & CHALLENGE_COUNT_SEED_MASK) << 7
        | (displayed_seed >> 7) & CHALLENGE_COUNT_SEED_MASK
}

/// Integer draw in `0..count-1` with the game's binary32 intermediate steps.
pub fn random_int(rng: &mut LcgStream, count: u32) -> Option<u32> {
    if count == 0 {
        return None;
    }
    // Each step stays in binary32, matching the reference's f32_mul chain.
    let random_float: f32 = f32_of(f64::from(rng.u16())) * f32_of(1.0 / 65536.0);
    let scaled: f32 = random_float * f32_of(f64::from(count));
    let result = scaled as u32;
    Some(result.min(count - 1))
}

/// Record byte +0x33 through native RVAs 0x10283F0/0x227FB10.
pub fn generate_challenge_attempt_count(displayed_seed: u32) -> i32 {
    let mut rng = LcgStream::new(derive_challenge_count_seed(displayed_seed));
    let span = (MAX_CHALLENGE_ATTEMPTS - MIN_CHALLENGE_ATTEMPTS + 1) as u32;
    MIN_CHALLENGE_ATTEMPTS + random_int(&mut rng, span).expect("span is non-zero") as i32
}

/// The exact NG3 rarity-3 growing sequence (four ordinary effects plus the
/// fixed `0x0001` growing token).
pub fn generate_ng3_rarity3_effect_sequence(
    index: &EffectTableIndex,
    seed: u32,
    level: u16,
) -> Result<ScrollRecord, SequenceError> {
    generate_rarity3_effect_sequence(index, NG3_PLAYTHROUGH, seed, level)
}

/// The rarity-3 growing sequence for any playthrough's scroll type. Only NG3 is
/// certified; other playthroughs are held to live native parity per context.
pub fn generate_rarity3_effect_sequence(
    index: &EffectTableIndex,
    playthrough: u8,
    seed: u32,
    level: u16,
) -> Result<ScrollRecord, SequenceError> {
    let record_type = record_type_for_playthrough(playthrough)?;
    let mut rng = LcgStream::new(seed);
    let source_effect_flags = [0u8, 0, 0, 0, RARITY3_TOKEN_EFFECT_FLAGS, 0, 0];
    let promoted = index.select_promoted_slot_indexes(
        PromotedSlotRequest {
            record_type,
            rarity: RARITY_GROWING,
            category_and_flags: &[0u8; 7],
            effect_flags: &source_effect_flags,
            // The fixed growing token occupies serialized slot 5 but is not a
            // source slot for the native promotion shuffle.
            slot_limit: Some(4),
            rarity5_type_floor: 0,
        },
        &mut rng,
    )?;
    let capacities = index.category_capacities(record_type, RARITY_GROWING)?;
    let mut builder = SequenceBuilder::new(
        index,
        rng,
        SequenceIdentity {
            record_type,
            rarity: RARITY_GROWING,
            playthrough,
            level,
            primary_source_index: 0,
            special_effect_id: Some(RARITY3_GROWING_TOKEN),
        },
        capacities,
        promoted,
    );

    let mut effects = Vec::with_capacity(usize::from(EFFECT_SLOT_COUNT));
    for source_index in 0..4u8 {
        effects.push(builder.ordinary_slot(source_index, source_index + 1)?);
    }
    effects.push(terminal_token_effect(
        index,
        RARITY3_GROWING_TOKEN,
        5,
        4,
        RARITY3_TOKEN_EFFECT_FLAGS,
        0,
    )?);

    let random_draws = 1 + if builder.promoted.is_empty() { 0 } else { 7 } + 4 * 3;
    builder.finish(seed, RARITY_GROWING, level, effects, random_draws, true)
}

/// The exact NG3 rarity-4 native stage-one sequence.
///
/// The fifth effect is a transient completion token. The rarity-4 finalizer
/// owns the canonical record, so this path deliberately stops at stage one.
pub fn generate_ng3_rarity4_stage_one_effect_sequence(
    index: &EffectTableIndex,
    grace_map: &GraceMap,
    seed: u32,
    level: u16,
) -> Result<ScrollRecord, SequenceError> {
    generate_rarity4_stage_one_effect_sequence(index, grace_map, NG3_PLAYTHROUGH, seed, level)
}

/// The rarity-4 stage-one sequence for any playthrough's scroll type, with that
/// context's captured stage-one map.
pub fn generate_rarity4_stage_one_effect_sequence(
    index: &EffectTableIndex,
    grace_map: &GraceMap,
    playthrough: u8,
    seed: u32,
    level: u16,
) -> Result<ScrollRecord, SequenceError> {
    let record_type = record_type_for_playthrough(playthrough)?;
    validate_rarity4_stage_one_map(grace_map, record_type)?;

    let mut rng = LcgStream::new(seed);
    let first_u16 = rng.u16();
    let special_id = grace_map
        .grace_id_for_first_u16(first_u16)
        .ok_or(SequenceError::GraceMapContext { field: "ranges" })?;
    let source_effect_flags = [SOURCE_GRACE_EFFECT_FLAGS, 0, 0, 0, 0, 0, 0];
    let promoted = index.select_promoted_slot_indexes(
        PromotedSlotRequest {
            record_type,
            rarity: RARITY_FINALIZABLE,
            category_and_flags: &[0u8; 7],
            effect_flags: &source_effect_flags,
            slot_limit: Some(5),
            rarity5_type_floor: 0,
        },
        &mut rng,
    )?;
    let capacities = index.category_capacities(record_type, RARITY_FINALIZABLE)?;
    let mut builder = SequenceBuilder::new(
        index,
        rng,
        SequenceIdentity {
            record_type,
            rarity: RARITY_FINALIZABLE,
            playthrough,
            level,
            primary_source_index: 1,
            special_effect_id: Some(special_id),
        },
        capacities,
        promoted,
    );

    let mut effects = Vec::with_capacity(usize::from(EFFECT_SLOT_COUNT));
    for source_index in 1..=4u8 {
        effects.push(builder.ordinary_slot(source_index, source_index)?);
    }
    effects.push(terminal_token_effect(
        index,
        special_id,
        RARITY4_STAGE_ONE_SLOT,
        0,
        SOURCE_GRACE_EFFECT_FLAGS,
        index.resolved_effect_value(special_id, 0, level)?,
    )?);

    let random_draws = 1 + 1 + if builder.promoted.is_empty() { 0 } else { 7 } + 4 * 3;
    builder.finish(seed, RARITY_FINALIZABLE, level, effects, random_draws, true)
}

/// Ordered rarity-5 effects for one playthrough's Grace context.
///
/// The returned order is the normalized display/save order: primary, ordinary
/// effects (including any promoted effect), then Grace.
pub fn generate_rarity5_grace_effect_sequence(
    index: &EffectTableIndex,
    grace_map: &GraceMap,
    playthrough: u8,
    seed: u32,
    level: u16,
) -> Result<ScrollRecord, SequenceError> {
    let record_type = record_type_for_playthrough(playthrough)?;
    validate_grace_map(grace_map, playthrough)?;

    let mut rng = LcgStream::new(seed);
    let first_u16 = rng.u16();
    let grace_id = grace_map
        .grace_id_for_first_u16(first_u16)
        .ok_or(SequenceError::GraceMapContext { field: "ranges" })?;
    let source_effect_flags = [SOURCE_GRACE_EFFECT_FLAGS, 0, 0, 0, 0, 0, 0];
    let promoted = index.select_promoted_slot_indexes(
        PromotedSlotRequest {
            record_type,
            rarity: RARITY_DIVINE,
            category_and_flags: &[0u8; 7],
            effect_flags: &source_effect_flags,
            slot_limit: Some(6),
            rarity5_type_floor: 0,
        },
        &mut rng,
    )?;
    let capacities = index.category_capacities(record_type, RARITY_DIVINE)?;
    let mut builder = SequenceBuilder::new(
        index,
        rng,
        SequenceIdentity {
            record_type,
            rarity: RARITY_DIVINE,
            playthrough,
            level,
            primary_source_index: 1,
            special_effect_id: Some(grace_id),
        },
        capacities,
        promoted,
    );

    let mut effects = Vec::with_capacity(usize::from(EFFECT_SLOT_COUNT));
    for source_index in 1..=5u8 {
        effects.push(builder.ordinary_slot(source_index, source_index)?);
    }
    effects.push(terminal_token_effect(
        index,
        grace_id,
        RARITY5_GRACE_SLOT,
        0,
        SOURCE_GRACE_EFFECT_FLAGS,
        index.resolved_effect_value(grace_id, 0, level)?,
    )?);

    let random_draws = 1 + 1 + if builder.promoted.is_empty() { 0 } else { 7 } + 5 * 3;
    builder.finish(seed, RARITY_DIVINE, level, effects, random_draws, true)
}

/// Rarity-5 records without a Grace (NG1/NG2 scroll types): six ordinary
/// effects, source 0 primary, and no draw-1 special. Held to live parity
/// against the native generator per context.
pub fn generate_rarity5_plain_effect_sequence(
    index: &EffectTableIndex,
    playthrough: u8,
    seed: u32,
    level: u16,
) -> Result<ScrollRecord, SequenceError> {
    let record_type = record_type_for_playthrough(playthrough)?;
    let mut rng = LcgStream::new(seed);
    let promoted = index.select_promoted_slot_indexes(
        PromotedSlotRequest {
            record_type,
            rarity: RARITY_DIVINE,
            category_and_flags: &[0u8; 7],
            effect_flags: &[0u8; 7],
            slot_limit: Some(6),
            rarity5_type_floor: 0,
        },
        &mut rng,
    )?;
    let capacities = index.category_capacities(record_type, RARITY_DIVINE)?;
    let mut builder = SequenceBuilder::new(
        index,
        rng,
        SequenceIdentity {
            record_type,
            rarity: RARITY_DIVINE,
            playthrough,
            level,
            primary_source_index: 0,
            special_effect_id: None,
        },
        capacities,
        promoted,
    );
    let mut effects = Vec::with_capacity(usize::from(EFFECT_SLOT_COUNT));
    for source_index in 0..6u8 {
        effects.push(builder.ordinary_slot(source_index, source_index + 1)?);
    }
    let random_draws = 1 + if builder.promoted.is_empty() { 0 } else { 7 } + 6 * 3;
    builder.finish(seed, RARITY_DIVINE, level, effects, random_draws, false)
}

/// NG1/NG2 rarity-4 stage-one special map `(first u16 start, end, Grace)`.
///
/// Measured live on PC v2.02 at the title screen (65536 native generations per
/// context): identical for NG1 and NG2 and at levels 180 and 120, and the same
/// method reproduces the shipped NG3 map range for range. Evidence:
/// `docs/knowledge/V082_NG12_OFFLINE_PARITY_20260927.md`.
const NG12_RARITY4_STAGE_ONE_RANGES: [(u16, u16, u32); 10] = [
    (0, 6559, 0x23E5),
    (6560, 13112, 0x2AE6),
    (13113, 19665, 0x8CCC),
    (19666, 26218, 0xB24F),
    (26219, 32771, 0x5012),
    (32772, 39324, 0x7BEA),
    (39325, 45877, 0x590C),
    (45878, 52430, 0x4FA3),
    (52431, 58983, 0xB1E9),
    (58984, 65535, 0xE8EB),
];

/// The embedded rarity-4 stage-one map of playthrough 1 or 2.
pub fn ng12_rarity4_stage_one_map(playthrough: u8) -> Result<GraceMap, SequenceError> {
    if !matches!(playthrough, 1 | 2) {
        return Err(SequenceError::UnsupportedPlaythrough(playthrough));
    }
    Ok(GraceMap {
        format: "nioh3-grace-first-u16-map-v2".to_string(),
        game_version: "2.02".to_string(),
        record_type: u32::from(record_type_for_playthrough(playthrough)?),
        rarity: RARITY_FINALIZABLE,
        capture_state: "ng12-live-native-title-screen".to_string(),
        effect_slot: RARITY4_STAGE_ONE_SLOT,
        ranges: NG12_RARITY4_STAGE_ONE_RANGES
            .iter()
            .map(|&(start, end, effect_id)| crate::effect::GraceRange {
                start,
                end,
                effect_id,
            })
            .collect(),
    })
}

/// Whether rarity-5 records of this playthrough carry a Grace (NG3 and later).
pub fn rarity5_has_grace(playthrough: u8) -> bool {
    playthrough >= NG3_PLAYTHROUGH
}

/// The preview sequence one Seed composes to in a playthrough/rarity context:
/// rarity 3 as generated, rarity 4 finalized, rarity 5 with or without its
/// Grace. `r4_map`/`r5_map` are the context's captured maps; NG1/NG2 need none
/// (their rarity-4 map is embedded and their rarity 5 has no Grace).
pub fn compose_preview_sequence(
    index: &EffectTableIndex,
    playthrough: u8,
    rarity: u8,
    seed: u32,
    level: u16,
    r4_map: Option<&GraceMap>,
    r5_map: Option<&GraceMap>,
) -> Result<ScrollRecord, SequenceError> {
    let record_type = record_type_for_playthrough(playthrough)?;
    match rarity {
        RARITY_GROWING => generate_rarity3_effect_sequence(index, playthrough, seed, level),
        RARITY_FINALIZABLE => {
            let embedded;
            let map = match r4_map {
                Some(map) => map,
                None => {
                    embedded = ng12_rarity4_stage_one_map(playthrough).map_err(|_| {
                        SequenceError::GraceMapContext {
                            field: "record_type",
                        }
                    })?;
                    &embedded
                }
            };
            let mut template = ScrollRecordBytes::zeroed();
            template.write_u16(0x00, record_type)?;
            let pair = materialize_rarity4_final_record(
                index,
                map,
                playthrough,
                &template,
                seed,
                level,
                0,
                0,
                0,
            )?;
            Ok(pair.preview_sequence().clone())
        }
        RARITY_DIVINE if !rarity5_has_grace(playthrough) => {
            generate_rarity5_plain_effect_sequence(index, playthrough, seed, level)
        }
        RARITY_DIVINE => {
            let map = r5_map.ok_or(SequenceError::GraceMapContext { field: "ranges" })?;
            generate_rarity5_grace_effect_sequence(index, map, playthrough, seed, level)
        }
        other => Err(SequenceError::Table(EffectError::UnsupportedRarity {
            rarity: other,
        })),
    }
}

/// Backward-compatible verified NG3 rarity-5 entry point.
pub fn generate_ng3_rarity5_effect_sequence(
    index: &EffectTableIndex,
    grace_map: &GraceMap,
    seed: u32,
    level: u16,
) -> Result<ScrollRecord, SequenceError> {
    generate_rarity5_grace_effect_sequence(index, grace_map, NG3_PLAYTHROUGH, seed, level)
}

/// Bind one NG3 rarity-4 stage-one sequence to a real save template.
///
/// Mirrors `effect_sequence.materialize_ng3_rarity4_stage_one_record`: every
/// template byte outside the lineage fields, the rarity pair, the challenge
/// count, the seven effect slots and the transfer count is preserved verbatim.
/// The result is an internal native generation stage and is deliberately not a
/// safe install artifact on its own; use
/// [`materialize_ng3_rarity4_final_record`] for the paired outputs.
///
/// The reference's range checks on `seed`, `level`, `recommended_level`,
/// `generation_serial` and `transfer_count` are structural here: every one of
/// them is already a `u32`/`u16`.
// The positional parameter list mirrors the reference materializer's keyword
// arguments one-for-one, so callers and the parity gate stay directly
// comparable.
#[allow(clippy::too_many_arguments)]
pub fn materialize_ng3_rarity4_stage_one_record(
    index: &EffectTableIndex,
    stage_one_grace_map: &GraceMap,
    template: &ScrollRecordBytes,
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
    transfer_count: u32,
) -> Result<(ScrollRecordBytes, ScrollRecord), SequenceError> {
    materialize_rarity4_stage_one_record(
        index,
        stage_one_grace_map,
        NG3_PLAYTHROUGH,
        template,
        seed,
        level,
        recommended_level,
        generation_serial,
        transfer_count,
    )
}

/// [`materialize_ng3_rarity4_stage_one_record`] for any playthrough; the
/// template must carry that playthrough's scroll type.
#[allow(clippy::too_many_arguments)]
pub fn materialize_rarity4_stage_one_record(
    index: &EffectTableIndex,
    stage_one_grace_map: &GraceMap,
    playthrough: u8,
    template: &ScrollRecordBytes,
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
    transfer_count: u32,
) -> Result<(ScrollRecordBytes, ScrollRecord), SequenceError> {
    let template_type = template.record_type();
    if template_type != record_type_for_playthrough(playthrough)? {
        return Err(SequenceError::TemplateRecordType {
            record_type: template_type,
        });
    }
    let sequence = generate_rarity4_stage_one_effect_sequence(
        index,
        stage_one_grace_map,
        playthrough,
        seed,
        level,
    )?;

    let mut record = template.clone();
    // +0x0C is the R4 completion salt, not a lineage field. A newly generated
    // stage-one record starts from the canonical zero salt used by the native
    // receive path, so an inherited non-zero salt cannot shift the finalizer
    // RNG stream away from the game-closed preview.
    record.write_u16(0x0C, 0)?;
    record.write_u16(0x06, level)?;
    record.write_u16(0x08, level)?;
    record.write_u16(0x10, recommended_level)?;
    record.write_u16(0x12, recommended_level)?;
    record.write_u32(0x20, seed)?;
    record.write_u32(0x28, generation_serial)?;
    record.write_u8(0x30, RARITY_FINALIZABLE)?;
    record.write_u8(0x31, RARITY_FINALIZABLE)?;
    record.write_u8(0x33, challenge_attempt_count_byte(seed))?;
    record.write_bytes(
        crate::record::EFFECT_SLOT_BASE,
        &sequence.serialize_rarity4_stage_one_slots()?,
    )?;
    record.write_u32(0xDC, transfer_count)?;
    Ok((record, sequence))
}

/// Build the paired rarity-4 outputs for one template.
///
/// Composes the stage-one materializer with the native completion pass, so the
/// returned pair carries both the record the save must receive and the preview
/// the game's reveal path will produce.
// See [`materialize_ng3_rarity4_stage_one_record`] for why the argument list is
// positional and complete.
#[allow(clippy::too_many_arguments)]
pub fn materialize_ng3_rarity4_final_record(
    index: &EffectTableIndex,
    stage_one_grace_map: &GraceMap,
    template: &ScrollRecordBytes,
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
    transfer_count: u32,
) -> Result<Rarity4RecordPair, SequenceError> {
    let (stage_one, sequence) = materialize_ng3_rarity4_stage_one_record(
        index,
        stage_one_grace_map,
        template,
        seed,
        level,
        recommended_level,
        generation_serial,
        transfer_count,
    )?;
    let finalizer = crate::r4_finalizer::R4FinalizerEngine::new(index)?;
    Ok(finalizer.build_rarity4_pair(&stage_one, sequence.final_rng_state)?)
}

/// [`materialize_ng3_rarity4_final_record`] for any playthrough.
#[allow(clippy::too_many_arguments)]
pub fn materialize_rarity4_final_record(
    index: &EffectTableIndex,
    stage_one_grace_map: &GraceMap,
    playthrough: u8,
    template: &ScrollRecordBytes,
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
    transfer_count: u32,
) -> Result<Rarity4RecordPair, SequenceError> {
    let (stage_one, sequence) = materialize_rarity4_stage_one_record(
        index,
        stage_one_grace_map,
        playthrough,
        template,
        seed,
        level,
        recommended_level,
        generation_serial,
        transfer_count,
    )?;
    let finalizer = crate::r4_finalizer::R4FinalizerEngine::for_playthrough(index, playthrough)?;
    Ok(finalizer.build_rarity4_pair(&stage_one, sequence.final_rng_state)?)
}

/// Challenge attempt count as the serialized byte at `+0x33`.
fn challenge_attempt_count_byte(seed: u32) -> u8 {
    // `generate_challenge_attempt_count` is bounded by
    // `MIN_CHALLENGE_ATTEMPTS..=MAX_CHALLENGE_ATTEMPTS`, so the cast is exact.
    generate_challenge_attempt_count(seed) as u8
}

/// Identity carried by every ordinary sequence path.
#[derive(Debug, Clone, Copy)]
struct SequenceIdentity {
    record_type: u16,
    rarity: u8,
    playthrough: u8,
    level: u16,
    /// Source index that carries the 0x40 primary flag.
    primary_source_index: u8,
    special_effect_id: Option<u32>,
}

/// Mutable state shared by the three ordinary sequence loops.
struct SequenceBuilder<'a> {
    index: &'a EffectTableIndex,
    rng: LcgStream,
    identity: SequenceIdentity,
    capacities: [u16; CATEGORY_CAPACITY_SLOTS],
    accepted: Vec<u32>,
    promoted: Vec<u8>,
}

impl<'a> SequenceBuilder<'a> {
    fn new(
        index: &'a EffectTableIndex,
        rng: LcgStream,
        identity: SequenceIdentity,
        capacities: [u16; CATEGORY_CAPACITY_SLOTS],
        promoted: Vec<u8>,
    ) -> Self {
        Self {
            index,
            rng,
            identity,
            capacities,
            accepted: Vec::with_capacity(6),
            promoted,
        }
    }

    /// Generate one ordinary slot exactly as the reference loop does.
    fn ordinary_slot(&mut self, source_index: u8, slot: u8) -> Result<ScrollEffect, SequenceError> {
        let effect_flags = if self.promoted.contains(&source_index) {
            EFFECT_FLAG_PROMOTED
        } else {
            0
        };
        let category_and_flags = if source_index == self.identity.primary_source_index {
            EFFECT_FLAG_PRIMARY
        } else {
            0
        };
        let request = CandidatePoolRequest {
            context: NativeWeightContext {
                record_type: self.identity.record_type,
                rarity: self.identity.rarity,
                playthrough: self.identity.playthrough,
                restricted_destination_slot: false,
                extra_selector: 0,
                rarity5_type_floor: 0,
            },
            destination_category_and_flags: category_and_flags,
            destination_effect_flags: effect_flags,
            remaining_category_capacities: self.capacities,
            special_effect_id: self.identity.special_effect_id,
            alternate_runtime_context: false,
        };
        let pool = self
            .index
            .weighted_candidate_pool(&request, &self.accepted)?;
        let selected = self
            .index
            .select_weighted_candidate(&pool, &mut self.rng)?
            .ok_or(SequenceError::EmptyCandidatePool {
                rarity: self.identity.rarity,
            })?;
        let roll_percent =
            self.index
                .roll_effect_percentile(self.identity.rarity, &mut self.rng, false)?;
        let category = category_of(self.index, u32::from(selected.effect_id))?;
        let resolved_value = self.index.resolved_effect_value(
            u32::from(selected.effect_id),
            roll_percent,
            self.identity.level,
        )?;
        let prefix_word = self
            .index
            .effect(selected.effect_id)
            .ok_or(EffectError::UnknownEffect {
                effect_id: u32::from(selected.effect_id),
            })?
            .group_key;
        let effect = ScrollEffect {
            slot,
            source_index,
            effect_id: u32::from(selected.effect_id),
            roll_percent,
            category_and_flags: category_and_flags | category,
            effect_flags,
            candidate_count: pool.len() as u32,
            resolved_value,
            prefix_word,
        };
        self.accepted.push(u32::from(selected.effect_id));
        let capacity = self.capacities[usize::from(category)];
        if capacity == 0 {
            return Err(SequenceError::CategoryCapacityUnderflow { category });
        }
        self.capacities[usize::from(category)] = capacity - 1;
        Ok(effect)
    }

    /// Close the sequence, asserting the claimed draw count against the stream.
    fn finish(
        self,
        seed: u32,
        rarity: u8,
        level: u16,
        effects: Vec<ScrollEffect>,
        random_draws: u32,
        terminal_is_special: bool,
    ) -> Result<ScrollRecord, SequenceError> {
        if self.rng.draws() != u64::from(random_draws) {
            return Err(SequenceError::DrawCountMismatch {
                expected: random_draws,
                actual: self.rng.draws(),
            });
        }
        Ok(ScrollRecord {
            seed,
            record_type: self.identity.record_type,
            rarity,
            playthrough: self.identity.playthrough,
            level,
            effects,
            promoted_source_indexes: self.promoted,
            random_draws,
            final_rng_state: self.rng.state(),
            terminal_is_special,
        })
    }
}

/// Terminal slot builder for the fixed token / Grace slot.
fn terminal_token_effect(
    index: &EffectTableIndex,
    effect_id: u32,
    slot: u8,
    source_index: u8,
    effect_flags: u8,
    resolved_value: i32,
) -> Result<ScrollEffect, SequenceError> {
    Ok(ScrollEffect {
        slot,
        source_index,
        effect_id,
        roll_percent: 0,
        category_and_flags: category_of(index, effect_id)?,
        effect_flags,
        candidate_count: 0,
        resolved_value,
        prefix_word: index.effect_u32(effect_id)?.group_key,
    })
}

/// Category key of an effect's group, as an exact serialized `uint8`.
fn category_of(index: &EffectTableIndex, effect_id: u32) -> Result<u8, SequenceError> {
    let key = index.category_for_effect_u32(effect_id)?.category_key;
    u8::try_from(key).map_err(|_| EffectError::CategoryDoesNotFitByte { category_key: key }.into())
}

/// Rarity-4 stage-one map gate, mirroring `_validate_rarity4_stage_mapping`.
fn validate_rarity4_stage_one_map(
    grace_map: &GraceMap,
    record_type: u16,
) -> Result<(), SequenceError> {
    if grace_map.record_type != u32::from(record_type) {
        return Err(SequenceError::GraceMapContext {
            field: "record_type",
        });
    }
    if grace_map.rarity != RARITY_FINALIZABLE {
        return Err(SequenceError::GraceMapContext { field: "rarity" });
    }
    if grace_map.effect_slot != RARITY4_STAGE_ONE_SLOT {
        return Err(SequenceError::GraceMapContext {
            field: "effect_slot",
        });
    }
    grace_map.validate_partition()?;
    Ok(())
}

/// Rarity-5 map gate, mirroring `_validate_grace_mapping`.
fn validate_grace_map(grace_map: &GraceMap, playthrough: u8) -> Result<(), SequenceError> {
    let expected_type = CATEGORY_TO_TYPE[usize::from(playthrough)];
    if grace_map.record_type != u32::from(expected_type) {
        return Err(SequenceError::GraceMapContext {
            field: "record_type",
        });
    }
    if grace_map.rarity != RARITY_DIVINE {
        return Err(SequenceError::GraceMapContext { field: "rarity" });
    }
    if grace_map.effect_slot != RARITY5_GRACE_SLOT {
        return Err(SequenceError::GraceMapContext {
            field: "effect_slot",
        });
    }
    grace_map.validate_partition()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_count_matches_native_vectors() {
        assert_eq!(generate_challenge_attempt_count(1), 4);
        assert_eq!(generate_challenge_attempt_count(0x0FFF_FFFF), 7);
        assert_eq!(generate_challenge_attempt_count(82_212_268), 5);
        assert_eq!(generate_challenge_attempt_count(183_696_634), 6);
    }

    #[test]
    fn challenge_seed_rotation_is_bounded() {
        assert_eq!(derive_challenge_count_seed(0), 0);
        assert_eq!(derive_challenge_count_seed(1), 1 << 7);
        // (0xFFFFFFFF & 0x001FC07F) << 7 | (0xFFFFFFFF >> 7) & 0x001FC07F
        assert_eq!(derive_challenge_count_seed(0xFFFF_FFFF), 0x0FFF_FFFF);
        assert_eq!(derive_challenge_count_seed(7), 7 << 7);
    }

    #[test]
    fn random_int_is_zero_bounded() {
        let mut rng = LcgStream::new(0x2FAC_1E69);
        assert_eq!(random_int(&mut rng, 0), None);
        assert_eq!(random_int(&mut rng, 1), Some(0));
    }
}

#[cfg(test)]
mod sequence_tests {
    use super::*;
    use crate::effect::test_support::{
        context_less_resource, dense_map, promoting_index, synthetic_index,
    };

    fn stage_one_map() -> GraceMap {
        dense_map(
            u32::from(NG3_RECORD_TYPE),
            RARITY_FINALIZABLE,
            RARITY4_STAGE_ONE_SLOT,
        )
    }

    fn grace_map_for(record_type: u16) -> GraceMap {
        dense_map(u32::from(record_type), RARITY_DIVINE, RARITY5_GRACE_SLOT)
    }

    #[test]
    fn rarity3_reports_the_exact_token_and_draw_count() {
        let index = synthetic_index();
        let record =
            generate_ng3_rarity3_effect_sequence(&index, 7, 180).expect("rarity 3 generates");
        assert_eq!(record.effects.len(), 5);
        assert_eq!(record.record_type, NG3_RECORD_TYPE);
        assert_eq!(record.rarity, RARITY_GROWING);
        assert_eq!(record.playthrough, NG3_PLAYTHROUGH);
        assert_eq!(record.promoted_source_indexes, Vec::<u8>::new());
        assert_eq!(record.random_draws, 13);
        assert!(record.terminal_is_special);
        let token = record.terminal().expect("terminal token");
        assert_eq!(
            (token.slot, token.source_index, token.effect_id),
            (5, 4, RARITY3_GROWING_TOKEN)
        );
        assert_eq!(token.effect_flags, RARITY3_TOKEN_EFFECT_FLAGS);
        assert_eq!(token.resolved_value, 0);
        assert_eq!(record.secondaries().len(), 3);
        // The claimed draw count is re-derived through the affine jump rather
        // than trusting the internal counter.
        assert_eq!(
            record.final_rng_state,
            crate::rng::state_after(7, u64::from(record.random_draws))
        );
    }

    #[test]
    fn rarity4_stage_one_and_rarity5_keep_their_terminal_slots() {
        let index = synthetic_index();
        let stage_one =
            generate_ng3_rarity4_stage_one_effect_sequence(&index, &stage_one_map(), 11, 180)
                .expect("stage one generates");
        assert_eq!(stage_one.rarity, RARITY_FINALIZABLE);
        assert_eq!(stage_one.effects.len(), 5);
        assert_eq!(stage_one.effects[4].slot, RARITY4_STAGE_ONE_SLOT);
        assert_eq!(stage_one.random_draws, 14);

        let record =
            generate_ng3_rarity5_effect_sequence(&index, &grace_map_for(NG3_RECORD_TYPE), 11, 180)
                .expect("rarity 5 generates");
        assert_eq!(record.rarity, RARITY_DIVINE);
        assert_eq!(record.effects.len(), 6);
        assert_eq!(record.effects[5].slot, RARITY5_GRACE_SLOT);
        assert_eq!(record.effects[5].effect_flags, SOURCE_GRACE_EFFECT_FLAGS);
        assert_eq!(record.random_draws, 17);
        assert_eq!(record.secondaries().len(), 4);
        assert_eq!(
            record.final_rng_state,
            crate::rng::state_after(11, u64::from(record.random_draws))
        );
    }

    #[test]
    fn promotion_trials_consume_the_shuffle_draws() {
        let index = promoting_index();
        let rarity3 =
            generate_ng3_rarity3_effect_sequence(&index, 3, 180).expect("rarity 3 generates");
        assert!(!rarity3.promoted_source_indexes.is_empty());
        assert_eq!(rarity3.random_draws, 20);
        assert!(rarity3
            .promoted_source_indexes
            .iter()
            .all(|slot| *slot <= 3));
        assert!(rarity3
            .effects
            .iter()
            .any(|effect| effect.effect_flags == EFFECT_FLAG_PROMOTED));

        let stage_one =
            generate_ng3_rarity4_stage_one_effect_sequence(&index, &stage_one_map(), 3, 180)
                .expect("stage one generates");
        assert_eq!(stage_one.random_draws, 21);
        assert!(stage_one
            .promoted_source_indexes
            .iter()
            .all(|slot| (1..=4).contains(slot)));

        let rarity5 =
            generate_ng3_rarity5_effect_sequence(&index, &grace_map_for(NG3_RECORD_TYPE), 3, 180)
                .expect("rarity 5 generates");
        assert_eq!(rarity5.random_draws, 24);
    }

    #[test]
    fn grace_maps_are_gated_on_context_and_partition() {
        let index = synthetic_index();
        assert_eq!(
            generate_ng3_rarity4_stage_one_effect_sequence(
                &index,
                &grace_map_for(NG3_RECORD_TYPE),
                1,
                180,
            )
            .unwrap_err(),
            SequenceError::GraceMapContext { field: "rarity" }
        );

        let mut gapped = stage_one_map();
        gapped.ranges[1].start = 0x8001;
        assert!(matches!(
            generate_ng3_rarity4_stage_one_effect_sequence(&index, &gapped, 1, 180).unwrap_err(),
            SequenceError::Table(EffectError::GraceMapNotDense { .. })
        ));

        let mut wrong_slot = stage_one_map();
        wrong_slot.effect_slot = RARITY5_GRACE_SLOT;
        assert_eq!(
            generate_ng3_rarity4_stage_one_effect_sequence(&index, &wrong_slot, 1, 180)
                .unwrap_err(),
            SequenceError::GraceMapContext {
                field: "effect_slot"
            }
        );
    }

    #[test]
    fn ng4_and_ng5_require_a_matching_captured_map() {
        let index = synthetic_index();
        assert_eq!(
            generate_rarity5_grace_effect_sequence(
                &index,
                &grace_map_for(NG3_RECORD_TYPE),
                4,
                1,
                180
            )
            .unwrap_err(),
            SequenceError::GraceMapContext {
                field: "record_type"
            }
        );
        // A capture for the NG4 record type is accepted and keeps its identity.
        let ng4 = generate_rarity5_grace_effect_sequence(
            &index,
            &grace_map_for(CATEGORY_TO_TYPE[4]),
            4,
            1,
            180,
        )
        .expect("NG4 generates once a matching map exists");
        assert_eq!(ng4.record_type, CATEGORY_TO_TYPE[4]);
        assert_eq!(ng4.playthrough, 4);
        assert_eq!(ng4.random_draws, 17);
    }

    #[test]
    fn the_embedded_ng12_rarity4_map_is_the_live_capture() {
        // `map-ng1-r4.json` / `map-ng2-r4.json` of the live capture, in decimal.
        let captured: [(u16, u16, u32); 10] = [
            (0, 6559, 9189),
            (6560, 13112, 10982),
            (13113, 19665, 36044),
            (19666, 26218, 45647),
            (26219, 32771, 20498),
            (32772, 39324, 31722),
            (39325, 45877, 22796),
            (45878, 52430, 20387),
            (52431, 58983, 45545),
            (58984, 65535, 59627),
        ];
        for playthrough in [1u8, 2] {
            let map = ng12_rarity4_stage_one_map(playthrough).unwrap();
            map.validate_partition().unwrap();
            assert_eq!(
                map.record_type,
                u32::from(CATEGORY_TO_TYPE[usize::from(playthrough)])
            );
            let ranges: Vec<(u16, u16, u32)> = map
                .ranges
                .iter()
                .map(|range| (range.start, range.end, range.effect_id))
                .collect();
            assert_eq!(ranges, captured);
        }
        assert!(ng12_rarity4_stage_one_map(3).is_err());
    }

    #[test]
    fn ng1_and_ng2_rarity5_draw_six_ordinary_effects_and_no_grace() {
        // Live PC v2.02: native NG1/NG2 rarity-5 records carry six ordinary
        // effects, the primary in the first slot, and an empty seventh slot.
        let index = synthetic_index();
        for playthrough in [1u8, 2] {
            let record = generate_rarity5_plain_effect_sequence(&index, playthrough, 1, 180)
                .expect("the plain rarity-5 path generates");
            assert_eq!(
                record.record_type,
                CATEGORY_TO_TYPE[usize::from(playthrough)]
            );
            assert_eq!(record.playthrough, playthrough);
            assert_eq!(record.effects.len(), 6);
            assert!(!record.terminal_is_special);
            assert_eq!(
                record.effects[0].category_and_flags & EFFECT_FLAG_PRIMARY,
                EFFECT_FLAG_PRIMARY
            );
            let promotion = if record.promoted_source_indexes.is_empty() {
                0
            } else {
                7
            };
            assert_eq!(record.random_draws, 1 + promotion + 6 * 3);
        }
    }

    #[test]
    fn unsupported_playthroughs_are_rejected_before_table_access() {
        let index = synthetic_index();
        let map = grace_map_for(NG3_RECORD_TYPE);
        assert_eq!(
            generate_rarity5_grace_effect_sequence(&index, &map, 0, 1, 180).unwrap_err(),
            SequenceError::UnsupportedPlaythrough(0)
        );
        // NG2 is a known context; the NG3 map is refused as another context's capture.
        assert_eq!(
            generate_rarity5_grace_effect_sequence(&index, &map, 2, 1, 180).unwrap_err(),
            SequenceError::GraceMapContext {
                field: "record_type"
            }
        );
        assert_eq!(
            generate_rarity5_grace_effect_sequence(&index, &map, 6, 1, 180).unwrap_err(),
            SequenceError::UnsupportedPlaythrough(6)
        );
    }

    #[test]
    fn an_empty_native_pool_is_reported_instead_of_guessed() {
        let resource = context_less_resource();
        let index = EffectTableIndex::from_resource(&resource).expect("synthetic resource indexes");
        assert_eq!(
            generate_ng3_rarity3_effect_sequence(&index, 1, 180).unwrap_err(),
            SequenceError::EmptyCandidatePool {
                rarity: RARITY_GROWING
            }
        );
    }
}
