//! Build the current outfit's ordered source rules from ordinary process reads.
//! This step has no dependency on a private generation or a damage template.
use crate::error::HostError;
use nioh3_loadout::action_rows::ActionRow;
use nioh3_loadout::context::{
    sha256, CharacterInputs, ContextManifest, CoreEntry, LoadoutContext, NonGearSources,
    Provenance, Scenario, SceneBytes,
};
use nioh3_loadout::pipeline::Frame;
use nioh3_loadout::pipeline::{
    source_numeric::{Inputs, Parameters},
    source_rebuild::{
        BaselineBinding, Binding, CharacterBinding, CoreInput, EvidenceScope, GuardianSubmission,
        Program, SourceOutfit, SourceRules, Submission,
    },
    Tables,
};
use nioh3_runtime::combat_context::encounter::direct_encounter;
use nioh3_runtime::combat_context::progression::{unarmed_levels, ActorBaseline};
use nioh3_runtime::{
    combat_context::{progression::read_progression, CombatSnapshot},
    InventoryMemory,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

pub(crate) struct SourceRead {
    prefix: Vec<Submission>,
    guardians: Vec<GuardianSubmission>,
    cores: Vec<CoreInput>,
    late: [Submission; 4],
    post: [[bool; 4]; 2],
    guardian_requirement: u16,
    active_guardian: u8,
    digest: String,
    core_matches: bool,
    selected_matches: bool,
    category: u32,
    cap: u32,
    tables: Tables,
    parameters: Parameters,
    basis: ActorBaseline,
}

pub(crate) fn read(
    memory: &dyn InventoryMemory,
    snapshot: &CombatSnapshot,
    scene: &Path,
) -> Result<SourceRead, HostError> {
    let invalid =
        |error: &str| HostError::rejected(format!("LOADOUT_SOURCE_INPUT_INVALID: {error}"));
    let load = |name: &str| fs::read(scene.join(name)).map_err(|e| invalid(&e.to_string()));
    let tables = Tables::load(&load("tables.bin")?).map_err(invalid)?;
    let parameters = Parameters::load(&load("source_numeric.bin")?).map_err(invalid)?;
    let input = &snapshot.inputs;
    let active = input.active_bank as usize;
    let offset = active * 0xf70 + input.held_weapon_slot as usize * 0xe8;
    let id = input
        .inventory
        .get(offset..offset + 2)
        .ok_or_else(|| invalid("held weapon outside the current inventory"))?;
    let key = u16::from_le_bytes([id[0], id[1]]);
    let category = tables.category(key).unwrap_or(0x3c2e);
    let progress = read_progression(memory, snapshot, category).map_err(HostError::from_runtime)?;
    if progress.rule_table_digest != parameters.rule_table_digest() {
        return Err(invalid(
            "live guardian or core parameter tables differ from installed scene",
        ));
    }
    let source = &snapshot.core_source_raw;
    let (guardians, cores) = parameters
        .source_rules(
            &tables,
            source,
            input.progression_gate,
            &progress.core_unlock_thresholds,
        )
        .map_err(invalid)?;
    let submission = |r: &nioh3_runtime::combat_context::progression::Submission| Submission {
        group: r.group,
        value: r.value,
    };
    let prefix: Vec<_> = progress
        .layers()
        .into_iter()
        .flatten()
        .map(submission)
        .collect();
    let late = progress.late_clan.each_ref().map(submission);
    let active_guardian = source[0x70];
    let guardian_requirement = u16::from_le_bytes([source[0x72], source[0x73]]);
    let rules = SourceRules {
        prefix: &prefix,
        active_guardian,
        guardian_requirement,
        guardians: &guardians,
        cores: &cores,
        late_clan: &late,
        reference_inventory: &input.inventory,
        post_record_present: progress.post_record_present,
    };
    // Original PC202 profile progression controls the equipment requirement cap.
    let cap = match input.profile[0x45] {
        2 => 15,
        3 => 30,
        4 => 45,
        5 => 120,
        _ => 5,
    };
    let maps = rules
        .rebuild_maps(
            &tables,
            SourceOutfit {
                inventory: &input.inventory,
                mode: input.mode,
                active,
                cap,
            },
            &input.raw_attrs,
        )
        .map_err(invalid)?;
    let map = usize::try_from(input.mode)
        .ok()
        .and_then(|i| maps.get(i))
        .ok_or_else(|| invalid("unsupported active form"))?;
    let agrees = |observed: &[nioh3_runtime::combat_context::AggregateEntry]| {
        map.iter()
            .map(|v| (v.key, v.base, v.dup))
            .eq(observed.iter().map(|v| (v.key, v.base, v.dup)))
    };
    let form = input.mode as usize;
    let core_matches = agrees(&input.core_groups[form]);
    let selected_matches = agrees(&input.groups[form]);
    let basis = nioh3_runtime::combat_context::progression::read_actor_baseline(memory, snapshot)
        .map_err(HostError::from_runtime)?;
    let mut identity = serde_json::to_vec(&(
        &snapshot.process,
        &snapshot.roots,
        &input.inventory,
        &input.raw_attrs,
        &prefix,
        &guardians,
        &cores,
        &late,
        progress.post_record_present,
        progress.digest(),
    ))
    .map_err(|e| invalid(&e.to_string()))?;
    identity.extend(sha256(&basis.stat).bytes());
    identity.extend(sha256(&basis.materialized).bytes());
    Ok(SourceRead {
        prefix,
        guardians,
        cores,
        late,
        post: progress.post_record_present,
        guardian_requirement,
        active_guardian,
        digest: sha256(&identity),
        core_matches,
        selected_matches,
        category,
        cap,
        tables,
        parameters,
        basis,
    })
}

impl SourceRead {
    #[cfg(test)]
    pub(crate) fn replay_observed_outfit(
        &self,
        current: &CombatSnapshot,
        other: &[u8],
    ) -> Result<Value, HostError> {
        let rules = SourceRules {
            prefix: &self.prefix,
            guardians: &self.guardians,
            active_guardian: self.active_guardian,
            guardian_requirement: self.guardian_requirement,
            cores: &self.cores,
            late_clan: &self.late,
            reference_inventory: &current.inputs.inventory,
            post_record_present: self.post,
        };
        let maps = rules
            .rebuild_maps(
                &self.tables,
                SourceOutfit {
                    inventory: other,
                    active: current.inputs.active_bank as usize,
                    mode: current.inputs.mode,
                    cap: self.cap,
                },
                &current.inputs.raw_attrs,
            )
            .map_err(HostError::rejected)?;
        Ok(json!(maps[current.inputs.mode as usize]
            .iter()
            .map(|e| [u32::from(e.key), e.base, u32::from(e.dup)])
            .collect::<Vec<_>>()))
    }
    pub fn matched(&self) -> bool {
        self.core_matches && self.selected_matches
    }
    pub fn summary(
        &self,
        save: &str,
        source: &str,
        bundle: &str,
        snapshot: &CombatSnapshot,
        reason: &str,
    ) -> Value {
        json!({"state":"sources_read","save_id":save,"source_sha256":source,"bundle":bundle,"school":if snapshot.inputs.mode==1{"ninja"}else{"samurai"},
        "level":snapshot.inputs.level,"attrs":snapshot.inputs.raw_attrs,"submissions":self.prefix.len(),"source_state":if self.matched() {"current_maps_matched"}else{"current_maps_differ"},
        "source_digest":self.digest,"missing":"damage_context","detail":reason})
    }
    #[cfg(any(test, feature = "first-use-acceptance"))]
    pub(crate) fn report(&self) -> Value {
        json!({"state":if self.matched(){"current_maps_matched"}else{"current_maps_differ"},"prefix":self.prefix,"guardians":self.guardians,"cores":self.cores,"late_clan":self.late,
          "active_guardian":self.active_guardian,"guardian_requirement":self.guardian_requirement,"post_record_present":self.post,
          "numeric_basis":{"stat_sha256":sha256(&self.basis.stat),"materialized_sha256":sha256(&self.basis.materialized),
             "reset_stat_sha256":sha256(&self.basis.reset_stat),"reset_damage_sha256":sha256(&self.basis.reset_damage),
             "health_seed":self.basis.health_seed,"primary_plus_bonus":self.basis.primary_plus_bonus}})
    }

    pub(crate) fn context(
        &self,
        memory: &dyn InventoryMemory,
        snap: &CombatSnapshot,
        data_root: &Path,
        scene: &Path,
    ) -> Result<LoadoutContext, HostError> {
        let invalid = |e: &str| HostError::rejected(format!("LOADOUT_FIRST_USE_SCENE: {e}"));
        let input = &snap.inputs;
        if !self.matched()
            || input.raw_attrs != input.synchronized_attrs
            || !nioh3_runtime::combat_context::progression::ordinary_equipment_branch(
                memory,
                snap,
                &self.basis.stat,
            )
            .map_err(HostError::from_runtime)?
        {
            return Err(invalid("source or synchronization branch is unavailable"));
        }
        // Use the PC202 integer and binary32 scale from the same versioned
        // parameter resource used by the equipment finalizer.
        let asset = nioh3_data::load_effect_resource_for_file_version(data_root, (2, 0, 2, 0))
            .map_err(|e| invalid(&e.to_string()))?;
        let mut matches = (0..asset.optional_multiplier.row_count())
            .filter_map(|i| asset.optional_multiplier.row(i))
            .filter(|row| row[0x14..0x18] == 0xaeacu32.to_le_bytes());
        let row = matches
            .next()
            .ok_or_else(|| invalid("missing plus-cap parameter AEAC"))?;
        if matches.next().is_some() {
            return Err(invalid("duplicate plus-cap parameter AEAC"));
        }
        let base = i32::from_le_bytes(
            row[0x10..0x14]
                .try_into()
                .map_err(|_| invalid("invalid plus-cap parameter"))?,
        );
        let scale = f32::from_le_bytes(
            row[0x18..0x1c]
                .try_into()
                .map_err(|_| invalid("invalid plus-cap scale"))?,
        );
        let amount = self.cap as f32 + base as f32 * scale;
        if !amount.is_finite() || !(0.0..=500.0).contains(&amount) {
            return Err(invalid("unsupported equipment plus cap"));
        }
        let plus = amount as u32;
        let levels = unarmed_levels(memory, snap).map_err(HostError::from_runtime)?;
        let unarmed = self.tables.unarmed_attack(levels, plus);
        let num = input
            .source_numeric_inputs
            .as_ref()
            .ok_or_else(|| invalid("missing numeric source inputs"))?;
        let seed = self
            .parameters
            .derive_for_caps(
                &self.tables,
                self.cap,
                plus,
                Inputs {
                    guardian_records: &num.guardian_records,
                    active_guardian: num.active_guardian,
                    guardian_numbers_disabled: num.guardian_numbers_disabled,
                    core_records: &num.core_records,
                },
            )
            .map_err(invalid)?;
        let resource = input
            .owner_resource_bits
            .ok_or_else(|| invalid("owner resource unavailable"))?;
        let layout = fs::read(scene.join("frame.bin")).map_err(|e| invalid(&e.to_string()))?;
        let mut frame = Frame::first_use_frame(
            &layout,
            &self.basis.stat,
            &self.basis.reset_stat,
            &self.basis.materialized,
            &self.basis.reset_damage,
            [
                self.basis.primary_plus_bonus,
                self.basis.health_seed,
                seed.defense,
            ],
            resource,
        )
        .map_err(invalid)?;
        let char_binding = CharacterBinding {
            active: input.active_bank as usize,
            cap: self.cap,
            plus_cap: plus,
            style: input.style,
            unarmed,
            level: input.level,
        };
        let stat_hash = Frame::private_stat_digest(&frame).map_err(invalid)?;
        let program = Program {
            schema: nioh3_loadout::pipeline::source_rebuild::SCHEMA.into(),
            binding: Binding {
                scope: EvidenceScope::CurrentProgress,
                input_sha256: self.digest.clone(),
                tables_sha256: self.tables.input_digest().into(),
                generation: self.digest.clone(),
                form: input.mode,
                native_item_category: self.category,
                compiler_return_rva: 0x3774ad,
                owner_kind: 0,
                item_sync: false,
                second_weapon_pass: false,
                runtime_input_sha256: Some(self.digest.clone()),
                epoch_receipt_sha256: None,
                character: char_binding,
                source_scalar_words: std::array::from_fn(|i| {
                    let p = 0x5880 + 4 * i;
                    let s = &snap.core_source_raw;
                    u32::from_le_bytes([s[p], s[p + 1], s[p + 2], s[p + 3]])
                }),
            },
            prefix: self.prefix.clone(),
            active_guardian: self.active_guardian,
            guardian_requirement: self.guardian_requirement,
            guardians: self.guardians.clone(),
            cores: self.cores.clone(),
            late_clan: self.late.clone(),
            post_inventory_projection: true,
            reference_inventory: input.inventory.clone(),
            post_record_present: self.post,
            baseline: Some(BaselineBinding {
                stat_sha256: stat_hash,
                reset_d_sha256: sha256(&self.basis.reset_damage),
                attrs: input.raw_attrs,
                health_seed: self.basis.health_seed,
                defense_sources: seed.defense,
                main_base: self.basis.primary_plus_bonus,
            }),
        };
        frame = Frame::with_source_rebuild(&frame, &program).map_err(invalid)?;
        #[derive(serde::Deserialize)]
        struct AttackSelection {
            resource_action: ActionRow,
        }
        let descriptor =
            fs::read(scene.join("bundle.json")).map_err(|e| invalid(&e.to_string()))?;
        let attack: AttackSelection =
            serde_json::from_slice(&descriptor).map_err(|e| invalid(&e.to_string()))?;
        let row = attack.resource_action.resolve().map_err(invalid)?;
        let encounter = direct_encounter(
            memory,
            snap,
            &self.basis.stat,
            self.category,
            &row.key,
            row.bytes,
        )
        .map_err(HostError::from_runtime)?;
        let confirmed = direct_encounter(
            memory,
            snap,
            &self.basis.stat,
            self.category,
            &row.key,
            row.bytes,
        )
        .map_err(HostError::from_runtime)?;
        if encounter != confirmed
            || !matches!(nioh3_runtime::combat_context::read_combat_snapshot(memory).map_err(HostError::from_runtime)?,
            nioh3_runtime::combat_context::SnapshotOutcome::Ready(current) if current.roots==snap.roots && current.inputs==snap.inputs && current.status_tree_raw==snap.status_tree_raw)
        {
            return Err(invalid("role or encounter changed during preparation"));
        }
        let character = CharacterInputs {
            mode: input.mode,
            active: input.active_bank,
            cap: self.cap,
            plus_cap: plus,
            style: input.style,
            unarmed,
            level: input.level,
            core_mode: input.mode,
            core_maps: input.core_groups.each_ref().map(|m| {
                m.iter()
                    .map(|v| CoreEntry {
                        key: v.key,
                        base: v.base,
                        dup: v.dup,
                    })
                    .collect()
            }),
        };
        let provenance = Provenance {
            capture_id: format!("first-use-{}", self.digest),
            source_sha256: self.digest.clone(),
            game_version: [2, 0, 2, 0],
            description: format!("Current PC202 normal progress and equipped inventory, frontal training opponent, resource {} action {} row {}",
                attack.resource_action.resource, attack.resource_action.action, attack.resource_action.row),
        };
        let manifest = ContextManifest {
            schema: nioh3_loadout::context::CONTEXT_SCHEMA.into(),
            provenance,
            scenario: Scenario::CapturedScene {
                image_sha256: sha256(&encounter.image),
                image_known_sha256: sha256(&encounter.known),
                scene_entry_sha256: sha256(&encounter.entry),
                attack_weapon_slot: input.held_weapon_slot,
                main_flags_298: encounter.flags,
            },
            attrs: input.raw_attrs,
            character,
            non_gear: NonGearSources {
                attack_gain: seed.attack_gain,
                defense: seed.defense,
                layers: vec![],
            },
            inventory_sha256: sha256(&input.inventory),
            frame_sha256: sha256(&frame),
            native_scalars: None,
        };
        LoadoutContext::new_with_scene(
            manifest,
            input.inventory.clone(),
            frame,
            SceneBytes::CapturedScene {
                image: encounter.image.into(),
                image_known: encounter.known.into(),
                scene_entry: encounter.entry.into(),
            },
        )
        .map_err(|e| invalid(&e.to_string()))
    }
}

pub(crate) fn verify_equipped(
    engine: &nioh3_loadout::engine::Engine,
    snap: &CombatSnapshot,
) -> Result<(), HostError> {
    use nioh3_loadout::damage::DamageDetails;
    use nioh3_loadout::engine::{Backpack, Candidate, Limits, Outcome, SLOTS};
    let invalid = |s: &str| HostError::rejected(format!("LOADOUT_FIRST_USE_VERIFY: {s}"));
    let c = Candidate {
        attrs: engine.start_attrs,
        items: [None; SLOTS],
    };
    let pack = Backpack::from_slots(&[]).map_err(invalid)?;
    let actual = engine
        .prepared_stats(&c, &pack, &mut engine.work())
        .map_err(invalid)?;
    let expected = &snap.inputs.stats;
    // Validate the actual scoring Engine as well as the SOURCE-only producer.
    // Both completed maps preserve ordering, zero-valued members and duplicates.
    let encoded = engine
        .encoded_context()
        .ok_or_else(|| invalid("engine has no character"))?;
    let frame = Frame::load(
        encoded
            .file("frame.bin")
            .ok_or_else(|| invalid("engine frame missing"))?,
    )
    .map_err(invalid)?;
    let character = Frame::characters(
        encoded
            .file("characters.bin")
            .ok_or_else(|| invalid("engine character missing"))?,
    )
    .map_err(invalid)?
    .into_iter()
    .next()
    .ok_or_else(|| invalid("empty engine character"))?;
    let mut scratch = nioh3_loadout::pipeline::Scratch::new();
    engine
        .tables()
        .pipeline(&frame, &character, &engine.start_attrs, &[], &mut scratch)
        .map_err(invalid)?;
    let trace = scratch
        .source_rebuild_trace
        .as_ref()
        .ok_or_else(|| invalid("engine did not rebuild current sources"))?;
    let bank = snap.inputs.mode as usize;
    let agrees = |a: &[nioh3_loadout::pipeline::Entry],
                  b: &[nioh3_runtime::combat_context::AggregateEntry]| {
        a.iter()
            .map(|v| (v.key, v.base, v.dup))
            .eq(b.iter().map(|v| (v.key, v.base, v.dup)))
    };
    if !agrees(&trace.completed_core[bank], &snap.inputs.core_groups[bank])
        || !agrees(&scratch.source.maps[bank], &snap.inputs.groups[bank])
    {
        return Err(invalid("engine source maps differ from the current role"));
    }
    if (
        actual.primary_attack,
        Some(actual.secondary_attack),
        actual.defense,
        actual.health,
    ) != (
        expected.primary_attack,
        expected.secondary_attack,
        expected.defense,
        expected.health,
    ) {
        return Err(invalid(
            "rebuilt primary/secondary attack, defense or health differs from the game",
        ));
    }
    let details = engine.damage_details(&c, &pack, &mut engine.work());
    let Outcome::Damage(score) = engine.evaluate(&c, &pack, &Limits::NONE, &mut engine.work())
    else {
        return Err(invalid("equipped attack could not be scored"));
    };
    let DamageDetails::Available { contacts, .. } = details.verify_total(score) else {
        return Err(invalid(
            "equipped damage breakdown differs from its ranking score",
        ));
    };
    let expected_contacts = engine
        .native_words(&c, &pack, &mut engine.work())
        .map_err(invalid)?
        .1
        .len();
    if contacts.len() != expected_contacts {
        return Err(invalid(
            "equipped damage breakdown omits or adds a scoring contact",
        ));
    }
    Ok(())
}
