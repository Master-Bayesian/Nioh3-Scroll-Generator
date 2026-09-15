//! Guarded save-transaction lifecycle: plan, commit, discard, backup, restore.
//!
//! Mirrors the shipped Python transaction semantics in
//! `nioh3_scroll_editor/savegame.py` and `save_application.py`:
//!
//! - a write is prepared against a **quiescent** generation (two identical
//!   fingerprints of the main, backup and system files),
//! - the exact quiet generation is copied to a backup directory *before* any
//!   write,
//! - the commit replaces the main file durably, then reads the installed bytes
//!   back and verifies the expected digest,
//! - a failed or unprovable commit rolls the checkpoint back and records the
//!   uncertain outcome in a journal, so a retry never replays silently,
//! - an operation identifier is written once; reusing it is refused.
//!
//! This module never touches a user save or the game. Callers pass explicit
//! fixture paths; the tests drive it only against task-local temporary copies.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::SaveReadError;
use crate::save::sha256_hex;
use crate::transform::{InstallRequest, PlannedWrite, SaveTransformHost, SlotEdit};

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// The shipped quiescence window, in milliseconds.
///
/// Mirrors `savegame.SAVE_QUIESCENCE_SECONDS = 0.20`: the main, backup and
/// system files must be byte-identical across this span before a write is
/// allowed to proceed. It is a timing window, not an instantaneous snapshot:
/// a game that is writing save files stays caught only because the two
/// fingerprint passes are separated by this interval.
pub const SAVE_QUIESCENCE_MILLIS: u64 = 200;

/// Which file of a save generation is being described.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveRole {
    Main,
    GameBackup,
    System,
}

impl SaveRole {
    /// Stable label used in journals and error messages.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Main => "main_save",
            Self::GameBackup => "game_backup",
            Self::System => "system_save",
        }
    }
}

/// One file of a save generation, with the identity needed to detect drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFingerprint {
    pub role: SaveRole,
    pub path: PathBuf,
    pub exists: bool,
    pub length: u64,
    pub modified_nanos: u128,
    pub sha256: String,
}

/// The files a scroll write must keep quiescent.
///
/// Mirrors `savegame.related_save_paths`: the selected `SAVEDATA.BIN`, its
/// sibling `BACKUP.BIN`, and the account's system save.
pub fn related_save_paths(save_path: &Path) -> Vec<(SaveRole, PathBuf)> {
    let parent = save_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let account = parent.parent().map(Path::to_path_buf).unwrap_or_default();
    vec![
        (SaveRole::Main, save_path.to_path_buf()),
        (SaveRole::GameBackup, parent.join("BACKUP.BIN")),
        (
            SaveRole::System,
            account.join("SYSTEMSAVEDATA00").join("SAVEDATA.BIN"),
        ),
    ]
}

fn fingerprint(role: SaveRole, path: &Path) -> Result<FileFingerprint, SaveReadError> {
    match fs::metadata(path) {
        Ok(metadata) => {
            let bytes = fs::read(path).map_err(|error| SaveReadError::Io {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
            let modified_nanos = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            Ok(FileFingerprint {
                role,
                path: path.to_path_buf(),
                exists: true,
                length: metadata.len(),
                modified_nanos,
                sha256: sha256_hex(&bytes),
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FileFingerprint {
            role,
            path: path.to_path_buf(),
            exists: false,
            length: 0,
            modified_nanos: 0,
            sha256: String::new(),
        }),
        Err(error) => Err(SaveReadError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        }),
    }
}

/// Fingerprint the whole generation once.
pub fn capture_related_fingerprints(
    save_path: &Path,
) -> Result<Vec<FileFingerprint>, SaveReadError> {
    related_save_paths(save_path)
        .into_iter()
        .map(|(role, path)| fingerprint(role, &path))
        .collect()
}

/// A prepared, not yet committed write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavePlan {
    pub plan_id: String,
    /// Backup directory this plan will create (or must read, for a restore).
    pub backup_id: String,
    pub kind: PlanKind,
    pub save_path: PathBuf,
    pub source_sha256: String,
    /// Account identity the plan was prepared against.
    #[serde(default)]
    pub account_id: String,
    /// Character slot the plan was prepared against.
    #[serde(default)]
    pub save_slot: String,
    /// The product operation to replay at commit time. `None` marks a raw byte
    /// install, which only the bounded transaction self-test uses.
    #[serde(default)]
    pub product: Option<ProductPlanData>,
    pub command: PlanCommand,
    pub baseline: Vec<FileFingerprint>,
}

/// The product operation a plan must replay at commit time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum ProductPlanData {
    Edit { edits: Vec<SlotEdit> },
    Delete { slots: Vec<usize> },
    Install { request: InstallRequest },
    InstallMany { requests: Vec<InstallRequest> },
}

/// The product operation a plan represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanKind {
    Edit,
    Delete,
    Install,
    Restore,
}

impl PlanKind {
    /// Stable label used in receipts.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::Delete => "delete",
            Self::Install => "install",
            Self::Restore => "restore",
        }
    }
}

/// What the commit must do to the main file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanCommand {
    /// Replace the main file with exactly these bytes.
    WriteMain { bytes: Vec<u8> },
    /// Restore the main file from a recorded backup directory.
    RestoreFromBackup { backup_id: String },
}

/// Injected transaction faults for the fault gate.
///
/// Counters live behind an `Arc`, so a helper can arm one fault set and share it
/// with the host it drives. Every trigger fires once.
#[derive(Debug, Clone, Default)]
pub struct TransactionFaults {
    inner: Arc<FaultCounters>,
}

#[derive(Debug, Default)]
struct FaultCounters {
    fail_after_checkpoint: AtomicU32,
    fail_after_receipt: AtomicU32,
    fail_after_stage: AtomicU32,
    fail_after_replace: AtomicU32,
    fail_after_readback: AtomicU32,
}

/// One injectable commit stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    AfterCheckpoint,
    AfterReceipt,
    AfterStage,
    AfterReplace,
    AfterReadback,
}

