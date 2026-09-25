//! Port of `nioh3_scroll_editor/save_application.SaveApplication`.
//!
//! The save role owns snapshot-bound plans over the frozen `nioh3-save`
//! transaction host. Every write touches only the fixture the caller registered
//! through `save.register`; no user save and no game process is involved, and
//! the guarded commit semantics (quiescent baseline, backup before write,
//! durable replace, readback, no replay of an operation id) come from the crate
//! rather than being re-derived here.
//!
//! Layout note: the protocol ledger mirrors the shipped path and shape
//! (`<state_root>/v2-operations/<plan_id>.json`), so a restarted host reconciles
//! the same receipts. The transaction crate's own plan/receipt/backup bundle is
//! rooted one level down (`<state_root>/protected-internal`) so the two never
//! write the same file.
//!
//! Authority note: the save-core journal under
//! `<state_root>/protected-internal/v2-operations/<plan_id>.json` is the single
//! authority for whether an operation committed, rolled back, or still needs
//! recovery. This outer ledger projects that authority instead of deciding for
//! itself: `save.operation` and `save.operations` overlay the core receipt onto
//! a stale `executing` intent, and `commit` resolves the same intent through that
//! receipt. Two identities stay separate on the wire:
//! `details.reviewed_source_sha256` is the target generation the human reviewed
//! (the drift guard), while `details.installed_sha256` is the source bytes the
//! commit installed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use nioh3_data::PreviewResources;
use nioh3_domain::effect::{EffectResourceBytes, EffectTableIndex, GraceMap};
use nioh3_domain::install_materialize::{
    materialize_ng3_certified_install_record, materialize_ng3_certified_record,
    InstallMaterializeError,
};
use nioh3_domain::preview::{compose_auxiliary_preview, PreviewTables};
use nioh3_domain::record::{EFFECT_SLOT_BASE, EFFECT_SLOT_COUNT, EFFECT_SLOT_STRIDE};
use nioh3_domain::sequence::generate_challenge_attempt_count;
use nioh3_save::backup::{list_backup_entries, move_backup_to_recycle_bin};
use nioh3_save::codec::prepare_candidate_for_install;
use nioh3_save::error::SaveReadError;
use nioh3_save::inventory::{SaveInventory, ScrollInventoryEntry};
use nioh3_save::save::DecryptedSave;
use nioh3_save::transaction::{PlanCommand, PlanKind, SavePlan, SaveTransactionHost};
use nioh3_save::transform::{
    patch_local_scroll_header, patch_local_scroll_record, read_local_effect_slots, EffectPatch,
    HeaderPatch, InstallRequest, LocalEffectSlot, SlotEdit,
};
use nioh3_worker::engine::EngineContext;
use nioh3_worker::grace_map as worker_grace_map;
use nioh3_worker::recommended_level::RecommendedLevelCurve;

use crate::app::{JobContext, Role, RoleApplication};
use crate::error::HostError;

const PLAN_TTL: Duration = Duration::from_secs(600);
const RECORD_BYTES: usize = 0xE8;
/// `cache_application.grace_map_cache_path`'s directory under the state root.
const GRACE_MAP_CACHE_DIR: &str = "grace-output-maps";
/// `models.CandidateRecordStage` spellings the transfer carries.
const STAGE_FINAL_RECORD: &str = "final_record";
const STAGE_NATIVE_STAGE_ONE: &str = "native_stage_one";
const STAGE_EFFECT_SEQUENCE_ONLY: &str = "effect_sequence_only";
/// `models.ScrollCandidate.install_blocker`'s effect-sequence refusal.
const EFFECT_SEQUENCE_REFUSAL: &str =
    "当前候选只包含离线词条序列，而且该周目/稀有度尚未通过完整记录原生一致性门禁，\
暂不允许写入。";
/// `save_application.materialize_live_many`'s batch guard.
const MATERIALIZE_BATCH_REFUSAL: &str = "Expected 1-200 distinct candidates";
/// `save_application.materialize_live_many`'s non-materializing level guard.
const RECOMMENDED_LEVEL_REFUSAL: &str = "Regenerate candidate with the selected recommended level";
/// `save_application.SaveApplication.operation`'s stale-intent warning.
///
/// The protected host reports it for a claimed write whose outcome the save
/// core never recorded, so an interrupted operation never reads as success.
const STALE_INTENT_WARNING: &str =
    "Previous process ended before recording the outcome; inspect backups and \
     current save before further writes";

struct Snapshot {
    snapshot_id: String,
    source_sha256: String,
    inventory: SaveInventory,
}

struct Plan {
    save_id: String,
    source_hash: String,
    kind: &'static str,
    save_plan: SavePlan,
    expires_at: Instant,
}

/// The save role application.
pub struct SaveApplication {
    state_root: PathBuf,
    transaction_root: PathBuf,
    data_root: PathBuf,
    context: EngineContext,
    curve: RecommendedLevelCurve,
    saves: HashMap<String, PathBuf>,
    snapshots: HashMap<String, Snapshot>,
    plans: HashMap<String, Plan>,
    receipts: HashMap<String, Value>,
    /// The last validated decrypt per save, keyed by the container digest that
    /// was validated when it was produced. A repeated `save.inventory` for
    /// unchanged bytes reuses it; any digest change falls back to a fresh
    /// decrypt, so the cache can never serve a stale generation.
    inventory_cache: HashMap<String, (String, SaveInventory)>,
    /// `load_preview_resources`, held for the process lifetime after first use.
    preview_resources: Option<ResourceCache<PreviewResources>>,
    /// The shipped effect tables plus their measured Grace maps.
    materialization: Option<ResourceCache<MaterializationResources>>,
}

/// The verified tables one installation record is materialized from.
struct MaterializationResources {
    index: EffectTableIndex,
    effect: EffectResourceBytes,
}

/// The offline resource selection one frozen generation context resolved to.
///
/// A production context carries the exact installed executable version, so the
/// lazy loaders resolve the same versioned interface the search worker's
/// materializer uses. `None` is the opt-in, non-production legacy identity: it
/// keeps the pre-version loader so its explicit tests reproduce the shipped
/// v2.00.02 payload.
///
/// `cache_identity` is the digest of the same frozen context, so a cache entry
/// can never be handed to a different identity: the two values are read from one
/// context and compared together.
#[derive(Clone, PartialEq, Eq)]
struct ResourceBinding {
    cache_identity: String,
    version: Option<(u16, u16, u16, u16)>,
}

/// One lazily loaded resource set plus the binding it was loaded for.
struct ResourceCache<T> {
    binding: ResourceBinding,
    value: T,
}

impl SaveApplication {
    /// Build the save role for one state root and data root.
    pub fn new(
        state_root: PathBuf,
        data_root: &Path,
        context: EngineContext,
    ) -> Result<Self, HostError> {
        let curve = nioh3_worker::recommended_level::load(data_root)
            .map_err(|error| HostError::coded("RESOURCE_MISMATCH", error.to_string()))?;
        Ok(Self {
            transaction_root: state_root.join("protected-internal"),
            state_root,
            data_root: data_root.to_path_buf(),
            context,
            curve,
            saves: HashMap::new(),
            snapshots: HashMap::new(),
            plans: HashMap::new(),
            receipts: HashMap::new(),
            inventory_cache: HashMap::new(),
            preview_resources: None,
            materialization: None,
        })
    }

    fn host(&self) -> SaveTransactionHost {
        // The transaction's own plans and receipts stay private (they would
        // collide with the protected ledger under `v2-operations`), while the
        // bundles live in the canonical public root the shipped host uses, so a
        // user's existing backups are discovered and restored in place.
        let backup_root = self.backup_root();
        match host_fault_point() {
            Some(point) => {
                SaveTransactionHost::with_faults(&self.transaction_root, fault_set_for(point))
                    .with_backup_root(backup_root)
            }
            None => SaveTransactionHost::new(&self.transaction_root).with_backup_root(backup_root),
        }
    }

    /// `savegame.SaveInstaller`'s `state_root/backups`, unchanged by migration.
    fn backup_root(&self) -> PathBuf {
        self.state_root.join("backups")
    }

    fn ledger_dir(&self) -> PathBuf {
        self.state_root.join("v2-operations")
    }

    fn save_path(&self, save_id: &str) -> Result<PathBuf, HostError> {
        self.saves.get(save_id).cloned().ok_or_else(|| {
            HostError::rejected("Unknown save ID; select or discover the save first")
        })
    }

