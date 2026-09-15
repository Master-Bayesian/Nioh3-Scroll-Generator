//! Seed-level entry points with the retained product support boundary.
//!
//! Two layers live here. [`generate_enemy_preview`] is the M1 enemy-stage
//! entry point. The composition section below assembles the complete offline
//! NG3 preview payload that `nioh3_scroll_editor/worker_contracts.py`
//! serializes: terrain display effects, the auxiliary enemy groups, the three
//! ordered special rules, per-occurrence enemy state, and the Seed-derived
//! challenge capacity.
//!
//! Everything here is pure: resource loading stays in `nioh3-data`, and the
//! application-owned fields (`candidate_id`, `context_digest`, the operation
//! policy decision, cursor, evidence and transport framing) deliberately do
//! not exist in this module.

use crate::{
    auxiliary::{
        generate_special_rules, terrain_display_effect_keys, SpecialRuleResult, SpecialRuleTables,
    },
    context::{optional_threshold, resolve_context, ResolvedContext},
    enemy::{
        ContextTables, CurseGate, EnemyError, EnemyStateTables, MissionVariant, Possession,
        RosterInput, RosterResult, RosterTables, WraithResult,
    },
    record::ScrollRecord,
    roster::generate_roster,
    sequence::generate_challenge_attempt_count,
    wraith::generate_wraith,
};

/// Scope note the shipped preview attaches to every Curse field.
pub const CURSE_SCOPE: &str = "late runtime context not captured; no Seed-only certainty claimed";
/// Curse input the shipped preview always reports as missing.
pub const CURSE_MISSING_INPUT: &str = "Curse: selector gates, resolved per-occurrence eligibility, probability/3B37 and placement source for the same invocation";

/// Byte offset of the wave-budget block inside a 48-byte context row.
const CONTEXT_BUDGET_OFFSET: usize = 0x04;
/// Byte offset of the auxiliary mode byte inside a 48-byte context row.
const CONTEXT_MODE_OFFSET: usize = 0x28;
/// Number of wave budgets stored in one context row.
const WAVE_BUDGET_COUNT: usize = 5;
/// Lookup key of the optional-multiplier row behind the role-4/role-5 split.
const ROLE5_THRESHOLD_KEY: u32 = 0xCEFC;
/// Scratch rule keys the native descriptor treats as "no rule".
const NO_SCRATCH_RULE: u16 = 0xFFFF;

#[derive(Debug, Clone)]
pub struct EnemyPreview {
    pub context: ResolvedContext,
    pub roster: RosterResult,
    pub wraith: WraithResult,
}

/// Generate supported enemy occurrences and their known or unknown Wraith state.
/// Explicit low-level roster generation remains available for research, but
/// this entry point never enables the unverified nonzero-selector branch.
pub fn generate_enemy_preview(
    seed: u32,
    playthrough: u8,
    variant: MissionVariant,
    roster_tables: &RosterTables,
    context_tables: &ContextTables,
    state_tables: &EnemyStateTables,
) -> Result<EnemyPreview, EnemyError> {
    let context = resolve_context(seed, roster_tables, context_tables)?;
    if context.selector != 0 {
        return Err(EnemyError::Unsupported(
            "selector-nonzero branch needs independent parity".into(),
        ));
    }
    let input = RosterInput {
        seed,
        playthrough,
        variant,
        auxiliary_mode: context.auxiliary_mode,
        terrain_row_index: context.terrain_row_index,
        selector: context.selector,
        flags: context.flags,
    };
    let threshold = if context.mode_branch == 0 {
        optional_threshold(context_tables, ROLE5_THRESHOLD_KEY)?
    } else {
        0 // Other branches do not consult the role-5 threshold.
    };
    let roster = generate_roster(input, roster_tables, threshold)?;
    let wraith = generate_wraith(&roster, state_tables);
    Ok(EnemyPreview {
        context,
        roster,
        wraith,
    })
}

/// One payload-shaped effect slot, mirroring
/// `models.ScrollCandidate.from_effect_sequence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectPreview {
    pub slot: u8,
    pub effect_id: u32,
    pub value: i32,
    pub metadata: u32,
    pub prefix: u16,
    pub tail_0: u32,
    pub tail_1: u32,
    pub roll_percent: Option<u8>,
}

