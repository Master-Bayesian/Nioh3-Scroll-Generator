//! Typed effect-generation tables and row decoding.
//!
//! The crate stays standard-library-only and free of filesystem access: a data
//! loader hands over header-stripped table bytes, and this module owns field
//! decoding, index construction and the validation the reference performs.
//! Field offsets mirror `nioh3_scroll_editor/effect_generation_tables.py` and
//! `nioh3_scroll_editor/r4_finalizer_reference.py` at the M2.1 baseline.

use std::collections::BTreeMap;

use crate::rng::LcgStream;

/// Row sizes the M2.1 slice depends on, as confirmed against the shipped
/// manifest. A loader supplies the size; these are asserted, never assumed.
pub const ITEM_ROW_BYTES: usize = 0x1A0;
pub const EFFECT_ROW_BYTES: usize = 0xD8;
pub const EFFECT_GROUP_ROW_BYTES: usize = 0x70;
pub const CATEGORY_ROW_BYTES: usize = 0x6C;
pub const CATEGORY_COUNT_MULTIPLIER_ROW_BYTES: usize = 0x20;
pub const OPTIONAL_MULTIPLIER_ROW_BYTES: usize = 0x20;
pub const RARITY_ROLL_ROW_BYTES: usize = 248;
pub const LEVEL_CURVE_ROW_BYTES: usize = 10;
/// Row size of the shipped `special_context` table consumed by the recovered
/// auxiliary-mode and R4 finalizer weight-slot paths.
pub const SPECIAL_CONTEXT_ROW_BYTES: usize = 48;
/// Rows in the rarity-roll table; one per rarity index 0..=5.
pub const RARITY_ROLL_ROW_COUNT: usize = 6;
/// Width of the native per-slot category capacity vector built at RVA 0x91B6E8.
pub const CATEGORY_CAPACITY_SLOTS: usize = 32;
/// Highest level the resolved-value curve reads before clamping.
pub const MAX_CURVE_LEVEL: u16 = 500;
/// Effect flag that marks a promoted-slot candidate.
pub const EFFECT_FLAG_PROMOTED: u8 = 0x04;
/// Effect/category flag that marks physical slot 1.
pub const EFFECT_FLAG_PRIMARY: u8 = 0x40;
/// Destination flag the recovered pool builder does not support yet.
pub const UNSUPPORTED_DESTINATION_FLAG: u8 = 0x40;

/// Scroll record types that carry generated effects, in reference order.
pub const SCROLL_RECORD_TYPES: [u16; 5] = [0x1E82, 0x516D, 0xE604, 0xDD82, 0xD523];
/// Scroll item mode used by the shipped item rows.
pub const SCROLL_ITEM_MODE: u8 = 0x12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectError {
    /// A table's payload is not a whole number of declared rows.
    RowSizeMismatch {
        table: &'static str,
        row_size: usize,
        bytes: usize,
    },
    /// A stride differs from the value this slice depends on.
    UnexpectedStride {
        table: &'static str,
        expected: usize,
        actual: usize,
    },
    /// A row was too short for an accessed field.
    ShortRow {
        table: &'static str,
        need: usize,
        have: usize,
    },
    /// The reference rejects duplicate index keys instead of overwriting them.
    DuplicateKey { table: &'static str, key: u32 },
    /// An effect references a group that does not exist.
    UnknownGroup { group_key: u16 },
    /// A required scroll item row is absent.
    MissingScrollItem { record_type: u16 },
    /// A Grace map is not a dense inclusive partition of the u16 domain.
    GraceMapNotDense {
        index: usize,
        start: u16,
        expected_start: u16,
    },
    /// A Grace map's provenance metadata is not the verified shipped context.
    GraceMapMetadata { field: &'static str },
    /// A rarity outside the recovered 0..=5 range was requested.
    UnsupportedRarity { rarity: u8 },
    /// The requested effect ID is absent from the effect table, or is not a
    /// `uint16` row key at all.
    UnknownEffect { effect_id: u32 },
    /// An effect row refers to a group whose category is absent from the table.
    UnknownCategory { category_key: u16 },
    /// A record type is absent from the scroll item rows.
    UnknownRecordType { record_type: u16 },
    /// A capacity vector slot is outside the native 32-entry vector.
    CategoryOutsideNativeVector { category_key: u16 },
    /// The remaining-capacity vector does not fit the native `uint8` contract.
    CapacityNotByte { category_key: u16, value: u16 },
    /// The rarity-roll table does not carry exactly six rows.
    RarityRollRowCount { rows: usize },
    /// A level-curve selector outside 0..=2 was requested.
    CurveSelector { selector: u16 },
    /// The playthrough selector is outside the shipped 1..=5 range.
    PlaythroughSelector { selector: u8 },
    /// A slot flag vector was not exactly seven `uint8` entries.
    SlotFlagVector { len: usize },
    /// A slot limit outside 0..=7 was requested.
    SlotLimit { slot_limit: u8 },
    /// A table row was not the mode-0x12 scroll item this slice supports.
    UnexpectedMode {
        table: &'static str,
        record_type: u16,
        mode: u8,
    },
    /// A zero draw bound reached the native integer-draw helper.
    ZeroDrawBound,
    /// The native `total + 1` lottery bound wrapped to zero.
    LotteryWrapped,
    /// The recovered pool builder does not support destination effect flag 0x40.
    UnsupportedDestinationFlag,
    /// A type-class-5 weight needed an optional multiplier key the table lacks.
    MissingOptionalMultiplier { lookup_key: u32 },
    /// A category-count multiplier key is absent from the shipped table.
    MissingCategoryCountMultiplier { lookup_key: u32 },
    /// An auxiliary mode resolved to more than one `special_context` row.
    AmbiguousSpecialContext { mode: u8 },
    /// A binary32 intermediate was not finite, so the reference's `int()` fails.
    NonFiniteIntermediate { stage: &'static str },
    /// A binary32 intermediate was outside the truncation contract.
    OutOfRangeIntermediate { stage: &'static str },
    /// A category key cannot be represented in the serialized `uint8` field.
    CategoryDoesNotFitByte { category_key: u16 },
}

/// Header-stripped, fixed-stride table payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectTableBytes {
    pub row_size: usize,
    pub rows: Vec<u8>,
}

impl EffectTableBytes {
    pub fn new(table: &'static str, row_size: usize, rows: Vec<u8>) -> Result<Self, EffectError> {
        if row_size == 0 || !rows.len().is_multiple_of(row_size) {
            return Err(EffectError::RowSizeMismatch {
                table,
                row_size,
                bytes: rows.len(),
            });
        }
        Ok(Self { row_size, rows })
    }

    pub fn row_count(&self) -> usize {
        self.rows.len() / self.row_size
    }

    pub fn row(&self, index: usize) -> Option<&[u8]> {
        let start = index.checked_mul(self.row_size)?;
        self.rows.get(start..start + self.row_size)
    }
}

/// One inclusive first-u16 range of a measured Grace output map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraceRange {
    pub start: u16,
    pub end: u16,
    pub effect_id: u32,
}

/// Capture-state label carried by both shipped Grace maps.
///
/// The label is provenance from the measured file, not a progression number.
/// It is preserved verbatim; the requested progression belongs to the request
/// side and is validated there.
pub const GRACE_MAP_CAPTURE_STATE: &str = "current-loaded-state";

/// Typed Grace output map context, as shipped for rarity 4 and 5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraceMap {
    pub format: String,
    pub game_version: String,
    pub record_type: u32,
    pub rarity: u8,
    /// Exact `context.playthrough` label from the shipped JSON.
    pub capture_state: String,
    pub effect_slot: u8,
    /// Dense inclusive partition of `0..=0xFFFF`, ascending and non-overlapping.
    pub ranges: Vec<GraceRange>,
}

impl GraceMap {
    /// Reject a map whose metadata or ranges break the shipped contract.
    ///
    /// The range walk rejects holes, overlaps, duplicate buckets, inverted
    /// ranges and any partition that does not end at `0xFFFF`.
    pub fn validate(&self) -> Result<(), EffectError> {
        if self.format != "nioh3-grace-first-u16-map-v2" {
            return Err(EffectError::GraceMapMetadata { field: "format" });
        }
        if self.game_version.is_empty() {
            return Err(EffectError::GraceMapMetadata {
                field: "game_version",
            });
        }
        if self.capture_state != GRACE_MAP_CAPTURE_STATE {
            return Err(EffectError::GraceMapMetadata {
                field: "capture_state",
            });
        }
        if self.record_type != 0xE604 {
            return Err(EffectError::GraceMapMetadata {
                field: "record_type",
            });
        }
        let expected_slot = match self.rarity {
            4 => 5,
            5 => 6,
            _ => return Err(EffectError::GraceMapMetadata { field: "rarity" }),
        };
        if self.effect_slot != expected_slot {
            return Err(EffectError::GraceMapMetadata {
                field: "effect_slot",
            });
        }
        self.validate_partition()
    }

    /// Dense-partition rule only, for maps whose metadata belongs to another
    /// context (for example a future NG4/NG5 capture).
    pub fn validate_partition(&self) -> Result<(), EffectError> {
        if self.ranges.is_empty() {
            return Err(EffectError::GraceMapNotDense {
                index: 0,
                start: 0,
                expected_start: 0,
            });
        }
        let mut next = 0u32;
        for (index, range) in self.ranges.iter().enumerate() {
            if u32::from(range.start) != next || range.end < range.start {
                return Err(EffectError::GraceMapNotDense {
                    index,
                    start: range.start,
                    expected_start: next as u16,
                });
            }
            next = u32::from(range.end) + 1;
        }
        if next != 0x1_0000 {
            return Err(EffectError::GraceMapNotDense {
                index: self.ranges.len(),
                start: 0,
                expected_start: next as u16,
            });
        }
        Ok(())
    }

    /// Exact reference lookup: first `u16` maps through the measured ranges.
    pub fn grace_id_for_first_u16(&self, first_u16: u16) -> Option<u32> {
        self.ranges
            .binary_search_by(|range| {
                if first_u16 < range.start {
                    std::cmp::Ordering::Greater
                } else if first_u16 > range.end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
            .map(|index| self.ranges[index].effect_id)
    }
}

/// Everything the effect sequence needs from the shipped R4 resource bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectResourceBytes {
    pub schema: String,
    pub item: EffectTableBytes,
    pub effect_group: EffectTableBytes,
    pub category: EffectTableBytes,
    pub category_count_multiplier: EffectTableBytes,
    pub effect: EffectTableBytes,
    pub level_curve: EffectTableBytes,
    pub optional_multiplier: EffectTableBytes,
    pub rarity_roll: EffectTableBytes,
    pub special_context: EffectTableBytes,
    /// 0x58-byte bonus-curve rows without a header.
    pub bonus_curve_rows: Vec<u8>,
    /// u32 bonus-curve index entries.
    pub bonus_curve_index: Vec<u8>,
    /// i32 playthrough progress entries.
    pub playthrough_progress: Vec<u8>,
    /// Rarity 4 and rarity 5 measured Grace maps.
    pub grace_maps: Vec<GraceMap>,
}

fn u16_at(row: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([row[offset], row[offset + 1]])
}

fn u32_at(row: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        row[offset],
        row[offset + 1],
        row[offset + 2],
        row[offset + 3],
    ])
}

