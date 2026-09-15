//! Seed-level entry point with the retained product support boundary.

use crate::{
    context::{optional_threshold, resolve_context, ResolvedContext},
    enemy::{
        ContextTables, EnemyError, EnemyStateTables, MissionVariant, RosterInput, RosterResult,
        RosterTables, WraithResult,
    },
    roster::generate_roster,
    wraith::generate_wraith,
};

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
        optional_threshold(context_tables, 0xCEFC)?
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