/// Map one generated sequence into the payload's effect entries.
///
/// The reference packs `metadata` as
/// `roll_percent | category_and_flags << 8 | effect_flags << 16` and leaves the
/// two tail words zero; both sides must keep that exact packing.
pub fn effect_previews(record: &ScrollRecord) -> Vec<EffectPreview> {
    record
        .effects
        .iter()
        .map(|effect| EffectPreview {
            slot: effect.slot,
            effect_id: effect.effect_id,
            value: effect.resolved_value,
            metadata: u32::from(effect.roll_percent)
                | (u32::from(effect.category_and_flags) << 8)
                | (u32::from(effect.effect_flags) << 16),
            prefix: effect.prefix_word,
            tail_0: 0,
            tail_1: 0,
            roll_percent: Some(effect.roll_percent),
        })
        .collect()
}

/// One ordered enemy entry before record/UI name resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnemyEntryPreview {
    pub row_index: usize,
    pub lookup_key: u32,
    pub role: u8,
    pub scratch_rule_key: u16,
}

/// One displayed enemy group plus the wave budget it was drawn from.
///
/// Entries are the wave's base selection only. Expedition-only extras are
/// reported through [`EnemyStatePreview`], exactly like the reference, which
/// builds `auxiliary.enemy_groups` from the class generators and
/// `enemy_states` from the full roster.
#[derive(Debug, Clone, PartialEq)]
pub struct EnemyGroupPreview {
    pub entries: Vec<EnemyEntryPreview>,
    pub source_budget: f32,
}

/// Terrain phase values the payload exposes.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainPreview {
    pub value: u8,
    pub display_effect_keys: Vec<u16>,
    pub scoped_seed: u32,
    pub used_filtered_pool: bool,
    pub selected_row_index: usize,
}

/// Complete offline auxiliary composition for one displayed Seed.
#[derive(Debug, Clone, PartialEq)]
pub struct AuxiliaryPreview {
    pub mode: u8,
    pub mode_branch: u8,
    pub terrain: TerrainPreview,
    pub descriptor_selector: u8,
    pub descriptor_flags: [bool; 3],
    pub enemy_groups: Vec<EnemyGroupPreview>,
    pub special_rules: SpecialRuleResult,
}

/// Whether an occurrence belongs to the base roster or to the expedition extras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccurrenceAvailability {
    Base,
    ExpeditionOnly,
}

/// Conditional Curse bound for one occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurseConditional {
    Guaranteed,
    Never,
    Unknown,
}

/// One payload-shaped enemy occurrence with its Possessed and Curse state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnemyStateOccurrence {
    pub wave_index: usize,
    pub position: usize,
    pub lookup_key: u32,
    pub role: u8,
    pub source_row_index: usize,
    pub availability: OccurrenceAvailability,
    pub native_spawn_key: u32,
    pub possessed: Possession,
    pub curse_if_fresh_null_source_selector_runs: CurseConditional,
}

/// Complete enemy-state preview for one mission variant.
#[derive(Debug, Clone, PartialEq)]
pub struct EnemyStatePreview {
    pub seed: u32,
    pub playthrough: u8,
    pub variant: MissionVariant,
    pub terrain: u8,
    pub occurrences: Vec<EnemyStateOccurrence>,
    pub possessed_complete: bool,
    pub missing_inputs: Vec<String>,
    pub curse_scope: &'static str,
}

/// Complete offline NG3 preview payload minus the application-owned fields.
#[derive(Debug, Clone, PartialEq)]
pub struct Ng3PreviewComposition {
    pub auxiliary: AuxiliaryPreview,
    /// Solo first, expedition second, matching `candidate_payload`.
    pub enemy_states: [EnemyStatePreview; 2],
    pub initial_challenge_capacity: i32,
}

/// Typed resource bundle the composition functions read.
#[derive(Debug, Clone, Copy)]
pub struct PreviewTables<'a> {
    pub roster: &'a RosterTables,
    pub context: &'a ContextTables,
    pub rules: &'a SpecialRuleTables,
    pub states: &'a EnemyStateTables,
}