fn i16_at(row: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([row[offset], row[offset + 1]])
}

fn i32_at(row: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        row[offset],
        row[offset + 1],
        row[offset + 2],
        row[offset + 3],
    ])
}

fn f32_at(row: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        row[offset],
        row[offset + 1],
        row[offset + 2],
        row[offset + 3],
    ])
}

// Binary32 steps mirror `r4_finalizer_reference.f32_mul` and friends exactly:
// each operands is already binary32, so widening to binary64, applying the
// operation and rounding back reproduces the reference's intermediate value,
// including its double-rounding behaviour.
pub(crate) fn f32_mul(left: f32, right: f32) -> f32 {
    (f64::from(left) * f64::from(right)) as f32
}

pub(crate) fn f32_add(left: f32, right: f32) -> f32 {
    (f64::from(left) + f64::from(right)) as f32
}

pub(crate) fn f32_sub(left: f32, right: f32) -> f32 {
    (f64::from(left) - f64::from(right)) as f32
}

pub(crate) fn f32_div(left: f32, right: f32) -> f32 {
    (f64::from(left) / f64::from(right)) as f32
}

/// Python `int(value)` for a value the native path only ever produces in range.
///
/// The reference truncates toward zero and raises on non-finite input; the
/// integer result is not bounded, so anything outside the `i64` contract is
/// rejected instead of silently saturating.
pub(crate) fn trunc_f32(value: f32, stage: &'static str) -> Result<i64, EffectError> {
    if !value.is_finite() {
        return Err(EffectError::NonFiniteIntermediate { stage });
    }
    let widened = f64::from(value);
    if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&widened) {
        return Err(EffectError::OutOfRangeIntermediate { stage });
    }
    Ok(widened.trunc() as i64)
}

/// Progress bucket selected by the effect row's `+0x54` gate (RVA 0x578C18).
fn progress_bucket(threshold: u16) -> usize {
    if threshold < 7000 {
        0
    } else if threshold < 8000 {
        1
    } else if threshold < 9000 {
        2
    } else {
        3
    }
}

/// The recovered rarity-roll percentile lottery at RVA 0x110A275..0x110A314.
fn roll_percentile(minimum: u32, maximum: u32, rng: &mut LcgStream) -> Result<u8, EffectError> {
    if minimum >= maximum {
        return Ok((minimum & 0xFF) as u8);
    }
    let first = draw_int(rng, 46)?;
    let second = draw_int(rng, 46)?;
    let lottery = first + second + if first == second { 10 } else { 0 };
    let span = maximum.wrapping_sub(minimum);
    let scaled = f32_div(f32_mul(lottery as f32, span as f32), f32_of(100.0));
    let result = trunc_f32(f32_add(minimum as f32, scaled), "roll_percentile")?;
    Ok((result & 0xFF) as u8)
}

/// The inclusive integer draw at RVA 0x56C6A8, shared with `sequence::random_int`.
fn draw_int(rng: &mut LcgStream, count: u32) -> Result<u32, EffectError> {
    crate::sequence::random_int(rng, count).ok_or(EffectError::ZeroDrawBound)
}

/// Truncating binary32-to-`int32` conversion for the recovered value formula.
fn trunc_f32_i32(value: f32, stage: &'static str) -> Result<i32, EffectError> {
    let truncated = trunc_f32(value, stage)?;
    i32::try_from(truncated).map_err(|_| EffectError::OutOfRangeIntermediate { stage })
}

fn f32_of(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
fn need(table: &'static str, row: &[u8], offset: usize, width: usize) -> Result<(), EffectError> {
    if row.len() < offset + width {
        return Err(EffectError::ShortRow {
            table,
            need: offset + width,
            have: row.len(),
        });
    }
    Ok(())
}

/// One 0xD8-byte effect row.
#[derive(Debug, Clone, Copy)]
pub struct EffectRow<'a> {
    raw: &'a [u8],
}

impl<'a> EffectRow<'a> {
    pub fn new(raw: &'a [u8]) -> Result<Self, EffectError> {
        if raw.len() != EFFECT_ROW_BYTES {
            return Err(EffectError::UnexpectedStride {
                table: "effect",
                expected: EFFECT_ROW_BYTES,
                actual: raw.len(),
            });
        }
        Ok(Self { raw })
    }

    pub fn effect_id(&self) -> u16 {
        u16_at(self.raw, 0x00)
    }

    pub fn group_key(&self) -> u16 {
        u16_at(self.raw, 0x02)
    }

    pub fn flags(&self) -> u32 {
        u32_at(self.raw, 0x1C)
    }

    pub fn normalization_flags(&self) -> u32 {
        u32_at(self.raw, 0x20)
    }

    pub fn progress_threshold(&self) -> u16 {
        u16_at(self.raw, 0x54)
    }

    pub fn alternate_threshold(&self) -> u16 {
        u16_at(self.raw, 0x56)
    }

    pub fn lottery_weights(&self) -> [u16; 64] {
        let mut weights = [0u16; 64];
        for (index, weight) in weights.iter_mut().enumerate() {
            *weight = u16_at(self.raw, 0x58 + index * 2);
        }
        weights
    }

    /// Localized rarity weights, matching `EffectRow.rarity_weight`.
    pub fn rarity_weight(&self, rarity: u8) -> f32 {
        const OFFSETS: [usize; 6] = [0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C];
        if rarity == 0xFF {
            return (0x28..=0x34)
                .step_by(4)
                .map(|offset| f32_at(self.raw, offset))
                .fold(f32::MIN, f32::max);
        }
        match OFFSETS.get(rarity as usize) {
            Some(offset) => f32_at(self.raw, *offset),
            None => 1.0,
        }
    }

    /// Type-class multiplier, matching `EffectRow.type_multiplier`.
    pub fn type_multiplier(&self, type_class: u8) -> f32 {
        match type_class {
            3 => f32_at(self.raw, 0x48),
            4 => f32_at(self.raw, 0x4C),
            5 => f32_at(self.raw, 0x50),
            _ => f32_at(self.raw, 0x44),
        }
    }

    pub fn slot_weight(&self, weight_slot: usize) -> u16 {
        if weight_slot >= 64 {
            return 0;
        }
        u16_at(self.raw, 0x58 + weight_slot * 2)
    }
}

/// One 0x70-byte effect-group row.
#[derive(Debug, Clone, Copy)]
pub struct GroupRow<'a> {
    raw: &'a [u8],
}

impl<'a> GroupRow<'a> {
    pub fn new(raw: &'a [u8]) -> Result<Self, EffectError> {
        if raw.len() != EFFECT_GROUP_ROW_BYTES {
            return Err(EffectError::UnexpectedStride {
                table: "effect_group",
                expected: EFFECT_GROUP_ROW_BYTES,
                actual: raw.len(),
            });
        }
        Ok(Self { raw })
    }

    pub fn group_key(&self) -> u16 {
        u16_at(self.raw, 0x0C)
    }

    pub fn category_key(&self) -> u16 {
        u16_at(self.raw, 0x24)
    }

    pub fn conflict_mask_0(&self) -> u32 {
        u32_at(self.raw, 0x54)
    }

