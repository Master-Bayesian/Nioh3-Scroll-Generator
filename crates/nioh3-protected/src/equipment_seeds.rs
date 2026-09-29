//! Seeded equipment generation for adding natural items (PC v2.02 tables).
//!
//! Every item the game can drop comes from one of 65,536 seeds. For one item,
//! rarity and player state this module runs the game's generator over all of
//! them, reports how many produce the effects the player wants, and builds the
//! exact record of the seed the player picks.

use std::path::Path;
use std::sync::OnceLock;

use nioh3_domain::effect::{EffectResourceBytes, EffectTableIndex};
use nioh3_domain::equipment_generation::{
    EquipmentGenerator, GeneratedEntry, GenerationError, PlayerState, PreparedItem, EMPTY_ENTRY,
    GENERATED_RECORD_BYTES, SEED_COUNT,
};
use nioh3_save::character::GenerationProgress;
use serde_json::{json, Value};

use crate::HostError;

static GENERATOR: OnceLock<Option<EquipmentGenerator<'static>>> = OnceLock::new();

/// Outcomes listed for one search, best first.
const DEFAULT_LIMIT: usize = 40;
const MAX_LIMIT: usize = 200;

fn load(data_root: &Path) -> Option<EquipmentGenerator<'static>> {
    let resource: &'static EffectResourceBytes = Box::leak(Box::new(
        nioh3_data::load_effect_resource_for_file_version(data_root, (2, 0, 2, 0)).ok()?,
    ));
    let index: &'static EffectTableIndex =
        Box::leak(Box::new(EffectTableIndex::from_resource(resource).ok()?));
    Some(EquipmentGenerator::new(resource, index))
}

fn generator(data_root: &Path) -> Result<&'static EquipmentGenerator<'static>, HostError> {
    GENERATOR
        .get_or_init(|| load(data_root))
        .as_ref()
        .ok_or_else(|| {
            HostError::rejected("Equipment generation is unavailable for this game version")
        })
}

fn generation_error(error: GenerationError) -> HostError {
    match error {
        GenerationError::UnknownItem(_) | GenerationError::NotEquipment(_) => {
            HostError::rejected("This item cannot be generated: it is not equipment")
        }
        GenerationError::UnsupportedRarity(_) => {
            HostError::rejected("This item cannot be generated at this rarity")
        }
        GenerationError::UnsupportedDifficulty(_) => {
            HostError::rejected("The difficulty must be 1 to 5")
        }
        GenerationError::Table(error) => HostError::rejected(format!("{error:?}")),
    }
}

fn param_u64(params: &Value, key: &str, max: u64) -> Result<u64, HostError> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value <= max)
        .ok_or_else(|| HostError::rejected(format!("{key} is missing or out of range")))
}

/// The state a request names: a difficulty and its progress vector.
fn requested_state(params: &Value) -> Result<PlayerState, HostError> {
    let type_class = param_u64(params, "difficulty", 5)? as u8;
    let progress = params
        .get("progress")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 4)
        .ok_or_else(HostError::invalid_request)?;
    let mut vector = [0u32; 4];
    for (slot, value) in vector.iter_mut().zip(progress) {
        *slot = value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(HostError::invalid_request)?;
    }
    Ok(PlayerState {
        type_class,
        progress: vector,
    })
}

/// The `generation` block of `save.character`: the difficulty the character
/// is on and the progress of each difficulty an item could have dropped on.
pub fn generation_json(progress: Option<&GenerationProgress>) -> Value {
    let Some(progress) = progress else {
        return Value::Null;
    };
    json!({
        "difficulty": progress.difficulty,
        "difficulties": progress
            .difficulties()
            .into_iter()
            .map(|difficulty| json!({
                "difficulty": difficulty,
                "progress": progress.progress(difficulty),
            }))
            .collect::<Vec<_>>(),
    })
}

/// The state a saved add uses, re-read from the save itself: the requested
/// difficulty must be one the character could have played.
pub fn state_from_save(
    progress: Option<&GenerationProgress>,
    requested: &Value,
) -> Result<PlayerState, HostError> {
    let progress = progress.ok_or_else(|| {
        HostError::rejected("The save's difficulty and progress could not be read")
    })?;
    let difficulty = requested
        .get("difficulty")
        .and_then(Value::as_u64)
        .map_or(progress.difficulty, |value| value.min(255) as u8);
    if !progress.difficulties().contains(&difficulty) {
        return Err(HostError::rejected(
            "The character has not played this difficulty",
        ));
    }
    Ok(PlayerState {
        type_class: difficulty,
        progress: progress.progress(difficulty),
    })
}