/// Compose terrain, auxiliary enemy groups and the ordered special rules.
///
/// The base roster is variant independent: the reference builds these groups
/// from the class generators, and a solo roster reproduces every group entry
/// byte for byte. Expedition extras are composed by
/// [`compose_enemy_state_preview`].
pub fn compose_auxiliary_preview(
    seed: u32,
    playthrough: u8,
    tables: &PreviewTables<'_>,
) -> Result<AuxiliaryPreview, EnemyError> {
    let preview = generate_enemy_preview(
        seed,
        playthrough,
        MissionVariant::Solo,
        tables.roster,
        tables.context,
        tables.states,
    )?;
    let budgets = wave_budgets(tables.roster, preview.context.auxiliary_mode)?;

    let mut enemy_groups = Vec::with_capacity(preview.roster.waves.len());
    for (wave, occurrences) in preview.roster.waves.iter().enumerate() {
        let entries = occurrences
            .iter()
            .filter(|occurrence| occurrence.selector_class == 0)
            .map(|occurrence| EnemyEntryPreview {
                row_index: occurrence.source_row_index,
                lookup_key: occurrence.lookup_key,
                role: occurrence.role,
                scratch_rule_key: occurrence.scratch_rule_key,
            })
            .collect();
        enemy_groups.push(EnemyGroupPreview {
            entries,
            source_budget: budgets[wave],
        });
    }

    let scratch_rule_keys: Vec<u16> = enemy_groups
        .iter()
        .flat_map(|group| group.entries.iter())
        .map(|entry| entry.scratch_rule_key)
        .filter(|key| *key != NO_SCRATCH_RULE)
        .collect();
    let special_rules =
        generate_special_rules(seed, playthrough, &scratch_rule_keys, tables.rules)?;

    let terrain_row = tables
        .roster
        .terrains
        .get(preview.context.terrain_row_index)
        .ok_or_else(|| {
            EnemyError::InvalidInput(format!(
                "terrain row {} is outside {} terrain rows",
                preview.context.terrain_row_index,
                tables.roster.terrains.len()
            ))
        })?;

    Ok(AuxiliaryPreview {
        mode: preview.context.auxiliary_mode,
        mode_branch: preview.context.mode_branch,
        terrain: TerrainPreview {
            value: preview.context.terrain_value,
            display_effect_keys: terrain_display_effect_keys(
                terrain_row,
                preview.context.terrain_value,
            ),
            scoped_seed: crate::context::derive_terrain_seed(seed),
            used_filtered_pool: preview.context.used_filtered_pool,
            selected_row_index: preview.context.terrain_row_index,
        },
        descriptor_selector: preview.context.selector,
        descriptor_flags: preview.context.flags,
        enemy_groups,
        special_rules,
    })
}

/// Compose one variant's complete enemy-state preview.
///
/// A missing or partially captured input returns `possessed_complete == false`
/// with every occurrence `Possession::Unknown` and a non-empty
/// `missing_inputs`; it is never represented as a proven negative.
pub fn compose_enemy_state_preview(
    seed: u32,
    playthrough: u8,
    variant: MissionVariant,
    tables: &PreviewTables<'_>,
) -> Result<EnemyStatePreview, EnemyError> {
    let preview = generate_enemy_preview(
        seed,
        playthrough,
        variant,
        tables.roster,
        tables.context,
        tables.states,
    )?;

    let mut occurrences = Vec::new();
    for (index, occurrence) in preview.roster.occurrences().enumerate() {
        let possessed = preview
            .wraith
            .states
            .get(index)
            .copied()
            .unwrap_or(Possession::Unknown);
        occurrences.push(EnemyStateOccurrence {
            wave_index: occurrence.wave_index,
            position: occurrence.position,
            lookup_key: occurrence.lookup_key,
            role: occurrence.role,
            source_row_index: occurrence.source_row_index,
            availability: if occurrence.selector_class == 0 {
                OccurrenceAvailability::Base
            } else {
                OccurrenceAvailability::ExpeditionOnly
            },
            native_spawn_key: occurrence.native_spawn_key,
            possessed,
            curse_if_fresh_null_source_selector_runs: curse_conditional(
                playthrough,
                possessed,
                tables
                    .states
                    .curse_gates
                    .get(&occurrence.lookup_key)
                    .copied(),
            ),
        });
    }

    let mut missing_inputs = preview.wraith.missing.clone();
    missing_inputs.push(CURSE_MISSING_INPUT.to_string());

    Ok(EnemyStatePreview {
        seed,
        playthrough,
        variant,
        terrain: preview.roster.terrain,
        occurrences,
        possessed_complete: preview.wraith.exact,
        missing_inputs,
        curse_scope: CURSE_SCOPE,
    })
}

