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
        GenerationError::NotHellCapable(_) => {
            HostError::rejected("This item never becomes a hell weapon")
        }
        GenerationError::UnsupportedHellRarity(_) => {
            HostError::rejected("A hell weapon has the rarity the character's progress gives it")
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

fn role(generator: &EquipmentGenerator<'_>, entry: &GeneratedEntry) -> &'static str {
    if generator.is_hell_effect(entry.effect_id) {
        "hell"
    } else if entry.flags & 1 != 0 {
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
fn quality(generator: &EquipmentGenerator<'_>, entries: &[GeneratedEntry; 7]) -> u32 {
    entries
        .iter()
        .filter(|entry| {
            entry.effect_id != EMPTY_ENTRY && matches!(role(generator, entry), "random" | "hell")
        })
        .map(|entry| u32::from(entry.roll) + if entry.flags & 4 != 0 { 200 } else { 0 })
        .sum()
}

/// The effects of a built record, in the order the game stores them.
fn record_effects(
    generator: &EquipmentGenerator<'_>,
    record: &[u8; GENERATED_RECORD_BYTES],
) -> Vec<Value> {
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
                "role": role(generator, &generated),
            }))
        })
        .collect()
}

struct Search {
    matches: u32,
    empty: u32,
    best: Vec<(u32, u16)>,
}