    fn register_path(&mut self, selected: PathBuf) -> Result<Value, HostError> {
        let name = selected
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_uppercase);
        if name.as_deref() != Some("SAVEDATA.BIN") {
            return Err(HostError::rejected("Select a character SAVEDATA.BIN"));
        }
        let account = nioh3_save::paths::account_id_from_save_path(&selected)
            .map_err(HostError::from_save)?;
        let slot = nioh3_save::paths::save_slot_index_from_path(&selected)
            .map_err(HostError::from_save)?;
        let save_id = save_id_for(&selected);
        if !self.saves.contains_key(&save_id) && self.saves.len() >= 16 {
            return Err(HostError::rejected(
                "Save registry is full; restart the idle save worker",
            ));
        }
        self.saves.insert(save_id.clone(), selected.clone());
        Ok(json!({
            "save_id": save_id,
            "path": python_path_string(&selected),
            "account_id": account.to_string(),
            "save_slot": slot,
        }))
    }

    fn register(&mut self, path: &str) -> Result<Value, HostError> {
        let selected = resolve_strict(Path::new(path))?;
        self.register_path(selected)
    }

    /// `SaveApplication.discover`: the shipped per-user save location.
    ///
    /// `savegame.discover_save_paths()` computes
    /// `%LOCALAPPDATA%/KoeiTecmo/NIOH3/Savedata` and never consults the state
    /// root, so the protected host must not either - the state root is the
    /// application's own data directory, which holds no game saves.
    fn discover(&mut self) -> Value {
        let mut saves = Vec::new();
        if let Some(root) = shipped_savedata_root() {
            if let Ok(paths) = nioh3_save::paths::discover_save_paths(&root) {
                for path in paths.into_iter().take(16) {
                    if let Ok(reference) = self.register_path(path) {
                        saves.push(reference);
                    }
                }
            }
        }
        json!({"saves": saves})
    }

    fn inventory(&mut self, save_id: &str) -> Result<Value, HostError> {
        let save_path = self.save_path(save_id)?;
        let before = sha256_file(&save_path)?;
        // The decrypted inventory is a pure function of the validated container
        // bytes: reuse it when the digest is unchanged and decrypt again the
        // moment it is not.
        let inventory = match self.inventory_cache.get(save_id) {
            Some((digest, cached)) if digest.eq_ignore_ascii_case(&before) => cached.clone(),
            _ => {
                let bytes = read_bytes(&save_path)?;
                let decrypted =
                    DecryptedSave::from_container(&bytes).map_err(HostError::from_save)?;
                let loaded = SaveInventory::load(&save_path, decrypted, true)
                    .map_err(HostError::from_save)?;
                if before != sha256_file(&save_path)? {
                    return Err(HostError::rejected("Save changed while reading inventory"));
                }
                self.inventory_cache
                    .insert(save_id.to_string(), (before.clone(), loaded.clone()));
                loaded
            }
        };
        let snapshot_id = new_snapshot_id();
        let mut entries = Vec::new();
        for entry in inventory.scroll_entries(false) {
            entries.push(self.entry_json(&entry)?);
        }
        let response = json!({
            "save_id": save_id,
            "snapshot_id": snapshot_id,
            "source_sha256": before,
            "account_id": inventory.account_id.to_string(),
            "empty_slots": inventory.empty_slots.len(),
            "entries": entries,
        });
        self.snapshots.insert(
            save_id.to_string(),
            Snapshot {
                snapshot_id,
                source_sha256: before,
                inventory,
            },
        );
        Ok(response)
    }

    fn entry_json(&self, entry: &ScrollInventoryEntry) -> Result<Value, HostError> {
        let record = *entry.record_bytes();
        let header = local_header(&record)?;
        let effects = read_local_effect_slots(&record).map_err(HostError::from_save)?;
        let seed = header["seed"].as_u64().unwrap_or(0) as u32;
        let recommended = header["recommended_level"].as_u64().unwrap_or(0) as i32;
        let canonical = self.curve.canonical_internal_level(recommended);
        let derived = json!({
            "initial_challenge_capacity":
                nioh3_domain::sequence::generate_challenge_attempt_count(seed),
            "remaining_challenge_attempts": record[0x33],
            "recommended_displayed_level": self.curve.displayed_level(canonical),
            "recommended_raw_was_clamped": recommended != canonical,
            // The stored raw value is reported unchanged so an editor can keep
            // it when nothing else changed instead of rewriting an over-cap record.
            "recommended_raw_level": recommended,
        });
        Ok(json!({
            "slot_index": entry.slot_index,
            "header": header,
            "effects": effects.into_iter().map(effect_json).collect::<Vec<_>>(),
            "derived": derived,
        }))
    }

    fn template(
        &self,
        save_id: &str,
        snapshot_id: &str,
        playthrough: u8,
    ) -> Result<Value, HostError> {
        let snapshot = self.snapshot(save_id, snapshot_id)?;
        let template = snapshot
            .inventory
            .template_record_for_playthrough(playthrough)
            .map_err(HostError::from_save)?;
        Ok(json!({
            "template_hex": hex(template.as_bytes()),
            "save_fingerprint": snapshot.inventory.decrypted().sha256(),
            "source_sha256": snapshot.source_sha256.to_lowercase(),
            "context_digest": self.context.digest(),
        }))
    }

    fn snapshot(&self, save_id: &str, snapshot_id: &str) -> Result<&Snapshot, HostError> {
        match self.snapshots.get(save_id) {
            Some(snapshot) if snapshot.snapshot_id == snapshot_id => Ok(snapshot),
            _ => Err(HostError::rejected("Snapshot expired; refresh inventory")),
        }
    }

    /// `SaveApplication.live_add_source`: broker-only handoff of the registered,
    /// still-current save path for the runtime side's backup checks.
    fn live_add_source(&mut self, save_id: &str, snapshot_id: &str) -> Result<Value, HostError> {
        self.current_snapshot_hash(save_id, snapshot_id)?;
        Ok(json!({"save_path": python_path_string(&self.save_path(save_id)?)}))
    }

    /// `SaveApplication.count_edit_source`: the occupied slot's record for a
    /// remaining-count plan.
    fn count_edit_source(
        &mut self,
        save_id: &str,
        snapshot_id: &str,
        slot_index: usize,
    ) -> Result<Value, HostError> {
        let (source_sha256, inventory) = self.snapshot_copy(save_id, snapshot_id)?;
        let entry = inventory
            .entry(slot_index)
            .map_err(|_| HostError::rejected("Select an occupied scroll slot"))?;
        Ok(json!({
            "count_source": {
                "save_path": python_path_string(&self.save_path(save_id)?),
                "source_sha256": source_sha256,
                "record_hex": hex(entry.record_bytes()),
            }
        }))
    }

    /// `SaveApplication._snapshot`: the retained hash, re-checked against disk.
    fn current_snapshot_hash(&self, save_id: &str, snapshot_id: &str) -> Result<String, HostError> {
        let snapshot = self.snapshot(save_id, snapshot_id)?;
        let save_path = self.save_path(save_id)?;
        if sha256_file(&save_path)? != snapshot.source_sha256 {
            return Err(HostError::rejected(
                "Save changed after preview; refresh inventory",
            ));
        }
        Ok(snapshot.source_sha256.clone())
    }

    fn snapshot_copy(
        &self,
        save_id: &str,
        snapshot_id: &str,
    ) -> Result<(String, SaveInventory), HostError> {
        let snapshot = self.snapshot(save_id, snapshot_id)?;
        Ok((snapshot.source_sha256.clone(), snapshot.inventory.clone()))
    }

    /// The resource selection the frozen generation context resolved to.
    ///
    /// Only the production variant carries an executable version, and it is
    /// never inferred: the host resolved it through the same
    /// `ContextSelection::Production` path the worker uses, so an unregistered
    /// version already failed closed at startup.
    fn resource_binding(&self) -> ResourceBinding {
        let version = match &self.context {
            EngineContext::Production(context) => Some((
                context.game_file_version.0,
                context.game_file_version.1,
                context.game_file_version.2,
                context.game_file_version.3,
            )),
            EngineContext::LegacyTest(_) => None,
        };
        ResourceBinding {
            cache_identity: self.context.digest().to_string(),
            version,
        }
    }

    /// `load_preview_resources`, loaded on first use like the shipped caches.
    ///
    /// The auxiliary half is composed from the context tables of the selected
    /// resource directory, so a production context must read the same versioned
    /// directory the worker's preview used instead of the shipped legacy one.
    fn preview_resources(&mut self) -> Result<&PreviewResources, HostError> {
        let binding = self.resource_binding();
        if !matches!(&self.preview_resources, Some(cache) if cache.binding == binding) {
            let loaded = match binding.version {
                Some(version) => {
                    nioh3_data::load_preview_resources_for_file_version(&self.data_root, version)
                }
                None => nioh3_data::load_preview_resources(&self.data_root),
            }
            .map_err(|error| HostError::rejected(error.to_string()))?;
            self.preview_resources = Some(ResourceCache {
                binding,
                value: loaded,
            });
        }
        self.preview_resources
            .as_ref()
            .map(|cache| &cache.value)
            .ok_or_else(|| HostError::rejected("auxiliary preview resources are unavailable"))
    }

    /// `load_effect_resource` plus its table index, loaded on first use.
    ///
    /// Both halves - the effect tables behind record composition and the
    /// measured Grace maps - come from the version-selected resource, so a
    /// production candidate is materialized from the tables its search-side
    /// preview was composed with.
    fn materialization_resources(&mut self) -> Result<&MaterializationResources, HostError> {
        let binding = self.resource_binding();
        if !matches!(&self.materialization, Some(cache) if cache.binding == binding) {
            let effect = match binding.version {
                Some(version) => {
                    nioh3_data::load_effect_resource_for_file_version(&self.data_root, version)
                }
                None => nioh3_data::load_effect_resource(&self.data_root),
            }
            .map_err(|error| HostError::rejected(error.to_string()))?;
            let index = EffectTableIndex::from_resource(&effect)
                .map_err(|error| HostError::rejected(format!("{error:?}")))?;
            self.materialization = Some(ResourceCache {
                binding,
                value: MaterializationResources { index, effect },
            });
        }
        self.materialization
            .as_ref()
            .map(|cache| &cache.value)
            .ok_or_else(|| HostError::rejected("effect resources are unavailable"))
    }

    /// `SaveApplication.auxiliary_preview`: the complete offline auxiliary
    /// object for one displayed Seed, serialized exactly like `json.dumps`.
    fn auxiliary_preview(&mut self, seed: u32, playthrough: u8) -> Result<Value, HostError> {
        let resources = self.preview_resources()?;
        let tables = PreviewTables {
            roster: &resources.roster,
            context: &resources.context,
            rules: &resources.rules,
            states: &resources.states,
        };
        let auxiliary = compose_auxiliary_preview(seed, playthrough, &tables)
            .map_err(|error| HostError::rejected(error.to_string()))?;
        let enemy_groups: Vec<Value> = auxiliary
            .enemy_groups
            .iter()
            .map(|group| {
                Value::Array(
                    group
                        .entries
                        .iter()
                        .map(|entry| {
                            json!({
                                "lookup_key": entry.lookup_key,
                                "role": entry.role,
                            })
                        })
                        .collect(),
                )
            })
            .collect();
        let special_rules: Vec<Value> = auxiliary
            .special_rules
            .entries
            .iter()
            .map(|entry| {
                json!({
                    "key": entry.key,
                    // `raw_value` is a binary32 in the reference and is carried
                    // through its exact f64 widening, which is what Python's
                    // `json.dumps(float(f32))` prints.
                    "raw_value": entry.raw_value.map(f64::from),
                    "display_value": entry.display_value,
                    "display_unit": entry.display_unit,
                    "display_grade": entry.display_grade,
                    "qualifier_kind": entry.qualifier_kind,
                    "qualifier_key": entry.qualifier_key,
                })
            })
            .collect();
        let value = json!({
            "terrain": {
                "value": auxiliary.terrain.value,
                "display_effect_keys": auxiliary.terrain.display_effect_keys,
            },
            "enemy_groups": enemy_groups,
            "special_rules": special_rules,
            "initial_challenge_capacity": generate_challenge_attempt_count(seed),
        });
        Ok(json!({"auxiliary_json": python_json(&value)}))
    }

    /// `SaveApplication.cached_grace`: the on-disk measured map for this save
    /// fingerprint, playthrough, rarity and generation context.
    ///
    /// The payload is decoded by the worker's existing Grace-map codec; only the
    /// save-context fingerprint gate and the cache path are added here, because
    /// the worker codec carries the digest gate alone.
    fn cached_grace(
        &mut self,
        save_id: &str,
        snapshot_id: &str,
        playthrough: u8,
        rarity: u8,
    ) -> Result<Value, HostError> {
        self.current_snapshot_hash(save_id, snapshot_id)?;
        let (_source_hash, inventory) = self.snapshot_copy(save_id, snapshot_id)?;
        let fingerprint = inventory.decrypted().sha256();
        let digest = self.context.digest().to_string();
        let path =
            grace_map_cache_path(&self.state_root, &fingerprint, playthrough, rarity, &digest);
        let payload = read_json_file(&path)?;
        let mapping = worker_grace_map::from_cache_payload(&payload, Some(&digest))
            .map_err(HostError::rejected)?;
        let stored = payload
            .get("context_fingerprint")
            .map(|value| match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();
        if stored.trim().to_lowercase() != fingerprint.trim().to_lowercase() {
            return Err(HostError::rejected(
                "Grace output map belongs to a different save context",
            ));
        }
        let cache = grace_map_to_cache_payload(&mapping, &fingerprint, &digest)?;
        Ok(json!({"cache_json": python_json(&cache)}))
    }

    /// `SaveApplication.materialize_live_many`: the broker-only installation
    /// producer.
    ///
    /// A certified effect-sequence candidate is materialized through the domain
    /// stage-one/final pair, and the two records stay separate: the exported
    /// `installation_record_hex` is the record the save must receive and
    /// `record_hex` is the completed record the reveal path produces. Every
    /// other candidate keeps the shipped non-materializing branch. Nothing is
    /// written here, so no plan, receipt or backup is created.
    fn materialize_live_many(
        &mut self,
        save_id: &str,
        snapshot_id: &str,
        candidates: &Value,
        recommended_level: u16,
        transfer_count: u32,
    ) -> Result<Value, HostError> {
        let payloads = candidates
            .as_array()
            .filter(|list| (1..=200).contains(&list.len()))
            .ok_or_else(|| HostError::rejected(MATERIALIZE_BATCH_REFUSAL))?;
        let mut seen: Vec<&str> = Vec::with_capacity(payloads.len());
        for payload in payloads {
            let candidate_id = payload
                .get("candidate_id")
                .and_then(Value::as_str)
                .ok_or_else(|| HostError::rejected(MATERIALIZE_BATCH_REFUSAL))?;
            if seen.contains(&candidate_id) {
                return Err(HostError::rejected(MATERIALIZE_BATCH_REFUSAL));
            }
            seen.push(candidate_id);
        }

        self.current_snapshot_hash(save_id, snapshot_id)?;
        let (_source_hash, inventory) = self.snapshot_copy(save_id, snapshot_id)?;
        let save_path = python_path_string(&self.save_path(save_id)?);
        let context_digest = self.context.digest().to_string();

        let mut exported = Vec::with_capacity(payloads.len());
        {
            let resources = self.materialization_resources()?;
            for payload in payloads {
                let source = import_candidate(payload, &context_digest)?;
                // `OperationCommand.INSTALL_GENERATED` through
                // `CandidateApplicationService.prepare_generated_install`; the
                // plan it returns is not read by this method.
                require_installable(&source)?;
                let level = payload
                    .get("level")
                    .and_then(Value::as_u64)
                    .ok_or_else(HostError::invalid_request)?;
                exported.push(materialize_one(
                    resources,
                    &inventory,
                    &context_digest,
                    &source,
                    level,
                    recommended_level,
                    transfer_count,
                )?);
            }
        }
        Ok(json!({"candidates": exported, "save_path": save_path}))
    }

    fn plan_payload(
        &mut self,
        save_id: &str,
        source_hash: &str,
        kind: &'static str,
        save_plan: SavePlan,
        preview: Value,
    ) -> Value {
        let response = json!({
            "plan_id": save_plan.plan_id,
            "save_id": save_id,
            "kind": kind,
            "source_sha256": source_hash,
            "expires_in_seconds": 600,
            "preview": preview,
        });
        self.plans.insert(
            save_plan.plan_id.clone(),
            Plan {
                save_id: save_id.to_string(),
                source_hash: source_hash.to_string(),
                kind,
                save_plan,
                expires_at: Instant::now() + PLAN_TTL,
            },
        );
        response
    }

    fn prepare_edit(
        &mut self,
        save_id: &str,
        snapshot_id: &str,
        edits: &Value,
    ) -> Result<Value, HostError> {
        let (source_hash, inventory) = self.snapshot_copy(save_id, snapshot_id)?;
        let edits = edits
            .as_array()
            .ok_or_else(|| HostError::rejected("Expected an edit list"))?;
        let mut seen: Vec<usize> = Vec::new();
        let mut slot_edits = Vec::new();
        let mut changes = Vec::new();
        for edit in edits {
            let slot_index = edit
                .get("slot_index")
                .and_then(Value::as_u64)
                .ok_or_else(HostError::invalid_request)? as usize;
            if seen.contains(&slot_index) {
                return Err(HostError::rejected(
                    "A slot cannot be edited twice in one plan",
                ));
            }
            seen.push(slot_index);
            let entry = inventory
                .entry(slot_index)
                .map_err(|_| HostError::rejected("Only occupied scroll slots may be edited"))?;
            let original = *entry.record_bytes();
            let header = header_patch(edit)?;
            let after_header = header_json(&header);
            // The editor always sends the whole header. When every field equals
            // the stored record the header is untouched, so its bytes -
            // including the `+0x08`/`+0x12`/`+0x31` mirrors - stay exactly as
            // stored instead of being re-normalized by an effect-only edit.
            let patch = if after_header == local_header(&original)? {
                original
            } else {
                patch_local_scroll_header(&original, &header).map_err(HostError::from_save)?
            };
            let replacement = patch_local_scroll_record(&patch, &effect_patches(edit)?)
                .map_err(HostError::from_save)?;
            changes.push(json!({
                "slot_index": slot_index,
                "before_header": local_header(&original)?,
                "after_header": after_header,
                "changed_offsets": original
                    .iter()
                    .zip(replacement.iter())
                    .enumerate()
                    .filter(|(_, (before, after))| before != after)
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>(),
                "before_effects": effects_json(&original)?,
                "after_effects": effects_json(&replacement)?,
            }));
            slot_edits.push(SlotEdit {
                slot_index,
                expected_original: original,
                replacement,
            });
        }
        let save_path = self.save_path(save_id)?;
        let plan = self
            .host()
            .plan_edit(&save_path, &source_hash, slot_edits)
            .map_err(HostError::from_save)?;
        Ok(self.plan_payload(
            save_id,
            &source_hash,
            "edit",
            plan,
            json!({"changes": changes, "local_only": true}),
        ))
    }

    fn prepare_delete(
        &mut self,
        save_id: &str,
        snapshot_id: &str,
        slots: &Value,
    ) -> Result<Value, HostError> {
        let (source_hash, inventory) = self.snapshot_copy(save_id, snapshot_id)?;
        let slots = slots
            .as_array()
            .ok_or_else(|| HostError::rejected("Delete requires distinct occupied scroll slots"))?;
        let mut indices: Vec<usize> = Vec::new();
        for slot in slots {
            let index = slot.as_u64().ok_or_else(|| {
                HostError::rejected("Delete requires distinct occupied scroll slots")
            })? as usize;
            inventory.entry(index).map_err(|_| {
                HostError::rejected("Delete requires distinct occupied scroll slots")
            })?;
            if indices.contains(&index) {
                return Err(HostError::rejected(
                    "Delete requires distinct occupied scroll slots",
                ));
            }
            indices.push(index);
        }
        let save_path = self.save_path(save_id)?;
        let plan = self
            .host()
            .plan_delete(&save_path, &source_hash, indices.clone())
            .map_err(HostError::from_save)?;
        Ok(self.plan_payload(
            save_id,
            &source_hash,
            "delete",
            plan,
            json!({"slots": indices, "local_only": true}),
        ))
    }

    /// The installation record for one candidate payload.
    ///
    /// Mirrors `save_application.SaveApplication.prepare_install`: the candidate
    /// is imported and policy-checked first, so a stale generation context is
    /// refused with the shipped context error; a certified
    /// `effect_sequence_only` candidate is then *materialized* through the same
    /// `materialize_one` the live batch path uses, instead of being refused for
    /// its stage. Every other candidate must already carry a complete
    /// installation record whose recommended level matches.
    fn resolve_install_record(
        &mut self,
        inventory: &SaveInventory,
        candidate: &Value,
        recommended_level: u16,
        transfer_count: u32,
    ) -> Result<([u8; RECORD_BYTES], Value), HostError> {
        let context_digest = self.context.digest().to_string();
        let source = import_candidate(candidate, &context_digest)?;
        require_installable(&source)?;
        if can_materialize_for_install(&source) {
            let level = candidate
                .get("level")
                .and_then(Value::as_u64)
                .ok_or_else(HostError::invalid_request)?;
            let exported = {
                let resources = self.materialization_resources()?;
                materialize_one(
                    resources,
                    inventory,
                    &context_digest,
                    &source,
                    level,
                    recommended_level,
                    transfer_count,
                )?
            };
            // The shipped preview keeps the *input* candidate's identity while
            // the record comes from the materialized pair.
            return install_record_from(candidate, &exported, recommended_level, transfer_count);
        }
        install_record(candidate, recommended_level, transfer_count)
    }

    fn prepare_install(
        &mut self,
        save_id: &str,
        snapshot_id: &str,
        candidate: &Value,
        recommended_level: u16,
        transfer_count: u32,
    ) -> Result<Value, HostError> {
        let (source_hash, inventory) = self.snapshot_copy(save_id, snapshot_id)?;
        // The shipped host imports the candidate before it looks at the record,
        // so a payload from another generation context is refused with the
        // context error even when its stage is also unacceptable. A certified
        // `effect_sequence_only` candidate is materialized here exactly as
        // `save_application.prepare_install` does.
        let (record, preview) =
            self.resolve_install_record(&inventory, candidate, recommended_level, transfer_count)?;
        let save_path = self.save_path(save_id)?;
        let plan = self
            .host()
            .plan_install(
                &save_path,
                &source_hash,
                InstallRequest {
                    candidate_record: record,
                    transfer_count,
                },
            )
            .map_err(HostError::from_save)?;
        Ok(self.plan_payload(save_id, &source_hash, "install", plan, preview))
    }

    fn prepare_install_many(
        &mut self,
        save_id: &str,
        snapshot_id: &str,
        candidates: &Value,
        recommended_level: u16,
        transfer_count: u32,
    ) -> Result<Value, HostError> {
        let list = candidates
            .as_array()
            .filter(|list| (1..=200).contains(&list.len()))
            .ok_or_else(|| HostError::rejected("Batch size must be 1-200"))?;
        let (source_hash, inventory) = self.snapshot_copy(save_id, snapshot_id)?;
        let mut requests = Vec::new();
        let mut items = Vec::new();
        for candidate in list {
            let (record, preview) = self.resolve_install_record(
                &inventory,
                candidate,
                recommended_level,
                transfer_count,
            )?;
            requests.push(InstallRequest {
                candidate_record: record,
                transfer_count,
            });
            items.push(preview);
        }
        let save_path = self.save_path(save_id)?;
        let plan = self
            .host()
            .plan_install_many(&save_path, &source_hash, requests)
            .map_err(HostError::from_save)?;
        Ok(self.plan_payload(
            save_id,
            &source_hash,
            "install_many",
            plan,
            json!({"count": items.len(), "items": items}),
        ))
    }

    fn prepare_restore(
        &mut self,
        save_id: &str,
        snapshot_id: &str,
        backup_id: &str,
    ) -> Result<Value, HostError> {
        let (source_hash, _inventory) = self.snapshot_copy(save_id, snapshot_id)?;
        let allowed = self.backups(save_id)?;
        let permitted = allowed["backups"].as_array().is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry["backup_id"].as_str() == Some(backup_id))
        });
        if !permitted {
            return Err(HostError::rejected(
                "Backup does not belong to the selected save",
            ));
        }
        let save_path = self.save_path(save_id)?;
        let plan = self
            .host()
            .plan(
                PlanKind::Restore,
                &save_path,
                &source_hash,
                PlanCommand::RestoreFromBackup {
                    backup_id: backup_id.to_string(),
                },
            )
            .map_err(HostError::from_save)?;
        Ok(self.plan_payload(
            save_id,
            &source_hash,
            "restore",
            plan,
            json!({"backup_id": backup_id}),
        ))
    }

    fn backups(&mut self, save_id: &str) -> Result<Value, HostError> {
        let save_path = self.save_path(save_id)?;
        let account = nioh3_save::paths::account_id_from_save_path(&save_path)
            .map_err(HostError::from_save)?;
        let slot = nioh3_save::paths::save_slot_index_from_path(&save_path)
            .map_err(HostError::from_save)?;
        let entries = list_backup_entries(&self.state_root).map_err(HostError::from_save)?;
        let mut backups = Vec::new();
        for entry in entries {
            if entry.account_id != Some(account) || entry.save_slot_index != Some(slot) {
                continue;
            }
            // `savegame.list_backup_entries` publishes the directory name, which
            // is the UTC creation stamp, as the timestamp. The shared backups live
            // under the state root, so the previous mtime lookup under
            // `protected-internal/backups` always produced an empty string.
            backups.push(json!({
                "timestamp": entry.backup_id.clone(),
                "backup_id": entry.backup_id,
                "action": entry.action,
                "manifest_schema": entry.manifest_schema.unwrap_or_default(),
                "file_count": entry.file_count,
            }));
            if backups.len() == 256 {
                break;
            }
        }
        Ok(json!({"backups": backups}))
    }

    fn recycle_backups(&mut self, save_id: &str, backup_ids: &Value) -> Result<Value, HostError> {
        let requested = backup_ids
            .as_array()
            .filter(|list| !list.is_empty())
            .ok_or_else(|| HostError::rejected("Select backups belonging to this save"))?;
        let allowed: Vec<String> = self.backups(save_id)?["backups"]
            .as_array()
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| entry["backup_id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let mut seen: Vec<String> = Vec::new();
        for id in requested {
            let id = id
                .as_str()
                .ok_or_else(|| HostError::rejected("Select backups belonging to this save"))?;
            if !allowed.iter().any(|entry| entry == id) {
                return Err(HostError::rejected("Select backups belonging to this save"));
            }
            if seen.iter().any(|entry| entry == id) {
                continue;
            }
            seen.push(id.to_string());
            move_backup_to_recycle_bin(&self.state_root, id).map_err(HostError::from_save)?;
        }
        self.backups(save_id)
    }

    fn discard(&mut self, plan_id: &str) -> Value {
        if let Some(plan) = self.plans.remove(plan_id) {
            let _ = self.host().discard(plan.save_plan);
        }
        json!({"discarded": true})
    }

    fn operation(&mut self, plan_id: &str) -> Result<Value, HostError> {
        if let Some(receipt) = self.receipts.get(plan_id) {
            return Ok(receipt.clone());
        }
        if plan_id.len() != 32 || !plan_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(HostError::rejected("Invalid operation ID"));
        }
        let path = self.ledger_dir().join(format!("{plan_id}.json"));
        if !path.is_file() {
            return Err(HostError::rejected("Unknown operation ID"));
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|error| HostError::rejected(format!("{}: {error}", path.display())))?;
        let mut value: Value = serde_json::from_str(&text).map_err(|error| {
            HostError::rejected(format!("operation ledger is not JSON: {error}"))
        })?;
        if value.get("commit_status").and_then(Value::as_str) == Some("executing") {
            self.project_core_authority(plan_id, &mut value);
            // The projected word becomes the durable record so the ledger and the
            // public receipt cannot disagree after a restart. It stays a
            // projection: the core journal remains the authority, so a failed
            // rewrite only costs the next reader one recomputation.
            if let Err(error) = self.write_ledger(plan_id, &value, true) {
                eprintln!(
                    "protected save host: could not persist the projected receipt for \
                     {plan_id}: {}",
                    error.message
                );
            }
        }
        self.receipts.insert(plan_id.to_string(), value.clone());
        Ok(value)
    }

    /// Project the save-core receipt onto a stale outer intent.
    ///
    /// The save-core operation journal is the single authority for a commit's
    /// terminal state, so a restarted host reads it through the transaction
    /// crate rather than deciding `committed`/`rolled_back` from the outer
    /// ledger. An unreadable core leaves the intent untouched, which then reads
    /// as `unknown` instead of fabricating an outcome the core never recorded.
    fn project_core_authority(&self, plan_id: &str, value: &mut Value) {
        let receipt = match self.host().receipt(plan_id) {
            Ok(Some(receipt)) => receipt,
            Ok(None) | Err(_) => {
                value["commit_status"] = json!("unknown");
                value["warning"] = json!(STALE_INTENT_WARNING);
                return;
            }
        };
        // The core's own words are the authority; the outer code only carries
        // them. A core that still reads `pending` stays `unknown` because the
        // operation never reached a terminal state.
        let projected = match receipt.outcome.as_str() {
            "committed" => "committed",
            "not_committed" => "not_committed",
            _ => "unknown",
        };
        value["commit_status"] = json!(projected);
        value["warning"] = match receipt.message.as_deref().map(str::trim) {
            Some(message) if !message.is_empty() => json!(message),
            // A non-terminal core cannot explain itself, so the operator keeps
            // the shipped warning rather than a silent null.
            _ if projected == "unknown" => json!(STALE_INTENT_WARNING),
            _ => Value::Null,
        };
        // Only a terminal outcome may publish the core's installed facts: a
        // `pending` or `uncertain` core never confirmed an installation, so
        // naming an `installed_sha256` for it would report something the core
        // did not record. The reviewed target generation stays distinct from it.
        if projected != "unknown" {
            if let Some(details) = value.get_mut("details").and_then(Value::as_object_mut) {
                details.insert(
                    "installed_sha256".to_string(),
                    json!(receipt.installed_sha256),
                );
                details.insert("backup_id".to_string(), json!(receipt.backup_id));
                details.insert("core_outcome".to_string(), json!(receipt.outcome));
            }
        }
    }

    fn operations(&mut self, save_id: &str) -> Result<Value, HostError> {
        self.save_path(save_id)?;
        let directory = self.ledger_dir();
        let mut receipts = Vec::new();
        if directory.is_dir() {
            let mut paths: Vec<PathBuf> = std::fs::read_dir(&directory)
                .map_err(|error| HostError::rejected(error.to_string()))?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
                .collect();
            // Newest first, like `SaveApplication.operations`. Operation ids are
            // random, so ordering by file name showed an arbitrary window.
            let modified = |path: &PathBuf| {
                std::fs::metadata(path)
                    .and_then(|metadata| metadata.modified())
                    .ok()
            };
            paths.sort_by(|left, right| {
                modified(right)
                    .cmp(&modified(left))
                    .then_with(|| right.cmp(left))
            });
            for path in paths {
                let stem = path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .map(str::to_string);
                let Some(stem) = stem else { continue };
                let Ok(receipt) = self.operation(&stem) else {
                    continue;
                };
                if receipt.get("save_id").and_then(Value::as_str) != Some(save_id) {
                    continue;
                }
                // The display window is 128 receipts, but an unresolved
                // operation is never hidden by it: the client decides whether a
                // write is still uncertain from this list.
                let unresolved = matches!(
                    receipt.get("commit_status").and_then(Value::as_str),
                    Some("unknown" | "executing")
                );
                if receipts.len() < 128 || unresolved {
                    receipts.push(receipt);
                }
            }
        }
        Ok(json!({"operations": receipts}))
    }

    fn commit(&mut self, plan_id: &str) -> Result<Value, HostError> {
        if self.ledger_dir().join(format!("{plan_id}.json")).is_file() {
            // The outer ledger already has a durable entry for this operation.
            // `operation` projects the save-core journal onto it, so a stale
            // `executing` intent whose core commit really landed resolves to the
            // core's own `committed` state instead of a blanket `unknown` that a
            // later retry could try to replay. A terminal outer record is
            // returned exactly as written.
            return self.operation(plan_id);
        }
        let plan = self
            .plans
            .get(plan_id)
            .filter(|plan| plan.expires_at > Instant::now())
            .ok_or_else(|| HostError::rejected("Plan expired; prepare a new plan"))?;
        let save_id = plan.save_id.clone();
        let kind = plan.kind;
        let source_hash = plan.source_hash.clone();
        let save_plan = plan.save_plan.clone();
        let save_path = self.save_path(&save_id)?;
        if sha256_file(&save_path)? != source_hash {
            return Err(HostError::rejected(
                "Save changed after preparation; no write attempted",
            ));
        }
        // Durable intent precedes every write, exactly as the shipped
        // `SaveApplication.commit` does before its installer call. A process
        // death anywhere after this point leaves a receipt naming the operation,
        // the save and the reviewed identity, so a restart can report `unknown`
        // with the shipped warning instead of losing the claimed write.
        let intent = json!({
            "operation_id": plan_id,
            "save_id": save_id,
            "commit_status": "executing",
            "warning": Value::Null,
            "details": {
                "save_path": python_path_string(&save_path),
                "reviewed_source_sha256": source_hash,
            },
        });
        self.write_ledger(plan_id, &intent, false)?;
        let core_started = std::time::Instant::now();
        let core = self.host().commit(&save_plan);
        if host_timing_enabled() {
            eprintln!(
                "host-timing\tcore-commit\t{}",
                core_started.elapsed().as_micros()
            );
        }
        // The business commit and the quality of its bookkeeping are two
        // separate facts. A core that already wrote the target must never turn
        // into a `failed` job with `result = None` just because a record could
        // not be persisted, and an unprovable outcome must read as `unknown`
        // rather than as a retryable not-committed result.
        let (mut commit_status, mut warning, core_outcome) = match core {
            Ok(receipt) => (
                commit_word(&receipt.outcome).to_string(),
                receipt.message.clone(),
                receipt.outcome.clone(),
            ),
            // The core wrote the target and read it back, but its own terminal
            // record did not land. The plan is consumed, so the operation id
            // must not be retried.
            Err(SaveReadError::CommitCompletedWithWarning { warning, .. }) => (
                "committed_with_warning".to_string(),
                Some(warning),
                "committed_with_warning".to_string(),
            ),
            // The core can prove neither that the bytes landed nor that they did
            // not. `unknown` is the honest published word; fabricating either
            // success or a retryable failure would be worse than the raw error.
            Err(SaveReadError::CommitUncertain { message }) => (
                "unknown".to_string(),
                Some(message),
                "uncertain".to_string(),
            ),
            // Every other core error happened before the target could move (or
            // could be proven not to have moved), so the shipped refusal stays a
            // refusal and no receipt is invented for it.
            Err(error) => return Err(HostError::from_save(error)),
        };
        // The two error arms still have a durable core record whenever the core
        // managed to write one; re-read it so the published facts come from the
        // core instead of being published as nulls the core actually recorded.
        let core_record = self.host().receipt(plan_id).ok().flatten();
        let installed_sha256 = core_record
            .as_ref()
            .and_then(|record| record.installed_sha256.clone());
        let backup_id = core_record
            .as_ref()
            .and_then(|record| record.backup_id.clone());
        let mut result = json!({
            "operation_id": plan_id,
            "save_id": save_id,
            "commit_status": commit_status,
            "warning": warning,
            "details": {
                "save_path": python_path_string(&save_path),
                // The target generation the human reviewed. This is the drift
                // guard the outer transaction crate compares at commit time and
                // is deliberately distinct from `installed_sha256`, which is the
                // selected source bundle's identity. The two must never be
                // conflated on the wire.
                "reviewed_source_sha256": source_hash,
                "kind": kind,
                "installed_sha256": installed_sha256,
                "backup_id": backup_id,
                "core_outcome": core_outcome,
            },
        });
        if let Err(error) = self.write_ledger(plan_id, &result, true) {
            // Shipped `save_application.SaveApplication.commit`: a failed outer
            // ledger update never demotes a completed write. `committed` becomes
            // `committed_with_warning` and the failure is appended to the
            // warning; any other status keeps its own word and gains the note.
            if commit_status == "committed" {
                commit_status = "committed_with_warning".to_string();
            }
            let note = format!("Operation ledger update failed: {}", error.message);
            warning = Some(match warning.as_deref().map(str::trim) {
                Some(existing) if !existing.is_empty() => format!("{existing} {note}"),
                _ => note,
            });
            result["commit_status"] = json!(commit_status);
            result["warning"] = json!(warning);
            eprintln!(
                "protected save host: could not persist the terminal receipt for {plan_id}: {}",
                error.message
            );
        }
        self.receipts.insert(plan_id.to_string(), result.clone());
        if let Some(plan) = self.plans.remove(plan_id) {
            self.snapshots.remove(&plan.save_id);
        }
        Ok(result)
    }

    /// Persist one outer ledger record.
    ///
    /// `terminal` marks the record written after the save core returned. Only
    /// that write is subject to the RW02 fault gate; the durable intent that
    /// precedes a claimed write must keep failing hard, because a write without
    /// a recorded claim is exactly what the intent exists to prevent.
    fn write_ledger(&self, plan_id: &str, result: &Value, terminal: bool) -> Result<(), HostError> {
        let directory = self.ledger_dir();
        std::fs::create_dir_all(&directory)
            .map_err(|error| HostError::rejected(error.to_string()))?;
        let path = directory.join(format!("{plan_id}.json"));
        let temporary = path.with_extension("tmp");
        let text = serde_json::to_string(result)
            .map_err(|error| HostError::rejected(error.to_string()))?;
        let fault = if terminal { take_ledger_fault() } else { None };
        if fault == Some(LedgerFault::Temp) {
            return Err(ledger_fault_error(plan_id, "temp"));
        }
        // The receipt must survive a crash, not just a rename: write, flush to
        // the device, then replace, mirroring `_write_receipt`'s fsync.
        match fault {
            Some(LedgerFault::Write) | Some(LedgerFault::Flush) => {
                std::fs::write(&temporary, text.as_bytes())
                    .map_err(|error| HostError::rejected(error.to_string()))?;
                return Err(ledger_fault_error(
                    plan_id,
                    if fault == Some(LedgerFault::Flush) {
                        "flush"
                    } else {
                        "write"
                    },
                ));
            }
            _ => {
                nioh3_save::transaction::write_durable(&temporary, text.as_bytes())
                    .map_err(HostError::from_save)?;
            }
        }
        if fault == Some(LedgerFault::Rename) {
            return Err(ledger_fault_error(plan_id, "rename"));
        }
        std::fs::rename(&temporary, &path).map_err(|error| HostError::rejected(error.to_string()))
    }

    fn data_directory(&self, action: &str, path: Option<&str>) -> Result<Value, HostError> {
        if action == "inspect" {
            return Ok(json!({
                "data_directory": python_path_string(&self.state_root),
                "restart_required": false,
            }));
        }
        let selected = match action {
            "reset" => default_state_root(&self.data_root),
            "set" => match path.filter(|value| !value.is_empty()) {
                Some(value) => PathBuf::from(value),
                None => return Err(HostError::rejected("Invalid data directory action")),
            },
            _ => return Err(HostError::rejected("Invalid data directory action")),
        };
        let saved = write_data_root(&selected, &self.data_root)?;
        Ok(json!({
            "data_directory": python_path_string(&saved),
            "restart_required": true,
        }))
    }
}