    pub fn conflict_mask_1(&self) -> u32 {
        u32_at(self.raw, 0x58)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollItemDefinition {
    pub record_type: u16,
    pub field_154: u32,
    pub field_15c: u32,
    pub mode: u8,
    pub candidate_item_flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectGroupDefinition {
    pub group_key: u16,
    pub category_key: u16,
    pub conflict_mask_0: u32,
    pub conflict_mask_1: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryDefinition {
    /// Row index inside the category table. The mode-0x12 category lottery is
    /// order-sensitive, so the reference iterates rows, not sorted keys.
    pub row_index: usize,
    pub category_key: u16,
    pub rarity_capacities: [u16; 6],
    pub mode12_lottery_weight: u16,
    pub mode12_capacity: u16,
    pub mode12_count_multiplier_key: u16,
}

/// One 48-byte `special_context` row used by the auxiliary and weight-slot
/// paths.
///
/// `mode` is the recovered descriptor byte `+0x1E` (`row +0x28`); the
/// remaining fields are the ones the recovered code reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecialContextDefinition {
    /// Auxiliary mode byte at `+0x28`.
    pub mode: u8,
    /// Auxiliary branch class at `+0x29`.
    pub branch_class: u8,
    /// Bit 0 of `+0x2F`: the finalizer moves to weight slots `0x3D`/`0x3E`.
    pub reveal_weight_flag: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CategoryCountMultiplierDefinition {
    pub lookup_key: u32,
    pub multipliers: [f32; 7],
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectDefinition {
    pub effect_id: u16,
    pub group_key: u16,
    pub flags: u32,
    pub normalization_flags: u32,
    pub progress_threshold: u16,
    pub alternate_threshold: u16,
    pub lottery_weights: [u16; 64],
    /// Base value at `+0x08`.
    pub base_value: i16,
    /// Roll interpolation bounds at `+0x0A` and `+0x0C`.
    pub roll_low: i16,
    pub roll_high: i16,
    /// Localized rarity weights at `+0x28..+0x3C` (index 0 is rarity 0).
    pub rarity_weights: [f32; 6],
    /// Type multipliers at `+0x44`, `+0x48`, `+0x4C`, `+0x50`.
    pub type_multipliers: [f32; 4],
    /// Level-curve selector at `+0x06`.
    pub curve_selector: u16,
    /// Row index inside the effect table; row 0 is the unused sentinel row.
    pub row_index: usize,
}

impl EffectDefinition {
    /// Slot weight at `+0x58 + 2 * weight_slot`, mirroring `EffectRow.slot_weight`.
    pub fn slot_weight(&self, weight_slot: usize) -> u16 {
        self.lottery_weights.get(weight_slot).copied().unwrap_or(0)
    }

    /// Localized rarity weight, mirroring `EffectRow.rarity_weight`.
    ///
    /// `0xFF` selects the max-like path used by the alternate native branch.
    pub fn rarity_weight(&self, rarity: u8) -> f32 {
        if rarity == 0xFF {
            return self.rarity_weights[..4]
                .iter()
                .copied()
                .fold(f32::MIN, f32::max);
        }
        self.rarity_weights
            .get(usize::from(rarity))
            .copied()
            .unwrap_or(1.0)
    }

    /// Type-class multiplier, mirroring `EffectRow.type_multiplier`.
    pub fn type_multiplier(&self, type_class: u8) -> f32 {
        match type_class {
            3 => self.type_multipliers[1],
            4 => self.type_multipliers[2],
            5 => self.type_multipliers[3],
            _ => self.type_multipliers[0],
        }
    }
}

/// One optional-multiplier row (`lookup_key` at `+0x14`, value at `+0x18`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptionalMultiplierDefinition {
    pub lookup_key: u32,
    /// Signed base at `+0x10`; the auxiliary lottery multiplies it by
    /// [`Self::multiplier`] to build its integer threshold.
    pub base_value: i32,
    pub multiplier: f32,
}

/// One rarity-roll row, indexed by rarity 0..=5.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RarityGenerationDefinition {
    pub rarity: u8,
    pub minimum_roll_percent: u32,
    pub maximum_roll_percent: u32,
    pub base_slot_count: u32,
    pub total_slot_count: u32,
    pub promotion_trials: u32,
    pub promotion_probability_percent: f32,
}

/// One weighted pool entry, mirroring `WeightedEffectCandidate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightedEffectCandidate {
    pub effect_id: u16,
    pub weight: i64,
}

/// Table context for [`EffectTableIndex::native_effect_weight`] (RVA 0x57896C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NativeWeightContext {
    pub record_type: u16,
    pub rarity: u8,
    pub playthrough: u8,
    /// Native callers substitute weight slot `0x29` for restricted slots.
    pub restricted_destination_slot: bool,
    pub extra_selector: u8,
    pub rarity5_type_floor: u8,
}

/// Inputs for [`EffectTableIndex::weighted_candidate_pool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePoolRequest {
    pub context: NativeWeightContext,
    pub destination_category_and_flags: u8,
    pub destination_effect_flags: u8,
    pub remaining_category_capacities: [u16; CATEGORY_CAPACITY_SLOTS],
    pub special_effect_id: Option<u32>,
    pub alternate_runtime_context: bool,
}

/// Inputs for [`EffectTableIndex::select_promoted_slot_indexes`].
#[derive(Debug, Clone, Copy)]
pub struct PromotedSlotRequest<'a> {
    pub record_type: u16,
    pub rarity: u8,
    pub category_and_flags: &'a [u8; 7],
    pub effect_flags: &'a [u8; 7],
    /// `None` uses the rarity row's `total_slot_count`.
    pub slot_limit: Option<u8>,
    pub rarity5_type_floor: u8,
}

/// Decoded, duplicate-checked indexes over the shipped tables.
#[derive(Debug, Clone)]
pub struct EffectTableIndex {
    pub items_by_record_type: BTreeMap<u16, ScrollItemDefinition>,
    pub groups_by_key: BTreeMap<u16, EffectGroupDefinition>,
    pub categories_by_key: BTreeMap<u16, CategoryDefinition>,
    pub count_multipliers_by_key: BTreeMap<u32, CategoryCountMultiplierDefinition>,
    pub effects_by_id: BTreeMap<u16, EffectDefinition>,
    /// Effect rows in table order, mirroring the reference's insertion-ordered
    /// dict. Weighted lotteries are order-sensitive, so this vector, not the
    /// sorted map, feeds [`Self::weighted_candidate_pool`].
    pub effects_in_row_order: Vec<EffectDefinition>,
    pub optional_multipliers_by_key: BTreeMap<u32, OptionalMultiplierDefinition>,
    /// Rarity rows 0..=5 from the rarity-roll table.
    pub rarity_generation: Vec<RarityGenerationDefinition>,
    /// `special_context` rows in table order, as the auxiliary and finalizer
    /// weight-slot paths consume them.
    pub special_context: Vec<SpecialContextDefinition>,
    /// Playthrough selectors 1..=5 as four `u32` progress values each; the
    /// vector index is `selector - 1`.
    pub playthrough_progress: Vec<[u32; 4]>,
    /// Level-curve rows: three `u16` selectors per curve level.
    pub level_curve: Vec<[u16; 3]>,
}

