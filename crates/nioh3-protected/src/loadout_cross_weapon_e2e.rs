//! Rebuild held-out weapon states using only the starting role and an owned replacement.
use super::*;

#[test]
#[ignore = "requires independently observed equipped roles and the same owned backpack"]
fn cross_weapon_source_rebuild_from_one_observation() -> Result<(), Box<dyn std::error::Error>> {
    use nioh3_runtime::{
        combat_context::{progression::read_progression, read_combat_snapshot, SnapshotOutcome},
        FixtureMemory,
    };
    use std::path::PathBuf;
    #[derive(serde::Deserialize)]
    struct Role {
        name: String,
        observation: PathBuf,
        owned_slot: usize,
        scene: String,
        equipped: PathBuf,
    }
    let root = PathBuf::from(std::env::var("NIOH3_CROSS_WEAPON_CASE")?);
    let roles: Vec<Role> = serde_json::from_slice(&fs::read(root.join("roles.json"))?)?;
    let mut evidence = Vec::new();
    for from in &roles {
        let observed: Value = serde_json::from_slice(&fs::read(
            from.observation.join("ordinary-read-memory.json"),
        )?)?;
        let memory = FixtureMemory::from_json(&observed)?;
        let SnapshotOutcome::Ready(snapshot) = read_combat_snapshot(&memory)? else {
            return Err("source role unavailable".into());
        };
        let mut source = read(&memory, &snapshot, &from.observation.join("bundle"))
            .map_err(|e| e.to_string())?;
        for to in &roles {
            if to.name == from.name {
                continue;
            }
            // The only replacement input is the named owned item in the save.
            // The independently observed destination is opened after rebuilding.
            let items: Value =
                serde_json::from_slice(&fs::read(to.observation.join("inventory.json"))?)?;
            let row = items["records"]
                .as_array()
                .ok_or("missing items")?
                .iter()
                .find(|r| r["slot_index"].as_u64() == Some(to.owned_slot as u64))
                .ok_or("missing owned row")?;
            let record: Vec<u8> = serde_json::from_value(row["record_bytes"].clone())?;
            let key = u16::from_le_bytes(record[..2].try_into()?);
            let category = source
                .tables
                .category(key)
                .ok_or("unknown weapon category")?;
            let progress = read_progression(&memory, &snapshot, category)?;
            source.prefix = progress
                .layers()
                .into_iter()
                .flatten()
                .map(|s| Submission {
                    group: s.group,
                    value: s.value,
                })
                .collect();
            let mut outfit = snapshot.inputs.inventory.clone();
            let first = (snapshot.inputs.active_bank as usize) * 0xf70;
            outfit[first..first + 0xe8].copy_from_slice(&record);
            let predicted = source
                .replay_observed_outfit(&snapshot, &outfit)
                .map_err(|e| e.to_string())?;
            use nioh3_loadout::engine::{position_of, Backpack, Candidate, Engine, SLOTS};
            let data = PathBuf::from(std::env::var("NIOH3_CROSS_WEAPON_DATA")?);
            let target_scene = data.join("loadout").join(&to.scene);
            let projected = read(&memory, &snapshot, &target_scene).map_err(|e| e.to_string())?;
            let context = projected
                .context(&memory, &snapshot, &data, &target_scene)
                .map_err(|e| e.to_string())?;
            let engine = Engine::open_with_context(&target_scene, &context)?;
            let pack = Backpack::from_slots(&[(0, record.as_slice().try_into()?)])?;
            let mut candidate = Candidate {
                attrs: engine.start_attrs,
                items: [None; SLOTS],
            };
            candidate.items[position_of(0).ok_or("missing weapon axis")?] = Some(0);
            let stats = engine.prepared_stats(&candidate, &pack, &mut engine.work())?;
            let damage =
                serde_json::to_value(engine.damage_details(&candidate, &pack, &mut engine.work()))?;
            let native: Value = serde_json::from_slice(&fs::read(&to.equipped)?)?;
            let same_damage = damage == native["top_case"]["components"];
            let destination: Value =
                serde_json::from_slice(&fs::read(to.observation.join("snapshot.json"))?)?;
            let dst = &destination["inputs"]["stats"];
            let computed = json!([
                stats.primary_attack,
                stats.secondary_attack,
                stats.defense,
                stats.health
            ]);
            let observed = json!([
                dst["primary_attack"],
                dst["secondary_attack"],
                dst["defense"],
                dst["health"]
            ]);
            let mut matches = Vec::new();
            for map in ["core_groups", "groups"] {
                let expected: Vec<_> = destination["inputs"][map][snapshot.inputs.mode as usize]
                    .as_array()
                    .ok_or("missing source witness")?
                    .iter()
                    .map(|a| json!([a["key"], a["base"], a["dup"]]))
                    .collect();
                matches.push(predicted == json!(expected));
            }
            evidence.push(json!({"from":from.name,"to":to.name,"item":key,"category":category,"entries":predicted.as_array().map(Vec::len),"stats":computed,"observed_stats":observed,"stats_match":computed==observed,"damage_matches":same_damage,"damage":damage,"core_matches":matches[0],"selected_matches":matches[1]}));
        }
    }
    fs::write(
        root.join("RESULT.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    if evidence.is_empty()
        || evidence.iter().any(|r| {
            r["core_matches"] != true
                || r["selected_matches"] != true
                || r["stats_match"] != true
                || r["damage_matches"] != true
        })
    {
        return Err("cross-weapon source witness differs".into());
    }
    Ok(())
}

#[test]
#[ignore = "requires PC202 Ninja original combat snapshots and the ordinary role fixture"]
fn ninja_training_weapons_share_the_stat_producer() -> Result<(), Box<dyn std::error::Error>> {
    use nioh3_loadout::engine::{position_of, Backpack, Candidate, Engine, ITEM, SLOTS};
    use nioh3_runtime::{
        combat_context::{progression::read_progression, read_combat_snapshot, SnapshotOutcome},
        FixtureMemory,
    };
    use std::path::PathBuf;
    let root = PathBuf::from(std::env::var("NIOH3_NINJA_CROSS_CASE")?);
    let setup: Value = serde_json::from_slice(&fs::read(root.join("case.json"))?)?;
    let path = |key: &str| {
        setup[key]
            .as_str()
            .map(PathBuf::from)
            .ok_or("missing fixture path")
    };
    let observed: Value = serde_json::from_slice(&fs::read(path("memory")?)?)?;
    let memory = FixtureMemory::from_json(&observed)?;
    let SnapshotOutcome::Ready(snapshot) = read_combat_snapshot(&memory)? else {
        return Err("incomplete Ninja snapshot".into());
    };
    let data = path("data")?;
    let scene = data.join("loadout/kusarigama-strong");
    let targets: Value = serde_json::from_slice(&fs::read(root.join("ORIGINAL_WITNESSES.json"))?)?;
    let mut report = Vec::new();
    for (name, witness) in targets.as_object().ok_or("missing targets")? {
        let mut source = read(&memory, &snapshot, &scene).map_err(|e| e.to_string())?;
        let raw = witness["inventory"]
            .as_str()
            .ok_or("missing item bytes")?
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                u8::from_str_radix(std::str::from_utf8(p).map_err(|_| "bad item bytes")?, 16)
                    .map_err(|_| "bad item bytes")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bank = snapshot.inputs.active_bank as usize * 0xf70;
        let mut predicted_inventory = snapshot.inputs.inventory.clone();
        let mut selected = Vec::new();
        // The older training roles had an empty head slot as well as a
        // different held weapon and arm piece.
        for slot in [0usize, 4, 6] {
            let at = bank + slot * ITEM;
            let item: [u8; ITEM] = raw[at..at + ITEM].try_into()?;
            selected.push((slot as u32, item));
            predicted_inventory[at..at + ITEM].copy_from_slice(&item);
        }
        let key = u16::from_le_bytes(raw[bank..bank + 2].try_into()?);
        let category = source
            .tables
            .category(key)
            .ok_or("missing Ninja category")?;
        let prefix: Vec<_> = read_progression(&memory, &snapshot, category)?
            .layers()
            .into_iter()
            .flatten()
            .map(|e| Submission {
                group: e.group,
                value: e.value,
            })
            .collect();
        source.projected_weapon_class = source.tables.item_type_class(key);
        source.category_prefixes.insert(category, prefix.clone());
        let ctx = source
            .context(&memory, &snapshot, &data, &scene)
            .map_err(|e| e.to_string())?;
        let engine = Engine::open_with_context(&scene, &ctx)?;
        let pack = Backpack::from_slots(&selected)?;
        let mut candidate = Candidate {
            attrs: engine.start_attrs,
            items: [None; SLOTS],
        };
        for (i, &origin) in pack.origin.iter().enumerate() {
            candidate.items[position_of(selected[origin].0).ok_or("invalid slot")?] =
                Some(i as u32);
        }
        let stats = engine.prepared_stats(&candidate, &pack, &mut engine.work())?;
        let got = json!({"primary":stats.primary_attack,"secondary":stats.secondary_attack,"defense":stats.defense,"health":stats.health});
        source.prefix = prefix;
        let predicted = source
            .replay_observed_outfit(&snapshot, &predicted_inventory)
            .map_err(|e| e.to_string())?;
        let matched = |k: &str| -> Result<bool, &'static str> {
            let expected: Vec<_> = witness[k][1]
                .as_array()
                .ok_or("missing Ninja source")?
                .iter()
                .map(|e| json!([e["key"], e["base"], e["dup"]]))
                .collect();
            Ok(predicted == json!(expected))
        };
        report.push(json!({"weapon":name,"type":source.projected_weapon_class,"core":matched("core_groups")?,
          "selected":matched("groups")?,"predicted_stats":got,"observed_stats":witness["stats"],"stats_match":got==witness["stats"]}));
    }
    fs::write(
        root.join("NUMERIC_RESULT.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    if report.len() != 3
        || report
            .iter()
            .any(|r| r["core"] != true || r["selected"] != true || r["stats_match"] != true)
    {
        return Err("Ninja held-out witness differs".into());
    }
    Ok(())
}