impl RoleApplication for SaveApplication {
    fn role(&self) -> Role {
        Role::Save
    }

    fn context_payload(&self) -> Value {
        crate::app::protected_context_payload(&self.context)
    }

    fn direct(&mut self, method: &str, _params: &Value) -> Result<Value, HostError> {
        Err(HostError::rejected(format!(
            "INVALID_REQUEST: {method} is not an inline protected method"
        )))
    }

    fn run(
        &mut self,
        operation: &str,
        params: Value,
        _ctx: &JobContext,
    ) -> Result<Value, HostError> {
        let started = std::time::Instant::now();
        let result = self.dispatch(operation, params);
        if host_timing_enabled() {
            // Diagnostic only, off unless the operator asks for it. The gate
            // reads these lines from stderr to separate real per-operation cost
            // from client-side round-trip time.
            eprintln!(
                "host-timing\t{operation}\t{}",
                started.elapsed().as_micros()
            );
        }
        result
    }

    fn shutdown(&mut self) -> Result<Value, HostError> {
        Ok(json!({"safe_to_shutdown": true}))
    }
}

impl SaveApplication {
    /// One dispatched save operation, timed by [`RoleApplication::run`].
    fn dispatch(&mut self, operation: &str, params: Value) -> Result<Value, HostError> {
        match operation {
            "register" => {
                let path = param_str(&params, "path")?;
                self.register(&path)
            }
            "discover" => Ok(self.discover()),
            "inventory" => self.inventory(&param_str(&params, "save_id")?),
            "template" => {
                let playthrough = param_u64(&params, "playthrough")? as u8;
                self.template(
                    &param_str(&params, "save_id")?,
                    &param_str(&params, "snapshot_id")?,
                    playthrough,
                )
            }
            "prepare_edit" => {
                let edits = params.get("edits").cloned().unwrap_or(Value::Null);
                self.prepare_edit(
                    &param_str(&params, "save_id")?,
                    &param_str(&params, "snapshot_id")?,
                    &edits,
                )
            }
            "prepare_delete" => {
                let slots = params.get("slots").cloned().unwrap_or(Value::Null);
                self.prepare_delete(
                    &param_str(&params, "save_id")?,
                    &param_str(&params, "snapshot_id")?,
                    &slots,
                )
            }
            "prepare_install" => {
                let candidate = params
                    .get("candidate")
                    .cloned()
                    .ok_or_else(HostError::invalid_request)?;
                self.prepare_install(
                    &param_str(&params, "save_id")?,
                    &param_str(&params, "snapshot_id")?,
                    &candidate,
                    param_u64(&params, "recommended_level")? as u16,
                    param_u64(&params, "transfer_count")? as u32,
                )
            }
            "prepare_install_many" => {
                let candidates = params
                    .get("candidates")
                    .cloned()
                    .ok_or_else(HostError::invalid_request)?;
                self.prepare_install_many(
                    &param_str(&params, "save_id")?,
                    &param_str(&params, "snapshot_id")?,
                    &candidates,
                    param_u64(&params, "recommended_level")? as u16,
                    param_u64(&params, "transfer_count")? as u32,
                )
            }
            "prepare_restore" => {
                let backup_id = param_str(&params, "backup_id")?;
                self.prepare_restore(
                    &param_str(&params, "save_id")?,
                    &param_str(&params, "snapshot_id")?,
                    &backup_id,
                )
            }
            "auxiliary_preview" => {
                let seed = param_u64(&params, "seed")? as u32;
                let playthrough = param_u64(&params, "playthrough")? as u8;
                self.auxiliary_preview(seed, playthrough)
            }
            "cached_grace" => {
                let playthrough = param_u64(&params, "playthrough")? as u8;
                let rarity = param_u64(&params, "rarity")? as u8;
                self.cached_grace(
                    &param_str(&params, "save_id")?,
                    &param_str(&params, "snapshot_id")?,
                    playthrough,
                    rarity,
                )
            }
            "materialize_live_many" => {
                let candidates = params
                    .get("candidates")
                    .cloned()
                    .ok_or_else(HostError::invalid_request)?;
                self.materialize_live_many(
                    &param_str(&params, "save_id")?,
                    &param_str(&params, "snapshot_id")?,
                    &candidates,
                    param_u64(&params, "recommended_level")? as u16,
                    param_u64(&params, "transfer_count")? as u32,
                )
            }
            "backups" => self.backups(&param_str(&params, "save_id")?),
            "live_add_source" => self.live_add_source(
                &param_str(&params, "save_id")?,
                &param_str(&params, "snapshot_id")?,
            ),
            "count_edit_source" => self.count_edit_source(
                &param_str(&params, "save_id")?,
                &param_str(&params, "snapshot_id")?,
                param_u64(&params, "slot_index")? as usize,
            ),
            "recycle_backups" => {
                let ids = params.get("backup_ids").cloned().unwrap_or(Value::Null);
                self.recycle_backups(&param_str(&params, "save_id")?, &ids)
            }
            "discard" => {
                let plan_id = param_str(&params, "plan_id")?;
                Ok(self.discard(&plan_id))
            }
            "commit" => {
                let plan_id = param_str(&params, "plan_id")?;
                self.commit(&plan_id)
            }
            "operation" => {
                let plan_id = param_str(&params, "plan_id")?;
                self.operation(&plan_id)
            }
            "operations" => self.operations(&param_str(&params, "save_id")?),
            "data_directory" => {
                let action = param_str(&params, "action")?;
                let path = params
                    .get("path")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                self.data_directory(&action, path.as_deref())
            }
            "backup_location" => {
                // The shipped host's public backup root, so the UI and any
                // pre-migration bundles agree on one directory.
                let directory = self.backup_root();
                std::fs::create_dir_all(&directory)
                    .map_err(|error| HostError::rejected(error.to_string()))?;
                Ok(json!({"backup_directory": python_path_string(&directory)}))
            }
            other => Err(HostError::rejected(format!(
                "OPERATION_REJECTED: the protected save host does not serve {other}"
            ))),
        }
    }
}

