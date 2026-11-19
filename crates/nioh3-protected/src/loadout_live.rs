//! Refresh the optimizer's current player inputs through the read-only runtime adapter.
use crate::error::HostError;
use nioh3_loadout::{
    context::{
        sha256, ContextManifest, CoreEntry, GroupOperation, LoadoutContext, Scenario, SceneBytes,
        SourceKind, SourceLayer,
    },
    engine::{Backpack, Candidate, Engine, SLOTS},
};
use nioh3_runtime::combat_context::{AggregateEntry, CombatInputs, CombatSnapshot, SnapshotOutcome};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::Arc};

fn invalid(detail: impl std::fmt::Display) -> HostError {
    HostError::rejected(format!("LOADOUT_CONTEXT_INVALID: {detail}"))
}

#[cfg(windows)]
pub fn read() -> Result<Option<SnapshotOutcome>, HostError> {
    #[cfg(debug_assertions)]
    if crate::offline_e2e::enabled() { return Err(HostError::rejected("OFFLINE_E2E_GAME_ACCESS_FORBIDDEN")); }
    use nioh3_runtime::{
        file_sha256, process_creation_filetime, single_process_id, verify_game_executable,
        FileVersion, ProcessInventoryMemory, ReadOnlyProcess, RuntimeError, GAME_IMAGE_NAME,
        GAME_MODULE_NAME, INVENTORY_EXECUTABLE_SHA256,
    };
    let pid = match single_process_id(GAME_IMAGE_NAME) {
        Ok(pid) => pid,
        Err(RuntimeError::ProcessAbsent { .. }) => return Ok(None),
        Err(error) => return Err(HostError::from_runtime(error)),
    };
    let creation = process_creation_filetime(pid).map_err(HostError::from_runtime)?;
    let process =
        ReadOnlyProcess::open(pid, GAME_MODULE_NAME, creation).map_err(HostError::from_runtime)?;
    let path = process.image_path().map_err(HostError::from_runtime)?;
    let status = verify_game_executable(&path);
    if status.file_version != Some(FileVersion::new(2, 0, 2, 0))
        || file_sha256(&path).map_err(HostError::from_runtime)? != INVENTORY_EXECUTABLE_SHA256
    {
        return Err(HostError::rejected("LOADOUT_CONTEXT_VERSION_UNSUPPORTED"));
    }
    nioh3_runtime::combat_context::read_combat_snapshot(&ProcessInventoryMemory::new(&process))
        .map(Some)
        .map_err(HostError::from_runtime)
}

#[cfg(not(windows))]
pub fn read() -> Result<Option<SnapshotOutcome>, HostError> {
    Ok(None)
}

fn total(base: u32, dup: u16) -> u32 {
    base.wrapping_add(dup.into())
}

fn observations(
    gear: &[CoreEntry],
    current: &[AggregateEntry],
) -> Result<Vec<GroupOperation>, HostError> {
    let mut remaining: BTreeMap<_, _> =
        gear.iter().map(|e| (e.key, total(e.base, e.dup))).collect();
    let mut operations = Vec::new();
    for entry in current {
        let value = total(entry.base, entry.dup);
        let previous = remaining.remove(&entry.key);
        let delta = i64::from(value) - i64::from(previous.unwrap_or(0));
        if delta != 0 || previous.is_none() {
            operations.push(GroupOperation {
                group: entry.key,
                value: delta,
            });
        }
    }
    for (group, value) in remaining {
        if value == 0 {
            return Err(invalid(
                "equipped source presence differs from the observed aggregate",
            ));
        }
        operations.push(GroupOperation {
            group,
            value: -i64::from(value),
        });
    }
    Ok(operations)
}

fn composed_groups(gear: &[CoreEntry], layers: &[SourceLayer]) -> BTreeMap<u16, u32> {
    let mut entries = gear.to_vec();
    for op in layers.iter().filter(|layer| !is_native_source(layer))
        .flat_map(|layer| &layer.operations) {
        if let Some(index) = entries.iter().position(|entry| entry.key == op.group) {
            let entry = &mut entries[index];
            entry.base = entry.base.wrapping_add(op.value as u32);
            if op.value < 0 && entry.base == 0 && entry.dup == 0 {
                entries.remove(index);
            }
        } else {
            entries.push(CoreEntry {
                key: op.group,
                base: op.value as u32,
                dup: 0,
            });
        }
    }
    entries
        .into_iter()
        .map(|entry| (entry.key, total(entry.base, entry.dup)))
        .collect()
}

fn is_native_source(layer: &SourceLayer) -> bool {
    matches!(layer.kind, SourceKind::NativeBeforeEquipment | SourceKind::NativeAfterSets)
}

/// Inputs that own a compiled native source program. Equipped record changes
/// are intentionally excluded: the candidate pipeline replays native equipment
/// and aggregate competition. Source recompilation is needed when these values
/// change; a matching final aggregate alone is not an upstream certificate.
pub(crate) fn native_source_input_digest(input: &CombatInputs) -> Result<String, HostError> {
    let mut value = serde_json::json!({
        "mode":input.mode,"style":input.style,"profile":input.profile,
        "level":input.level,"configured_skills":input.configured_skills,
        "raw_attrs":input.raw_attrs,"synchronized_attrs":input.synchronized_attrs,
        "core_raw_attrs":input.core_raw_attrs,"core_synchronized_attrs":input.core_synchronized_attrs,
        "core_level":input.core_level,"source_scalars":input.source_scalars,
        "core_source_scalars":input.core_source_scalars,"core_groups":input.core_groups,
    });
    if let Some(observation) = &input.compiled_source_observation {
        value["compiled_source"] = json!({
            "progression_gate": input.progression_gate,
            "observation": observation,
        });
    }
    Ok(sha256(&serde_json::to_vec(&value).map_err(invalid)?))
}

fn matches_native_source_binding(binding: &Value, input: &CombatInputs) -> Result<bool, HostError> {
    let observed = native_source_input_digest(input)?;
    Ok(binding["membership_scope"].as_str() == Some("same_type_equipment")
        && binding["inputs_sha256"].as_str() == Some(observed.as_str()))
}


/// Whether the stored non-equipment record still explains the live character under the outfit now worn. The stored
/// layers and gains were observed under some earlier outfit, so a difference means the equipment model's groups are
/// wrong for one of the two outfits, or a source counted as non-equipment depends on what is worn.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SourceCheck {
    Consistent,
    Reobserved {
        /// (group, value the stored record predicts, value in the game); a missing group counts as absent.
        groups: Vec<(u16, Option<u32>, Option<u32>)>,
        /// (stored, now) when they differ.
        attack_gain: Option<(u32, u32)>,
        defense: Option<(u32, u32)>,
    },
}

