//! Port of `grace_map.build_live_grace_output_map`.
//!
//! A live special-result map is the inverse of the first draw: for each of the
//! 65,536 high-16 states the game's own generator answers one effect id, and the
//! consecutive equal answers collapse into ranges. Every probe is a real native
//! generation, so the map is measured from the game rather than derived from a
//! model, and the caller's cancellation is checked before every batch.
//!
//! Rarity 4 maps the stage-one slot-5 Grace and rarity 5 maps the final slot-6
//! Grace; the map is context-specific, so a category-2 rarity-4 map can never be
//! substituted for the measured category-3 map even when the effect-id sets
//! overlap. That rule is enforced by [`require_grace_acceleration_context`] on
//! the consuming side and by the record-type gate here.

use serde_json::{json, Value};

use nioh3_worker::grace_map::{GraceOutputMap, GraceRange};

use crate::error::HostError;
use crate::oracle::{BatchOracle, ORACLE_MAP_TIMEOUT_MS};
use crate::scan::{record_type_of, CATEGORY_TO_TYPE, SCROLL_RECORD_SIZE};

/// The parent LCG's multiplier (`nioh3_seed_math.LCG_MULTIPLIER`).
pub const LCG_MULTIPLIER: u32 = 0x0001_0DCD;
/// Its modular inverse, so a state can be rewound to its producing scroll id.
pub const LCG_MULTIPLIER_INVERSE: u32 = 0xA5E2_A705;
/// High-16 draw buckets a complete map covers.
pub const GRACE_MAP_BUCKETS: usize = 0x10000;

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

/// `grace_map.GraceMapProgress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraceMapProgress {
    pub mapped_buckets: usize,
    pub total_buckets: usize,
}

impl GraceMapProgress {
    pub fn to_json(&self) -> Value {
        json!({
            "mapped_buckets": self.mapped_buckets,
            "total_buckets": self.total_buckets,
        })
    }
}

/// `grace_map.save_grace_map_cache`: persist a measured map for later
/// game-closed reuse.
///
/// The shipped writer serializes with `sort_keys=True, indent=2`, writes a
/// per-process temporary beside the target and then replaces it, so a reader
/// either sees the previous map or the new one and never a partial file.
pub fn save_grace_map_cache(path: &std::path::Path, payload: &Value) -> Result<(), HostError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| HostError::rejected(format!("{}: {error}", parent.display())))?;
    }
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|error| HostError::rejected(error.to_string()))?
    );
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("grace-map.json");
    let temporary = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&temporary, text)
        .map_err(|error| HostError::rejected(format!("{}: {error}", temporary.display())))?;
    let replaced = std::fs::rename(&temporary, path);
    if replaced.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    replaced.map_err(|error| HostError::rejected(format!("{}: {error}", path.display())))
}