impl EffectTableIndex {
    pub fn from_resource(resource: &EffectResourceBytes) -> Result<Self, EffectError> {
        expect_stride("item", resource.item.row_size, ITEM_ROW_BYTES)?;
        expect_stride("effect", resource.effect.row_size, EFFECT_ROW_BYTES)?;
        expect_stride(
            "effect_group",
            resource.effect_group.row_size,
            EFFECT_GROUP_ROW_BYTES,
        )?;
        expect_stride("category", resource.category.row_size, CATEGORY_ROW_BYTES)?;
        expect_stride(
            "category_count_multiplier",
            resource.category_count_multiplier.row_size,
            CATEGORY_COUNT_MULTIPLIER_ROW_BYTES,
        )?;
        expect_stride(
            "optional_multiplier",
            resource.optional_multiplier.row_size,
            OPTIONAL_MULTIPLIER_ROW_BYTES,
        )?;
        expect_stride(
            "rarity_roll",
            resource.rarity_roll.row_size,
            RARITY_ROLL_ROW_BYTES,
        )?;
        expect_stride(
            "level_curve",
            resource.level_curve.row_size,
            LEVEL_CURVE_ROW_BYTES,
        )?;
        expect_stride(
            "special_context",
            resource.special_context.row_size,
            SPECIAL_CONTEXT_ROW_BYTES,
        )?;

        let mut items = BTreeMap::new();
        for index in 0..resource.item.row_count() {
            let row = resource.item.row(index).expect("row index within count");
            let record_type = u16_at(row, 0x152);
            if !SCROLL_RECORD_TYPES.contains(&record_type) {
                continue;
            }
            insert_unique(
                &mut items,
                record_type,
                ScrollItemDefinition {
                    record_type,
                    field_154: u32_at(row, 0x154),
                    field_15c: u32_at(row, 0x15C),
                    mode: row[0x182],
                    candidate_item_flags: u32_at(row, 0xB0),
                },
                "item",
            )?;
        }
        for record_type in SCROLL_RECORD_TYPES {
            if !items.contains_key(&record_type) {
                return Err(EffectError::MissingScrollItem { record_type });
            }
        }

        let mut groups = BTreeMap::new();
        for index in 0..resource.effect_group.row_count() {
            let group = GroupRow::new(resource.effect_group.row(index).expect("row in range"))?;
            insert_unique(
                &mut groups,
                group.group_key(),
                EffectGroupDefinition {
                    group_key: group.group_key(),
                    category_key: group.category_key(),
                    conflict_mask_0: group.conflict_mask_0(),
                    conflict_mask_1: group.conflict_mask_1(),
                },
                "effect_group",
            )?;
        }

        let mut categories = BTreeMap::new();
        for index in 0..resource.category.row_count() {
            let row = resource.category.row(index).expect("row in range");
            let mut rarity_capacities = [0u16; 6];
            for (rarity, capacity) in rarity_capacities.iter_mut().enumerate() {
                *capacity = u16_at(row, 0x18 + rarity * 2);
            }
            insert_unique(
                &mut categories,
                u16_at(row, 0x08),
                CategoryDefinition {
                    row_index: index,
                    category_key: u16_at(row, 0x08),
                    rarity_capacities,
                    mode12_lottery_weight: u16_at(row, 0x5A),
                    mode12_capacity: u16_at(row, 0x5C),
                    mode12_count_multiplier_key: u16_at(row, 0x5E),
                },
                "category",
            )?;
        }

        let mut multipliers = BTreeMap::new();
        for index in 0..resource.category_count_multiplier.row_count() {
            let row = resource
                .category_count_multiplier
                .row(index)
                .expect("row in range");
            let mut values = [0f32; 7];
            for (slot, value) in values.iter_mut().enumerate() {
                *value = f32_at(row, slot * 4);
            }
            insert_unique(
                &mut multipliers,
                u32_at(row, 0x1C),
                CategoryCountMultiplierDefinition {
                    lookup_key: u32_at(row, 0x1C),
                    multipliers: values,
                },
                "category_count_multiplier",
            )?;
        }

        let mut effects = BTreeMap::new();
        let mut effects_in_row_order = Vec::with_capacity(resource.effect.row_count());
        for row_index in 0..resource.effect.row_count() {
            let row = resource.effect.row(row_index).expect("row in range");
            let effect = EffectRow::new(row)?;
            let mut rarity_weights = [0f32; 6];
            for (rarity, weight) in rarity_weights.iter_mut().enumerate() {
                *weight = f32_at(row, 0x28 + rarity * 4);
            }
            let mut type_multipliers = [0f32; 4];
            for (class, multiplier) in type_multipliers.iter_mut().enumerate() {
                *multiplier = f32_at(row, 0x44 + class * 4);
            }
            let definition = EffectDefinition {
                effect_id: effect.effect_id(),
                group_key: effect.group_key(),
                flags: effect.flags(),
                normalization_flags: effect.normalization_flags(),
                progress_threshold: effect.progress_threshold(),
                alternate_threshold: effect.alternate_threshold(),
                lottery_weights: effect.lottery_weights(),
                base_value: i16_at(row, 0x08),
                roll_low: i16_at(row, 0x0A),
                roll_high: i16_at(row, 0x0C),
                rarity_weights,
                type_multipliers,
                curve_selector: u16_at(row, 0x06),
                row_index,
            };
            insert_unique(
                &mut effects,
                definition.effect_id,
                definition.clone(),
                "effect",
            )?;
            effects_in_row_order.push(definition);
        }
        for effect in effects.values() {
            if !groups.contains_key(&effect.group_key) {
                return Err(EffectError::UnknownGroup {
                    group_key: effect.group_key,
                });
            }
        }

        let mut optional_multipliers = BTreeMap::new();
        for index in 0..resource.optional_multiplier.row_count() {
            let row = resource
                .optional_multiplier
                .row(index)
                .expect("row in range");
            let definition = OptionalMultiplierDefinition {
                lookup_key: u32_at(row, 0x14),
                base_value: i32_at(row, 0x10),
                multiplier: f32_at(row, 0x18),
            };
            insert_unique(
                &mut optional_multipliers,
                definition.lookup_key,
                definition,
                "optional_multiplier",
            )?;
        }

        let rarity_rows = resource.rarity_roll.row_count();
        if rarity_rows != RARITY_ROLL_ROW_COUNT {
            return Err(EffectError::RarityRollRowCount { rows: rarity_rows });
        }
        let mut rarity_generation = Vec::with_capacity(rarity_rows);
        for row_index in 0..rarity_rows {
            let row = resource.rarity_roll.row(row_index).expect("row in range");
            rarity_generation.push(RarityGenerationDefinition {
                rarity: row_index as u8,
                minimum_roll_percent: u32_at(row, 0x1C),
                maximum_roll_percent: u32_at(row, 0x20),
                base_slot_count: u32_at(row, 0x44),
                total_slot_count: u32_at(row, 0x4C),
                promotion_trials: u32_at(row, 0x58),
                promotion_probability_percent: f32_at(row, 0xDC),
            });
        }

        let mut special_context = Vec::with_capacity(resource.special_context.row_count());
        for index in 0..resource.special_context.row_count() {
            let row = resource
                .special_context
                .row(index)
                .expect("row index within count");
            special_context.push(SpecialContextDefinition {
                mode: row[0x28],
                branch_class: row[0x29],
                reveal_weight_flag: row[0x2F] & 0x01 != 0,
            });
        }

        // Table-only callers may omit the progress blob; the weight path then
        // fails closed on the missing selector instead of guessing a vector.
        if !resource.playthrough_progress.len().is_multiple_of(16) {
            return Err(EffectError::RowSizeMismatch {
                table: "playthrough_progress",
                row_size: 16,
                bytes: resource.playthrough_progress.len(),
            });
        }
        let playthrough_progress = resource
            .playthrough_progress
            .chunks_exact(16)
            .map(|chunk| {
                let mut values = [0u32; 4];
                for (slot, value) in values.iter_mut().enumerate() {
                    *value = u32::from_le_bytes([
                        chunk[slot * 4],
                        chunk[slot * 4 + 1],
                        chunk[slot * 4 + 2],
                        chunk[slot * 4 + 3],
                    ]);
                }
                values
            })
            .collect();

        let mut level_curve = Vec::with_capacity(resource.level_curve.row_count());
        for index in 0..resource.level_curve.row_count() {
            let row = resource.level_curve.row(index).expect("row in range");
            level_curve.push([u16_at(row, 0), u16_at(row, 2), u16_at(row, 4)]);
        }

        Ok(Self {
            items_by_record_type: items,
            groups_by_key: groups,
            categories_by_key: categories,
            count_multipliers_by_key: multipliers,
            effects_by_id: effects,
            effects_in_row_order,
            optional_multipliers_by_key: optional_multipliers,
            rarity_generation,
            special_context,
            playthrough_progress,
            level_curve,
        })
    }

    pub fn effect(&self, effect_id: u16) -> Option<&EffectDefinition> {
        self.effects_by_id.get(&effect_id)
    }

    pub fn group_for_effect(&self, effect_id: u16) -> Option<&EffectGroupDefinition> {
        let group_key = self.effect(effect_id)?.group_key;
        self.groups_by_key.get(&group_key)
    }

    pub fn category_for_effect(&self, effect_id: u16) -> Option<&CategoryDefinition> {
        let category_key = self.group_for_effect(effect_id)?.category_key;
        self.categories_by_key.get(&category_key)
    }

    /// Rarity-indexed category capacities, matching `category_capacities`.
    ///
    /// RVA 0x91B6E8 builds a 32-entry `uint8` vector; category `0x1A` keeps its
    /// raw rarity capacity while every other key is clamped by the mode-0x12
    /// capacity.
    pub fn category_capacities(
        &self,
        record_type: u16,
        rarity: u8,
    ) -> Result<[u16; CATEGORY_CAPACITY_SLOTS], EffectError> {
        let item = self.item(record_type)?;
        if item.mode != SCROLL_ITEM_MODE {
            return Err(EffectError::UnexpectedMode {
                table: "item",
                record_type,
                mode: item.mode,
            });
        }
        let rarity_index = usize::from(rarity);
        if rarity_index > 5 {
            return Err(EffectError::UnsupportedRarity { rarity });
        }
        let mut capacities = [0u16; CATEGORY_CAPACITY_SLOTS];
        for category in self.categories_by_key.values() {
            let key = usize::from(category.category_key);
            if key >= CATEGORY_CAPACITY_SLOTS {
                return Err(EffectError::CategoryOutsideNativeVector {
                    category_key: category.category_key,
                });
            }
            let base = category.rarity_capacities[rarity_index];
            capacities[key] = if category.category_key == 0x1A {
                base
            } else {
                base.min(category.mode12_capacity)
            };
        }
        Ok(capacities)
    }

    /// Scroll item row lookup, mirroring `EffectGenerationTableIndex.item`.
    pub fn item(&self, record_type: u16) -> Result<&ScrollItemDefinition, EffectError> {
        self.items_by_record_type
            .get(&record_type)
            .ok_or(EffectError::UnknownRecordType { record_type })
    }

    /// Effect lookup that also enforces the `uint16` row-key contract.
    pub fn effect_u32(&self, effect_id: u32) -> Result<&EffectDefinition, EffectError> {
        let key = u16::try_from(effect_id).map_err(|_| EffectError::UnknownEffect { effect_id })?;
        self.effects_by_id
            .get(&key)
            .ok_or(EffectError::UnknownEffect { effect_id })
    }

    /// Group row for an effect ID, mirroring `group_for_effect`.
    pub fn group_for_effect_u32(
        &self,
        effect_id: u32,
    ) -> Result<&EffectGroupDefinition, EffectError> {
        let group_key = self.effect_u32(effect_id)?.group_key;
        self.groups_by_key
            .get(&group_key)
            .ok_or(EffectError::UnknownGroup { group_key })
    }

    /// Category row for an effect ID, mirroring `category_for_effect`.
    pub fn category_for_effect_u32(
        &self,
        effect_id: u32,
    ) -> Result<&CategoryDefinition, EffectError> {
        let category_key = self.group_for_effect_u32(effect_id)?.category_key;
        self.categories_by_key
            .get(&category_key)
            .ok_or(EffectError::UnknownCategory { category_key })
    }

    /// Playthrough progress vector for selector 1..=5 (RVA 0x578CD4).
    pub fn playthrough_progress(&self, selector: u8) -> Result<[u32; 4], EffectError> {
        if selector == 0 {
            return Err(EffectError::PlaythroughSelector { selector });
        }
        self.playthrough_progress
            .get(usize::from(selector) - 1)
            .copied()
            .ok_or(EffectError::PlaythroughSelector { selector })
    }

