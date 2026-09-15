//! Measured-map machinery: seed math, the joint constraint solver, the primary
//! output maps and `native_search_maps.prepare_maps`.
//!
//! The shipped host accelerates a bounded search by measuring two things once
//! and reusing them: which first-draw bucket resolves to which result
//! (`GraceOutputMap`), and which draw bucket carries which primary effect
//! (`PrimaryOutputMap`, `PrimaryFirstDrawOutputMap`). A search then enumerates
//! the *intersection* of the caller's constraints instead of walking seeds, and
//! every proposed seed is still regenerated and verified by the game.
//!
//! Ports, and the reason each is faithful rather than approximate:
//!
//! - `nioh3_seed_math`'s LCG, float32 `RandomInt`, natural-ID shape and lazy
//!   first-u16 enumeration, including the exact f32 rounding;
//! - `joint_solver`'s `U16Runs`, `DrawConstraint`, the pivot choice, the stable
//!   bucket permutation and the resumable `pivot_trial` cursor. The shipped
//!   native accelerator computes the same `(seed, pivot_trial)` pairs in the
//!   same order, so the pure loop is order-equivalent;
//! - `primary_map`'s capture, validation, JSON payload and construction, so a
//!   later game-closed solve reads exactly what a live capture wrote;
//! - `native_search_maps.prepare_maps`, including the shipped rule that a
//!   measured map is only reused when its save fingerprint and generation
//!   context digest both match.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use nioh3_worker::grace_map::{GraceOutputMap, GraceRange};

use crate::error::HostError;
use crate::oracle::{BatchOracle, ORACLE_MAP_TIMEOUT_MS};
use crate::scan::{record_type_of, CATEGORY_TO_TYPE, SCROLL_RECORD_SIZE};

/// `nioh3_seed_math.LCG_MULTIPLIER`.
pub const LCG_MULTIPLIER: u32 = 0x0001_0DCD;
/// Its modular inverse.
pub const LCG_MULTIPLIER_INVERSE: u32 = 0xA5E2_A705;
/// High-16 draw buckets any complete map covers.
pub const DRAW_BUCKETS: usize = 0x10000;
/// `joint_solver.NATIVE_ACCELERATOR_CHUNK_TRIALS`' sibling: the shipped pivot
/// low-16 stride.
pub const LOW16_STRIDE: u32 = 0x9E37;
/// The shipped `random_int_count` for a first-draw enumeration.
pub const FIRST_DRAW_RANDOM_INT_COUNT: u32 = 10_000;
/// `primary_map.PRIMARY_MAP_SCHEMA`.
pub const PRIMARY_MAP_SCHEMA: &str = "nioh3-primary-effect-output-map/v2";
/// `primary_map`'s certified game version string.
pub const PRIMARY_MAP_GAME_VERSION: &str = "2.00.02";

/// Record type to category, the inverse of `CATEGORY_TO_TYPE`.
pub fn category_for_record_type(record_type: u16) -> Option<u8> {
    CATEGORY_TO_TYPE
        .iter()
        .position(|value| *value == record_type && *value != 0)
        .map(|index| index as u8)
}

/// `nioh3_seed_math.lcg_step`.
pub fn lcg_step(state: u32) -> u32 {
    LCG_MULTIPLIER.wrapping_mul(state).wrapping_add(1)
}

/// `nioh3_seed_math.lcg_rewind`.
pub fn lcg_rewind(state_after_draw: u32) -> u32 {
    LCG_MULTIPLIER_INVERSE.wrapping_mul(state_after_draw.wrapping_sub(1))
}

/// `nioh3_seed_math.seed_from_state_after_draw`.
pub fn seed_from_state_after_draw(state_after_draw: u32, draw_index: u32) -> u32 {
    let mut state = state_after_draw;
    for _ in 0..draw_index {
        state = lcg_rewind(state);
    }
    state
}

/// `nioh3_seed_math.state_after_draw_from_seed`.
pub fn state_after_draw_from_seed(seed: u32, draw_index: u32) -> u32 {
    let mut state = seed;
    for _ in 0..draw_index {
        state = lcg_step(state);
    }
    state
}

/// `nioh3_seed_math.is_natural_scroll_id`.
pub fn is_natural_scroll_id(seed: u32) -> bool {
    (seed & 0xF000_0000) == 0 && (seed & 0xFFFF) != 0
}

/// `nioh3_seed_math.game_random_int_from_u16`.
///
/// Two binary32 roundings and a truncation toward zero, which is the whole
/// reason this helper exists rather than a float-free equivalent.
pub fn game_random_int_from_u16(random_u16: u16, count: u32) -> u32 {
    let random_float = (f32::from(random_u16)) * (1.0f32 / 65536.0f32);
    let scaled = random_float * (count as f32);
    let result = scaled as i64;
    result.min(i64::from(count) - 1).max(0) as u32
}

/// `nioh3_seed_math.FirstDrawSeed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstDrawSeed {
    pub seed: u32,
    pub state1: u32,
    pub random_u16: u16,
    pub low16: u16,
    pub random_int: u32,
}

/// `nioh3_seed_math.iter_natural_seeds_for_first_u16`, materialized lazily.
pub fn iter_natural_seeds_for_first_u16(
    random_u16: u16,
    random_int_count: u32,
    start_low16: u32,
) -> Vec<FirstDrawSeed> {
    let sampled = game_random_int_from_u16(random_u16, random_int_count);
    let mut seeds = Vec::new();
    for low16 in start_low16..=0xFFFF {
        let state1 = (u32::from(random_u16) << 16) | low16;
        let seed = lcg_rewind(state1);
        if is_natural_scroll_id(seed) {
            seeds.push(FirstDrawSeed {
                seed,
                state1,
                random_u16,
                low16: low16 as u16,
                random_int: sampled,
            });
        }
    }
    seeds
}

/// `joint_solver.U16Runs`: sorted, disjoint, non-adjacent inclusive ranges.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct U16Runs {
    pub runs: Vec<(u32, u32)>,
}

impl U16Runs {
    /// `U16Runs.from_ranges`: sort, then merge adjacent and overlapping runs.
    pub fn from_ranges(ranges: impl IntoIterator<Item = (u32, u32)>) -> Self {
        let mut ordered: Vec<(u32, u32)> = ranges.into_iter().collect();
        ordered.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::new();
        for (start, end) in ordered {
            match merged.last_mut() {
                Some(last) if start <= last.1 + 1 => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        Self { runs: merged }
    }

    /// `U16Runs.from_values`.
    pub fn from_values(values: impl IntoIterator<Item = u32>) -> Self {
        let mut ordered: Vec<u32> = values.into_iter().collect();
        ordered.sort_unstable();
        ordered.dedup();
        Self::from_ranges(ordered.into_iter().map(|value| (value, value)))
    }

    pub fn bucket_count(&self) -> u32 {
        self.runs.iter().map(|(start, end)| end - start + 1).sum()
    }

    pub fn contains(&self, value: u32) -> bool {
        self.runs
            .iter()
            .any(|(start, end)| *start <= value && value <= *end)
    }

    pub fn iter_values(&self) -> impl Iterator<Item = u32> + '_ {
        self.runs.iter().flat_map(|(start, end)| *start..=*end)
    }
}

/// `joint_solver.DrawConstraint`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawConstraint {
    pub name: String,
    pub draw_index: u32,
    pub allowed_u16: U16Runs,
}

