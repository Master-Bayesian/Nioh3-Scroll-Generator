//! Offline port of the PC v2.01 enemy roster stage (RVA 0x1029E80) and its
//! 0x1027A10/0x1027FE0 helpers.
//!
//! This is a direct port of `nioh3_scroll_editor/enemy_variant_generation.py`
//! (`generate_roster`, `_pool`, `append_group_extras`, `budget_and_extras`)
//! together with the gate and ticket helpers documented in
//! `nioh3_scroll_editor/auxiliary_generation.py` (`_enemy_cost`,
//! `_enemy_lookup_key`, `_enemy_parameter_gate_accepts`,
//! `_enemy_terrain_gate_accepts`, `_select_enemy_by_u16_ticket`).
//!
//! Contract notes:
//!
//! - The parent LCG and the per-wave local streams stay separate: each budget
//!   helper consumes exactly one parent draw and forks an independent local
//!   stream, so the extra-group MT generator never advances the parent. The
//!   parent state and parent draw count are reported on the result.
//! - Candidate order and duplicate entries are preserved. Branch 1/2 pool
//!   shaping rewrites role 0/2 entries in place, so one row can appear in the
//!   local pool several times, and branch 0 budget selection can repeat a row.
//! - Extras are excluded by row index, not by enemy identity, and a native
//!   zero group is a singleton that consumes no draw.
//! - A wave whose budget cannot be reduced by the selected native cost fails
//!   closed instead of looping, and the native spawn cap bounds budget
//!   selection because every selection becomes an occurrence.
//! - Table completeness is enforced: `contexts`, `enemies` and
//!   `parameter_types` must be non-empty (the reference models absence as
//!   `None`, which has no Rust equivalent here), `terrain_keys` must match the
//!   terrain row count, and every candidate cost plus all five wave budgets
//!   must be finite with positive costs.
//! - Not an actor, persistent-task, or Curse oracle: this returns spawn keys,
//!   roles and RNG bookkeeping only. Coordinates and source-flag assignment
//!   stay in their own ports.
//! - No I/O, JSON, tracing, random-number crate, or other third-party
//!   dependency is involved, and nothing here is wired into product entry
//!   points.

use std::collections::BTreeMap;

use crate::enemy::{
    EnemyError, MissionVariant, Occurrence, RosterInput, RosterResult, RosterTables,
};
use crate::rng::{f32_of, lottery_10000, native_shuffle, LcgStream, Mt19937};

/// Width of one enemy-candidate row.
const ENEMY_ROW_BYTES: usize = 28;
/// Width of one roster-context row.
const CONTEXT_ROW_BYTES: usize = 48;
/// Width of one terrain row.
const TERRAIN_ROW_BYTES: usize = 52;

const ENEMY_LOOKUP_KEY_OFFSET: usize = 0x04;
const ENEMY_COST_OFFSET: usize = 0x0C;
const ENEMY_SCRATCH_RULE_OFFSET: usize = 0x12;
const ENEMY_TERRAIN_MASK_OFFSET: usize = 0x14;
const ENEMY_PLAYTHROUGH_MASK_OFFSET: usize = 0x16;
const ENEMY_GROUP_OFFSET: usize = 0x18;
const ENEMY_SELECTOR_OFFSET: usize = 0x19;
const ENEMY_ROLE_OFFSET: usize = 0x1A;
const ENEMY_PARAMETER_OFFSET: usize = 0x1B;

const CONTEXT_BUDGET_OFFSET: usize = 0x04;
const CONTEXT_MODE_OFFSET: usize = 0x28;
const CONTEXT_BRANCH_OFFSET: usize = 0x29;
const CONTEXT_COUNT_OFFSET: usize = 0x2A;

const TERRAIN_BLOCK_OFFSET: usize = 0x2C;
const TERRAIN_EXTRA_BLOCK_OFFSET: usize = 0x2E;
const TERRAIN_DESCRIPTOR_OFFSET: usize = 0x31;

/// Number of wave budgets stored in one context row.
const WAVE_BUDGET_COUNT: usize = 5;
/// First native spawn key assigned by the roster stage.
const SPAWN_BASE: u32 = 0xF3C;
/// Last spawn key the native stage can hand out.
const SPAWN_LAST: u32 = 0xF96;
/// Number of occurrences one roster may hold before the native stage truncates.
const SPAWN_CAP: usize = (SPAWN_LAST - SPAWN_BASE + 1) as usize;
/// Mask applied to the displayed seed at RVA 0x10295B4..0x10295D7.
const SEED_MASK: u32 = 0x0FFF_FFFF;

const ROLE_FOUR: u8 = 4;
const ROLE_FIVE: u8 = 5;

