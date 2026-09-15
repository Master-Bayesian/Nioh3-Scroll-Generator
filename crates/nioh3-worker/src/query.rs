//! Typed search query and the cross-field validation the shipped worker runs.
//!
//! Ports `worker_contracts.SearchQuery.from_payload` and the
//! `effect_seed_solver.EffectSeedRequest` invariants. Parameter *shape* is
//! already enforced by the shipped request schema (`crate::schema`); this module
//! owns the semantic checks that make one schema-valid query a real search and
//! binds the query to the digest the resume token is minted against.
//!
//! Checks whose reference implementation needs a product data primitive the
//! development worker does not load yet fail closed with an explicit message
//! instead of being silently skipped.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::protocol::RequestError;
use crate::schema::integral;

/// Roster variant an enemy-occurrence search runs against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyVariant {
    Solo,
    Expedition,
}

impl EnemyVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            EnemyVariant::Solo => "solo",
            EnemyVariant::Expedition => "expedition",
        }
    }
}

/// The six auxiliary requirement sets, in schema order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuxiliaryCriteria {
    pub required_terrain_effect_keys: Vec<u32>,
    pub required_terrain_effect_key_groups: Vec<Vec<u32>>,
    pub required_special_rule_keys: Vec<u32>,
    pub required_special_rule_key_groups: Vec<Vec<u32>>,
    pub required_enemy_lookup_keys: Vec<u32>,
    pub required_enemy_lookup_key_groups: Vec<Vec<u32>>,
}

/// Display-slot scope one occurrence requirement may occupy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccurrenceScope {
    Primary,
    Secondary,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectAlternative {
    pub effect_id: u32,
    pub minimum_roll_percent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectOccurrence {
    pub scope: OccurrenceScope,
    pub alternatives: Vec<EffectAlternative>,
}

/// Enemy occurrence state filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyStateFilter {
    Any,
    Possessed,
    Curse,
}

/// Enemy occurrence availability filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Any,
    Base,
    ExpeditionOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnemyOccurrenceItem {
    pub lookup_keys: Vec<u32>,
    pub state: EnemyStateFilter,
    pub availability: Availability,
}

/// One validated, digest-bound search query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub playthrough: u8,
    pub rarity: u8,
    pub level: u16,
    pub primary_effect_ids: Vec<u32>,
    pub required_secondary_ids: Vec<u32>,
    pub required_secondary_id_groups: Vec<Vec<u32>>,
    pub grace_effect_id: Option<u32>,
    /// Roll constraints outside any secondary any-of group.
    pub minimum_roll_percent_by_effect_id: Vec<(u32, u32)>,
    /// Roll constraints that belong to a secondary any-of group.
    pub grouped_rolls: Vec<(u32, u32)>,
    pub auxiliary: AuxiliaryCriteria,
    pub terrain_selection_ids: Vec<String>,
    pub initial_challenge_counts: Vec<u8>,
    pub grace_effect_ids: Vec<u32>,
    pub effect_occurrences: Vec<EffectOccurrence>,
    pub enemy_variant: EnemyVariant,
    pub enemy_occurrence_groups: Vec<Vec<EnemyOccurrenceItem>>,
    /// SHA-256 over the canonical query JSON, matching the shipped
    /// `json.dumps(payload, sort_keys=True, separators=(',', ':'))`.
    pub digest: String,
}