impl DrawConstraint {
    pub fn new(name: &str, draw_index: u32, allowed_u16: U16Runs) -> Result<Self, HostError> {
        if draw_index == 0 {
            return Err(HostError::rejected("draw_index must be positive"));
        }
        if allowed_u16.bucket_count() == 0 {
            return Err(HostError::rejected("constraint cannot be empty"));
        }
        Ok(Self {
            name: name.to_string(),
            draw_index,
            allowed_u16,
        })
    }

    /// `DrawConstraint.matches`.
    pub fn matches(&self, seed: u32) -> bool {
        let state = state_after_draw_from_seed(seed, self.draw_index);
        self.allowed_u16.contains(state >> 16)
    }
}

/// `joint_solver.SeedSolution`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedSolution {
    pub seed: u32,
    pub pivot_trial: u64,
    pub pivot_u16: u32,
    pub pivot_state_low16: u32,
}

/// `joint_solver.permuted_pivot_values`.
pub fn permuted_pivot_values(runs: &U16Runs) -> Vec<u32> {
    let values: Vec<u32> = runs.iter_values().collect();
    if values.is_empty() {
        return values;
    }
    let length = values.len() as u32;
    let mut stride = LOW16_STRIDE % length;
    if stride == 0 {
        stride = 1;
    }
    if stride.is_multiple_of(2) {
        stride += 1;
    }
    while gcd(stride, length) != 1 {
        stride += 2;
    }
    (0..length)
        .map(|index| values[((index * stride) % length) as usize])
        .collect()
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    a
}

/// `joint_solver.iter_constraint_intersection`, as a resumable cursor.
///
/// The shipped generator offers a native accelerator for the same family; the
/// inverse mapping in that path reconstructs `(seed, pivot_trial)` from the
/// flat index, so this pure enumeration yields the identical pairs in the
/// identical order.
pub struct ConstraintIntersection<'a> {
    constraints: Vec<DrawConstraint>,
    values: Vec<u32>,
    flat_index: u64,
    stop_index: u64,
    natural_only: bool,
    low16_stride: u32,
    pivot_name: String,
    pivot_draw_index: u32,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> ConstraintIntersection<'a> {
    /// `iter_constraint_intersection`'s argument validation and setup.
    pub fn new(
        constraints: Vec<DrawConstraint>,
        start_after_trial: u64,
        max_trials: Option<u64>,
    ) -> Result<Self, HostError> {
        if constraints.is_empty() {
            return Err(HostError::rejected("at least one constraint is required"));
        }
        let mut names: Vec<&str> = constraints.iter().map(|item| item.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        if names.len() != constraints.len() {
            return Err(HostError::rejected("constraint names must be unique"));
        }
        // `choose_pivot`: fewest buckets, then the later draw, then the name.
        let pivot = constraints
            .iter()
            .min_by(|left, right| {
                left.allowed_u16
                    .bucket_count()
                    .cmp(&right.allowed_u16.bucket_count())
                    .then(right.draw_index.cmp(&left.draw_index))
                    .then(left.name.cmp(&right.name))
            })
            .cloned()
            .ok_or_else(|| HostError::rejected("at least one constraint is required"))?;
        let values = permuted_pivot_values(&pivot.allowed_u16);
        let family_size = (values.len() as u64) * 0x1_0000;
        let first_index = start_after_trial.min(family_size);
        let stop_index = match max_trials {
            Some(max_trials) => family_size.min(first_index + max_trials),
            None => family_size,
        };
        Ok(Self {
            constraints,
            values,
            flat_index: first_index,
            stop_index,
            natural_only: true,
            low16_stride: LOW16_STRIDE,
            pivot_name: pivot.name,
            pivot_draw_index: pivot.draw_index,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn pivot_name(&self) -> &str {
        &self.pivot_name
    }

    /// The next exact intersection, or `None` when the family is exhausted.
    pub fn next_solution(&mut self) -> Option<SeedSolution> {
        let length = self.values.len() as u64;
        while self.flat_index < self.stop_index {
            let flat_index = self.flat_index;
            self.flat_index += 1;
            let low_index = flat_index / length;
            let bucket_index = flat_index % length;
            let low16 = (low_index as u32).wrapping_mul(self.low16_stride) & 0xFFFF;
            let rotation = (low_index % length) as usize;
            let u16_value = self.values[(rotation + bucket_index as usize) % self.values.len()];
            let state = (u16_value << 16) | low16;
            let seed = seed_from_state_after_draw(state, self.pivot_draw_index);
            if self.natural_only && !is_natural_scroll_id(seed) {
                continue;
            }
            let pivot_name = &self.pivot_name;
            if !self
                .constraints
                .iter()
                .filter(|item| item.name != *pivot_name)
                .all(|item| item.matches(seed))
            {
                continue;
            }
            return Some(SeedSolution {
                seed,
                pivot_trial: flat_index + 1,
                pivot_u16: u16_value,
                pivot_state_low16: low16,
            });
        }
        None
    }

    pub fn more(&self) -> bool {
        self.flat_index < self.stop_index
    }
}

/// `grace_map.grace_id_for_first_u16` (the shipped bisect over range starts).
pub fn grace_id_for_first_u16(first_u16: u32, mapping: &GraceOutputMap) -> Option<u32> {
    mapping
        .ranges
        .iter()
        .find(|range| range.start <= first_u16 && first_u16 <= range.end)
        .map(|range| range.grace_id)
}

/// `grace_map.first_u16_ranges_for_grace`.
pub fn first_u16_ranges_for_grace(
    grace_id: u32,
    mapping: &GraceOutputMap,
) -> Result<Vec<GraceRange>, HostError> {
    let result: Vec<GraceRange> = mapping
        .ranges
        .iter()
        .filter(|range| range.grace_id == grace_id)
        .copied()
        .collect();
    if result.is_empty() {
        return Err(HostError::rejected(format!(
            "grace ID 0x{grace_id:X} is not present in the grace output map"
        )));
    }
    Ok(result)
}

/// `grace_map.iter_natural_seeds_for_grace`, as a resumable cursor.
pub struct GraceSeedCursor {
    grace_ranges: Vec<GraceRange>,
    range_index: usize,
    first_u16: u32,
    low16: u32,
    pending: std::vec::IntoIter<FirstDrawSeed>,
    random_int_count: u32,
    started: bool,
}

impl GraceSeedCursor {
    /// `iter_natural_seeds_for_grace`'s setup, including the shipped resume
    /// contract: the cursor must name a natural seed whose own first draw
    /// belongs to the requested result.
    pub fn new(
        grace_id: u32,
        mapping: &GraceOutputMap,
        start_after_seed: Option<u32>,
    ) -> Result<Self, HostError> {
        let grace_ranges = first_u16_ranges_for_grace(grace_id, mapping)?;
        let mut range_index = 0usize;
        let mut resume_first_u16: Option<u32> = None;
        let mut resume_low16 = 0u32;
        if let Some(seed) = start_after_seed {
            if !is_natural_scroll_id(seed) {
                return Err(HostError::rejected(
                    "start_after_seed must be a natural scroll ID",
                ));
            }
            let state1 = lcg_step(seed);
            let first_u16 = state1 >> 16;
            if grace_id_for_first_u16(first_u16, mapping) != Some(grace_id) {
                return Err(HostError::rejected(
                    "start_after_seed does not belong to the requested grace",
                ));
            }
            match grace_ranges
                .iter()
                .position(|range| range.start <= first_u16 && first_u16 <= range.end)
            {
                Some(index) => {
                    range_index = index;
                    resume_first_u16 = Some(first_u16);
                    resume_low16 = (state1 & 0xFFFF) + 1;
                }
                None => {
                    return Err(HostError::rejected(
                        "start_after_seed is not covered by the requested grace",
                    ))
                }
            }
        }
        let first_u16 = resume_first_u16.unwrap_or_else(|| {
            grace_ranges
                .get(range_index)
                .map(|range| range.start)
                .unwrap_or(0)
        });
        Ok(Self {
            grace_ranges,
            range_index,
            first_u16,
            low16: resume_low16,
            pending: Vec::new().into_iter(),
            random_int_count: FIRST_DRAW_RANDOM_INT_COUNT,
            started: false,
        })
    }

    /// The next natural seed for this Grace, or `None` when the ranges end.
    pub fn next_seed(&mut self) -> Option<FirstDrawSeed> {
        loop {
            if let Some(seed) = self.pending.next() {
                return Some(seed);
            }
            let range = self.grace_ranges.get(self.range_index)?;
            if !self.started {
                // A resumed cursor starts inside its own range at the recorded
                // first_u16; a fresh one starts at the range's own start.
                self.started = true;
                self.first_u16 = self.first_u16.max(range.start);
            }
            if self.first_u16 > range.end {
                self.range_index += 1;
                self.low16 = 0;
                self.first_u16 = self
                    .grace_ranges
                    .get(self.range_index)
                    .map(|next| next.start)
                    .unwrap_or(0);
                continue;
            }
            let start_low16 = self.low16;
            self.low16 = 0;
            let first_u16 = self.first_u16;
            self.first_u16 += 1;
            self.pending = iter_natural_seeds_for_first_u16(
                first_u16 as u16,
                self.random_int_count,
                start_low16,
            )
            .into_iter();
        }
    }
}

/// `primary_map`'s representative draw-2 primary map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryOutputMap {
    pub game_version: String,
    pub record_type: u16,
    pub rarity: u8,
    pub playthrough: String,
    pub grace_effect_id: u32,
    pub grace_effect_slot: u8,
    pub draw_index: u32,
    pub effects: Vec<(u32, U16Runs)>,
}

/// `primary_map`'s first-draw primary map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryFirstDrawOutputMap {
    pub game_version: String,
    pub record_type: u16,
    pub rarity: u8,
    pub category: u8,
    pub draw_index: u32,
    pub effects: Vec<(u32, U16Runs)>,
}

/// One primary map of either kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimaryMap {
    Grace(PrimaryOutputMap),
    FirstDraw(PrimaryFirstDrawOutputMap),
}