fn search(
    generator: &EquipmentGenerator<'_>,
    prepared: &PreparedItem<'_, '_>,
    wanted: &[Vec<u32>],
    limit: usize,
) -> Search {
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
                        found.best.push((quality(generator, &entries), seed as u16));
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
    let hell = params.get("hell").and_then(Value::as_bool) == Some(true);
    let prepared = prepare(generator, item_id, rarity, state, hell)?;
    let found = search(generator, &prepared, &wanted, limit);
    let mut outcomes = Vec::new();
    for (_, seed) in &found.best {
        let entries = prepared.generate(*seed);
        let record = prepared
            .build_record(&entries, *seed, level, plus)
            .map_err(generation_error)?;
        outcomes.push(json!({ "seed": seed, "effects": record_effects(generator, &record) }));
    }
    Ok(json!({
        "item_id": item_id,
        "rarity": rarity,
        "level": level,
        "difficulty": state.type_class,
        "hell": hell,
        "hell_skills": if hell { natural_hell_skills(data_root, item_id, level) } else { Vec::new() },
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
    let hell = requested.get("hell").and_then(Value::as_bool) == Some(true);
    let prepared = prepare(generator, item_id, rarity, state, hell)?;
    let entries = prepared.generate(seed);
    if entries.iter().all(|entry| entry.effect_id == EMPTY_ENTRY) {
        return Err(HostError::rejected(
            "This seed gives the item no effects; choose another",
        ));
    }
    if !hell {
        return prepared
            .build_record(&entries, seed, level, plus)
            .map_err(generation_error);
    }
    let skill = requested
        .get("hell_skill")
        .and_then(Value::as_u64)
        .and_then(|skill| u16::try_from(skill).ok())
        .filter(|skill| natural_hell_skills(data_root, item_id, level).contains(skill))
        .ok_or_else(|| {
            HostError::rejected("Choose a hell martial skill this weapon can have at this level")
        })?;
    prepared
        .build_hell_record(&entries, seed, level, plus, skill)
        .map_err(generation_error)
}

fn prepare<'g, 'a>(
    generator: &'g EquipmentGenerator<'a>,
    item_id: u16,
    rarity: u8,
    state: PlayerState,
    hell: bool,
) -> Result<PreparedItem<'g, 'a>, HostError> {
    if hell {
        generator.prepare_hell(item_id, rarity, state)
    } else {
        generator.prepare(item_id, rarity, state)
    }
    .map_err(generation_error)
}

/// Hell martial skills +0x2286D6C can draw for this weapon at this level.
fn natural_hell_skills(data_root: &Path, item_id: u16, level: u16) -> Vec<u16> {
    crate::equipment_rules::rules(data_root)
        .and_then(|rules| {
            let item = rules.item(item_id)?;
            Some(rules.hell_skills(item, level))
        })
        .unwrap_or_default()
}

/// Whether the record is exactly what the game generates from its own seed
/// (`+0x22`) for this item and rarity on a difficulty the character played:
/// every entry's group key, effect, roll, category and set/grace/star flags
/// in the stored order. Values follow the level at generation time and the marker
/// bits change with use, so neither is compared.
pub fn replays_from_seed(
    data_root: &Path,
    record: &[u8],
    progress: Option<&GenerationProgress>,
) -> bool {
    let (Ok(generator), Some(progress)) = (generator(data_root), progress) else {
        return false;
    };
    if record.len() != GENERATED_RECORD_BYTES {
        return false;
    }
    let item_id = u16::from_le_bytes([record[0], record[1]]);
    let seed = u16::from_le_bytes([record[0x22], record[0x23]]);
    let rarity = record[0x30];
    let level = u16::from_le_bytes([record[6], record[7]]);
    // A hell weapon is regenerated by the conversion; its skill is drawn
    // from the weapon's rows at the converted level.
    let hell = record[0x1A] & 0x10 != 0;
    if hell
        && !natural_hell_skills(data_root, item_id, level)
            .contains(&u16::from_le_bytes([record[0x10], record[0x11]]))
    {
        return false;
    }
    let same = |built: &[u8; GENERATED_RECORD_BYTES]| {
        (0..7).all(|index| {
            let at = 0x34 + index * 0x18;
            built[at..at + 2] == record[at..at + 2]
                && built[at + 4..at + 8] == record[at + 4..at + 8]
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
        let Ok(prepared) = prepare(generator, item_id, rarity, state, hell) else {
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn data_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
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

    /// Natural hell weapons from the owner's saves (2026-10-11/13): item, seed,
    /// level, the difficulty they replay on, skill, and the stored effects and
    /// rolls. The normal route reproduces none of them.
    type HellVector = (u16, u16, u16, u8, u16, &'static [(u32, u8)]);
    const OWNER_HELL_WEAPONS: [HellVector; 12] = [
        (
            0x145e,
            0x3edf,
            170,
            3,
            0xd6f9,
            &[
                (0x5b7f, 95),
                (0xe34e, 88),
                (0x6826, 92),
                (0x76d8, 94),
                (0x7bea, 0),
            ],
        ),
        (
            0xb881,
            0x1ecd,
            170,
            3,
            0xdd8a,
            &[
                (0x8641, 93),
                (0x48de, 89),
                (0x6800, 89),
                (0x1a1b, 83),
                (0xce68, 0),
            ],
        ),
        (
            0x3172,
            0xaad0,
            146,
            1,
            0x26d6,
            &[
                (0xe9b1, 87),
                (0xd8ea, 88),
                (0xbb13, 89),
                (0x776d, 84),
                (0xb24f, 0),
            ],
        ),
        (
            0x5b65,
            0x7bd1,
            149,
            1,
            0x8d12,
            &[
                (0x4feb, 95),
                (0xdc17, 87),
                (0x5305, 85),
                (0x6800, 91),
                (0x4fa3, 0),
            ],
        ),
        (
            0xa7cd,
            0x68fa,
            147,
            1,
            0x26d6,
            &[
                (0x21d3, 94),
                (0x1a2c, 93),
                (0x76d8, 95),
                (0xebf5, 94),
                (0x4fa3, 0),
            ],
        ),
        (
            0xbba1,
            0xb89c,
            148,
            1,
            0xdd8a,
            &[
                (0x3a53, 90),
                (0x4bd3, 82),
                (0x794b, 93),
                (0xc4c3, 90),
                (0x4fa3, 0),
            ],
        ),
        (
            0x166a,
            0x5073,
            147,
            1,
            0x3fb9,
            &[
                (0x89c2, 81),
                (0x36e6, 93),
                (0xdc17, 93),
                (0x510b, 88),
                (0x8ccc, 0),
            ],
        ),
        (
            0x7543,
            0xde64,
            150,
            1,
            0x6f70,
            &[
                (0xcaad, 88),
                (0xbb28, 88),
                (0x5305, 92),
                (0x510b, 83),
                (0x2ae6, 0),
            ],
        ),
        (
            0xd1fe,
            0xca06,
            144,
            1,
            0xb623,
            &[
                (0x4feb, 85),
                (0x510b, 88),
                (0x517d, 84),
                (0x88a9, 88),
                (0x23e5, 0),
            ],
        ),
        (
            0x825b,
            0xb67a,
            148,
            1,
            0x6122,
            &[
                (0x8641, 93),
                (0x3743, 84),
                (0x54f1, 93),
                (0xc07e, 87),
                (0xb24f, 0),
            ],
        ),
        (
            0xa696,
            0x42b5,
            148,
            1,
            0x99c0,
            &[
                (0x4feb, 91),
                (0xf437, 96),
                (0x6800, 90),
                (0x4ad3, 89),
                (0x590c, 0),
            ],
        ),
        (
            0x4877,
            0x2141,
            175,
            1,
            0xdd8a,
            &[
                (0x3c4a, 91),
                (0x76d8, 93),
                (0x6800, 83),
                (0xdc17, 86),
                (0xe8eb, 0),
            ],
        ),
    ];

    fn stored_effects(record: &[u8; GENERATED_RECORD_BYTES]) -> Vec<(u32, u8)> {
        (0..7)
            .map(|index| {
                let at = 0x34 + index * 0x18;
                let id = u32::from_le_bytes(record[at + 4..at + 8].try_into().unwrap());
                (id, record[at + 0x0C])
            })
            .filter(|(id, _)| *id != EMPTY_ENTRY)
            .collect()
    }

    #[test]
    fn the_hell_conversion_reproduces_natural_hell_weapons() {
        let root = data_root();
        let generator = generator(&root).unwrap();
        let progress = owner_progress();
        for (item, seed, level, difficulty, skill, stored) in OWNER_HELL_WEAPONS {
            let state = PlayerState {
                type_class: difficulty,
                progress: progress.progress(difficulty),
            };
            let hell = generator.prepare_hell(item, 4, state).unwrap();
            let entries = hell.generate(seed);
            let record = hell
                .build_hell_record(&entries, seed, level, 0, skill)
                .unwrap();
            assert_eq!(
                stored_effects(&record),
                stored,
                "item {item:#x} seed {seed:#x}"
            );
            assert!(generator.is_hell_effect(stored[0].0));
            assert_eq!(u16::from_le_bytes([record[0x10], record[0x11]]), skill);
            assert_eq!(record[0x1A] & 0x10, 0x10);
            assert!(replays_from_seed(&root, &record, Some(&progress)));
            let normal = generator.prepare(item, 4, state).unwrap();
            let built = normal
                .build_record(&normal.generate(seed), seed, level, 0)
                .unwrap();
            assert_ne!(stored_effects(&built), stored);
        }
    }

    /// The hell Bizen katana `0x4BF7` the game's own converter built on
    /// 2026-09-26 (seed `0x698C`, owner on difficulty 1 or 2): lightning
    /// imbue, strong-attack Ki cost, hell-weapon drop rate, mid-stance
    /// martial-art Ki damage and Hotei's grace, as the owner saw in game.
    #[test]
    fn the_natively_converted_bizen_katana_replays() {
        let root = data_root();
        let generator = generator(&root).unwrap();
        let progress = owner_progress();
        for difficulty in [1, 2] {
            let state = PlayerState {
                type_class: difficulty,
                progress: progress.progress(difficulty),
            };
            let hell = generator.prepare_hell(0x4BF7, 4, state).unwrap();
            let record = hell
                .build_hell_record(&hell.generate(0x698C), 0x698C, 170, 22, 0x3435)
                .unwrap();
            let effects: Vec<u32> = stored_effects(&record).iter().map(|e| e.0).collect();
            assert_eq!(effects, [0x8641, 0x511C, 0x9982, 0xD7EF, 0x590C]);
        }
    }

    #[test]
    fn a_hell_search_and_add_follow_the_conversion() {
        let root = data_root();
        let request = json!({
            "item_id": 0x4BF7, "rarity": 4, "level": 170, "difficulty": 3,
            "progress": [6510, 7710, 0, 7710], "limit": 3, "hell": true,
            "want": [0x8641],
        });
        let found = equipment_seeds_json(&root, &request).unwrap();
        assert_eq!(found["hell"], true);
        assert_eq!(found["hell_skills"], json!([0xC0C1, 0x3435, 0xAC06]));
        assert!(found["matches"].as_u64().unwrap() > 0);
        let outcome = &found["outcomes"][0];
        assert!(outcome["effects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|effect| effect["effect_id"] == 0x8641 && effect["role"] == "hell"));
        let state = PlayerState {
            type_class: 3,
            progress: [6510, 7710, 0, 7710],
        };
        let mut add = request.clone();
        add["seed"] = outcome["seed"].clone();
        add["plus"] = json!(20);
        add["hell_skill"] = json!(0x3435);
        let record = generated_record(&root, &add, state).unwrap();
        assert_eq!(u16::from_le_bytes([record[0x10], record[0x11]]), 0x3435);
        assert_eq!(record[0x1A] & 0x10, 0x10);
        assert!(replays_from_seed(&root, &record, Some(&owner_progress())));
        // A skill of another weapon type, a rarity the conversion never
        // gives and armor are refused.
        add["hell_skill"] = json!(0x8D12);
        assert!(generated_record(&root, &add, state).is_err());
        add.as_object_mut().unwrap().remove("hell_skill");
        assert!(generated_record(&root, &add, state).is_err());
        let mut rare = request.clone();
        rare["rarity"] = json!(5);
        assert!(equipment_seeds_json(&root, &rare).is_err());
        let generator = generator(&root).unwrap();
        let resource =
            nioh3_data::load_effect_resource_for_file_version(&root, (2, 0, 2, 0)).unwrap();
        let armor = resource
            .item
            .rows
            .chunks(resource.item.row_size)
            .find(|row| row[0x182] == 3)
            .map(|row| u16::from_le_bytes([row[0x152], row[0x153]]))
            .unwrap();
        assert!(!generator.hell_capable(armor));
        assert!(generator.prepare_hell(armor, 4, state).is_err());
    }
}