impl SourceCheck {
    pub(crate) fn compare(
        predicted: &BTreeMap<u16, u32>,
        observed: &BTreeMap<u16, u32>,
        attack_gain: (u32, u32),
        defense: (u32, u32),
    ) -> SourceCheck {
        let groups: Vec<_> = predicted
            .keys()
            .chain(observed.keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|key| predicted.get(key) != observed.get(key))
            .map(|key| {
                (
                    *key,
                    predicted.get(key).copied(),
                    observed.get(key).copied(),
                )
            })
            .collect();
        let differs = |pair: (u32, u32)| (pair.0 != pair.1).then_some(pair);
        match (groups.is_empty(), differs(attack_gain), differs(defense)) {
            (true, None, None) => SourceCheck::Consistent,
            (_, attack_gain, defense) => SourceCheck::Reobserved {
                groups,
                attack_gain,
                defense,
            },
        }
    }

    pub(crate) fn json(&self) -> Value {
        match self {
            SourceCheck::Consistent => json!({"state":"consistent"}),
            SourceCheck::Reobserved {
                groups,
                attack_gain,
                defense,
            } => {
                let pair = |p: &Option<(u32, u32)>| {
                    p.map(|(stored, now)| json!({"stored":stored,"now":now}))
                };
                json!({
                    "state":"reobserved",
                    "groups":groups.iter().map(|(group, predicted, observed)| json!({"group":group,"predicted":predicted,"observed":observed})).collect::<Vec<_>>(),
                    "attack_gain":pair(attack_gain),
                    "defense":pair(defense),
                })
            }
        }
    }
}

pub(crate) fn scene_word(image: &[u8], known: &[u8], address: u64) -> Result<u32, HostError> {
    let base = address & !0xFFFF;
    let chunk = image
        .as_chunks::<65544>()
        .0
        .iter()
        .find(|chunk| chunk[..8] == base.to_le_bytes())
        .ok_or_else(|| invalid("canonical scene word is not mapped"))?;
    let mask = known
        .as_chunks::<8200>()
        .0
        .iter()
        .find(|chunk| chunk[..8] == base.to_le_bytes())
        .ok_or_else(|| invalid("canonical scene known mask is missing"))?;
    let offset = (address & 0xFFFF) as usize;
    if offset + 4 > 65536 || (offset..offset + 4).any(|i| mask[8 + i / 8] >> (i % 8) & 1 == 0) {
        return Err(invalid("canonical scene word is unknown"));
    }
    Ok(u32::from_le_bytes(
        chunk[8 + offset..12 + offset].try_into().map_err(invalid)?,
    ))
}

pub(crate) fn scene_bytes(image: &[u8], known: &[u8], address: u64, length: usize) -> Result<Vec<u8>, HostError> {
    let mut result = Vec::with_capacity(length);
    for offset in (0..length).step_by(4) {
        let at = address.checked_add(offset as u64)
            .ok_or_else(|| invalid("canonical scene address overflows"))?;
        result.extend_from_slice(&scene_word(image, known, at)?.to_le_bytes());
    }
    result.truncate(length);
    Ok(result)
}

pub(crate) fn scene_pointer(image: &[u8], known: &[u8], address: u64) -> Result<u64, HostError> {
    Ok(u64::from_le_bytes(scene_bytes(image, known, address, 8)?.try_into()
        .map_err(|_| invalid("canonical pointer is not eight bytes"))?))
}

