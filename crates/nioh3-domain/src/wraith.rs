//! Possessed (descriptor `+0x0F`) port of
//! `nioh3_scroll_editor/possessed_generation.py` for PC v2.01.
//!
//! Only the seed-derived parent-LCG phase is ported: `position_parent_stream`
//! plus the `generate_possessed` source lottery. Spawn coordinates and the local
//! MT permutation are not needed to derive the parent state and stay out of
//! scope. A missing *captured* row is unknown, which is deliberately distinct
//! from a proven native null lookup.

use std::collections::BTreeMap;

use crate::enemy::{
    Eligibility, EnemyError, EnemyStateTables, MissionVariant, Occurrence, Possession,
    RosterResult, SourceTrial, WraithResult, ENEMY_TEXT_SHA256,
};
use crate::rng::{lottery_10000, threshold_from_config, LcgStream};

/// Byte offsets inside a 0x18-byte position row.
const POSITION_MASK: usize = 0x10;
const POSITION_SYNC: usize = 0x13;
const POSITION_FIRST_WAVE_FLAG: usize = 0x14;
/// First-wave rows whose bit 0 is set are skipped by the role pools.
const POSITION_FIRST_WAVE_BIT: u8 = 0x01;
/// Roles the native code builds a pool for on every wave.
const ROLE_COUNT: u8 = 6;

/// Native lookup eligibility for one roster lookup key.
///
/// A captured row that proves the native enemy lookup returns null is eligible.
/// A key that is simply absent from the capture is only eligible when the
/// complete enemy index is on hand; otherwise it stays unknown and the caller
/// must fail closed instead of reporting a proven `no`.
pub fn eligible(tables: &EnemyStateTables, lookup: u32) -> Result<bool, EnemyError> {
    match tables.eligibility.get(&lookup) {
        Some(Eligibility::EnemyAbsent) => Ok(true),
        Some(Eligibility::SubtypeAbsent) => Ok(false),
        Some(Eligibility::SubtypeFlags(flags)) => Ok(flags & 1 == 0),
        Some(Eligibility::Unknown) => Err(EnemyError::MissingData(format!(
            "subtype gate unknown: 0x{lookup:X}"
        ))),
        None if tables.enemy_index_complete => Ok(true),
        None => Err(EnemyError::MissingData(format!(
            "enemy/subtype lookup not captured: 0x{lookup:X}"
        ))),
    }
}

/// Possessed state for every occurrence of `roster`, in flattened order.
///
/// The parent LCG phase is replayed from `RosterResult::state_after_roster`, so
/// the caller never supplies a captured RNG state. Any missing or unknown input
/// yields `exact == false` with every occurrence `Unknown` and no partial
/// fields, because a preflight failure is not a proven `no`.
pub fn generate_wraith(roster: &RosterResult, tables: &EnemyStateTables) -> WraithResult {
    let occurrence_count = roster.occurrences().count();
    let unknown = |message: String| WraithResult {
        exact: false,
        states: vec![Possession::Unknown; occurrence_count],
        source_entry_state: None,
        source_entry_draw: None,
        final_state: None,
        final_draws: None,
        trials: Vec::new(),
        missing: vec![message],
    };

    let mut stream = match position_parent_stream(roster, tables) {
        Ok(stream) => stream,
        Err(error) => return unknown(error.to_string()),
    };
    let entry_state = stream.state();
    let entry_draw = stream.draws();

    // Preflight every gate before any source draw, so no partial failure can be
    // represented as a proven `no`.
    let mut eligibility: BTreeMap<u32, bool> = BTreeMap::new();
    for occurrence in roster.occurrences() {
        match eligible(tables, occurrence.lookup_key) {
            Ok(value) => {
                eligibility.insert(occurrence.lookup_key, value);
            }
            Err(error) => return unknown(error.to_string()),
        }
    }
    let threshold =
        match threshold_from_config(tables.config_4543.as_ref().map(|row| row.as_slice())) {
            Ok(value) => value,
            Err(error) => return unknown(format!("configuration row rejected: {error:?}")),
        };

    let occurrences: Vec<&Occurrence> = roster.occurrences().collect();
    let mut states = vec![Possession::No; occurrences.len()];
    let mut trials: Vec<SourceTrial> = Vec::new();
    for selector in selector_classes(roster.variant) {
        for (index, occurrence) in occurrences.iter().enumerate() {
            if occurrence.selector_class != *selector {
                continue;
            }
            if !eligibility
                .get(&occurrence.lookup_key)
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            let ticket = lottery_10000(stream.u16());
            let accepted = ticket <= threshold;
            trials.push(SourceTrial {
                selector: *selector,
                spawn: occurrence.native_spawn_key,
                ticket,
                state: stream.state(),
                draw: stream.draws(),
                accepted,
            });
            if accepted {
                states[index] = Possession::Yes;
                return exact_result(states, entry_state, entry_draw, &stream, trials);
            }
        }
    }
    exact_result(states, entry_state, entry_draw, &stream, trials)
}

