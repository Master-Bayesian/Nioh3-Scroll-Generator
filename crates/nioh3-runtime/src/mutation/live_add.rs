//! Reviewed live additions from broker-owned, ready search candidates.
//!
//! Port of `live_add_application.LiveAddApplication`: prepare, execute, status,
//! recover and cancel over `LiveAddOperations` receipts, with backup and claim
//! ordered before the dispatch, a durable per-item receipt, and no replay of an
//! uncertain insertion.
//!
//! The candidate is a typed transfer (`InstallationCandidate`). It carries the
//! finalized record and, when the game must complete a native stage-one record
//! itself, that install record as a separate field. The install record is never
//! reconstructed from the preview payload or from renderer bytes.

use crate::error::RuntimeError;
use crate::mutation::count::{exclusive_json, new_operation_id, read_bytes, read_json, sha256_hex};
use crate::mutation::descriptor::{assembly_descriptor, new_assembly_record};
use crate::mutation::evidence::{verify, verify_persistence};
use crate::mutation::inventory::{
    index_entries, inventory_entries, inventory_json, Inventory, NativeIndex, RECORD_SIZE,
};
use crate::mutation::operations::{LiveAddOperations, OperationSnapshot, OperationState};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// `savegame.SCROLL_GROUP_OFFSET`.
pub const SCROLL_GROUP_OFFSET: usize = 0x176CCE;
/// `savegame.SCROLL_SLOT_COUNT`.
pub const SCROLL_SLOT_COUNT: usize = 400;

/// Display version the live-add layout is accepted for.
pub const LIVE_ADD_DISPLAY_VERSION: &str = "PC v2.01";

fn rejected(detail: &str) -> RuntimeError {
    RuntimeError::LiveAddRejected {
        detail: detail.to_string(),
    }
}

fn candidate_rejected(detail: &str) -> RuntimeError {
    RuntimeError::CandidateRejected {
        detail: detail.to_string(),
    }
}

/// `models.CandidateRecordStage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateStage {
    FinalRecord,
    NativeStageOne,
    EffectSequenceOnly,
}

impl CandidateStage {
    pub const fn value(self) -> &'static str {
        match self {
            Self::FinalRecord => "final_record",
            Self::NativeStageOne => "native_stage_one",
            Self::EffectSequenceOnly => "effect_sequence_only",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "final_record" => Some(Self::FinalRecord),
            "native_stage_one" => Some(Self::NativeStageOne),
            "effect_sequence_only" => Some(Self::EffectSequenceOnly),
            _ => None,
        }
    }
}

/// `models.ScrollEffect` in transfer form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateEffect {
    pub slot: u32,
    pub effect_id: u32,
    pub value: u32,
    pub metadata: u32,
    pub prefix: u32,
    pub tail_0: u32,
    pub tail_1: u32,
}

/// The typed candidate transfer the broker hands to live addition.
///
/// `record` is the record the search produced (`final_record` or the rarity-4
/// `native_stage_one` preview); `installation_record` is present only when the
/// game itself must complete the record on reveal. Both are carried separately
/// and the identity digest covers exactly this pair, so a consumer can never
/// infer one from the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationCandidate {
    pub candidate_id: String,
    pub context_digest: String,
    pub level: u32,
    pub seed: u32,
    pub playthrough: Option<u32>,
    pub rarity: u8,
    pub stage: CandidateStage,
    pub record: Vec<u8>,
    pub installation_record: Option<Vec<u8>>,
    pub effects: Vec<CandidateEffect>,
}