fn captured_status_keys(image: &[u8], known: &[u8], owner: u64) -> Result<Vec<u16>, HostError> {
    let head = scene_pointer(image, known, owner + 0x3108)?;
    let count = scene_pointer(image, known, owner + 0x3110)?;
    if head == 0 || count > 64 {
        return Err(invalid("captured status tree identity or count is unsupported"));
    }
    let mut stack = vec![scene_pointer(image, known, head.checked_add(8)
        .ok_or_else(|| invalid("captured status head overflows"))?)?];
    let mut seen = std::collections::BTreeSet::new();
    let mut keys = Vec::new();
    while let Some(node) = stack.pop() {
        if node == head { continue; }
        if node == 0 || !seen.insert(node) || seen.len() > 64 {
            return Err(invalid("captured status tree has an invalid traversal"));
        }
        let bytes = scene_bytes(image, known, node, 0x30)?;
        let pointer = |offset: usize| -> Result<u64, HostError> {
            Ok(u64::from_le_bytes(bytes[offset..offset+8].try_into().map_err(invalid)?))
        };
        stack.push(pointer(0)?);
        stack.push(pointer(0x10)?);
        keys.push(u16::from_le_bytes(bytes[0x20..0x22].try_into().map_err(invalid)?));
    }
    keys.sort_unstable();
    if keys.len() as u64 != count || keys.windows(2).any(|keys| keys[0] == keys[1]) {
        return Err(invalid("captured status tree count or membership differs"));
    }
    Ok(keys)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SnapshotUse {
    CurrentProcess,
    RetainedDiagnostic,
}

pub(crate) fn validate_status_objects(
    image: &[u8], known: &[u8], owner: u64,
    raw: &nioh3_runtime::combat_context::RawStatusTree,
) -> Result<(), HostError> {
    let unsupported = || HostError::rejected("LOADOUT_CONTEXT_STATUS_OBJECT_UNSUPPORTED");
    if scene_bytes(image, known, owner + 0x3108, 16)? != raw.header {
        return Err(unsupported());
    }
    let head = scene_pointer(image, known, owner + 0x3108)?;
    if scene_bytes(image, known, head, 0x30)? != raw.sentinel {
        return Err(unsupported());
    }
    for node in &raw.nodes {
        if scene_bytes(image, known, node.address, node.bytes.len())? != node.bytes {
            return Err(unsupported());
        }
    }
    for object in &raw.objects {
        let bytes = object.bytes.as_ref().ok_or_else(unsupported)?;
        if scene_bytes(image, known, object.address, bytes.len())? != *bytes {
            return Err(unsupported());
        }
    }
    Ok(())
}

/// Whole-object projection for the independently checked singleton type54
/// graph. The recipe changes only declared pointer roles; all 176 remaining
/// object bytes and all non-pointer tree bytes are copied from the observation.
/// Other classes retain exact-byte admission. A key alone never owns an object.
struct StatusProjection {
    writes: Vec<(u64, Vec<u8>)>,
    evidence: Value,
}

/// Module and source-player identity from the same observed snapshot.
/// The canonical HP owner remains a separate projection argument.
struct ObservedStatusIdentity {
    module_base: u64,
    source_player: u64,
}

fn project_type54_status(
    image: &[u8], known: &[u8], owner: u64, layout: &Value,
    raw: &nioh3_runtime::combat_context::RawStatusTree, observed: ObservedStatusIdentity,
    keys: &[u16],
) -> Result<StatusProjection, HostError> {
    let unsupported = || HostError::rejected("LOADOUT_CONTEXT_STATUS_PROJECTION_UNSUPPORTED");
    let q = |bytes: &[u8], at: usize| -> Result<u64, HostError> {
        Ok(u64::from_le_bytes(bytes.get(at..at + 8).ok_or_else(unsupported)?
            .try_into().map_err(invalid)?))
    };
    if layout["recipe"] != "type54_singleton_v1" || keys != [0x16B]
        || raw.header.len() != 16 || raw.sentinel.len() != 48
        || raw.nodes.len() != 1 || raw.objects.len() != 1
    { return Err(unsupported()); }
    let address = |name: &str| layout[name].as_u64().filter(|v| *v > 0).ok_or_else(unsupported);
    let head = address("head")?;
    let node = address("node")?;
    let object = address("object")?;
    let player = address("source_player")?;
    let vtable = address("vtable")?;
    if [head, node, object].into_iter().collect::<std::collections::BTreeSet<_>>().len() != 3
        || scene_pointer(image, known, owner + 0x3108)? != head
        // This second alias was found by the complete known-image scan. It is
        // a frozen scenario input, not an undeclared pointer to rebase.
        || scene_pointer(image, known, owner + 0x3118)? != head
        || scene_pointer(image, known, owner)? != player
        || scene_pointer(image, known, player + 0x3A0)? != owner
    { return Err(unsupported()); }
    let old_node = scene_bytes(image, known, node, 48)?;
    let old_object = scene_bytes(image, known, object, 192)?;
    scene_bytes(image, known, head, 48)?;
    if old_node[0x20..0x22] != 0x16Bu16.to_le_bytes()
        || q(&old_node, 40)? != object || q(&old_object, 0)? != vtable
        || q(&old_object, 8)? != player
    { return Err(unsupported()); }
    let observed_node = &raw.nodes[0];
    let observed_object = &raw.objects[0];
    let payload = observed_object.bytes.as_ref().ok_or_else(unsupported)?;
    if observed_node.bytes.len() != 48 || payload.len() != 192
        || observed_node.bytes[0x20..0x22] != 0x16Bu16.to_le_bytes()
        || q(&raw.header, 8)? != 1 || observed_object.node_address != observed_node.address
        || q(&observed_node.bytes, 40)? != observed_object.address
        || q(payload, 0)? != observed.module_base.checked_add(0x3AA55D0).ok_or_else(unsupported)?
        || q(payload, 8)? != observed.source_player
    { return Err(unsupported()); }
    let old_head = q(&raw.header, 0)?;
    let mapping = BTreeMap::from([(old_head, head), (observed_node.address, node),
                                 (observed_object.address, object)]);
    if old_head == 0 || mapping.len() != 3 { return Err(unsupported()); }
    let mut header = raw.header.clone();
    let mut sentinel = raw.sentinel.clone();
    let mut node_bytes = observed_node.bytes.clone();
    let mut object_bytes = payload.clone();
    for (bytes, offsets) in [(&mut header, &[0usize][..]),
        (&mut sentinel, &[0, 8, 16][..]), (&mut node_bytes, &[0, 8, 16, 40][..])]
    {
        for &offset in offsets {
            let replacement = mapping.get(&q(bytes, offset)?).ok_or_else(unsupported)?;
            bytes[offset..offset + 8].copy_from_slice(&replacement.to_le_bytes());
        }
    }
    object_bytes[0..8].copy_from_slice(&vtable.to_le_bytes());
    object_bytes[8..16].copy_from_slice(&player.to_le_bytes());
    let evidence = json!({"recipe":"type54_singleton_v1", "bytes_preserved":192,
        "pointer_roles":["tree_links","node_object","versioned_vtable","source_player"],
        "observed_object":observed_object.address, "canonical_object":object,
        "observed_payload_sha256":sha256(payload), "projected_payload_sha256":sha256(&object_bytes),
        "frozen_owner_3118":head, "general_payload_omission_proven":false});
    Ok(StatusProjection { writes:vec![(owner + 0x3108, header), (head, sentinel),
        (node, node_bytes), (object, object_bytes)], evidence })
}

/// Legacy v2 replay remains explicitly diagnostic and never authorizes a new
/// process read. SnapshotUse is independent from the receipt's origin label.
#[cfg(test)]
pub(crate) fn refresh_snapshot(
    template: &LoadoutContext,
    bundle: &Path,
    metadata: &mut Value,
    snapshot: &CombatSnapshot,
    usage: SnapshotUse,
) -> Result<RefreshedContext, HostError> {
    refresh_snapshot_for_scene(template, bundle, metadata, snapshot, usage,
        super::loadout::BuffScene::Current)
}

/// Acquire character inputs for the selected scenario. Current observations and
/// hypothetical activation conditions have different owners: only Current uses
/// the observed status aggregates. No scenario estimates an unbuffed value by
/// dividing a captured result or applies a final damage multiplier.
pub(crate) fn refresh_snapshot_for_scene(
    template: &LoadoutContext,
    bundle: &Path,
    metadata: &mut Value,
    snapshot: &CombatSnapshot,
    usage: SnapshotUse,
    scene: super::loadout::BuffScene,
) -> Result<RefreshedContext, HostError> {
    use super::loadout::BuffScene;
    metadata["refresh_policy"] = json!({"buff_scene":scene,"snapshot_use":usage});
    snapshot.validate().map_err(invalid)?;
    if usage == SnapshotUse::CurrentProcess && snapshot.inputs.owner_flags_188.is_none() {
        return Err(HostError::rejected("LOADOUT_CONTEXT_OWNER_BRANCH_PROVENANCE_MISSING"));
    }
    if !matches!(scene, BuffScene::Current) {
        let mut selected = snapshot.inputs.clone();
        // These scenes declare a neutral temporary-status state or the one
        // verified Counterattack Deflect damage pool. Arts activation remains
        // an attack-event input in its own bundle, not an arbitrary status flag.
        // Their source trees are empty; the class aggregate has an independent
        // native lifetime and is not reconstructed from tree presence.
        let deflect = matches!(scene, BuffScene::Deflect | BuffScene::DeflectArts);
        let arts = matches!(scene, BuffScene::Arts | BuffScene::DeflectArts);
        if (deflect || arts) && selected.mode != 0 {
            return Err(HostError::rejected("LOADOUT_BUFF_REQUIRES_SAMURAI"));
        }
        if deflect && !selected.configured_skills.iter().any(|skill|
            skill.key == 42922 && skill.rank > 0 && skill.enabled != 0)
        {
            return Err(HostError::rejected("LOADOUT_BUFF_SOURCE_NOT_ENABLED: Counterattack Deflect"));
        }
        let encoded = template.encode(&[]);
        let image = encoded.file("image.bin").ok_or_else(|| invalid("missing image.bin"))?;
        let known = encoded.file("image_known.bin").ok_or_else(|| invalid("missing image_known.bin"))?;
        let owner = metadata["bindings"]["canonical_source_owner"].as_u64()
            .ok_or_else(|| invalid("missing scenario source owner"))?;
        if !captured_status_keys(image, known, owner)?.is_empty() {
            return Err(invalid("declared activation scenario requires an empty source tree"));
        }
        let expected = if deflect { 1.1f32.to_bits() } else { 1.0f32.to_bits() };
        if scene_word(image, known, owner + 0x2E3C)? != expected {
            return Err(invalid("scenario class0 aggregate differs from its verified condition"));
        }
        selected.status_keys.clear();
        selected.status_single_use_bits = [1.0f32.to_bits(); 2];
        selected.status_class0_bits = expected;
        selected.status_class1_bits = 1.0f32.to_bits();
        let refreshed = refresh(template, bundle, metadata, &selected)?;
        metadata["buff_condition"] = json!({
            "kind":"declared_activation", "scene":scene,
            "source_eligibility": if deflect {"configured_ranked_counterattack_deflect"} else {"selected_form"},
            "class0_bits":expected, "single_use":"absent", "arts_activation":arts,
            "combination":"one_deflect_pool_with_optional_arts_event",
            "not_a_peak_over_all_statuses":true,
        });
        if deflect {
            metadata["buffs"] = json!([{"key":786,"name":"造成伤害增加","condition":"反攻格挡触发"}]);
        }
        return Ok(refreshed);
    }
    let mut projection = None;
    if !snapshot.inputs.status_keys.is_empty() {
        match &snapshot.status_tree_raw {
            Some(raw) => {
                let owner = metadata["bindings"]["canonical_source_owner"].as_u64()
                    .filter(|owner| owner.checked_add(0x10000).is_some())
                    .ok_or_else(|| invalid("the capture has no bounded canonical player binding"))?;
                let encoded = template.encode(&[]);
                let image = encoded.file("image.bin").ok_or_else(|| invalid("missing image.bin"))?;
                let known = encoded.file("image_known.bin").ok_or_else(|| invalid("missing image_known.bin"))?;
                if snapshot.inputs.status_keys == [0x16B]
                    && metadata["status_projection_canonical"].is_object()
                {
                    projection = Some(project_type54_status(image, known, owner,
                        &metadata["status_projection_canonical"], raw, ObservedStatusIdentity {
                            module_base: snapshot.module_base, source_player: snapshot.roots.player,
                        }, &snapshot.inputs.status_keys)?);
                } else {
                    validate_status_objects(image, known, owner, raw)?;
                }
            }
            None if usage == SnapshotUse::CurrentProcess => {
                return Err(HostError::rejected("LOADOUT_CONTEXT_STATUS_PROVENANCE_MISSING"));
            }
            None => {}
        }
    }
    if let Some(projection) = &projection {
        metadata["status_projection"] = projection.evidence.clone();
    }
    refresh_with_projection(template, bundle, metadata, &snapshot.inputs, projection.as_ref())
}

pub(crate) fn replace_scene_word(
    image: &mut [u8],
    known: &mut [u8],
    address: u64,
    bytes: &[u8],
) -> Result<(), HostError> {
    const IMAGE_RECORD: usize = 8 + 65536;
    const KNOWN_RECORD: usize = 8 + 8192;
    let base = address & !0xffff;
    let (chunk, mask) = image
        .as_chunks_mut::<IMAGE_RECORD>()
        .0
        .iter_mut()
        .zip(known.as_chunks_mut::<KNOWN_RECORD>().0.iter_mut())
        .find(|(chunk, _)| chunk[..8] == base.to_le_bytes())
        .ok_or_else(|| invalid("live combat word has no canonical image chunk"))?;
    let offset = (address & 0xffff) as usize;
    let end = offset + bytes.len();
    chunk
        .get_mut(8 + offset..8 + end)
        .ok_or_else(|| invalid("live combat word spans two chunks"))?
        .copy_from_slice(bytes);
    for index in offset..end {
        mask[8 + index / 8] |= 1 << (index % 8);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshEvidence {
    pub gear: [Vec<CoreEntry>; 2],
    pub predicted: Vec<(u16, u32)>,
    pub observed: [Vec<AggregateEntry>; 2],
    pub stored_attack_gain: u32,
    pub observed_attack_gain: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_secondary_attack_gain: Option<u32>,
    pub stored_defense_gain: u32,
    pub observed_defense_gain: u32,
}

pub struct RefreshedContext {
    pub context: LoadoutContext,
    pub evidence: RefreshEvidence,
    pub bundle_input_sha256: Option<String>,
}

fn checked_secondary_gain(primary_gain: u32, observed: Option<u32>, prepared: u32) -> Result<Option<u32>, HostError> {
    observed.map(|observed| {
        observed.checked_sub(prepared)
            .filter(|gain| *gain == primary_gain)
            .ok_or_else(|| HostError::rejected("LOADOUT_CONTEXT_SECONDARY_CHANNEL_UNSUPPORTED"))
    }).transpose()
}

fn actor_branch_matches(observed: u32, captured: u32) -> bool {
    ((observed ^ captured) & 2) == 0
}

pub fn refresh(
    template: &LoadoutContext,
    bundle: &Path,
    metadata: &mut Value,
    input: &CombatInputs,
) -> Result<RefreshedContext, HostError> {
    refresh_with_projection(template, bundle, metadata, input, None)
}

fn refresh_with_projection(
    template: &LoadoutContext, bundle: &Path, metadata: &mut Value,
    input: &CombatInputs, projection: Option<&StatusProjection>,
) -> Result<RefreshedContext, HostError> {
    input.validate_layout().map_err(invalid)?;
    // A hit-time CombatInputs projection does not carry the complete compiler
    // input digest/epoch. Never silently carry an imported source program into
    // a new live refresh, even when the observed final maps happen to match.
    if nioh3_loadout::pipeline::Frame::load(template.frame()).map_err(invalid)?
        .has_source_rebuild()
    {
        return Err(HostError::rejected("LOADOUT_SOURCE_REBUILD_REFRESH_REQUIRED"));
    }
    let mut manifest = template.manifest().clone();
    let bindings = &metadata["bindings"];
    let owner = bindings["canonical_source_owner"]
        .as_u64()
        .filter(|owner| owner.checked_add(0x10000).is_some())
        .ok_or_else(|| invalid("the capture has no canonical player binding"))?;
    if input.mode != manifest.character.mode || input.held_weapon_slot != 0 {
        return Err(HostError::rejected("LOADOUT_CONTEXT_SCHOOL_MISMATCH"));
    }
    if input.raw_attrs != input.synchronized_attrs
        || input.core_raw_attrs != input.core_synchronized_attrs
        || bindings["profile44"].as_u64() != Some(input.profile[0x44].into())
        || bindings["profile45"].as_u64() != Some(input.profile[0x45].into())
        || input.source_scalars != [0; 3]
    {
        return Err(HostError::rejected("LOADOUT_CONTEXT_PROFILE_UNSUPPORTED"));
    }
    if let Some(flags) = input.owner_flags_188 {
        let encoded = template.encode(&[]);
        let image = encoded.file("image.bin").ok_or_else(|| invalid("missing image.bin"))?;
        let known = encoded.file("image_known.bin").ok_or_else(|| invalid("missing image_known.bin"))?;
        if !actor_branch_matches(flags, scene_word(image, known, owner + 0x188)?) {
            return Err(HostError::rejected("LOADOUT_CONTEXT_RESOURCE_BRANCH_UNSUPPORTED"));
        }
    }
    // A different nonempty membership needs its own object and per-event
    // producer contract. Displaying current keys over a captured tree is unsafe.
    if !input.status_keys.is_empty() && projection.is_none() {
        let encoded = template.encode(&[]);
        let image = encoded.file("image.bin").ok_or_else(|| invalid("missing image.bin"))?;
        let known = encoded.file("image_known.bin").ok_or_else(|| invalid("missing image_known.bin"))?;
        if captured_status_keys(image, known, owner)? != input.status_keys {
            return Err(HostError::rejected("LOADOUT_CONTEXT_STATUS_MEMBERSHIP_UNSUPPORTED"));
        }
    }
    manifest.attrs = input.raw_attrs;
    manifest.character.active = input.active_bank;
    manifest.character.style = input.style;
    manifest.character.level = input.level;
    manifest.character.core_maps = input.core_groups.clone().map(|rows| {
        rows.into_iter()
            .map(|entry| CoreEntry {
                key: entry.key,
                base: entry.base,
                dup: entry.dup,
            })
            .collect()
    });
    manifest.inventory_sha256 = sha256(&input.inventory);
    let old_layers = std::mem::take(&mut manifest.non_gear.layers);
    let native_layers: Vec<_> = old_layers.iter().filter(|layer| is_native_source(layer)).cloned().collect();
    if !native_layers.is_empty() {
        let binding = &metadata["native_source_binding"];
        if !matches_native_source_binding(binding, input)? {
            return Err(HostError::rejected("LOADOUT_NATIVE_SOURCE_RECOMPILE_REQUIRED"));
        }
        manifest.non_gear.layers = native_layers.clone();
    }
    let scene = manifest.scenario.clone();
    let stored_gains = (manifest.non_gear.attack_gain, manifest.non_gear.defense);
    manifest.non_gear.attack_gain = 0;
    manifest.non_gear.defense = 0;
    let candidate = Candidate {
        attrs: input.raw_attrs,
        items: [None; SLOTS],
    };
    let pack = Backpack {
        pool: Vec::new(),
        ranges: Vec::new(),
        origin: Vec::new(),
    };
    let make_basis = |manifest: &ContextManifest| {
        let context = template
            .rebind_character_inputs(manifest.clone(), input.inventory.clone())
            .map_err(invalid)?;
        Engine::open_with_context(bundle, &context).map_err(invalid)
    };
    let basis = make_basis(&manifest)?;
    let bundle_input_sha256 = basis.bundle_input_digest();
    let captured_bank = template.manifest().character.active as usize * 0xF70;
    let live_bank = input.active_bank as usize * 0xF70;
    let item_key = |inventory: &[u8], offset: usize| {
        u16::from_le_bytes([inventory[offset], inventory[offset + 1]])
    };
    let expected_type = basis.item_type_class(item_key(template.inventory(), captured_bank));
    let actual_type = basis.item_type_class(item_key(&input.inventory, live_bank));
    if expected_type.is_none() || actual_type != expected_type {
        return Err(HostError::rejected("LOADOUT_CONTEXT_WEAPON_MISMATCH"));
    }

    let gear = basis
        .equipment_groups(&candidate, &pack, &mut basis.work())
        .map_err(invalid)?;
    let form = input.mode as usize;
    let current: BTreeMap<_, _> = input.groups[form]
        .iter()
        .map(|e| (e.key, total(e.base, e.dup)))
        .collect();
    let current_ops = observations(&gear[form], &input.groups[form])?;
    // Keep the independently observed source decomposition when its sum still
    // matches. It carries the proven zero-valued source entries and their order.
    let mut provenance = manifest.provenance.clone();
    let mut profile_identity = json!([
        input.raw_attrs,
        input.level,
        input.status_class0_bits,
        input.status_single_use_bits,
        input.current_health,
        input.maximum_health,
        current,
        input.status_keys,
        input
            .configured_skills
            .iter()
            .map(|s| [s.key, s.rank, s.enabled])
            .collect::<Vec<_>>()
    ]);
    if let Some(observation) = &input.compiled_source_observation {
        profile_identity.as_array_mut().ok_or_else(|| invalid("profile identity is not an array"))?
            .push(json!({"progression_gate": input.progression_gate, "compiled_source": observation}));
    }
    if let Some(numeric)=&input.source_numeric_inputs {
        profile_identity.as_array_mut().ok_or_else(||invalid("profile identity is not an array"))?.push(serde_json::to_value(numeric).map_err(invalid)?);
    }
    let profile_bytes=serde_json::to_vec(&profile_identity).map_err(invalid)?;
    provenance.source_sha256 = sha256(&profile_bytes);
    provenance.capture_id = format!("live-{}", &provenance.source_sha256[..12]);
    provenance.description =
        "Current game character in the recorded training-warrior attack scenario".into();
    manifest.provenance = provenance.clone();
    let predicted = composed_groups(&gear[form], &old_layers);
    manifest.non_gear.layers = if predicted == current {
        old_layers
    } else {
        let mut layers = native_layers;
        layers.push(SourceLayer {
            name: "current_non_equipment".into(),
            kind: SourceKind::ObservedNonEquipment,
            provenance,
            operations: current_ops,
        });
        layers
    };
    drop(basis);
    let basis = make_basis(&manifest)?;
    if basis.bundle_input_digest() != bundle_input_sha256 {
        return Err(invalid("attack bundle changed during source reconstruction"));
    }
    let stats = basis
        .prepared_stats(&candidate, &pack, &mut basis.work())
        .map_err(invalid)?;
    manifest.non_gear.attack_gain = input
        .stats
        .primary_attack
        .checked_sub(stats.primary_attack)
        .ok_or_else(|| invalid("current attack contribution is outside the additive model"))?;
    let observed_secondary_attack_gain = checked_secondary_gain(
        manifest.non_gear.attack_gain, input.stats.secondary_attack, stats.secondary_attack,
    )?;
    manifest.non_gear.defense = input
        .stats
        .defense
        .checked_sub(stats.defense)
        .ok_or_else(|| invalid("current defense contribution is outside the additive model"))?;
    if stats.health != input.stats.health {
        return Err(invalid(
            "current maximum health differs from the equipment model",
        ));
    }
    crate::loadout_numeric::integrate(&basis,input,&mut manifest,metadata).map_err(invalid)?;
    metadata["source_check"] = SourceCheck::compare(
        &predicted,
        &current,
        (stored_gains.0, manifest.non_gear.attack_gain),
        (stored_gains.1, manifest.non_gear.defense),
    )
    .json();
    metadata["source_channel_evidence"] = json!({
        "primary":"observed_residual",
        "secondary":if observed_secondary_attack_gain.is_some() {"observed_equal_residual"} else {"legacy_primary_only"},
    });
    metadata["actor_branch_evidence"] = json!(if input.owner_flags_188.is_some() {
        "observed_owner_188_bit1_matches_capture"
    } else {
        "legacy_frozen_owner_188_branch"
    });
    let encoded = template.encode(&[]);
    let file = |name: &str| {
        encoded
            .file(name)
            .ok_or_else(|| invalid(format!("missing {name}")))
    };
    let mut image = file("image.bin")?.to_vec();
    let mut known = file("image_known.bin")?.to_vec();
    if let Some([tag, active, cancelled]) = input.actor_agility_bits {
        let player = scene_pointer(&image, &known, owner)?;
        if (scene_word(&image, &known, player)? as u8 ^ tag) & 15 != 0 {
            return Err(HostError::rejected("LOADOUT_CONTEXT_SOURCE_ENTITY_KIND_CHANGED"));
        }
        // The vtable remains the canonical typed module pointer. Only the
        // observed actor flag bytes are replayed; target flags are untouched.
        replace_scene_word(&mut image, &mut known, player + 0x886, &[active])?;
        replace_scene_word(&mut image, &mut known, player + 0x88A, &[cancelled])?;
    }
    if let Some(projection) = projection {
        for (address, bytes) in &projection.writes {
            replace_scene_word(&mut image, &mut known, *address, bytes)?;
        }
    }
    let mut current_words = std::collections::BTreeMap::from([
        (owner + 0x2E34, input.status_single_use_bits[0]),
        (owner + 0x2E38, input.status_single_use_bits[1]),
    ]);
    if let Some(projection) = projection {
        // The first event owns the current snapshot. Later removal and rebuild
        // phases retain their independently declared event words.
        for (address, bytes) in &projection.writes {
            for (index, word) in bytes.as_chunks::<4>().0.iter().enumerate() {
                current_words.insert(address + (index * 4) as u64,
                    u32::from_le_bytes(*word));
            }
        }
    }
    if let Some([current, maximum]) = input.owner_resource_bits {
        current_words.insert(owner + 0x15C, current);
        current_words.insert(owner + 0x160, maximum);
    }
    // The production decoder proves empty membership. The standard native sentinel
    // links to itself,exactly as the retained post-consumption snapshot does.
    if input.status_keys.is_empty() {
        let head = u64::from(scene_word(&image, &known, owner + 0x3108)?)
            | (u64::from(scene_word(&image, &known, owner + 0x310C)?) << 32);
        for offset in [0, 8, 16] {
            current_words.insert(head + offset, head as u32);
            current_words.insert(head + offset + 4, (head >> 32) as u32);
        }
        current_words.insert(owner + 0x3110, 0);
        current_words.insert(owner + 0x3114, 0);
    }
    let (hit_addresses, hit_rows) = basis.hit_inputs();
    let mut runtime_rows = hit_rows.to_vec();
    let mut changed = false;
    for (&address, &value) in &current_words {
        replace_scene_word(&mut image, &mut known, address, &value.to_le_bytes())?;
        if let Some(index) = hit_addresses
            .iter()
            .position(|candidate| *candidate == address)
        {
            if runtime_rows[0][index] != value {
                runtime_rows[0][index] = value;
                changed = true;
            }
        }
    }
    let runtime_hits = if changed {
        Some(
            format!(
                "{}\n{}\n{}",
                runtime_rows.len(),
                hit_addresses
                    .iter()
                    .map(|address| format!("{address:x}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                runtime_rows
                    .iter()
                    .map(|row| row
                        .iter()
                        .map(|word| format!("{word:x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                        + "\n")
                    .collect::<String>()
            )
            .into_bytes(),
        )
    } else {
        None
    };
    let frame = match runtime_hits {
        Some(bytes) => {
            nioh3_loadout::pipeline::Frame::with_runtime_hit_inputs(template.frame(), &bytes)
                .map_err(invalid)?
        }
        None => template.frame().to_vec(),
    };
    manifest.frame_sha256 = sha256(&frame);
    drop(basis);
    let Scenario::CapturedScene {
        scene_entry_sha256,
        attack_weapon_slot,
        main_flags_298,
        ..
    } = scene
    else {
        return Err(invalid(
            "the live profile requires a captured attack scenario",
        ));
    };
    replace_scene_word(
        &mut image,
        &mut known,
        owner + 0x2e3c,
        &input.status_class0_bits.to_le_bytes(),
    )?;
    replace_scene_word(
        &mut image,
        &mut known,
        owner + 0x2e40,
        &input.status_class1_bits.to_le_bytes(),
    )?;
    replace_scene_word(
        &mut image,
        &mut known,
        owner + 0x50,
        &input.maximum_health.to_le_bytes(),
    )?;
    replace_scene_word(
        &mut image,
        &mut known,
        owner + 0x58,
        &input.current_health.to_le_bytes(),
    )?;
    manifest.scenario = Scenario::CapturedScene {
        image_sha256: sha256(&image),
        image_known_sha256: sha256(&known),
        scene_entry_sha256,
        attack_weapon_slot,
        main_flags_298,
    };
    let names = &metadata["skill_catalog"];
    let forms = &metadata["skill_forms"];
    let skills: Vec<_> = input.configured_skills.iter().filter(|skill| {
        let category = forms[skill.key.to_string()].as_u64();
        skill.enabled != 0 && (category == Some(0x17) || category == Some(0x15 + form as u64))
    }).map(|skill| json!({"key":skill.key,"rank":skill.rank,"name":names[skill.key.to_string()].as_str().map(str::to_owned).unwrap_or_else(||format!("Skill {:04X}",skill.key))})).collect();
    let buffs: Vec<_> = input.status_keys.iter().map(|&key| json!({"key":key,"name":metadata["buff_catalog"][key.to_string()].as_str().map(str::to_owned).unwrap_or_else(||format!("Status {key:04X}"))})).collect();
    metadata["skills"] = json!(skills);
    metadata["buffs"] = json!(buffs);
    if let Some(notes) = metadata["notes"].as_array_mut() {
        for note in notes.iter_mut() {
            if note
                .as_str()
                .is_some_and(|text| text.contains("武器及目标使用本次读取的数据"))
            {
                *note = json!("强化下一击若存在，仅作用于首击；计算目标沿用已验证的训练场快照。");
            }
        }
    }
    metadata["captured_at"] = json!(snapshot_time());
    let evidence = RefreshEvidence {
        gear,
        predicted: predicted.into_iter().collect(),
        observed: input.groups.clone(),
        stored_attack_gain: stored_gains.0,
        observed_attack_gain: manifest.non_gear.attack_gain,
        observed_secondary_attack_gain,
        stored_defense_gain: stored_gains.1,
        observed_defense_gain: manifest.non_gear.defense,
    };
    let context = LoadoutContext::new_with_scene(
        manifest,
        input.inventory.clone(),
        frame,
        SceneBytes::CapturedScene {
            image: Arc::from(image),
            image_known: Arc::from(known),
            scene_entry: Arc::from(file("scene_entry.bin")?),
        },
    )
    .map_err(invalid)?;
    Ok(RefreshedContext { context, evidence, bundle_input_sha256 })
}

fn snapshot_time() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut days = seconds / 86400;
    let mut year = 1970;
    let leap = |year| year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    while days >= 365 + u64::from(leap(year)) {
        days -= 365 + u64::from(leap(year));
        year += 1;
    }
    let months = [
        31,
        28 + u64::from(leap(year)),
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for length in months {
        if days < length {
            break;
        }
        days -= length;
        month += 1;
    }
    format!(
        "{year:04}-{month:02}-{:02}T{:02}:{:02}:{:02}Z",
        days + 1,
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn map(rows: &[(u16, u32)]) -> BTreeMap<u16, u32> {
        rows.iter().copied().collect()
    }

    #[test]
    fn equal_groups_and_gains_are_consistent() {
        let groups = map(&[(0x6A7C, 12), (0x7F69, 0)]);
        assert_eq!(
            SourceCheck::compare(&groups, &groups, (348, 348), (340, 340)),
            SourceCheck::Consistent
        );
    }

    #[test]
    fn every_kind_of_difference_is_reported_with_both_values() {
        // A changed value, a group only the stored record predicts, and one only the game holds. A zero-valued group
        // is present, so it differs from an absent one.
        let predicted = map(&[(1, 10), (2, 5), (4, 0)]);
        let observed = map(&[(1, 11), (3, 7)]);
        let check = SourceCheck::compare(&predicted, &observed, (348, 351), (340, 340));
        assert_eq!(
            check,
            SourceCheck::Reobserved {
                groups: vec![
                    (1, Some(10), Some(11)),
                    (2, Some(5), None),
                    (3, None, Some(7)),
                    (4, Some(0), None)
                ],
                attack_gain: Some((348, 351)),
                defense: None,
            }
        );
        let json = check.json();
        assert_eq!(json["state"], "reobserved");
        assert_eq!(
            json["groups"][1],
            json!({"group":2,"predicted":5,"observed":null})
        );
        assert_eq!(json["attack_gain"], json!({"stored":348,"now":351}));
        assert!(json["defense"].is_null());
    }

    #[test]
    fn a_gain_difference_alone_is_not_consistent() {
        let groups = map(&[(1, 10)]);
        assert_eq!(
            SourceCheck::compare(&groups, &groups, (348, 348), (340, 333)),
            SourceCheck::Reobserved {
                groups: vec![],
                attack_gain: None,
                defense: Some((340, 333))
            }
        );
    }

    #[test]
    fn secondary_residual_is_observed_or_explicitly_legacy() {
        assert_eq!(checked_secondary_gain(20, Some(120), 100).unwrap(), Some(20));
        assert!(checked_secondary_gain(20, Some(119), 100).is_err());
        assert!(checked_secondary_gain(20, Some(99), 100).is_err());
        assert_eq!(checked_secondary_gain(20, None, 100).unwrap(), None);
    }

    #[test]
    fn held_actor_counter_branch_must_match_the_captured_bit() {
        assert!(actor_branch_matches(65, 65));
        assert!(actor_branch_matches(1, 65));
        assert!(!actor_branch_matches(67, 65));
        assert!(!actor_branch_matches(65, 67));
    }

    #[test]
    fn same_status_key_cannot_reuse_a_different_or_unknown_object() {
        use nioh3_runtime::combat_context::{RawCombatNode, RawStatusObject, RawStatusTree};
        let base = 0x10000u64;
        let owner = base + 0x1000;
        let head = base + 0x5000;
        let node = base + 0x5040;
        let object = base + 0x5080;
        let mut image = base.to_le_bytes().to_vec(); image.resize(8 + 65536, 0);
        let mut known = base.to_le_bytes().to_vec(); known.resize(8 + 8192, 255);
        let mut header = head.to_le_bytes().to_vec(); header.extend_from_slice(&1u64.to_le_bytes());
        let mut sentinel = vec![0; 0x30];
        for offset in [0, 8, 16] { sentinel[offset..offset+8].copy_from_slice(&node.to_le_bytes()); }
        sentinel[0x19] = 1;
        let mut node_bytes = vec![0; 0x30];
        for offset in [0, 8, 16] { node_bytes[offset..offset+8].copy_from_slice(&head.to_le_bytes()); }
        node_bytes[0x20..0x22].copy_from_slice(&0x16Bu16.to_le_bytes());
        node_bytes[0x28..0x30].copy_from_slice(&object.to_le_bytes());
        let mut payload = vec![0; 0xC0];
        payload[..8].copy_from_slice(&0x40000u64.to_le_bytes());
        payload[0xA8..0xAC].copy_from_slice(&1.06f32.to_bits().to_le_bytes());
        for (address, bytes) in [(owner + 0x3108, &header), (head, &sentinel),
            (node, &node_bytes), (object, &payload)] {
            let offset = 8 + (address-base) as usize;
            image[offset..offset+bytes.len()].copy_from_slice(bytes);
        }
        let raw = RawStatusTree { header, sentinel, nodes:vec![RawCombatNode { address:node, bytes:node_bytes }],
            objects:vec![RawStatusObject { node_address:node, address:object, vtable:0x40000, bytes:Some(payload) }] };
        assert_eq!(captured_status_keys(&image, &known, owner).unwrap(), vec![0x16B]);
        validate_status_objects(&image, &known, owner, &raw).unwrap();
        for offset in [0, 0x20, 0xA8] {
            let mut changed = raw.clone();
            changed.objects[0].bytes.as_mut().unwrap()[offset] ^= 1;
            assert!(validate_status_objects(&image, &known, owner, &changed).is_err(),
                "same-key status object drift at {offset:x} was accepted");
        }
        let mut unknown = raw.clone(); unknown.objects[0].bytes = None;
        assert!(validate_status_objects(&image, &known, owner, &unknown).is_err());
        let offset = (object + 0xA8 - base) as usize;
        known[8 + offset/8] &= !(1 << (offset%8));
        assert!(validate_status_objects(&image, &known, owner, &raw).is_err(),
            "readable bytes without known provenance were accepted");
    }

    #[test]
    fn type54_projection_preserves_every_payload_byte_and_rejects_foreign_roots() {
        use nioh3_runtime::combat_context::{RawCombatNode, RawStatusObject, RawStatusTree};
        let base = 0x10000u64;
        let (owner, player, head, node, object) = (base+0x1000, base+0x6000,
            base+0x5000, base+0x5040, base+0x5080);
        let (module, observed_module, observed_player) = (0x10000000u64, 0x20000000u64, 0x30000000u64);
        let (raw_head, raw_node, raw_object) = (0x40000000u64, 0x40000100u64, 0x40000200u64);
        let mut image = base.to_le_bytes().to_vec(); image.resize(8+65536, 0);
        let mut known = base.to_le_bytes().to_vec(); known.resize(8+8192, 255);
        let put = |image: &mut Vec<u8>, address, bytes: &[u8]| {
            let offset = 8+(address-base) as usize;
            image[offset..offset+bytes.len()].copy_from_slice(bytes);
        };
        for (at, value) in [(owner, player), (player+0x3A0, owner), (owner+0x3108, head),
            (owner+0x3118, head), (object, module+0x3AA55D0), (object+8, player), (node+40, object)]
        { put(&mut image, at, &value.to_le_bytes()); }
        put(&mut image, node+32, &0x16Bu16.to_le_bytes());
        let mut header = raw_head.to_le_bytes().to_vec(); header.extend(1u64.to_le_bytes());
        let mut sentinel = vec![0u8;48]; let mut node_bytes = vec![0u8;48];
        for at in [0,8,16] {
            sentinel[at..at+8].copy_from_slice(&raw_node.to_le_bytes());
            node_bytes[at..at+8].copy_from_slice(&raw_head.to_le_bytes());
        }
        sentinel[0x19] = 1;
        node_bytes[32..34].copy_from_slice(&0x16Bu16.to_le_bytes());
        node_bytes[40..48].copy_from_slice(&raw_object.to_le_bytes());
        let mut payload: Vec<_> = (0..192).map(|i| (i*17) as u8).collect();
        payload[..8].copy_from_slice(&(observed_module+0x3AA55D0).to_le_bytes());
        payload[8..16].copy_from_slice(&observed_player.to_le_bytes());
        let raw = RawStatusTree { header, sentinel, nodes:vec![RawCombatNode { address:raw_node, bytes:node_bytes }],
            objects:vec![RawStatusObject { node_address:raw_node, address:raw_object,
                vtable:observed_module+0x3AA55D0, bytes:Some(payload.clone()) }] };
        let recipe = json!({"recipe":"type54_singleton_v1", "head":head,"node":node,
            "object":object,"source_player":player,"vtable":module+0x3AA55D0});
        let result = project_type54_status(&image, &known, owner, &recipe, &raw,
            ObservedStatusIdentity { module_base: observed_module, source_player: observed_player }, &[0x16B]).unwrap();
        let projected = &result.writes[3].1;
        assert_eq!(&projected[16..], &payload[16..]);
        assert_eq!(&projected[8..16], &player.to_le_bytes());
        assert!(project_type54_status(&image, &known, owner, &recipe, &raw,
            ObservedStatusIdentity { module_base: observed_module, source_player: observed_player+8 }, &[0x16B]).is_err());
        let mut missing = raw.clone(); missing.objects[0].bytes = None;
        assert!(project_type54_status(&image, &known, owner, &recipe, &missing,
            ObservedStatusIdentity { module_base: observed_module, source_player: observed_player }, &[0x16B]).is_err());
        let offset = (object+191-base) as usize;
        known[8+offset/8] &= !(1<<(offset%8));
        assert!(project_type54_status(&image, &known, owner, &recipe, &raw,
            ObservedStatusIdentity { module_base: observed_module, source_player: observed_player }, &[0x16B]).is_err());
    }
}


#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod compiled_source_refresh_e2e {
    use super::*;
    use nioh3_runtime::combat_context::CompiledSourceObservation;
    use std::path::PathBuf;

    #[test]
    #[ignore = "Requires a saved current source epoch for the native refresh replay"]
    fn saved_source_epoch_changes_retire_legacy_source_binding() {
        let evidence = PathBuf::from(std::env::var_os("NIOH3_SOURCE_REFRESH_EVIDENCE")
            .expect("current source refresh fixture root"));
        let input: CombatInputs = serde_json::from_slice(
            &std::fs::read(evidence.join("input.json")).unwrap(),
        ).unwrap();
        input.validate_layout().unwrap();
        let compiled = std::fs::read(evidence.join("core.bin")).unwrap();
        let selected = std::fs::read(evidence.join("selected.bin")).unwrap();
        let mut input = input;
        input.compiled_source_observation = Some(
            CompiledSourceObservation::from_sources(&compiled, &selected).unwrap(),
        );
        let binding = json!({
            "inputs_sha256": native_source_input_digest(&input).unwrap(),
            "membership_scope": "same_type_equipment",
        });
        assert!(matches_native_source_binding(&binding, &input).unwrap());

        let mut cases = Vec::new();
        for (name, selected_stage, offset) in [
            ("active_guardian", true, 0x70),
            ("guardian_requirement", true, 0x72),
            ("last_core_record", true, 0x80 + 7 * 0xE8),
            ("compiled_passive_bit", true, 0x58C0),
            ("core_guardian_record", false, 0x58),
        ] {
            let mut changed = input.clone();
            let mut core = compiled.clone();
            let mut source = selected.clone();
            if selected_stage { source[offset] ^= 1; } else { core[offset] ^= 1; }
            changed.compiled_source_observation = Some(
                CompiledSourceObservation::from_sources(&core, &source).unwrap(),
            );
            assert_eq!(changed.groups, input.groups);
            assert_eq!(changed.core_groups, input.core_groups);
            assert!(!matches_native_source_binding(&binding, &changed).unwrap(), "{name}");
            cases.push(name);
        }
        let mut changed = input.clone();
        changed.progression_gate += 1;
        assert!(!matches_native_source_binding(&binding, &changed).unwrap());
        cases.push("progression_gate");
        changed = input.clone();
        changed.compiled_source_observation = None;
        assert!(!matches_native_source_binding(&binding, &changed).unwrap());
        cases.push("missing_compiled_source");
        std::fs::write(evidence.join("REFRESH_CONTROL.json"), serde_json::to_vec_pretty(&json!({
            "passed": true, "same_generation_reused": true, "stale_cases": cases,
            "reference_sha256": binding["inputs_sha256"], "game_access": false,
        })).unwrap()).unwrap();
    }
}