/// Assemble the exact result once the source phase has finished.
///
/// Only the draws the source-first-success phase actually consumed are reported,
/// so a skipped occurrence contributes no trial.
fn exact_result(
    states: Vec<Possession>,
    entry_state: u32,
    entry_draw: u64,
    stream: &LcgStream,
    trials: Vec<SourceTrial>,
) -> WraithResult {
    WraithResult {
        exact: true,
        states,
        source_entry_state: Some(entry_state),
        source_entry_draw: Some(entry_draw),
        final_state: Some(stream.state()),
        final_draws: Some(stream.draws()),
        trials,
        missing: Vec::new(),
    }
}

/// Parent stream advanced by the location phase, ready for the source lottery.
///
/// The native location phase builds all six role pools for *every* wave, so a
/// pool the final roster never uses still consumes one parent draw when it holds
/// more than one row. First-wave restricted rows are skipped.
fn position_parent_stream(
    roster: &RosterResult,
    tables: &EnemyStateTables,
) -> Result<LcgStream, EnemyError> {
    if !tables.text_sha256.eq_ignore_ascii_case(ENEMY_TEXT_SHA256) {
        return Err(EnemyError::Unsupported(
            "unsupported executable/table identity".to_string(),
        ));
    }
    let rows = tables
        .positions_by_terrain
        .get(&roster.terrain)
        .ok_or_else(|| {
            EnemyError::MissingData(format!(
                "complete manager+A90 slice missing: terrain 0x{:X}",
                roster.terrain
            ))
        })?;

    let mut stream = LcgStream::with_progress(roster.state_after_roster, roster.parent_draws);
    for wave in 0..roster.waves.len() {
        for role in 0..ROLE_COUNT {
            let size = rows
                .iter()
                .filter(|row| role_pool_includes(row, wave, role))
                .count();
            if size > 1 {
                stream.u16();
            }
        }
    }
    Ok(stream)
}

/// Whether one position row belongs to the pool of `role` on `wave`.
fn role_pool_includes(row: &[u8; 24], wave: usize, role: u8) -> bool {
    if row[POSITION_SYNC] < 1 {
        return false;
    }
    if wave == 0 && row[POSITION_FIRST_WAVE_FLAG] & POSITION_FIRST_WAVE_BIT != 0 {
        return false;
    }
    let mask = u16::from_le_bytes([row[POSITION_MASK], row[POSITION_MASK + 1]]);
    (mask >> role) & 1 != 0
}