/// Resolve the native roster for one explicit, already-resolved generator
/// context.
///
/// `role5_threshold` is the optional-multiplier threshold for key `0xCEFC`
/// that the caller resolves from the R4 finalizer resource; the reference
/// resolves it internally. It is only consulted for branch 0 waves other than
/// the last one, and only while role-5 candidates exist.
///
/// `RosterResult::waves` is returned in wave-index order, while spawn keys are
/// assigned in construction order (branch 0/1 walk waves high-to-low, branch 2
/// walks them low-to-high).
pub fn generate_roster(
    input: RosterInput,
    tables: &RosterTables,
    role5_threshold: i32,
) -> Result<RosterResult, EnemyError> {
    if !(1..=5).contains(&input.playthrough) {
        return Err(EnemyError::InvalidInput(format!(
            "playthrough {} is outside the supported 1..=5 progression",
            input.playthrough
        )));
    }
    if tables.contexts.is_empty() {
        return Err(EnemyError::MissingData(
            "roster context table is empty".to_string(),
        ));
    }
    if tables.enemies.is_empty() {
        return Err(EnemyError::MissingData(
            "enemy candidate table is empty".to_string(),
        ));
    }
    if tables.parameter_types.is_empty() {
        return Err(EnemyError::MissingData(
            "enemy parameter gate is empty".to_string(),
        ));
    }
    if tables.terrain_keys.len() != tables.terrains.len() {
        return Err(EnemyError::MissingData(format!(
            "terrain key count {} does not match {} terrain rows",
            tables.terrain_keys.len(),
            tables.terrains.len()
        )));
    }
    if input.terrain_row_index >= tables.terrains.len() {
        return Err(EnemyError::InvalidInput(format!(
            "terrain row index {} is outside {} terrain rows",
            input.terrain_row_index,
            tables.terrains.len()
        )));
    }

    let matches: Vec<&[u8; CONTEXT_ROW_BYTES]> = tables
        .contexts
        .iter()
        .filter(|row| row[CONTEXT_MODE_OFFSET] == input.auxiliary_mode)
        .collect();
    let context = match matches.as_slice() {
        [row] => *row,
        _ => {
            return Err(EnemyError::MissingData(format!(
                "auxiliary mode 0x{:02X} resolved to {} context rows",
                input.auxiliary_mode,
                matches.len()
            )))
        }
    };
    let branch = context[CONTEXT_BRANCH_OFFSET];
    if branch > 2 {
        return Err(EnemyError::Unsupported(format!(
            "unknown enemy branch class {branch}"
        )));
    }

    let rows = &tables.enemies;
    let terrain_row = &tables.terrains[input.terrain_row_index];
    let terrain = (tables.terrain_keys[input.terrain_row_index] & 0xFF) as u8;

    for row in rows {
        let cost = enemy_cost(row);
        if !cost.is_finite() || cost <= 0.0 {
            return Err(EnemyError::InvalidInput(format!(
                "enemy cost {cost} for lookup key 0x{:08X} must be positive and finite",
                enemy_lookup_key(row)
            )));
        }
    }

    let mut budgets = [0f32; WAVE_BUDGET_COUNT];
    for (index, budget) in budgets.iter_mut().enumerate() {
        *budget = read_f32(context, CONTEXT_BUDGET_OFFSET + 4 * index);
        if !budget.is_finite() {
            return Err(EnemyError::InvalidInput(format!(
                "wave {index} budget {budget} must be a finite native value"
            )));
        }
    }
    // The native profile ends at the first non-positive budget.
    let wave_count = budgets
        .iter()
        .position(|budget| *budget <= 0.0)
        .unwrap_or(WAVE_BUDGET_COUNT);

    let order: Vec<usize> = if branch == 2 {
        (0..wave_count).collect()
    } else {
        (0..wave_count).rev().collect()
    };

    let pool = pool(
        rows,
        terrain_row,
        input.flags,
        input.selector,
        input.playthrough,
        &tables.parameter_types,
    );
    let general: Vec<usize> = pool
        .iter()
        .copied()
        .filter(|&index| !is_extended_role(rows[index][ENEMY_ROLE_OFFSET]))
        .collect();
    let mut four: Vec<usize> = pool
        .iter()
        .copied()
        .filter(|&index| rows[index][ENEMY_ROLE_OFFSET] == ROLE_FOUR)
        .collect();
    let mut five: Vec<usize> = pool
        .iter()
        .copied()
        .filter(|&index| rows[index][ENEMY_ROLE_OFFSET] == ROLE_FIVE)
        .collect();
    let common: Vec<usize> = general
        .iter()
        .copied()
        .filter(|&index| matches!(rows[index][ENEMY_ROLE_OFFSET], 0 | 2))
        .collect();
    let special: Vec<usize> = general
        .iter()
        .copied()
        .filter(|&index| !matches!(rows[index][ENEMY_ROLE_OFFSET], 0 | 2))
        .collect();

    let mut parent = LcgStream::new(input.seed & SEED_MASK);
    let mut waves: Vec<Vec<Occurrence>> = vec![Vec::new(); wave_count];
    let mut spawn = SPAWN_BASE;

    for wave in order {
        let starting_budget = f64::from(budgets[wave]);
        let mut work: Vec<usize> = general.clone();
        let mut selected: Vec<usize> = Vec::new();
        let count = if input.variant == MissionVariant::Expedition {
            usize::from(context[CONTEXT_COUNT_OFFSET + wave])
        } else {
            0
        };

        let base_count = if branch == 1 || branch == 2 {
            let highest = branch == 1 && wave == wave_count - 1;
            let mut remaining = starting_budget;
            if !highest && !input.flags[2] {
                if let Some(first) = select_by_ticket(&special, rows, remaining, parent.u16()) {
                    selected.push(first);
                    remaining = f64::from(f32_of(remaining - f64::from(enemy_cost(&rows[first]))));
                }
                if let Some(second) = select_by_ticket(&common, rows, remaining, parent.u16()) {
                    // Native general contains the second row's group, so every
                    // role 0/2 slot is rewritten in place and can repeat.
                    let group = rows[second][ENEMY_GROUP_OFFSET];
                    let mut replacements = vec![second];
                    if group != 0 {
                        replacements.extend(general.iter().copied().filter(|&index| {
                            index != second && rows[index][ENEMY_GROUP_OFFSET] == group
                        }));
                    }
                    let mut next = 0usize;
                    for slot in work.iter_mut() {
                        if matches!(rows[*slot][ENEMY_ROLE_OFFSET], 0 | 2) {
                            *slot = replacements[next % replacements.len()];
                            next += 1;
                        }
                    }
                }
            }
            let wave_pool: &[usize] = if highest { &five } else { &work };
            budget_and_extras(
                wave_pool,
                &general,
                &mut selected,
                remaining,
                rows,
                &mut parent,
                count,
                highest,
                false,
            )?
        } else {
            let mut role5 = !five.is_empty();
            if role5 && wave != wave_count - 1 {
                role5 = lottery_10000(parent.u16()) >= role5_threshold;
            }
            let wave_pool: &[usize] = if role5 { &five } else { &four };
            let base_count = budget_and_extras(
                wave_pool,
                &general,
                &mut selected,
                starting_budget,
                rows,
                &mut parent,
                count,
                false,
                true,
            )?;
            // Consumed rows leave the active pool, and a nonzero group drops
            // every sibling from both extended-role pools.
            for &index in &selected {
                let position = if role5 {
                    five.iter().position(|&candidate| candidate == index)
                } else {
                    four.iter().position(|&candidate| candidate == index)
                };
                if let Some(position) = position {
                    if role5 {
                        five.remove(position);
                    } else {
                        four.remove(position);
                    }
                }
                let group = rows[index][ENEMY_GROUP_OFFSET];
                if group != 0 {
                    four.retain(|&candidate| rows[candidate][ENEMY_GROUP_OFFSET] != group);
                    five.retain(|&candidate| rows[candidate][ENEMY_GROUP_OFFSET] != group);
                }
            }
            base_count
        };

        let mut occurrences = Vec::with_capacity(selected.len());
        for (position, &index) in selected.iter().enumerate() {
            if spawn > SPAWN_LAST {
                return Err(EnemyError::Unsupported(
                    "native spawn cap reached; truncated profile not supported".to_string(),
                ));
            }
            let row = &rows[index];
            occurrences.push(Occurrence {
                wave_index: wave,
                position,
                native_spawn_key: spawn,
                lookup_key: enemy_lookup_key(row),
                role: row[ENEMY_ROLE_OFFSET],
                source_row_index: index,
                selector_class: u8::from(position >= base_count),
                scratch_rule_key: read_u16(row, ENEMY_SCRATCH_RULE_OFFSET),
            });
            spawn += 1;
        }
        waves[wave] = occurrences;
    }

    Ok(RosterResult {
        seed: input.seed,
        playthrough: input.playthrough,
        variant: input.variant,
        auxiliary_mode: input.auxiliary_mode,
        terrain,
        branch_class: branch,
        waves,
        state_after_roster: parent.state(),
        parent_draws: parent.draws(),
    })
}