impl FaultPoint {
    /// Every injectable stage, in commit order.
    pub const ALL: [Self; 5] = [
        Self::AfterCheckpoint,
        Self::AfterReceipt,
        Self::AfterStage,
        Self::AfterReplace,
        Self::AfterReadback,
    ];

    /// Stable label used by the fault gate's markers.
    pub const fn label(self) -> &'static str {
        match self {
            Self::AfterCheckpoint => "after-checkpoint",
            Self::AfterReceipt => "after-receipt",
            Self::AfterStage => "after-stage",
            Self::AfterReplace => "after-replace",
            Self::AfterReadback => "after-readback",
        }
    }
}

impl TransactionFaults {
    /// Arm one point to fail exactly once.
    pub fn arm(&self, point: FaultPoint) {
        self.counter(point).store(1, Ordering::SeqCst);
    }

    fn counter(&self, point: FaultPoint) -> &AtomicU32 {
        match point {
            FaultPoint::AfterCheckpoint => &self.inner.fail_after_checkpoint,
            FaultPoint::AfterReceipt => &self.inner.fail_after_receipt,
            FaultPoint::AfterStage => &self.inner.fail_after_stage,
            FaultPoint::AfterReplace => &self.inner.fail_after_replace,
            FaultPoint::AfterReadback => &self.inner.fail_after_readback,
        }
    }

    /// Whether this armed point fires now, consuming exactly one arming.
    fn should_fire(&self, point: FaultPoint) -> bool {
        self.counter(point)
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                if value == 0 {
                    None
                } else {
                    Some(value - 1)
                }
            })
            .is_ok()
    }
}

/// The terminal record of one commit attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationReceipt {
    pub operation_id: String,
    pub kind: String,
    pub outcome: String,
    pub installed_sha256: Option<String>,
    pub backup_id: Option<String>,
    pub message: Option<String>,
}

/// A guarded save-transaction host rooted at one state directory.
/// The bytes one commit will install, plus the identity of the decrypted
/// payload when the operation was a product transform.
struct PreparedBytes {
    container: Vec<u8>,
    plaintext_sha256: Option<String>,
}

/// A guarded save-transaction host rooted at one state directory.
pub struct SaveTransactionHost {
    state_root: PathBuf,
    backup_root: PathBuf,
    quiescence_interval: Duration,
    faults: TransactionFaults,
}

impl SaveTransactionHost {
    /// Create a host whose backups and journals live under `state_root`.
    pub fn new(state_root: &Path) -> Self {
        Self {
            state_root: state_root.to_path_buf(),
            backup_root: state_root.join("backups"),
            quiescence_interval: Duration::from_millis(SAVE_QUIESCENCE_MILLIS),
            faults: TransactionFaults::default(),
        }
    }

    /// Create a host with a caller-supplied fault set for the fault gate.
    pub fn with_faults(state_root: &Path, faults: TransactionFaults) -> Self {
        Self {
            state_root: state_root.to_path_buf(),
            backup_root: state_root.join("backups"),
            quiescence_interval: Duration::ZERO,
            faults,
        }
    }

    /// Keep this host's journals private but read and write backup bundles in a
    /// different root.
    ///
    /// The protected host's canonical backup root is
    /// `<state_root>/backups` — the same public directory the shipped host
    /// keeps, so bundles a user already has are discovered and restored in
    /// place. The transaction's own plans and receipts stay under the private
    /// root so they cannot collide with the protected ledger.
    pub fn with_backup_root(mut self, backup_root: PathBuf) -> Self {
        self.backup_root = backup_root;
        self
    }

    /// The fault set this host consults.
    pub fn faults(&self) -> &TransactionFaults {
        &self.faults
    }

    /// Disable the quiescence delay.
    ///
    /// The delay is skipped, never the comparison: the two fingerprint passes
    /// still have to agree. The fault gate uses this to stay fast.
    pub fn without_quiescence_delay(mut self) -> Self {
        self.quiescence_interval = Duration::ZERO;
        self
    }

    /// The active quiescence window.
    pub const fn quiescence_interval(&self) -> Duration {
        self.quiescence_interval
    }

    /// Require a different quiet window than the shipped default.
    ///
    /// The window has one source of truth in the shipped tool
    /// (`SAVE_QUIESCENCE_SECONDS`); a caller may widen it (a deterministic
    /// external-writer probe) or shrink it to zero, but the two-pass comparison
    /// is not optional.
    pub fn with_quiescence_interval(mut self, interval: Duration) -> Self {
        self.quiescence_interval = interval;
        self
    }

    fn receipt_dir(&self) -> PathBuf {
        self.state_root.join("v2-operations")
    }

    fn plan_dir(&self) -> PathBuf {
        self.state_root.join("v2-plans")
    }

    /// The backup directory a plan or restore will use.
    pub fn backup_dir(&self, backup_id: &str) -> PathBuf {
        self.backup_root.join(backup_id)
    }

    /// The root that holds backup bundles for this host.
    pub fn backup_root(&self) -> &Path {
        &self.backup_root
    }

    /// Read one recorded receipt, if the operation id exists.
    pub fn receipt(&self, operation_id: &str) -> Result<Option<OperationReceipt>, SaveReadError> {
        let path = self.receipt_dir().join(format!("{operation_id}.json"));
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .map(Some)
                .map_err(|error| SaveReadError::Io {
                    path: path.display().to_string(),
                    message: error.to_string(),
                }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(SaveReadError::Io {
                path: path.display().to_string(),
                message: error.to_string(),
            }),
        }
    }