    /// Optional-multiplier lookup by key, mirroring `optional_multiplier`.
    pub fn optional_multiplier(&self, lookup_key: u32) -> Result<f32, EffectError> {
        self.optional_multipliers_by_key
            .get(&lookup_key)
            .map(|definition| definition.multiplier)
            .ok_or(EffectError::MissingOptionalMultiplier { lookup_key })
    }

    /// Optional-multiplier row with its signed base, for the auxiliary lottery.
    pub fn optional_multiplier_row(
        &self,
        lookup_key: u32,
    ) -> Result<&OptionalMultiplierDefinition, EffectError> {
        self.optional_multipliers_by_key
            .get(&lookup_key)
            .ok_or(EffectError::MissingOptionalMultiplier { lookup_key })
    }

    /// Category-count multiplier selected by the recovered RVA 0x3DADC4 path.
    ///
    /// The reference indexes the row's seven `f32` values by the live category
    /// count and falls back to slot zero once the count reaches the row width.
    pub fn category_count_multiplier(
        &self,
        lookup_key: u32,
        count: usize,
    ) -> Result<f32, EffectError> {
        let definition = self
            .count_multipliers_by_key
            .get(&lookup_key)
            .ok_or(EffectError::MissingCategoryCountMultiplier { lookup_key })?;
        let index = if count < definition.multipliers.len() {
            count
        } else {
            0
        };
        Ok(definition.multipliers[index])
    }

    /// The unique `special_context` row carrying one auxiliary mode byte.
    ///
    /// Duplicate modes are rejected instead of silently taking the first row,
    /// matching the reference's uniqueness check.
    pub fn special_context_row_for_mode(
        &self,
        mode: u8,
    ) -> Result<Option<&SpecialContextDefinition>, EffectError> {
        let mut matched = self
            .special_context
            .iter()
            .filter(|definition| definition.mode == mode);
        let first = matched.next();
        if matched.next().is_some() {
            return Err(EffectError::AmbiguousSpecialContext { mode });
        }
        Ok(first)
    }

    /// Rarity row, mirroring `rarity_generation[rarity]`.
    pub fn rarity_generation(&self, rarity: u8) -> Result<RarityGenerationDefinition, EffectError> {
        self.rarity_generation
            .get(usize::from(rarity))
            .copied()
            .ok_or(EffectError::UnsupportedRarity { rarity })
    }

    /// Level-curve lookup at RVA 0x571570, on header-stripped 10-byte rows.
    pub fn curve_scale(&self, level: u16, selector: u16) -> Result<i32, EffectError> {
        let Some(row) = self.level_curve.get(usize::from(level)) else {
            return Ok(0);
        };
        row.get(usize::from(selector))
            .copied()
            .map(i32::from)
            .ok_or(EffectError::CurveSelector { selector })
    }

    /// Base resolved value at RVA 0x571478, excluding the optional additions
    /// handled by 0x5712D8.
    pub fn resolved_effect_value(
        &self,
        effect_id: u32,
        roll_percent: u8,
        level: u16,
    ) -> Result<i32, EffectError> {
        let effect = self.effect_u32(effect_id)?;
        let level = level.min(MAX_CURVE_LEVEL);
        let base = i32::from(effect.base_value);
        let low = i32::from(effect.roll_low);
        let high = i32::from(effect.roll_high);

        let scaled = if effect.normalization_flags & 0x10 != 0 {
            if roll_percent < 80 {
                return Ok(base);
            }
            f32_div(
                f32_sub(f32_of(f64::from(roll_percent)), f32_of(20.0)),
                f32_of(20.0),
            )
        } else {
            f32_mul(f32_of(f64::from(roll_percent)), f32_of(0.01))
        };

        let interpolated = f32_add(
            f32_of(f64::from(low)),
            f32_mul(f32_of(f64::from(high - low)), scaled),
        );
        let curve = self.curve_scale(level, effect.curve_selector)?;
        let scaled_curve = f32_mul(
            f32_mul(f32_of(f64::from(curve)), f32_of(0.001)),
            interpolated,
        );
        trunc_f32_i32(
            f32_add(f32_of(f64::from(base)), scaled_curve),
            "resolved_base_value",
        )
    }

    /// Candidate weight at RVA 0x57896C, including binary32 ordering and
    /// integer truncation.
    pub fn native_effect_weight(
        &self,
        effect_id: u16,
        context: NativeWeightContext,
    ) -> Result<i64, EffectError> {
        let item = self.item(context.record_type)?;
        if item.mode != SCROLL_ITEM_MODE {
            return Err(EffectError::UnexpectedMode {
                table: "item",
                record_type: context.record_type,
                mode: item.mode,
            });
        }
        if context.rarity > 5 {
            return Err(EffectError::UnsupportedRarity {
                rarity: context.rarity,
            });
        }
        let weight_slot = if context.restricted_destination_slot {
            0x29
        } else {
            // Out-of-range selectors fall through to the native "weight 0"
            // branch, which `EffectDefinition::slot_weight` already models.
            usize::try_from(item.field_15c).unwrap_or(usize::MAX)
        };
        self.native_effect_weight_with_slot(effect_id, context, weight_slot)
    }

    /// [`Self::native_effect_weight`] with an explicit weight-slot selector.
    ///
    /// The recovered R4 finalizer substitutes slots `0x3C`/`0x3D`/`0x3E` for
    /// the item row's `+0x15C` selector, so the slot is an input rather than a
    /// derived field. Out-of-range selectors keep the native "weight 0" branch.
    pub fn native_effect_weight_with_slot(
        &self,
        effect_id: u16,
        context: NativeWeightContext,
        weight_slot: usize,
    ) -> Result<i64, EffectError> {
        let item = self.item(context.record_type)?;
        if item.mode != SCROLL_ITEM_MODE {
            return Err(EffectError::UnexpectedMode {
                table: "item",
                record_type: context.record_type,
                mode: item.mode,
            });
        }
        if context.rarity > 5 {
            return Err(EffectError::UnsupportedRarity {
                rarity: context.rarity,
            });
        }
        let effect = self
            .effects_by_id
            .get(&effect_id)
            .ok_or(EffectError::UnknownEffect {
                effect_id: u32::from(effect_id),
            })?;
        let type_class = type_class_for_record_type(
            context.record_type,
            context.rarity,
            context.rarity5_type_floor,
        );
        let progress = self.playthrough_progress(context.playthrough)?;
        self.effect_weight(effect, weight_slot, type_class, context, &progress)
    }

    fn effect_weight(
        &self,
        effect: &EffectDefinition,
        weight_slot: usize,
        type_class: u8,
        context: NativeWeightContext,
        progress: &[u32; 4],
    ) -> Result<i64, EffectError> {
        let gate = effect.progress_threshold;
        let enabled = progress_value_for(progress, gate) >= u32::from(gate);
        let mut accumulator = f32_of(if enabled { 1.0 } else { 0.0 });
        let base = effect.rarity_weight(context.rarity);
        let mut optional = f32_of(1.0);

        if type_class >= 5 && effect.alternate_threshold != 0 {
            let key = if u32::from(context.extra_selector) == u32::from(effect.alternate_threshold)
            {
                0x0415
            } else {
                0xA6D1
            };
            optional = self.optional_multiplier(key)?;
        }

        accumulator = f32_mul(accumulator, effect.type_multiplier(type_class));
        accumulator = f32_mul(accumulator, base);
        if type_class >= 5 {
            accumulator = f32_mul(accumulator, optional);
        }

        let slot_weight = effect.slot_weight(weight_slot);
        let result = f32_mul(
            f32_mul(f32_of(f64::from(slot_weight)), f32_of(100.0)),
            accumulator,
        );
        trunc_f32(result, "effect_weight")
    }

    /// Effect/category context gate from RVA 0x5788FC.
    pub fn candidate_context_allowed(
        &self,
        effect_id: u16,
        record_type: u16,
        alternate_runtime_context: bool,
    ) -> Result<bool, EffectError> {
        let effect = self
            .effects_by_id
            .get(&effect_id)
            .ok_or(EffectError::UnknownEffect {
                effect_id: u32::from(effect_id),
            })?;
        let item = self.item(record_type)?;
        let required_context_bit = if alternate_runtime_context {
            0x80
        } else {
            0x40
        };
        if effect.flags & required_context_bit == 0 {
            return Ok(false);
        }
        if effect.flags & 0x04 != 0 && item.candidate_item_flags & 0x0800 == 0 {
            return Ok(false);
        }
        if effect.flags & 0x08 != 0 && item.candidate_item_flags & 0x1000 == 0 {
            return Ok(false);
        }
        Ok(true)
    }

