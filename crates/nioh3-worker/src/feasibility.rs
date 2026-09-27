//! The shipped structural preflight, ported once for every route.
//!
//! Port of `effect_seed_solver.validate_effect_request_feasibility`, which the
//! shipped worker runs while it builds a `search.start` request
//! (`worker_contracts`). It rejects combinations that cannot fit the native
//! PC v2.00.02 layout before a Seed family is opened: ordinary-slot count,
//! missing or ungeneratable table rows, effect conflicts, the rarity-5 deep
//! slot, and category capacity.
//!
//! This is deliberately one guard in the query layer rather than a rule inside a
//! route: a structural refusal must not depend on which route a request would
//! have taken, and a route must never answer a request the product refuses.

use nioh3_domain::effect::{EffectTableIndex, NativeWeightContext};

use crate::grace_map::CATEGORY_TO_TYPE;
use crate::query::SearchQuery;

/// The promoted/deep slot flag (`normalization_flags & 0x08`).
const PROMOTED_SLOT_FLAG: u32 = 0x08;

/// `validate_effect_request_feasibility`: reject a structurally impossible
/// request with the shipped reason, or return the first reason it fails.
pub fn validate_query_feasibility(
    query: &SearchQuery,
    tables: &EffectTableIndex,
) -> Result<(), String> {
    let Some(max_secondaries) = max_secondaries(query) else {
        return Ok(());
    };
    let record_type = match CATEGORY_TO_TYPE.get(usize::from(query.playthrough)) {
        Some(record_type) => *record_type,
        None => return Ok(()),
    };
    let capacities = tables
        .category_capacities(record_type, query.rarity)
        .map_err(|error| format!("category capacities: {error:?}"))?;
    let primary_options: Vec<Option<u32>> = if query.primary_effect_ids.is_empty() {
        vec![None]
    } else {
        sorted_unique(&query.primary_effect_ids)
            .into_iter()
            .map(Some)
            .collect()
    };
    let grouped_sets = grouped_choice_sets(query);
    let mut errors: Vec<Option<String>> = Vec::new();
    for grouped in &grouped_sets {
        let options: Vec<Option<u32>> = if query.primary_effect_ids.is_empty() {
            let mut anywhere: Vec<u32> = sorted_unique(&query.required_secondary_ids);
            anywhere.extend(grouped.iter().copied());
            anywhere.sort_unstable();
            anywhere.dedup();
            let mut options = vec![None];
            options.extend(anywhere.into_iter().map(Some));
            options
        } else {
            primary_options.clone()
        };
        for primary in options {
            errors.push(validate_option(
                query,
                tables,
                record_type,
                &capacities,
                max_secondaries,
                primary,
                grouped,
            ));
        }
    }
    if !errors.is_empty() && errors.iter().all(Option::is_some) {
        let detail = errors
            .into_iter()
            .flatten()
            .next()
            .unwrap_or_else(|| "unspecified structural conflict".to_string());
        return Err(format!(
            "the selected effect combination has no solution in the native generation \
             structure: {detail}"
        ));
    }
    Ok(())
}

/// `{3: 3, 4: 3 if grace else 4, 5: 4}`.
fn max_secondaries(query: &SearchQuery) -> Option<usize> {
    match query.rarity {
        3 => Some(3),
        4 => Some(if query.grace_effect_id.is_some() {
            3
        } else {
            4
        }),
        // NG1/NG2 rarity 5 carries six ordinary effects and no Grace.
        5 if query.playthrough < 3 => Some(5),
        5 => Some(4),
        _ => None,
    }
}

/// `product(*(sorted(group) for group in groups))`, or one empty combination.
fn grouped_choice_sets(query: &SearchQuery) -> Vec<Vec<u32>> {
    if query.required_secondary_id_groups.is_empty() {
        return vec![Vec::new()];
    }
    let mut sets: Vec<Vec<u32>> = vec![Vec::new()];
    for group in &query.required_secondary_id_groups {
        let mut next: Vec<Vec<u32>> = Vec::new();
        for prefix in &sets {
            for effect_id in sorted_unique(group) {
                let mut combination = prefix.clone();
                combination.push(effect_id);
                next.push(combination);
            }
        }
        sets = next;
    }
    sets
}