impl PrimaryMap {
    pub fn effects(&self) -> &[(u32, U16Runs)] {
        match self {
            PrimaryMap::Grace(map) => &map.effects,
            PrimaryMap::FirstDraw(map) => &map.effects,
        }
    }

    pub fn bucket_count(&self) -> u32 {
        self.effects()
            .iter()
            .map(|(_, runs)| runs.bucket_count())
            .sum()
    }

    /// `runs_for_effects`: the union of the requested effects' buckets, with the
    /// shipped "this effect is not in this candidate pool" refusal.
    pub fn runs_for_effects(
        &self,
        effect_ids: &std::collections::BTreeSet<u32>,
        first_draw: bool,
    ) -> Result<U16Runs, HostError> {
        runs_for_effects(self.effects(), effect_ids, first_draw)
    }
}

/// `PrimaryOutputMap.runs_for_effects` over one effect partition.
pub fn runs_for_effects(
    effects: &[(u32, U16Runs)],
    effect_ids: &std::collections::BTreeSet<u32>,
    first_draw: bool,
) -> Result<U16Runs, HostError> {
    let available: BTreeMap<u32, &U16Runs> = effects.iter().map(|(id, runs)| (*id, runs)).collect();
    let mut missing: Vec<u32> = effect_ids
        .iter()
        .copied()
        .filter(|id| !available.contains_key(id))
        .collect();
    missing.sort_unstable();
    if !missing.is_empty() {
        let joined = missing
            .iter()
            .map(|id| format!("0x{id:04X}"))
            .collect::<Vec<_>>()
            .join(", ");
        let scope = if first_draw {
            "主词条在所选周目/稀有度候选池中不存在"
        } else {
            "主词条在当前恩宠/周目候选池中不存在"
        };
        return Err(HostError::rejected(format!("{scope}：{joined}")));
    }
    Ok(U16Runs::from_ranges(
        effect_ids
            .iter()
            .filter_map(|id| available.get(id))
            .flat_map(|runs| runs.runs.iter().copied()),
    ))
}

/// `primary_map._validate_complete_partition`.
fn validate_complete_partition(mapping: &PrimaryMap) -> Result<(), HostError> {
    let mut seen = vec![false; DRAW_BUCKETS];
    // The reference rejects an effect id above uint32 because a Python integer
    // can exceed it; the field is a `u32` here, so the type is the check.
    for (_effect_id, runs) in mapping.effects() {
        for value in runs.iter_values() {
            let index = value as usize;
            if index >= DRAW_BUCKETS {
                return Err(HostError::rejected("run leaves the uint16 domain"));
            }
            if seen[index] {
                return Err(HostError::rejected(format!(
                    "draw bucket 0x{value:04X} appears more than once"
                )));
            }
            seen[index] = true;
        }
    }
    if mapping.bucket_count() != DRAW_BUCKETS as u32 || seen.iter().any(|value| !value) {
        return Err(HostError::rejected(
            "primary output map does not cover all 65,536 draw buckets",
        ));
    }
    Ok(())
}

fn sha256_hex_text(value: &str, field: &str) -> Result<String, HostError> {
    let lowered = value.trim().to_lowercase();
    if lowered.len() != 64 || !lowered.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HostError::rejected(format!(
            "{field} must be a 64-character SHA-256 hex string"
        )));
    }
    Ok(lowered)
}