/// `grace_map.build_live_grace_output_map`.
///
/// The returned map's ranges partition every bucket exactly, which is what the
/// consuming scanner relies on when it enumerates natural seeds for one Grace.
#[allow(clippy::too_many_arguments)]
pub fn build_live_grace_output_map(
    oracle: &mut dyn BatchOracle,
    template: &[u8],
    category: u8,
    rarity: u8,
    level: u16,
    recommended_level: u16,
    transfer_count: u32,
    cancelled: &mut dyn FnMut() -> bool,
    progress: &mut dyn FnMut(GraceMapProgress),
) -> Result<GraceOutputMap, HostError> {
    if template.len() != SCROLL_RECORD_SIZE {
        return Err(HostError::rejected("template must be exactly 0xE8 bytes"));
    }
    if !(1..=5).contains(&category) {
        return Err(HostError::rejected(
            "live special-result mapping supports categories 1 through 5",
        ));
    }
    if !matches!(rarity, 4 | 5) {
        return Err(HostError::rejected(
            "live special-result mapping supports rarity 4 or 5",
        ));
    }
    if rarity == 5 && matches!(category, 1 | 2) {
        return Err(HostError::rejected(
            "rarity-5 Grace mapping is not enabled for categories 1 or 2",
        ));
    }
    let record_type = record_type_of(template);
    let expected_type = CATEGORY_TO_TYPE[usize::from(category)];
    if record_type != expected_type {
        return Err(HostError::rejected(format!(
            "category {category} requires record type 0x{expected_type:04X}, \
             got 0x{record_type:04X}"
        )));
    }
    let effect_slot = if rarity == 4 { 5usize } else { 6usize };

    let batch = oracle.max_batch_size().max(1);
    let mut outputs: Vec<u32> = Vec::with_capacity(GRACE_MAP_BUCKETS);
    let mut start = 0usize;
    while start < GRACE_MAP_BUCKETS {
        if cancelled() {
            return Err(HostError::rejected("特殊结果映射已取消"));
        }
        let stop = (start + batch).min(GRACE_MAP_BUCKETS);
        let mut seeds = Vec::with_capacity(stop - start);
        let mut sources = Vec::with_capacity(stop - start);
        for bucket in start..stop {
            let seed = seed_from_state_after_draw((bucket as u32) << 16, 1);
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
                "游戏原生生成器返回了错误数量的特殊结果映射记录",
            ));
        }
        for (seed, record) in seeds.iter().zip(records.iter()) {
            if crate::scan::seed_of(record) != *seed {
                return Err(HostError::rejected(
                    "游戏原生生成器改变了特殊结果映射探针 Seed",
                ));
            }
            if record_type_of(record) != record_type {
                return Err(HostError::rejected(
                    "游戏原生生成器改变了特殊结果映射记录类型",
                ));
            }
            outputs.push(
                crate::scan::slot_effect_id(record, effect_slot - 1)
                    .ok_or_else(HostError::invalid_request)?,
            );
        }
        progress(GraceMapProgress {
            mapped_buckets: stop,
            total_buckets: GRACE_MAP_BUCKETS,
        });
        start = stop;
    }

    let mut ranges: Vec<GraceRange> = Vec::new();
    let mut range_start: u32 = 0;
    let mut current = *outputs
        .first()
        .ok_or_else(|| HostError::rejected("Grace output map produced no buckets"))?;
    for (bucket, effect_id) in outputs.iter().enumerate().skip(1) {
        if *effect_id == current {
            continue;
        }
        ranges.push(GraceRange {
            start: range_start,
            end: (bucket as u32) - 1,
            grace_id: current,
        });
        range_start = bucket as u32;
        current = *effect_id;
    }
    ranges.push(GraceRange {
        start: range_start,
        end: 0xFFFF,
        grace_id: current,
    });
    Ok(GraceOutputMap {
        record_type,
        rarity,
        playthrough: format!("category-{category}-live-native"),
        effect_slot: effect_slot as u8,
        ranges,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::oracle::scripted::ScriptedOracle;
    use crate::scan::{EFFECT_START, EFFECT_STRIDE};

    fn template(category: u8) -> Vec<u8> {
        let mut template = vec![0u8; SCROLL_RECORD_SIZE];
        template[0..2].copy_from_slice(&CATEGORY_TO_TYPE[usize::from(category)].to_le_bytes());
        template
    }

    #[test]
    fn the_seed_helpers_invert_each_other() {
        let seed = 0x0B2A_1234u32;
        assert_eq!(seed_from_state_after_draw(lcg_step(seed), 1), seed);
        assert_eq!(
            seed_from_state_after_draw(lcg_step(lcg_step(seed)), 2),
            seed
        );
    }

    #[test]
    fn a_complete_map_partitions_every_bucket() {
        // A tiny batch keeps the scripted run short, but the shipped default is
        // the oracle's own batch size; the map must still cover all 65,536
        // buckets and partition them exactly.
        let mut oracle = ScriptedOracle::new(vec![], template(3), 4, 180, 183, 0);
        oracle.set_max_batch_size(0x4000);
        let mut ticks = Vec::new();
        let mapping = build_live_grace_output_map(
            &mut oracle,
            &template(3),
            3,
            4,
            180,
            183,
            0,
            &mut || false,
            &mut |progress| ticks.push(progress),
        )
        .expect("map builds");
        assert_eq!(mapping.record_type, 0xE604);
        assert_eq!(mapping.rarity, 4);
        assert_eq!(mapping.effect_slot, 5);
        assert_eq!(mapping.playthrough, "category-3-live-native");
        let mut expected_start = 0u32;
        for range in &mapping.ranges {
            assert_eq!(range.start, expected_start);
            assert!(range.end >= range.start);
            expected_start = range.end + 1;
        }
        assert_eq!(
            expected_start, 0x1_0000,
            "the ranges must reach the last bucket"
        );
        assert_eq!(ticks.len(), 4, "one progress tick per batch");
        assert_eq!(ticks.last().map(|tick| tick.mapped_buckets), Some(0x10000));
        // Every probe is one explicit source record.
        assert_eq!(oracle.calls.len(), 4);
    }

    #[test]
    fn a_category_and_template_mismatch_is_refused() {
        let mut oracle = ScriptedOracle::new(vec![], template(2), 4, 180, 183, 0);
        let failure = build_live_grace_output_map(
            &mut oracle,
            &template(2),
            3,
            4,
            180,
            183,
            0,
            &mut || false,
            &mut |_| {},
        )
        .expect_err("category 3 needs an E604 template");
        assert!(failure.message.contains("requires record type"));
        assert!(oracle.calls.is_empty(), "no probe may run before the gate");

        let mut oracle = ScriptedOracle::new(vec![], template(3), 4, 180, 183, 0);
        let failure = build_live_grace_output_map(
            &mut oracle,
            &template(3),
            1,
            5,
            180,
            183,
            0,
            &mut || false,
            &mut |_| {},
        )
        .expect_err("rarity-5 mapping is not enabled for category 1");
        assert!(failure
            .message
            .contains("not enabled for categories 1 or 2"));
    }

    #[test]
    fn a_cancelled_capture_stops_before_the_next_batch() {
        let mut oracle = ScriptedOracle::new(vec![], template(3), 4, 180, 183, 0);
        oracle.set_max_batch_size(0x4000);
        let seen = std::cell::Cell::new(0u32);
        let failure = build_live_grace_output_map(
            &mut oracle,
            &template(3),
            3,
            4,
            180,
            183,
            0,
            &mut || {
                let count = seen.get();
                seen.set(count + 1);
                count >= 1
            },
            &mut |_| {},
        )
        .expect_err("cancellation refuses the capture");
        assert_eq!(failure.message, "特殊结果映射已取消");
        assert_eq!(oracle.calls.len(), 1);
    }

    // The slot reader the builder uses must agree with the scan's own reader.
    #[test]
    fn the_slot_reader_is_the_shared_one() {
        let mut record = template(3);
        let start = EFFECT_START + 4 * EFFECT_STRIDE;
        record[start + 4..start + 8].copy_from_slice(&0x4242u32.to_le_bytes());
        assert_eq!(crate::scan::slot_effect_id(&record, 4), Some(0x4242));
    }
}