fn effects_json(record: &[u8; RECORD_BYTES]) -> Result<Vec<Value>, HostError> {
    Ok(read_local_effect_slots(record)
        .map_err(HostError::from_save)?
        .into_iter()
        .map(effect_json)
        .collect())
}

fn effect_json(effect: LocalEffectSlot) -> Value {
    json!({
        "slot_index": effect.slot_index,
        "effect_id": effect.effect_id,
        "value": effect.value,
        "prefix": effect.prefix,
        "metadata": effect.metadata,
        "tail_0": effect.tail_0,
        "tail_1": effect.tail_1,
    })
}

fn install_record(
    candidate: &Value,
    recommended_level: u16,
    transfer_count: u32,
) -> Result<([u8; RECORD_BYTES], Value), HostError> {
    install_record_from(candidate, candidate, recommended_level, transfer_count)
}

/// One installation record plus the preview the UI receives.
///
/// `identity` is the payload the caller sent — the shipped host echoes its
/// `candidate_id`, seed, rarity, playthrough and level in the preview even when
/// the record itself came from a materialization — while `record_source` is the
/// payload whose `installation_record_hex`/`record_hex` is installed.
fn install_record_from(
    identity: &Value,
    record_source: &Value,
    recommended_level: u16,
    transfer_count: u32,
) -> Result<([u8; RECORD_BYTES], Value), HostError> {
    let stage = record_source
        .get("record_stage")
        .and_then(Value::as_str)
        .ok_or_else(HostError::invalid_request)?;
    if stage == "effect_sequence_only" {
        return Err(HostError::rejected(
            "当前候选只包含离线词条序列，而且该周目/稀有度尚未通过完整记录原生一致性门禁，\
暂不允许写入。",
        ));
    }
    if stage == "native_stage_one" && record_source.get("rarity").and_then(Value::as_u64) == Some(4)
    {
        return Err(HostError::rejected(
            "当前候选仍是原生中间态，包含尚未完成最终解析的结果码，拒绝写入。",
        ));
    }
    let selected = record_source
        .get("installation_record_hex")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .or_else(|| record_source.get("record_hex").and_then(Value::as_str))
        .ok_or_else(HostError::invalid_request)?;
    let bytes = unhex(selected)?;
    let record: [u8; RECORD_BYTES] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| HostError::rejected("Candidate record length is invalid"))?;
    if u16::from_le_bytes([record[0x10], record[0x11]]) != recommended_level {
        return Err(HostError::rejected(
            "Recommended level differs from the native candidate; regenerate before installation",
        ));
    }
    // `models.ScrollCandidate.custom_only`: an early-playthrough rarity-4 scroll
    // is a custom-only identity, and the shipped install preview publishes the
    // flag for the UI.
    let playthrough = identity.get("playthrough").and_then(Value::as_u64);
    let custom_only = matches!(playthrough, Some(1) | Some(2))
        && identity.get("rarity").and_then(Value::as_u64) == Some(4);
    let preview = json!({
        "candidate_id": identity.get("candidate_id").cloned().unwrap_or(Value::Null),
        "seed": identity.get("seed").cloned().unwrap_or(Value::Null),
        "rarity": identity.get("rarity").cloned().unwrap_or(Value::Null),
        "playthrough": playthrough.map_or(Value::Null, |value| json!(value)),
        "level": identity.get("level").cloned().unwrap_or(Value::Null),
        "recommended_level": recommended_level,
        "transfer_count": transfer_count,
        "installation_sha256": sha256_bytes(&record),
        "custom_only": custom_only,
    });
    Ok((record, preview))
}