impl InstallationCandidate {
    /// `candidate_transfer.import_candidate`.
    pub fn from_payload(payload: &Value, context_digest: &str) -> Result<Self, RuntimeError> {
        if payload.get("context_digest").and_then(Value::as_str) != Some(context_digest) {
            return Err(candidate_rejected(
                "Candidate generation context has changed",
            ));
        }
        let error =
            || candidate_rejected("Candidate identity does not match the transferred payload");
        let text = |key: &str| -> Result<String, RuntimeError> {
            payload
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(error)
        };
        let number = |key: &str| -> Result<u64, RuntimeError> {
            payload.get(key).and_then(Value::as_u64).ok_or_else(error)
        };
        let stage_text = text("record_stage")?;
        let stage = CandidateStage::parse(&stage_text).ok_or_else(error)?;
        let record =
            crate::mutation::inventory::hex_decode(&text("record_hex")?).map_err(|_| error())?;
        let installation_record = match payload.get("installation_record_hex") {
            Some(Value::String(value)) if !value.is_empty() => {
                Some(crate::mutation::inventory::hex_decode(value).map_err(|_| error())?)
            }
            _ => None,
        };
        let mut effects = Vec::new();
        for item in payload
            .get("effects")
            .and_then(Value::as_array)
            .ok_or_else(error)?
        {
            let field = |key: &str| -> Result<u32, RuntimeError> {
                item.get(key)
                    .and_then(Value::as_u64)
                    .map(|value| value as u32)
                    .ok_or_else(error)
            };
            effects.push(CandidateEffect {
                slot: field("slot")?,
                effect_id: field("effect_id")?,
                value: field("value")?,
                metadata: field("metadata")?,
                prefix: field("prefix")?,
                tail_0: field("tail_0")?,
                tail_1: field("tail_1")?,
            });
        }
        let candidate = Self {
            candidate_id: text("candidate_id")?,
            context_digest: context_digest.to_string(),
            level: number("level")? as u32,
            seed: number("seed")? as u32,
            playthrough: payload
                .get("playthrough")
                .and_then(Value::as_u64)
                .map(|value| value as u32),
            rarity: number("rarity")? as u8,
            stage,
            record,
            installation_record,
            effects,
        };
        if candidate.identity()? != candidate.candidate_id {
            return Err(error());
        }
        Ok(candidate)
    }

    /// `core_services.candidate_identity`, byte for byte.
    pub fn identity(&self) -> Result<String, RuntimeError> {
        let mut digest = Sha256::new();
        digest.update(self.context_digest.as_bytes());
        digest.update(self.seed.to_le_bytes());
        digest.update((self.playthrough.unwrap_or(0) as i32).to_le_bytes());
        digest.update((self.rarity as i32).to_le_bytes());
        digest.update(self.stage.value().as_bytes());
        digest.update(&self.record);
        digest.update(self.installation_record.as_deref().unwrap_or(&[]));
        for effect in &self.effects {
            for value in [
                effect.slot,
                effect.effect_id,
                effect.value,
                effect.metadata,
                effect.prefix,
                effect.tail_0,
                effect.tail_1,
            ] {
                digest.update(value.to_le_bytes());
            }
        }
        let mut rendered = String::with_capacity(64);
        for byte in digest.finalize() {
            rendered.push_str(&format!("{byte:02x}"));
        }
        Ok(rendered)
    }

    /// The record a live insertion must write.
    pub fn install_record(&self) -> &[u8] {
        self.installation_record.as_deref().unwrap_or(&self.record)
    }

    /// `search_application.require_search_candidate_ready`.
    pub fn require_search_candidate_ready(&self) -> Result<(), RuntimeError> {
        if self.rarity == 4 && self.stage == CandidateStage::NativeStageOne {
            // Shipped user-facing contract text, kept identical on purpose.
            return Err(candidate_rejected(
                "稀有度4搜索结果仍是原生待揭露中间态，已拒绝加入候选列表",
            ));
        }
        Ok(())
    }