/// Compose the complete offline NG3 preview: auxiliary, both variants, capacity.
///
/// This entry point is the NG3 (`playthrough == 3`) product slice. Other
/// progressions keep [`compose_auxiliary_preview`] only, because the shipped
/// payload emits `enemy_states` as null for them.
pub fn compose_ng3_preview(
    seed: u32,
    playthrough: u8,
    tables: &PreviewTables<'_>,
) -> Result<Ng3PreviewComposition, EnemyError> {
    if playthrough != crate::sequence::NG3_PLAYTHROUGH {
        return Err(EnemyError::Unsupported(format!(
            "playthrough {playthrough} has no certified NG3 enemy-state preview"
        )));
    }
    Ok(Ng3PreviewComposition {
        auxiliary: compose_auxiliary_preview(seed, playthrough, tables)?,
        enemy_states: [
            compose_enemy_state_preview(seed, playthrough, MissionVariant::Solo, tables)?,
            compose_enemy_state_preview(seed, playthrough, MissionVariant::Expedition, tables)?,
        ],
        initial_challenge_capacity: generate_challenge_attempt_count(seed),
    })
}

/// Wave budgets of the unique context row behind `auxiliary_mode`.
fn wave_budgets(
    tables: &RosterTables,
    auxiliary_mode: u8,
) -> Result<[f32; WAVE_BUDGET_COUNT], EnemyError> {
    let matches: Vec<&[u8; 48]> = tables
        .contexts
        .iter()
        .filter(|row| row[CONTEXT_MODE_OFFSET] == auxiliary_mode)
        .collect();
    let context = match matches.as_slice() {
        [row] => *row,
        _ => {
            return Err(EnemyError::MissingData(format!(
                "auxiliary mode 0x{auxiliary_mode:02X} resolved to {} context rows",
                matches.len()
            )))
        }
    };
    let mut budgets = [0f32; WAVE_BUDGET_COUNT];
    for (index, budget) in budgets.iter_mut().enumerate() {
        *budget = f32::from_le_bytes([
            context[CONTEXT_BUDGET_OFFSET + 4 * index],
            context[CONTEXT_BUDGET_OFFSET + 4 * index + 1],
            context[CONTEXT_BUDGET_OFFSET + 4 * index + 2],
            context[CONTEXT_BUDGET_OFFSET + 4 * index + 3],
        ]);
    }
    Ok(budgets)
}