impl SearchQuery {
    /// Validate one schema-checked `query` object.
    pub fn from_payload(payload: &Value) -> Result<Self, RequestError> {
        let enemy_variant = match payload.get("enemy_variant").and_then(Value::as_str) {
            None | Some("solo") => EnemyVariant::Solo,
            Some("expedition") => EnemyVariant::Expedition,
            Some(_) => {
                return Err(RequestError::invalid_request_message(
                    "enemy variant must be solo or expedition",
                ))
            }
        };
        let enemy_occurrence_groups = parse_enemy_groups(payload)?;
        if enemy_occurrence_groups
            .iter()
            .flatten()
            .any(|item| item.state == EnemyStateFilter::Curse)
        {
            return Err(RequestError::invalid_request_message(
                "Curse changes between entries and cannot be searched exactly by Scroll ID",
            ));
        }
        if enemy_variant == EnemyVariant::Solo
            && enemy_occurrence_groups
                .iter()
                .flatten()
                .any(|item| item.availability == Availability::ExpeditionOnly)
        {
            return Err(RequestError::invalid_request_message(
                "Expedition-only enemies require the expedition preview variant",
            ));
        }

        let playthrough = u8_value(payload, "playthrough")?;
        let rarity = u8_value(payload, "rarity")?;
        let level = u16_value(payload, "level")?;
        if playthrough != 3
            && (enemy_variant != EnemyVariant::Solo || !enemy_occurrence_groups.is_empty())
        {
            return Err(RequestError::invalid_request_message(
                "Enemy-state and expedition filters are currently supported only in playthrough 3",
            ));
        }

        let auxiliary = parse_auxiliary(payload)?;
        let terrain_selection_ids = payload
            .get("terrain_selection_ids")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let rolls = parse_rolls(payload)?;
        let required_secondary_id_groups = u32_groups(payload, "required_secondary_id_groups")?;
        let required_secondary_ids = u32_list(payload, "required_secondary_ids")?;
        let primary_effect_ids = u32_list(payload, "primary_effect_ids")?;
        let grace_effect_id = match payload.get("grace_effect_id") {
            None | Some(Value::Null) => None,
            Some(value) => Some(u32_from(value)?),
        };
        let grace_effect_ids = u32_list(payload, "grace_effect_ids").unwrap_or_default();

        check_roll_constraints(
            &rolls,
            &primary_effect_ids,
            &required_secondary_ids,
            &required_secondary_id_groups,
        )?;
        check_secondary_groups(&required_secondary_id_groups, &required_secondary_ids)?;
        check_grace_filter(rarity, grace_effect_id, &grace_effect_ids)?;

        let grouped_ids: Vec<u32> = {
            let mut ids: Vec<u32> = required_secondary_id_groups
                .iter()
                .flatten()
                .copied()
                .collect();
            ids.sort_unstable();
            ids.dedup();
            ids
        };
        let grouped_rolls = rolls
            .iter()
            .copied()
            .filter(|(effect_id, _)| grouped_ids.contains(effect_id))
            .collect::<Vec<_>>();
        let plain_rolls = rolls
            .iter()
            .copied()
            .filter(|(effect_id, _)| !grouped_ids.contains(effect_id))
            .collect::<Vec<_>>();

        let initial_challenge_counts = payload
            .get("initial_challenge_counts")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| integral(value).and_then(|value| u8::try_from(value).ok()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(Self {
            playthrough,
            rarity,
            level,
            primary_effect_ids,
            required_secondary_ids,
            required_secondary_id_groups,
            grace_effect_id,
            minimum_roll_percent_by_effect_id: plain_rolls,
            grouped_rolls,
            auxiliary,
            terrain_selection_ids,
            initial_challenge_counts,
            grace_effect_ids,
            effect_occurrences: parse_occurrences(payload)?,
            enemy_variant,
            enemy_occurrence_groups,
            digest: query_digest(payload),
        })
    }
}

/// Canonical query digest. `serde_json::Map` is key-sorted by default and the
/// compact form uses the same separators as the shipped
/// `json.dumps(..., sort_keys=True, separators=(',', ':'))`.
pub fn query_digest(payload: &Value) -> String {
    let mut digest = Sha256::new();
    digest.update(serde_json::to_vec(payload).expect("query payload serializes"));
    crate::context::hex_lower(&digest.finalize())
}

fn u8_value(payload: &Value, key: &str) -> Result<u8, RequestError> {
    u32_from(payload.get(key).expect("schema requires the key"))
        .and_then(|value| u8::try_from(value).map_err(|_| invalid()))
}

fn u16_value(payload: &Value, key: &str) -> Result<u16, RequestError> {
    u32_from(payload.get(key).expect("schema requires the key"))
        .and_then(|value| u16::try_from(value).map_err(|_| invalid()))
}

fn u32_from(value: &Value) -> Result<u32, RequestError> {
    integral(value)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(invalid)
}

fn invalid() -> RequestError {
    RequestError::invalid_request()
}

fn u32_list(payload: &Value, key: &str) -> Result<Vec<u32>, RequestError> {
    let Some(values) = payload.get(key) else {
        return Ok(Vec::new());
    };
    let Some(values) = values.as_array() else {
        return Ok(Vec::new());
    };
    values.iter().map(u32_from).collect()
}

fn u32_groups(payload: &Value, key: &str) -> Result<Vec<Vec<u32>>, RequestError> {
    let values = payload
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    values
        .iter()
        .map(|group| {
            group
                .as_array()
                .map(|items| items.iter().map(u32_from).collect())
                .unwrap_or_else(|| Err(invalid()))
        })
        .collect()
}

fn parse_rolls(payload: &Value) -> Result<Vec<(u32, u32)>, RequestError> {
    let values = payload
        .get("minimum_roll_percent_by_effect_id")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut rolls = Vec::with_capacity(values.len());
    for pair in &values {
        let pair = pair.as_array().ok_or_else(invalid)?;
        let effect_id = pair.first().ok_or_else(invalid)?;
        let minimum_roll = pair.get(1).ok_or_else(invalid)?;
        rolls.push((u32_from(effect_id)?, u32_from(minimum_roll)?));
    }
    Ok(rolls)
}

fn parse_auxiliary(payload: &Value) -> Result<AuxiliaryCriteria, RequestError> {
    let auxiliary = payload.get("auxiliary").ok_or_else(invalid)?;
    Ok(AuxiliaryCriteria {
        required_terrain_effect_keys: u32_list(auxiliary, "required_terrain_effect_keys")?,
        required_terrain_effect_key_groups: u32_groups(
            auxiliary,
            "required_terrain_effect_key_groups",
        )?,
        required_special_rule_keys: u32_list(auxiliary, "required_special_rule_keys")?,
        required_special_rule_key_groups: u32_groups(
            auxiliary,
            "required_special_rule_key_groups",
        )?,
        required_enemy_lookup_keys: u32_list(auxiliary, "required_enemy_lookup_keys")?,
        required_enemy_lookup_key_groups: u32_groups(
            auxiliary,
            "required_enemy_lookup_key_groups",
        )?,
    })
}

fn parse_occurrences(payload: &Value) -> Result<Vec<EffectOccurrence>, RequestError> {
    let values = payload
        .get("effect_occurrences")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut occurrences = Vec::with_capacity(values.len());
    for occurrence in &values {
        let scope = match occurrence.get("scope").and_then(Value::as_str) {
            Some("primary") => OccurrenceScope::Primary,
            Some("secondary") => OccurrenceScope::Secondary,
            _ => OccurrenceScope::Any,
        };
        let alternatives = occurrence
            .get("alternatives")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut parsed = Vec::with_capacity(alternatives.len());
        for alternative in &alternatives {
            parsed.push(EffectAlternative {
                effect_id: u32_from(alternative.get("effect_id").ok_or_else(invalid)?)?,
                minimum_roll_percent: u32_from(
                    alternative
                        .get("minimum_roll_percent")
                        .ok_or_else(invalid)?,
                )?,
            });
        }
        occurrences.push(EffectOccurrence {
            scope,
            alternatives: parsed,
        });
    }
    Ok(occurrences)
}

fn parse_enemy_groups(payload: &Value) -> Result<Vec<Vec<EnemyOccurrenceItem>>, RequestError> {
    let groups = payload
        .get("enemy_occurrence_groups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut parsed = Vec::with_capacity(groups.len());
    for group in &groups {
        let items = group.as_array().ok_or_else(invalid)?;
        let mut parsed_items = Vec::with_capacity(items.len());
        for item in items {
            let lookup_keys = item
                .get("lookup_keys")
                .and_then(Value::as_array)
                .ok_or_else(invalid)?
                .iter()
                .map(u32_from)
                .collect::<Result<Vec<_>, _>>()?;
            let state = match item.get("state").and_then(Value::as_str) {
                Some("possessed") => EnemyStateFilter::Possessed,
                Some("curse") => EnemyStateFilter::Curse,
                _ => EnemyStateFilter::Any,
            };
            let availability = match item.get("availability").and_then(Value::as_str) {
                Some("base") => Availability::Base,
                Some("expedition_only") => Availability::ExpeditionOnly,
                _ => Availability::Any,
            };
            parsed_items.push(EnemyOccurrenceItem {
                lookup_keys,
                state,
                availability,
            });
        }
        parsed.push(parsed_items);
    }
    Ok(parsed)
}

/// `EffectSeedRequest.__post_init__` for the any-of groups.
fn check_secondary_groups(
    groups: &[Vec<u32>],
    required_secondary_ids: &[u32],
) -> Result<(), RequestError> {
    let mut grouped: Vec<u32> = Vec::new();
    for group in groups {
        if group.is_empty() {
            return Err(RequestError::invalid_request_message(
                "secondary any-of groups cannot be empty",
            ));
        }
        if let Some(overlap) = group.iter().find(|id| grouped.contains(id)) {
            return Err(RequestError::invalid_request_message(format!(
                "secondary any-of groups must not overlap ({overlap})"
            )));
        }
        grouped.extend_from_slice(group);
    }
    if let Some(overlap) = required_secondary_ids
        .iter()
        .find(|id| grouped.contains(id))
    {
        return Err(RequestError::invalid_request_message(format!(
            "a secondary effect cannot be both mandatory and in an any-of group ({overlap})"
        )));
    }
    Ok(())
}

/// `EffectSeedRequest.__post_init__` for the roll constraints.
fn check_roll_constraints(
    rolls: &[(u32, u32)],
    primary_effect_ids: &[u32],
    required_secondary_ids: &[u32],
    groups: &[Vec<u32>],
) -> Result<(), RequestError> {
    let grouped: Vec<u32> = groups.iter().flatten().copied().collect();
    let mut seen: Vec<u32> = Vec::new();
    for (effect_id, minimum_roll) in rolls {
        if seen.contains(effect_id) {
            return Err(RequestError::invalid_request_message(
                "roll constraints must contain unique effect IDs",
            ));
        }
        seen.push(*effect_id);
        let selected = primary_effect_ids.contains(effect_id)
            || required_secondary_ids.contains(effect_id)
            || grouped.contains(effect_id);
        if !selected {
            return Err(RequestError::invalid_request_message(
                "roll constraints require a selected effect ID",
            ));
        }
        if grouped.contains(effect_id) {
            return Err(RequestError::invalid_request_message(
                "secondary any-of groups currently require arbitrary values",
            ));
        }
        if *minimum_roll > 100 {
            return Err(RequestError::invalid_request_message(
                "minimum roll percent must be in 0..100",
            ));
        }
    }
    Ok(())
}

/// Grace-selection membership. The map-backed rarities fail closed until the
/// corresponding map primitive is ported.
fn check_grace_filter(
    rarity: u8,
    _grace_effect_id: Option<u32>,
    grace_effect_ids: &[u32],
) -> Result<(), RequestError> {
    if grace_effect_ids.is_empty() {
        return Ok(());
    }
    match rarity {
        4 | 5 => Err(RequestError::invalid_request_message(
            "Grace filtering needs the rarity-specific Grace output map, which this \
             development worker does not load yet",
        )),
        _ => Err(RequestError::invalid_request_message(
            "Grace choices do not belong to this rarity",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn query(extra: Value) -> Value {
        let mut base = json!({
            "playthrough": 3,
            "rarity": 4,
            "level": 180,
            "primary_effect_ids": [],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": null,
            "minimum_roll_percent_by_effect_id": [],
            "auxiliary": {
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": [64956, 113, 20893],
                "required_special_rule_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": []
            }
        });
        if let (Some(base), Some(extra)) = (base.as_object_mut(), extra.as_object()) {
            for (key, value) in extra {
                base.insert(key.clone(), value.clone());
            }
        }
        base
    }

    #[test]
    fn accepts_the_reported_three_rule_query() {
        let parsed = SearchQuery::from_payload(&query(json!({}))).expect("valid query");
        assert_eq!(parsed.playthrough, 3);
        assert_eq!(parsed.rarity, 4);
        assert_eq!(
            parsed.auxiliary.required_special_rule_keys,
            vec![64956, 113, 20893]
        );
        assert_eq!(parsed.enemy_variant, EnemyVariant::Solo);
        assert_eq!(parsed.digest.len(), 64);
    }

    #[test]
    fn digest_is_key_order_independent_and_value_sensitive() {
        let first = SearchQuery::from_payload(&query(json!({}))).expect("valid query");
        let reordered = json!({
            "level": 180,
            "rarity": 4,
            "playthrough": 3,
            "primary_effect_ids": [],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": null,
            "minimum_roll_percent_by_effect_id": [],
            "auxiliary": {
                "required_enemy_lookup_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_special_rule_key_groups": [],
                "required_special_rule_keys": [64956, 113, 20893],
                "required_terrain_effect_key_groups": [],
                "required_terrain_effect_keys": []
            }
        });
        let second = SearchQuery::from_payload(&reordered).expect("valid query");
        assert_eq!(first.digest, second.digest);
        let changed = SearchQuery::from_payload(&query(json!({"rarity": 5}))).expect("valid query");
        assert_ne!(first.digest, changed.digest);
    }

    #[test]
    fn auxiliary_criteria_and_enemy_groups_reach_the_compiler() {
        // The table-derived compiler resolves terrain option ids and compiles
        // enemy occurrence groups, so the parser carries both through untouched.
        let parsed = SearchQuery::from_payload(&query(json!({
            "terrain_selection_ids": ["exact:1"],
            "enemy_occurrence_groups": [[
                {"lookup_keys": [1], "state": "possessed", "availability": "any"}
            ]]
        })))
        .expect("terrain ids and enemy groups are the compiler's job");
        assert_eq!(parsed.terrain_selection_ids, vec!["exact:1".to_string()]);
        assert_eq!(parsed.enemy_occurrence_groups.len(), 1);
        assert_eq!(
            parsed.enemy_occurrence_groups[0][0].state,
            EnemyStateFilter::Possessed
        );
    }

    #[test]
    fn an_empty_map_grace_filter_fails_closed_with_a_named_message() {
        let grace = SearchQuery::from_payload(&query(json!({
            "grace_effect_ids": [5858]
        })))
        .expect_err("rarity-5/4 Grace filtering needs the output map");
        assert!(grace.message.contains("Grace output map"));
        assert_eq!(grace.code, "INVALID_REQUEST");
    }

    #[test]
    fn rejects_curse_and_expedition_only_mismatches() {
        let curse = SearchQuery::from_payload(&query(json!({
            "enemy_occurrence_groups": [[
                {"lookup_keys": [1], "state": "curse", "availability": "any"}
            ]]
        })))
        .expect_err("curse cannot be searched exactly");
        assert!(curse.message.contains("Curse changes between entries"));

        let expedition_only = SearchQuery::from_payload(&query(json!({
            "enemy_occurrence_groups": [[
                {"lookup_keys": [1], "state": "any", "availability": "expedition_only"}
            ]]
        })))
        .expect_err("solo cannot ask for expedition-only enemies");
        assert!(expedition_only
            .message
            .contains("expedition preview variant"));
    }

    #[test]
    fn rejects_duplicate_or_unselected_roll_constraints() {
        let duplicate = SearchQuery::from_payload(&query(json!({
            "primary_effect_ids": [1],
            "minimum_roll_percent_by_effect_id": [[1, 10], [1, 20]]
        })))
        .expect_err("roll effect ids must be unique");
        assert!(duplicate.message.contains("unique effect IDs"));

        let unselected = SearchQuery::from_payload(&query(json!({
            "minimum_roll_percent_by_effect_id": [[7, 10]]
        })))
        .expect_err("roll constraints need a selected effect");
        assert!(unselected.message.contains("require a selected effect ID"));
    }

    #[test]
    fn splits_grouped_rolls_out_of_the_plain_constraints() {
        let parsed = SearchQuery::from_payload(&query(json!({
            "required_secondary_id_groups": [[11, 12]],
            "required_secondary_ids": [13]
        })))
        .expect("a grouped secondary list is valid");
        assert!(parsed.grouped_rolls.is_empty());
        assert!(parsed.minimum_roll_percent_by_effect_id.is_empty());
        assert_eq!(parsed.required_secondary_id_groups, vec![vec![11, 12]]);
    }

    #[test]
    fn overlapping_groups_are_rejected() {
        let overlap = SearchQuery::from_payload(&query(json!({
            "required_secondary_id_groups": [[11, 12], [12, 13]]
        })))
        .expect_err("groups must not overlap");
        assert!(overlap.message.contains("must not overlap"));
    }
}