    /// `models.ScrollCandidate.install_blocker` for the record-shaped branches.
    ///
    /// The catalog-dependent branches (`EFFECT_SEQUENCE_ONLY` materialization
    /// and unresolved effect slots) belong to [`CatalogPolicy`].
    pub fn record_blocker(&self) -> Result<Option<&'static str>, RuntimeError> {
        if matches!(self.playthrough, Some(4) | Some(5)) {
            return Ok(Some(
                "四、五周目候选仅供研究预览，禁止通过生成候选安装写入存档。",
            ));
        }
        if let Some(installation) = &self.installation_record {
            if installation.len() != RECORD_SIZE {
                return Ok(Some("候选携带的待揭露记录长度无效，拒绝写入。"));
            }
            let u32_at = |offset: usize| {
                u32::from_le_bytes([
                    installation[offset],
                    installation[offset + 1],
                    installation[offset + 2],
                    installation[offset + 3],
                ])
            };
            if u32_at(0x20) != self.seed {
                return Ok(Some("候选预览与待揭露记录的 Seed 不一致，拒绝写入。"));
            }
            if installation[0x30] != self.rarity {
                return Ok(Some("候选预览与待揭露记录的稀有度不一致，拒绝写入。"));
            }
            if !self.record.is_empty() && installation[..2] != self.record[..2] {
                return Ok(Some("候选预览与待揭露记录的绘卷类型不一致，拒绝写入。"));
            }
        }
        Ok(None)
    }
}

/// `models.ScrollCandidate.install_blocker` branches that need the effect
/// catalog. The runtime crate carries no catalog, so the domain crate supplies
/// this and the product wiring must pass it explicitly.
pub trait CatalogPolicy {
    /// `EFFECT_SEQUENCE_ONLY` materialization and `unresolved_effect_slots`.
    fn catalog_blocker(
        &mut self,
        candidate: &InstallationCandidate,
    ) -> Result<Option<String>, RuntimeError>;
}

/// The native executor seam.
///
/// The concrete product implementation is the peer-owned native transport; the
/// runtime crate owns the plan, the receipts and the verification.
pub trait LiveAddExecutor {
    /// `LiveAddAdapter.inspect`: identity, addresses, inventory and index.
    fn inspect(&mut self) -> Result<(Value, Inventory, NativeIndex), RuntimeError>;

    /// `LiveAddAdapter.preview` over the isolated assembly record.
    fn preview(&mut self, plan: &Value, assembly_record: &[u8]) -> Result<Value, RuntimeError>;

    /// `LiveAddAdapter.insert`, including its own wait for the receipt.
    fn insert(&mut self, plan: &Value) -> Result<Value, RuntimeError>;

    /// `LiveAddAdapter.readback`.
    fn readback(&mut self) -> Result<(Inventory, NativeIndex), RuntimeError>;

    /// `LiveAddAdapter.recover`: read an existing receipt, never dispatch.
    fn recover(
        &mut self,
        operation_id: &str,
        pid: u32,
        process_creation_time: Option<&str>,
    ) -> Result<Value, RuntimeError>;

    /// `LiveAddAdapter.require_process_instance`, a hard refusal when the
    /// current process lifetime is not the planned one.
    fn require_process_instance(
        &mut self,
        pid: u32,
        process_creation_time: Option<&str>,
    ) -> Result<(), RuntimeError>;

    /// `submission_absent`: only a transport that can prove it never accepted
    /// this operation id may release ownership of a failed dispatch.
    fn submission_absent(&mut self, _operation_id: &str) -> bool {
        false
    }

    /// `safe_to_shutdown`.
    fn safe_to_shutdown(&mut self) -> bool;
}

/// The checkpoint the save side produces for every live-addition operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveCheckpoint {
    pub directory: PathBuf,
    pub backup_path: PathBuf,
    pub decrypted: Vec<u8>,
}

/// Backup creation belongs to the save side (`create_backup_directory`,
/// `write_backup_manifest`, `SaveCrypto`), which is a separate workstream.
///
/// The implementation must validate the `<account>/SAVEDATAxx/SAVEDATA.BIN`
/// path shape, copy the source save, verify the copy, decrypt it and publish the
/// same account/slot/hash manifest the offline operations publish.
pub trait SaveBackup {
    fn checkpoint(
        &mut self,
        source: &Path,
        raw: &[u8],
        operation_id: &str,
    ) -> Result<SaveCheckpoint, RuntimeError>;
}