fn role(entry: &GeneratedEntry) -> &'static str {
    if entry.flags & 1 != 0 {
        "set"
    } else if entry.flags & 2 != 0 {
        "grace"
    } else if entry.marker & 0x40 != 0 {
        "innate"
    } else {
        "random"
    }
}

/// How good a seed's drawn effects are: their rolls, and a star above any roll.
fn quality(entries: &[GeneratedEntry; 7]) -> u32 {
    entries
        .iter()
        .filter(|entry| entry.effect_id != EMPTY_ENTRY && role(entry) == "random")
        .map(|entry| u32::from(entry.roll) + if entry.flags & 4 != 0 { 200 } else { 0 })
        .sum()
}

/// The effects of a built record, in the order the game stores them.
fn record_effects(record: &[u8; GENERATED_RECORD_BYTES]) -> Vec<Value> {
    (0..7)
        .filter_map(|index| {
            let at = 0x34 + index * 0x18;
            let entry = &record[at..at + 0x18];
            let effect_id = u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]);
            if effect_id == EMPTY_ENTRY {
                return None;
            }
            let generated = GeneratedEntry {
                effect_id,
                roll: entry[0x0C],
                marker: entry[0x0D],
                flags: entry[0x0E],
            };
            Some(json!({
                "effect_id": effect_id,
                "value": i32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]),
                "roll": entry[0x0C],
                "star": entry[0x0E] & 4 != 0,
                "role": role(&generated),
            }))
        })
        .collect()
}

struct Search {
    matches: u32,
    empty: u32,
    best: Vec<(u32, u16)>,
}

fn search(prepared: &PreparedItem<'_, '_>, wanted: &[Vec<u32>], limit: usize) -> Search {
    let threads = std::thread::available_parallelism()
        .map_or(4, usize::from)
        .clamp(1, 16);
    let chunk = (SEED_COUNT as usize).div_ceil(threads);
    let parts: Vec<Search> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|part| {
                scope.spawn(move || {
                    let mut found = Search {
                        matches: 0,
                        empty: 0,
                        best: Vec::new(),
                    };
                    let start = part * chunk;
                    let end = ((part + 1) * chunk).min(SEED_COUNT as usize);
                    for seed in start..end {
                        let entries = prepared.generate(seed as u16);
                        if entries.iter().all(|entry| entry.effect_id == EMPTY_ENTRY) {
                            found.empty += 1;
                            continue;
                        }
                        if !wanted
                            .iter()
                            .all(|any| entries.iter().any(|entry| any.contains(&entry.effect_id)))
                        {
                            continue;
                        }
                        found.matches += 1;
                        found.best.push((quality(&entries), seed as u16));
                        if found.best.len() > limit * 4 {
                            found
                                .best
                                .sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                            found.best.truncate(limit);
                        }
                    }
                    found
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle.join().unwrap_or(Search {
                    matches: 0,
                    empty: 0,
                    best: Vec::new(),
                })
            })
            .collect()
    });
    let mut total = Search {
        matches: 0,
        empty: 0,
        best: Vec::new(),
    };
    for part in parts {
        total.matches += part.matches;
        total.empty += part.empty;
        total.best.extend(part.best);
    }
    total
        .best
        .sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    total.best.truncate(limit);
    total
}