    /// Read every recorded receipt, newest name order, validating each one.
    ///
    /// Mirrors `SaveApplication.operations`: a receipt that fails validation is
    /// skipped rather than returned, so an untrusted ledger file cannot surface
    /// as an operation.
    pub fn operations(&self) -> Result<Vec<OperationReceipt>, SaveReadError> {
        let directory = self.receipt_dir();
        if !directory.is_dir() {
            return Ok(Vec::new());
        }
        let mut names: Vec<String> = Vec::new();
        for entry in fs::read_dir(&directory).map_err(|error| SaveReadError::Io {
            path: directory.display().to_string(),
            message: error.to_string(),
        })? {
            let entry = entry.map_err(|error| SaveReadError::Io {
                path: directory.display().to_string(),
                message: error.to_string(),
            })?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".json") {
                if is_operation_id(stem) {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        names.reverse();
        let mut receipts = Vec::with_capacity(names.len());
        for name in names {
            if let Ok(Some(receipt)) = self.receipt(&name) {
                if validate_receipt(&receipt).is_ok() {
                    receipts.push(receipt);
                }
            }
        }
        Ok(receipts)
    }

    /// Reconcile a receipt whose outcome could not be proven.
    ///
    /// Mirrors `SaveApplication.operation`: a receipt still marked `pending`
    /// means a previous process ended before recording the outcome, so it is
    /// reported as `unknown` with the same warning and never replayed.
    pub fn reconcile(&self, operation_id: &str) -> Result<Option<OperationReceipt>, SaveReadError> {
        let Some(mut receipt) = self.receipt(operation_id)? else {
            return Ok(None);
        };
        validate_receipt(&receipt)?;
        if receipt.outcome == "pending" {
            receipt.outcome = "unknown".to_string();
            receipt.message = Some(
                "Previous process ended before recording the outcome; inspect backups and \
                 the current save before further writes"
                    .to_string(),
            );
        }
        Ok(Some(receipt))
    }

    /// Persist a prepared plan so a later process can commit it by id.
    pub fn store_plan(&self, plan: &SavePlan) -> Result<(), SaveReadError> {
        let directory = self.plan_dir();
        fs::create_dir_all(&directory).map_err(|error| SaveReadError::Io {
            path: directory.display().to_string(),
            message: error.to_string(),
        })?;
        let text = serde_json::to_string(plan).map_err(|error| SaveReadError::Io {
            path: plan.plan_id.clone(),
            message: error.to_string(),
        })?;
        let path = directory.join(format!("{}.json", plan.plan_id));
        write_durable(&path, text.as_bytes())
    }

    /// Load a plan previously stored with [`Self::store_plan`].
    pub fn load_plan(&self, plan_id: &str) -> Result<SavePlan, SaveReadError> {
        let path = self.plan_dir().join(format!("{plan_id}.json"));
        let text = fs::read_to_string(&path).map_err(|_| SaveReadError::UnknownIdentifier {
            kind: "plan",
            value: plan_id.to_string(),
        })?;
        serde_json::from_str(&text).map_err(|error| SaveReadError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })
    }

    /// Prepare a write against a quiescent generation.
    ///
    /// `expected_source_sha256` is the digest the caller observed when it built
    /// the command; a mismatch means the save moved on and the plan is refused.
    pub fn plan(
        &self,
        kind: PlanKind,
        save_path: &Path,
        expected_source_sha256: &str,
        command: PlanCommand,
    ) -> Result<SavePlan, SaveReadError> {
        let baseline = self.require_quiescent(save_path)?;
        let main = baseline
            .iter()
            .find(|entry| entry.role == SaveRole::Main)
            .ok_or(SaveReadError::SaveChanged {
                path: save_path.display().to_string(),
            })?;
        if !main.exists {
            return Err(SaveReadError::SaveChanged {
                path: save_path.display().to_string(),
            });
        }
        if !main.sha256.eq_ignore_ascii_case(expected_source_sha256) {
            return Err(SaveReadError::IntegrityMismatch {
                path: save_path.display().to_string(),
                expected: expected_source_sha256.to_ascii_lowercase(),
                actual: main.sha256.clone(),
            });
        }
        Ok(SavePlan {
            plan_id: entropy_id(),
            backup_id: match &command {
                PlanCommand::RestoreFromBackup { backup_id } => backup_id.clone(),
                PlanCommand::WriteMain { .. } => String::new(),
            },
            kind,
            save_path: save_path.to_path_buf(),
            source_sha256: main.sha256.clone(),
            account_id: String::new(),
            save_slot: String::new(),
            product: None,
            command,
            baseline,
        })
    }

    /// Prepare a product operation against a quiescent generation.
    ///
    /// The returned plan carries the validated account/slot identity and the
    /// product operation itself, so the commit can rebuild the exact bytes from
    /// the live save instead of trusting bytes stored on disk.
    pub fn plan_product(
        &self,
        kind: PlanKind,
        save_path: &Path,
        expected_source_sha256: &str,
        product: ProductPlanData,
    ) -> Result<SavePlan, SaveReadError> {
        let baseline = self.require_quiescent(save_path)?;
        let main = baseline
            .iter()
            .find(|entry| entry.role == SaveRole::Main)
            .ok_or(SaveReadError::SaveChanged {
                path: save_path.display().to_string(),
            })?;
        if !main.exists {
            return Err(SaveReadError::SaveChanged {
                path: save_path.display().to_string(),
            });
        }
        if !main.sha256.eq_ignore_ascii_case(expected_source_sha256) {
            return Err(SaveReadError::IntegrityMismatch {
                path: save_path.display().to_string(),
                expected: expected_source_sha256.to_ascii_lowercase(),
                actual: main.sha256.clone(),
            });
        }
        let account_id = crate::paths::account_id_from_save_path(save_path)?;
        let save_slot = crate::paths::save_slot_index_from_path(save_path)?;
        Ok(SavePlan {
            plan_id: entropy_id(),
            backup_id: String::new(),
            kind,
            save_path: save_path.to_path_buf(),
            source_sha256: main.sha256.clone(),
            account_id: account_id.to_string(),
            save_slot: save_slot.to_string(),
            product: Some(product),
            command: PlanCommand::WriteMain { bytes: Vec::new() },
            baseline,
        })
    }

    /// Prepare an edit of existing occupied slots.
    pub fn plan_edit(
        &self,
        save_path: &Path,
        expected_source_sha256: &str,
        edits: Vec<SlotEdit>,
    ) -> Result<SavePlan, SaveReadError> {
        self.plan_product(
            PlanKind::Edit,
            save_path,
            expected_source_sha256,
            ProductPlanData::Edit { edits },
        )
    }

    /// Prepare an in-place delete of occupied slots.
    pub fn plan_delete(
        &self,
        save_path: &Path,
        expected_source_sha256: &str,
        slots: Vec<usize>,
    ) -> Result<SavePlan, SaveReadError> {
        self.plan_product(
            PlanKind::Delete,
            save_path,
            expected_source_sha256,
            ProductPlanData::Delete { slots },
        )
    }

    /// Prepare one candidate install.
    pub fn plan_install(
        &self,
        save_path: &Path,
        expected_source_sha256: &str,
        request: InstallRequest,
    ) -> Result<SavePlan, SaveReadError> {
        self.plan_product(
            PlanKind::Install,
            save_path,
            expected_source_sha256,
            ProductPlanData::Install { request },
        )
    }

    /// Prepare a batch candidate install.
    pub fn plan_install_many(
        &self,
        save_path: &Path,
        expected_source_sha256: &str,
        requests: Vec<InstallRequest>,
    ) -> Result<SavePlan, SaveReadError> {
        self.plan_product(
            PlanKind::Install,
            save_path,
            expected_source_sha256,
            ProductPlanData::InstallMany { requests },
        )
    }

    /// Drop a plan so it can never be committed.
    pub fn discard(&self, plan: SavePlan) -> OperationReceipt {
        // The persisted plan is removed, so a later commit by id fails closed.
        let _ = fs::remove_file(self.plan_dir().join(format!("{}.json", plan.plan_id)));
        OperationReceipt {
            operation_id: plan.plan_id,
            kind: plan.kind.label().to_string(),
            outcome: "discarded".to_string(),
            installed_sha256: None,
            backup_id: None,
            message: None,
        }
    }

    /// Commit a plan: quiescence, backup, durable replace, readback, receipt.
    pub fn commit(&self, plan: &SavePlan) -> Result<OperationReceipt, SaveReadError> {
        self.validate_plan(plan)?;
        if self.receipt(&plan.plan_id)?.is_some() {
            return Err(SaveReadError::UnknownIdentifier {
                kind: "already-committed operation",
                value: plan.plan_id.clone(),
            });
        }
        // The plan must still describe its own recorded identity before any
        // filesystem gate runs, so a rewritten target names the account or slot
        // that failed instead of surfacing as generic drift.
        self.require_plan_identity(plan)?;
        // The generation must not have moved since preparation.
        let stage = std::time::Instant::now();
        self.require_generation_unchanged(&plan.save_path, &plan.baseline)?;
        timing("quiescent-baseline", stage);
        let stage = std::time::Instant::now();
        let backup_id = if plan.backup_id.is_empty() {
            self.write_backup(&plan.plan_id, plan.kind.label(), &plan.baseline)?
        } else {
            // A restore must not overwrite the checkpoint it reads from.
            plan.backup_id.clone()
        };
        timing("checkpoint", stage);
        if self.faults.should_fire(FaultPoint::AfterCheckpoint) {
            return Err(SaveReadError::InjectedFault {
                stage: FaultPoint::AfterCheckpoint.label().to_string(),
            });
        }
        let stage = std::time::Instant::now();
        let prepared = self.expected_bytes_for_commit(plan, &backup_id)?;
        timing("prepare-bytes", stage);
        let expected = prepared.container;
        // The digest of the bytes this commit will own once the replace lands.
        let owned_sha256 = sha256_hex(&expected);
        let stage = std::time::Instant::now();
        self.write_receipt(&OperationReceipt {
            operation_id: plan.plan_id.clone(),
            kind: plan.kind.label().to_string(),
            outcome: "pending".to_string(),
            installed_sha256: Some(sha256_hex(&expected)),
            backup_id: Some(backup_id.clone()),
            message: None,
        })?;
        timing("durable-intent", stage);
        if self.faults.should_fire(FaultPoint::AfterReceipt) {
            return Err(SaveReadError::InjectedFault {
                stage: FaultPoint::AfterReceipt.label().to_string(),
            });
        }
        let stage = std::time::Instant::now();
        let installed = self.install(
            plan,
            &expected,
            &owned_sha256,
            prepared.plaintext_sha256.as_deref(),
        );
        timing("install+readback", stage);
        match installed {
            Ok(installed) => {
                let receipt = OperationReceipt {
                    operation_id: plan.plan_id.clone(),
                    kind: plan.kind.label().to_string(),
                    outcome: "committed".to_string(),
                    installed_sha256: Some(installed),
                    backup_id: Some(backup_id),
                    message: None,
                };
                self.write_receipt(&receipt)?;
                Ok(receipt)
            }
            Err(error) => {
                // A fault injected after the staged write exists but before the
                // replacement is a known, non-mutating outcome: the durable
                // receipt stays `pending` rather than being resolved to
                // `not_committed`, so a later reader can tell the operation was
                // interrupted at a named stage.
                if matches!(error, SaveReadError::InjectedFault { .. })
                    && error.to_string().contains("after-stage")
                {
                    let receipt = OperationReceipt {
                        operation_id: plan.plan_id.clone(),
                        kind: plan.kind.label().to_string(),
                        outcome: "pending".to_string(),
                        installed_sha256: Some(owned_sha256.clone()),
                        backup_id: Some(backup_id.clone()),
                        message: Some(error.to_string()),
                    };
                    self.write_receipt(&receipt)?;
                    return Err(error);
                }
                // Unprovable outcome. Roll the checkpoint back ONLY if the bytes
                // on disk are still the ones this commit wrote; an externally
                // changed file is never overwritten, it is reported as uncertain
                // with no mutation.
                let replaced = replaced_by_this_commit(&plan.save_path, &owned_sha256);
                let rollback = if replaced {
                    self.rollback(&plan.save_path, &backup_id, &owned_sha256)
                } else {
                    Ok(())
                };
                let outcome = if replaced {
                    "uncertain"
                } else {
                    "not_committed"
                };
                let receipt = OperationReceipt {
                    operation_id: plan.plan_id.clone(),
                    kind: plan.kind.label().to_string(),
                    outcome: outcome.to_string(),
                    installed_sha256: None,
                    backup_id: Some(backup_id),
                    message: Some(match rollback {
                        Ok(()) => format!("{error}; checkpoint restored"),
                        Err(rollback_error) => {
                            format!("{error}; rollback also failed: {rollback_error}")
                        }
                    }),
                };
                self.write_receipt(&receipt)?;
                if replaced {
                    Err(SaveReadError::CommitUncertain {
                        message: receipt.message.unwrap_or_default(),
                    })
                } else {
                    Err(error)
                }
            }
        }
    }

    /// Commit a plan loaded from disk by id.
    ///
    /// This is the cross-process entry point: the plan file is untrusted input
    /// and is validated before anything is replayed.
    pub fn commit_by_id(&self, plan_id: &str) -> Result<OperationReceipt, SaveReadError> {
        if let Some(receipt) = self.receipt(plan_id)? {
            return Ok(receipt);
        }
        let plan = self.load_plan_untrusted(plan_id)?;
        self.commit(&plan)
    }

    /// Load a stored plan and validate every identity field.
    pub fn load_plan_untrusted(&self, plan_id: &str) -> Result<SavePlan, SaveReadError> {
        if !is_operation_id(plan_id) {
            return Err(SaveReadError::TamperedRecord {
                kind: "plan",
                message: format!("operation id {plan_id:?} is not 32 lowercase hex characters"),
            });
        }
        let path = self.plan_dir().join(format!("{plan_id}.json"));
        let text = fs::read_to_string(&path).map_err(|_| SaveReadError::UnknownIdentifier {
            kind: "plan",
            value: plan_id.to_string(),
        })?;
        let plan: SavePlan =
            serde_json::from_str(&text).map_err(|error| SaveReadError::TamperedRecord {
                kind: "plan",
                message: error.to_string(),
            })?;
        if plan.plan_id != plan_id {
            return Err(SaveReadError::TamperedRecord {
                kind: "plan",
                message: "the stored plan id does not match its file name".to_string(),
            });
        }
        self.validate_plan(&plan)?;
        Ok(plan)
    }

    /// Validate a plan as untrusted input before any write.
    fn validate_plan(&self, plan: &SavePlan) -> Result<(), SaveReadError> {
        if !is_operation_id(&plan.plan_id) {
            return Err(SaveReadError::TamperedRecord {
                kind: "plan",
                message: format!(
                    "operation id {:?} is not 32 lowercase hex characters",
                    plan.plan_id
                ),
            });
        }
        if !is_sha256(&plan.source_sha256) {
            return Err(SaveReadError::TamperedRecord {
                kind: "plan",
                message: "source_sha256 is not a 64-character hex digest".to_string(),
            });
        }
        let main = plan
            .baseline
            .iter()
            .find(|entry| entry.role == SaveRole::Main)
            .ok_or_else(|| SaveReadError::TamperedRecord {
                kind: "plan",
                message: "the baseline has no main-save entry".to_string(),
            })?;
        if !main.exists || !main.sha256.eq_ignore_ascii_case(&plan.source_sha256) {
            return Err(SaveReadError::TamperedRecord {
                kind: "plan",
                message: "the baseline main digest does not match source_sha256".to_string(),
            });
        }
        if !plan.backup_id.is_empty() && !is_safe_component(&plan.backup_id) {
            return Err(SaveReadError::TamperedRecord {
                kind: "plan",
                message: "backup_id escapes the managed backups root".to_string(),
            });
        }
        Ok(())
    }

    /// Validate that a plan targets this account and slot, and revalidate the
    /// account/slot binding recorded for it.
    fn require_plan_identity(&self, plan: &SavePlan) -> Result<(), SaveReadError> {
        // A rewritten path may no longer even carry a numeric account directory,
        // so surface that as a target mismatch rather than a bare path error.
        let target_mismatch = |message: String| SaveReadError::PlanTargetMismatch {
            expected: format!(
                "account {} slot {} from the prepared plan",
                plan.account_id, plan.save_slot
            ),
            actual: message,
        };
        let account_id =
            crate::paths::account_id_from_save_path(&plan.save_path).map_err(|_| {
                target_mismatch(format!(
                    "a path with no usable account identity: {}",
                    plan.save_path.display()
                ))
            })?;
        let save_slot = crate::paths::save_slot_index_from_path(&plan.save_path).map_err(|_| {
            target_mismatch(format!(
                "a path with no usable slot identity: {}",
                plan.save_path.display()
            ))
        })?;
        if plan.product.is_some() {
            // The plan records the account and slot it was prepared against. A
            // rewritten `save_path` changes the identity derived from the path,
            // so this refuses the plan before any filesystem gate runs and names
            // which binding broke.
            if let Ok(recorded_account) = plan.account_id.parse::<u64>() {
                if recorded_account != account_id {
                    return Err(SaveReadError::PlanTargetMismatch {
                        expected: format!("account {recorded_account}"),
                        actual: format!("account {account_id} at {}", plan.save_path.display()),
                    });
                }
            }
            if let Ok(recorded_slot) = plan.save_slot.parse::<u8>() {
                if recorded_slot != save_slot {
                    return Err(SaveReadError::PlanTargetMismatch {
                        expected: format!("slot {recorded_slot}"),
                        actual: format!("slot {save_slot} at {}", plan.save_path.display()),
                    });
                }
            }
            if plan.account_id != account_id.to_string() {
                return Err(SaveReadError::TamperedRecord {
                    kind: "plan",
                    message: format!(
                        "plan account {} does not match the save path account {account_id}",
                        plan.account_id
                    ),
                });
            }
            if plan.save_slot != save_slot.to_string() {
                return Err(SaveReadError::TamperedRecord {
                    kind: "plan",
                    message: format!(
                        "plan slot {} does not match the save path slot {save_slot}",
                        plan.save_slot
                    ),
                });
            }
        }
        Ok(())
    }

    /// The exact bytes this commit must install.
    fn expected_bytes_for_commit(
        &self,
        plan: &SavePlan,
        backup_id: &str,
    ) -> Result<PreparedBytes, SaveReadError> {
        match &plan.product {
            Some(product) => {
                let host = SaveTransformHost::register(&plan.save_path)?;
                let planned: PlannedWrite = match product {
                    ProductPlanData::Edit { edits } => host.edit(edits)?,
                    ProductPlanData::Delete { slots } => host.delete(slots)?,
                    ProductPlanData::Install { request } => host.install(request)?,
                    ProductPlanData::InstallMany { requests } => host.install_many(requests)?,
                };
                let container = crate::crypto::encrypt_container(&planned.plaintext)?;
                Ok(PreparedBytes {
                    container,
                    plaintext_sha256: Some(planned.plaintext_sha256),
                })
            }
            None => Ok(PreparedBytes {
                container: self.expected_bytes(plan, backup_id)?,
                plaintext_sha256: None,
            }),
        }
    }

    fn expected_bytes(&self, plan: &SavePlan, backup_id: &str) -> Result<Vec<u8>, SaveReadError> {
        match &plan.command {
            PlanCommand::WriteMain { bytes } => Ok(bytes.clone()),
            PlanCommand::RestoreFromBackup {
                backup_id: recorded,
            } => {
                let _ = backup_id;
                let source = self
                    .backup_dir(backup_id)
                    .join(crate::backup::role_backup_file(SaveRole::Main));
                let _ = recorded;
                fs::read(&source).map_err(|_| SaveReadError::UnknownIdentifier {
                    kind: "backup",
                    value: backup_id.to_string(),
                })
            }
        }
    }

    fn install(
        &self,
        plan: &SavePlan,
        expected: &[u8],
        owned_sha256: &str,
        prepared_plaintext_sha256: Option<&str>,
    ) -> Result<String, SaveReadError> {
        // The write target is derived from the validated plan, never from an
        // argument a caller could redirect.
        let save_path = plan.save_path.as_path();
        let staged =
            save_path.with_file_name(format!("{}.scroll-generator.tmp", file_name(save_path)?));
        if staged.exists() {
            let _ = fs::remove_file(&staged);
        }
        let stage = std::time::Instant::now();
        write_durable(&staged, expected)?;
        timing("stage-write", stage);
        if self.faults.should_fire(FaultPoint::AfterStage) {
            let _ = fs::remove_file(&staged);
            return Err(SaveReadError::InjectedFault {
                stage: FaultPoint::AfterStage.label().to_string(),
            });
        }
        // Staging and receipt I/O happen after the earlier quiescence check, so
        // revalidate the whole generation across one more window immediately
        // before the replacement. A save that moved while this commit was
        // staging is refused with its bytes intact instead of being
        // overwritten; the shipped commit rechecks at exactly this point.
        let stage = std::time::Instant::now();
        if let Err(error) = self.require_generation_unchanged(save_path, &plan.baseline) {
            let _ = fs::remove_file(&staged);
            return Err(error);
        }
        timing("revalidate", stage);
        // One-step replacement: the old file is never removed before the new
        // one lands, so a crash cannot leave the slot empty.
        let stage = std::time::Instant::now();
        replace_durable(&staged, save_path)?;
        timing("replace", stage);
        if self.faults.should_fire(FaultPoint::AfterReplace) {
            return Err(SaveReadError::InjectedFault {
                stage: FaultPoint::AfterReplace.label().to_string(),
            });
        }
        let stage = std::time::Instant::now();
        let readback = fs::read(save_path).map_err(|error| SaveReadError::Io {
            path: save_path.display().to_string(),
            message: error.to_string(),
        })?;
        let digest = sha256_hex(&readback);
        // `owned_sha256` is the digest of exactly these bytes, computed once by
        // the caller; recomputing it here would hash the same buffer again.
        if !digest.eq_ignore_ascii_case(owned_sha256) {
            return Err(SaveReadError::IntegrityMismatch {
                path: save_path.display().to_string(),
                expected: owned_sha256.to_ascii_lowercase(),
                actual: digest,
            });
        }
        // Readback verification of the decrypted payload: the installed
        // container must decrypt to the exact plaintext this commit prepared,
        // exactly as the shipped commit re-decrypts and compares. The expected
        // identity is captured *before* the write: re-deriving it from the
        // already-replaced file would only prove the file equals itself.
        if let Some(prepared) = prepared_plaintext_sha256 {
            let installed = crate::crypto::decrypt_container(&readback)?;
            let installed_sha256 = sha256_hex(&installed);
            if !installed_sha256.eq_ignore_ascii_case(prepared) {
                return Err(SaveReadError::IntegrityMismatch {
                    path: save_path.display().to_string(),
                    expected: prepared.to_string(),
                    actual: installed_sha256,
                });
            }
            crate::save::DecryptedSave::new(installed)?;
        }
        timing("readback", stage);
        if self.faults.should_fire(FaultPoint::AfterReadback) {
            return Err(SaveReadError::InjectedFault {
                stage: FaultPoint::AfterReadback.label().to_string(),
            });
        }
        Ok(digest)
    }

    /// Restore the checkpoint, but only over bytes this commit owns.
    ///
    /// `expected_current_sha256` is the digest of the bytes this commit wrote. If
    /// the file on disk no longer matches it, something else changed the save, so
    /// the rollback must not clobber that work: the caller records an uncertain
    /// receipt and leaves the file exactly as it found it.
    fn rollback(
        &self,
        save_path: &Path,
        backup_id: &str,
        expected_current_sha256: &str,
    ) -> Result<(), SaveReadError> {
        let source = self
            .backup_dir(backup_id)
            .join(crate::backup::role_backup_file(SaveRole::Main));
        let bytes = fs::read(&source).map_err(|error| SaveReadError::Io {
            path: source.display().to_string(),
            message: error.to_string(),
        })?;
        let current = fs::read(save_path).map_err(|error| SaveReadError::Io {
            path: save_path.display().to_string(),
            message: error.to_string(),
        })?;
        if !sha256_hex(&current).eq_ignore_ascii_case(expected_current_sha256) {
            return Err(SaveReadError::SaveChanged {
                path: save_path.display().to_string(),
            });
        }
        replace_durable_bytes(&source, save_path, &bytes)
    }

    fn require_quiescent(&self, save_path: &Path) -> Result<Vec<FileFingerprint>, SaveReadError> {
        let first = capture_related_fingerprints(save_path)?;
        if !self.quiescence_interval.is_zero() {
            std::thread::sleep(self.quiescence_interval);
        }
        let second = capture_related_fingerprints(save_path)?;
        if first != second {
            return Err(SaveReadError::SaveChanged {
                path: save_path.display().to_string(),
            });
        }
        Ok(second)
    }

    /// Re-capture the generation across a fresh window and require it to match
    /// the recorded baseline.
    ///
    /// The shipped `commit_encrypted_main_save` refuses a commit whose related
    /// files moved between preparation and the replacement, because an earlier
    /// quiescence check is not a compare-and-swap. The window is applied here
    /// too, so a writer that starts during the window is still caught.
    fn require_generation_unchanged(
        &self,
        save_path: &Path,
        expected: &[FileFingerprint],
    ) -> Result<(), SaveReadError> {
        let current = self.require_quiescent(save_path)?;
        if current != expected {
            return Err(SaveReadError::SaveChanged {
                path: save_path.display().to_string(),
            });
        }
        Ok(())
    }

    fn write_backup(
        &self,
        operation_id: &str,
        action: &str,
        baseline: &[FileFingerprint],
    ) -> Result<String, SaveReadError> {
        let backup_id = format!("{}-{}", timestamp_label(), operation_id);
        let directory = self.backup_dir(&backup_id);
        fs::create_dir_all(&directory).map_err(|error| SaveReadError::Io {
            path: directory.display().to_string(),
            message: error.to_string(),
        })?;
        let mut copied: Vec<crate::backup::BackupFileEntry> = Vec::new();
        for entry in baseline {
            if !entry.exists {
                continue;
            }
            // Bundle names mirror the shipped layout (`SAVEDATA.BIN`,
            // `BACKUP.BIN`, `SYSTEMSAVEDATA.BIN`); the role label is the
            // manifest's `source_role`, not the file name.
            let backup_file = crate::backup::role_backup_file(entry.role);
            let target = directory.join(backup_file);
            let bytes = fs::read(&entry.path).map_err(|error| SaveReadError::Io {
                path: entry.path.display().to_string(),
                message: error.to_string(),
            })?;
            write_durable(&target, &bytes)?;
            let copied_sha256 = sha256_hex(&bytes);
            if !copied_sha256.eq_ignore_ascii_case(&entry.sha256) {
                return Err(SaveReadError::SaveChanged {
                    path: entry.path.display().to_string(),
                });
            }
            copied.push(crate::backup::BackupFileEntry {
                source_role: entry.role.label().to_string(),
                source_path: entry.path.display().to_string(),
                backup_file: backup_file.to_string(),
                size: entry.length,
                sha256: copied_sha256.to_ascii_uppercase(),
            });
        }
        // The manifest identifies the bundle before any save byte is replaced, so
        // a restore can validate schema, profile, account and slot.
        let main = baseline
            .iter()
            .find(|entry| entry.role == SaveRole::Main)
            .ok_or_else(|| SaveReadError::TamperedRecord {
                kind: "plan",
                message: "the baseline has no main-save entry".to_string(),
            })?;
        let manifest = crate::backup::BackupManifest {
            backup_manifest_schema: crate::backup::BACKUP_MANIFEST_SCHEMA.to_string(),
            save_schema_profile: crate::backup::SAVE_SCHEMA_PROFILE.to_string(),
            operation_id: operation_id.to_string(),
            created_at_utc: timestamp_label(),
            action: action.to_string(),
            steam_account_id: crate::paths::account_id_from_save_path(&main.path)?,
            save_slot_index: crate::paths::save_slot_index_from_path(&main.path)?,
            backup_files: copied,
        };
        crate::backup::write_backup_manifest(&directory, &manifest)?;
        Ok(backup_id)
    }

    fn write_receipt(&self, receipt: &OperationReceipt) -> Result<(), SaveReadError> {
        let directory = self.receipt_dir();
        fs::create_dir_all(&directory).map_err(|error| SaveReadError::Io {
            path: directory.display().to_string(),
            message: error.to_string(),
        })?;
        let path = directory.join(format!("{}.json", receipt.operation_id));
        let text = serde_json::to_string_pretty(receipt).map_err(|error| SaveReadError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        write_durable(&path, text.as_bytes())
    }
}

/// Replace one file with `bytes`, durably.
pub fn replace_durable_bytes(
    _source: &Path,
    target: &Path,
    bytes: &[u8],
) -> Result<(), SaveReadError> {
    let staged = target.with_extension("scroll-generator-rollback");
    write_durable(&staged, bytes)?;
    replace_durable(&staged, target)
}

/// Write a file and flush it before the caller may rename it.
pub fn write_durable(path: &Path, bytes: &[u8]) -> Result<(), SaveReadError> {
    use std::io::Write;
    let mut handle = fs::File::create(path).map_err(|error| SaveReadError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    handle.write_all(bytes).map_err(|error| SaveReadError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    handle.flush().map_err(|error| SaveReadError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    handle.sync_all().map_err(|error| SaveReadError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

/// Replace `target` with `staged` in one step.
pub fn replace_durable(staged: &Path, target: &Path) -> Result<(), SaveReadError> {
    fs::rename(staged, target).map_err(|error| SaveReadError::Io {
        path: target.display().to_string(),
        message: error.to_string(),
    })
}

fn entropy_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    format!("{:032x}", nanos ^ (pid << 64))
}

/// Whether a string is one operation id: exactly 32 lowercase hex characters.
pub fn is_operation_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether a string is a 64-character hex digest, either case.
pub fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Every outcome a recorded receipt may carry.
pub const VALID_OUTCOMES: [&str; 5] = [
    "pending",
    "committed",
    "uncertain",
    "not_committed",
    "discarded",
];

/// Validate one receipt as untrusted ledger input.
pub fn validate_receipt(receipt: &OperationReceipt) -> Result<(), SaveReadError> {
    if !is_operation_id(&receipt.operation_id) {
        return Err(SaveReadError::TamperedRecord {
            kind: "receipt",
            message: format!(
                "operation id {:?} is not 32 lowercase hex characters",
                receipt.operation_id
            ),
        });
    }
    if !VALID_OUTCOMES.contains(&receipt.outcome.as_str()) {
        return Err(SaveReadError::TamperedRecord {
            kind: "receipt",
            message: format!("outcome {:?} is not a known outcome", receipt.outcome),
        });
    }
    if let Some(digest) = receipt.installed_sha256.as_deref() {
        if !is_sha256(digest) {
            return Err(SaveReadError::TamperedRecord {
                kind: "receipt",
                message: "installed_sha256 is not a 64-character hex digest".to_string(),
            });
        }
    }
    if let Some(backup_id) = receipt.backup_id.as_deref() {
        if !is_safe_component(backup_id) {
            return Err(SaveReadError::TamperedRecord {
                kind: "receipt",
                message: "backup_id escapes the managed backups root".to_string(),
            });
        }
    }
    Ok(())
}

/// Whether a single path component stays inside the directory that holds it.
pub fn is_safe_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains(['/', '\\'])
        && !value.contains(':')
        && !value.contains('\0')
}

fn file_name(path: &Path) -> Result<String, SaveReadError> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .ok_or_else(|| SaveReadError::Io {
            path: path.display().to_string(),
            message: "the save path has no usable file name".to_string(),
        })
}

/// Whether the file at `path` holds exactly the bytes this commit wrote.
fn replaced_by_this_commit(path: &Path, owned_sha256: &str) -> bool {
    match fs::read(path) {
        Ok(bytes) => sha256_hex(&bytes).eq_ignore_ascii_case(owned_sha256),
        Err(_) => false,
    }
}

fn timestamp_label() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{nanos:020}")
}

/// One guarded stage's duration, printed only when the operator asks.
///
/// Off unless `NIOH3_SAVE_TIMING=1`. The acceptance gate reads these lines to
/// attribute a commit's cost to a stage instead of guessing.
fn timing(label: &str, started: std::time::Instant) {
    if std::env::var("NIOH3_SAVE_TIMING")
        .map(|value| value.trim() == "1")
        .unwrap_or(false)
    {
        eprintln!("save-timing\t{label}\t{}", started.elapsed().as_micros());
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::*;
    use std::sync::mpsc;

    /// A task-local fixture root that removes itself when the test ends.
    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(label: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            let root = std::env::temp_dir().join(format!(
                "nioh3-save-txn-{label}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_generation(root: &Path) -> PathBuf {
        let account = root.join("76561198000000000");
        let save = account.join("SAVEDATA00").join("SAVEDATA.BIN");
        fs::create_dir_all(save.parent().unwrap()).unwrap();
        fs::create_dir_all(account.join("SYSTEMSAVEDATA00")).unwrap();
        fs::write(&save, b"main-generation").unwrap();
        fs::write(save.parent().unwrap().join("BACKUP.BIN"), b"game-backup").unwrap();
        fs::write(
            account.join("SYSTEMSAVEDATA00").join("SAVEDATA.BIN"),
            b"system-save",
        )
        .unwrap();
        save
    }

    fn backup_path(save: &Path) -> PathBuf {
        save.parent().unwrap().join("BACKUP.BIN")
    }

    /// Start a writer that rewrites a related file once, after `delay`.
    fn start_writer(target: PathBuf, delay: Duration) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            fs::write(&target, b"the game rewrote this related file").unwrap();
        })
    }

    #[test]
    fn default_quiescence_window_matches_the_shipped_constant() {
        let root = TempRoot::new("default-window");
        let save = write_generation(&root.0);
        let host = SaveTransactionHost::new(&root.0);
        assert_eq!(
            host.quiescence_interval(),
            Duration::from_millis(SAVE_QUIESCENCE_MILLIS),
        );
        let started = std::time::Instant::now();
        host.require_quiescent(&save).unwrap();
        // The window is a real wait, not an instantaneous snapshot.
        assert!(
            started.elapsed() >= Duration::from_millis(SAVE_QUIESCENCE_MILLIS),
            "the guard returned in {:?}, faster than its own window",
            started.elapsed(),
        );
    }

    #[test]
    fn an_external_writer_inside_the_window_is_refused() {
        let root = TempRoot::new("writer-refused");
        let save = write_generation(&root.0);
        let writer = start_writer(backup_path(&save), Duration::from_millis(150));
        let host =
            SaveTransactionHost::new(&root.0).with_quiescence_interval(Duration::from_millis(600));
        let error = host.require_quiescent(&save).unwrap_err();
        assert!(
            matches!(&error, SaveReadError::SaveChanged { .. }),
            "a related file rewritten inside the window must be refused, got {error}",
        );
        writer.join().unwrap();
        // Only the related file moved; the write target is untouched.
        assert_eq!(fs::read(&save).unwrap().as_slice(), b"main-generation");
        assert_eq!(
            fs::read(backup_path(&save)).unwrap().as_slice(),
            b"the game rewrote this related file",
        );
    }

    #[test]
    fn the_window_is_what_catches_the_writer() {
        // Control for the case above: with the delay removed the same writer
        // lands after the two fingerprint passes, so the guard passes. That is
        // what makes the refusal above evidence of the timed window rather than
        // an artifact of the comparison alone.
        let root = TempRoot::new("writer-control");
        let save = write_generation(&root.0);
        let (started, started_rx) = mpsc::channel();
        let target = backup_path(&save);
        let writer = std::thread::spawn(move || {
            started.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(150));
            fs::write(&target, b"the game rewrote this related file").unwrap();
        });
        started_rx.recv().unwrap();
        let host = SaveTransactionHost::new(&root.0).without_quiescence_delay();
        assert!(host.require_quiescent(&save).is_ok());
        writer.join().unwrap();
    }
}