fn read_u16(row: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([row[offset], row[offset + 1]])
}

fn read_u32(row: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        row[offset],
        row[offset + 1],
        row[offset + 2],
        row[offset + 3],
    ])
}

fn read_f32(row: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        row[offset],
        row[offset + 1],
        row[offset + 2],
        row[offset + 3],
    ])
}

fn enemy_cost(row: &[u8; ENEMY_ROW_BYTES]) -> f32 {
    read_f32(row, ENEMY_COST_OFFSET)
}

fn enemy_lookup_key(row: &[u8; ENEMY_ROW_BYTES]) -> u32 {
    read_u32(row, ENEMY_LOOKUP_KEY_OFFSET)
}

fn is_extended_role(role: u8) -> bool {
    role == ROLE_FOUR || role == ROLE_FIVE
}

/// `auxiliary_generation._enemy_parameter_gate_accepts` (row +0x80 gate).
///
/// An unmapped lookup key is accepted, neutral parameter types are only
/// accepted for extended roles unless the descriptor asked for neutral types.
fn parameter_gate_accepts(
    row: &[u8; ENEMY_ROW_BYTES],
    descriptor_flag_22: bool,
    gate: &BTreeMap<u32, u32>,
) -> bool {
    let Some(&parameter_type) = gate.get(&enemy_lookup_key(row)) else {
        return true;
    };
    let neutral_type = parameter_type == 0 || parameter_type == 3;
    if descriptor_flag_22 {
        return neutral_type;
    }
    if !neutral_type {
        return true;
    }
    is_extended_role(row[ENEMY_ROLE_OFFSET])
}

/// `auxiliary_generation._enemy_terrain_gate_accepts`.
fn terrain_gate_accepts(row: &[u8; ENEMY_ROW_BYTES], terrain: &[u8; TERRAIN_ROW_BYTES]) -> bool {
    let mask = read_u16(row, ENEMY_TERRAIN_MASK_OFFSET);
    for bit in 0..3u16 {
        if mask & (1 << bit) == 0 {
            continue;
        }
        let blocked = if bit == 0 {
            read_u16(terrain, TERRAIN_BLOCK_OFFSET) != 0
        } else {
            read_u16(terrain, TERRAIN_EXTRA_BLOCK_OFFSET) >> (bit - 1) & 1 == 1
        };
        if blocked {
            return false;
        }
    }
    true
}