/// `primary_map.primary_map_to_payload`.
pub fn primary_map_to_payload(
    mapping: &PrimaryMap,
    context_fingerprint: &str,
    generation_context_digest: &str,
) -> Result<Value, HostError> {
    let fingerprint = sha256_hex_text(context_fingerprint, "context fingerprint")?;
    let digest = sha256_hex_text(generation_context_digest, "generation context digest")?;
    validate_complete_partition(mapping)?;
    let (game_version, record_type, rarity, draw_index) = match mapping {
        PrimaryMap::Grace(map) => (
            map.game_version.clone(),
            map.record_type,
            map.rarity,
            map.draw_index,
        ),
        PrimaryMap::FirstDraw(map) => (
            map.game_version.clone(),
            map.record_type,
            map.rarity,
            map.draw_index,
        ),
    };
    let effects: Vec<Value> = mapping
        .effects()
        .iter()
        .map(|(effect_id, runs)| {
            json!({
                "effect_id": format!("0x{effect_id:08X}"),
                "runs": runs.runs.iter().map(|(start, end)| json!([start, end])).collect::<Vec<_>>(),
            })
        })
        .collect();
    let mut payload = json!({
        "schema": PRIMARY_MAP_SCHEMA,
        "context_fingerprint": fingerprint,
        "generation_context_digest": digest,
        "game_version": game_version,
        "record_type": format!("0x{record_type:04X}"),
        "rarity": rarity,
        "draw_index": draw_index,
        "effects": effects,
    });
    let object = payload
        .as_object_mut()
        .ok_or_else(|| HostError::rejected("primary map payload is not an object"))?;
    match mapping {
        PrimaryMap::Grace(map) => {
            object.insert("kind".to_string(), json!("grace_conditioned_draw2"));
            object.insert("playthrough".to_string(), json!(map.playthrough));
            object.insert(
                "grace_effect_id".to_string(),
                json!(format!("0x{:08X}", map.grace_effect_id)),
            );
            object.insert(
                "grace_effect_slot".to_string(),
                json!(map.grace_effect_slot),
            );
        }
        PrimaryMap::FirstDraw(map) => {
            object.insert("kind".to_string(), json!("primary_draw1"));
            object.insert("category".to_string(), json!(map.category));
        }
    }
    Ok(payload)
}

/// `primary_map.primary_map_from_payload`.
pub fn primary_map_from_payload(
    payload: &Value,
    expected_context_fingerprint: Option<&str>,
    expected_generation_context_digest: Option<&str>,
) -> Result<PrimaryMap, HostError> {
    if payload.get("schema").and_then(Value::as_str) != Some(PRIMARY_MAP_SCHEMA) {
        return Err(HostError::rejected(format!(
            "unsupported primary-map schema: {:?}",
            payload.get("schema")
        )));
    }
    let fingerprint = payload
        .get("context_fingerprint")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let digest = payload
        .get("generation_context_digest")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HostError::rejected(
            "primary output map has no valid generation context",
        ));
    }
    if let Some(expected) = expected_context_fingerprint {
        if fingerprint != expected.trim().to_lowercase() {
            return Err(HostError::rejected(
                "primary output map belongs to a different save context",
            ));
        }
    }
    if let Some(expected) = expected_generation_context_digest {
        if digest != expected.trim().to_lowercase() {
            return Err(HostError::rejected(
                "primary output map belongs to a different generation context",
            ));
        }
    }
    let entries = payload
        .get("effects")
        .and_then(Value::as_array)
        .ok_or_else(|| HostError::rejected("primary output map has no effect partition"))?;
    let mut effects = Vec::with_capacity(entries.len());
    for entry in entries {
        let effect_id = entry
            .get("effect_id")
            .and_then(Value::as_str)
            .and_then(|text| u32::from_str_radix(text.trim_start_matches("0x"), 16).ok())
            .ok_or_else(|| HostError::rejected("invalid primary output map effect entry"))?;
        let runs = entry
            .get("runs")
            .and_then(Value::as_array)
            .ok_or_else(|| HostError::rejected("invalid primary output map run list"))?;
        let mut ranges = Vec::with_capacity(runs.len());
        for run in runs {
            let pair = run
                .as_array()
                .filter(|pair| pair.len() == 2)
                .ok_or_else(|| HostError::rejected("invalid primary output map run list"))?;
            let start = pair[0]
                .as_u64()
                .ok_or_else(|| HostError::rejected("invalid run start"))?;
            let end = pair[1]
                .as_u64()
                .ok_or_else(|| HostError::rejected("invalid run end"))?;
            ranges.push((start as u32, end as u32));
        }
        effects.push((effect_id, U16Runs::from_ranges(ranges)));
    }
    let game_version = payload
        .get("game_version")
        .and_then(Value::as_str)
        .ok_or_else(HostError::invalid_request)?
        .to_string();
    let record_type = payload
        .get("record_type")
        .and_then(Value::as_str)
        .and_then(|text| u16::from_str_radix(text.trim_start_matches("0x"), 16).ok())
        .ok_or_else(HostError::invalid_request)?;
    let rarity = payload
        .get("rarity")
        .and_then(Value::as_u64)
        .ok_or_else(HostError::invalid_request)? as u8;
    let draw_index = payload
        .get("draw_index")
        .and_then(Value::as_u64)
        .ok_or_else(HostError::invalid_request)? as u32;
    let mapping = match payload.get("kind").and_then(Value::as_str) {
        Some("grace_conditioned_draw2") => PrimaryMap::Grace(PrimaryOutputMap {
            game_version,
            record_type,
            rarity,
            playthrough: payload
                .get("playthrough")
                .and_then(Value::as_str)
                .ok_or_else(HostError::invalid_request)?
                .to_string(),
            grace_effect_id: payload
                .get("grace_effect_id")
                .and_then(Value::as_str)
                .and_then(|text| u32::from_str_radix(text.trim_start_matches("0x"), 16).ok())
                .ok_or_else(HostError::invalid_request)?,
            grace_effect_slot: payload
                .get("grace_effect_slot")
                .and_then(Value::as_u64)
                .ok_or_else(HostError::invalid_request)? as u8,
            draw_index,
            effects,
        }),
        Some("primary_draw1") => PrimaryMap::FirstDraw(PrimaryFirstDrawOutputMap {
            game_version,
            record_type,
            rarity,
            category: payload
                .get("category")
                .and_then(Value::as_u64)
                .ok_or_else(HostError::invalid_request)? as u8,
            draw_index,
            effects,
        }),
        other => {
            return Err(HostError::rejected(format!(
                "unsupported primary output map kind: {other:?}"
            )))
        }
    };
    validate_complete_partition(&mapping)?;
    Ok(mapping)
}

/// `primary_map.construct_conditioned_probe`.
pub fn construct_conditioned_probe(
    second_u16: u32,
    grace_effect_id: u32,
    mapping: &GraceOutputMap,
) -> Result<u32, HostError> {
    first_u16_ranges_for_grace(grace_effect_id, mapping)?;
    for low16 in 0..=0xFFFFu32 {
        let state2 = (second_u16 << 16) | low16;
        let state1 = lcg_rewind(state2);
        if grace_id_for_first_u16(state1 >> 16, mapping) == Some(grace_effect_id) {
            return Ok(lcg_rewind(state1));
        }
    }
    Err(HostError::rejected(format!(
        "cannot construct a draw-2 probe for bucket {second_u16}"
    )))
}