fn header_patch(edit: &Value) -> Result<HeaderPatch, HostError> {
    let header = edit.get("header").ok_or_else(HostError::invalid_request)?;
    let field = |name: &str| -> Result<u64, HostError> {
        header
            .get(name)
            .and_then(Value::as_u64)
            .ok_or_else(HostError::invalid_request)
    };
    Ok(HeaderPatch {
        playthrough: field("playthrough")? as u8,
        level: field("level")? as u16,
        recommended_level: field("recommended_level")? as u16,
        seed: field("seed")? as u32,
        rarity: field("rarity")? as u8,
        transfer_count: field("transfer_count")? as u32,
    })
}

fn header_json(header: &HeaderPatch) -> Value {
    json!({
        "playthrough": header.playthrough,
        "level": header.level,
        "recommended_level": header.recommended_level,
        "seed": header.seed,
        "rarity": header.rarity,
        "transfer_count": header.transfer_count,
    })
}

fn effect_patches(edit: &Value) -> Result<Vec<EffectPatch>, HostError> {
    let Some(effects) = edit.get("effects").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut patches = Vec::new();
    for effect in effects {
        let field = |name: &str| effect.get(name).and_then(Value::as_u64).map(|v| v as u32);
        patches.push(EffectPatch {
            slot_index: effect
                .get("slot_index")
                .and_then(Value::as_u64)
                .ok_or_else(HostError::invalid_request)? as usize,
            prefix: field("prefix"),
            effect_id: field("effect_id"),
            value: field("value"),
            metadata: field("metadata"),
            tail_0: field("tail_0"),
            tail_1: field("tail_1"),
        });
    }
    Ok(patches)
}