/// `enemy_variant_generation._pool` (RVA 0x102A160..0x102A3E0).
///
/// A nonzero descriptor selector switches to the selector branch, which
/// matches row +0x19 and jumps over the descriptor-flag and parameter gates.
/// The terrain gate applies to both branches.
fn pool(
    rows: &[[u8; ENEMY_ROW_BYTES]],
    terrain: &[u8; TERRAIN_ROW_BYTES],
    flags: [bool; 3],
    selector: u8,
    playthrough: u8,
    gate: &BTreeMap<u32, u32>,
) -> Vec<usize> {
    let playthrough_bit = 1u8 << (playthrough - 1);
    let mut out = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if row[ENEMY_PLAYTHROUGH_MASK_OFFSET] & playthrough_bit == 0 {
            continue;
        }
        if selector != 0 && !is_extended_role(row[ENEMY_ROLE_OFFSET]) {
            if row[ENEMY_SELECTOR_OFFSET] != selector {
                continue;
            }
        } else {
            let parameter = row[ENEMY_PARAMETER_OFFSET];
            if flags[0] && parameter != 0 && parameter != terrain[TERRAIN_DESCRIPTOR_OFFSET] {
                continue;
            }
            if !parameter_gate_accepts(row, flags[1], gate) {
                continue;
            }
        }
        if terrain_gate_accepts(row, terrain) {
            out.push(index);
        }
    }
    out
}

/// `auxiliary_generation._select_enemy_by_u16_ticket`.
fn select_by_ticket(
    candidates: &[usize],
    rows: &[[u8; ENEMY_ROW_BYTES]],
    budget: f64,
    ticket: u16,
) -> Option<usize> {
    let eligible: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|&index| f64::from(enemy_cost(&rows[index])) <= budget)
        .collect();
    if eligible.is_empty() {
        return None;
    }
    Some(eligible[usize::from(ticket) % eligible.len()])
}

/// `enemy_variant_generation.append_group_extras` (RVA 0x1027FE0).
///
/// Row identity controls exclusion. A zero group repeats the anchor itself and
/// consumes no draw; any other group shuffles `general` with an MT stream
/// seeded from one local draw. An empty anchor group returns no extras so the
/// caller can report the incoherent table instead of relabelling base entries.
fn append_group_extras(
    general: &[usize],
    selected: &[usize],
    anchor: usize,
    count: usize,
    rows: &[[u8; ENEMY_ROW_BYTES]],
    local: &mut LcgStream,
) -> Result<Vec<usize>, EnemyError> {
    if count == 0 {
        return Ok(Vec::new());
    }
    if count > 255 || anchor >= rows.len() {
        return Err(EnemyError::InvalidInput(format!(
            "invalid extra count/anchor: count {count}, anchor {anchor}"
        )));
    }
    let group = rows[anchor][ENEMY_GROUP_OFFSET];
    let choices = if group == 0 {
        vec![anchor]
    } else {
        let all_group: Vec<usize> = general
            .iter()
            .copied()
            .filter(|&index| rows[index][ENEMY_GROUP_OFFSET] == group)
            .collect();
        if all_group.is_empty() {
            return Ok(Vec::new());
        }
        let unselected: Vec<usize> = all_group
            .iter()
            .copied()
            .filter(|index| !selected.contains(index))
            .collect();
        let mut choices = if unselected.is_empty() {
            all_group
        } else {
            unselected
        };
        let seed = u32::from(local.u16());
        let mut mt = Mt19937::new(seed);
        native_shuffle(&mut choices, &mut mt);
        choices
    };
    Ok((0..count)
        .map(|index| choices[index % choices.len()])
        .collect())
}

