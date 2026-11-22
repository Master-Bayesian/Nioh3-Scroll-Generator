//! Local, opt-in host acceptance for the ordinary first read. No CE or live writes.
//! Copy only the reads issued by the built-in PC202 product path into private
//! host evidence. This module is not compiled into the shipped worker.
use crate::{error::HostError, loadout, loadout_context, loadout_first_use, loadout_live};
use nioh3_loadout::{context::sha256, engine::Engine};
use nioh3_runtime::{
    combat_context::{read_combat_snapshot, SnapshotOutcome},
    InventoryMemory, InventoryProcess, RuntimeError,
};
use serde_json::{json, Value};
use std::{cell::RefCell, collections::BTreeMap, fs, path::Path};

struct Recorded<'a> {
    inner: &'a dyn InventoryMemory,
    reads: RefCell<Vec<(u64, Vec<u8>)>>,
}
impl InventoryMemory for Recorded<'_> {
    fn module_base(&self) -> u64 {
        self.inner.module_base()
    }
    fn process(&self) -> InventoryProcess {
        self.inner.process()
    }
    fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
        self.inner.recheck_creation_filetime()
    }
    fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        let bytes = self.inner.read(address, size)?;
        self.reads.borrow_mut().push((address, bytes.clone()));
        Ok(bytes)
    }
}
fn failed(e: impl std::fmt::Display) -> HostError {
    HostError::rejected(e.to_string())
}
fn json_file(path: &Path, x: &impl serde::Serialize) -> Result<(), HostError> {
    fs::write(path, serde_json::to_vec(x).map_err(failed)?).map_err(failed)
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

fn fixed_bytes<const N: usize>(memory: &dyn InventoryMemory, address: u64) -> Option<[u8; N]> {
    memory.read(address, N).ok()?.try_into().ok()
}

/// PC202 1359FC. Retain both ordered lists for offline object-type and validity
/// evaluation. This evidence is optional on older snapshots and never scores an outfit.
fn effect_query_inputs(memory: &dyn InventoryMemory, owner: u64) -> Value {
    let ptr = |at| fixed_bytes(memory, at).map(u64::from_le_bytes);
    let Some(gate) = owner
        .checked_add(0x3368)
        .and_then(|at| fixed_bytes::<1>(memory, at))
    else {
        return json!({"state":"unobserved"});
    };
    if gate != [0] {
        return json!({"state":"closed","gate":gate[0]});
    }
    let mut lists = Vec::new();
    let mut occupied = false;
    for offset in [0x3328, 0x3308] {
        let sentinel = owner.checked_add(offset).and_then(ptr).filter(|a| *a != 0);
        let mut next = sentinel.and_then(ptr);
        let mut seen = std::collections::BTreeSet::new();
        let mut nodes = Vec::new();
        while let Some(node) = next.filter(|p| *p != 0 && Some(*p) != sentinel) {
            if !seen.insert(node) || seen.len() > 1024 {
                break;
            }
            let object = node.checked_add(0x10).and_then(ptr);
            occupied |= object.is_some_and(|p| p != 0);
            let fields = object.filter(|p| *p != 0).map(|p| {
                let vtable = ptr(p);
                let getter = vtable.and_then(|v| v.checked_add(0x10)).and_then(ptr);
                json!({
                    "vtable":vtable,"type_getter":getter,
                    "object_08_to_3b":p.checked_add(8).and_then(|a| fixed_bytes::<52>(memory,a)).map(|v|hex(&v)),
                    "counter_a0":p.checked_add(0xa0).and_then(|a| fixed_bytes::<8>(memory,a)).map(u64::from_le_bytes),
                    "ki_phase_b0":p.checked_add(0xb0).and_then(|a| fixed_bytes::<4>(memory,a)).map(u32::from_le_bytes)
                })
            });
            nodes.push(json!({"node":node,"object":object,"inputs":fields}));
            if object.is_none() {
                break;
            }
            next = ptr(node);
        }
        let complete = sentinel.is_some() && next == sentinel;
        lists.push(
            json!({"offset":offset,"sentinel":sentinel,"nodes":nodes,"closed_loop":complete}),
        );
        if !complete {
            return json!({"state":"incomplete","gate":0,"lists":lists});
        }
    }
    let state = if occupied {
        "objects_retained"
    } else {
        "empty"
    };
    json!({"state":state,"gate":0,"lists":lists})
}

fn action_facts(memory: &dyn InventoryMemory, actor: u64, owner: u64) -> Value {
    let mut fields = BTreeMap::new();
    let mut read = |name: &str, address: Option<u64>, size| {
        let bytes = address.and_then(|address| memory.read(address, size).ok());
        fields.insert(
            name.to_string(),
            json!({"address":address,"bytes":bytes.as_deref().map(hex)}),
        );
        bytes
            .and_then(|b| <[u8; 8]>::try_from(b).ok())
            .map(u64::from_le_bytes)
    };
    let nonnull = |ptr: Option<u64>| ptr.filter(|p| *p != 0);
    let descriptor = nonnull(read("descriptor", actor.checked_add(0xe8), 8));
    read(
        "agility_descriptor_flags",
        descriptor.and_then(|p| p.checked_add(0x68)),
        4,
    );
    let control = nonnull(read("control", actor.checked_add(0x3b0), 8));
    let mask = nonnull(read(
        "control_mask",
        control.and_then(|p| p.checked_add(0x40)),
        8,
    ));
    read("cost_modifier_exclusion", mask, 1);
    for (name, offset, size) in [
        ("class0_damage", 0x2e3c, 4),
        ("physical_and_elemental_factors", 0x2e48, 8),
        ("stance_ki_factors", 0x2e80, 12),
        ("status_element", 0x2e9c, 4),
        ("status_element_power", 0x2ea4, 4),
        ("deferred_status_begin", 0x3120, 8),
        ("deferred_status_end", 0x3128, 8),
        ("status_rebuild_flags", 0x32f4, 4),
        ("status_duration_adjustment", 0x2ddc, 4),
    ] {
        read(name, owner.checked_add(offset), size);
    }
    json!({"actor":actor,"owner":owner,"fields":fields})
}

fn effect_observation(memory: &dyn InventoryMemory, actor: u64, owner: u64) -> Value {
    let owner_link = || {
        actor
            .checked_add(0x3a0)
            .and_then(|p| fixed_bytes(memory, p))
            .map(u64::from_le_bytes)
    };
    let before = owner_link();
    let lists = if before == Some(owner) {
        effect_query_inputs(memory, owner)
    } else {
        json!({"state":"unobserved"})
    };
    let additive = owner
        .checked_add(0x2df4)
        .and_then(|p| fixed_bytes::<4>(memory, p))
        .map(|b| hex(&b));
    json!({"actor":actor,"owner":owner,"owner_before":before,"owner_after":owner_link(),
           "conditional_packet_additive":additive,"query":lists})
}

impl Recorded<'_> {
    fn save(&self, dir: &Path) -> Result<(), HostError> {
        let mut bytes = BTreeMap::new();
        let reads = self.reads.borrow();
        for (base, body) in reads.iter() {
            for (i, v) in body.iter().enumerate() {
                if bytes
                    .insert(base + i as u64, *v)
                    .is_some_and(|old| old != *v)
                {
                    // Preserve time order. A changing address has no single value
                    // that could faithfully represent both successful reads.
                    let mut stream = std::io::BufWriter::new(
                        fs::File::create(dir.join("ordered-reads.jsonl")).map_err(failed)?,
                    );
                    use std::io::Write;
                    for (address, value) in reads.iter() {
                        writeln!(stream, "{}", json!({"address":address,"bytes":hex(value)}))
                            .map_err(failed)?;
                    }
                    stream.flush().map_err(failed)?;
                    return Err(failed("host evidence changed; saved ordered reads only"));
                }
            }
        }
        let mut regions = Vec::new();
        let mut start = 0;
        let mut span = Vec::new();
        for (a, v) in bytes {
            if !span.is_empty() && a != start + span.len() as u64 {
                regions.push(json!({"address":start,"bytes":hex(&span)}));
                span.clear();
            }
            if span.is_empty() {
                start = a;
            }
            span.push(v);
        }
        if !span.is_empty() {
            regions.push(json!({"address":start,"bytes":hex(&span)}));
        }
        json_file(
            &dir.join("ordinary-read-memory.json"),
            &json!({"module_base":self.module_base(),"pid":self.process().pid,"creation_filetime":self.process().creation_filetime,"regions":regions}),
        )
    }
}