fn local_header(record: &[u8; RECORD_BYTES]) -> Result<Value, HostError> {
    let record_type = u16::from_le_bytes([record[0], record[1]]);
    let playthrough = nioh3_save::transform::playthrough_of(record_type)
        .ok_or_else(|| HostError::rejected("record is not a mapped scroll"))?;
    Ok(json!({
        "playthrough": playthrough,
        "level": u16::from_le_bytes([record[0x06], record[0x07]]),
        "recommended_level": u16::from_le_bytes([record[0x10], record[0x11]]),
        "seed": u32::from_le_bytes([record[0x20], record[0x21], record[0x22], record[0x23]]),
        "rarity": record[0x30],
        "transfer_count": u32::from_le_bytes([
            record[0xDC], record[0xDD], record[0xDE], record[0xDF],
        ]),
    }))
}

fn param_str(params: &Value, name: &str) -> Result<String, HostError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(HostError::invalid_request)
}

/// Whether per-operation host timing is printed to stderr.
///
/// Diagnostic only, and off unless `NIOH3_SAVE_HOST_TIMING=1`. The acceptance
/// gate turns it on to attribute time *inside* the host instead of inferring it
/// from client round trips.
fn host_timing_enabled() -> bool {
    std::env::var("NIOH3_SAVE_HOST_TIMING")
        .map(|value| value.trim() == "1")
        .unwrap_or(false)
}

/// The save-core fault point one process should inject, if any.
///
/// Acceptance and diagnosis drive the save-core fault gate from outside the
/// process; the shipped host exposes no protocol field for it, so the selector
/// travels in the environment exactly like `NIOH3_SAVE_HOST_TIMING`. It is unset
/// in every ordinary run, which leaves the host fault-free.
fn host_fault_point() -> Option<nioh3_save::transaction::FaultPoint> {
    let value = std::env::var("NIOH3_SAVE_HOST_FAULT").ok()?;
    nioh3_save::transaction::FaultPoint::ALL
        .into_iter()
        .find(|point| point.label() == value.trim())
}

/// One armed fault set for an externally selected point.
fn fault_set_for(
    point: nioh3_save::transaction::FaultPoint,
) -> nioh3_save::transaction::TransactionFaults {
    let faults = nioh3_save::transaction::TransactionFaults::default();
    faults.arm(point);
    faults
}

/// The published word for one save-core outcome.
///
/// `committed` and `not_committed` are the core's own terminal words; anything
/// else (a `pending` intent, an `uncertain` rollback) has no proven outcome and
/// is published as `unknown` instead of being rounded to a success.
fn commit_word(outcome: &str) -> &'static str {
    match outcome {
        "committed" => "committed",
        "not_committed" => "not_committed",
        _ => "unknown",
    }
}

/// One stage of the outer ledger write the RW02 gate can fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LedgerFault {
    /// Fail before the staged record exists.
    Temp,
    /// Fail after the staged record was created without a device flush.
    Write,
    /// Fail after the staged bytes were written, before the device flush.
    Flush,
    /// Fail after the staged record is durable, before the replacement.
    Rename,
}

/// Whether the outer-ledger fault gate already fired in this process.
static LEDGER_FAULT_FIRED: AtomicBool = AtomicBool::new(false);

/// The outer-ledger fault one process should inject, if any.
///
/// `NIOH3_SAVE_LEDGER_FAULT` names one stage (`temp`, `write`, `flush`,
/// `rename`) of the *terminal* ledger write. It travels in the environment like
/// `NIOH3_SAVE_HOST_FAULT`, is unset in every ordinary run, and fires at most
/// once so a restart in the same test is fault-free.
fn take_ledger_fault() -> Option<LedgerFault> {
    let value = std::env::var("NIOH3_SAVE_LEDGER_FAULT").ok()?;
    let fault = match value.trim() {
        "temp" => LedgerFault::Temp,
        "write" => LedgerFault::Write,
        "flush" => LedgerFault::Flush,
        "rename" => LedgerFault::Rename,
        _ => return None,
    };
    (!LEDGER_FAULT_FIRED.swap(true, Ordering::SeqCst)).then_some(fault)
}

/// The refusal the RW02 gate returns for one injected ledger stage.
fn ledger_fault_error(plan_id: &str, stage: &str) -> HostError {
    HostError::rejected(format!(
        "injected outer-ledger fault at {stage} for operation {plan_id}"
    ))
}

fn param_u64(params: &Value, name: &str) -> Result<u64, HostError> {
    params
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(HostError::invalid_request)
}

/// One transferred candidate, mirroring `models.ScrollCandidate`'s fields.
struct TransferredCandidate {
    seed: u32,
    playthrough: Option<u8>,
    rarity: u8,
    stage: &'static str,
    record: Vec<u8>,
    installation_record: Option<Vec<u8>>,
    effects: Vec<TransferredEffect>,
}

/// One transferred effect, in the wire order `models.ScrollEffect` uses.
struct TransferredEffect {
    slot: u32,
    effect_id: u32,
    value: i64,
    metadata: u32,
    prefix: u32,
    tail_0: u32,
    tail_1: u32,
    roll_percent: Option<i64>,
}

/// `candidate_transfer.import_candidate`.
fn import_candidate(
    payload: &Value,
    context_digest: &str,
) -> Result<TransferredCandidate, HostError> {
    let identity_error =
        || HostError::rejected("Candidate identity does not match the transferred payload");
    if payload.get("context_digest").and_then(Value::as_str) != Some(context_digest) {
        return Err(HostError::rejected(
            "Candidate generation context has changed",
        ));
    }
    let text = |key: &str| -> Result<String, HostError> {
        payload
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(identity_error)
    };
    let number = |key: &str| -> Result<u64, HostError> {
        payload
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(identity_error)
    };
    let stage = match text("record_stage")?.as_str() {
        STAGE_FINAL_RECORD => STAGE_FINAL_RECORD,
        STAGE_NATIVE_STAGE_ONE => STAGE_NATIVE_STAGE_ONE,
        STAGE_EFFECT_SEQUENCE_ONLY => STAGE_EFFECT_SEQUENCE_ONLY,
        _ => return Err(identity_error()),
    };
    let record = unhex(&text("record_hex")?).map_err(|_| identity_error())?;
    let installation_record = match payload
        .get("installation_record_hex")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(unhex(value).map_err(|_| identity_error())?),
        None => None,
    };
    let mut effects = Vec::new();
    for item in payload
        .get("effects")
        .and_then(Value::as_array)
        .ok_or_else(identity_error)?
    {
        let field = |key: &str| -> Result<u32, HostError> {
            item.get(key)
                .and_then(Value::as_u64)
                .map(|value| value as u32)
                .ok_or_else(identity_error)
        };
        effects.push(TransferredEffect {
            slot: field("slot")?,
            effect_id: field("effect_id")?,
            value: item
                .get("value")
                .and_then(Value::as_i64)
                .ok_or_else(identity_error)?,
            metadata: field("metadata")?,
            prefix: field("prefix")?,
            tail_0: field("tail_0")?,
            tail_1: field("tail_1")?,
            roll_percent: item.get("roll_percent").and_then(Value::as_i64),
        });
    }
    let candidate = TransferredCandidate {
        seed: number("seed")? as u32,
        playthrough: payload
            .get("playthrough")
            .and_then(Value::as_u64)
            .map(|value| value as u8),
        rarity: number("rarity")? as u8,
        stage,
        record,
        installation_record,
        effects,
    };
    if candidate_identity(
        context_digest,
        candidate.seed,
        candidate.playthrough,
        candidate.rarity,
        candidate.stage,
        &candidate.record,
        candidate.installation_record.as_deref(),
        &candidate.effects,
    ) != text("candidate_id")?
    {
        return Err(identity_error());
    }
    Ok(candidate)
}

/// `models.ScrollCandidate.can_materialize_for_install`.
fn can_materialize_for_install(candidate: &TransferredCandidate) -> bool {
    candidate.stage == STAGE_EFFECT_SEQUENCE_ONLY
        && candidate.playthrough == Some(3)
        && matches!(candidate.rarity, 3..=5)
}

/// The `OperationCommand.INSTALL_GENERATED` policy decision.
///
/// `OperationPolicy.evaluate` refuses with `CANDIDATE_NOT_INSTALLABLE`, so a
/// refused candidate is a coded job failure carrying `install_blocker`'s text
/// verbatim.
fn require_installable(candidate: &TransferredCandidate) -> Result<(), HostError> {
    if let Some(blocker) = install_blocker(candidate) {
        return Err(HostError::coded("CANDIDATE_NOT_INSTALLABLE", blocker));
    }
    Ok(())
}