/// `effective_required_secondary_ids`: the primary satisfies one duplicated
/// requirement when it was also selected as an ordinary effect.
fn effective_required_secondary_ids(query: &SearchQuery, primary_id: u32) -> Vec<u32> {
    let mut secondaries = query.required_secondary_ids.clone();
    if secondaries.contains(&primary_id)
        && (query.primary_effect_ids.is_empty() || query.primary_effect_ids.contains(&primary_id))
    {
        secondaries.retain(|effect_id| *effect_id != primary_id);
    }
    secondaries
}

/// One `validate_option` branch; `Some(reason)` means this option is impossible.
#[allow(clippy::too_many_arguments)]
fn validate_option(
    query: &SearchQuery,
    tables: &EffectTableIndex,
    record_type: u16,
    capacities: &[u16; 32],
    max_secondaries: usize,
    primary_id: Option<u32>,
    grouped: &[u32],
) -> Option<String> {
    let mut effective: Vec<u32> = match primary_id {
        Some(primary) => effective_required_secondary_ids(query, primary),
        None => query.required_secondary_ids.clone(),
    };
    effective.extend(grouped.iter().copied());
    effective.sort_unstable();
    effective.dedup();
    if let Some(primary) = primary_id {
        if query.primary_effect_ids.is_empty() {
            effective.retain(|effect_id| *effect_id != primary);
        }
    }
    if effective.len() > max_secondaries {
        return Some(format!(
            "it needs {} ordinary secondary slots but this structure has only {}",
            effective.len(),
            max_secondaries
        ));
    }
    let mut ordinary: Vec<u32> = effective.clone();
    if let Some(primary) = primary_id {
        ordinary.push(primary);
    }
    let mut all: Vec<u32> = ordinary.clone();
    if let Some(grace) = query.grace_effect_id {
        all.push(grace);
    }
    all.sort_unstable();
    all.dedup();
    for effect_id in &all {
        if !tables.effects_by_id.contains_key(&(*effect_id as u16)) {
            return Some(format!(
                "effect 0x{effect_id:04X} is not in the native parameter table"
            ));
        }
        if ordinary.contains(effect_id) {
            let allowed =
                match tables.candidate_context_allowed(*effect_id as u16, record_type, false) {
                    Ok(allowed) => allowed,
                    Err(error) => {
                        return Some(format!(
                        "effect 0x{effect_id:04X} cannot be checked against the native context: \
                         {error:?}"
                    ))
                    }
                };
            if !allowed {
                return Some(format!(
                    "effect 0x{effect_id:04X} cannot be generated for this scroll type"
                ));
            }
            let weight = match tables.native_effect_weight(
                *effect_id as u16,
                NativeWeightContext {
                    record_type,
                    rarity: query.rarity,
                    playthrough: query.playthrough,
                    restricted_destination_slot: false,
                    extra_selector: 0,
                    rarity5_type_floor: 0,
                },
            ) {
                Ok(weight) => weight,
                Err(error) => {
                    return Some(format!(
                        "effect 0x{effect_id:04X} has no native weight for this context: {error:?}"
                    ))
                }
            };
            if weight == 0 {
                return Some(format!(
                    "effect 0x{effect_id:04X} has weight 0 for this playthrough and rarity"
                ));
            }
        }
    }
    // NG1/NG2 rarity 5 never draws a promoted (star) effect in any slot: none of
    // 20000 live native records carries one, and the generic composition that
    // matches them byte for byte agrees.
    if query.rarity == 5 && query.playthrough < 3 {
        let promoted: Vec<String> = all
            .iter()
            .filter(|effect_id| {
                tables
                    .effects_by_id
                    .get(&(**effect_id as u16))
                    .is_some_and(|definition| {
                        definition.normalization_flags & PROMOTED_SLOT_FLAG != 0
                    })
            })
            .map(|effect_id| format!("0x{effect_id:04X}"))
            .collect();
        if !promoted.is_empty() {
            return Some(format!(
                "playthrough-{} rarity-5 scrolls never carry the promoted effect {}",
                query.playthrough,
                promoted.join("、")
            ));
        }
    } else if query.rarity == 5 {
        let promoted_only: Vec<u32> = effective
            .iter()
            .copied()
            .filter(|effect_id| {
                tables
                    .effects_by_id
                    .get(&(*effect_id as u16))
                    .is_some_and(|definition| {
                        definition.normalization_flags & PROMOTED_SLOT_FLAG != 0
                    })
            })
            .collect();
        if !promoted_only.is_empty() {
            let formatted = promoted_only
                .iter()
                .map(|effect_id| format!("0x{effect_id:04X}"))
                .collect::<Vec<String>>()
                .join("、");
            return Some(format!(
                "rarity 5 has a single deep slot and it becomes the primary, so the selected \
                 secondary {formatted} can only appear in that slot"
            ));
        }
    }
    for (index, left) in all.iter().enumerate() {
        for right in all.iter().skip(index + 1) {
            let compatible = match tables.is_compatible(*left as u16, &[*right], None) {
                Ok(compatible) => compatible,
                Err(error) => {
                    return Some(format!(
                        "effects 0x{left:04X} and 0x{right:04X} could not be checked for native \
                         conflicts: {error:?}"
                    ))
                }
            };
            if !compatible {
                return Some(format!(
                    "0x{left:04X} and 0x{right:04X} belong to native conflict groups and cannot \
                     appear together"
                ));
            }
        }
    }
    let mut counts = [0u32; 32];
    let mut members: [Vec<u32>; 32] = std::array::from_fn(|_| Vec::new());
    for effect_id in &ordinary {
        let group = match tables.group_for_effect_u32(*effect_id) {
            Ok(group) => group,
            Err(error) => {
                return Some(format!(
                    "effect 0x{effect_id:04X} has no native category: {error:?}"
                ))
            }
        };
        let category = usize::from(group.category_key);
        if category >= capacities.len() {
            return Some(format!(
                "effect 0x{effect_id:04X} has an unknown native category"
            ));
        }
        counts[category] += 1;
        members[category].push(*effect_id);
    }
    for (category, count) in counts.iter().enumerate() {
        if *count > u32::from(capacities[category]) {
            // Name the members so the interface can say which effects clash.
            let effects = members[category]
                .iter()
                .map(|effect_id| format!("0x{effect_id:04X}"))
                .collect::<Vec<String>>()
                .join("、");
            return Some(format!(
                "the selected effects {effects} share native category 0x{category:02X}, which \
                 holds at most {}, but {count} were selected",
                capacities[category]
            ));
        }
    }
    None
}