/// `primary_map.build_primary_output_map`.
#[allow(clippy::too_many_arguments)]
pub fn build_primary_output_map(
    oracle: &mut dyn BatchOracle,
    template: &[u8],
    grace_effect_id: u32,
    mapping: &GraceOutputMap,
    rarity: u8,
    level: u16,
    recommended_level: u16,
    transfer_count: u32,
    cancelled: &mut dyn FnMut() -> bool,
    progress: &mut dyn FnMut(usize),
) -> Result<PrimaryOutputMap, HostError> {
    if template.len() != SCROLL_RECORD_SIZE {
        return Err(HostError::rejected("template must be exactly 0xE8 bytes"));
    }
    let record_type = record_type_of(template);
    if record_type != mapping.record_type {
        return Err(HostError::rejected(format!(
            "主词条映射要求 0x{:04X} 模板，当前为 0x{record_type:04X}",
            mapping.record_type
        )));
    }
    if rarity != mapping.rarity || rarity != 5 || mapping.effect_slot != 6 {
        return Err(HostError::rejected(
            "联立主词条映射目前仅验证稀有度 5 / 第 6 槽恩宠",
        ));
    }
    let category = category_for_record_type(record_type);
    let mut valid = vec!["current-loaded-state".to_string()];
    if matches!(category, Some(3) | Some(4) | Some(5)) {
        if let Some(category) = category {
            valid.push(format!("category-{category}-live-native"));
        }
    }
    if !valid.contains(&mapping.playthrough) {
        return Err(HostError::rejected(
            "联立主词条映射与所选绘卷类型的原生生成上下文不匹配",
        ));
    }
    let grouped = capture_primary_buckets(
        oracle,
        template,
        rarity,
        level,
        recommended_level,
        transfer_count,
        grace_effect_id,
        mapping.effect_slot,
        Some(mapping),
        cancelled,
        progress,
        false,
    )?;
    let effects: Vec<(u32, U16Runs)> = grouped
        .into_iter()
        .map(|(effect_id, buckets)| (effect_id, U16Runs::from_values(buckets)))
        .collect();
    let result = PrimaryOutputMap {
        game_version: PRIMARY_MAP_GAME_VERSION.to_string(),
        record_type,
        rarity,
        playthrough: mapping.playthrough.clone(),
        grace_effect_id,
        grace_effect_slot: mapping.effect_slot,
        draw_index: 2,
        effects,
    };
    if result
        .effects
        .iter()
        .map(|(_, runs)| runs.bucket_count())
        .sum::<u32>()
        != DRAW_BUCKETS as u32
    {
        return Err(HostError::rejected(
            "主词条映射未完整覆盖 65,536 个 draw-2 桶",
        ));
    }
    Ok(result)
}

/// `primary_map.build_primary_first_draw_output_map`.
#[allow(clippy::too_many_arguments)]
pub fn build_primary_first_draw_output_map(
    oracle: &mut dyn BatchOracle,
    template: &[u8],
    category: u8,
    rarity: u8,
    level: u16,
    recommended_level: u16,
    transfer_count: u32,
    cancelled: &mut dyn FnMut() -> bool,
    progress: &mut dyn FnMut(usize),
) -> Result<PrimaryFirstDrawOutputMap, HostError> {
    if template.len() != SCROLL_RECORD_SIZE {
        return Err(HostError::rejected("template must be exactly 0xE8 bytes"));
    }
    if !matches!(category, 1 | 2) {
        return Err(HostError::rejected(
            "first-draw primary mapping is verified only for categories 1 and 2",
        ));
    }
    let record_type = record_type_of(template);
    let expected = CATEGORY_TO_TYPE[usize::from(category)];
    if record_type != expected {
        return Err(HostError::rejected(format!(
            "周目 {category} 主词条映射要求 0x{expected:04X} 模板，当前为 0x{record_type:04X}"
        )));
    }
    let grouped = capture_primary_buckets(
        oracle,
        template,
        rarity,
        level,
        recommended_level,
        transfer_count,
        0,
        0,
        None,
        cancelled,
        progress,
        true,
    )?;
    let result = PrimaryFirstDrawOutputMap {
        game_version: PRIMARY_MAP_GAME_VERSION.to_string(),
        record_type,
        rarity,
        category,
        draw_index: 1,
        effects: grouped
            .into_iter()
            .map(|(effect_id, buckets)| (effect_id, U16Runs::from_values(buckets)))
            .collect(),
    };
    if result
        .effects
        .iter()
        .map(|(_, runs)| runs.bucket_count())
        .sum::<u32>()
        != DRAW_BUCKETS as u32
    {
        return Err(HostError::rejected(
            "主词条映射未完整覆盖 65,536 个 draw-1 桶",
        ));
    }
    Ok(result)
}

/// The shared probe loop of both primary captures.
///
/// One native generation per bucket, exactly as shipped, with the same batch
/// boundary, the same cancellation point and the same contradiction refusals.
#[allow(clippy::too_many_arguments)]
fn capture_primary_buckets(
    oracle: &mut dyn BatchOracle,
    template: &[u8],
    rarity: u8,
    level: u16,
    recommended_level: u16,
    transfer_count: u32,
    grace_effect_id: u32,
    grace_effect_slot: u8,
    grace: Option<&GraceOutputMap>,
    cancelled: &mut dyn FnMut() -> bool,
    progress: &mut dyn FnMut(usize),
    first_draw: bool,
) -> Result<Vec<(u32, Vec<u32>)>, HostError> {
    let batch = oracle.max_batch_size().max(1);
    let mut grouped: Vec<(u32, Vec<u32>)> = Vec::new();
    let mut start = 0usize;
    while start < DRAW_BUCKETS {
        if cancelled() {
            return Err(HostError::rejected("主词条映射已取消"));
        }
        let stop = (start + batch).min(DRAW_BUCKETS);
        let mut seeds = Vec::with_capacity(stop - start);
        let mut sources = Vec::with_capacity(stop - start);
        for bucket in start..stop {
            let seed = if first_draw {
                seed_from_state_after_draw((bucket as u32) << 16, 1)
            } else {
                let mapping = grace.ok_or_else(|| {
                    HostError::rejected("a draw-2 primary map requires the measured Grace map")
                })?;
                construct_conditioned_probe(bucket as u32, grace_effect_id, mapping)?
            };
            let source = nioh3_runtime::mutation::oracle::source_record(
                template,
                seed,
                rarity,
                level,
                recommended_level,
                transfer_count,
            )
            .map_err(HostError::from_runtime)?;
            seeds.push(seed);
            sources.push(source);
        }
        let records = oracle
            .generate(&sources, ORACLE_MAP_TIMEOUT_MS)
            .map_err(HostError::from_runtime)?;
        if records.len() != seeds.len() {
            return Err(HostError::rejected(
                "游戏原生生成器返回了错误数量的主词条映射记录",
            ));
        }
        for (offset, record) in records.iter().enumerate() {
            let bucket = (start + offset) as u32;
            if crate::scan::seed_of(record) != seeds[offset] {
                return Err(HostError::rejected(
                    "游戏原生生成器改变了主词条映射探针 Seed",
                ));
            }
            if !first_draw {
                let actual_grace =
                    crate::scan::slot_effect_id(record, usize::from(grace_effect_slot) - 1)
                        .ok_or_else(HostError::invalid_request)?;
                if actual_grace != grace_effect_id {
                    return Err(HostError::rejected(format!(
                        "主词条映射与恩宠 first-u16 映射矛盾：预期 0x{grace_effect_id:04X}，\
                         实际 0x{actual_grace:04X}"
                    )));
                }
            }
            let primary_id =
                crate::scan::slot_effect_id(record, 0).ok_or_else(HostError::invalid_request)?;
            match grouped.iter_mut().find(|(id, _)| *id == primary_id) {
                Some((_, buckets)) => buckets.push(bucket),
                None => grouped.push((primary_id, vec![bucket])),
            }
        }
        progress(stop);
        start = stop;
    }
    grouped.sort_by_key(|(effect_id, _)| *effect_id);
    Ok(grouped)
}