pub fn export(
    data: &Path,
    save_copy: &Path,
    out: &Path,
    fixture: Option<&Path>,
    id: &str,
) -> Result<(), HostError> {
    export_with_reference_target(data, save_copy, out, fixture, id, false)
}

pub fn export_with_reference_target(
    data: &Path,
    save_copy: &Path,
    out: &Path,
    fixture: Option<&Path>,
    id: &str,
    reference_target: bool,
) -> Result<(), HostError> {
    export_with_controls(data, save_copy, out, fixture, id, reference_target, false)
}

pub fn export_with_controls(
    data: &Path,
    save_copy: &Path,
    out: &Path,
    fixture: Option<&Path>,
    id: &str,
    reference_target: bool,
    record_action_facts: bool,
) -> Result<(), HostError> {
    let bundle = loadout::find_bundle(data, &json!({"bundle":id}))?;
    let dir = bundle.directory(data);
    let saved = fs::read(save_copy).map_err(failed)?;
    let plain = nioh3_save::decrypt_container(&saved).map_err(HostError::from_save)?;
    fs::create_dir(out).map_err(failed)?;
    let capture = |memory: &dyn InventoryMemory| -> Result<(), HostError> {
        let memory = Recorded {
            inner: memory,
            reads: Default::default(),
        };
        let mut observed = None;
        let mut capture_role = || -> Result<(), HostError> {
            let SnapshotOutcome::Ready(snapshot) =
                read_combat_snapshot(&memory).map_err(HostError::from_runtime)?
            else {
                return Err(failed("current role is not stable"));
            };
            observed = Some(snapshot);
            let snap = observed
                .as_deref()
                .ok_or_else(|| failed("missing observed role"))?;
            json_file(&out.join("snapshot.json"), snap)?;
            let facts = || action_facts(&memory, snap.roots.player, snap.roots.owner);
            let initial_facts = record_action_facts.then(facts);
            // A running timer can legitimately change during a role export. Keep its
            // two observations separate from the ordinary immutable action facts.
            let effects = || effect_observation(memory.inner, snap.roots.player, snap.roots.owner);
            let initial_effects = record_action_facts.then(effects);
            if let Some(initial) = &initial_facts {
                json_file(&out.join("action-facts-first.json"), initial)?;
            }
            if reference_target {
                use nioh3_runtime::combat_context::{
                    encounter::observe_target_operands,
                    target_conditions::{read_source_conditions, read_target_conditions},
                };
                let target = || {
                    read_target_conditions(&memory, snap.roots.player, snap.roots.style_entity)
                        .map_err(HostError::from_runtime)
                };
                let source = || {
                    read_source_conditions(&memory, snap.roots.owner)
                        .map_err(HostError::from_runtime)
                };
                let opponent = target()?;
                let actor = source()?;
                let tables = nioh3_loadout::pipeline::Tables::load(
                    &nioh3_loadout::kernel_asset::read(&dir, "tables.bin").map_err(failed)?,
                )
                .map_err(failed)?;
                let offset = snap.inputs.active_bank as usize * 0xf70
                    + snap.inputs.held_weapon_slot as usize * 0xe8;
                let key = u16::from_le_bytes([
                    snap.inputs.inventory[offset],
                    snap.inputs.inventory[offset + 1],
                ]);
                let native_category = tables
                    .category(key)
                    .ok_or_else(|| failed("unbound held category"))?;
                let mut categories: Vec<_> = (0..24)
                    .flat_map(|class| tables.categories_for_weapon_class(class))
                    .collect();
                categories.sort_unstable();
                categories.dedup();
                let progress =
                    nioh3_runtime::combat_context::progression::read_progression_for_categories(
                        &memory,
                        snap,
                        native_category,
                        &categories,
                    )
                    .map_err(HostError::from_runtime)?;
                let ranges =
                    observe_target_operands(&memory, &opponent).map_err(HostError::from_runtime)?;
                // These optional broad reads can include timers. Keep their
                // provenance separate from the repeated semantic comparisons.
                let mut broad = vec![
                    ("player_owner", snap.roots.owner, 0x4000),
                    ("target_owner", opponent.owner, 0x4000),
                    ("player_actor", snap.roots.player, 0x1600),
                    ("target_actor", opponent.actor, 0x1600),
                ];
                broad.extend(
                    opponent
                        .objects
                        .iter()
                        .map(|o| ("target_status", o.address, 0x94)),
                );
                let auxiliary:Vec<_> = broad.into_iter().map(|(role,address,size)|match memory.inner.read(address,size){
                    Ok(bytes)=>json!({"role":role,"address":address,"bytes":hex(&bytes),"sha256":sha256(&bytes)}),
                    Err(error)=>json!({"role":role,"address":address,"error":error.to_string()}),
                }).collect();
                json_file(
                    &out.join("reference-auxiliary.json"),
                    &json!({"scope":"single-pass auxiliary ranges","regions":auxiliary}),
                )?;
                if opponent != target()? || actor != source()? {
                    return Err(failed("opponent or actor conditions changed during read"));
                }
                json_file(
                    &out.join("reference-target.json"),
                    &json!({
                        "scope":"new fixed encounter observation","process":snap.process,
                        "actor":snap.roots.player,"opponent":opponent,"actor_conditions":actor,
                        "opponent_operand_ranges":ranges, "weapon_categories":categories,
                        "progress_rule_digest":progress.rule_table_digest
                    }),
                )?;
            }
            if !loadout_context::outfit_is_owned(
                &snap.inputs.inventory,
                snap.inputs.active_bank as usize * 0xf70,
                &plain,
            )? {
                return Err(failed("equipped records differ from the save copy"));
            }
            let source = loadout_first_use::read(&memory, snap, &dir)?;
            json_file(&out.join("source-inputs.json"), &source.report())?;
            let (context, _) = source.context(&memory, snap, data, &dir, &plain)?;
            let engine = Engine::open_with_context(&dir, &context).map_err(failed)?;
            {
                use nioh3_loadout::pipeline::{Frame, Scratch};
                let encoded = engine
                    .encoded_context()
                    .ok_or_else(|| failed("missing scoring context"))?;
                let frame = Frame::load(context.frame()).map_err(failed)?;
                let character = Frame::characters(
                    encoded
                        .file("characters.bin")
                        .ok_or_else(|| failed("missing character"))?,
                )
                .map_err(failed)?
                .into_iter()
                .next()
                .ok_or_else(|| failed("empty character"))?;
                let mut scratch = Scratch::new();
                engine
                    .tables()
                    .pipeline(&frame, &character, &engine.start_attrs, &[], &mut scratch)
                    .map_err(failed)?;
                let observed_stat = memory
                    .read(snap.roots.stat, 0x900)
                    .map_err(HostError::from_runtime)?;
                let cols = |bytes: &[u8], index: usize| -> Vec<u32> {
                    bytes[0x24 + index * 64..0x24 + index * 64 + 32]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|w| u32::from_le_bytes(*w))
                        .collect()
                };
                let records:Vec<_> = (0..5).map(|i|json!({"prepared":cols(&scratch.stat,i),"observed":cols(&observed_stat,i)})).collect();
                json_file(
                    &out.join("equipped-stages.json"),
                    &json!({"attack_gain":context.manifest().non_gear.attack_gain,"records":records,"character":context.manifest().character}),
                )?;
            }
            let verification = loadout_first_use::verify_equipped(&engine, snap);
            let unverified = out.join("unverified");
            let out = if verification.is_ok() {
                out
            } else {
                unverified.as_path()
            };
            let bundle = out.join("bundle");
            let private = bundle.join("context");
            fs::create_dir_all(&private).map_err(failed)?;
            for entry in fs::read_dir(&dir).map_err(failed)? {
                let entry = entry.map_err(failed)?;
                if entry.file_type().map_err(failed)?.is_file() {
                    fs::copy(entry.path(), bundle.join(entry.file_name())).map_err(failed)?;
                }
            }
            for name in nioh3_loadout::kernel_asset::POOLED_FILES {
                let path = nioh3_loadout::kernel_asset::resolve(&dir, name).map_err(failed)?;
                if path.is_file() && !bundle.join(name).exists() {
                    fs::copy(path, bundle.join(name)).map_err(failed)?;
                }
            }
            json_file(&private.join("context.json"), context.manifest())?;
            fs::write(private.join("inventory.bin"), context.inventory()).map_err(failed)?;
            fs::write(private.join("frame.bin"), context.frame()).map_err(failed)?;
            let encoded = context.encode(&[]);
            for name in ["image.bin", "image_known.bin", "scene_entry.bin"] {
                fs::write(
                    private.join(name),
                    encoded
                        .file(name)
                        .ok_or_else(|| failed("missing private scene input"))?,
                )
                .map_err(failed)?;
            }
            let mut rows = Vec::new();
            for item in loadout::all_owned_search_items(data, &plain, id)? {
                let record = nioh3_save::character::equipment_record(&plain, item.slot_index)
                    .map_err(HostError::from_save)?;
                rows.push(json!({"slot":item.slot,"slot_index":item.slot_index,"record_sha256":sha256(&record[..0xe8]),"record_bytes":&record[..0xe8],"assignment_bytes":&record[0xe8..0xf0]}));
            }
            json_file(
                &out.join("inventory.json"),
                &json!({"schema":"nioh3-current-whole-backpack/v1","only_reads_save":true,"source_sha256":sha256(&saved),"searchable_items":rows.len(),"records":rows}),
            )?;
            if let Some(initial) = initial_facts {
                let final_read = facts();
                json_file(
                    &out.join("action-facts.json"),
                    &json!({
                        "schema":"nioh3-pc202-action-facts/v1","source":if fixture.is_some() {"fixture"} else {"process"},
                        "pid":memory.process().pid,"creation_filetime":memory.process().creation_filetime,
                        "module_base":memory.module_base(),
                        "unchanged":initial==final_read,"first":initial,"last":final_read,
                    }),
                )?;
            }
            if let Some(first) = initial_effects {
                let last = effects();
                json_file(
                    &out.join("effect-query-inputs.json"),
                    &json!({
                        "schema":"nioh3-pc202-effect-object-inputs/v1",
                        "source":if fixture.is_some() {"fixture"} else {"process"},
                        "pid":memory.process().pid,"creation_filetime":memory.process().creation_filetime,
                        "module_base":memory.module_base(),"unchanged":first==last,"first":first,"last":last
                    }),
                )?;
            }
            verification
        };
        let result = capture_role();
        if result.is_err() {
            if let Some(snapshot) = observed {
                let additional = loadout_first_use::sample_encounter(&memory, &snapshot, &dir);
                json_file(
                    &out.join("supplemental-encounter.json"),
                    &json!({
                        "scene":id, "sampled":additional.is_ok(), "admitted":false,
                        "error":additional.err().map(|e| e.to_string())
                    }),
                )?;
            }
        }
        let replay = memory.save(out);
        let info = json!({"ready":result.is_ok(),"replay_saved":replay.is_ok(),"scope":if fixture.is_some(){"retained memory"}else{"ordinary verified game read"},"save_sha256":sha256(&saved),"save_writes":0,"game_writes":0,"error":result.as_ref().err().map(ToString::to_string),"replay_error":replay.as_ref().err().map(ToString::to_string)});
        json_file(&out.join("receipt.json"), &info)?;
        result.and(replay)
    };
    if let Some(path) = fixture {
        let raw: Value =
            serde_json::from_slice(&fs::read(path).map_err(failed)?).map_err(failed)?;
        capture(&nioh3_runtime::FixtureMemory::from_json(&raw).map_err(HostError::from_runtime)?)
    } else {
        loadout_live::with_memory(|m| capture(m.ok_or_else(|| failed("game not running"))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_query_preserves_unknown_and_native_list_order(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let owner = 0x2000u64;
        let fixture = |buf: &[u8]| {
            nioh3_runtime::FixtureMemory::from_json(&json!({
                "pid":1, "creation_filetime":2, "module_base":4096,
                "regions":[{"address":owner,"bytes":hex(buf)}]
            }))
        };
        let mut buf = vec![0; 0x3370];
        let word =
            |buf: &mut [u8], at: usize, v: u64| buf[at..at + 8].copy_from_slice(&v.to_le_bytes());
        assert_eq!(
            effect_query_inputs(&fixture(&[0])?, owner)["state"],
            "unobserved"
        );
        buf[0x3368] = 1;
        assert_eq!(
            effect_query_inputs(&fixture(&buf)?, owner)["state"],
            "closed"
        );
        buf[0x3368] = 0;
        assert_eq!(
            effect_query_inputs(&fixture(&buf)?, owner)["state"],
            "incomplete"
        );
        for (field, head) in [(0x3328, 0x200), (0x3308, 0x280)] {
            word(&mut buf, field, owner + head);
            word(&mut buf, head as usize, owner + head);
        }
        assert_eq!(
            effect_query_inputs(&fixture(&buf)?, owner)["state"],
            "empty"
        );
        word(&mut buf, 0x200, owner + 0x400);
        word(&mut buf, 0x400, owner + 0x200);
        word(&mut buf, 0x410, owner + 0x500);
        word(&mut buf, 0x500, owner + 0x700);
        word(&mut buf, 0x710, 0x1a00);
        word(&mut buf, 0x5a0, 17);
        let observation = effect_query_inputs(&fixture(&buf)?, owner);
        assert_eq!(observation["state"], "objects_retained");
        assert_eq!(observation["lists"][0]["offset"], 0x3328);
        assert_eq!(
            observation["lists"][0]["nodes"][0]["inputs"]["type_getter"],
            0x1a00
        );
        assert_eq!(
            observation["lists"][0]["nodes"][0]["inputs"]["counter_a0"],
            17
        );
        assert_eq!(observation["lists"][1]["offset"], 0x3308);
        word(&mut buf, 0x400, owner + 0x400);
        assert_eq!(
            effect_query_inputs(&fixture(&buf)?, owner)["state"],
            "incomplete"
        );
        Ok(())
    }

    #[test]
    fn repeated_reads_with_changed_bytes_preserve_both_observations(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = |bytes| {
            nioh3_runtime::FixtureMemory::from_json(&json!({
                "pid":1,"creation_filetime":2,"module_base":4096,
                "regions":[{"address":4100,"bytes":bytes}]
            }))
        };
        let before = fixture("01ff")?;
        let after = fixture("02ff")?;
        let mut record = Recorded {
            inner: &before,
            reads: Default::default(),
        };
        record.read(4100, 2)?;
        record.inner = &after;
        record.read(4100, 2)?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nioh-first-read-{nonce}"));
        fs::create_dir(&dir)?;
        assert!(record.save(&dir).is_err());
        assert!(!dir.join("ordinary-read-memory.json").exists());
        let log = dir.join("ordered-reads.jsonl");
        let rows: Vec<Value> = fs::read_to_string(&log)?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        assert_eq!(
            rows,
            [
                json!({"address":4100,"bytes":"01ff"}),
                json!({"address":4100,"bytes":"02ff"})
            ]
        );
        fs::remove_file(log)?;
        fs::remove_dir(dir)?;
        Ok(())
    }
}