/// What `prepare` publishes once the reviewed plan exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedLiveAdd {
    pub snapshot: OperationSnapshot,
    pub seed: u32,
    pub rarity: u8,
    pub count_before: usize,
    pub backup_path: String,
    pub instance_serial: String,
}

impl PreparedLiveAdd {
    pub fn to_json(&self) -> Value {
        let mut value = self.snapshot.to_json();
        if let Some(object) = value.as_object_mut() {
            object.insert("seed".to_string(), json!(self.seed));
            object.insert("rarity".to_string(), json!(self.rarity));
            object.insert("count_before".to_string(), json!(self.count_before));
            object.insert("backup_path".to_string(), json!(self.backup_path));
            object.insert("instance_serial".to_string(), json!(self.instance_serial));
            object.insert(
                "persistence".to_string(),
                json!("requires_normal_game_save"),
            );
        }
        value
    }
}

/// The live-add facts `RuntimeApplication.status` publishes.
///
/// Port of `self.live_add is not None and not self.live_add.safe_to_shutdown()`:
/// an unresolved insertion or a pending native call keeps the runtime unsafe to
/// shut down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveAddOwnership {
    pub safe_to_shutdown: bool,
    pub unresolved: Vec<String>,
}

impl LiveAddOwnership {
    /// The boolean `RuntimeOwnership::set_live_add_unsafe` takes.
    pub fn unsafe_ownership(&self) -> bool {
        !self.safe_to_shutdown || !self.unresolved.is_empty()
    }
}

/// One prepared, reviewed live-addition application.
pub struct LiveAddApplication {
    state_root: PathBuf,
    operations: LiveAddOperations,
    context_digest: String,
    executor: Box<dyn LiveAddExecutor + Send>,
    backup: Box<dyn SaveBackup + Send>,
    policy: Box<dyn CatalogPolicy + Send>,
}

impl LiveAddApplication {
    /// The adapter boxes are `Send` because the protected host owns one
    /// application behind a single-threaded lock and hands it to the job
    /// thread; a non-`Send` transport could not cross that boundary.
    pub fn new(
        state_root: &Path,
        context_digest: &str,
        executor: Box<dyn LiveAddExecutor + Send>,
        backup: Box<dyn SaveBackup + Send>,
        policy: Box<dyn CatalogPolicy + Send>,
    ) -> Result<Self, RuntimeError> {
        let operations = LiveAddOperations::new(&state_root.join("live-add"))?;
        Ok(Self {
            state_root: state_root.to_path_buf(),
            operations,
            context_digest: context_digest.to_string(),
            executor,
            backup,
            policy,
        })
    }

    pub fn operations(&self) -> &LiveAddOperations {
        &self.operations
    }

    /// `LiveAddApplication.validate_candidate`.
    pub fn validate_candidate(
        &mut self,
        payload: &Value,
    ) -> Result<(InstallationCandidate, Vec<u8>), RuntimeError> {
        let candidate = InstallationCandidate::from_payload(payload, &self.context_digest)?;
        candidate.require_search_candidate_ready()?;
        if let Some(blocker) = candidate.record_blocker()? {
            return Err(candidate_rejected(blocker));
        }
        if let Some(blocker) = self.policy.catalog_blocker(&candidate)? {
            return Err(candidate_rejected(&blocker));
        }
        if candidate.stage != CandidateStage::FinalRecord {
            return Err(candidate_rejected(
                "Materialize and finalize the candidate before live addition",
            ));
        }
        let assembly = new_assembly_record(candidate.install_record())?;
        Ok((candidate, assembly))
    }

