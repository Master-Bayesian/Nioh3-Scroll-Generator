//! Stable read-only combat inputs for the caller-verified PC 2.0.2.0 image.
//!
//! This decoder never opens a process or writes memory. The protected caller
//! verifies the executable before supplying InventoryMemory. Ready means two
//! equal logical reads plus unchanged roots and process birth, not a complete
//! skill/buff causal model or atomicity across every game thread.
use crate::{
    error::RuntimeError,
    inventory::{InventoryMemory, InventoryProcess},
};
use std::collections::{BTreeMap, BTreeSet};

pub const COMBAT_GAME_VERSION: &str = "2.0.2.0";
pub const INVENTORY_BYTES: usize = 0x2018;
pub const SOURCE_BYTES: usize = 0x6000;
pub const PROFILE_BYTES: usize = 0x46;
const PLAYER_GLOBAL: u64 = 0x474_5348;
const PROFILE_GLOBAL: u64 = 0x475_14D0;
const PROGRESS_GLOBAL: u64 = 0x475_1860;
const BANK_BYTES: usize = 0xF70;
const MAP_OFFSETS: [usize; 2] = [0x35D0, 0x4728];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AggregateEntry {
    pub key: u16,
    pub base: u32,
    pub dup: u16,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod portable_tests {
    use super::*;
    use crate::inventory::FixtureMemory;
    use serde_json::{json, Value};
    use std::{cell::Cell, fs, path::PathBuf};

    #[test]
    fn duplicate_source_keys_are_not_collapsed_into_one_observation() {
        let mut bytes = vec![0; SOURCE_BYTES];
        bytes[0x41D0..0x41D4].copy_from_slice(&2u32.to_le_bytes());
        bytes[0x35D0..0x35D2].copy_from_slice(&7u16.to_le_bytes());
        bytes[0x35DC..0x35DE].copy_from_slice(&7u16.to_le_bytes());
        assert!(groups(&bytes).is_err());
        bytes[0x35DC..0x35DE].copy_from_slice(&8u16.to_le_bytes());
        assert_eq!(groups(&bytes).unwrap()[0].len(), 2);
    }

    struct NumericChange {
        memory: FixtureMemory,
        source: u64,
        offset: usize,
        reads: Cell<usize>,
    }
    impl InventoryMemory for NumericChange {
        fn process(&self) -> InventoryProcess {
            self.memory.process()
        }
        fn module_base(&self) -> u64 {
            self.memory.module_base()
        }
        fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
            self.memory.recheck_creation_filetime()
        }
        fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            let mut raw = self.memory.read(address, size)?;
            if address == self.source && size == SOURCE_BYTES {
                let n = self.reads.get();
                self.reads.set(n + 1);
                if n > 0 {
                    raw[self.offset] ^= 1;
                }
            }
            Ok(raw)
        }
    }
    #[test]
    #[ignore = "Requires explicit retained memory evidence; no game process"]
    fn numeric_only_drift_cannot_become_a_ready_snapshot() {
        let root = PathBuf::from(
            std::env::var_os("NIOH3_REFRESH_EVIDENCE_ROOT").expect("retained fixture root"),
        );
        let fixture: Value =
            serde_json::from_slice(&fs::read(root.join("reaper-after-memory.json")).unwrap())
                .unwrap();
        let historical: CombatSnapshot =
            serde_json::from_slice(&fs::read(root.join("reaper-after-snapshot.json")).unwrap())
                .unwrap();
        assert!(
            historical.inputs.source_numeric_inputs.is_none(),
            "old capsule omission must remain explicit"
        );
        historical.validate().unwrap();
        let memory = FixtureMemory::from_json(&fixture).unwrap();
        let SnapshotOutcome::Ready(fresh) = read_combat_snapshot(&memory).unwrap() else {
            panic!("stable retained decoder refused")
        };
        fresh.validate().unwrap();
        assert_eq!(
            fresh.inputs.source_numeric_inputs,
            Some(SourceNumericInputs::from_source(&fresh.source_raw).unwrap())
        );
        let mut forged = (*fresh).clone();
        forged
            .inputs
            .source_numeric_inputs
            .as_mut()
            .unwrap()
            .core_records[6] ^= 1;
        assert!(
            forged.validate().is_err(),
            "logical input must bind to retained raw bytes"
        );
        let mut rows = Vec::new();
        for offset in [
            0x58usize, 0x5c, 0x70, 0x71, 0x72, 0x80, 0x86, 0xb8, 0x67a, 0x58c0,
        ] {
            let drift = NumericChange {
                memory: FixtureMemory::from_json(&fixture).unwrap(),
                source: historical.roots.source,
                offset,
                reads: Cell::new(0),
            };
            let outcome = read_combat_snapshot(&drift).unwrap();
            assert!(
                matches!(outcome, SnapshotOutcome::Changed(_)),
                "numeric-only drift at{offset:x} became Ready"
            );
            rows.push(json!({"offset":offset,"reads":drift.reads.get(),"outcome":"changed"}));
        }
        if let Some(path) = std::env::var_os("NIOH3_NUMERIC_RUNTIME_RESULT") {
            fs::write(path,serde_json::to_vec_pretty(&json!({"passed":true,"game_access":false,"ce_access":false,"old_omission_preserved":true,"new_logical_fields_known":true,"forged_raw_binding_rejected":true,"cases":rows})).unwrap()).unwrap();
        }
    }

    struct ObjectChange {
        memory: FixtureMemory,
        address: u64,
        offset: usize,
        reads: Cell<usize>,
    }
    impl InventoryMemory for ObjectChange {
        fn process(&self) -> InventoryProcess {
            self.memory.process()
        }
        fn module_base(&self) -> u64 {
            self.memory.module_base()
        }
        fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
            self.memory.recheck_creation_filetime()
        }
        fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            let mut bytes = self.memory.read(address, size)?;
            if address == self.address && size == 0xC0 {
                let n = self.reads.get();
                self.reads.set(n + 1);
                if n > 0 {
                    bytes[self.offset] ^= 1;
                }
            }
            Ok(bytes)
        }
    }

    #[test]
    #[ignore = "Requires the supplied portable Nov15 evidence, not a process or full game image"]
    fn packaged_decoder_retains_raw_graphs_and_rejects_same_key_object_drift() {
        let root =
            PathBuf::from(std::env::var_os("NIOH3_REFRESH_EVIDENCE_ROOT").expect("evidence root"));
        let mut cases = Vec::new();
        for stem in ["reaper", "reaper-after"] {
            let fixture: Value = serde_json::from_slice(
                &fs::read(root.join(format!("{stem}-memory.json"))).unwrap(),
            )
            .unwrap();
            let historical: Value = serde_json::from_slice(
                &fs::read(root.join(format!("{stem}-snapshot.json"))).unwrap(),
            )
            .unwrap();
            let mut expected: CombatSnapshot = serde_json::from_value(historical.clone()).unwrap();
            assert_eq!(
                serde_json::to_value(&expected).unwrap(),
                historical,
                "optional evidence fields must not fabricate historical raw bytes"
            );
            let memory = FixtureMemory::from_json(&fixture).unwrap();
            let SnapshotOutcome::Ready(snapshot) = read_combat_snapshot(&memory).unwrap() else {
                panic!("retained portable decoder refused {stem}")
            };
            let secondary = u(&memory, snapshot.roots.stat + 0x28).unwrap();
            expected.inputs.stats.secondary_attack = Some(secondary);
            let owner_word = u(&memory, snapshot.roots.owner + 0x188).unwrap();
            assert_eq!(owner_word as u8, 65);
            assert_eq!(snapshot.inputs.owner_flags_188, Some(owner_word));
            expected.inputs.owner_flags_188 = Some(owner_word);
            expected.inputs.source_numeric_inputs =
                Some(SourceNumericInputs::from_source(&expected.source_raw).unwrap());
            expected.inputs.compiled_source_observation = Some(
                CompiledSourceObservation::from_sources(
                    &expected.core_source_raw,
                    &expected.source_raw,
                )
                .unwrap(),
            );
            expected.inputs.owner_resource_bits = Some([
                u(&memory, snapshot.roots.owner + 0x15C).unwrap(),
                u(&memory, snapshot.roots.owner + 0x160).unwrap(),
            ]);
            expected.inputs.actor_agility_bits = Some([
                read(&memory, snapshot.roots.player, 1).unwrap()[0],
                read(&memory, snapshot.roots.player + 0x886, 1).unwrap()[0],
                read(&memory, snapshot.roots.player + 0x88A, 1).unwrap()[0],
            ]);
            assert_eq!(snapshot.inputs, expected.inputs);
            assert_eq!(snapshot.source_raw, expected.source_raw);
            assert_eq!(snapshot.core_source_raw, expected.core_source_raw);
            let raw = snapshot.status_tree_raw.as_ref().unwrap();
            assert_eq!(raw.nodes.len(), snapshot.inputs.status_keys.len());
            assert_eq!(
                snapshot.configured_skills_raw.as_ref().unwrap().nodes.len(),
                snapshot.inputs.configured_skills.len()
            );
            snapshot.validate().unwrap();
            let mut contradictory = snapshot.as_ref().clone();
            contradictory.source_raw[0x14] ^= 1;
            assert!(
                contradictory.validate().is_err(),
                "raw source contradiction was accepted"
            );
            let mut malformed = snapshot.inputs.clone();
            malformed.profile.truncate(1);
            assert!(malformed.validate_layout().is_err());
            malformed = snapshot.inputs.clone();
            malformed.active_bank = u32::MAX;
            assert!(malformed.validate_layout().is_err());
            if stem == "reaper" {
                assert_eq!(raw.objects.len(), 1);
                assert!(
                    raw.objects[0].bytes.is_some(),
                    "the retained type54 object must be collected"
                );
                for (name, offset) in [("magnitude", 0xA8), ("expiry", 0x20), ("class", 0)] {
                    let memory = ObjectChange {
                        memory: FixtureMemory::from_json(&fixture).unwrap(),
                        address: raw.objects[0].address,
                        offset,
                        reads: Cell::new(0),
                    };
                    assert!(
                        matches!(
                            read_combat_snapshot(&memory).unwrap(),
                            SnapshotOutcome::Changed(ChangeReason::StatusTree)
                        ),
                        "same logical key with changed {name} was accepted"
                    );
                }
                let mut graph = snapshot.as_ref().clone();
                graph.status_tree_raw.as_mut().unwrap().nodes[0].bytes[0x20] ^= 1;
                assert!(
                    graph.validate().is_err(),
                    "raw status key contradiction was accepted"
                );
            }
            cases.push(json!({"capture":stem,"status_nodes":raw.nodes.len(),
                "configured_nodes":snapshot.configured_skills_raw.as_ref().unwrap().nodes.len(),
                "secondary_attack":secondary,"owner_flags_188":owner_word,
                "owner_flag_byte_188":owner_word as u8,"legacy_graph_bytes_invented":false}));
        }
        // This is validation of the already completed fresh read's retained
        // bytes, not a new live-process observation or damage measurement.
        let live = root.join("live-refresh-20261115/eb44f06c141594617a44a767507a635fc443a7c217d9f61ae4ac7b60023dca77.json");
        let capsule: Value = serde_json::from_slice(&fs::read(live).unwrap()).unwrap();
        let snapshot: CombatSnapshot = serde_json::from_value(capsule["snapshot"].clone()).unwrap();
        snapshot.validate().unwrap();
        assert!(snapshot.status_tree_raw.is_none());
        assert!(snapshot.inputs.stats.secondary_attack.is_none());
        assert!(snapshot.inputs.owner_flags_188.is_none());
        assert_eq!(serde_json::to_value(snapshot).unwrap(), capsule["snapshot"]);
        let report = json!({"passed":true,"cases":cases,"fresh_Nov15_retained_snapshot_valid":true,
            "same_key_object_drift_controls":3,"live_process_accessed":false,
            "scope":"Actual read-only production decoder over packaged retained memory plus controlled byte mutations; no numerical full-MAIN acceptance."});
        if let Some(output) = std::env::var_os("NIOH3_LIVE_REFRESH_RESULTS") {
            let output = PathBuf::from(output);
            fs::create_dir_all(&output).unwrap();
            fs::write(
                output.join("runtime_decoder.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
        }
        println!("{report}");
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredSkill {
    pub key: u16,
    pub rank: u16,
    pub enabled: u16,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatStats {
    pub primary_attack: u32,
    /// Legacy capsules observed only the primary channel. Absence is not proof
    /// that the second channel has the same non-equipment contribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_attack: Option<u32>,
    pub defense: u32,
    pub health: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatRoots {
    pub player_root: u64,
    pub player: u64,
    pub owner: u64,
    /// Zero selects the inline owner+25A8 STAT; stat is the resolved address.
    pub selected_stat_pointer: u64,
    pub stat: u64,
    pub source: u64,
    pub core_source: u64,
    pub materializer: u64,
    pub profile_root: u64,
    pub profile: u64,
    pub progress_root: u64,
    pub progress: u64,
    pub entity_tag: u8,
    pub style_query: u64,
    pub style_entity: u64,
}

/// Every field here participates in the double-read equality check. The bounded
/// status tree and supported object bytes are also compared. Unmodelled actor
/// motion flags and undeclared raw source bytes do not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceNumericInputs {
    pub guardian_records: Vec<u8>,
    pub active_guardian: u8,
    pub guardian_numbers_disabled: u8,
    pub core_records: Vec<u8>,
}
impl SourceNumericInputs {
    pub fn from_source(source: &[u8]) -> Result<Self, RuntimeError> {
        if source.len() != SOURCE_BYTES {
            return Err(layout("source numeric input extent differs"));
        }
        Ok(Self {
            guardian_records: source[0x58..0x68].to_vec(),
            active_guardian: source[0x70],
            guardian_numbers_disabled: source[0x71],
            core_records: source[0x80..0x250].to_vec(),
        })
    }
}

/// Guardian and soul-core selections observed after rebuild. The compiled
/// passive bitmap records the producer's result. Rebuild admission still
/// requires the separate compiler-entry presence and parameter inputs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledSourceObservation {
    core: SourceSelections,
    selected: SourceSelections,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceSelections {
    guardian_records: [u8; 16],
    active_guardian: u8,
    guardian_numbers_disabled: u8,
    guardian_requirement: u16,
    core_records: Vec<u8>,
    compiled_passive_bits: [u8; 32],
}

impl SourceSelections {
    const CORE_RECORD_BYTES: usize = 8 * 0xE8;

    fn from_source(raw: &[u8]) -> Result<Self, RuntimeError> {
        if raw.len() != SOURCE_BYTES {
            return Err(layout("compiled source observation extent differs"));
        }
        let mut guardian_records = [0; 16];
        guardian_records.copy_from_slice(&raw[0x58..0x68]);
        let mut compiled_passive_bits = [0; 32];
        compiled_passive_bits.copy_from_slice(&raw[0x58C0..0x58E0]);
        Ok(Self {
            guardian_records,
            active_guardian: raw[0x70],
            guardian_numbers_disabled: raw[0x71],
            guardian_requirement: u16_at(raw, 0x72)?,
            core_records: raw[0x80..0x80 + Self::CORE_RECORD_BYTES].to_vec(),
            compiled_passive_bits,
        })
    }
}

impl CompiledSourceObservation {
    pub fn from_sources(core: &[u8], selected: &[u8]) -> Result<Self, RuntimeError> {
        Ok(Self {
            core: SourceSelections::from_source(core)?,
            selected: SourceSelections::from_source(selected)?,
        })
    }

    fn validate_layout(&self) -> Result<(), RuntimeError> {
        if [&self.core, &self.selected]
            .iter()
            .any(|stage| stage.core_records.len() != SourceSelections::CORE_RECORD_BYTES)
        {
            return Err(layout(
                "compiled source observation has truncated core records",
            ));
        }
        Ok(())
    }
}

/// Motion flags and undeclared raw source bytes remain outside the logical read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatInputs {
    pub mode: i32,
    pub active_bank: u32,
    pub held_weapon_slot: u32,
    pub style: i32,
    pub raw_attrs: [i32; 7],
    pub synchronized_attrs: [i32; 7],
    pub level: u32,
    pub inventory: Vec<u8>,
    pub groups: [Vec<AggregateEntry>; 2],
    pub core_groups: [Vec<AggregateEntry>; 2],
    pub core_raw_attrs: [i32; 7],
    pub core_synchronized_attrs: [i32; 7],
    pub core_level: u32,
    pub source_scalars: [u32; 3],
    /// Guardian selector/disable and first two core records consumed by numeric
    /// additions. New reads compare these in both logical observations; old
    /// capsules retain omission instead of inventing that stability evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_numeric_inputs: Option<SourceNumericInputs>,
    /// Included in both logical observations and bound to both raw SOURCE
    /// objects. Older snapshots retain the missing evidence as None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiled_source_observation: Option<CompiledSourceObservation>,
    pub core_source_scalars: [u32; 3],
    pub profile: Vec<u8>,
    pub progression_gate: i32,
    /// Raw IEEE754 bits, without guessing the class-1 aggregate's semantics.
    pub status_single_use_bits: [u32; 2],
    pub status_class0_bits: u32,
    pub status_class1_bits: u32,
    pub current_health: u64,
    pub maximum_health: u64,
    /// Bit1 selects the retained held-Reaper counter-limit branch. Other bits
    /// remain raw evidence; no general actor-flag semantics are inferred here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_flags_188: Option<u32>,
    /// Original2FEBFC resource input: owner+15C binary32 current and owner+160
    /// signed maximum. Kept as bits; old capsules do not invent this evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_resource_bits: Option<[u32; 2]>,
    /// Original81789C conditional minimum20 owner-entity gate: byte0,886,88A.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_agility_bits: Option<[u8; 3]>,
    pub status_keys: Vec<u16>,
    /// Configured entries and raw enabled/rank fields, not an activation verdict.
    pub configured_skills: Vec<ConfiguredSkill>,
    pub stats: CombatStats,
}

/// A bounded node read, owned by the same second logical observation as the
/// snapshot. Bytes outside the declared logical fields are evidence only.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCombatNode {
    pub address: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawStatusTree {
    pub header: Vec<u8>,
    pub sentinel: Vec<u8>,
    pub nodes: Vec<RawCombatNode>,
    pub objects: Vec<RawStatusObject>,
}

/// Type54 is the one retained class whose bounded object layout is supplied.
/// A different vtable is recorded as unknown rather than assigned this layout.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawStatusObject {
    pub node_address: u64,
    pub address: u64,
    pub vtable: u64,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfiguredSkills {
    pub header: Vec<u8>,
    /// Only this sentinel word is read by the bounded list collector.
    pub sentinel_next: u64,
    pub nodes: Vec<RawCombatNode>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatSnapshot {
    pub process: InventoryProcess,
    pub module_base: u64,
    pub roots: CombatRoots,
    pub inputs: CombatInputs,
    /// The second read's full buffers. Only their declared logical fields were
    /// compared; these bytes are retained for evidence, not a whole-buffer claim.
    pub source_raw: Vec<u8>,
    pub core_source_raw: Vec<u8>,
    /// Older v2 capsules retained logical membership only. Omission preserves
    /// their content identity and does not invent missing historical node bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_tree_raw: Option<RawStatusTree>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured_skills_raw: Option<RawConfiguredSkills>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotLoadedAt {
    PlayerRoot,
    Player,
    Owner,
    SelectedSource,
    CoreSource,
    Materializer,
    ProfileRoot,
    Profile,
    ProgressRoot,
    Progress,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeReason {
    ProcessIdentity,
    ModuleBase,
    RootGraph,
    OwnerAlias,
    StatOwnerAlias,
    SourceOwnerAlias,
    CoreSourceOwnerAlias,
    LogicalInputs,
    StatusTree,
    ConfiguredSkills,
}
#[derive(Debug, Clone)]
pub enum SnapshotOutcome {
    Ready(Box<CombatSnapshot>),
    NotLoaded(NotLoadedAt),
    Changed(ChangeReason),
}
enum Observation<T> {
    Value(T),
    NotLoaded(NotLoadedAt),
    Changed(ChangeReason),
}

fn layout(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InventoryChain {
        detail: format!("combat context: {}", detail.into()),
    }
}
fn add(address: u64, offset: u64) -> Result<u64, RuntimeError> {
    address
        .checked_add(offset)
        .ok_or_else(|| layout("address overflow"))
}
fn read(memory: &dyn InventoryMemory, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
    let bytes = memory.read(address, size)?;
    if bytes.len() != size {
        return Err(RuntimeError::ShortRead {
            address,
            expected: size,
            actual: bytes.len(),
        });
    }
    Ok(bytes)
}
fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, RuntimeError> {
    let b = bytes
        .get(offset..offset + 8)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| layout("short u64 field"))?;
    Ok(u64::from_le_bytes(b))
}
fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, RuntimeError> {
    let b = bytes
        .get(offset..offset + 4)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| layout("short u32 field"))?;
    Ok(u32::from_le_bytes(b))
}
fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, RuntimeError> {
    let b = bytes
        .get(offset..offset + 2)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| layout("short u16 field"))?;
    Ok(u16::from_le_bytes(b))
}
fn q(memory: &dyn InventoryMemory, address: u64) -> Result<u64, RuntimeError> {
    u64_at(&read(memory, address, 8)?, 0)
}
fn u(memory: &dyn InventoryMemory, address: u64) -> Result<u32, RuntimeError> {
    u32_at(&read(memory, address, 4)?, 0)
}
fn attrs(bytes: &[u8], offset: usize) -> Result<[i32; 7], RuntimeError> {
    let mut result = [0; 7];
    for (i, v) in result.iter_mut().enumerate() {
        *v = u32_at(bytes, offset + 4 * i)? as i32;
    }
    Ok(result)
}
fn scalar_words(bytes: &[u8]) -> Result<[u32; 3], RuntimeError> {
    Ok([
        u32_at(bytes, 0x5888)?,
        u32_at(bytes, 0x5890)?,
        u32_at(bytes, 0x5894)?,
    ])
}
fn groups(bytes: &[u8]) -> Result<[Vec<AggregateEntry>; 2], RuntimeError> {
    let mut output = [Vec::new(), Vec::new()];
    for (offset, rows) in MAP_OFFSETS.into_iter().zip(&mut output) {
        let count = u32_at(bytes, offset + 0xC00)? as usize;
        if count > 256 {
            return Err(layout("source aggregate exceeds its 256-entry buffer"));
        }
        let mut keys = BTreeSet::new();
        for index in 0..count {
            let at = offset + 12 * index;
            let key = u16_at(bytes, at)?;
            if !keys.insert(key) {
                return Err(layout("source aggregate contains a repeated key"));
            }
            rows.push(AggregateEntry {
                key,
                base: u32_at(bytes, at + 4)?,
                dup: u16_at(bytes, at + 8)?,
            });
        }
    }
    Ok(output)
}

impl CombatInputs {
    /// Check the public serialized boundary before any caller indexes a buffer.
    /// This validates shape and uniqueness, not source causality or game origin.
    pub fn validate_layout(&self) -> Result<(), RuntimeError> {
        if self.profile.len() != PROFILE_BYTES || self.inventory.len() != INVENTORY_BYTES {
            return Err(layout(
                "logical buffer lengths differ from the recorded layout",
            ));
        }
        if !matches!(self.mode, 0 | 1) || self.active_bank > 1 || self.held_weapon_slot > 1 {
            return Err(layout(
                "logical form, bank or held slot is outside the recorded layout",
            ));
        }
        for entries in self.groups.iter().chain(&self.core_groups) {
            if entries.len() > 256
                || entries
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != entries.len()
            {
                return Err(layout(
                    "logical source aggregate is oversized or has repeated keys",
                ));
            }
        }
        if self.status_keys.len() > 64 || self.status_keys.windows(2).any(|keys| keys[0] >= keys[1])
        {
            return Err(layout(
                "logical status membership is oversized or not unique and sorted",
            ));
        }
        if self.configured_skills.len() > 4096 {
            return Err(layout(
                "logical configured skills exceed the collector bound",
            ));
        }
        if let Some(observation) = &self.compiled_source_observation {
            observation.validate_layout()?;
        }
        Ok(())
    }
}

fn retained_status_keys(raw: &RawStatusTree) -> Result<Vec<u16>, RuntimeError> {
    if raw.header.len() != 16 || raw.sentinel.len() != 0x30 || raw.nodes.len() > 64 {
        return Err(layout("retained status tree has an invalid buffer length"));
    }
    let sentinel = u64_at(&raw.header, 0)?;
    let count = u64_at(&raw.header, 8)?;
    if sentinel == 0 || count != raw.nodes.len() as u64 || raw.objects.len() != raw.nodes.len() {
        return Err(layout("retained status tree identity or count differs"));
    }
    let mut nodes = BTreeMap::new();
    for node in &raw.nodes {
        if node.address == 0
            || node.address == sentinel
            || node.bytes.len() != 0x30
            || nodes.insert(node.address, node.bytes.as_slice()).is_some()
        {
            return Err(layout(
                "retained status tree has an invalid or repeated node",
            ));
        }
    }
    let mut object_nodes = BTreeSet::new();
    for object in &raw.objects {
        let node = nodes
            .get(&object.node_address)
            .ok_or_else(|| layout("retained status object has no recorded node"))?;
        if !object_nodes.insert(object.node_address)
            || object.address == 0
            || u64_at(node, 0x28)? != object.address
        {
            return Err(layout(
                "retained status object differs from its node binding",
            ));
        }
        if let Some(bytes) = &object.bytes {
            if bytes.len() != 0xC0 || u64_at(bytes, 0)? != object.vtable {
                return Err(layout("retained status object layout or vtable differs"));
            }
        }
    }
    let mut stack = vec![u64_at(&raw.sentinel, 8)?];
    let mut seen = BTreeSet::new();
    let mut keys = Vec::new();
    while let Some(address) = stack.pop() {
        if address == sentinel {
            continue;
        }
        if !seen.insert(address) {
            return Err(layout("retained status tree repeats a traversal node"));
        }
        let bytes = nodes
            .get(&address)
            .ok_or_else(|| layout("retained status tree references an unrecorded node"))?;
        keys.push(u16_at(bytes, 0x20)?);
        stack.push(u64_at(bytes, 0)?);
        stack.push(u64_at(bytes, 0x10)?);
    }
    if seen.len() != nodes.len() {
        return Err(layout("retained status tree contains unreachable nodes"));
    }
    keys.sort_unstable();
    if keys.windows(2).any(|keys| keys[0] == keys[1]) {
        return Err(layout("status tree contains repeated membership keys"));
    }
    Ok(keys)
}

fn retained_configured_skills(
    raw: &RawConfiguredSkills,
) -> Result<Vec<ConfiguredSkill>, RuntimeError> {
    if raw.header.len() != 0x18 || raw.nodes.len() > 4096 {
        return Err(layout(
            "retained configured skills have an invalid buffer length",
        ));
    }
    let sentinel = u64_at(&raw.header, 8)?;
    if sentinel == 0 || u64_at(&raw.header, 0x10)? != raw.nodes.len() as u64 {
        return Err(layout(
            "retained configured skills identity or count differs",
        ));
    }
    let mut current = raw.sentinel_next;
    let mut previous = sentinel;
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for node in &raw.nodes {
        if current == 0
            || current == sentinel
            || node.address != current
            || node.bytes.len() != 0x20
            || !seen.insert(current)
            || u64_at(&node.bytes, 8)? != previous
        {
            return Err(layout("retained configured skill link or node differs"));
        }
        output.push(ConfiguredSkill {
            key: u16_at(&node.bytes, 0x10)?,
            rank: u16_at(&node.bytes, 0x1A)?,
            enabled: u16_at(&node.bytes, 0x1C)?,
        });
        previous = current;
        current = u64_at(&node.bytes, 0)?;
    }
    if current != sentinel {
        return Err(layout(
            "retained configured skill list does not end at its sentinel",
        ));
    }
    Ok(output)
}

impl CombatSnapshot {
    /// Bind retained raw source and graph bytes to the typed inputs consumed by
    /// refresh. Legacy graph omission stays explicit; it is never filled with
    /// synthetic process bytes. This does not authenticate an external capsule.
    pub fn validate(&self) -> Result<(), RuntimeError> {
        self.inputs.validate_layout()?;
        for (
            raw,
            expected_attrs,
            expected_sync,
            expected_level,
            expected_scalars,
            expected_groups,
        ) in [
            (
                &self.source_raw,
                self.inputs.raw_attrs,
                self.inputs.synchronized_attrs,
                self.inputs.level,
                self.inputs.source_scalars,
                &self.inputs.groups,
            ),
            (
                &self.core_source_raw,
                self.inputs.core_raw_attrs,
                self.inputs.core_synchronized_attrs,
                self.inputs.core_level,
                self.inputs.core_source_scalars,
                &self.inputs.core_groups,
            ),
        ] {
            if raw.len() != SOURCE_BYTES
                || u64_at(raw, 8)? != self.roots.owner
                || attrs(raw, 0x14)? != expected_attrs
                || attrs(raw, 0x34)? != expected_sync
                || u32_at(raw, 0x30)? != expected_level
                || scalar_words(raw)? != expected_scalars
                || groups(raw)? != *expected_groups
            {
                return Err(layout(
                    "retained raw source differs from its logical inputs",
                ));
            }
        }
        if let Some(numeric) = &self.inputs.source_numeric_inputs {
            if *numeric != SourceNumericInputs::from_source(&self.source_raw)? {
                return Err(layout(
                    "retained source numeric fields differ from logical inputs",
                ));
            }
        }
        if let Some(observation) = &self.inputs.compiled_source_observation {
            if *observation
                != CompiledSourceObservation::from_sources(&self.core_source_raw, &self.source_raw)?
            {
                return Err(layout(
                    "retained compiled source choices differ from logical inputs",
                ));
            }
        }
        if let Some(raw) = &self.status_tree_raw {
            if retained_status_keys(raw)? != self.inputs.status_keys {
                return Err(layout(
                    "retained status tree differs from logical membership",
                ));
            }
            for object in &raw.objects {
                if object.bytes.is_some() && object.vtable != add(self.module_base, 0x3AA55D0)? {
                    return Err(layout(
                        "retained status object is not the versioned type54 layout",
                    ));
                }
                if let Some(bytes) = &object.bytes {
                    if u64_at(bytes, 8)? != self.roots.player {
                        return Err(layout("type54 object belongs to another source player"));
                    }
                }
            }
        }
        if let Some(raw) = &self.configured_skills_raw {
            if retained_configured_skills(raw)? != self.inputs.configured_skills {
                return Err(layout(
                    "retained configured skill bytes differ from logical entries",
                ));
            }
        }
        Ok(())
    }
}

fn roots(
    memory: &dyn InventoryMemory,
    base: u64,
) -> Result<Observation<CombatRoots>, RuntimeError> {
    let player_root = q(memory, add(base, PLAYER_GLOBAL)?)?;
    if player_root == 0 {
        return Ok(Observation::NotLoaded(NotLoadedAt::PlayerRoot));
    }
    let player = q(memory, add(player_root, 0x1338)?)?;
    if player == 0 {
        return Ok(Observation::NotLoaded(NotLoadedAt::Player));
    }
    let owner = q(memory, add(player, 0x3A0)?)?;
    if owner == 0 {
        return Ok(Observation::NotLoaded(NotLoadedAt::Owner));
    }
    if q(memory, owner)? != player {
        return Ok(Observation::Changed(ChangeReason::OwnerAlias));
    }
    let selected_stat_pointer = q(memory, add(owner, 0x2D90)?)?;
    let stat = if selected_stat_pointer == 0 {
        add(owner, 0x25A8)?
    } else {
        selected_stat_pointer
    };
    if q(memory, add(stat, 8)?)? != owner {
        return Ok(Observation::Changed(ChangeReason::StatOwnerAlias));
    }
    let source = q(memory, add(stat, 0x7D8)?)?;
    if source == 0 {
        return Ok(Observation::NotLoaded(NotLoadedAt::SelectedSource));
    }
    if q(memory, add(source, 8)?)? != owner {
        return Ok(Observation::Changed(ChangeReason::SourceOwnerAlias));
    }
    let core_source = q(memory, add(owner, 0x2D80)?)?;
    if core_source == 0 {
        return Ok(Observation::NotLoaded(NotLoadedAt::CoreSource));
    }
    if q(memory, add(core_source, 8)?)? != owner {
        return Ok(Observation::Changed(ChangeReason::CoreSourceOwnerAlias));
    }
    let materializer = q(memory, add(stat, 0x7C0)?)?;
    if materializer == 0 {
        return Ok(Observation::NotLoaded(NotLoadedAt::Materializer));
    }
    let profile_root = q(memory, add(base, PROFILE_GLOBAL)?)?;
    if profile_root == 0 {
        return Ok(Observation::NotLoaded(NotLoadedAt::ProfileRoot));
    }
    let profile = q(memory, add(profile_root, 8)?)?;
    if profile == 0 {
        return Ok(Observation::NotLoaded(NotLoadedAt::Profile));
    }
    let progress_root = q(memory, add(base, PROGRESS_GLOBAL)?)?;
    if progress_root == 0 {
        return Ok(Observation::NotLoaded(NotLoadedAt::ProgressRoot));
    }
    let progress = q(memory, progress_root)?;
    if progress == 0 {
        return Ok(Observation::NotLoaded(NotLoadedAt::Progress));
    }
    let entity_tag = read(memory, player, 1)?[0] & 15;
    let style_query = if entity_tag == 0 {
        q(memory, add(player, 0x3B0)?)?
    } else {
        0
    };
    let style_entity = if style_query != 0 {
        q(memory, add(style_query, 8)?)?
    } else {
        0
    };
    Ok(Observation::Value(CombatRoots {
        player_root,
        player,
        owner,
        selected_stat_pointer,
        stat,
        source,
        core_source,
        materializer,
        profile_root,
        profile,
        progress_root,
        progress,
        entity_tag,
        style_query,
        style_entity,
    }))
}

fn status_keys(
    memory: &dyn InventoryMemory,
    owner: u64,
    base: u64,
) -> Result<Observation<(Vec<u16>, RawStatusTree)>, RuntimeError> {
    let address = add(owner, 0x3108)?;
    let header = read(memory, address, 16)?;
    let sentinel = u64_at(&header, 0)?;
    let count = u64_at(&header, 8)?;
    if count > 64 {
        return Err(layout("status tree exceeds the 64-node collector bound"));
    }
    if sentinel == 0 {
        return Err(layout("status tree has a null sentinel"));
    }
    let head = read(memory, sentinel, 0x30)?;
    let mut stack = vec![u64_at(&head, 8)?];
    let mut seen = BTreeSet::new();
    let mut keys = Vec::new();
    let mut nodes = Vec::new();
    let mut objects = Vec::new();
    while let Some(node) = stack.pop() {
        if node == sentinel {
            continue;
        }
        if node == 0 || !seen.insert(node) {
            return Err(layout("status tree contains a null or repeated node"));
        }
        if seen.len() > 64 {
            return Err(layout("status tree traversal exceeds its collector bound"));
        }
        let bytes = read(memory, node, 0x30)?;
        keys.push(u16_at(&bytes, 0x20)?);
        stack.push(u64_at(&bytes, 0)?);
        stack.push(u64_at(&bytes, 0x10)?);
        let object = u64_at(&bytes, 0x28)?;
        if object == 0 {
            return Err(layout("status node has a null object"));
        }
        let vtable = q(memory, object)?;
        let object_bytes = if vtable == add(base, 0x3AA55D0)? {
            Some(read(memory, object, 0xC0)?)
        } else {
            None
        };
        if let Some(bytes) = &object_bytes {
            if u64_at(bytes, 0)? != vtable {
                return Ok(Observation::Changed(ChangeReason::StatusTree));
            }
        }
        objects.push(RawStatusObject {
            node_address: node,
            address: object,
            vtable,
            bytes: object_bytes,
        });
        nodes.push(RawCombatNode {
            address: node,
            bytes,
        });
    }
    if read(memory, address, 16)? != header || read(memory, sentinel, 0x18)? != head[..0x18] {
        return Ok(Observation::Changed(ChangeReason::StatusTree));
    }
    if keys.len() as u64 != count {
        return Err(layout("status tree count differs from its traversed nodes"));
    }
    keys.sort_unstable();
    let raw = RawStatusTree {
        header,
        sentinel: head,
        nodes,
        objects,
    };
    if retained_status_keys(&raw)? != keys {
        return Err(layout(
            "collected status tree differs from its logical membership",
        ));
    }
    Ok(Observation::Value((keys, raw)))
}
fn configured_skills(
    memory: &dyn InventoryMemory,
    progress: u64,
) -> Result<Observation<(Vec<ConfiguredSkill>, RawConfiguredSkills)>, RuntimeError> {
    let address = add(progress, 0x6020)?;
    let header = read(memory, address, 0x18)?;
    let sentinel = u64_at(&header, 8)?;
    let count = u64_at(&header, 0x10)?;
    if count > 4096 {
        return Err(layout(
            "configured skills exceed the 4096-node collector bound",
        ));
    }
    if sentinel == 0 {
        return Err(layout("configured skills have a null sentinel"));
    }
    let first = q(memory, sentinel)?;
    let mut node = first;
    let mut previous = sentinel;
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    let mut nodes = Vec::new();
    while node != sentinel {
        if node == 0 || !seen.insert(node) {
            return Err(layout("configured skills contain a null or repeated node"));
        }
        if seen.len() > 4096 {
            return Err(layout(
                "configured skill traversal exceeds its collector bound",
            ));
        }
        let bytes = read(memory, node, 0x20)?;
        if u64_at(&bytes, 8)? != previous {
            return Err(layout("configured skill predecessor link differs"));
        }
        output.push(ConfiguredSkill {
            key: u16_at(&bytes, 0x10)?,
            rank: u16_at(&bytes, 0x1A)?,
            enabled: u16_at(&bytes, 0x1C)?,
        });
        previous = node;
        node = u64_at(&bytes, 0)?;
        nodes.push(RawCombatNode {
            address: previous,
            bytes,
        });
    }
    if read(memory, address, 0x18)? != header || q(memory, sentinel)? != first {
        return Ok(Observation::Changed(ChangeReason::ConfiguredSkills));
    }
    if output.len() as u64 != count {
        return Err(layout(
            "configured skill count differs from its traversed nodes",
        ));
    }
    let raw = RawConfiguredSkills {
        header,
        sentinel_next: first,
        nodes,
    };
    if retained_configured_skills(&raw)? != output {
        return Err(layout(
            "collected configured skills differ from their logical entries",
        ));
    }
    Ok(Observation::Value((output, raw)))
}

fn once(
    memory: &dyn InventoryMemory,
    process: InventoryProcess,
    base: u64,
    roots: CombatRoots,
) -> Result<Observation<CombatSnapshot>, RuntimeError> {
    let source_raw = read(memory, roots.source, SOURCE_BYTES)?;
    let core_source_raw = read(memory, roots.core_source, SOURCE_BYTES)?;
    if u64_at(&source_raw, 8)? != roots.owner {
        return Ok(Observation::Changed(ChangeReason::SourceOwnerAlias));
    }
    if u64_at(&core_source_raw, 8)? != roots.owner {
        return Ok(Observation::Changed(ChangeReason::CoreSourceOwnerAlias));
    }
    let mode = u(memory, add(roots.owner, 0x2C)?)? as i32;
    let active_bank = u(memory, add(roots.owner, 0x2470)?)?;
    if active_bank > 1 {
        return Err(layout(
            "active bank is outside the complete two-bank inventory",
        ));
    }
    let inventory = read(memory, add(roots.owner, 0x590)?, INVENTORY_BYTES)?;
    let held_weapon_slot = u32::from(inventory[active_bank as usize * BANK_BYTES + 0xF68] == 0);
    let (status_keys, status_tree_raw) = match status_keys(memory, roots.owner, base)? {
        Observation::Value(v) => v,
        Observation::Changed(r) => return Ok(Observation::Changed(r)),
        Observation::NotLoaded(r) => return Ok(Observation::NotLoaded(r)),
    };
    let (configured_skills, configured_skills_raw) =
        match configured_skills(memory, roots.progress)? {
            Observation::Value(v) => v,
            Observation::Changed(r) => return Ok(Observation::Changed(r)),
            Observation::NotLoaded(r) => return Ok(Observation::NotLoaded(r)),
        };
    let inputs = CombatInputs {
        mode,
        active_bank,
        held_weapon_slot,
        style: if roots.entity_tag == 0 && roots.style_entity != 0 {
            u(memory, add(roots.style_entity, 0x85C)?)? as i32
        } else {
            0
        },
        raw_attrs: attrs(&source_raw, 0x14)?,
        synchronized_attrs: attrs(&source_raw, 0x34)?,
        level: u32_at(&source_raw, 0x30)?,
        groups: groups(&source_raw)?,
        core_groups: groups(&core_source_raw)?,
        core_raw_attrs: attrs(&core_source_raw, 0x14)?,
        core_synchronized_attrs: attrs(&core_source_raw, 0x34)?,
        core_level: u32_at(&core_source_raw, 0x30)?,
        source_scalars: scalar_words(&source_raw)?,
        source_numeric_inputs: Some(SourceNumericInputs::from_source(&source_raw)?),
        compiled_source_observation: Some(CompiledSourceObservation::from_sources(
            &core_source_raw,
            &source_raw,
        )?),
        core_source_scalars: scalar_words(&core_source_raw)?,
        profile: read(memory, roots.profile, PROFILE_BYTES)?,
        progression_gate: u(memory, add(roots.progress, 0xAC)?)? as i32,
        status_single_use_bits: [
            u(memory, add(roots.owner, 0x2E34)?)?,
            u(memory, add(roots.owner, 0x2E38)?)?,
        ],
        status_class0_bits: u(memory, add(roots.owner, 0x2E3C)?)?,
        status_class1_bits: u(memory, add(roots.owner, 0x2E40)?)?,
        current_health: q(memory, add(roots.owner, 0x58)?)?,
        maximum_health: q(memory, add(roots.owner, 0x50)?)?,
        owner_flags_188: Some(u(memory, add(roots.owner, 0x188)?)?),
        owner_resource_bits: Some([
            u(memory, add(roots.owner, 0x15C)?)?,
            u(memory, add(roots.owner, 0x160)?)?,
        ]),
        actor_agility_bits: Some([
            read(memory, roots.player, 1)?[0],
            read(memory, add(roots.player, 0x886)?, 1)?[0],
            read(memory, add(roots.player, 0x88A)?, 1)?[0],
        ]),
        stats: CombatStats {
            primary_attack: u(memory, add(roots.stat, 0x24)?)?,
            secondary_attack: Some(u(memory, add(roots.stat, 0x28)?)?),
            defense: u(memory, add(roots.stat, 0x1C8)?)?,
            health: u(memory, add(roots.stat, 0x18)?)?,
        },
        inventory,
        status_keys,
        configured_skills,
    };
    Ok(Observation::Value(CombatSnapshot {
        process,
        module_base: base,
        roots,
        inputs,
        source_raw,
        core_source_raw,
        status_tree_raw: Some(status_tree_raw),
        configured_skills_raw: Some(configured_skills_raw),
    }))
}
fn stable(
    memory: &dyn InventoryMemory,
    process: InventoryProcess,
    base: u64,
) -> Result<SnapshotOutcome, RuntimeError> {
    let before = match roots(memory, base)? {
        Observation::Value(v) => v,
        Observation::NotLoaded(r) => return Ok(SnapshotOutcome::NotLoaded(r)),
        Observation::Changed(r) => return Ok(SnapshotOutcome::Changed(r)),
    };
    let first = match once(memory, process, base, before.clone())? {
        Observation::Value(v) => v,
        Observation::NotLoaded(r) => return Ok(SnapshotOutcome::NotLoaded(r)),
        Observation::Changed(r) => return Ok(SnapshotOutcome::Changed(r)),
    };
    match roots(memory, base)? {
        Observation::Value(v) if v == before => {}
        Observation::Value(_) | Observation::NotLoaded(_) | Observation::Changed(_) => {
            return Ok(SnapshotOutcome::Changed(ChangeReason::RootGraph))
        }
    }
    let second = match once(memory, process, base, before.clone())? {
        Observation::Value(v) => v,
        Observation::NotLoaded(_) => return Ok(SnapshotOutcome::Changed(ChangeReason::RootGraph)),
        Observation::Changed(r) => return Ok(SnapshotOutcome::Changed(r)),
    };
    match roots(memory, base)? {
        Observation::Value(v) if v == before => {}
        Observation::Value(_) | Observation::NotLoaded(_) | Observation::Changed(_) => {
            return Ok(SnapshotOutcome::Changed(ChangeReason::RootGraph))
        }
    }
    if first.inputs != second.inputs {
        return Ok(SnapshotOutcome::Changed(ChangeReason::LogicalInputs));
    }
    if first.status_tree_raw != second.status_tree_raw {
        return Ok(SnapshotOutcome::Changed(ChangeReason::StatusTree));
    }
    second.validate()?;
    Ok(SnapshotOutcome::Ready(Box::new(second)))
}

/// Read through an already version-verified read-only handle. No retries,
/// process opening, calls into the game, defaults, or writes occur here.
pub fn read_combat_snapshot(memory: &dyn InventoryMemory) -> Result<SnapshotOutcome, RuntimeError> {
    let process = memory.process();
    let base = memory.module_base();
    if memory.recheck_creation_filetime()? != process.creation_filetime {
        return Ok(SnapshotOutcome::Changed(ChangeReason::ProcessIdentity));
    }
    let result = stable(memory, process, base)?;
    if memory.process() != process
        || memory.recheck_creation_filetime()? != process.creation_filetime
    {
        return Ok(SnapshotOutcome::Changed(ChangeReason::ProcessIdentity));
    }
    if memory.module_base() != base {
        return Ok(SnapshotOutcome::Changed(ChangeReason::ModuleBase));
    }
    Ok(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::inventory::FixtureMemory;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{cell::Cell, fs, path::PathBuf};

    fn number(value: &Value) -> u64 {
        value.as_u64().unwrap_or_else(|| {
            let text = value.as_str().unwrap();
            if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                u64::from_str_radix(hex, 16).unwrap()
            } else {
                text.parse().unwrap()
            }
        })
    }
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }
    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }
    fn fixture() -> Value {
        let directory = std::env::var_os("NIOH3_COMBAT_CAPTURE_ROOT")
            .map(PathBuf::from)
            .expect("set NIOH3_COMBAT_CAPTURE_ROOT to the current capture evidence directory");
        let hit_bytes = fs::read(directory.join("hit-02/capture.json")).unwrap();
        let hit: Value = serde_json::from_slice(&hit_bytes).unwrap();
        let sources: Value = serde_json::from_slice(
            &fs::read(directory.join("player-sources-03/capture.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(digest(&hit_bytes), sources["hit_sha256"]);
        assert_eq!(
            hit["identity"]["process_id"],
            sources["identity"]["process_id"]
        );
        assert_eq!(
            hit["identity"]["creation_filetime"],
            sources["identity"]["creation_filetime"]
        );
        let snapshot = &hit["raw_capture"]["snapshots"][0];
        let regions = snapshot["regions"].as_array().unwrap();
        let owner = number(
            &regions
                .iter()
                .find(|r| r["role"] == "source.HP_owner")
                .unwrap()["address_hex"],
        );
        let mut rows = Vec::new();
        for (name, address) in [
            ("inventory.bin", owner + 0x590),
            ("progress.bin", number(&sources["progress"])),
        ] {
            let bytes = fs::read(directory.join("player-sources-03").join(name)).unwrap();
            assert_eq!(digest(&bytes), sources["files"][name]["sha256"]);
            rows.push(json!({"address":address,"bytes":hex(&bytes)}));
        }
        // The recorded collector traversed forward from this sentinel. Its
        // first-node address preserves the observed next link without inventing
        // the unrecorded sentinel payload or reading any live process.
        let configured = &sources["graphs"]["configured6020"];
        let header = unhex(configured["header_hex"].as_str().unwrap());
        let sentinel = u64_at(&header, 8).unwrap();
        let nodes = configured["nodes"].as_array().unwrap();
        assert_eq!(nodes.len() as u64, number(&configured["count"]));
        let first = nodes.first().map_or(sentinel, |n| number(&n["address"]));
        rows.push(json!({"address":sentinel,"bytes":hex(&first.to_le_bytes())}));
        for node in nodes {
            rows.push(json!({"address":number(&node["address"]),"bytes":node["hex"]}));
        }
        for region in regions {
            let Some(bytes) = region["bytes_hex"].as_str() else {
                continue;
            };
            let Some(known) = region["known_mask_hex"].as_str() else {
                continue;
            };
            if region["read_status"] != "read"
                || bytes.len() != known.len()
                || !known.bytes().all(|b| b == b'f' || b == b'F')
            {
                continue;
            }
            rows.push(json!({"address":number(&region["address_hex"]),"bytes":bytes}));
        }
        json!({"module_base":number(&hit["identity"]["module_base"]),"pid":number(&hit["identity"]["process_id"]),"creation_filetime":number(&hit["identity"]["creation_filetime"]),"regions":rows,
            "fixture_provenance":{"hit_sha256":digest(&hit_bytes),"source_capture":"player-sources-03","scope":"stored capture composite; not a new atomic game observation","sentinel_next":"reconstructed from the recorded forward traversal order"}})
    }
    #[derive(Clone, Copy)]
    enum Mutation {
        None,
        Birth,
        PlayerNull,
        OwnerAlias,
        SourceGraph,
        LogicalAttrs,
        UnmodelledByte,
        BadCount,
        ShortSource,
        SkillEnabled,
        StatusHeader,
    }
    struct Controlled {
        memory: FixtureMemory,
        roots: CombatRoots,
        mutation: Mutation,
        reads: Cell<usize>,
        births: Cell<usize>,
        first_skill: u64,
    }
    impl InventoryMemory for Controlled {
        fn module_base(&self) -> u64 {
            self.memory.module_base()
        }
        fn process(&self) -> InventoryProcess {
            self.memory.process()
        }
        fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
            let n = self.births.get();
            self.births.set(n + 1);
            Ok(self.memory.process().creation_filetime
                + u64::from(matches!(self.mutation, Mutation::Birth) && n > 0))
        }
        fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            let mut bytes = self.memory.read(address, size)?;
            match self.mutation {
                Mutation::PlayerNull
                    if address == self.module_base() + PLAYER_GLOBAL && size == 8 =>
                {
                    bytes.fill(0)
                }
                Mutation::OwnerAlias if address == self.roots.owner && size == 8 => bytes.fill(0),
                Mutation::SourceGraph if address == self.roots.stat + 0x7D8 && size == 8 => {
                    let n = self.reads.get();
                    self.reads.set(n + 1);
                    if n > 0 {
                        bytes.copy_from_slice(&self.roots.core_source.to_le_bytes());
                    }
                }
                Mutation::LogicalAttrs
                | Mutation::UnmodelledByte
                | Mutation::BadCount
                | Mutation::ShortSource
                    if address == self.roots.source && size == SOURCE_BYTES =>
                {
                    let n = self.reads.get();
                    self.reads.set(n + 1);
                    match self.mutation {
                        Mutation::LogicalAttrs if n > 0 => bytes[0x14] ^= 1,
                        Mutation::UnmodelledByte => bytes[0x1000] ^= (n + 1) as u8,
                        Mutation::BadCount => {
                            bytes[0x41D0..0x41D4].copy_from_slice(&257u32.to_le_bytes())
                        }
                        Mutation::ShortSource => {
                            bytes.pop();
                        }
                        Mutation::None
                        | Mutation::Birth
                        | Mutation::PlayerNull
                        | Mutation::OwnerAlias
                        | Mutation::SourceGraph
                        | Mutation::LogicalAttrs
                        | Mutation::SkillEnabled
                        | Mutation::StatusHeader => {}
                    }
                }
                Mutation::SkillEnabled if address == self.first_skill && size == 0x20 => {
                    let n = self.reads.get();
                    self.reads.set(n + 1);
                    if n > 0 {
                        bytes[0x1C] ^= 1;
                    }
                }
                Mutation::StatusHeader if address == self.roots.owner + 0x3108 && size == 16 => {
                    let n = self.reads.get();
                    self.reads.set(n + 1);
                    if n > 0 {
                        bytes[8] ^= 1;
                    }
                }
                Mutation::None
                | Mutation::Birth
                | Mutation::PlayerNull
                | Mutation::OwnerAlias
                | Mutation::SourceGraph
                | Mutation::LogicalAttrs
                | Mutation::UnmodelledByte
                | Mutation::BadCount
                | Mutation::ShortSource
                | Mutation::SkillEnabled
                | Mutation::StatusHeader => {}
            }
            Ok(bytes)
        }
    }
    fn output_root() -> PathBuf {
        let base = std::env::var_os("NIOH3_BUILD_ROOT")
            .map(PathBuf::from)
            .expect("F: build root required");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = base
            .join("deliverables/combat-context")
            .join(format!("{}-{stamp}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }
    #[test]
    #[ignore = "Requires the current private game capture and source evidence"]
    fn captured_memory_snapshot_and_consistency_controls() {
        let root = output_root();
        let fixture = fixture();
        fs::write(
            root.join("fixture.json"),
            serde_json::to_vec(&fixture).unwrap(),
        )
        .unwrap();
        let memory = FixtureMemory::from_json(&fixture).unwrap();
        let value = read_combat_snapshot(&memory).unwrap();
        let SnapshotOutcome::Ready(snapshot) = value else {
            panic!("captured fixture did not produce Ready: {value:?}")
        };
        assert_eq!(snapshot.inputs.raw_attrs, [5, 45, 15, 200, 5, 60, 155]);
        assert_eq!(
            snapshot.inputs.synchronized_attrs,
            snapshot.inputs.raw_attrs
        );
        assert_eq!(snapshot.inputs.level, 450);
        assert_eq!(snapshot.inputs.mode, 0);
        assert_eq!(snapshot.inputs.active_bank, 0);
        assert_eq!(snapshot.inputs.held_weapon_slot, 0);
        assert_eq!(snapshot.inputs.style, 1);
        assert_eq!(snapshot.inputs.profile.len(), 0x46);
        assert_eq!(snapshot.source_raw.len(), SOURCE_BYTES);
        assert_eq!(snapshot.core_source_raw.len(), SOURCE_BYTES);
        assert_eq!(snapshot.inputs.current_health, 3198);
        assert_eq!(snapshot.inputs.maximum_health, 3198);
        assert_eq!(snapshot.inputs.configured_skills.len(), 839);
        assert!(!snapshot.inputs.groups[0].is_empty());
        assert!(
            snapshot.inputs.groups.iter().flatten().any(|g| g.base == 0),
            "zero-valued aggregate entries are retained"
        );
        let header = memory.read(snapshot.roots.progress + 0x6020, 0x18).unwrap();
        let first_skill = q(&memory, u64_at(&header, 8).unwrap()).unwrap();
        let mut controls = Vec::new();
        for (name, mutation, expected) in [
            ("stable", Mutation::None, "ready"),
            ("process_birth_changed", Mutation::Birth, "changed"),
            ("player_not_loaded", Mutation::PlayerNull, "not_loaded"),
            ("owner_backlink_changed", Mutation::OwnerAlias, "changed"),
            ("selected_source_changed", Mutation::SourceGraph, "changed"),
            ("attributes_changed", Mutation::LogicalAttrs, "changed"),
            (
                "unmodelled_source_byte_changed",
                Mutation::UnmodelledByte,
                "ready",
            ),
            ("aggregate_count_malformed", Mutation::BadCount, "error"),
            ("source_read_short", Mutation::ShortSource, "error"),
            (
                "configured_enabled_changed",
                Mutation::SkillEnabled,
                "changed",
            ),
            ("status_header_changed", Mutation::StatusHeader, "changed"),
        ] {
            let controlled = Controlled {
                memory: FixtureMemory::from_json(&fixture).unwrap(),
                roots: snapshot.roots.clone(),
                mutation,
                reads: Cell::new(0),
                births: Cell::new(0),
                first_skill,
            };
            let result = read_combat_snapshot(&controlled);
            let (status, detail) = match result {
                Ok(SnapshotOutcome::Ready(_)) => ("ready", String::new()),
                Ok(SnapshotOutcome::NotLoaded(r)) => ("not_loaded", format!("{r:?}")),
                Ok(SnapshotOutcome::Changed(r)) => ("changed", format!("{r:?}")),
                Err(e) => ("error", e.to_string()),
            };
            assert_eq!(status, expected, "{name}: {detail}");
            controls.push(json!({"case":name,"outcome":status,"detail":detail}));
        }
        let entries = |rows: &[Vec<AggregateEntry>; 2]| {
            rows.iter()
                .map(|bank| {
                    bank.iter()
                        .map(|g| json!({"key":g.key,"base":g.base,"dup":g.dup}))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        fs::write(root.join("results.json"),serde_json::to_vec_pretty(&json!({"passed":true,"scope":"same production InventoryMemory decoder over recorded capture bytes; no game access", "provenance":fixture["fixture_provenance"],"process":{"pid":snapshot.process.pid,"creation_filetime":snapshot.process.creation_filetime.to_string()},"attrs":snapshot.inputs.raw_attrs,"level":snapshot.inputs.level,"mode":snapshot.inputs.mode,"style":snapshot.inputs.style,"held_weapon_slot":snapshot.inputs.held_weapon_slot,"profile_hex":hex(&snapshot.inputs.profile),"progression_gate":snapshot.inputs.progression_gate,"class0_bits":snapshot.inputs.status_class0_bits,"class1_bits":snapshot.inputs.status_class1_bits,"health":{"current":snapshot.inputs.current_health,"maximum":snapshot.inputs.maximum_health},"stats":{"attack":snapshot.inputs.stats.primary_attack,"defense":snapshot.inputs.stats.defense,"health":snapshot.inputs.stats.health},"status_keys":snapshot.inputs.status_keys,"configured_skills":snapshot.inputs.configured_skills.iter().map(|s|json!({"key":s.key,"rank":s.rank,"enabled":s.enabled})).collect::<Vec<_>>(),"groups":entries(&snapshot.inputs.groups),"core_groups":entries(&snapshot.inputs.core_groups),"controls":controls})).unwrap()).unwrap();
        println!("Combat snapshot E2E evidence: {}", root.display());
    }
}