/// The measured maps one search may reuse, mirroring `prepare_maps`' result.
#[derive(Debug, Clone, Default)]
pub enum PreparedMaps {
    /// Nothing measured for this search.
    #[default]
    None,
    /// `grace_output_map` only: the accelerated Grace path.
    Grace(GraceOutputMap),
    /// `primary_first_output_map`: the draw-1 primary path.
    PrimaryFirst(PrimaryFirstDrawOutputMap),
    /// `grace_output_map` plus `primary_output_map`: the joint rarity-5 path.
    Joint {
        grace: GraceOutputMap,
        primary: PrimaryOutputMap,
    },
}

impl PreparedMaps {
    /// Nothing measured: the shipped `maps = {}` case.
    pub fn none() -> Self {
        PreparedMaps::None
    }

    /// `bool(maps)`, the shipped `accelerate_grace` argument.
    pub fn is_empty(&self) -> bool {
        matches!(self, PreparedMaps::None)
    }

    /// `any(key.startswith('primary_') for key in maps)`.
    pub fn uses_primary(&self) -> bool {
        matches!(
            self,
            PreparedMaps::PrimaryFirst(_) | PreparedMaps::Joint { .. }
        )
    }

    pub fn grace(&self) -> Option<&GraceOutputMap> {
        match self {
            PreparedMaps::Grace(map) => Some(map),
            PreparedMaps::Joint { grace, .. } => Some(grace),
            _ => None,
        }
    }
}

/// `cache_application.primary_map_cache_path`.
pub fn primary_map_cache_path(
    state_root: &Path,
    save_fingerprint: &str,
    playthrough: u8,
    rarity: u8,
    grace_effect_id: Option<u32>,
    generation_context_digest: &str,
) -> PathBuf {
    let digest = generation_context_digest.to_lowercase();
    let short: String = digest.chars().take(16).collect();
    let kind = match grace_effect_id {
        None => "draw1".to_string(),
        Some(effect_id) => format!("grace-{effect_id:08X}-draw2"),
    };
    state_root.join("primary-effect-maps").join(format!(
        "{}-{short}-p{playthrough}-r{rarity}-{kind}.json",
        save_fingerprint.to_lowercase()
    ))
}