    /// `LiveAddApplication.prepare`.
    pub fn prepare(
        &mut self,
        payload: &Value,
        save_path: &Path,
        previous_operation_id: Option<&str>,
    ) -> Result<PreparedLiveAdd, RuntimeError> {
        let (candidate, assembly) = self.validate_candidate(payload)?;
        let (context, before, index_before) = self.executor.inspect()?;
        for operation_id in self.operations.unresolved_ids()? {
            let (_digest, plan) = self.operations.plan(&operation_id)?;
            if (
                plan.get("pid").cloned(),
                plan.get("process_creation_time").cloned(),
            ) == (
                context.get("pid").cloned(),
                context.get("process_creation_time").cloned(),
            ) {
                return Err(RuntimeError::LiveAddUncertain { operation_id });
            }
        }
        let mapping = index_entries(&index_before.to_json())?;
        let serial = context
            .get("serial")
            .and_then(Value::as_u64)
            .ok_or_else(|| rejected("Prepared live-add plan expired; prepare a new plan"))?
            .to_string();
        if mapping.contains_key(&serial)
            || inventory_entries(&before)?
                .iter()
                .any(|(key, entry)| mapping.get(key).copied() != Some(entry.slot_index as u32))
        {
            return Err(candidate_rejected(
                "Native serial index differs from inventory",
            ));
        }
        let operation_id = new_operation_id()?;
        let source = save_path.canonicalize().map_err(|error| RuntimeError::Io {
            path: save_path.display().to_string(),
            detail: error.to_string(),
        })?;
        let raw = read_bytes(&source)?;
        let checkpoint = self.backup.checkpoint(&source, &raw, &operation_id)?;
        let source_sha256 = sha256_hex(&raw);
        let mut plan: Map<String, Value> = context
            .as_object()
            .cloned()
            .ok_or_else(|| rejected("Prepared live-add plan expired; prepare a new plan"))?;
        plan.insert(
            "parent_operation_id".to_string(),
            json!(operation_id.clone()),
        );
        plan.insert(
            "source_save_path".to_string(),
            json!(source.display().to_string()),
        );
        plan.insert(
            "candidate_id".to_string(),
            json!(candidate.candidate_id.clone()),
        );
        let mut persistence_baseline = before.clone();
        if let Some(previous_operation_id) = previous_operation_id {
            let previous = self.operations.snapshot(previous_operation_id)?;
            if previous.state != OperationState::Verified {
                return Err(rejected("Previous batch item has not been verified"));
            }
            let (_digest, parent) = self.operations.plan(previous_operation_id)?;
            if parent.get("source_save_path").and_then(Value::as_str)
                != Some(source.display().to_string().as_str())
                || parent.get("source_save_sha256").and_then(Value::as_str)
                    != Some(source_sha256.as_str())
            {
                return Err(rejected("Batch source save changed"));
            }
            for field in [
                "pid",
                "process_creation_time",
                "profile_id",
                "manager",
                "data",
                "scheduler_owner",
            ] {
                if parent.get(field) != context.get(field) {
                    return Err(rejected("Batch process context changed"));
                }
            }
            let previous_directory = self.operations.directory(previous_operation_id)?;
            let verified_after = read_json(&previous_directory.join("inventory-after.json"))?;
            let verified_index = read_json(&previous_directory.join("index-after.json"))?;
            let before_json = inventory_json(&before, LIVE_ADD_DISPLAY_VERSION);
            for field in [
                "pid",
                "entries",
                "serial_counter",
                "acquisition_order_counter",
                "container_sha256",
            ] {
                if before_json.get(field) != verified_after.get(field) {
                    return Err(rejected("Inventory changed between batch items"));
                }
            }
            if mapping != index_entries(&verified_index)? {
                return Err(rejected("Native index changed between batch items"));
            }
            persistence_baseline = Inventory::from_json(
                parent
                    .get("persistence_baseline")
                    .or_else(|| parent.get("before"))
                    .ok_or_else(|| rejected("Stored plan content changed"))?,
            )?;
        }
        let saved_records = saved_scroll_records(&checkpoint.decrypted)?;
        verify_persistence(&persistence_baseline, &saved_records, false)?;

        let plan_value = Value::Object(plan.clone());
        let preview = self.executor.preview(&plan_value, &assembly)?;
        let (after_preview, after_index) = self.executor.readback()?;
        let before_json = inventory_json(&before, LIVE_ADD_DISPLAY_VERSION);
        let after_json = inventory_json(&after_preview, LIVE_ADD_DISPLAY_VERSION);
        for field in [
            "pid",
            "entries",
            "serial_counter",
            "acquisition_order_counter",
            "container_sha256",
        ] {
            if before_json.get(field) != after_json.get(field) {
                return Err(rejected(
                    "Preview changed inventory or its planning context expired",
                ));
            }
        }
        if index_entries(&after_index.to_json())? != mapping {
            return Err(rejected("Native serial index changed during preview"));
        }

        plan.insert("operation_id".to_string(), json!(operation_id.clone()));
        plan.insert(
            "descriptor_hex".to_string(),
            json!(hex(&assembly_descriptor(&assembly, true)?)),
        );
        plan.insert("expected_record_hex".to_string(), json!(hex(&assembly)));
        plan.insert(
            "before".to_string(),
            inventory_json(&before, LIVE_ADD_DISPLAY_VERSION),
        );
        plan.insert("index_before".to_string(), index_before.to_json());
        plan.insert("source_save_sha256".to_string(), json!(source_sha256));
        plan.insert(
            "backup_path".to_string(),
            json!(checkpoint.backup_path.display().to_string()),
        );
        plan.insert(
            "persistence_baseline".to_string(),
            inventory_json(&persistence_baseline, LIVE_ADD_DISPLAY_VERSION),
        );
        plan.insert(
            "previous_operation_id".to_string(),
            match previous_operation_id {
                Some(value) => json!(value),
                None => Value::Null,
            },
        );
        exclusive_json(&checkpoint.directory.join("preview.json"), &preview)?;
        let snapshot = self
            .operations
            .prepare(&operation_id, &Value::Object(plan))?;
        Ok(PreparedLiveAdd {
            snapshot,
            seed: candidate.seed,
            rarity: candidate.rarity,
            count_before: before.entries.len(),
            backup_path: checkpoint.backup_path.display().to_string(),
            instance_serial: serial,
        })
    }