/// The conditional Curse bound, exactly as the shipped preview derives it.
///
/// This says nothing about the top-level selector having run, existing actor
/// flags, or the player's bonus; it only binds a Possessed occurrence to the
/// captured `enemy_weight244` evidence.
fn curse_conditional(
    playthrough: u8,
    possessed: Possession,
    gate: Option<CurseGate>,
) -> CurseConditional {
    if playthrough < 3 || possessed != Possession::Yes {
        return CurseConditional::Unknown;
    }
    match gate {
        Some(CurseGate::EnemyRowAbsent) => CurseConditional::Guaranteed,
        Some(CurseGate::Weight244(weight)) if weight > 0 => CurseConditional::Guaranteed,
        Some(CurseGate::Weight244(_)) => CurseConditional::Never,
        Some(CurseGate::MissingWeight244) | Some(CurseGate::Unknown) | None => {
            CurseConditional::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auxiliary::SpecialRuleTables;
    use crate::enemy::RosterTables;
    use crate::record::{ScrollEffect, ScrollRecord};
    use std::collections::BTreeMap;

    fn scroll_effect(slot: u8) -> ScrollEffect {
        ScrollEffect {
            slot,
            source_index: slot - 1,
            effect_id: 0x0001_0000 + u32::from(slot),
            roll_percent: 94,
            category_and_flags: 0x42,
            effect_flags: 0x84,
            candidate_count: 3,
            resolved_value: -7,
            prefix_word: 0xA051,
        }
    }

    fn scroll_record() -> ScrollRecord {
        ScrollRecord {
            seed: 1,
            record_type: crate::sequence::NG3_RECORD_TYPE,
            rarity: crate::sequence::RARITY_GROWING,
            playthrough: crate::sequence::NG3_PLAYTHROUGH,
            level: 180,
            effects: vec![scroll_effect(1), scroll_effect(2)],
            promoted_source_indexes: vec![0, 1],
            random_draws: 24,
            final_rng_state: 0x2FAC_1E69,
            terminal_is_special: false,
        }
    }

    fn empty_preview_tables<'a>(
        roster: &'a RosterTables,
        context: &'a ContextTables,
        rules: &'a SpecialRuleTables,
        states: &'a EnemyStateTables,
    ) -> PreviewTables<'a> {
        PreviewTables {
            roster,
            context,
            rules,
            states,
        }
    }

    #[test]
    fn effect_previews_pack_the_shipped_payload_fields() {
        let previews = effect_previews(&scroll_record());
        assert_eq!(previews.len(), 2);
        let first = previews[0];
        assert_eq!(first.slot, 1);
        assert_eq!(first.effect_id, 0x0001_0001);
        assert_eq!(first.value, -7, "resolved value is carried verbatim");
        assert_eq!(
            first.metadata,
            94 | (0x42u32 << 8) | (0x84u32 << 16),
            "metadata packs roll_percent, category and flags"
        );
        assert_eq!(first.prefix, 0xA051);
        assert_eq!(first.tail_0, 0);
        assert_eq!(first.tail_1, 0);
        assert_eq!(first.roll_percent, Some(94));
    }

    #[test]
    fn curse_conditions_need_a_possessed_ng3_occurrence_and_captured_evidence() {
        assert_eq!(
            curse_conditional(3, Possession::Yes, Some(CurseGate::Weight244(200))),
            CurseConditional::Guaranteed
        );
        assert_eq!(
            curse_conditional(3, Possession::Yes, Some(CurseGate::Weight244(0))),
            CurseConditional::Never
        );
        assert_eq!(
            curse_conditional(3, Possession::Yes, Some(CurseGate::EnemyRowAbsent)),
            CurseConditional::Guaranteed
        );
        assert_eq!(
            curse_conditional(3, Possession::Yes, Some(CurseGate::MissingWeight244)),
            CurseConditional::Unknown
        );
        assert_eq!(
            curse_conditional(3, Possession::Yes, None),
            CurseConditional::Unknown,
            "an uncaptured lookup is unknown, never a negative"
        );
        assert_eq!(
            curse_conditional(2, Possession::Yes, Some(CurseGate::Weight244(200))),
            CurseConditional::Unknown,
            "the conditional bound is NG3 only"
        );
        assert_eq!(
            curse_conditional(3, Possession::No, Some(CurseGate::Weight244(200))),
            CurseConditional::Unknown
        );
        assert_eq!(
            curse_conditional(3, Possession::Unknown, Some(CurseGate::Weight244(200))),
            CurseConditional::Unknown
        );
    }

    #[test]
    fn the_ng3_entry_point_rejects_other_progressions() {
        let roster = RosterTables {
            enemies: Vec::new(),
            contexts: Vec::new(),
            terrains: Vec::new(),
            terrain_keys: Vec::new(),
            parameter_types: BTreeMap::new(),
        };
        let context = ContextTables {
            contexts: Vec::new(),
            optional_multipliers: Vec::new(),
        };
        let rules = SpecialRuleTables {
            rules: Vec::new(),
            rule_keys: Vec::new(),
            conflicts: Vec::new(),
            conflict_keys: Vec::new(),
        };
        let states = EnemyStateTables {
            text_sha256: crate::enemy::ENEMY_TEXT_SHA256.to_string(),
            positions_by_terrain: BTreeMap::new(),
            eligibility: BTreeMap::new(),
            curse_gates: BTreeMap::new(),
            enemy_index_complete: false,
            config_4543: None,
        };
        let tables = empty_preview_tables(&roster, &context, &rules, &states);
        for playthrough in [0u8, 1, 2, 4, 5] {
            let error = compose_ng3_preview(1, playthrough, &tables)
                .expect_err("only NG3 has a certified enemy-state preview");
            assert!(
                error.to_string().contains("playthrough"),
                "unexpected error: {error}"
            );
        }
        // Empty tables must fail closed for the supported progression instead
        // of returning an empty-but-successful payload.
        assert!(compose_ng3_preview(1, 3, &tables).is_err());
        assert!(compose_auxiliary_preview(1, 3, &tables).is_err());
        assert!(compose_enemy_state_preview(1, 3, MissionVariant::Solo, &tables).is_err());
    }
}