/// Source selector classes in native evaluation order.
fn selector_classes(variant: MissionVariant) -> &'static [u8] {
    match variant {
        MissionVariant::Expedition => &[0, 1],
        MissionVariant::Solo => &[0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{state_after, A_INV};

    const TERRAIN: u8 = 0x8E;
    /// Ticket the boundary test accepts: `lottery_10000(29039) == 4431`.
    const THRESHOLD: i32 = 4431;
    const ACCEPTING_HIGH16: u16 = 29039;

    fn position_row(mask: u16, sync: u8, flags: u8) -> [u8; 24] {
        let mut row = [0u8; 24];
        row[POSITION_MASK..POSITION_MASK + 2].copy_from_slice(&mask.to_le_bytes());
        row[POSITION_SYNC] = sync;
        row[POSITION_FIRST_WAVE_FLAG] = flags;
        row
    }

    fn config_row(base: i32, scale: f32) -> [u8; 32] {
        let mut row = [0u8; 32];
        row[0x10..0x14].copy_from_slice(&base.to_le_bytes());
        row[0x18..0x1C].copy_from_slice(&scale.to_le_bytes());
        row
    }

    fn tables(positions: Vec<[u8; 24]>, config: Option<[u8; 32]>) -> EnemyStateTables {
        let mut positions_by_terrain = BTreeMap::new();
        positions_by_terrain.insert(TERRAIN, positions);
        EnemyStateTables {
            text_sha256: ENEMY_TEXT_SHA256.to_string(),
            positions_by_terrain,
            eligibility: BTreeMap::new(),
            enemy_index_complete: false,
            config_4543: config,
        }
    }

    fn occurrence(wave: usize, position: usize, lookup: u32) -> Occurrence {
        Occurrence {
            wave_index: wave,
            position,
            native_spawn_key: 0xF40 + position as u32,
            lookup_key: lookup,
            role: 1,
            source_row_index: 0,
            selector_class: 0,
            scratch_rule_key: 0xFFFF,
        }
    }

    fn roster(
        variant: MissionVariant,
        waves: Vec<Vec<Occurrence>>,
        state_after_roster: u32,
        parent_draws: u64,
    ) -> RosterResult {
        RosterResult {
            seed: 1,
            playthrough: 3,
            variant,
            auxiliary_mode: 0x20,
            terrain: TERRAIN,
            branch_class: 1,
            waves,
            state_after_roster,
            parent_draws,
        }
    }

    /// Parent state whose next draw exposes exactly `high16`.
    fn pre_state_for_high16(high16: u16) -> u32 {
        let target = u32::from(high16) << 16;
        A_INV.wrapping_mul(target.wrapping_sub(1))
    }

    /// Lowest ticket `high16` that reaches at least `ticket`.
    fn first_high16_with_ticket(ticket: i32) -> u16 {
        (0..=u16::MAX)
            .find(|high16| lottery_10000(*high16) >= ticket)
            .expect("ticket is reachable inside the uint16 domain")
    }

    #[test]
    fn missing_capture_is_not_a_native_null_lookup() {
        let mut state = tables(vec![position_row(0b1, 1, 0)], Some(config_row(0, 1.0)));
        state.eligibility.insert(0x1111, Eligibility::EnemyAbsent);
        state.eligibility.insert(0x2222, Eligibility::SubtypeAbsent);
        state
            .eligibility
            .insert(0x3333, Eligibility::SubtypeFlags(0));
        state
            .eligibility
            .insert(0x4444, Eligibility::SubtypeFlags(1));
        state.eligibility.insert(0x5555, Eligibility::Unknown);

        // Native null enemy row: the lookup is proven absent, which is eligible.
        assert!(eligible(&state, 0x1111).unwrap());
        assert!(!eligible(&state, 0x2222).unwrap());
        assert!(eligible(&state, 0x3333).unwrap());
        assert!(!eligible(&state, 0x4444).unwrap());

        // Uncapped capture and an explicit unknown both fail closed.
        assert!(matches!(
            eligible(&state, 0x5555),
            Err(EnemyError::MissingData(_))
        ));
        assert!(matches!(
            eligible(&state, 0x9999),
            Err(EnemyError::MissingData(_))
        ));

        // A complete enemy index proves the native lookup returns null.
        state.enemy_index_complete = true;
        assert!(eligible(&state, 0x9999).unwrap());
        assert!(matches!(
            eligible(&state, 0x5555),
            Err(EnemyError::MissingData(_))
        ));
    }

    #[test]
    fn unknown_eligibility_makes_every_occurrence_unknown_without_partial_fields() {
        let mut state = tables(
            vec![position_row(0b1, 1, 0)],
            Some(config_row(THRESHOLD, 1.0)),
        );
        state
            .eligibility
            .insert(0x1111, Eligibility::SubtypeFlags(0));
        let roster = roster(
            MissionVariant::Solo,
            vec![vec![occurrence(0, 0, 0x1111), occurrence(0, 1, 0x9999)]],
            0x1234_5678,
            4,
        );

        let result = generate_wraith(&roster, &state);
        assert!(!result.exact);
        assert_eq!(
            result.states,
            vec![Possession::Unknown, Possession::Unknown]
        );
        assert!(result.source_entry_state.is_none());
        assert!(result.source_entry_draw.is_none());
        assert!(result.final_state.is_none());
        assert!(result.final_draws.is_none());
        assert!(result.trials.is_empty());
        assert_eq!(result.missing.len(), 1);
        assert!(result.missing[0].contains("0x9999"));
    }

    #[test]
    fn every_role_pool_of_every_wave_counts_and_draws_at_most_once() {
        // Role 0 holds two general rows; role 3 holds one general row plus a
        // first-wave-restricted row; role 1 holds one syncable row plus a row the
        // native pool filter rejects.
        let positions = vec![
            position_row(0b001001, 1, 0),
            position_row(0b000001, 1, 0),
            position_row(0b001000, 1, 1),
            position_row(0b000010, 1, 0),
            position_row(0b000010, 0, 0),
        ];
        let mut state = tables(positions, Some(config_row(THRESHOLD, 1.0)));
        state.eligibility.insert(0x1111, Eligibility::SubtypeAbsent);
        let roster = roster(
            MissionVariant::Solo,
            vec![
                vec![occurrence(0, 0, 0x1111)],
                vec![occurrence(1, 0, 0x1111)],
            ],
            0xDEAD_BEEF,
            10,
        );

        let result = generate_wraith(&roster, &state);
        assert!(result.exact);
        assert_eq!(result.states, vec![Possession::No, Possession::No]);
        // Ineligible occurrences are skipped without consuming a source draw.
        assert!(result.trials.is_empty());
        // Wave 0: role 0 (two unrestricted rows) only.
        // Wave 1: role 0 and role 3 (now two rows) only.
        assert_eq!(result.source_entry_draw, Some(13));
        assert_eq!(result.final_draws, Some(13));
        assert_eq!(result.source_entry_state, Some(state_after(0xDEAD_BEEF, 3)));
    }

    #[test]
    fn source_lottery_accepts_exactly_at_the_threshold() {
        let positions = vec![position_row(0b1, 1, 0)];
        let mut state = tables(positions, Some(config_row(THRESHOLD, 1.0)));
        state
            .eligibility
            .insert(0x1111, Eligibility::SubtypeFlags(0));

        let accepted = generate_wraith(
            &roster(
                MissionVariant::Solo,
                vec![vec![occurrence(0, 0, 0x1111)]],
                pre_state_for_high16(ACCEPTING_HIGH16),
                0,
            ),
            &state,
        );
        assert!(accepted.exact);
        assert_eq!(accepted.states, vec![Possession::Yes]);
        assert_eq!(accepted.source_entry_draw, Some(0));
        assert_eq!(accepted.final_draws, Some(1));
        assert_eq!(accepted.trials.len(), 1);
        assert_eq!(accepted.trials[0].ticket, THRESHOLD);
        assert!(accepted.trials[0].accepted);
        assert_eq!(accepted.trials[0].draw, 1);

        // One ticket above the threshold is a proven no, not an unknown.
        let rejecting_high16 = first_high16_with_ticket(THRESHOLD + 1);
        let rejected = generate_wraith(
            &roster(
                MissionVariant::Solo,
                vec![vec![occurrence(0, 0, 0x1111)]],
                pre_state_for_high16(rejecting_high16),
                0,
            ),
            &state,
        );
        assert!(rejected.exact);
        assert_eq!(rejected.states, vec![Possession::No]);
        assert_eq!(rejected.trials.len(), 1);
        assert_eq!(rejected.trials[0].ticket, THRESHOLD + 1);
        assert!(!rejected.trials[0].accepted);
    }

    #[test]
    fn expedition_visits_class_zero_before_class_one() {
        let mut state = tables(vec![position_row(0b1, 1, 0)], Some(config_row(0, 1.0)));
        state
            .eligibility
            .insert(0x1111, Eligibility::SubtypeFlags(0));
        state
            .eligibility
            .insert(0x2222, Eligibility::SubtypeFlags(0));
        let mut class_one = occurrence(0, 1, 0x2222);
        class_one.selector_class = 1;
        let roster = roster(
            MissionVariant::Expedition,
            vec![vec![occurrence(0, 0, 0x1111), class_one]],
            0x0F0F_0F0F,
            0,
        );

        // Threshold 0 rejects every ticket, so both classes are visited in order.
        let result = generate_wraith(&roster, &state);
        assert_eq!(result.states, vec![Possession::No, Possession::No]);
        assert_eq!(
            result
                .trials
                .iter()
                .map(|t| t.selector)
                .collect::<Vec<u8>>(),
            vec![0, 1]
        );
        assert_eq!(result.final_draws, Some(2));
    }
}
