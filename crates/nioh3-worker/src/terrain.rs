//! Player-visible terrain options and their exact row unions.
//!
//! Port of `catalog_application.terrain_choices` and
//! `resolve_terrain_selections`. The catalog publishes these option ids and the
//! search resolves them here, against the same context-bound terrain table, so
//! an option can never mean one row set on screen and another in a search.
//!
//! Semantics, exactly as the reference defines them:
//!
//! * `exact:<K1>,<K2>` is every row whose displayed effect combination is that
//!   ordered key tuple; `exact:` is the rows with no displayed terrain effect
//!   (it is a real choice, never "unrestricted").
//! * `contains:<K>` is the union of every combination containing key `K`; it is
//!   published only when more than one combination contains that key.
//! * Several selected ids are the union of their rows (OR). An empty selection
//!   list is no row restriction at all; explicit terrain keys and key groups
//!   still apply on top of a row union (AND).
//! * An unknown id is refused instead of being guessed.

use std::collections::BTreeSet;

use nioh3_domain::auxiliary::terrain_display_effect_keys;

/// Terrain `+0x30` magnitude byte inside one 52-byte terrain row.
pub const TERRAIN_VALUE_OFFSET: usize = 0x30;

/// One published terrain option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainChoice {
    /// The stable, locale-independent id the UI sends back.
    pub option_id: String,
    /// The displayed effect keys, in display order.
    pub keys: Vec<u16>,
    /// The terrain rows this option selects.
    pub rows: BTreeSet<u32>,
    /// Whether this is a `contains:` aggregate rather than an exact combination.
    pub aggregate: bool,
}

/// The displayed effect keys of one terrain row, as the catalog shows them.
fn row_keys(row: &[u8; 52]) -> Vec<u16> {
    terrain_display_effect_keys(row, row[TERRAIN_VALUE_OFFSET])
}

fn hex_keys(keys: &[u16]) -> String {
    keys.iter()
        .map(|key| format!("{key:X}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// `catalog_application.terrain_choices`, in the reference's insertion order:
/// every exact combination by first occurrence, then the `contains:` aggregates
/// in ascending key order.
pub fn terrain_choices(rows: &[[u8; 52]]) -> Vec<TerrainChoice> {
    let mut combinations: Vec<(Vec<u16>, BTreeSet<u32>)> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let keys = row_keys(row);
        let index = index as u32;
        match combinations
            .iter_mut()
            .find(|(existing, _)| *existing == keys)
        {
            Some((_, members)) => {
                members.insert(index);
            }
            None => combinations.push((keys, BTreeSet::from([index]))),
        }
    }
    let mut choices: Vec<TerrainChoice> = combinations
        .iter()
        .map(|(keys, members)| TerrainChoice {
            option_id: format!("exact:{}", hex_keys(keys)),
            keys: keys.clone(),
            rows: members.clone(),
            aggregate: false,
        })
        .collect();
    let aggregate_keys: BTreeSet<u16> = combinations
        .iter()
        .flat_map(|(keys, _)| keys.iter().copied())
        .collect();
    for key in aggregate_keys {
        let matching: Vec<&BTreeSet<u32>> = combinations
            .iter()
            .filter(|(keys, _)| keys.contains(&key))
            .map(|(_, members)| members)
            .collect();
        if matching.len() <= 1 {
            continue;
        }
        choices.push(TerrainChoice {
            option_id: format!("contains:{key:X}"),
            keys: vec![key],
            rows: matching.into_iter().flatten().copied().collect(),
            aggregate: true,
        });
    }
    choices
}

/// `catalog_application.resolve_terrain_selections`: the sorted row union of
/// the selected option ids. An empty selection resolves to no rows (no
/// restriction); an unknown id is an error, never a guess.
pub fn resolve_terrain_selections(
    rows: &[[u8; 52]],
    selection_ids: &[String],
) -> Result<Vec<u32>, String> {
    if selection_ids.is_empty() {
        return Ok(Vec::new());
    }
    let choices = terrain_choices(rows);
    let mut selected: BTreeSet<u32> = BTreeSet::new();
    for selection in selection_ids {
        let choice = choices
            .iter()
            .find(|choice| &choice.option_id == selection)
            .ok_or_else(|| {
                "Unknown terrain option; refresh the context-bound catalog".to_string()
            })?;
        selected.extend(choice.rows.iter().copied());
    }
    if selected.is_empty() {
        // Every published option owns at least one row, so an empty union can
        // only mean an inconsistent table; it must never widen to "any terrain".
        return Err("the selected terrain options resolve to no terrain row".to_string());
    }
    Ok(selected.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One synthetic terrain row: `crucible` sets the `+0x2C` word and
    /// `value` is the `+0x30` magnitude.
    fn row(crucible: bool, value: u8) -> [u8; 52] {
        let mut row = [0u8; 52];
        if crucible {
            row[0x2C] = 1;
        }
        row[TERRAIN_VALUE_OFFSET] = value;
        row
    }

    fn table() -> Vec<[u8; 52]> {
        vec![
            row(false, 0),    // 0: no effect
            row(true, 0),     // 1: crucible only (0x24)
            row(true, 0x2D),  // 2: crucible + 0x39
            row(false, 0x2D), // 3: 0x39 only
            row(false, 0),    // 4: no effect again
            row(true, 0xD8),  // 5: crucible + 0x58
        ]
    }

    #[test]
    fn choices_follow_the_reference_order_and_row_unions() {
        let choices = terrain_choices(&table());
        let ids: Vec<&str> = choices.iter().map(|c| c.option_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "exact:",
                "exact:24",
                "exact:24,39",
                "exact:39",
                "exact:24,58",
                "contains:24",
                "contains:39",
            ]
        );
        assert_eq!(choices[0].rows, BTreeSet::from([0, 4]));
        assert_eq!(choices[5].rows, BTreeSet::from([1, 2, 5]));
        assert_eq!(choices[6].rows, BTreeSet::from([2, 3]));
        // 0x58 appears in one combination only, so it has no aggregate.
        assert!(!ids.contains(&"contains:58"));
    }

    #[test]
    fn selections_resolve_to_the_exact_row_union() {
        let rows = table();
        assert_eq!(resolve_terrain_selections(&rows, &[]), Ok(Vec::new()));
        // `exact:` is the no-effect rows, never "unrestricted".
        assert_eq!(
            resolve_terrain_selections(&rows, &["exact:".to_string()]),
            Ok(vec![0, 4])
        );
        assert_eq!(
            resolve_terrain_selections(
                &rows,
                &["exact:24,58".to_string(), "contains:39".to_string()]
            ),
            Ok(vec![2, 3, 5])
        );
        assert!(resolve_terrain_selections(&rows, &["exact:1".to_string()])
            .unwrap_err()
            .contains("Unknown terrain option"));
    }
}