/// `native_search_maps.prepare_maps`.
///
/// A cached map is reused only when its save fingerprint and generation
/// context digest both match, so a stale or foreign map is rebuilt from the
/// live oracle rather than trusted.
#[allow(clippy::too_many_arguments)]
pub fn prepare_maps(
    oracle: &mut dyn BatchOracle,
    state_root: &Path,
    template: &[u8],
    save_fingerprint: &str,
    generation_context_digest: &str,
    playthrough: u8,
    rarity: u8,
    level: u16,
    recommended_level: u16,
    transfer_count: u32,
    target_grace: Option<u32>,
    primary_effect_ids: &std::collections::BTreeSet<u32>,
    cancelled: &mut dyn FnMut() -> bool,
    progress: &mut dyn FnMut(&str, Value),
) -> Result<PreparedMaps, HostError> {
    let mut grace_map: Option<GraceOutputMap> = None;
    if target_grace.is_some() {
        let path = crate::save_app::grace_map_cache_path(
            state_root,
            save_fingerprint,
            playthrough,
            rarity,
            generation_context_digest,
        );
        let cached = load_cached_json(&path)?;
        let mapping = match cached {
            Some(payload) => {
                let mapping = nioh3_worker::grace_map::from_cache_payload(
                    &payload,
                    Some(generation_context_digest),
                )
                .map_err(HostError::rejected)?;
                let stored = payload
                    .get("context_fingerprint")
                    .map(|value| match value {
                        Value::String(text) => text.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                if stored.trim().to_lowercase() != save_fingerprint.trim().to_lowercase() {
                    return Err(HostError::rejected(
                        "Grace output map belongs to a different save context",
                    ));
                }
                mapping
            }
            None => {
                let mapping = crate::grace_capture::build_live_grace_output_map(
                    oracle,
                    template,
                    playthrough,
                    rarity,
                    level,
                    recommended_level,
                    transfer_count,
                    cancelled,
                    &mut |tick| {
                        let mut value = tick.to_json();
                        if let Some(object) = value.as_object_mut() {
                            object.insert(
                                "phase".to_string(),
                                Value::String("capture_special_map".to_string()),
                            );
                        }
                        progress("capture_special_map", value);
                    },
                )?;
                let payload = crate::save_app::grace_map_to_cache_payload(
                    &mapping,
                    save_fingerprint,
                    generation_context_digest,
                )?;
                crate::grace_capture::save_grace_map_cache(&path, &payload)?;
                mapping
            }
        };
        grace_map = Some(mapping);
    }

    let first =
        matches!(playthrough, 1 | 2) && !primary_effect_ids.is_empty() && target_grace.is_none();
    let second = !primary_effect_ids.is_empty() && target_grace.is_some() && rarity == 5;
    let mut primary_map: Option<PrimaryMap> = None;
    if first || second {
        let path = primary_map_cache_path(
            state_root,
            save_fingerprint,
            playthrough,
            rarity,
            target_grace,
            generation_context_digest,
        );
        let cached = load_cached_json(&path)?;
        primary_map = Some(match cached {
            Some(payload) => {
                let mapping = primary_map_from_payload(
                    &payload,
                    Some(save_fingerprint),
                    Some(generation_context_digest),
                )?;
                let matches_kind = if first {
                    matches!(mapping, PrimaryMap::FirstDraw(_))
                } else {
                    matches!(mapping, PrimaryMap::Grace(_))
                };
                if !matches_kind {
                    return Err(HostError::rejected(
                        "Primary map kind does not match the search",
                    ));
                }
                mapping
            }
            None => {
                let mapping = if first {
                    PrimaryMap::FirstDraw(build_primary_first_draw_output_map(
                        oracle,
                        template,
                        playthrough,
                        rarity,
                        level,
                        recommended_level,
                        transfer_count,
                        cancelled,
                        &mut |mapped| {
                            progress(
                                "capture_primary_map",
                                json!({"mapped_buckets": mapped, "total_buckets": DRAW_BUCKETS}),
                            )
                        },
                    )?)
                } else {
                    let grace = grace_map.as_ref().ok_or_else(|| {
                        HostError::rejected("a joint primary map requires the measured Grace map")
                    })?;
                    PrimaryMap::Grace(build_primary_output_map(
                        oracle,
                        template,
                        target_grace.ok_or_else(HostError::invalid_request)?,
                        grace,
                        rarity,
                        level,
                        recommended_level,
                        transfer_count,
                        cancelled,
                        &mut |mapped| {
                            progress(
                                "capture_primary_map",
                                json!({"mapped_buckets": mapped, "total_buckets": DRAW_BUCKETS}),
                            )
                        },
                    )?)
                };
                let payload =
                    primary_map_to_payload(&mapping, save_fingerprint, generation_context_digest)?;
                crate::grace_capture::save_grace_map_cache(&path, &payload)?;
                mapping
            }
        });
    }

    Ok(match (grace_map, primary_map) {
        (Some(grace), Some(PrimaryMap::Grace(primary))) => PreparedMaps::Joint { grace, primary },
        (Some(grace), Some(PrimaryMap::FirstDraw(first_map))) => {
            let _ = first_map;
            PreparedMaps::Grace(grace)
        }
        (Some(grace), None) => PreparedMaps::Grace(grace),
        (None, Some(PrimaryMap::FirstDraw(map))) => PreparedMaps::PrimaryFirst(map),
        (None, Some(PrimaryMap::Grace(_))) => PreparedMaps::None,
        (None, None) => PreparedMaps::None,
    })
}

/// Read a cache file, reporting absence as `None` rather than an error.
fn load_cached_json(path: &Path) -> Result<Option<Value>, HostError> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|error| HostError::rejected(error.to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(HostError::rejected(format!("{}: {error}", path.display()))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::oracle::scripted::ScriptedOracle;
    use crate::scan::{EFFECT_START, EFFECT_STRIDE};

    fn grace_map(ranges: Vec<(u32, u32, u32)>) -> GraceOutputMap {
        GraceOutputMap {
            record_type: 0xE604,
            rarity: 5,
            playthrough: "category-3-live-native".to_string(),
            effect_slot: 6,
            ranges: ranges
                .into_iter()
                .map(|(start, end, grace_id)| GraceRange {
                    start,
                    end,
                    grace_id,
                })
                .collect(),
        }
    }

    #[test]
    fn the_seed_math_matches_the_shipped_identities() {
        assert_eq!(
            seed_from_state_after_draw(lcg_step(0x0B2A_1234), 1),
            0x0B2A_1234
        );
        assert!(is_natural_scroll_id(0x0B2A_1234));
        assert!(!is_natural_scroll_id(0x1000_0000));
        assert!(!is_natural_scroll_id(0x0B2A_0000));
        // game_random_int_from_u16 is a two-step binary32 contraction.
        assert_eq!(game_random_int_from_u16(0xFFFF, 10_000), 9_999);
        assert_eq!(game_random_int_from_u16(0x0000, 10_000), 0);
        assert_eq!(game_random_int_from_u16(0x8000, 10_000), 5_000);
    }

    #[test]
    fn u16_runs_merge_and_count_like_the_shipped_helper() {
        let runs = U16Runs::from_values(vec![5, 4, 4, 9, 3]);
        assert_eq!(runs.runs, vec![(3, 5), (9, 9)]);
        assert_eq!(runs.bucket_count(), 4);
        assert!(runs.contains(4));
        assert!(!runs.contains(6));
        let merged = U16Runs::from_ranges(vec![(0, 2), (3, 5)]);
        assert_eq!(merged.runs, vec![(0, 5)]);
    }

    #[test]
    fn the_pivot_permutation_is_a_bijection_with_the_shipped_stride() {
        let runs = U16Runs::from_ranges(vec![(0, 9)]);
        let permuted = permuted_pivot_values(&runs);
        let mut sorted = permuted.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..10).collect::<Vec<u32>>());
        // `stride = 0x9E37 % 10 = 3`, which is odd and coprime with 10, so the
        // permutation is `values[(i * 3) % 10]` exactly as the shipped helper.
        assert_eq!(permuted, vec![0, 3, 6, 9, 2, 5, 8, 1, 4, 7]);
    }

    #[test]
    fn the_constraint_intersection_yields_exact_matches_and_counts_trials() {
        // Draw 2 must land in 7..=9 while draw 1 is unconstrained, so every
        // natural pivot state is a genuine intersection and the states the
        // natural-ID shape rejects stay visible as counted trials.
        let first = DrawConstraint::new("grace", 1, U16Runs::from_ranges(vec![(0, 0xFFFF)]))
            .expect("constraint");
        let second = DrawConstraint::new("primary", 2, U16Runs::from_ranges(vec![(7, 9)]))
            .expect("constraint");
        let mut cursor =
            ConstraintIntersection::new(vec![first.clone(), second.clone()], 0, Some(4096))
                .expect("cursor");
        // The pivot is chosen the shipped way: fewest buckets, then the later
        // draw, then the name.
        assert_eq!(cursor.pivot_name(), "primary");
        let mut found = 0u64;
        let mut last_trial = 0u64;
        while let Some(solution) = cursor.next_solution() {
            assert!(
                first.matches(solution.seed) && second.matches(solution.seed),
                "every solution must satisfy both constraints"
            );
            assert!(is_natural_scroll_id(solution.seed));
            assert!(
                solution.pivot_trial > last_trial,
                "the cursor must advance monotonically"
            );
            last_trial = solution.pivot_trial;
            found += 1;
        }
        assert!(found > 0, "the intersection must be non-empty");
        assert!(last_trial <= 4096);
    }

    #[test]
    fn a_resumed_cursor_continues_rather_than_replaying() {
        let first = DrawConstraint::new("grace", 1, U16Runs::from_ranges(vec![(0, 0)]))
            .expect("constraint");
        let mut all = ConstraintIntersection::new(vec![first.clone()], 0, None).expect("cursor");
        let mut seen = Vec::new();
        for _ in 0..3 {
            if let Some(solution) = all.next_solution() {
                seen.push(solution.pivot_trial);
            }
        }
        let last = *seen.last().expect("at least one solution");
        let mut resumed = ConstraintIntersection::new(vec![first], last, None).expect("cursor");
        let next = resumed.next_solution().expect("resume yields more");
        assert!(next.pivot_trial > last, "the cursor must not replay");
    }

    #[test]
    fn grace_ranges_and_seed_enumeration_follow_the_measured_map() {
        let mapping = grace_map(vec![(0, 99, 0xAAAA), (100, 0xFFFF, 0xBBBB)]);
        assert_eq!(grace_id_for_first_u16(50, &mapping), Some(0xAAAA));
        assert_eq!(grace_id_for_first_u16(5_000, &mapping), Some(0xBBBB));
        let ranges = first_u16_ranges_for_grace(0xBBBB, &mapping).expect("range exists");
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start, 100);
        assert!(
            first_u16_ranges_for_grace(0xCCCC, &mapping).is_err(),
            "an absent result is refused"
        );

        let mut cursor = GraceSeedCursor::new(0xBBBB, &mapping, None).expect("cursor");
        let mut seeds = Vec::new();
        for _ in 0..3 {
            let seed = cursor.next_seed().expect("a seed exists");
            // The seed's own first draw must belong to the requested range.
            let state = lcg_step(seed.seed);
            assert!(grace_id_for_first_u16(state >> 16, &mapping) == Some(0xBBBB));
            seeds.push(seed.seed);
        }
        // Resuming after the first seed must skip it.
        let mut resumed = GraceSeedCursor::new(0xBBBB, &mapping, Some(seeds[0])).expect("cursor");
        let next = resumed.next_seed().expect("a seed exists");
        assert_ne!(next.seed, seeds[0]);
        assert!(
            GraceSeedCursor::new(0xBBBB, &mapping, Some(0x1000_0000)).is_err(),
            "a non-natural resume cursor is refused"
        );
    }

    #[test]
    fn a_primary_map_round_trips_through_its_payload() {
        let mapping = PrimaryMap::FirstDraw(PrimaryFirstDrawOutputMap {
            game_version: PRIMARY_MAP_GAME_VERSION.to_string(),
            record_type: 0x1E82,
            rarity: 5,
            category: 1,
            draw_index: 1,
            effects: vec![
                (0x1111_1111, U16Runs::from_ranges(vec![(0, 0x7FFF)])),
                (0x2222_2222, U16Runs::from_ranges(vec![(0x8000, 0xFFFF)])),
            ],
        });
        let fingerprint = "a".repeat(64);
        let digest = "b".repeat(64);
        let payload = primary_map_to_payload(&mapping, &fingerprint, &digest).expect("payload");
        let restored = primary_map_from_payload(&payload, Some(&fingerprint), Some(&digest))
            .expect("round trip");
        assert_eq!(restored, mapping);
        // The context gates are real.
        assert!(
            primary_map_from_payload(&payload, Some(&"c".repeat(64)), None).is_err(),
            "a foreign save context is refused"
        );
        assert!(
            primary_map_from_payload(&payload, None, Some(&"d".repeat(64))).is_err(),
            "a foreign generation context is refused"
        );
    }

    #[test]
    fn an_incomplete_partition_is_refused() {
        let mapping = PrimaryMap::FirstDraw(PrimaryFirstDrawOutputMap {
            game_version: PRIMARY_MAP_GAME_VERSION.to_string(),
            record_type: 0x1E82,
            rarity: 5,
            category: 1,
            draw_index: 1,
            effects: vec![(0x1111_1111, U16Runs::from_ranges(vec![(0, 0x7FFF)]))],
        });
        let failure = primary_map_to_payload(&mapping, &"a".repeat(64), &"b".repeat(64))
            .expect_err("a partial partition must be refused");
        assert!(
            failure.message.contains("do not cover") || failure.message.contains("does not cover")
        );
    }

    #[test]
    fn the_first_draw_capture_maps_every_bucket_to_its_primary() {
        let mut template = vec![0u8; SCROLL_RECORD_SIZE];
        template[0..2].copy_from_slice(&0x1E82u16.to_le_bytes());
        // The scripted oracle answers an unscripted seed with the pure source
        // record, whose slot-1 id is empty, so every bucket lands in one entry
        // that still has to cover all 65,536 buckets.
        let mut oracle = ScriptedOracle::new(vec![], template.clone(), 5, 180, 183, 0);
        oracle.set_max_batch_size(0x8000);
        let mut ticks = Vec::new();
        let mapping = build_primary_first_draw_output_map(
            &mut oracle,
            &template,
            1,
            5,
            180,
            183,
            0,
            &mut || false,
            &mut |mapped| ticks.push(mapped),
        )
        .expect("capture succeeds");
        assert_eq!(ticks, vec![0x8000, 0x10000]);
        assert_eq!(
            mapping
                .effects
                .iter()
                .map(|(_, runs)| runs.bucket_count())
                .sum::<u32>(),
            DRAW_BUCKETS as u32
        );
        assert_eq!(oracle.calls.len(), 2);
    }

    #[test]
    fn a_category_mismatch_refuses_the_first_draw_capture() {
        let mut template = vec![0u8; SCROLL_RECORD_SIZE];
        template[0..2].copy_from_slice(&0x1E82u16.to_le_bytes());
        let mut oracle = ScriptedOracle::new(vec![], template.clone(), 5, 180, 183, 0);
        let failure = build_primary_first_draw_output_map(
            &mut oracle,
            &template,
            2,
            5,
            180,
            183,
            0,
            &mut || false,
            &mut |_| {},
        )
        .expect_err("category 2 needs a 0x516D template");
        assert!(failure.message.contains("0x516D"));
        assert!(oracle.calls.is_empty(), "no probe runs before the gate");
    }

    #[test]
    fn the_primary_map_path_names_draw1_or_the_grace_draw2() {
        let root = Path::new("state");
        let draw1 = primary_map_cache_path(root, "AB", 1, 5, None, "CDEF");
        assert!(draw1.ends_with("ab-cdef-p1-r5-draw1.json"), "{draw1:?}");
        let draw2 = primary_map_cache_path(root, "AB", 3, 5, Some(0x1234ABCD), "CDEF");
        assert!(
            draw2.ends_with("ab-cdef-p3-r5-grace-1234ABCD-draw2.json"),
            "{draw2:?}"
        );
    }

    #[test]
    fn prepare_maps_builds_and_then_reuses_one_measured_grace_map() {
        let template = {
            let mut template = vec![0u8; SCROLL_RECORD_SIZE];
            template[0..2].copy_from_slice(&0xE604u16.to_le_bytes());
            template
        };
        let mut oracle = ScriptedOracle::new(vec![], template.clone(), 4, 180, 183, 0);
        oracle.set_max_batch_size(0x10000);
        let directory =
            std::env::temp_dir().join(format!("nioh3-prepare-maps-{}", std::process::id()));
        let fingerprint = "a".repeat(64);
        let digest = "b".repeat(64);
        let mut phases = Vec::new();
        let prepared = prepare_maps(
            &mut oracle,
            &directory,
            &template,
            &fingerprint,
            &digest,
            3,
            4,
            180,
            183,
            0,
            Some(0x9999),
            &std::collections::BTreeSet::new(),
            &mut || false,
            &mut |phase, _| phases.push(phase.to_string()),
        )
        .expect("maps prepare");
        assert!(prepared.grace().is_some());
        assert_eq!(phases, vec!["capture_special_map"]);
        let calls_after_build = oracle.calls.len();
        // A second preparation must reuse the cache instead of measuring again.
        let reused = prepare_maps(
            &mut oracle,
            &directory,
            &template,
            &fingerprint,
            &digest,
            3,
            4,
            180,
            183,
            0,
            Some(0x9999),
            &std::collections::BTreeSet::new(),
            &mut || false,
            &mut |_, _| {},
        )
        .expect("maps prepare again");
        assert!(reused.grace().is_some());
        assert_eq!(
            oracle.calls.len(),
            calls_after_build,
            "a cached map must not re-measure"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    // The recorded slot reader must be the same one the scan uses.
    #[test]
    fn the_probe_reader_is_the_shared_one() {
        let mut record = vec![0u8; SCROLL_RECORD_SIZE];
        let start = EFFECT_START + 4 * EFFECT_STRIDE + 4;
        record[start..start + 4].copy_from_slice(&0x7777u32.to_le_bytes());
        assert_eq!(crate::scan::slot_effect_id(&record, 4), Some(0x7777));
    }
}