/// `runtime.equipment_seeds`: which seeds give one item every wanted effect.
pub fn equipment_seeds_json(data_root: &Path, params: &Value) -> Result<Value, HostError> {
    let generator = generator(data_root)?;
    let item_id = param_u64(params, "item_id", u64::from(u16::MAX))? as u16;
    let rarity = param_u64(params, "rarity", 5)? as u8;
    let level = param_u64(params, "level", u64::from(u16::MAX))? as u16;
    let plus = params
        .get("plus")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u64::from(u16::MAX)) as u16;
    let state = requested_state(params)?;
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .map_or(DEFAULT_LIMIT, |value| (value as usize).clamp(1, MAX_LIMIT));
    // Each wanted entry is an effect id, or ids that read the same (any one
    // of them counts).
    let id = |value: &Value| {
        value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(HostError::invalid_request)
    };
    let wanted = params
        .get("want")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| match value.as_array() {
                    Some(any) if !any.is_empty() && any.len() <= 16 => {
                        any.iter().map(id).collect::<Result<Vec<_>, _>>()
                    }
                    Some(_) => Err(HostError::invalid_request()),
                    None => id(value).map(|id| vec![id]),
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if wanted.len() > 7 {
        return Err(HostError::rejected("Ask for at most seven effects"));
    }
    let prepared = generator
        .prepare(item_id, rarity, state)
        .map_err(generation_error)?;
    let found = search(&prepared, &wanted, limit);
    let mut outcomes = Vec::new();
    for (_, seed) in &found.best {
        let entries = prepared.generate(*seed);
        let record = prepared
            .build_record(&entries, *seed, level, plus)
            .map_err(generation_error)?;
        outcomes.push(json!({ "seed": seed, "effects": record_effects(&record) }));
    }
    Ok(json!({
        "item_id": item_id,
        "rarity": rarity,
        "level": level,
        "difficulty": state.type_class,
        "seeds": SEED_COUNT,
        "empty": found.empty,
        "matches": found.matches,
        "outcomes": outcomes,
    }))
}

/// The in-memory record the game builds for one seed, for a saved add.
pub fn generated_record(
    data_root: &Path,
    requested: &Value,
    state: PlayerState,
) -> Result<[u8; GENERATED_RECORD_BYTES], HostError> {
    let generator = generator(data_root)?;
    let item_id = param_u64(requested, "item_id", u64::from(u16::MAX))? as u16;
    let rarity = param_u64(requested, "rarity", 5)? as u8;
    let level = param_u64(requested, "level", u64::from(u16::MAX))? as u16;
    let plus = requested
        .get("plus")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u64::from(u16::MAX)) as u16;
    let seed = param_u64(requested, "seed", u64::from(u16::MAX))? as u16;
    if requested.get("hell").and_then(Value::as_bool) == Some(true) {
        return Err(HostError::rejected(
            "A hell weapon cannot be generated from a seed",
        ));
    }
    let prepared = generator
        .prepare(item_id, rarity, state)
        .map_err(generation_error)?;
    let entries = prepared.generate(seed);
    if entries.iter().all(|entry| entry.effect_id == EMPTY_ENTRY) {
        return Err(HostError::rejected(
            "This seed gives the item no effects; choose another",
        ));
    }
    prepared
        .build_record(&entries, seed, level, plus)
        .map_err(generation_error)
}