/// Exact port of `models.ScrollCandidate.install_blocker`.
fn install_blocker(candidate: &TransferredCandidate) -> Option<String> {
    if matches!(candidate.playthrough, Some(4) | Some(5)) {
        return Some("四、五周目候选仅供研究预览，禁止通过生成候选安装写入存档。".to_string());
    }
    if let Some(installation) = &candidate.installation_record {
        if installation.len() != RECORD_BYTES {
            return Some("候选携带的待揭露记录长度无效，拒绝写入。".to_string());
        }
        let installed_seed = u32::from_le_bytes([
            installation[0x20],
            installation[0x21],
            installation[0x22],
            installation[0x23],
        ]);
        if installed_seed != candidate.seed {
            return Some("候选预览与待揭露记录的 Seed 不一致，拒绝写入。".to_string());
        }
        if installation[0x30] != candidate.rarity {
            return Some("候选预览与待揭露记录的稀有度不一致，拒绝写入。".to_string());
        }
        if !candidate.record.is_empty() && installation[..2] != candidate.record[..2] {
            return Some("候选预览与待揭露记录的绘卷类型不一致，拒绝写入。".to_string());
        }
    }
    if candidate.stage == STAGE_EFFECT_SEQUENCE_ONLY {
        if can_materialize_for_install(candidate) {
            return None;
        }
        return Some(EFFECT_SEQUENCE_REFUSAL.to_string());
    }
    if candidate.stage == STAGE_NATIVE_STAGE_ONE && candidate.rarity == 4 {
        // Fail closed: the observed resolution list is incomplete by
        // definition, so a rarity-4 native stage-one record always carries an
        // unresolved terminal slot.
        return Some(
            "当前候选仍是原生中间态，包含尚未完成最终解析的结果码，拒绝写入。".to_string(),
        );
    }
    if candidate.stage == STAGE_NATIVE_STAGE_ONE && candidate.rarity < 4 {
        return Some("当前低稀有度原生候选尚未通过最终记录一致性验证，暂不允许写入。".to_string());
    }
    None
}

/// `savegame.read_local_scroll_header`'s guards plus its recommended level.
fn record_recommended_level(record: &[u8]) -> Result<u16, HostError> {
    if record.len() != RECORD_BYTES {
        return Err(HostError::rejected("record must be exactly 0xE8 bytes"));
    }
    let record_type = u16::from_le_bytes([record[0], record[1]]);
    if nioh3_save::transform::playthrough_of(record_type).is_none() {
        return Err(HostError::rejected(format!(
            "record type 0x{record_type:04X} is not a mapped scroll"
        )));
    }
    Ok(u16::from_le_bytes([record[0x10], record[0x11]]))
}

/// The shipped non-materializing branch's recommended-level refusal.
fn recommended_level_guard(record: &[u8], recommended_level: u16) -> Result<(), HostError> {
    if record_recommended_level(record)? != recommended_level {
        return Err(HostError::rejected(RECOMMENDED_LEVEL_REFUSAL));
    }
    Ok(())
}

/// `effect_sequence`'s refusal surface, reported like the shipped `ValueError`.
fn install_error(error: InstallMaterializeError) -> HostError {
    HostError::rejected(error.to_string())
}

/// The measured map the certified materializers bind to for one rarity.
///
/// Rarity 4 uses the stage-one map and rarity 5 the final Grace map; rarity 3
/// has no special slot of its own and never reads it.
fn grace_map_for_rarity(
    resources: &MaterializationResources,
    rarity: u8,
) -> Result<&GraceMap, HostError> {
    let maps = &resources.effect.grace_maps;
    let selected = if rarity == 5 {
        maps.get(1)
    } else {
        maps.first()
    };
    selected.ok_or_else(|| HostError::rejected("the shipped Grace maps are unavailable"))
}

/// One candidate's export, exactly as `materialize_live_many` builds it.
fn materialize_one(
    resources: &MaterializationResources,
    inventory: &SaveInventory,
    context_digest: &str,
    source: &TransferredCandidate,
    level: u64,
    recommended_level: u16,
    transfer_count: u32,
) -> Result<Value, HostError> {
    if !can_materialize_for_install(source) {
        recommended_level_guard(&source.record, recommended_level)?;
        let record = prepare_candidate_for_install(&source.record, transfer_count)
            .map_err(HostError::from_save)?;
        let installation = match source.installation_record.as_deref() {
            Some(bytes) => Some(
                prepare_candidate_for_install(bytes, transfer_count)
                    .map_err(HostError::from_save)?,
            ),
            None => None,
        };
        return Ok(export_candidate(
            context_digest,
            level,
            source.seed,
            source.playthrough,
            source.rarity,
            source.stage,
            &record,
            installation.as_ref().map(|bytes| bytes.as_slice()),
            &source.effects,
        ));
    }

    let template = inventory
        .template_record_for_playthrough(3)
        .map_err(HostError::from_save)?;
    let grace_map = grace_map_for_rarity(resources, source.rarity)?;
    let level = level as u16;
    // `materialize_effect_sequence_candidate`: the stage-one record the save
    // must receive, validated against the solver preview.
    let (install_record, installed_preview) = materialize_ng3_certified_install_record(
        &resources.index,
        grace_map,
        &template,
        source.rarity,
        source.seed,
        level,
        recommended_level,
        inventory.next_generation_serial(),
        transfer_count,
    )
    .map_err(install_error)?;
    verify_materialized_preview(source, &installed_preview)?;
    if nioh3_save::inventory::account_id_from_record(&install_record) != inventory.account_id {
        return Err(HostError::rejected(
            "安装时物化记录没有绑定当前存档来源账号，已拒绝写入。",
        ));
    }
    // `materialize_ng3_certified_record`: the completed record the reveal path
    // produces. The shipped producer allocates this serial separately, so the
    // pair is never collapsed into one record.
    let (finalized, _completed) = materialize_ng3_certified_record(
        &resources.index,
        grace_map,
        &template,
        source.rarity,
        source.seed,
        level,
        recommended_level,
        inventory.next_generation_serial(),
        transfer_count,
    )
    .map_err(install_error)?;
    let effects = record_effects(finalized.as_bytes())?;
    Ok(export_candidate(
        context_digest,
        u64::from(level),
        finalized.displayed_seed(),
        Some(3),
        finalized.rarity(),
        STAGE_FINAL_RECORD,
        finalized.as_bytes(),
        Some(install_record.as_bytes()),
        &effects,
    ))
}

/// `materialize_effect_sequence_candidate`'s preview/solver agreement checks.
fn verify_materialized_preview(
    source: &TransferredCandidate,
    sequence: &nioh3_domain::record::ScrollRecord,
) -> Result<(), HostError> {
    let materialized: Vec<(u32, i64, u32, u32, u32, u32)> = sequence
        .effects
        .iter()
        .map(|effect| {
            let metadata = u32::from(effect.roll_percent)
                | (u32::from(effect.category_and_flags) << 8)
                | (u32::from(effect.effect_flags) << 16);
            (
                effect.effect_id,
                i64::from(effect.resolved_value),
                metadata,
                u32::from(effect.prefix_word),
                0,
                0,
            )
        })
        .collect();
    let expected: Vec<(u32, i64, u32, u32, u32, u32)> = source
        .effects
        .iter()
        .map(|effect| {
            (
                effect.effect_id,
                effect.value,
                effect.metadata,
                effect.prefix,
                effect.tail_0,
                effect.tail_1,
            )
        })
        .collect();
    let count = expected.len();
    if materialized.iter().take(count).copied().collect::<Vec<_>>() != expected {
        return Err(HostError::rejected(
            "安装记录的揭露后结果与求解器预览不一致，已拒绝写入",
        ));
    }
    if materialized
        .iter()
        .skip(count)
        .any(|slot| slot.0 != nioh3_domain::record::EMPTY_EFFECT_ID)
    {
        return Err(HostError::rejected(
            "安装时物化记录出现预览之外的额外词条，已拒绝写入",
        ));
    }
    Ok(())
}

/// `models.ScrollCandidate.from_record`'s seven decoded slots.
fn record_effects(record: &[u8]) -> Result<Vec<TransferredEffect>, HostError> {
    if record.len() != RECORD_BYTES {
        return Err(HostError::rejected("record must be exactly 0xE8 bytes"));
    }
    let mut effects = Vec::with_capacity(EFFECT_SLOT_COUNT);
    for index in 0..EFFECT_SLOT_COUNT {
        let start = EFFECT_SLOT_BASE + index * EFFECT_SLOT_STRIDE;
        let word = |offset: usize| {
            u32::from_le_bytes([
                record[start + offset],
                record[start + offset + 1],
                record[start + offset + 2],
                record[start + offset + 3],
            ])
        };
        effects.push(TransferredEffect {
            slot: (index + 1) as u32,
            prefix: word(0),
            effect_id: word(4),
            value: i64::from(word(8)),
            metadata: word(0x0C),
            tail_0: word(0x10),
            tail_1: word(0x14),
            roll_percent: None,
        });
    }
    Ok(effects)
}

/// `candidate_transfer.export_candidate`.
#[allow(clippy::too_many_arguments)]
fn export_candidate(
    context_digest: &str,
    level: u64,
    seed: u32,
    playthrough: Option<u8>,
    rarity: u8,
    stage: &'static str,
    record: &[u8],
    installation_record: Option<&[u8]>,
    effects: &[TransferredEffect],
) -> Value {
    let candidate_id = candidate_identity(
        context_digest,
        seed,
        playthrough,
        rarity,
        stage,
        record,
        installation_record,
        effects,
    );
    json!({
        "candidate_id": candidate_id,
        "context_digest": context_digest,
        "level": level,
        "seed": seed,
        "playthrough": playthrough,
        "rarity": rarity,
        "record_stage": stage,
        "record_hex": hex(record),
        "installation_record_hex": installation_record.map(hex),
        "effects": effects
            .iter()
            .map(|effect| json!({
                "slot": effect.slot,
                "effect_id": effect.effect_id,
                "value": effect.value,
                "metadata": effect.metadata,
                "prefix": effect.prefix,
                "tail_0": effect.tail_0,
                "tail_1": effect.tail_1,
                "roll_percent": effect.roll_percent,
            }))
            .collect::<Vec<_>>(),
    })
}

/// `core_services.candidate_identity`, byte for byte.
///
/// The digest streams the context digest, the seed, playthrough, rarity, the
/// record stage, both record buffers and every `<7I>` effect tuple in order.
#[allow(clippy::too_many_arguments)]
fn candidate_identity(
    context_digest: &str,
    seed: u32,
    playthrough: Option<u8>,
    rarity: u8,
    stage: &str,
    record: &[u8],
    installation_record: Option<&[u8]>,
    effects: &[TransferredEffect],
) -> String {
    let mut digest = Sha256::new();
    digest.update(context_digest.as_bytes());
    digest.update(seed.to_le_bytes());
    digest.update(i32::from(playthrough.unwrap_or(0)).to_le_bytes());
    digest.update(i32::from(rarity).to_le_bytes());
    digest.update(stage.as_bytes());
    digest.update(record);
    digest.update(installation_record.unwrap_or(&[]));
    for effect in effects {
        for word in [
            effect.slot,
            effect.effect_id,
            effect.value as u32,
            effect.metadata,
            effect.prefix,
            effect.tail_0,
            effect.tail_1,
        ] {
            digest.update(word.to_le_bytes());
        }
    }
    let bytes = digest.finalize();
    hex(bytes.as_slice())
}