    /// `LiveAddApplication.execute`. A claimed operation is never replayed.
    pub fn execute(
        &mut self,
        operation_id: &str,
        plan_digest: &str,
    ) -> Result<OperationSnapshot, RuntimeError> {
        let snapshot = self.operations.snapshot(operation_id)?;
        if snapshot.plan_digest != plan_digest {
            return Err(rejected("Reviewed live-add plan digest differs"));
        }
        if matches!(
            snapshot.state,
            OperationState::Verified | OperationState::RejectedBeforeDispatch
        ) {
            return Ok(snapshot);
        }
        if snapshot.state != OperationState::Prepared {
            return Err(rejected(
                "Operation is cancelled or uncertain; do not replay it",
            ));
        }
        let (_digest, plan) = self.operations.plan(operation_id)?;
        let (current, _inventory, _index) = self.executor.inspect()?;
        for field in [
            "pid",
            "process_creation_time",
            "profile_id",
            "manager",
            "data",
            "serial",
            "slot",
            "scheduler_owner",
            "function_address",
            "container_hex",
            "builder_code_hex",
            "insertion_code_hex",
        ] {
            if current.get(field) != plan.get(field) {
                return Err(rejected(
                    "Prepared live-add plan expired; prepare a new plan",
                ));
            }
        }
        let source_path = plan
            .get("source_save_path")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Stored plan content changed"))?;
        let source_sha256 = plan
            .get("source_save_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Stored plan content changed"))?;
        if sha256_hex(&read_bytes(Path::new(source_path))?) != source_sha256 {
            return Err(rejected("Save changed after preparation; prepare again"));
        }
        let backup_path = plan
            .get("backup_path")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Stored plan content changed"))?;
        let backup = Path::new(backup_path);
        if !backup.is_file() || sha256_hex(&read_bytes(backup)?) != source_sha256 {
            return Err(rejected(
                "Automatic save backup is missing or changed; insertion was not dispatched",
            ));
        }
        self.operations.claim(operation_id, plan_digest)?;
        match self.executor.insert(&plan) {
            Ok(execution) => self.finish(&plan, &execution),
            Err(error) => {
                if self.executor.submission_absent(operation_id) {
                    return self.operations.complete(
                        operation_id,
                        &json!({
                            "operation_id": operation_id,
                            "state": "rejected_before_dispatch",
                            "redirect_count": 0,
                            "error": error.message(),
                        }),
                    );
                }
                Err(error)
            }
        }
    }

    /// `LiveAddApplication._finish`: independent verification, then publish.
    pub fn finish(
        &mut self,
        plan: &Value,
        execution: &Value,
    ) -> Result<OperationSnapshot, RuntimeError> {
        let operation_id = plan
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Stored plan content changed"))?
            .to_string();
        let plan_creation = plan
            .get("process_creation_time")
            .and_then(Value::as_str)
            .map(str::to_string);
        let plan_pid = plan
            .get("pid")
            .and_then(Value::as_u64)
            .ok_or_else(|| rejected("Stored plan content changed"))? as u32;
        let idle_miss = execution.get("redirect_count").and_then(Value::as_u64) == Some(0)
            && execution.get("released").and_then(Value::as_bool) == Some(true)
            && execution.get("active").and_then(Value::as_bool) == Some(false)
            && execution
                .get("breakpoints")
                .and_then(Value::as_array)
                .map(Vec::is_empty)
                == Some(true)
            && execution.get("operation_id").and_then(Value::as_str) == Some(operation_id.as_str())
            && execution.get("pid").and_then(Value::as_u64) == Some(plan_pid as u64);
        if idle_miss {
            return self.operations.complete(
                &operation_id,
                &json!({
                    "operation_id": operation_id,
                    "state": "rejected_before_dispatch",
                    "redirect_count": 0,
                    "error": execution.get("error").cloned().unwrap_or(Value::Null),
                }),
            );
        }
        self.executor
            .require_process_instance(plan_pid, plan_creation.as_deref())?;
        if execution
            .get("process_creation_time")
            .and_then(Value::as_str)
            .or(plan_creation.as_deref())
            != plan_creation.as_deref()
        {
            return Err(rejected(
                "Native receipt belongs to another process lifetime",
            ));
        }
        let (after, index_after) = self.executor.readback()?;
        if after.process_creation_time != plan_creation.clone().unwrap_or_default()
            || index_after.process_creation_time != plan_creation.clone().unwrap_or_default()
        {
            return Err(rejected(
                "PROCESS_INSTANCE_CHANGED: receipt and readback lifetimes differ",
            ));
        }
        let directory = self.operations.directory(&operation_id)?;
        let attempt = directory.join("verification").join(new_operation_id()?);
        std::fs::create_dir_all(&attempt).map_err(|error| RuntimeError::Io {
            path: attempt.display().to_string(),
            detail: error.to_string(),
        })?;
        let before = Inventory::from_json(
            plan.get("before")
                .ok_or_else(|| rejected("Stored plan content changed"))?,
        )?;
        let index_before = NativeIndex::from_json(
            plan.get("index_before")
                .ok_or_else(|| rejected("Stored plan content changed"))?,
        )?;
        let payloads: [(&str, Value); 3] = [
            ("execution", execution.clone()),
            (
                "inventory-after",
                inventory_json(&after, LIVE_ADD_DISPLAY_VERSION),
            ),
            ("index-after", index_after.to_json()),
        ];
        for (name, payload) in &payloads {
            exclusive_json(&attempt.join(format!("{name}.json")), payload)?;
        }
        let result = verify(
            plan,
            execution,
            &before,
            &after,
            &index_before,
            &index_after,
        )?;
        let attempt_name = attempt
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        for (name, payload) in &payloads {
            let temporary = directory.join(format!("{name}-{attempt_name}.tmp"));
            exclusive_json(&temporary, payload)?;
            std::fs::rename(&temporary, directory.join(format!("{name}.json"))).map_err(
                |error| RuntimeError::Io {
                    path: temporary.display().to_string(),
                    detail: error.to_string(),
                },
            )?;
        }
        let mut receipt = result;
        if let Some(object) = receipt.as_object_mut() {
            object.insert("state".to_string(), json!("verified"));
            object.insert(
                "verification_evidence".to_string(),
                json!(attempt.display().to_string()),
            );
        }
        self.operations.complete(&operation_id, &receipt)
    }

    /// `LiveAddApplication.recover`: read the receipt, never dispatch again.
    pub fn recover(&mut self, operation_id: &str) -> Result<OperationSnapshot, RuntimeError> {
        let snapshot = self.operations.snapshot(operation_id)?;
        if snapshot.state != OperationState::Uncertain {
            return Ok(snapshot);
        }
        let (_digest, plan) = self.operations.plan(operation_id)?;
        let pid = plan
            .get("pid")
            .and_then(Value::as_u64)
            .ok_or_else(|| rejected("Stored plan content changed"))? as u32;
        let creation = plan
            .get("process_creation_time")
            .and_then(Value::as_str)
            .map(str::to_string);
        let execution = self
            .executor
            .recover(operation_id, pid, creation.as_deref())?;
        self.finish(&plan, &execution)
    }

    /// `LiveAddApplication.cancel`.
    pub fn cancel(&mut self, operation_id: &str) -> Result<OperationSnapshot, RuntimeError> {
        self.operations.cancel(operation_id)
    }

    pub fn status(&self, operation_id: &str) -> Result<OperationSnapshot, RuntimeError> {
        self.operations.snapshot(operation_id)
    }

    pub fn effective_root(&self) -> &Path {
        &self.state_root
    }

    pub fn safe_to_shutdown(&mut self) -> bool {
        self.executor.safe_to_shutdown()
    }

    /// `runtime.status` fact for live addition.
    pub fn ownership(&mut self) -> Result<LiveAddOwnership, RuntimeError> {
        Ok(LiveAddOwnership {
            safe_to_shutdown: self.executor.safe_to_shutdown(),
            unresolved: self.operations.unresolved_ids()?,
        })
    }

    /// `live_add_application` persistence check against the decrypted save.
    pub fn verify_saved_persistence(
        &self,
        plan: &Value,
        decrypted: &[u8],
    ) -> Result<Value, RuntimeError> {
        let baseline = Inventory::from_json(
            plan.get("persistence_baseline")
                .or_else(|| plan.get("before"))
                .ok_or_else(|| rejected("Stored plan content changed"))?,
        )?;
        verify_persistence(&baseline, &saved_scroll_records(decrypted)?, false)
    }
}

/// The 400 scroll records inside a decrypted save.
pub fn saved_scroll_records(decrypted: &[u8]) -> Result<Vec<Vec<u8>>, RuntimeError> {
    let end = SCROLL_GROUP_OFFSET + SCROLL_SLOT_COUNT * RECORD_SIZE;
    if decrypted.len() < end {
        return Err(rejected("Decrypted save is shorter than the scroll group"));
    }
    Ok((0..SCROLL_SLOT_COUNT)
        .map(|slot| {
            let start = SCROLL_GROUP_OFFSET + slot * RECORD_SIZE;
            decrypted[start..start + RECORD_SIZE].to_vec()
        })
        .collect())
}

fn hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}