/// Whether the record is exactly what the game generates from its own seed
/// (`+0x22`) for this item and rarity on a difficulty the character played:
/// every entry's effect, roll, category and set/grace/star flags in the
/// stored order. Values follow the level at generation time and the marker
/// bits change with use, so neither is compared.
pub fn replays_from_seed(
    data_root: &Path,
    record: &[u8],
    progress: Option<&GenerationProgress>,
) -> bool {
    let (Ok(generator), Some(progress)) = (generator(data_root), progress) else {
        return false;
    };
    if record.len() != GENERATED_RECORD_BYTES || record[0x1A] & 0x10 != 0 {
        return false;
    }
    let item_id = u16::from_le_bytes([record[0], record[1]]);
    let seed = u16::from_le_bytes([record[0x22], record[0x23]]);
    let rarity = record[0x30];
    let level = u16::from_le_bytes([record[6], record[7]]);
    let same = |built: &[u8; GENERATED_RECORD_BYTES]| {
        (0..7).all(|index| {
            let at = 0x34 + index * 0x18;
            built[at + 4..at + 8] == record[at + 4..at + 8]
                && built[at + 0x0C] == record[at + 0x0C]
                && built[at + 0x0D] & 0x3F == record[at + 0x0D] & 0x3F
                && built[at + 0x0E] & 7 == record[at + 0x0E] & 7
        })
    };
    progress.difficulties().into_iter().any(|difficulty| {
        let state = PlayerState {
            type_class: difficulty,
            progress: progress.progress(difficulty),
        };
        let Ok(prepared) = generator.prepare(item_id, rarity, state) else {
            return false;
        };
        let entries = prepared.generate(seed);
        !entries.iter().all(|entry| entry.effect_id == EMPTY_ENTRY)
            && prepared
                .build_record(&entries, seed, level, 0)
                .is_ok_and(|built| same(&built))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data")
    }

    #[test]
    fn a_search_counts_every_seed_and_lists_the_best() {
        let owner = json!({
            "item_id": 0x27BF, "rarity": 4, "level": 170, "difficulty": 3,
            "progress": [6510, 7710, 0, 7710], "limit": 5,
        });
        let started = std::time::Instant::now();
        let all = equipment_seeds_json(&data_root(), &owner).unwrap();
        assert!(
            started.elapsed().as_secs() < 20,
            "a search takes {:?}",
            started.elapsed()
        );
        let matches = all["matches"].as_u64().unwrap();
        let empty = all["empty"].as_u64().unwrap();
        assert_eq!(matches + empty, u64::from(SEED_COUNT));
        let outcomes = all["outcomes"].as_array().unwrap();
        assert_eq!(outcomes.len(), 5);
        // Asking for an effect of the best outcome finds it again.
        let first = &outcomes[0]["effects"][0]["effect_id"];
        let mut narrowed = owner.clone();
        narrowed["want"] = json!([first]);
        let some = equipment_seeds_json(&data_root(), &narrowed).unwrap();
        assert!(some["matches"].as_u64().unwrap() > 0);
        assert!(some["matches"].as_u64().unwrap() <= matches);
        // An effect no equipment carries is reported as absent.
        narrowed["want"] = json!([0xFFFE]);
        let none = equipment_seeds_json(&data_root(), &narrowed).unwrap();
        assert_eq!(none["matches"], 0);
        assert!(none["outcomes"].as_array().unwrap().is_empty());
    }

    fn owner_progress() -> GenerationProgress {
        GenerationProgress {
            difficulty: 3,
            counters: [
                [0, 6510, 6510, 6510, 0, 0, 0, 0],
                [0, 7710, 7710, 7710, 0, 0, 0, 0],
                [0; 8],
            ],
        }
    }

    /// Every item a seed gives, stored the way the inventory stores it,
    /// audits natural; one changed roll does not replay.
    #[test]
    fn seeded_records_audit_natural() {
        let root = data_root();
        let generator = generator(&root).unwrap();
        let progress = owner_progress();
        let state = PlayerState {
            type_class: 3,
            progress: progress.progress(3),
        };
        let resource =
            nioh3_data::load_effect_resource_for_file_version(&root, (2, 0, 2, 0)).unwrap();
        let items: Vec<u16> = resource
            .item
            .rows
            .chunks(resource.item.row_size)
            .filter(|row| matches!(row[0x182], 1 | 2 | 3 | 9))
            .map(|row| u16::from_le_bytes([row[0x152], row[0x153]]))
            .collect();
        let mut free = vec![0u8; GENERATED_RECORD_BYTES];
        for index in 0..7 {
            free[0x38 + index * 0x18..0x3C + index * 0x18].fill(0xFF);
        }
        let mut checked = 0;
        let mut replayed = 0;
        for (n, item) in items.iter().enumerate().step_by(5) {
            for rarity in 0..=5u8 {
                let Ok(prepared) = generator.prepare(*item, rarity, state) else {
                    continue;
                };
                let seed = (n as u16).wrapping_mul(977) ^ u16::from(rarity);
                let entries = prepared.generate(seed);
                if entries.iter().all(|entry| entry.effect_id == EMPTY_ENTRY) {
                    continue;
                }
                let built = prepared.build_record(&entries, seed, 170, 0).unwrap();
                let record = nioh3_save::character::build_generated_equipment_record(
                    &free, &built, 0x100, 0x5000,
                )
                .unwrap();
                let audit =
                    crate::equipment_rules::audit_json_replayed(&root, &record, Some(&progress));
                assert_eq!(
                    audit["natural"], true,
                    "item {item:#x} rarity {rarity} seed {seed:#x}: {audit}"
                );
                checked += 1;
                if audit.get("replayed").is_some() {
                    replayed += 1;
                }
                if let Some(slot) = (0..7).find(|slot| {
                    let at = 0x34 + slot * 0x18;
                    record[at + 0x0D] & 0x40 == 0 && record[at + 4..at + 8] != [0xFF; 4]
                }) {
                    let mut changed = record.clone();
                    changed[0x34 + slot * 0x18 + 0x0C] ^= 1;
                    assert!(!replays_from_seed(&root, &changed, Some(&progress)));
                }
            }
        }
        assert!(checked > 1000, "{checked}");
        assert!(replayed > 0);
    }
}
