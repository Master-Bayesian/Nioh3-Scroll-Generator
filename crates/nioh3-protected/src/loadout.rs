//! The outfit search over the player's own items.
//!
//! An installed attack bundle lives in `data/loadout/<id>/`, with metadata
//! naming the form, native weapon TYPE, attack and buff scenario. Its immutable
//! scene template is application data; current player inputs are acquired by
//! loadout_context and bound independently of any private research profile.
//!
//! The renderer names which equipment slot each of the player's items goes to;
//! the host reads the record from the save itself and refuses an item whose
//! type class cannot go to that slot, so a wrong mapping fails instead of
//! searching a backpack the game would not allow.

#[path = "loadout_exact_search.rs"]
mod exact_search;

use crate::app::JobContext;
use nioh3_loadout::search_control::{SearchObserver, SearchProgress};
use std::path::{Path, PathBuf};

use nioh3_loadout::engine::{
    position_of, slot_of, Backpack, Candidate, Engine, Facts, Limits, Outcome, ITEM, SLOTS,
};
use nioh3_loadout::search::{
    Backend, Request, Search, SetOption, SetRule, SourceAdmissionPolicy, SET_REDUCTION_GROUP,
};
use nioh3_save::character::{
    equipment_record, equipment_slot_is_empty, EQUIPMENT_NOT_WORN, EQUIPMENT_SET_OFFSETS,
    EQUIPMENT_SLOT_COUNT,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::HostError;

/// Highest rarity the bundles were built for; rarer records use effect rules the paths never saw.
const MAX_RARITY: u8 = 5;
const MAX_ITEMS: usize = EQUIPMENT_SLOT_COUNT;
const SHOWN: usize = 20;
/// Bounds on a set request: every rule's options together name at most this many sets, so the class split stays small.
const MAX_RULES: usize = 4;
const MAX_SET_KEYS: usize = 6;
/// Largest search space on the graphics card and on the CPU.
const CARD_SPACE_LIMIT: u64 = 3_000_000_000;
const CPU_SPACE_LIMIT: u64 = 30_000_000;

/// Armour type classes: five slots, three weights each.
const ARMOR_FIRST: u32 = 24;
const ARMOR_LAST: u32 = 38;
const SAMURAI_ACCESSORY: u32 = 39;
const NINJA_ACCESSORY: u32 = 40;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Stance {
    High,
    Mid,
    Low,
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum AttackKind {
    Quick,
    Strong,
    Martial,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BuffScene {
    Current,
    Untriggered,
    Deflect,
    Arts,
    DeflectArts,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AttackScene {
    stance: Stance,
    kind: AttackKind,
    attack_id: String,
    buff: BuffScene,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Bundle {
    id: String,
    pub(crate) name: String,
    weapon: String,
    damage: String,
    pub(crate) school: String,
    /// TYPE with an installed candidate source contract; None alone is not
    /// permission to silently narrow a requested TYPE to its captured weapon.
    pub(crate) weapon_class: Option<u32>,
    /// Requested TYPE identity, including a scene whose source scope is narrower.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) selection_weapon_class: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scene: Option<AttackScene>,
    /// Retained engineering scenarios remain inspectable but are not ordinary attack choices.
    diagnostic: bool,
    /// Alternative buff scenes can share the attack's read-only evaluator files.
    #[serde(skip_serializing)]
    assets: Option<String>,
}

impl Bundle {
    fn requested_weapon_class(&self) -> Option<u32> {
        self.selection_weapon_class.or(self.weapon_class)
    }

    fn source_admission(&self) -> SourceAdmissionPolicy {
        if self.weapon_class.is_none() && self.selection_weapon_class.is_some() {
            SourceAdmissionPolicy::CapturedWeaponOnly
        } else {
            SourceAdmissionPolicy::OwnedType
        }
    }

    pub(crate) fn directory(&self, data_root: &Path) -> PathBuf {
        data_root
            .join("loadout")
            .join(self.assets.as_ref().unwrap_or(&self.id))
    }

    pub(crate) fn buff_scene(&self) -> BuffScene {
        self.scene
            .as_ref()
            .map_or(BuffScene::Current, |scene| scene.buff)
    }
}

fn valid_bundle_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn bundles(data_root: &Path) -> Vec<Bundle> {
    let Ok(entries) = std::fs::read_dir(data_root.join("loadout")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        let Ok(text) = std::fs::read_to_string(dir.join("bundle.json")) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let (Some(id), Some(name), Some(school)) = (
            meta["id"].as_str(),
            meta["name"].as_str(),
            meta["school"].as_str(),
        ) else {
            continue;
        };
        let weapon = &meta["weapon_class"];
        let weapon_class = weapon.as_u64().and_then(|class| u32::try_from(class).ok());
        if weapon_class.is_none() && !weapon.is_null() {
            continue;
        }
        if !matches!(school, "samurai" | "ninja")
            || !valid_bundle_id(id)
            || dir.file_name().and_then(|name| name.to_str()) != Some(id)
            || weapon_class.is_some_and(|class| class >= ARMOR_FIRST)
        {
            continue;
        }
        let selection = &meta["selection_weapon_class"];
        let selection_weapon_class = selection
            .as_u64()
            .and_then(|class| u32::try_from(class).ok());
        if (!selection.is_null() && selection_weapon_class.is_none())
            || selection_weapon_class.is_some_and(|class| {
                class >= ARMOR_FIRST
                    || weapon_class.is_some_and(|candidate_class| candidate_class != class)
            })
        {
            continue;
        }
        let scene = if let Some(scene) = meta.get("scene") {
            let Ok(scene) = serde_json::from_value::<AttackScene>(scene.clone()) else {
                continue;
            };
            if !valid_bundle_id(&scene.attack_id) {
                continue;
            }
            Some(scene)
        } else {
            None
        };
        let assets = meta["assets"].as_str().map(str::to_owned);
        if (!meta["assets"].is_null() && assets.is_none())
            || assets.as_deref().is_some_and(|name| !valid_bundle_id(name))
        {
            continue;
        }
        out.push(Bundle {
            id: id.to_string(),
            name: name.to_string(),
            weapon: meta["weapon"].as_str().unwrap_or(name).to_string(),
            damage: meta["damage"].as_str().unwrap_or(name).to_string(),
            school: school.to_string(),
            weapon_class,
            selection_weapon_class,
            scene,
            diagnostic: meta["diagnostic"].as_bool().unwrap_or(false),
            assets,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub(crate) fn find_bundle(data_root: &Path, params: &Value) -> Result<Bundle, HostError> {
    let id = params["bundle"]
        .as_str()
        .ok_or_else(|| HostError::rejected("missing bundle"))?;
    bundles(data_root)
        .into_iter()
        .find(|b| b.id == id)
        .ok_or_else(|| HostError::rejected("LOADOUT_BUNDLE_MISSING"))
}

/// Whether an item of `class` may be worn in equipment `slot` for this bundle.
fn slot_allows(bundle: &Bundle, class: u32, slot: u32) -> bool {
    match slot {
        0 => bundle.requested_weapon_class() == Some(class),
        4..=8 => {
            (ARMOR_FIRST..=ARMOR_LAST).contains(&class) && (class - ARMOR_FIRST) / 3 + 4 == slot
        }
        // Either accessory slot takes any accessory of the bundle's school; the two schools do not share them.
        9 | 10 => {
            class
                == if bundle.school == "ninja" {
                    NINJA_ACCESSORY
                } else {
                    SAMURAI_ACCESSORY
                }
        }
        _ => false,
    }
}

/// One exact owned record address and its attack scenario search slot.
#[derive(Clone, Copy, Debug)]
pub struct OwnedSearchItem {
    pub slot_index: usize,
    pub slot: u32,
}

/// Derive the complete request from the saved records using the same rules as
/// production request decoding. Both accessory positions share slot 9's pool.
pub fn all_owned_search_items(
    data_root: &Path,
    plain: &[u8],
    bundle_id: &str,
) -> Result<Vec<OwnedSearchItem>, HostError> {
    let bundle = find_bundle(data_root, &json!({"bundle": bundle_id}))?;
    let mut items = Vec::new();
    for slot_index in 0..nioh3_save::character::EQUIPMENT_SLOT_COUNT {
        let record = equipment_record(plain, slot_index).map_err(HostError::from_save)?;
        if equipment_slot_is_empty(record) || record[0x30] > MAX_RARITY {
            continue;
        }
        let key = u16::from_le_bytes([record[0], record[1]]);
        if let Some(class) = crate::item_kinds::type_class(data_root, key) {
            if let Some(slot) = [0, 4, 5, 6, 7, 8, 9]
                .into_iter()
                .find(|&slot| slot_allows(&bundle, class, slot))
            {
                items.push(OwnedSearchItem { slot_index, slot });
            }
        }
    }
    Ok(items)
}

/// One input item: where it is in the save, what it is, and the slot the bundle's school wears it in now, if any.
#[derive(Clone, Copy, Debug)]
struct Chosen {
    slot_index: usize,
    item_id: u16,
    worn: Option<u32>,
}

/// The player's chosen items, read from the save and checked against their slots.
/// Returns the backpack and, per input item, its save slot index and item id.
/// With `enhanced`, each record is raised to what the blacksmith can make of it before the search sees it.
fn backpack(
    data_root: &Path,
    bundle: &Bundle,
    plain: &[u8],
    params: &Value,
    enhanced: Option<&Engine>,
) -> Result<(Backpack, Vec<Chosen>), HostError> {
    let items = params["items"]
        .as_array()
        .ok_or_else(|| HostError::rejected("missing items"))?;
    if items.len() > MAX_ITEMS {
        return Err(HostError::rejected("too many items"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut chosen = Vec::with_capacity(items.len());
    let mut origin = Vec::with_capacity(items.len());
    for item in items {
        let (Some(slot_index), Some(slot)) = (item["slot_index"].as_u64(), item["slot"].as_u64())
        else {
            return Err(HostError::rejected("item needs slot_index and slot"));
        };
        // JSON integers must not wrap into an admitted native slot or save row.
        let slot = u32::try_from(slot).map_err(|_| HostError::rejected("LOADOUT_ITEM_SLOT"))?;
        let slot_index =
            usize::try_from(slot_index).map_err(|_| HostError::rejected("LOADOUT_ITEM_SLOT"))?;
        if !seen.insert(slot_index) {
            return Err(HostError::rejected("LOADOUT_ITEM_REPEATED"));
        }
        let record = equipment_record(plain, slot_index).map_err(HostError::from_save)?;
        if equipment_slot_is_empty(record) {
            return Err(HostError::rejected("LOADOUT_ITEM_EMPTY"));
        }
        let Some(mut bytes) = record
            .get(..ITEM)
            .and_then(|b| <[u8; ITEM]>::try_from(b).ok())
        else {
            return Err(HostError::rejected("LOADOUT_ITEM_EMPTY"));
        };
        let item_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        let class = crate::item_kinds::type_class(data_root, item_id);
        if bytes[0x30] > MAX_RARITY || !class.is_some_and(|c| slot_allows(bundle, c, slot)) {
            return Err(HostError::rejected(format!(
                "LOADOUT_ITEM_SLOT: slot_index {slot_index} item 0x{item_id:04X} class {class:?} rarity {} slot {slot}",
                bytes[0x30]
            )));
        }
        let worn_word = EQUIPMENT_SET_OFFSETS[usize::from(bundle.school == "ninja")];
        let word = u32::from_le_bytes([
            record[worn_word],
            record[worn_word + 1],
            record[worn_word + 2],
            record[worn_word + 3],
        ]);
        let worn = (word != EQUIPMENT_NOT_WORN).then_some(word);
        if let Some(engine) = enhanced {
            engine.enhance(&mut bytes);
        }
        chosen.push((slot, bytes));
        origin.push(Chosen {
            slot_index,
            item_id,
            worn,
        });
    }
    let pack =
        Backpack::from_slots(&chosen).map_err(|_| HostError::rejected("LOADOUT_ITEM_SLOT"))?;
    Ok((pack, origin))
}

/// `save.loadout_bundles`: the attacks a search can be run for.
pub fn list(data_root: &Path) -> Value {
    json!({"bundles": bundles(data_root)})
}

#[path = "loadout_catalog.rs"]
mod catalog;

/// Ordinary discovery includes a bounded page of all known action identities.
pub fn list_with_query(data_root: &Path, params: &Value) -> Result<Value, HostError> {
    let mut result = list(data_root);
    result["catalog"] = catalog::page(data_root, params)?;
    if serde_json::to_vec(&result)
        .map_err(|_| HostError::invalid_request())?
        .len()
        > nioh3_worker::MAX_FRAME_BYTES as usize - 16_384
    {
        return Err(HostError::rejected("LOADOUT_CATALOG_PAGE_TOO_LARGE"));
    }
    Ok(result)
}

/// `save.loadout_sets`: the sets the chosen items hold, with their member items and the set effect that names them.
pub fn sets(
    data_root: &Path,
    plain: &[u8],
    params: &Value,
    engine: &Engine,
) -> Result<Value, HostError> {
    let bundle = find_bundle(data_root, params)?;

    let (pack, origin) = backpack(data_root, &bundle, plain, params, None)?;
    let search = Search::new(engine, &pack);
    let mut rows: Vec<Value> = search
        .sets()
        .iter()
        .map(|(key, by_slot)| {
            let rows = search.set_rows(*key);
            let members: Vec<u16> = rows
                .iter()
                .map(|&r| origin[pack.origin[r as usize]].item_id)
                .collect();
            // The effect line that carries the set's side key is the set effect itself (a set's or a grace's name).
            let effect = rows.iter().find_map(|&r| {
                engine
                    .tables()
                    .effect_lines(&pack.pool[r as usize][..])
                    .into_iter()
                    .find(|l| l.2 == *key)
                    .map(|l| l.0)
            });
            json!({
                "key": key,
                "pieces": by_slot.iter().map(|e| e.1).sum::<usize>(),
                // Two accessory slots draw from one pool, so accessories hold up to two pieces wherever they were put.
                "slots": by_slot.iter().filter(|e| e.0 < 9).count()
                    + by_slot.iter().filter(|e| e.0 >= 9).map(|e| e.1).sum::<usize>().min(2),
                "item_ids": members,
                "effect": effect,
            })
        })
        .collect();
    // A pinned weapon or other unsearched slot can supply a real set even
    // when none of the replacement-pool records carry it. Offer that native
    // membership as a constraint; exact candidate qualification still decides.
    let current = Candidate {
        attrs: engine.start_attrs,
        items: [None; SLOTS],
    };
    let retained = engine.equipment_set_state(&current, &pack, &mut engine.work());
    let current_records = engine
        .encoded_context()
        .and_then(|context| context.file("characters.bin"))
        .and_then(|bytes| nioh3_loadout::pipeline::Frame::characters(bytes).ok())
        .and_then(|characters| characters.into_iter().next());
    match &retained {
        Ok(state) => {
            for &(key, count) in &state.pieces {
                if rows
                    .iter()
                    .any(|row| row["key"].as_u64() == Some(u64::from(key)))
                {
                    continue;
                }
                let mut members = Vec::new();
                let mut effect = None;
                if let Some(character) = &current_records {
                    for bank in 0..2 {
                        for slot in 0..17 {
                            let start = bank * 0xf70 + slot * ITEM;
                            let record = &character.inv[start..start + ITEM];
                            let lines = engine.tables().effect_lines(record);
                            if let Some(line) = lines.iter().find(|line| line.2 == key) {
                                let id = u16::from_le_bytes([record[0], record[1]]);
                                if !members.contains(&id) {
                                    members.push(id);
                                }
                                effect.get_or_insert(line.0);
                            }
                        }
                    }
                }
                rows.push(json!({"key":key,"pieces":count,"slots":count,
                "item_ids":members,"effect":effect}));
            }
        }
        Err(reason) => eprintln!("Retained set catalog unavailable: {reason}"),
    }
    rows.sort_by_key(|row| row["key"].as_u64().unwrap_or(0));
    let mut reduction = pack
        .pool
        .iter()
        .filter(|item| {
            engine
                .tables()
                .effect_lines(&item[..])
                .iter()
                // The native set-reward branch tests presence, including zero.
                .any(|l| l.1 == SET_REDUCTION_GROUP)
        })
        .count();
    if reduction == 0 && retained.as_ref().is_ok_and(|state| state.reduction) {
        reduction = 1;
    }
    Ok(
        json!({"bundle": bundle.id, "context_id": params["context_id"], "sets": rows, "reduction_items": reduction}),
    )
}

/// `save.loadout_search`: the best outfits of the chosen items for the bundle's attack.
pub fn search(
    data_root: &Path,
    plain: &[u8],
    params: &Value,
    engine: &Engine,
) -> Result<Value, HostError> {
    search_with_observer(
        data_root,
        plain,
        params,
        engine,
        None,
        crate::loadout_context::SearchAuthority::BundleDefault,
    )
}

struct HostSearchObserver<'a> {
    context: &'a JobContext,
    scope: &'a Value,
}
impl SearchObserver for HostSearchObserver<'_> {
    fn cancelled(&self) -> bool {
        self.context.cancelled()
    }
    fn progress(&self, progress: SearchProgress) {
        self.context.progress(json!({
            "schema":"nioh3-loadout-progress/v1", "bundle":self.scope["bundle"],
            "save_id":self.scope["save_id"], "source_sha256":self.scope["source_sha256"],
            "context_id":self.scope["context_id"], "client_request_id":self.scope["client_request_id"], "stage":progress.stage.label(),
            "backend":progress.backend.label(), "completed":progress.completed,
            "total":progress.total, "batch_completed":progress.batch_completed,
            "batch_total":progress.batch_total, "batch_index":progress.batch_index,
            "attempt":progress.attempt,
        }));
    }
}

pub fn search_observed(
    data_root: &Path,
    plain: &[u8],
    params: &Value,
    engine: &Engine,
    context: &JobContext,
) -> Result<Value, HostError> {
    search_observed_with_authority(
        data_root,
        plain,
        params,
        engine,
        context,
        crate::loadout_context::SearchAuthority::BundleDefault,
    )
}

pub(crate) fn search_observed_with_authority(
    data_root: &Path,
    plain: &[u8],
    params: &Value,
    engine: &Engine,
    context: &JobContext,
    authority: crate::loadout_context::SearchAuthority,
) -> Result<Value, HostError> {
    let observer = HostSearchObserver {
        context,
        scope: params,
    };
    observer.progress(SearchProgress::new(
        nioh3_loadout::search_control::SearchStage::Planning,
        nioh3_loadout::search_control::SearchBackend::Unselected,
        0,
        None,
    ));
    let result = search_with_observer(data_root, plain, params, engine, Some(&observer), authority);
    // Serialization and current-outfit evaluation also belong to this action.
    match result {
        Ok(mut value) => {
            if context.cancelled() {
                Err(HostError::rejected("LOADOUT_SEARCH_CANCELLED"))
            } else {
                if let Some(request_id) = params.get("client_request_id") {
                    value["client_request_id"] = request_id.clone();
                }
                Ok(value)
            }
        }
        Err(error) => Err(error),
    }
}

fn search_with_observer(
    data_root: &Path,
    plain: &[u8],
    params: &Value,
    engine: &Engine,
    observer: Option<&dyn SearchObserver>,
    authority: crate::loadout_context::SearchAuthority,
) -> Result<Value, HostError> {
    let bundle = find_bundle(data_root, params)?;

    let enhanced = params["enhanced"].as_bool().unwrap_or(false);
    let (pack, mut origin) = backpack(
        data_root,
        &bundle,
        plain,
        params,
        enhanced.then_some(engine),
    )?;
    // Keep the physical spare assignments before aligning the current
    // outfit display to the scene. ExactSweep checks the displaced records.
    let assigned_spares: Vec<_> = pack
        .ranges
        .iter()
        .filter(|range| range.0 == 0)
        .flat_map(|&(_, first, count)| first..first + count)
        .filter_map(|row| {
            let slot = origin[pack.origin[row as usize]].worn?;
            (1..=3).contains(&slot).then_some((row, slot as usize))
        })
        .collect();
    if engine.encoded_context().is_some() {
        bind_context_worn_items(engine, plain, &mut origin)?;
    }
    let sets = set_rules(&params["sets"])?;
    if sets.is_empty() {
        return Err(HostError::rejected("LOADOUT_SET_REQUIRED"));
    }
    let limits = &params["limits"];
    let request = Request {
        sets,
        limits: Limits {
            max_load: limits["max_load"]
                .as_f64()
                .map_or(f32::INFINITY, |v| v as f32),
            min_defense: limits["min_defense"].as_u64().map_or(0, |v| v as u32),
            min_health: limits["min_health"].as_u64().map_or(0, |v| v as u32),
        },
        move_points: params["move_points"].as_bool().unwrap_or(false),
        threads: 1,
    };
    let source_admission = match authority {
        crate::loadout_context::SearchAuthority::BundleDefault => bundle.source_admission(),
        crate::loadout_context::SearchAuthority::VerifiedNativeEpoch
        | crate::loadout_context::SearchAuthority::CurrentProgressBranch => {
            SourceAdmissionPolicy::OwnedType
        }
    };
    validate_source_search(source_admission, request.move_points)?;
    let weapon_domain = weapon_domain(&bundle, engine, &pack, params, source_admission);
    let keep_current = params["keep_current"].as_bool().unwrap_or(false);
    let exact = source_admission == SourceAdmissionPolicy::OwnedType
        && engine.has_dynamic_sources()
        && !request.move_points
        && !enhanced
        && !keep_current;
    let (space, report, excluded, mut details_work, swaps, execution_receipt) = if exact {
        let completed = exact_search::run(engine, &pack, request, &assigned_spares, observer)?;
        (
            completed.planned,
            completed.report,
            completed.excluded,
            completed.work,
            assigned_spares,
            Some(completed.execution_receipt),
        )
    } else {
        let search = Search::new(engine, &pack)
            .with_source_admission(source_admission)
            .with_cpu_fallback_limit(CPU_SPACE_LIMIT);
        let search = if params["keep_current"].as_bool().unwrap_or(false) {
            search.with_current_choices(if enhanced {
                nioh3_loadout::search::CurrentChoicePolicy::Enhanced
            } else {
                nioh3_loadout::search::CurrentChoicePolicy::Captured
            })
        } else {
            search
        };

        // Resolve the budget after planning so a small certified search can
        // run without opening the graphics-card kernel.
        let mut limit = CPU_SPACE_LIMIT;
        let limit_for = |plan: &nioh3_loadout::search::Plan| {
            if !plan.prefers_bounded_cpu() && engine.has_card() {
                limit = CARD_SPACE_LIMIT;
            }
            limit
        };
        let result = match observer {
            Some(observer) => search
                .run_limited_by_observed(&request, limit_for, observer)
                .map_err(|error| HostError::rejected(error.to_string()))?,
            None => search.run_limited_by(&request, limit_for),
        };
        let (space, report) = result.ok_or_else(|| {
            HostError::rejected(if limit == CPU_SPACE_LIMIT {
                "LOADOUT_SPACE_TOO_LARGE_CPU"
            } else {
                "LOADOUT_SPACE_TOO_LARGE"
            })
        })?;
        (space, report, 0, engine.work(), Vec::new(), None)
    };
    if let Some(failure) = &report.failure {
        return Err(HostError::rejected(failure.to_string()));
    }
    if std::env::var_os("NIOH3_E2E_PROFILE").is_some_and(|flag| !flag.is_empty() && flag != "0") {
        eprintln!(
            "loadout damage bounds: complete={} certified={} below_cutoff={} searched={}",
            report.complete, report.counts.certified, report.counts.bound_pruned, report.searched,
        );
    }

    let best: Vec<Value> = report
        .best
        .iter()
        .take(SHOWN)
        .map(|(s, facts)| {
            // Serialize exactly the scored slot assignment. Search completeness
            // is not a proof that exchanging accessories preserves damage.
            let worn_items = s.candidate.items;
            let items: Vec<Value> = (0..SLOTS)
                .filter_map(|p| {
                    worn_items[p].map(|row| {
                        let c = origin[pack.origin[row as usize]];
                        json!({"slot": slot_of(p), "slot_index": c.slot_index, "item_id": c.item_id})
                    })
                })
                .collect();
            let details = publication_damage_details(
                engine.damage_details(&s.candidate, &pack, &mut details_work), s.bits,
            )?;
            let mut row = json!({
                "damage": s.damage(),
                "damage_details":details,
                "attrs": s.candidate.attrs,
                "items": items,
                "facts": facts.map(facts_json),
            });
            if let Some(weapon) = nioh3_loadout::engine::position_of(0).and_then(|i| s.candidate.items[i]) {
                if let Some(&(_, slot)) = swaps.iter().find(|&&(owned, _)| owned == weapon) {
                    row["move_previous_weapon_to_slot"] = json!(slot);
                }
            }
            Ok(row)
        })
        .collect::<Result<Vec<_>, HostError>>()?;
    let current = current_outfit(engine, &pack, &origin, source_admission)?;
    let mut result = json!({
        "bundle": bundle.id,
        "context_id": params["context_id"],
        "start_attrs": engine.start_attrs,
        "best": best,
        "space": space,
        "full_size": report.full_size,
        "source_uncomputed": report.source_uncomputed,
        "excluded_by_set_bound": excluded,
        "weapon_domain": weapon_domain,
        "source_model_scope": match authority {
            crate::loadout_context::SearchAuthority::VerifiedNativeEpoch => "runtime_bound_rebuild",
            crate::loadout_context::SearchAuthority::CurrentProgressBranch => "conditional_rebuild",
            crate::loadout_context::SearchAuthority::BundleDefault => engine.source_model_scope(),
        },
        "complete": report.complete,
        "searched": report.searched,
        "seconds": report.seconds,
        "polish_rounds": report.polish_rounds,
        "answered": report.counts.answered,
        "no_path": report.counts.no_path,
        "refused": report.counts.refused,
        "outside_limits": report.counts.outside_limits,
        "missed": report.missed.len(),
        "backend": match report.backend {
            Backend::Card => "card",
            Backend::Cpu(_) => "cpu",
        },
        "current": current,
        "enhanced": enhanced,
    });
    if let Some(receipt) = execution_receipt {
        result["execution_receipt"] = serde_json::to_value(receipt).map_err(|error| {
            HostError::rejected(format!("LOADOUT_EXECUTION_RECEIPT_INVALID: {error}"))
        })?;
    }
    Ok(result)
}

fn validate_source_search(
    policy: SourceAdmissionPolicy,
    move_points: bool,
) -> Result<(), HostError> {
    match (policy, move_points) {
        (SourceAdmissionPolicy::CapturedWeaponOnly, true) => {
            Err(HostError::rejected("LOADOUT_SOURCE_ATTRIBUTES_UNPROVEN"))
        }
        (SourceAdmissionPolicy::CapturedWeaponOnly, false)
        | (SourceAdmissionPolicy::OwnedType, true | false) => Ok(()),
    }
}

/// Record-level source scope accompanies outfit-level coverage. Retaining the
/// actual context weapon is a separate option, not another owned pool record.
fn weapon_domain(
    bundle: &Bundle,
    engine: &Engine,
    pack: &Backpack,
    params: &Value,
    policy: SourceAdmissionPolicy,
) -> Value {
    let rows = pack
        .ranges
        .iter()
        .filter(|range| range.0 == 0)
        .flat_map(|&(_, first, count)| first..first + count)
        .collect::<Vec<_>>();
    let admitted = rows
        .iter()
        .filter(|&&row| match policy {
            SourceAdmissionPolicy::OwnedType => true,
            SourceAdmissionPolicy::CapturedWeaponOnly => {
                pack.pool[row as usize].as_slice() == engine.worn_item(9)
            }
        })
        .count();
    let name = if bundle.requested_weapon_class().is_none() {
        "retained"
    } else {
        match policy {
            SourceAdmissionPolicy::OwnedType => "owned_type",
            SourceAdmissionPolicy::CapturedWeaponOnly => "captured_record_only",
        }
    };
    json!({
        "type_class": bundle.requested_weapon_class(),
        "policy": name,
        "requested_items": rows.len(),
        "admitted_items": admitted,
        "uncomputed_items": rows.len() - admitted,
        "retained_current": rows.is_empty() || params["keep_current"].as_bool().unwrap_or(false),
        "reason": policy.reason(),
    })
}

fn facts_json(f: Facts) -> Value {
    json!({"load": f.load, "defense": f.defense, "health": f.health})
}

/// `sets`: a list of rules, each `{options: [{key, pieces}], at_least}`; all of them must hold.
fn set_rules(value: &Value) -> Result<Vec<SetRule>, HostError> {
    let Some(rules) = value.as_array() else {
        return if value.is_null() {
            Ok(Vec::new())
        } else {
            Err(HostError::rejected("sets must be a list"))
        };
    };
    if rules.len() > MAX_RULES {
        return Err(HostError::rejected("LOADOUT_SETS_TOO_MANY"));
    }
    let mut keys = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(rules.len());
    for rule in rules {
        let options = rule["options"]
            .as_array()
            .ok_or_else(|| HostError::rejected("set rule needs options"))?;
        let mut parsed = Vec::with_capacity(options.len());
        for o in options {
            let (Some(key), Some(pieces)) = (o["key"].as_u64(), o["pieces"].as_u64()) else {
                return Err(HostError::rejected("set option needs key and pieces"));
            };
            keys.insert(key);
            parsed.push(SetOption {
                key: key as u32,
                pieces: pieces as usize,
            });
        }
        let at_least = rule["at_least"].as_u64().unwrap_or(parsed.len() as u64) as usize;
        if parsed.is_empty() || at_least == 0 || at_least > parsed.len() {
            return Err(HostError::rejected("LOADOUT_SET_RULE_INVALID"));
        }
        // Zero pieces is true even for an absent set. Keep duplicate positive
        // options meaningful,but do not force a native backend for tautologies.
        let guaranteed = parsed.iter().filter(|option| option.pieces == 0).count();
        if guaranteed < at_least {
            parsed.retain(|option| option.pieces != 0);
            out.push(SetRule {
                options: parsed,
                at_least: at_least - guaranteed,
            });
        }
    }
    if keys.len() > MAX_SET_KEYS {
        return Err(HostError::rejected("LOADOUT_SETS_TOO_MANY"));
    }
    Ok(out)
}

/// Bind actual worn records before current scoring and result comparisons.
/// A save marker only disambiguates byte-identical records from the save.
fn bind_context_worn_items(
    engine: &Engine,
    plain: &[u8],
    origin: &mut [Chosen],
) -> Result<(), HostError> {
    let records = origin
        .iter()
        .map(|item| {
            equipment_record(plain, item.slot_index)
                .map(|record| &record[..ITEM])
                .map_err(HostError::from_save)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut bound = [None; SLOTS];
    for position in 0..SLOTS {
        let slot = slot_of(position);
        let worn = engine.worn_item(position);
        bound[position] = origin
            .iter()
            .zip(records.iter().copied())
            .enumerate()
            .filter(|(row, (_, record))| {
                *record == worn && !bound[..position].contains(&Some(*row))
            })
            .min_by_key(|(_, (item, _))| (item.worn != Some(slot), item.slot_index))
            .map(|(row, _)| row);
    }
    for item in origin.iter_mut() {
        item.worn = None;
    }
    for (position, row) in bound.into_iter().enumerate() {
        if let Some(row) = row {
            origin[row].worn = Some(slot_of(position));
        }
    }
    Ok(())
}

/// What the bundle's school wears now among the chosen items, evaluated like a
/// result. Missing searchable positions keep the captured context's records and
/// are reported explicitly in kept_slots.
fn current_outfit(
    engine: &Engine,
    pack: &Backpack,
    origin: &[Chosen],
    source_admission: SourceAdmissionPolicy,
) -> Result<Value, HostError> {
    let mut items = [None; SLOTS];
    let mut doubled = Vec::new();
    for (row, &input) in pack.origin.iter().enumerate() {
        let c = origin[input];
        // Accessory candidates share a pool. The binding retains their actual worn slot.
        let Some((slot, position)) = c.worn.and_then(|s| position_of(s).map(|p| (s, p))) else {
            continue;
        };
        if slot == 0
            && source_admission == SourceAdmissionPolicy::CapturedWeaponOnly
            && pack.pool[row].as_slice() != engine.worn_item(position)
        {
            continue;
        }
        if items[position].replace(row as u32).is_some() {
            doubled.push(slot);
        }
    }
    if !doubled.is_empty() {
        return Ok(json!({"status": "ambiguous", "slots": doubled}));
    }
    let candidate = Candidate {
        attrs: engine.start_attrs,
        items,
    };
    // The searched slots; accessories fill both accessory slots from one pool.
    let mut searched: Vec<u32> = pack.ranges.iter().map(|r| r.0).collect();
    if searched.iter().any(|s| matches!(s, 9 | 10)) {
        searched.extend([9, 10]);
    }
    searched.sort_unstable();
    searched.dedup();
    let kept: Vec<u32> = searched
        .into_iter()
        .filter(|&s| position_of(s).is_some_and(|p| items[p].is_none()))
        .collect();
    let worn: Vec<Value> = (0..SLOTS)
        .filter_map(|p| {
            items[p].map(|row| {
                let c = origin[pack.origin[row as usize]];
                json!({"slot": slot_of(p), "slot_index": c.slot_index, "item_id": c.item_id})
            })
        })
        .collect();
    let mut w = engine.work();
    let (status, damage) = match engine.evaluate(&candidate, pack, &Limits::NONE, &mut w) {
        Outcome::Damage(bits) => ("answered", Some(f32::from_bits(bits))),
        Outcome::NoPath => ("no_path", None),
        Outcome::Refused(_) => ("refused", None),
        Outcome::OutsideLimits => ("refused", None),
    };
    let details = match damage {
        Some(damage) => publication_damage_details(
            engine.damage_details(&candidate, pack, &mut w),
            damage.to_bits(),
        )?,
        None => nioh3_loadout::damage::DamageDetails::unavailable(
            nioh3_loadout::damage::DetailUnavailable::EvaluationFailed,
        ),
    };
    Ok(json!({
        "status": status,
        "damage": damage,
        "damage_details":details,
        "items": worn,
        "kept_slots": kept,
        "facts": engine.facts(&candidate, pack, &mut w).map(facts_json),
    }))
}

#[cfg(test)]
mod rule_normalization_tests {
    use super::*;
    #[test]
    fn zero_piece_options_do_not_force_a_native_set_backend() -> Result<(), HostError> {
        let rules = set_rules(&json!([{"options":[{"key":1,"pieces":0}],"at_least":1}]))?;
        assert!(rules.is_empty());
        Ok(())
    }
    #[test]
    fn guaranteed_options_reduce_the_requirement_without_collapsing_duplicates(
    ) -> Result<(), HostError> {
        let rules = set_rules(&json!([{"options":[{"key":1,"pieces":0},
            {"key":2,"pieces":1},{"key":2,"pieces":1}],"at_least":3}]))?;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].at_least, 2);
        assert_eq!(rules[0].options.len(), 2);
        assert!(rules[0]
            .options
            .iter()
            .all(|option| option.key == 2 && option.pieces == 1));
        Ok(())
    }
}

#[cfg(test)]
#[path = "loadout_domain_tests.rs"]
mod domain_tests;

/// A contradictory score must never become a publishable winner or baseline.
fn publication_damage_details(
    details: nioh3_loadout::damage::DamageDetails,
    score: u32,
) -> Result<nioh3_loadout::damage::DamageDetails, HostError> {
    use nioh3_loadout::damage::{DamageDetails, DetailUnavailable};
    let verified = details.verify_total(score);
    if matches!(
        verified,
        DamageDetails::Unavailable {
            reason: DetailUnavailable::ScoreMismatch
        }
    ) {
        return Err(HostError::rejected("LOADOUT_SCORE_VERIFICATION_MISMATCH"));
    }
    Ok(verified)
}

#[cfg(test)]
mod score_publication_tests {
    use super::publication_damage_details;
    use nioh3_loadout::damage::{DamageDetails, DetailUnavailable};

    #[test]
    fn contradictory_native_detail_blocks_rank_publication() {
        let detail = DamageDetails::Available {
            total: 723.0,
            physical: 702.0,
            elemental: 21.0,
            contacts: Vec::new(),
        };
        assert!(publication_damage_details(detail, 1016.0f32.to_bits()).is_err());
    }

    #[test]
    fn explicit_score_mismatch_cannot_be_downgraded_to_cosmetic_missing_detail() {
        let detail = DamageDetails::unavailable(DetailUnavailable::ScoreMismatch);
        assert!(publication_damage_details(detail, 1016.0f32.to_bits()).is_err());
    }

    #[test]
    fn matching_detail_and_legacy_capability_absence_remain_distinct() {
        let detail = DamageDetails::Available {
            total: 723.0,
            physical: 702.0,
            elemental: 21.0,
            contacts: Vec::new(),
        };
        assert!(publication_damage_details(detail, 723.0f32.to_bits()).is_ok());
        assert!(publication_damage_details(
            DamageDetails::unavailable(DetailUnavailable::MissingComponentCapability),
            723.0f32.to_bits(),
        )
        .is_ok());
    }
}