/// `enemy_variant_generation.budget_and_extras`.
///
/// One parent draw seeds all local budget, anchor, and shuffle work. The
/// ticket is drawn before affordability is known, so a wave that cannot
/// afford another row still consumes that local draw before the extra anchor
/// is chosen. Returns the number of base selections, which is also the first
/// selector-class-1 position.
#[allow(clippy::too_many_arguments)]
fn budget_and_extras(
    pool: &[usize],
    general: &[usize],
    selected: &mut Vec<usize>,
    remaining_budget: f64,
    rows: &[[u8; ENEMY_ROW_BYTES]],
    parent: &mut LcgStream,
    count: usize,
    first_group: bool,
    class0: bool,
) -> Result<usize, EnemyError> {
    let mut local = LcgStream::new(u32::from(parent.u16()));
    let mut remaining = remaining_budget;
    let mut prefer_max = first_group || class0;
    while remaining > 0.0 && !pool.is_empty() {
        let ticket = local.u16();
        let mut eligible: Vec<usize> = pool
            .iter()
            .copied()
            .filter(|&index| f64::from(enemy_cost(&rows[index])) <= remaining)
            .collect();
        if eligible.is_empty() {
            break;
        }
        if prefer_max {
            let maximum = eligible
                .iter()
                .map(|&index| enemy_cost(&rows[index]))
                .fold(f32::NEG_INFINITY, f32::max);
            eligible.retain(|&index| enemy_cost(&rows[index]) == maximum);
        }
        let index = eligible[usize::from(ticket) % eligible.len()];
        selected.push(index);
        if first_group {
            break;
        }
        if selected.len() > SPAWN_CAP {
            // Every selection becomes an occurrence, so the native stage
            // rejects this wave when it builds the profile.
            return Err(EnemyError::Unsupported(
                "native spawn cap reached; truncated profile not supported".to_string(),
            ));
        }
        let next = f64::from(f32_of(remaining - f64::from(enemy_cost(&rows[index]))));
        if next >= remaining {
            return Err(EnemyError::InvalidInput(format!(
                "wave budget {remaining} cannot be reduced by the selected native cost"
            )));
        }
        remaining = next;
        prefer_max = false;
    }

    let base_count = selected.len();
    let anchors: Vec<usize> = selected
        .iter()
        .copied()
        .filter(|&index| !is_extended_role(rows[index][ENEMY_ROLE_OFFSET]))
        .collect();
    if count != 0 && !anchors.is_empty() {
        let anchor = anchors[usize::from(local.u16()) % anchors.len()];
        let extras = append_group_extras(general, selected, anchor, count, rows, &mut local)?;
        if extras.len() != count {
            return Err(EnemyError::MissingData(
                "extra helper could not fulfil native anchor group".to_string(),
            ));
        }
        selected.extend(extras);
    }
    Ok(base_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mapped to a parameter type, so the parameter gate has an opinion.
    const GATED_KEY: u32 = 7;
    /// Carried by the gate map without matching any fixture row.
    const SPARE_KEY: u32 = 0xDEAD_BEEF;

    fn write_u16(row: &mut [u8], offset: usize, value: u16) {
        row[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    #[derive(Clone, Copy)]
    struct EnemyRow([u8; ENEMY_ROW_BYTES]);

    impl EnemyRow {
        fn new(cost: f32, role: u8) -> Self {
            let mut row = [0u8; ENEMY_ROW_BYTES];
            row[ENEMY_PLAYTHROUGH_MASK_OFFSET] = 0b1_1111;
            row[ENEMY_ROLE_OFFSET] = role;
            row[ENEMY_COST_OFFSET..ENEMY_COST_OFFSET + 4].copy_from_slice(&cost.to_le_bytes());
            Self(row)
        }

        fn lookup(mut self, key: u32) -> Self {
            self.0[ENEMY_LOOKUP_KEY_OFFSET..ENEMY_LOOKUP_KEY_OFFSET + 4]
                .copy_from_slice(&key.to_le_bytes());
            self
        }

        fn group(mut self, value: u8) -> Self {
            self.0[ENEMY_GROUP_OFFSET] = value;
            self
        }

        fn selector(mut self, value: u8) -> Self {
            self.0[ENEMY_SELECTOR_OFFSET] = value;
            self
        }

        fn parameter(mut self, value: u8) -> Self {
            self.0[ENEMY_PARAMETER_OFFSET] = value;
            self
        }

        fn playthrough_mask(mut self, value: u8) -> Self {
            self.0[ENEMY_PLAYTHROUGH_MASK_OFFSET] = value;
            self
        }

        fn terrain_mask(mut self, value: u16) -> Self {
            write_u16(&mut self.0, ENEMY_TERRAIN_MASK_OFFSET, value);
            self
        }

        fn row(self) -> [u8; ENEMY_ROW_BYTES] {
            self.0
        }
    }

    fn context_row(
        mode: u8,
        branch: u8,
        budgets: [f32; WAVE_BUDGET_COUNT],
        counts: [u8; WAVE_BUDGET_COUNT],
    ) -> [u8; CONTEXT_ROW_BYTES] {
        let mut row = [0u8; CONTEXT_ROW_BYTES];
        row[CONTEXT_MODE_OFFSET] = mode;
        row[CONTEXT_BRANCH_OFFSET] = branch;
        for (index, budget) in budgets.iter().enumerate() {
            let offset = CONTEXT_BUDGET_OFFSET + 4 * index;
            row[offset..offset + 4].copy_from_slice(&budget.to_le_bytes());
        }
        row[CONTEXT_COUNT_OFFSET..CONTEXT_COUNT_OFFSET + WAVE_BUDGET_COUNT]
            .copy_from_slice(&counts);
        row
    }

    /// One terrain row, a matching key count, and an inert parameter gate.
    fn tables(
        enemies: Vec<[u8; ENEMY_ROW_BYTES]>,
        context: [u8; CONTEXT_ROW_BYTES],
        terrain: [u8; TERRAIN_ROW_BYTES],
        terrain_key: u16,
    ) -> RosterTables {
        RosterTables {
            enemies,
            contexts: vec![context],
            terrains: vec![terrain],
            terrain_keys: vec![terrain_key],
            parameter_types: BTreeMap::from([(SPARE_KEY, 3u32)]),
        }
    }

    fn input(variant: MissionVariant, flags: [bool; 3]) -> RosterInput {
        RosterInput {
            seed: 0x1234_5678,
            playthrough: 1,
            variant,
            auxiliary_mode: 0x01,
            terrain_row_index: 0,
            selector: 0,
            flags,
        }
    }

    #[test]
    fn parameter_gate_follows_type_and_role_rules() {
        let gate = BTreeMap::from([(GATED_KEY, 0u32), (GATED_KEY + 1, 5u32)]);
        let neutral_role0 = EnemyRow::new(1.0, 0).lookup(GATED_KEY).row();
        let neutral_role5 = EnemyRow::new(1.0, 5).lookup(GATED_KEY).row();
        let non_neutral = EnemyRow::new(1.0, 0).lookup(GATED_KEY + 1).row();
        let unmapped = EnemyRow::new(1.0, 0).lookup(9).row();

        assert!(parameter_gate_accepts(&unmapped, true, &gate));
        assert!(parameter_gate_accepts(&neutral_role0, true, &gate));
        assert!(!parameter_gate_accepts(&neutral_role0, false, &gate));
        assert!(parameter_gate_accepts(&neutral_role5, false, &gate));
        assert!(!parameter_gate_accepts(&non_neutral, true, &gate));
        assert!(parameter_gate_accepts(&non_neutral, false, &gate));
    }

    #[test]
    fn terrain_gate_blocks_flagged_rows() {
        let mut terrain = [0u8; TERRAIN_ROW_BYTES];
        write_u16(&mut terrain, TERRAIN_BLOCK_OFFSET, 1);
        write_u16(&mut terrain, TERRAIN_EXTRA_BLOCK_OFFSET, 0b10);

        assert!(!terrain_gate_accepts(
            &EnemyRow::new(1.0, 0).terrain_mask(0b001).row(),
            &terrain
        ));
        assert!(terrain_gate_accepts(
            &EnemyRow::new(1.0, 0).terrain_mask(0b010).row(),
            &terrain
        ));
        assert!(!terrain_gate_accepts(
            &EnemyRow::new(1.0, 0).terrain_mask(0b100).row(),
            &terrain
        ));
        assert!(terrain_gate_accepts(&EnemyRow::new(1.0, 0).row(), &terrain));
    }

    #[test]
    fn pool_applies_playthrough_selector_and_terrain_gates() {
        let mut terrain = [0u8; TERRAIN_ROW_BYTES];
        write_u16(&mut terrain, TERRAIN_BLOCK_OFFSET, 1);
        let gate = BTreeMap::from([(GATED_KEY, 0u32)]);
        let rows = vec![
            EnemyRow::new(1.0, 0).parameter(5).row(),
            EnemyRow::new(1.0, 2).lookup(GATED_KEY).row(),
            EnemyRow::new(1.0, 0).lookup(GATED_KEY).selector(9).row(),
            EnemyRow::new(1.0, 0).selector(4).row(),
            EnemyRow::new(1.0, 0).playthrough_mask(0b0000_0010).row(),
            EnemyRow::new(1.0, 0).terrain_mask(0b1).row(),
        ];

        // Row 1 and 2 are neutral parameter rows that are not extended roles,
        // row 4 needs playthrough 2, and row 5 is blocked by terrain bit 0.
        assert_eq!(
            pool(&rows, &terrain, [false, false, false], 0, 1, &gate),
            vec![0, 3]
        );
        assert_eq!(
            pool(&rows, &terrain, [false, false, false], 0, 2, &gate),
            vec![0, 3, 4]
        );
        // Descriptor flag 0 compares row +0x1B against terrain +0x31.
        assert_eq!(
            pool(&rows, &terrain, [true, false, false], 0, 1, &gate),
            vec![3]
        );
        // The selector branch matches +0x19 only and skips both gates.
        assert_eq!(
            pool(&rows, &terrain, [false, true, false], 9, 1, &gate),
            vec![2]
        );
    }

    #[test]
    fn append_group_extras_respects_group_identity_and_row_exclusion() {
        let rows = vec![
            EnemyRow::new(1.0, 0).group(0).row(),
            EnemyRow::new(1.0, 0).group(5).row(),
            EnemyRow::new(1.0, 0).group(5).row(),
            EnemyRow::new(1.0, 0).group(5).row(),
        ];
        let general = vec![1, 2, 3];

        // A zero group is a native singleton: the anchor repeats, no draw.
        let mut local = LcgStream::new(0);
        assert_eq!(
            append_group_extras(&general, &[], 0, 3, &rows, &mut local).unwrap(),
            vec![0, 0, 0]
        );
        assert_eq!(local.draws(), 0);

        // Row identity controls exclusion, so row 2 can never be an extra.
        let mut local = LcgStream::new(0);
        let extras = append_group_extras(&general, &[2], 1, 2, &rows, &mut local).unwrap();
        assert_eq!(extras.len(), 2);
        assert!(extras.iter().all(|&index| index == 1 || index == 3));
        assert_eq!(local.draws(), 1);

        assert!(append_group_extras(&general, &[], 1, 0, &rows, &mut local)
            .unwrap()
            .is_empty());
        assert!(matches!(
            append_group_extras(&general, &[], 0, 256, &rows, &mut local),
            Err(EnemyError::InvalidInput(_))
        ));
    }

    #[test]
    fn budget_and_extras_consumes_the_failed_affordability_draw() {
        let rows = vec![
            EnemyRow::new(1.0, 0).group(0).row(),
            EnemyRow::new(1.0, 0).group(0).row(),
        ];
        let general = vec![0, 1];

        // Parent seed 6 seeds a local stream whose draws are 6, 43539, 1623
        // and 17332. A budget of exactly 2.0 ends the loop with no failed
        // attempt, so the extras anchor is draw 3 (odd, second anchor). A
        // budget of 2.5 leaves 0.5, so the loop draws once more before giving
        // up and the anchor becomes draw 4 (even, first anchor).
        let mut parent = LcgStream::new(6);
        let mut selected = Vec::new();
        let base = budget_and_extras(
            &general,
            &general,
            &mut selected,
            2.0,
            &rows,
            &mut parent,
            1,
            false,
            false,
        )
        .unwrap();
        assert_eq!(base, 2);
        assert_eq!(selected, vec![0, 1, 1]);
        assert_eq!(parent.draws(), 1);

        let mut parent = LcgStream::new(6);
        let mut selected = Vec::new();
        let base = budget_and_extras(
            &general,
            &general,
            &mut selected,
            2.5,
            &rows,
            &mut parent,
            1,
            false,
            false,
        )
        .unwrap();
        assert_eq!(base, 2);
        assert_eq!(selected, vec![0, 1, 0]);
        assert_eq!(parent.draws(), 1);
    }

    #[test]
    fn branch_zero_reverses_wave_order_and_selects_role_five_last() {
        let rows = vec![
            EnemyRow::new(1.0, 4).group(0).row(),
            EnemyRow::new(1.0, 4).group(0).row(),
            EnemyRow::new(1.0, 5).group(0).row(),
            EnemyRow::new(1.0, 5).group(0).row(),
        ];
        let context = context_row(0x01, 0, [2.0, 2.0, 0.0, 0.0, 0.0], [0; WAVE_BUDGET_COUNT]);
        let tables = tables(rows, context, [0u8; TERRAIN_ROW_BYTES], 0x1234);

        let result = generate_roster(input(MissionVariant::Solo, [false; 3]), &tables, 10_000)
            .expect("roster");

        assert_eq!(result.branch_class, 0);
        assert_eq!(result.terrain, 0x34);
        assert_eq!(result.waves.len(), 2);
        // Wave 1 is built first and keeps low spawn keys; only the last wave
        // is allowed to use the role-5 pool.
        assert_eq!(result.waves[1][0].native_spawn_key, SPAWN_BASE);
        assert!(result.waves[1].iter().all(|entry| entry.role == ROLE_FIVE));
        assert_eq!(result.waves[0][0].native_spawn_key, SPAWN_BASE + 2);
        assert!(result.waves[0].iter().all(|entry| entry.role == ROLE_FOUR));
        assert!(result.occurrences().all(|entry| entry.selector_class == 0));
        assert!(result
            .waves
            .iter()
            .flat_map(|wave| wave.iter())
            .all(|entry| entry.wave_index <= 1));
        assert_eq!(result.parent_draws, 2);
    }

    #[test]
    fn branch_two_keeps_forward_order_and_adds_expedition_extras() {
        let rows: Vec<[u8; ENEMY_ROW_BYTES]> = (0..4)
            .map(|index| EnemyRow::new(1.0, 0).group(0).lookup(100 + index).row())
            .collect();
        let context = context_row(0x01, 2, [3.0, 3.0, 0.0, 0.0, 0.0], [1; WAVE_BUDGET_COUNT]);
        let tables = tables(rows, context, [0u8; TERRAIN_ROW_BYTES], 0);

        let result = generate_roster(
            input(MissionVariant::Expedition, [false, false, true]),
            &tables,
            10_000,
        )
        .expect("roster");

        assert_eq!(result.waves.len(), 2);
        assert_eq!(result.waves[0][0].native_spawn_key, SPAWN_BASE);
        assert_eq!(result.waves[1][0].native_spawn_key, SPAWN_BASE + 4);
        for wave in &result.waves {
            assert_eq!(wave.len(), 4);
            assert_eq!(
                wave.iter().map(|entry| entry.position).collect::<Vec<_>>(),
                vec![0, 1, 2, 3]
            );
            assert!(wave[..3].iter().all(|entry| entry.selector_class == 0));
            assert_eq!(wave[3].selector_class, 1);
            // The zero-group anchor repeats a base row, which the expedition
            // count turns into the role-0 extended entry.
            assert!(wave[..3]
                .iter()
                .any(|entry| entry.source_row_index == wave[3].source_row_index));
        }
    }

    #[test]
    fn branch_one_preselection_and_highest_wave_follow_native_rules() {
        let rows = vec![
            EnemyRow::new(1.0, 1).group(5).row(),
            EnemyRow::new(0.25, 0).group(7).row(),
            EnemyRow::new(0.25, 2).group(9).row(),
            EnemyRow::new(1.0, 5).group(0).row(),
            EnemyRow::new(0.25, 5).group(0).row(),
        ];
        let context = context_row(0x01, 1, [1.5, 1.5, 0.0, 0.0, 0.0], [0; WAVE_BUDGET_COUNT]);
        let tables = tables(rows, context, [0u8; TERRAIN_ROW_BYTES], 0);

        let result = generate_roster(input(MissionVariant::Solo, [false; 3]), &tables, 10_000)
            .expect("roster");

        // The highest wave picks a single affordable row and prefers the
        // maximum affordable cost.
        assert_eq!(result.waves[1].len(), 1);
        assert_eq!(result.waves[1][0].role, ROLE_FIVE);
        assert_eq!(result.waves[1][0].source_row_index, 3);
        assert_eq!(result.waves[1][0].native_spawn_key, SPAWN_BASE);

        // The lower wave preselects the special row, rewrites its role 0/2
        // slots with the second row's group, and keeps the duplicates.
        let wave = &result.waves[0];
        assert_eq!(wave.len(), 3);
        assert_eq!(wave[0].source_row_index, 0);
        assert_eq!(wave[0].role, 1);
        assert_eq!(wave[1].source_row_index, wave[2].source_row_index);
        assert!(matches!(wave[1].role, 0 | 2));
        assert!((1..=2).contains(&wave[1].source_row_index));
        assert_eq!(result.parent_draws, 4);
    }

    #[test]
    fn role_five_lottery_threshold_switches_the_wave_pool() {
        let rows: Vec<[u8; ENEMY_ROW_BYTES]> = (0..4)
            .map(|index| EnemyRow::new(1.0, 5).group(0).lookup(200 + index).row())
            .chain((0..2).map(|index| EnemyRow::new(1.0, 4).group(0).lookup(300 + index).row()))
            .collect();
        let context = context_row(0x01, 0, [2.0, 2.0, 0.0, 0.0, 0.0], [0; WAVE_BUDGET_COUNT]);
        let tables = tables(rows, context, [0u8; TERRAIN_ROW_BYTES], 0);

        let always =
            generate_roster(input(MissionVariant::Solo, [false; 3]), &tables, 0).expect("roster");
        assert!(always.waves[0].iter().all(|entry| entry.role == ROLE_FIVE));

        let never = generate_roster(input(MissionVariant::Solo, [false; 3]), &tables, 10_000)
            .expect("roster");
        assert!(never.waves[0].iter().all(|entry| entry.role == ROLE_FOUR));
        assert!(never.waves[1].iter().all(|entry| entry.role == ROLE_FIVE));
    }

    #[test]
    fn rejects_malformed_input_and_incomplete_tables() {
        let valid_rows = || vec![EnemyRow::new(1.0, 0).row()];
        let valid_context =
            || context_row(0x01, 2, [1.0, 0.0, 0.0, 0.0, 0.0], [0; WAVE_BUDGET_COUNT]);
        let valid_tables = |rows: Vec<[u8; ENEMY_ROW_BYTES]>, context: [u8; CONTEXT_ROW_BYTES]| {
            tables(rows, context, [0u8; TERRAIN_ROW_BYTES], 0)
        };
        let run = |tables: &RosterTables, input: RosterInput, threshold: i32| {
            generate_roster(input, tables, threshold)
        };

        // Progression and table completeness.
        for playthrough in [0u8, 6] {
            let mut request = input(MissionVariant::Solo, [false, false, true]);
            request.playthrough = playthrough;
            assert!(matches!(
                run(&valid_tables(valid_rows(), valid_context()), request, 0),
                Err(EnemyError::InvalidInput(_))
            ));
        }
        let mut no_contexts = valid_tables(valid_rows(), valid_context());
        no_contexts.contexts.clear();
        assert!(matches!(
            run(
                &no_contexts,
                input(MissionVariant::Solo, [false, false, true]),
                0
            ),
            Err(EnemyError::MissingData(_))
        ));
        let no_enemies = valid_tables(Vec::new(), valid_context());
        assert!(matches!(
            run(
                &no_enemies,
                input(MissionVariant::Solo, [false, false, true]),
                0
            ),
            Err(EnemyError::MissingData(_))
        ));
        let mut no_gate = valid_tables(valid_rows(), valid_context());
        no_gate.parameter_types.clear();
        assert!(matches!(
            run(
                &no_gate,
                input(MissionVariant::Solo, [false, false, true]),
                0
            ),
            Err(EnemyError::MissingData(_))
        ));
        let mut short_keys = valid_tables(valid_rows(), valid_context());
        short_keys.terrain_keys.clear();
        assert!(matches!(
            run(
                &short_keys,
                input(MissionVariant::Solo, [false, false, true]),
                0
            ),
            Err(EnemyError::MissingData(_))
        ));

        // Context resolution, branch class, and terrain bounds.
        let mut two_contexts = valid_tables(valid_rows(), valid_context());
        two_contexts.contexts.push(valid_context());
        assert!(matches!(
            run(
                &two_contexts,
                input(MissionVariant::Solo, [false, false, true]),
                0
            ),
            Err(EnemyError::MissingData(_))
        ));
        let mut other_mode = input(MissionVariant::Solo, [false, false, true]);
        other_mode.auxiliary_mode = 0x02;
        assert!(matches!(
            run(&valid_tables(valid_rows(), valid_context()), other_mode, 0),
            Err(EnemyError::MissingData(_))
        ));
        let unknown_branch = valid_tables(
            valid_rows(),
            context_row(0x01, 3, [1.0, 0.0, 0.0, 0.0, 0.0], [0; WAVE_BUDGET_COUNT]),
        );
        assert!(matches!(
            run(
                &unknown_branch,
                input(MissionVariant::Solo, [false, false, true]),
                0
            ),
            Err(EnemyError::Unsupported(_))
        ));
        let mut bad_terrain = input(MissionVariant::Solo, [false, false, true]);
        bad_terrain.terrain_row_index = 1;
        assert!(matches!(
            run(&valid_tables(valid_rows(), valid_context()), bad_terrain, 0),
            Err(EnemyError::InvalidInput(_))
        ));

        // Native costs and budgets must be positive and finite.
        for cost in [0.0f32, -1.0, f32::NAN, f32::INFINITY] {
            let bad_cost = valid_tables(vec![EnemyRow::new(cost, 0).row()], valid_context());
            assert!(matches!(
                run(
                    &bad_cost,
                    input(MissionVariant::Solo, [false, false, true]),
                    0
                ),
                Err(EnemyError::InvalidInput(_))
            ));
        }
        for budget in [f32::NAN, f32::INFINITY] {
            let bad_budget = valid_tables(
                valid_rows(),
                context_row(
                    0x01,
                    2,
                    [budget, 0.0, 0.0, 0.0, 0.0],
                    [0; WAVE_BUDGET_COUNT],
                ),
            );
            assert!(matches!(
                run(
                    &bad_budget,
                    input(MissionVariant::Solo, [false, false, true]),
                    0
                ),
                Err(EnemyError::InvalidInput(_))
            ));
        }

        // A subnormal cost cannot reduce a unit budget, so the wave fails
        // closed instead of looping forever.
        let unmovable = tables(
            vec![EnemyRow::new(f32::from_bits(1), 0).row()],
            valid_context(),
            [0u8; TERRAIN_ROW_BYTES],
            0,
        );
        assert!(matches!(
            run(
                &unmovable,
                input(MissionVariant::Solo, [false, false, true]),
                10_000
            ),
            Err(EnemyError::InvalidInput(_))
        ));

        // Every selection becomes an occurrence, so a wave past the native
        // spawn cap is rejected rather than truncated.
        let crowded: Vec<[u8; ENEMY_ROW_BYTES]> = (0..SPAWN_CAP + 1)
            .map(|index| {
                EnemyRow::new(1.0, 0)
                    .group(0)
                    .lookup(400 + index as u32)
                    .row()
            })
            .collect();
        let crowded = tables(
            crowded,
            context_row(0x01, 2, [500.0, 0.0, 0.0, 0.0, 0.0], [0; WAVE_BUDGET_COUNT]),
            [0u8; TERRAIN_ROW_BYTES],
            0,
        );
        assert!(matches!(
            run(
                &crowded,
                input(MissionVariant::Solo, [false, false, true]),
                0
            ),
            Err(EnemyError::Unsupported(_))
        ));
    }
}