/// `cache_application.grace_map_cache_path`.
pub(crate) fn grace_map_cache_path(
    state_root: &Path,
    save_fingerprint: &str,
    playthrough: u8,
    rarity: u8,
    generation_context_digest: &str,
) -> PathBuf {
    let digest = generation_context_digest.to_lowercase();
    let short: String = digest.chars().take(16).collect();
    state_root.join(GRACE_MAP_CACHE_DIR).join(format!(
        "{}-{short}-p{playthrough}-r{rarity}-draw1.json",
        save_fingerprint.to_lowercase()
    ))
}

/// `Path(path).read_text(encoding="utf-8")` plus `json.loads`, with the shipped
/// `FileNotFoundError` rendering for an absent cache.
fn read_json_file(path: &Path) -> Result<Value, HostError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(HostError::rejected(format!(
                "[Errno 2] No such file or directory: '{}'",
                python_path_string(path)
            )))
        }
        Err(error) => {
            return Err(HostError::rejected(format!(
                "{}: {error}",
                python_path_string(path)
            )))
        }
    };
    serde_json::from_str(&text).map_err(|error| HostError::rejected(error.to_string()))
}

/// `grace_map.grace_map_to_cache_payload`.
pub(crate) fn grace_map_to_cache_payload(
    mapping: &worker_grace_map::GraceOutputMap,
    context_fingerprint: &str,
    generation_context_digest: &str,
) -> Result<Value, HostError> {
    let fingerprint = context_fingerprint.trim().to_lowercase();
    if !is_sha256_hex(&fingerprint) {
        return Err(HostError::rejected(
            "context fingerprint must be a 64-character SHA-256 hex string",
        ));
    }
    let digest = generation_context_digest.trim().to_lowercase();
    if !is_sha256_hex(&digest) {
        return Err(HostError::rejected(
            "generation context digest must be a 64-character SHA-256 hex string",
        ));
    }
    let ranges: Vec<Value> = mapping
        .ranges
        .iter()
        .map(|range| {
            json!({
                "start": range.start,
                "end": range.end,
                "grace_id": format!("0x{:08X}", range.grace_id),
            })
        })
        .collect();
    Ok(json!({
        "schema": worker_grace_map::GRACE_MAP_CACHE_SCHEMA,
        "context_fingerprint": fingerprint,
        "generation_context_digest": digest,
        "game_version": worker_grace_map::EXPECTED_GAME_VERSION,
        "record_type": format!("0x{:04X}", mapping.record_type),
        "rarity": mapping.rarity,
        "playthrough": mapping.playthrough,
        "effect_slot": mapping.effect_slot,
        "draw_index": 1,
        "ranges": ranges,
    }))
}

fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Python `json.dumps(value)` with its default `, ` and `: ` separators.
///
/// Every published nested JSON string on this surface is produced by
/// `json.dumps`, so the separators belong to the value the broker compares.
fn python_json(value: &Value) -> String {
    let mut buffer = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, PythonJsonFormatter);
    if serde::Serialize::serialize(value, &mut serializer).is_err() {
        return String::new();
    }
    String::from_utf8(buffer).unwrap_or_default()
}

/// The JSON encoder whose only deviation from the compact form is Python's
/// default item and key separators.
struct PythonJsonFormatter;

impl serde_json::ser::Formatter for PythonJsonFormatter {
    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        writer.write_all(b": ")
    }
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, HostError> {
    std::fs::read(path).map_err(|error| HostError::rejected(format!("{}: {error}", path.display())))
}

/// `app_settings.SETTINGS_SCHEMA`.
const SETTINGS_SCHEMA: &str = "nioh3-scroll-generator-settings/v1";
/// `app_settings.SETTINGS_FILENAME`.
const SETTINGS_FILENAME: &str = "settings.json";
/// `app_settings.default_state_root`'s directory name under LocalAppData.
const STATE_ROOT_DIR: &str = "Nioh3ScrollGenerator";

/// `app_settings.default_state_root(fallback_root=...)`.
fn default_state_root(fallback_root: &Path) -> PathBuf {
    let local = std::env::var("LOCALAPPDATA")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let base = match local {
        Some(value) => PathBuf::from(value),
        None => fallback_root.to_path_buf(),
    };
    base.join(STATE_ROOT_DIR)
}

/// `savegame.discover_save_paths()`'s root: the per-user game save directory.
///
/// `None` when `LOCALAPPDATA` is unset, which is the shipped "no discoverable
/// save" case rather than a state-root substitute.
fn shipped_savedata_root() -> Option<PathBuf> {
    let local = std::env::var("LOCALAPPDATA")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    Some(
        PathBuf::from(local)
            .join("KoeiTecmo")
            .join("NIOH3")
            .join("Savedata"),
    )
}

/// `app_settings.save_data_root`: validate, create, and persist the pointer.
fn write_data_root(selected: &Path, fallback_root: &Path) -> Result<PathBuf, HostError> {
    if !selected.is_absolute() {
        return Err(HostError::rejected("data_root must be an absolute path"));
    }
    let resolved = resolve_soft(selected);
    if resolved.exists() && !resolved.is_dir() {
        return Err(HostError::rejected("data_root points to a file"));
    }
    std::fs::create_dir_all(&resolved)
        .map_err(|error| HostError::rejected(format!("{}: {error}", resolved.display())))?;
    let pointer = default_state_root(fallback_root).join(SETTINGS_FILENAME);
    if let Some(parent) = pointer.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| HostError::rejected(format!("{}: {error}", parent.display())))?;
    }
    // Mirrors `json.dumps(payload, ensure_ascii=False, indent=2) + "\n"`.
    let payload = json!({
        "schema": SETTINGS_SCHEMA,
        "data_root": python_path_string(&resolved),
        "update_channel": "stable",
    });
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&payload)
            .map_err(|error| HostError::rejected(error.to_string()))?
    );
    let temporary = pointer.with_extension("json.tmp");
    std::fs::write(&temporary, text)
        .map_err(|error| HostError::rejected(format!("{}: {error}", temporary.display())))?;
    std::fs::rename(&temporary, &pointer)
        .map_err(|error| HostError::rejected(format!("{}: {error}", pointer.display())))?;
    Ok(resolved)
}

/// `Path.resolve()` without requiring existence, with the verbatim prefix
/// removed so the rendered path matches Python on Windows.
fn resolve_soft(path: &Path) -> PathBuf {
    match path.canonicalize() {
        Ok(canonical) => {
            let text = canonical.to_string_lossy();
            match text.strip_prefix(r"\\?\") {
                Some(stripped) => PathBuf::from(stripped),
                None => canonical,
            }
        }
        Err(_) => path.to_path_buf(),
    }
}

/// `Path.resolve(strict=True)`, then the `\\?\` verbatim prefix removed so the
/// rendered path matches Python's `str(Path)` on Windows.
fn resolve_strict(path: &Path) -> Result<PathBuf, HostError> {
    let canonical = path
        .canonicalize()
        .map_err(|error| HostError::rejected(format!("{}: {error}", path.display())))?;
    let text = canonical.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(stripped) => Ok(PathBuf::from(stripped)),
        None => Ok(canonical),
    }
}

fn save_id_for(path: &Path) -> String {
    sha256_bytes(python_path_string(path).to_lowercase().as_bytes())
}

fn python_path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn sha256_file(path: &Path) -> Result<String, HostError> {
    // `savegame.sha256_file` returns uppercase hex, and every
    // `source_sha256`/`reviewed_source_sha256` on the wire carries that casing.
    Ok(sha256_bytes(&read_bytes(path)?).to_uppercase())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn new_snapshot_id() -> String {
    let mut digest = Sha256::new();
    digest.update(std::process::id().to_le_bytes());
    digest.update(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
            .to_le_bytes(),
    );
    let bytes = digest.finalize();
    let mut rendered = String::with_capacity(32);
    for byte in bytes.iter().take(16) {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

fn hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

fn unhex(text: &str) -> Result<Vec<u8>, HostError> {
    if !text.len().is_multiple_of(2) {
        return Err(HostError::invalid_request());
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(text.len() / 2);
    let mut index = 0;
    while index < bytes.len() {
        let high = (bytes[index] as char)
            .to_digit(16)
            .ok_or_else(HostError::invalid_request)?;
        let low = (bytes[index + 1] as char)
            .to_digit(16)
            .ok_or_else(HostError::invalid_request)?;
        out.push((high * 16 + low) as u8);
        index += 2;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect_sequence_candidate(playthrough: Option<u8>) -> TransferredCandidate {
        TransferredCandidate {
            seed: 0x1234_5678,
            playthrough,
            rarity: 4,
            stage: STAGE_EFFECT_SEQUENCE_ONLY,
            record: Vec::new(),
            installation_record: None,
            effects: vec![TransferredEffect {
                slot: 1,
                effect_id: 0x100,
                value: 5,
                metadata: 0,
                prefix: 0,
                tail_0: 0,
                tail_1: 0,
                roll_percent: Some(5),
            }],
        }
    }

    /// The unmaterializable effect-sequence refusal is the shipped Chinese text,
    /// and it is a `CANDIDATE_NOT_INSTALLABLE` job failure.
    #[test]
    fn the_effect_sequence_refusal_is_the_shipped_text_verbatim() {
        assert_eq!(
            EFFECT_SEQUENCE_REFUSAL,
            "当前候选只包含离线词条序列，而且该周目/稀有度尚未通过完整记录原生一致性门禁，\
暂不允许写入。"
        );
        let refused = effect_sequence_candidate(Some(2));
        assert_eq!(
            install_blocker(&refused).as_deref(),
            Some(EFFECT_SEQUENCE_REFUSAL)
        );
        let failure = require_installable(&refused)
            .err()
            .unwrap_or_else(|| HostError::rejected("playthrough 2 must be refused"));
        assert_eq!(failure.job_code(), "CANDIDATE_NOT_INSTALLABLE");
        assert_eq!(failure.message, EFFECT_SEQUENCE_REFUSAL);
        // The certified three-playthrough context passes the same gate, so the
        // refusal above comes from the materialization eligibility and not from
        // a blanket rejection.
        assert!(install_blocker(&effect_sequence_candidate(Some(3))).is_none());
    }

    /// The non-materializing branch refuses any other recommended level.
    #[test]
    fn the_recommended_level_branch_refuses() {
        let mut record = vec![0u8; RECORD_BYTES];
        record[0..2].copy_from_slice(&0xE604u16.to_le_bytes());
        record[0x10..0x12].copy_from_slice(&183u16.to_le_bytes());
        assert!(recommended_level_guard(&record, 183).is_ok());
        let failure = recommended_level_guard(&record, 180)
            .err()
            .unwrap_or_else(|| HostError::rejected("a different level must be refused"));
        assert_eq!(
            failure.message,
            "Regenerate candidate with the selected recommended level"
        );
        assert_eq!(failure.job_code(), "OPERATION_FAILED");
        // `read_local_scroll_header`'s own guards run first.
        let failure = recommended_level_guard(&[], 183)
            .err()
            .unwrap_or_else(|| HostError::rejected("a short record must be refused"));
        assert_eq!(failure.message, "record must be exactly 0xE8 bytes");
        let mut foreign = record.clone();
        foreign[0..2].copy_from_slice(&0x1234u16.to_le_bytes());
        let failure = recommended_level_guard(&foreign, 183)
            .err()
            .unwrap_or_else(|| HostError::rejected("an unmapped record must be refused"));
        assert_eq!(failure.message, "record type 0x1234 is not a mapped scroll");
    }
}