fn sorted_unique(values: &[u32]) -> Vec<u32> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn tables() -> EffectTableIndex {
        let bytes =
            nioh3_data::load_effect_resource(&repo_root().join("nioh3_scroll_editor").join("data"))
                .expect("effect resource loads");
        EffectTableIndex::from_resource(&bytes).expect("effect tables decode")
    }

    fn query(overrides: serde_json::Value) -> SearchQuery {
        let mut payload = json!({
            "playthrough": 3,
            "rarity": 3,
            "level": 180,
            "primary_effect_ids": [],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": null,
            "minimum_roll_percent_by_effect_id": [],
            "auxiliary": {
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": [],
                "required_special_rule_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": [],
            },
        });
        for (key, value) in overrides.as_object().unwrap() {
            payload[key] = value.clone();
        }
        SearchQuery::from_payload(&payload).expect("the query shape is valid")
    }

    /// The two combinations the M2.3d vectors pin down: one the shipped layer
    /// searches, and one it refuses because the only deep slot would have to
    /// hold a secondary.
    #[test]
    fn rarity5_deep_slot_combinations_are_refused_structurally() {
        let tables = tables();
        let feasible = query(json!({
            "rarity": 5,
            "primary_effect_ids": [20781],
            "required_secondary_ids": [6410, 12028, 28203, 41127],
            "grace_effect_id": 0x6553,
        }));
        assert_eq!(validate_query_feasibility(&feasible, &tables), Ok(()));
        let deep_only = query(json!({
            "rarity": 5,
            "primary_effect_ids": [41041],
            "required_secondary_ids": [13555, 15994, 44634, 54282],
            "grace_effect_id": 0x6553,
        }));
        let error = validate_query_feasibility(&deep_only, &tables)
            .expect_err("the deep-slot-only set must be refused");
        assert!(error.contains("deep slot"), "{error}");
        assert!(error.contains("no solution"), "{error}");
    }

    /// Too many ordinary selections for the structure is refused before any
    /// route is chosen.
    #[test]
    fn too_many_secondaries_are_refused() {
        let tables = tables();
        let overflow = query(json!({
            "rarity": 3,
            "primary_effect_ids": [60020],
            "required_secondary_ids": [12028, 16437, 39485, 17991],
        }));
        let error = validate_query_feasibility(&overflow, &tables)
            .expect_err("four secondaries cannot fit a rarity-3 layout");
        assert!(error.contains("ordinary secondary slots"), "{error}");
    }

    /// An effect id outside the native table is refused by name.
    #[test]
    fn unknown_effect_ids_are_refused() {
        let tables = tables();
        let unknown = query(json!({
            "rarity": 3,
            "primary_effect_ids": [65000],
            "required_secondary_ids": [],
        }));
        let error = validate_query_feasibility(&unknown, &tables)
            .expect_err("an id outside the table must be refused");
        assert!(
            error.contains("not in the native parameter table"),
            "{error}"
        );
    }

    fn tables_v202() -> EffectTableIndex {
        let bytes = nioh3_data::load_effect_resource_for_file_version(
            &repo_root().join("nioh3_scroll_editor").join("data"),
            (2, 0, 2, 0),
        )
        .expect("v2.02 effect resource loads");
        EffectTableIndex::from_resource(&bytes).expect("effect tables decode")
    }

    /// No live native NG1/NG2 record may be refused: a query naming exactly a
    /// record's own primary and ordinary secondaries must pass the preflight.
    #[test]
    fn every_live_ng12_record_passes_its_own_preflight() {
        let tables = tables_v202();
        let fixture: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                repo_root().join("crates/nioh3-data/tests/fixtures/ng12_native_records_v202.json"),
            )
            .expect("fixture"),
        )
        .expect("fixture json");
        let mut checked = 0;
        for context in fixture["contexts"].as_array().expect("contexts") {
            let playthrough = context["playthrough"].as_u64().expect("playthrough");
            let rarity = context["rarity"].as_u64().expect("rarity");
            for entry in context["records"].as_object().expect("records").values() {
                let hex = entry
                    .as_str()
                    .or_else(|| entry["final"].as_str())
                    .expect("record");
                let record: Vec<u8> = (0..hex.len())
                    .step_by(2)
                    .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("hex"))
                    .collect();
                // Ordinary effects: skip empty slots, the rarity-3 token and Graces.
                let ordinary: Vec<u32> = (0..7)
                    .map(|slot| &record[0x34 + slot * 0x18..0x34 + (slot + 1) * 0x18])
                    .filter(|entry| entry[0xE] & 0x02 == 0)
                    .map(|entry| u32::from_le_bytes(entry[4..8].try_into().expect("id")))
                    .filter(|effect_id| *effect_id != u32::MAX && *effect_id != 1)
                    .collect();
                let query = query(json!({
                    "playthrough": playthrough,
                    "rarity": rarity,
                    "primary_effect_ids": [ordinary[0]],
                    "required_secondary_ids": ordinary[1..],
                }));
                assert_eq!(
                    validate_query_feasibility(&query, &tables),
                    Ok(()),
                    "NG{playthrough} R{rarity} {ordinary:04X?} occurs natively"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 24);
    }

    /// Impossible NG1/NG2 requests are refused by name: two effects of one
    /// capacity-1 category (never together in 10000 native NG2 R4 records) and a
    /// promoted effect in rarity 5, which NG1/NG2 never draws.
    #[test]
    fn impossible_ng12_requests_are_refused() {
        let tables = tables_v202();
        let category = validate_query_feasibility(
            &query(json!({
                "playthrough": 2,
                "rarity": 4,
                "primary_effect_ids": [0x6E2B],
                "required_secondary_ids": [0x6CE3],
            })),
            &tables,
        )
        .expect_err("the pair never occurs");
        assert!(category.contains("share native category"), "{category}");
        let star = tables
            .effects_in_row_order
            .iter()
            // A promoted effect the NG1 rarity-5 context weighs, so only the
            // promotion rule can refuse it.
            .find(|effect| {
                effect.normalization_flags & PROMOTED_SLOT_FLAG != 0
                    && effect.effect_id != 0
                    && tables
                        .candidate_context_allowed(effect.effect_id, 0x1E82, false)
                        .unwrap_or(false)
                    && tables
                        .native_effect_weight(
                            effect.effect_id,
                            NativeWeightContext {
                                record_type: 0x1E82,
                                rarity: 5,
                                playthrough: 1,
                                restricted_destination_slot: false,
                                extra_selector: 0,
                                rarity5_type_floor: 0,
                            },
                        )
                        .unwrap_or(0)
                        > 0
            })
            .expect("a promoted effect exists")
            .effect_id;
        let promoted = validate_query_feasibility(
            &query(json!({
                "playthrough": 1,
                "rarity": 5,
                "required_secondary_ids": [star],
            })),
            &tables,
        )
        .expect_err("NG1 rarity 5 never draws a promoted effect");
        assert!(
            promoted.contains("never carry the promoted effect"),
            "{promoted}"
        );
    }
}