    /// Compatibility check shared by the pool builder, mirroring `is_compatible`.
    pub fn is_compatible(
        &self,
        candidate_effect_id: u16,
        existing_effect_ids: &[u32],
        special_effect_id: Option<u32>,
    ) -> Result<bool, EffectError> {
        let candidate =
            self.group_for_effect(candidate_effect_id)
                .ok_or(EffectError::UnknownEffect {
                    effect_id: u32::from(candidate_effect_id),
                })?;
        for existing in existing_effect_ids {
            if groups_conflict(candidate, self.group_for_effect_u32(*existing)?) {
                return Ok(false);
            }
        }
        if let Some(special) = special_effect_id {
            if groups_conflict(candidate, self.group_for_effect_u32(special)?) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Statically recovered per-slot candidate pool (RVAs 0x57818D..0x57825B).
    pub fn weighted_candidate_pool(
        &self,
        request: &CandidatePoolRequest,
        existing_effect_ids: &[u32],
    ) -> Result<Vec<WeightedEffectCandidate>, EffectError> {
        if request.destination_effect_flags & UNSUPPORTED_DESTINATION_FLAG != 0 {
            return Err(EffectError::UnsupportedDestinationFlag);
        }
        for (key, value) in request.remaining_category_capacities.iter().enumerate() {
            if *value > 0xFF {
                return Err(EffectError::CapacityNotByte {
                    category_key: key as u16,
                    value: *value,
                });
            }
        }
        let promoted = request.destination_effect_flags & EFFECT_FLAG_PROMOTED != 0;
        let requested_category = request.destination_category_and_flags & 0x3F;
        let mut result = Vec::new();
        for effect in &self.effects_in_row_order {
            if effect.row_index == 0 {
                continue;
            }
            let supports_promoted_slot = effect.normalization_flags & 0x08 != 0;
            if supports_promoted_slot != promoted {
                continue;
            }
            let category_key = self
                .groups_by_key
                .get(&effect.group_key)
                .ok_or(EffectError::UnknownGroup {
                    group_key: effect.group_key,
                })?
                .category_key;
            if !promoted && requested_category != 0 && u16::from(requested_category) != category_key
            {
                continue;
            }
            if !self.candidate_context_allowed(
                effect.effect_id,
                request.context.record_type,
                request.alternate_runtime_context,
            )? {
                continue;
            }
            let weight = self.native_effect_weight(
                effect.effect_id,
                NativeWeightContext {
                    restricted_destination_slot: false,
                    ..request.context
                },
            )?;
            if weight == 0 {
                continue;
            }
            let capacity = usize::from(category_key);
            if capacity >= CATEGORY_CAPACITY_SLOTS
                || request.remaining_category_capacities[capacity] == 0
            {
                continue;
            }
            if !self.is_compatible(
                effect.effect_id,
                existing_effect_ids,
                request.special_effect_id,
            )? {
                continue;
            }
            result.push(WeightedEffectCandidate {
                effect_id: effect.effect_id,
                weight,
            });
        }
        Ok(result)
    }

    /// Inclusive `0..=total` lottery at RVA 0x57830D..0x57833A.
    ///
    /// The first row carries one extra lattice point because the native
    /// comparison is `r <= w`; that bias is required for parity.
    pub fn select_weighted_candidate(
        &self,
        candidates: &[WeightedEffectCandidate],
        rng: &mut LcgStream,
    ) -> Result<Option<WeightedEffectCandidate>, EffectError> {
        let positive: Vec<WeightedEffectCandidate> = candidates
            .iter()
            .filter(|candidate| candidate.weight != 0)
            .copied()
            .collect();
        if positive.is_empty() {
            return Ok(None);
        }
        let total = positive.iter().fold(0i64, |accumulator, candidate| {
            accumulator + candidate.weight
        }) as u32;
        let upper_count = total.wrapping_add(1);
        if upper_count == 0 {
            return Err(EffectError::LotteryWrapped);
        }
        let mut ticket = draw_int(rng, upper_count)?.min(total);
        for candidate in positive {
            let weight = candidate.weight as u32;
            if ticket <= weight {
                return Ok(Some(candidate));
            }
            ticket = ticket.wrapping_sub(weight);
        }
        Ok(None)
    }

    /// Rarity-roll percentile lottery, mirroring RVA 0x980D58.
    pub fn roll_effect_percentile(
        &self,
        rarity: u8,
        rng: &mut LcgStream,
        use_fixed_maximum: bool,
    ) -> Result<u8, EffectError> {
        let definition = self.rarity_generation(rarity)?;
        if use_fixed_maximum {
            return Ok((definition.maximum_roll_percent & 0xFF) as u8);
        }
        roll_percentile(
            definition.minimum_roll_percent,
            definition.maximum_roll_percent,
            rng,
        )
    }

    /// Promoted-slot selection at RVA 0x110EE26..0x110EFAD for the ordinary
    /// fresh-scroll path with zero descriptor flag overrides.
    pub fn select_promoted_slot_indexes(
        &self,
        request: PromotedSlotRequest<'_>,
        rng: &mut LcgStream,
    ) -> Result<Vec<u8>, EffectError> {
        let item = self.item(request.record_type)?;
        if item.mode != SCROLL_ITEM_MODE {
            return Err(EffectError::UnexpectedMode {
                table: "item",
                record_type: request.record_type,
                mode: item.mode,
            });
        }
        let definition = self.rarity_generation(request.rarity)?;
        let slot_limit = request
            .slot_limit
            .map_or(definition.total_slot_count, u32::from);
        if slot_limit > 7 {
            return Err(EffectError::SlotLimit {
                slot_limit: slot_limit.min(u32::from(u8::MAX)) as u8,
            });
        }
        let slot_limit = slot_limit as u8;

        let threshold = trunc_f32(
            f32_mul(definition.promotion_probability_percent, f32_of(100.0)),
            "promotion_threshold",
        )?;
        let mut promoted_count = 0u32;
        for _ in 0..definition.promotion_trials {
            let ticket = trunc_f32(
                f32_mul(rng_float01(rng), f32_of(10000.0)),
                "promotion_ticket",
            )?
            .min(9999);
            if ticket < threshold {
                promoted_count += 1;
            }
        }

        let type_class = type_class_for_record_type(
            request.record_type,
            request.rarity,
            request.rarity5_type_floor,
        );
        if type_class < 3 || promoted_count == 0 {
            return Ok(Vec::new());
        }

        let mut order = [0u8, 1, 2, 3, 4, 5, 6];
        for position in 0..7usize {
            let swap_index = trunc_f32(f32_mul(rng_float01(rng), f32_of(7.0)), "promotion_swap")?
                .min(6) as usize;
            order.swap(position, swap_index);
        }

        let mut selected = Vec::new();
        for slot_index in order {
            if slot_index >= slot_limit {
                continue;
            }
            // Mode 0x12 explicitly permits category flag 0x40 here; effect
            // flags 0x01/0x02 still identify slots that cannot be promoted.
            if request.effect_flags[usize::from(slot_index)] & 0x03 != 0 {
                continue;
            }
            selected.push(slot_index);
            if selected.len() as u32 == promoted_count {
                break;
            }
        }
        Ok(selected)
    }
}

/// Next binary32 value in `[0, 1)`, mirroring `Lcg32.next_float01`.
pub(crate) fn rng_float01(rng: &mut LcgStream) -> f32 {
    f32_mul(f32_of(f64::from(rng.u16())), f32_of(1.0 / 65536.0))
}

/// Type class selected by the record type, mirroring `type_class_for_record_type`.
pub fn type_class_for_record_type(record_type: u16, rarity: u8, rarity5_floor: u8) -> u8 {
    let mut result = match record_type {
        0x1E82 => 1,
        0x516D => 2,
        0xDD82 => 4,
        0xD523 => 5,
        _ => 3,
    };
    if rarity == 5 {
        result = result.max(rarity5_floor);
    }
    result
}

fn progress_value_for(progress: &[u32; 4], threshold: u16) -> u32 {
    progress[progress_bucket(threshold)]
}

pub(crate) fn groups_conflict(
    candidate: &EffectGroupDefinition,
    existing: &EffectGroupDefinition,
) -> bool {
    candidate.group_key == existing.group_key
        || candidate.conflict_mask_0 & existing.conflict_mask_0 != 0
        || candidate.conflict_mask_1 & existing.conflict_mask_1 != 0
}

fn expect_stride(table: &'static str, actual: usize, expected: usize) -> Result<(), EffectError> {
    if actual != expected {
        return Err(EffectError::UnexpectedStride {
            table,
            expected,
            actual,
        });
    }
    Ok(())
}

fn insert_unique<K, T>(
    target: &mut BTreeMap<K, T>,
    key: K,
    value: T,
    table: &'static str,
) -> Result<(), EffectError>
where
    K: Ord + Copy + Into<u32>,
{
    if target.insert(key, value).is_some() {
        return Err(EffectError::DuplicateKey {
            table,
            key: key.into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_bytes_require_whole_rows() {
        assert!(EffectTableBytes::new("effect", 0xD8, vec![0; 0xD8 * 3]).is_ok());
        assert_eq!(
            EffectTableBytes::new("effect", 0xD8, vec![0; 0xD8 + 1]),
            Err(EffectError::RowSizeMismatch {
                table: "effect",
                row_size: 0xD8,
                bytes: 0xD9,
            })
        );
        assert_eq!(
            EffectTableBytes::new("effect", 0, vec![]),
            Err(EffectError::RowSizeMismatch {
                table: "effect",
                row_size: 0,
                bytes: 0,
            })
        );
    }

    #[test]
    fn effect_row_decodes_reference_offsets() {
        let mut raw = [0u8; EFFECT_ROW_BYTES];
        raw[0x00..0x02].copy_from_slice(&0x6553u16.to_le_bytes());
        raw[0x02..0x04].copy_from_slice(&0xB991u16.to_le_bytes());
        raw[0x1C..0x20].copy_from_slice(&0x0000_0004u32.to_le_bytes());
        raw[0x54..0x56].copy_from_slice(&7u16.to_le_bytes());
        raw[0x56..0x58].copy_from_slice(&9u16.to_le_bytes());
        raw[0x58..0x5A].copy_from_slice(&0x1234u16.to_le_bytes());
        let row = EffectRow::new(&raw).unwrap();
        assert_eq!(row.effect_id(), 0x6553);
        assert_eq!(row.group_key(), 0xB991);
        assert_eq!(row.flags(), 4);
        assert_eq!(row.progress_threshold(), 7);
        assert_eq!(row.alternate_threshold(), 9);
        assert_eq!(row.slot_weight(0), 0x1234);
        assert_eq!(row.slot_weight(64), 0);
        assert_eq!(row.lottery_weights().len(), 64);
        assert_eq!(
            EffectRow::new(&raw[..0xD7]).unwrap_err(),
            EffectError::UnexpectedStride {
                table: "effect",
                expected: EFFECT_ROW_BYTES,
                actual: 0xD7,
            }
        );
    }

    #[test]
    fn grace_map_lookup_is_inclusive_and_dense() {
        let map = GraceMap {
            format: "nioh3-grace-first-u16-map-v2".into(),
            game_version: "2.00.02".into(),
            record_type: 0xE604,
            rarity: 4,
            capture_state: GRACE_MAP_CAPTURE_STATE.into(),
            effect_slot: 5,
            ranges: vec![
                GraceRange {
                    start: 0,
                    end: 0x0FFF,
                    effect_id: 0x1111,
                },
                GraceRange {
                    start: 0x1000,
                    end: 0xFFFF,
                    effect_id: 0x2222,
                },
            ],
        };
        assert_eq!(map.validate(), Ok(()));
        assert_eq!(map.capture_state, "current-loaded-state");
        assert_eq!(map.grace_id_for_first_u16(0), Some(0x1111));
        assert_eq!(map.grace_id_for_first_u16(0x0FFF), Some(0x1111));
        assert_eq!(map.grace_id_for_first_u16(0x1000), Some(0x2222));
        assert_eq!(map.grace_id_for_first_u16(0xFFFF), Some(0x2222));

        let mut hole = map.clone();
        hole.ranges[1].start = 0x1001;
        assert!(hole.validate().is_err());
        let mut overlap = map.clone();
        overlap.ranges[1].start = 0x0F00;
        assert!(overlap.validate().is_err());
        let mut short = map.clone();
        short.ranges.pop();
        assert!(short.validate().is_err());
        let mut unknown_label = map.clone();
        unknown_label.capture_state = "NG3".into();
        assert_eq!(
            unknown_label.validate(),
            Err(EffectError::GraceMapMetadata {
                field: "capture_state"
            })
        );
        let mut empty_label = map;
        empty_label.capture_state = String::new();
        assert_eq!(
            empty_label.validate(),
            Err(EffectError::GraceMapMetadata {
                field: "capture_state"
            })
        );
    }

    #[test]
    fn unnamed_helper_offsets_are_bounds_checked() {
        let raw = [0u8; 4];
        assert_eq!(
            need("effect", &raw, 0x1C, 4),
            Err(EffectError::ShortRow {
                table: "effect",
                need: 0x20,
                have: 4,
            })
        );
        assert_eq!(need("effect", &raw, 0x00, 4), Ok(()));
    }
}

/// Synthetic table fixtures for the sequence and validation unit tests.
///
/// The shipped tables live behind the data adapter, so the domain tests build a
/// complete but deliberately tiny resource: six single-effect groups in one
/// category, exact rarity-roll rows and a five-entry progress store.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

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

    /// Ordinary (non-promotable) effect IDs; the first two back the Grace maps.
    pub(crate) const EFFECT_IDS: [u16; 6] = [0x1000, 0x1001, 0x1002, 0x1003, 0x1004, 0x1005];
    /// Effects whose `normalization_flags` permit a promoted slot.
    const PROMOTED_EFFECT_IDS: [u16; 3] = [0x1006, 0x1007, 0x1008];
    /// Fixed rarity-3 growing token, present in the shipped effect table.
    const GROWING_TOKEN_ID: u16 = 0x0001;
    const SYNTHETIC_GROUP_COUNT: usize = 10;

    fn item_table() -> EffectTableBytes {
        let mut table = TableBuilder::new(ITEM_ROW_BYTES);
        for record_type in SCROLL_RECORD_TYPES {
            let row = table.row();
            row[0x152..0x154].copy_from_slice(&record_type.to_le_bytes());
            row[0xB0..0xB4].copy_from_slice(&0x1800u32.to_le_bytes());
            row[0x15C..0x160].copy_from_slice(&0u32.to_le_bytes());
            row[0x182] = SCROLL_ITEM_MODE;
        }
        table.finish("item")
    }

    fn effect_table(curve_selector: u16, context_flags: u32) -> EffectTableBytes {
        let mut table = TableBuilder::new(EFFECT_ROW_BYTES);
        let sentinel = table.row();
        sentinel[0x02..0x04].copy_from_slice(&0x0100u16.to_le_bytes());
        let rows = EFFECT_IDS
            .iter()
            .copied()
            .map(|effect_id| (effect_id, 0u32))
            .chain(PROMOTED_EFFECT_IDS.iter().copied().map(|id| (id, 0x08u32)))
            .chain(std::iter::once((GROWING_TOKEN_ID, 0u32)));
        for (index, (effect_id, normalization_flags)) in rows.enumerate() {
            let row = table.row();
            row[0x00..0x02].copy_from_slice(&effect_id.to_le_bytes());
            row[0x02..0x04].copy_from_slice(&(0x0100u16 + index as u16).to_le_bytes());
            row[0x06..0x08].copy_from_slice(&curve_selector.to_le_bytes());
            row[0x08..0x0A].copy_from_slice(&10i16.to_le_bytes());
            row[0x0A..0x0C].copy_from_slice(&10i16.to_le_bytes());
            row[0x0C..0x0E].copy_from_slice(&20i16.to_le_bytes());
            row[0x1C..0x20].copy_from_slice(&context_flags.to_le_bytes());
            row[0x20..0x24].copy_from_slice(&normalization_flags.to_le_bytes());
            for offset in (0x28..0x3C).step_by(4) {
                row[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
            }
            row[0x3C..0x40].copy_from_slice(&1.0f32.to_le_bytes());
            for offset in (0x44..0x54).step_by(4) {
                row[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
            }
            for slot in 0..64 {
                row[0x58 + slot * 2..0x5A + slot * 2].copy_from_slice(&1u16.to_le_bytes());
            }
        }
        table.finish("effect")
    }

    fn table_with_rows(
        row_size: usize,
        rows: usize,
        name: &'static str,
        fill: impl Fn(usize, &mut [u8]),
    ) -> EffectTableBytes {
        let mut table = TableBuilder::new(row_size);
        for index in 0..rows {
            let row = table.row();
            fill(index, row);
        }
        table.finish(name)
    }

    /// Build a complete synthetic resource.
    ///
    /// `promotion_probability_percent` above 100 forces the promotion trial to
    /// succeed, which the accepted-path tests use to cover the shuffle branch.
    pub(crate) fn synthetic_resource_with(
        promotion_probability_percent: f32,
    ) -> EffectResourceBytes {
        let effect_group = table_with_rows(
            EFFECT_GROUP_ROW_BYTES,
            SYNTHETIC_GROUP_COUNT,
            "effect_group",
            |index, row| {
                row[0x0C..0x0E].copy_from_slice(&(0x0100u16 + index as u16).to_le_bytes());
                row[0x24..0x26].copy_from_slice(&5u16.to_le_bytes());
            },
        );
        let category = table_with_rows(CATEGORY_ROW_BYTES, 1, "category", |_index, row| {
            row[0x08..0x0A].copy_from_slice(&5u16.to_le_bytes());
            for rarity in 0..6 {
                row[0x18 + rarity * 2..0x1A + rarity * 2].copy_from_slice(&12u16.to_le_bytes());
            }
            row[0x5C..0x5E].copy_from_slice(&12u16.to_le_bytes());
            row[0x5E..0x60].copy_from_slice(&0u16.to_le_bytes());
        });
        let category_count_multiplier = table_with_rows(
            CATEGORY_COUNT_MULTIPLIER_ROW_BYTES,
            1,
            "category_count_multiplier",
            |_index, row| {
                for slot in 0..7 {
                    row[slot * 4..slot * 4 + 4].copy_from_slice(&1.0f32.to_le_bytes());
                }
            },
        );
        let rarity_roll =
            table_with_rows(RARITY_ROLL_ROW_BYTES, 6, "rarity_roll", |_index, row| {
                row[0x1C..0x20].copy_from_slice(&90u32.to_le_bytes());
                row[0x20..0x24].copy_from_slice(&94u32.to_le_bytes());
                row[0x44..0x48].copy_from_slice(&4u32.to_le_bytes());
                row[0x4C..0x50].copy_from_slice(&6u32.to_le_bytes());
                row[0x58..0x5C].copy_from_slice(&1u32.to_le_bytes());
                row[0xDC..0xE0].copy_from_slice(&promotion_probability_percent.to_le_bytes());
            });
        let level_curve =
            table_with_rows(LEVEL_CURVE_ROW_BYTES, 1, "level_curve", |_index, row| {
                for slot in 0..3 {
                    row[slot * 2..slot * 2 + 2].copy_from_slice(&1u16.to_le_bytes());
                }
            });
        let mut playthrough_progress = Vec::new();
        for selector in 1..=5u32 {
            for value in [selector, 0, 0, 0] {
                playthrough_progress.extend_from_slice(&value.to_le_bytes());
            }
        }

        EffectResourceBytes {
            schema: "nioh3-r4-finalizer-resource/v1".to_string(),
            item: item_table(),
            effect_group,
            category,
            category_count_multiplier,
            effect: effect_table(0, 0x40),
            level_curve,
            optional_multiplier: table_with_rows(
                OPTIONAL_MULTIPLIER_ROW_BYTES,
                0,
                "optional_multiplier",
                |_, _| {},
            ),
            rarity_roll,
            special_context: table_with_rows(48, 0, "special_context", |_, _| {}),
            bonus_curve_rows: Vec::new(),
            bonus_curve_index: Vec::new(),
            playthrough_progress,
            grace_maps: Vec::new(),
        }
    }

    pub(crate) fn synthetic_index() -> EffectTableIndex {
        EffectTableIndex::from_resource(&synthetic_resource_with(0.007))
            .expect("synthetic resource indexes")
    }

    pub(crate) fn promoting_index() -> EffectTableIndex {
        EffectTableIndex::from_resource(&synthetic_resource_with(100.0))
            .expect("synthetic resource indexes")
    }

    /// Resource whose effects fail the RVA 0x5788FC context gate.
    pub(crate) fn context_less_resource() -> EffectResourceBytes {
        let mut resource = synthetic_resource_with(0.007);
        resource.effect = effect_table(0, 0);
        resource
    }

    pub(crate) fn dense_map(record_type: u32, rarity: u8, effect_slot: u8) -> GraceMap {
        GraceMap {
            format: "nioh3-grace-first-u16-map-v2".to_string(),
            game_version: "2.00.02".to_string(),
            record_type,
            rarity,
            capture_state: GRACE_MAP_CAPTURE_STATE.to_string(),
            effect_slot,
            ranges: vec![
                GraceRange {
                    start: 0,
                    end: 0x7FFF,
                    effect_id: u32::from(EFFECT_IDS[0]),
                },
                GraceRange {
                    start: 0x8000,
                    end: 0xFFFF,
                    effect_id: u32::from(EFFECT_IDS[1]),
                },
            ],
        }
    }

    #[test]
    fn synthetic_resource_indexes_every_required_table() {
        let index = synthetic_index();
        assert_eq!(index.items_by_record_type.len(), SCROLL_RECORD_TYPES.len());
        assert_eq!(index.groups_by_key.len(), SYNTHETIC_GROUP_COUNT);
        // Sentinel row, ordinary rows, promoted-capable rows and the token.
        let expected_effects = EFFECT_IDS.len() + PROMOTED_EFFECT_IDS.len() + 2;
        assert_eq!(index.effects_by_id.len(), expected_effects);
        assert_eq!(index.effects_in_row_order.len(), expected_effects);
        assert_eq!(index.rarity_generation.len(), RARITY_ROLL_ROW_COUNT);
        assert_eq!(index.playthrough_progress.len(), 5);
        assert_eq!(index.playthrough_progress(3), Ok([3, 0, 0, 0]));
    }

    #[test]
    fn malformed_table_shapes_are_rejected() {
        let mut resource = synthetic_resource_with(0.007);
        resource.rarity_roll = table_with_rows(RARITY_ROLL_ROW_BYTES, 2, "rarity_roll", |_, _| {});
        assert_eq!(
            EffectTableIndex::from_resource(&resource).unwrap_err(),
            EffectError::RarityRollRowCount { rows: 2 }
        );

        let mut resource = synthetic_resource_with(0.007);
        resource.playthrough_progress.push(0);
        assert_eq!(
            EffectTableIndex::from_resource(&resource).unwrap_err(),
            EffectError::RowSizeMismatch {
                table: "playthrough_progress",
                row_size: 16,
                bytes: 81,
            }
        );

        let mut resource = synthetic_resource_with(0.007);
        resource.effect = effect_table(3, 0x40);
        let index = EffectTableIndex::from_resource(&resource).expect("indexes");
        assert_eq!(
            index.curve_scale(0, 3),
            Err(EffectError::CurveSelector { selector: 3 })
        );
        assert_eq!(index.curve_scale(400, 0), Ok(0));
    }

    #[test]
    fn capacity_and_item_gates_fail_closed() {
        let index = synthetic_index();
        assert_eq!(
            index.category_capacities(0x1234, 3),
            Err(EffectError::UnknownRecordType {
                record_type: 0x1234
            })
        );
        assert_eq!(
            index.category_capacities(0xE604, 6),
            Err(EffectError::UnsupportedRarity { rarity: 6 })
        );
        let capacities = index.category_capacities(0xE604, 3).expect("capacities");
        assert_eq!(capacities[5], 12);
        assert_eq!(capacities[0], 0);

        let mut resource = synthetic_resource_with(0.007);
        let mut item = TableBuilder::new(ITEM_ROW_BYTES);
        for record_type in SCROLL_RECORD_TYPES {
            let row = item.row();
            row[0x152..0x154].copy_from_slice(&record_type.to_le_bytes());
            row[0x182] = 0x11;
        }
        resource.item = item.finish("item");
        let index = EffectTableIndex::from_resource(&resource).expect("indexes");
        assert_eq!(
            index.category_capacities(0xE604, 3),
            Err(EffectError::UnexpectedMode {
                table: "item",
                record_type: 0xE604,
                mode: 0x11,
            })
        );
    }

    /// Stream whose next draw returns exactly `high16`.
    fn stream_yielding(high16: u16) -> LcgStream {
        let state = (u32::from(high16) << 16) | 0x1234;
        LcgStream::new(crate::rng::A_INV.wrapping_mul(state.wrapping_sub(1)))
    }

    #[test]
    fn inclusive_lottery_gives_the_extra_lattice_point_to_the_first_row() {
        let index = synthetic_index();
        // Two unit-weight rows: `total == 2`, so the native bound is 3 and the
        // lattice point `ticket == total == 2` belongs to the first row.
        let candidates = vec![
            WeightedEffectCandidate {
                effect_id: EFFECT_IDS[0],
                weight: 1,
            },
            WeightedEffectCandidate {
                effect_id: EFFECT_IDS[1],
                weight: 1,
            },
        ];
        // ticket = int(float01 * 3): high16 = 10000 -> 0, 30000 -> 1,
        // 43691 -> 2 (the full `0..=total` range of three lattice points).
        let mut rng = stream_yielding(10000);
        assert_eq!(
            index
                .select_weighted_candidate(&candidates, &mut rng)
                .expect("selection")
                .map(|candidate| candidate.effect_id),
            Some(EFFECT_IDS[0])
        );
        // `ticket == weight` stays with the first row: the inclusive `r <= w`
        // comparison is what gives the first row its extra lattice point.
        let mut boundary = stream_yielding(30000);
        assert_eq!(
            index
                .select_weighted_candidate(&candidates, &mut boundary)
                .expect("selection")
                .map(|candidate| candidate.effect_id),
            Some(EFFECT_IDS[0])
        );
        // The final lattice point subtracts the first row's weight and lands on
        // the second row.
        let mut second_row = stream_yielding(43691);
        assert_eq!(
            index
                .select_weighted_candidate(&candidates, &mut second_row)
                .expect("selection")
                .map(|candidate| candidate.effect_id),
            Some(EFFECT_IDS[1])
        );
        // Zero-weight rows are skipped rather than consuming the ticket.
        let zero_weighted = vec![
            WeightedEffectCandidate {
                effect_id: EFFECT_IDS[0],
                weight: 0,
            },
            WeightedEffectCandidate {
                effect_id: EFFECT_IDS[1],
                weight: 5,
            },
        ];
        let mut rng = stream_yielding(0);
        assert_eq!(
            index
                .select_weighted_candidate(&zero_weighted, &mut rng)
                .expect("selection")
                .map(|candidate| candidate.effect_id),
            Some(EFFECT_IDS[1])
        );
    }

    #[test]
    fn pool_and_weight_requests_reject_unsupported_parameters() {
        let index = synthetic_index();
        let mut request = CandidatePoolRequest {
            context: NativeWeightContext {
                record_type: 0xE604,
                rarity: 3,
                playthrough: 3,
                restricted_destination_slot: false,
                extra_selector: 0,
                rarity5_type_floor: 0,
            },
            destination_category_and_flags: 0x40,
            destination_effect_flags: 0x40,
            remaining_category_capacities: index
                .category_capacities(0xE604, 3)
                .expect("capacities"),
            special_effect_id: None,
            alternate_runtime_context: false,
        };
        assert_eq!(
            index.weighted_candidate_pool(&request, &[]),
            Err(EffectError::UnsupportedDestinationFlag)
        );

        request.destination_effect_flags = 0;
        request.remaining_category_capacities[5] = 300;
        assert_eq!(
            index.weighted_candidate_pool(&request, &[]),
            Err(EffectError::CapacityNotByte {
                category_key: 5,
                value: 300,
            })
        );
        request.remaining_category_capacities[5] = 12;

        // The playthrough selector must exist before any weight is computed.
        let mut resource = synthetic_resource_with(0.007);
        resource.playthrough_progress.clear();
        let table_only = EffectTableIndex::from_resource(&resource).expect("indexes");
        request.destination_effect_flags = 0;
        assert_eq!(
            table_only.weighted_candidate_pool(&request, &[]),
            Err(EffectError::PlaythroughSelector { selector: 3 })
        );

        let mut context = NativeWeightContext {
            record_type: 0xE604,
            rarity: 3,
            playthrough: 3,
            restricted_destination_slot: false,
            extra_selector: 0,
            rarity5_type_floor: 0,
        };
        assert!(
            index
                .native_effect_weight(EFFECT_IDS[0], context)
                .expect("weight")
                > 0
        );
        context.playthrough = 9;
        assert_eq!(
            index.native_effect_weight(EFFECT_IDS[0], context),
            Err(EffectError::PlaythroughSelector { selector: 9 })
        );

        assert_eq!(
            index.resolved_effect_value(0xFFFF, 90, 180),
            Err(EffectError::UnknownEffect { effect_id: 0xFFFF })
        );

        let promoted = PromotedSlotRequest {
            record_type: 0xE604,
            rarity: 3,
            category_and_flags: &[0u8; 7],
            effect_flags: &[0u8; 7],
            slot_limit: Some(8),
            rarity5_type_floor: 0,
        };
        assert_eq!(
            index.select_promoted_slot_indexes(promoted, &mut LcgStream::new(1)),
            Err(EffectError::SlotLimit { slot_limit: 8 })
        );
    }
}
