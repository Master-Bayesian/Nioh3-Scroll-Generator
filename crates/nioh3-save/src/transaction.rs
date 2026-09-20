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
use std::sync::{Arc, Mutex};

use serde_json::Value;

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
    /// Authenticated identity of the selected restore source bundle.
    ///
    /// This is deliberately separate from the checkpoint created from the
    /// target generation during commit.
    #[serde(default)]
    pub restore_source: Option<crate::backup::RestoreSourceIdentity>,
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
    fail_receipt_create: AtomicU32,
    fail_receipt_write: AtomicU32,
    fail_receipt_flush: AtomicU32,
    fail_receipt_replace: AtomicU32,
    fail_restore_role_replace: AtomicU32,
    fail_restore_predispatch: AtomicU32,
}

/// One injectable commit stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    AfterCheckpoint,
    AfterReceipt,
    AfterStage,
    AfterReplace,
    AfterReadback,
    ReceiptCreate,
    ReceiptWrite,
    ReceiptFlush,
    ReceiptReplace,
    RestoreRoleReplace,
    RestorePredispatch,
}

impl FaultPoint {
    /// Every injectable stage, in commit order.
    pub const ALL: [Self; 11] = [
        Self::AfterCheckpoint,
        Self::AfterReceipt,
        Self::AfterStage,
        Self::AfterReplace,
        Self::AfterReadback,
        Self::ReceiptCreate,
        Self::ReceiptWrite,
        Self::ReceiptFlush,
        Self::ReceiptReplace,
        Self::RestoreRoleReplace,
        Self::RestorePredispatch,
    ];

    /// Stable label used by the fault gate's markers.
    pub const fn label(self) -> &'static str {
        match self {
            Self::AfterCheckpoint => "after-checkpoint",
            Self::AfterReceipt => "after-receipt",
            Self::AfterStage => "after-stage",
            Self::AfterReplace => "after-replace",
            Self::AfterReadback => "after-readback",
            Self::ReceiptCreate => "receipt-create",
            Self::ReceiptWrite => "receipt-write",
            Self::ReceiptFlush => "receipt-flush",
            Self::ReceiptReplace => "receipt-replace",
            Self::RestoreRoleReplace => "restore-role-replace",
            Self::RestorePredispatch => "restore-predispatch",
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
            FaultPoint::ReceiptCreate => &self.inner.fail_receipt_create,
            FaultPoint::ReceiptWrite => &self.inner.fail_receipt_write,
            FaultPoint::ReceiptFlush => &self.inner.fail_receipt_flush,
            FaultPoint::ReceiptReplace => &self.inner.fail_receipt_replace,
            FaultPoint::RestoreRoleReplace => &self.inner.fail_restore_role_replace,
            FaultPoint::RestorePredispatch => &self.inner.fail_restore_predispatch,
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
    /// `true` once the target bytes for this operation are durably committed,
    /// even when the terminal record itself could not be persisted. Absent on
    /// records written before this field existed, so a reader falls back to the
    /// outcome string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed: Option<bool>,
}

/// A guarded save-transaction host rooted at one state directory.
/// The bytes one commit will install, plus the identity of the decrypted
/// payload when the operation was a product transform.
struct PreparedBytes {
    container: Vec<u8>,
    plaintext_sha256: Option<String>,
}

/// The new checkpoint holding target generation B for rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RollbackCheckpointId(String);

/// One role in the rollback checkpoint, including a previously missing role.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RollbackRoleIdentity {
    role: SaveRole,
    target: PathBuf,
    existed: bool,
    sha256: String,
    checkpoint_file: Option<PathBuf>,
}

/// Typed rollback state, never interchangeable with the selected source A.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RestoreRollbackCheckpoint {
    id: RollbackCheckpointId,
    roles: Vec<RollbackRoleIdentity>,
}

struct StagedRestoreRole {
    role: SaveRole,
    target: PathBuf,
    staged: PathBuf,
}

struct RestoreJournalStatus<'a> {
    state: &'a str,
    installed_sha256: Option<&'a str>,
    committed: &'a [SaveRole],
    /// Roles whose replacement was dispatched but whose realized state is not
    /// yet recorded. A crash after the rename leaves the durable journal naming
    /// these roles, so a restart can classify them by re-reading the bytes.
    pending: &'a [SaveRole],
    rollback_errors: &'a [String],
}

/// The files a requested operation can mutate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteScope {
    MainOnly,
    AllRelated,
}

/// The shipped host has one write lane. Keep the final fence check and the
/// intent/target side effects in that same in-process responsibility domain.
static COMMIT_SERIALIZER: Mutex<()> = Mutex::new(());

/// One role of an unresolved operation, compared with the bytes on disk.
///
/// `state` is `A_source` when the target holds the generation this operation
/// installs, `B_checkpoint` when it still holds the pre-operation generation the
/// checkpoint recorded, `external_C` when it holds neither, `missing` when the
/// role has no file, and `unknown` when the operation did not record enough
/// identity to decide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetRoleState {
    pub role: String,
    pub target_path: String,
    pub state: String,
    pub current_sha256: Option<String>,
    pub source_sha256: Option<String>,
    pub checkpoint_sha256: Option<String>,
    pub replacement_started: bool,
}

/// An unresolved operation that fences one save from a new write plan.
///
/// `target_identified` is `false` when the operation's checkpoint bundle and
/// restore journal could not tie it to an account/slot or to a target path; such
/// an operation is treated as possibly owning the queried save, because an
/// unidentifiable claim must never be answered by allowing a second write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetFence {
    pub operation_id: String,
    pub kind: String,
    pub outcome: String,
    pub backup_id: Option<String>,
    pub account_id: Option<u64>,
    pub save_slot_index: Option<u8>,
    pub target_identified: bool,
    pub roles: Vec<TargetRoleState>,
}

impl TargetFence {
    /// The compact per-role classification carried in a refusal message.
    pub fn role_summary(&self) -> String {
        self.roles
            .iter()
            .map(|role| format!("{}={}", role.role, role.state))
            .collect::<Vec<String>>()
            .join(", ")
    }
}

/// Whether a recorded outcome still owns its target.
///
/// `pending` never reached a terminal state, and `uncertain` may have replaced
/// the target without proving a restore. Both fence a new plan; every other
/// outcome is terminal and names what happened to the bytes.
fn is_unresolved_outcome(outcome: &str) -> bool {
    matches!(outcome, "pending" | "uncertain")
}

/// The lowercase SHA-256 of one file, or `None` when it does not exist.
fn read_digest_if_present(path: &Path) -> Result<Option<String>, SaveReadError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(sha256_hex(&bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(SaveReadError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        }),
    }
}

fn path_identity(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn paths_overlap(left: &[PathBuf], right: &[PathBuf]) -> bool {
    let right = right
        .iter()
        .map(|path| path_identity(path))
        .collect::<Vec<_>>();
    left.iter()
        .map(|path| path_identity(path))
        .any(|path| right.contains(&path))
}

/// Classify one role by the bytes on disk against the operation's two generations.
fn classify_role_state(
    current: Option<&str>,
    source: Option<&str>,
    checkpoint: Option<&str>,
) -> &'static str {
    let Some(current) = current else {
        return "missing";
    };
    let equals = |digest: &str| digest.eq_ignore_ascii_case(current);
    if source.is_some_and(equals) {
        return "A_source";
    }
    if checkpoint.is_some_and(equals) {
        return "B_checkpoint";
    }
    if source.is_none() && checkpoint.is_none() {
        return "unknown";
    }
    "external_C"
}

/// A guarded save-transaction host rooted at one state directory.
pub struct SaveTransactionHost {
    state_root: PathBuf,
    backup_root: PathBuf,
    quiescence_interval: Duration,
    faults: TransactionFaults,
    crash_hook: CrashHook,
}

impl SaveTransactionHost {
    /// Create a host whose backups and journals live under `state_root`.
    pub fn new(state_root: &Path) -> Self {
        Self {
            state_root: state_root.to_path_buf(),
            backup_root: state_root.join("backups"),
            quiescence_interval: Duration::from_millis(SAVE_QUIESCENCE_MILLIS),
            faults: TransactionFaults::default(),
            crash_hook: CrashHook::None,
        }
    }

    /// Create a host with a caller-supplied fault set for the fault gate.
    pub fn with_faults(state_root: &Path, faults: TransactionFaults) -> Self {
        Self {
            state_root: state_root.to_path_buf(),
            backup_root: state_root.join("backups"),
            quiescence_interval: Duration::ZERO,
            faults,
            crash_hook: CrashHook::None,
        }
    }

    /// Arm a deterministic process cut for the crash harness.
    ///
    /// The shipped default is [`CrashHook::None`]; this is the only way to ask a
    /// host to terminate inside the restore loop instead of recovering.
    pub fn with_crash_hook(mut self, hook: CrashHook) -> Self {
        self.crash_hook = hook;
        self
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
        let names = self.receipt_operation_ids()?;
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

    fn receipt_operation_ids(&self) -> Result<Vec<String>, SaveReadError> {
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
        Ok(names)
    }

    /// Read the operation ledger for a write-safety decision.
    ///
    /// The user-facing list deliberately skips malformed records. A safety
    /// fence cannot reuse that policy: a valid operation filename whose bytes
    /// are unreadable or invalid is an unknown claim, so the next write must
    /// fail closed instead of treating it as absent.
    fn authoritative_receipts(&self) -> Result<Vec<OperationReceipt>, SaveReadError> {
        let names = self.receipt_operation_ids()?;
        let mut receipts = Vec::with_capacity(names.len());
        for name in names {
            let receipt = self.receipt(&name)?.ok_or_else(|| SaveReadError::Io {
                path: self
                    .receipt_dir()
                    .join(format!("{name}.json"))
                    .display()
                    .to_string(),
                message: "authoritative operation record disappeared while it was read".to_string(),
            })?;
            validate_receipt(&receipt)?;
            receipts.push(receipt);
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

    /// Every unresolved operation that may have written `save_path`.
    ///
    /// A `pending` or `uncertain` receipt means an earlier process claimed a
    /// write and never recorded a terminal outcome. The receipt alone does not
    /// name the save, so the operation's restore journal (when it has one) and
    /// its checkpoint bundle are re-read: they tie the operation to one
    /// account/slot and classify each role against the bytes on disk.
    pub fn unresolved_target_operations(
        &self,
        save_path: &Path,
    ) -> Result<Vec<TargetFence>, SaveReadError> {
        self.unresolved_target_operations_for_scope(save_path, WriteScope::MainOnly)
    }

    fn unresolved_target_operations_for_scope(
        &self,
        save_path: &Path,
        scope: WriteScope,
    ) -> Result<Vec<TargetFence>, SaveReadError> {
        let requested_paths = match scope {
            WriteScope::MainOnly => vec![save_path.to_path_buf()],
            WriteScope::AllRelated => related_save_paths(save_path)
                .into_iter()
                .map(|(_, path)| path)
                .collect(),
        };
        let mut fences = Vec::new();
        for receipt in self.authoritative_receipts()? {
            if !is_unresolved_outcome(&receipt.outcome) {
                continue;
            }
            if let Some(fence) = self.classify_unresolved_operation(&receipt, &requested_paths)? {
                fences.push(fence);
            }
        }
        Ok(fences)
    }

    /// The one unresolved operation that fences `save_path`, if any.
    pub fn unresolved_target_operation(
        &self,
        save_path: &Path,
    ) -> Result<Option<TargetFence>, SaveReadError> {
        Ok(self
            .unresolved_target_operations(save_path)?
            .into_iter()
            .next())
    }

    fn unresolved_target_operation_for_scope(
        &self,
        save_path: &Path,
        scope: WriteScope,
    ) -> Result<Option<TargetFence>, SaveReadError> {
        Ok(self
            .unresolved_target_operations_for_scope(save_path, scope)?
            .into_iter()
            .next())
    }

    /// Classify one unresolved receipt against the queried save.
    ///
    /// `None` means the operation is provably about a different target, so it
    /// cannot fence this save. An operation whose target cannot be identified at
    /// all is returned with `target_identified = false`, because an unknown
    /// claim must never be answered by allowing a second write.
    fn classify_unresolved_operation(
        &self,
        receipt: &OperationReceipt,
        requested_paths: &[PathBuf],
    ) -> Result<Option<TargetFence>, SaveReadError> {
        let journal = self.restore_journal_for_operation(&receipt.operation_id)?;
        let receipt_manifest = match receipt.backup_id.as_deref() {
            Some(backup_id) if is_safe_component(backup_id) => {
                self.read_manifest_if_present(backup_id)?
            }
            _ => None,
        };
        let mut roles: Vec<TargetRoleState> = Vec::new();
        let mut operation_account = None;
        let mut operation_slot = None;
        let mut write_paths: Vec<PathBuf> = Vec::new();

        if let Some(journal) = journal.as_ref() {
            operation_account = journal.get("steam_account_id").and_then(Value::as_u64);
            operation_slot = journal
                .get("save_slot_index")
                .and_then(Value::as_u64)
                .map(|value| value as u8);
            for entry in journal
                .get("role_results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(target_path) = entry.get("target_path").and_then(Value::as_str) else {
                    continue;
                };
                write_paths.push(PathBuf::from(target_path));
                roles.push(TargetRoleState {
                    role: entry
                        .get("role")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string(),
                    target_path: target_path.to_string(),
                    state: String::new(),
                    current_sha256: None,
                    source_sha256: entry
                        .get("source_sha256")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    checkpoint_sha256: entry
                        .get("target_before_sha256")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    replacement_started: entry
                        .get("replacement_started")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                });
            }
        } else if receipt.kind == PlanKind::Restore.label() {
            // A restore receipt's `backup_id` is source A, never checkpoint B.
            // If the journal is unavailable, join A to the separately discovered
            // pre-restore checkpoint rather than relabelling A as the target's
            // former generation.
            let checkpoint =
                self.restore_checkpoint_manifest_for_operation(&receipt.operation_id)?;
            if let Some(checkpoint) = checkpoint.as_ref() {
                operation_account = Some(checkpoint.steam_account_id);
                operation_slot = Some(checkpoint.save_slot_index);
                for file in &checkpoint.backup_files {
                    if file.source_path.is_empty() {
                        continue;
                    }
                    let source_sha256 = receipt_manifest.as_ref().and_then(|source| {
                        source
                            .backup_files
                            .iter()
                            .find(|entry| entry.source_role == file.source_role)
                            .map(|entry| entry.sha256.to_ascii_lowercase())
                    });
                    write_paths.push(PathBuf::from(&file.source_path));
                    roles.push(TargetRoleState {
                        role: file.source_role.clone(),
                        target_path: file.source_path.clone(),
                        state: String::new(),
                        current_sha256: None,
                        source_sha256,
                        checkpoint_sha256: Some(file.sha256.to_ascii_lowercase()),
                        replacement_started: false,
                    });
                }
            }
        } else if let Some(manifest) = receipt_manifest.as_ref() {
            operation_account = Some(manifest.steam_account_id);
            operation_slot = Some(manifest.save_slot_index);
            for file in &manifest.backup_files {
                if file.source_path.is_empty() {
                    continue;
                }
                let is_main = file.source_role == SaveRole::Main.label();
                if is_main {
                    write_paths.push(PathBuf::from(&file.source_path));
                }
                roles.push(TargetRoleState {
                    role: file.source_role.clone(),
                    target_path: file.source_path.clone(),
                    state: String::new(),
                    current_sha256: None,
                    // A write installs only the main save, so only that role has
                    // a source generation distinct from its checkpoint.
                    source_sha256: is_main.then(|| receipt.installed_sha256.clone()).flatten(),
                    checkpoint_sha256: Some(file.sha256.to_ascii_lowercase()),
                    replacement_started: false,
                });
            }
        }

        let matches_target = paths_overlap(&write_paths, requested_paths);
        let identified =
            operation_account.is_some() && operation_slot.is_some() && !write_paths.is_empty();
        if identified && !matches_target {
            return Ok(None);
        }

        for role in roles.iter_mut() {
            let current = read_digest_if_present(Path::new(&role.target_path))?;
            role.state = classify_role_state(
                current.as_deref(),
                role.source_sha256.as_deref(),
                role.checkpoint_sha256.as_deref(),
            )
            .to_string();
            role.current_sha256 = current;
        }
        Ok(Some(TargetFence {
            operation_id: receipt.operation_id.clone(),
            kind: receipt.kind.clone(),
            outcome: receipt.outcome.clone(),
            backup_id: receipt.backup_id.clone(),
            account_id: operation_account,
            save_slot_index: operation_slot,
            target_identified: identified,
            roles,
        }))
    }

    /// The restore journal one unresolved operation wrote, if it wrote one.
    ///
    /// The checkpoint directory is named `<timestamp>-<operation_id>`, so the
    /// bundle root is scanned once and each journal must name this operation
    /// before it is trusted.
    fn restore_journal_for_operation(
        &self,
        operation_id: &str,
    ) -> Result<Option<Value>, SaveReadError> {
        let root = &self.backup_root;
        if !root.is_dir() {
            return Ok(None);
        }
        let entries = fs::read_dir(root).map_err(|error| SaveReadError::Io {
            path: root.display().to_string(),
            message: error.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| SaveReadError::Io {
                path: root.display().to_string(),
                message: error.to_string(),
            })?;
            let path = entry.path().join("restore-journal.json");
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(journal) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if journal.get("operation_id").and_then(Value::as_str) == Some(operation_id) {
                return Ok(Some(journal));
            }
        }
        Ok(None)
    }

    /// Read one checkpoint bundle's manifest when it is present and readable.
    fn read_manifest_if_present(
        &self,
        backup_id: &str,
    ) -> Result<Option<crate::backup::BackupManifest>, SaveReadError> {
        let path = self.backup_dir(backup_id).join("backup-manifest.json");
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Ok(None),
        };
        Ok(serde_json::from_str::<crate::backup::BackupManifest>(&text).ok())
    }

    /// Find the rollback checkpoint B for one restore operation.
    fn restore_checkpoint_manifest_for_operation(
        &self,
        operation_id: &str,
    ) -> Result<Option<crate::backup::BackupManifest>, SaveReadError> {
        if !self.backup_root.is_dir() {
            return Ok(None);
        }
        let suffix = format!("-{operation_id}");
        let mut found = None;
        for entry in fs::read_dir(&self.backup_root).map_err(|error| SaveReadError::Io {
            path: self.backup_root.display().to_string(),
            message: error.to_string(),
        })? {
            let entry = entry.map_err(|error| SaveReadError::Io {
                path: self.backup_root.display().to_string(),
                message: error.to_string(),
            })?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(&suffix) {
                continue;
            }
            let path = entry.path().join("backup-manifest.json");
            let text = fs::read_to_string(&path).map_err(|error| SaveReadError::Io {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
            let manifest: crate::backup::BackupManifest =
                serde_json::from_str(&text).map_err(|error| SaveReadError::TamperedRecord {
                    kind: "restore checkpoint manifest",
                    message: error.to_string(),
                })?;
            if manifest.operation_id != operation_id || manifest.action != "pre-restore-checkpoint"
            {
                continue;
            }
            if found.is_some() {
                return Err(SaveReadError::TamperedRecord {
                    kind: "restore checkpoint manifest",
                    message: format!(
                        "operation {operation_id} has more than one rollback checkpoint"
                    ),
                });
            }
            found = Some(manifest);
        }
        Ok(found)
    }

    /// Refuse a new plan while the same save owns an unresolved operation.
    fn require_no_unresolved_operation(
        &self,
        save_path: &Path,
        scope: WriteScope,
    ) -> Result<(), SaveReadError> {
        let Some(fence) = self.unresolved_target_operation_for_scope(save_path, scope)? else {
            return Ok(());
        };
        let detail = if fence.target_identified {
            format!("roles: {}", fence.role_summary())
        } else {
            format!(
                "its target could not be identified, so it is treated as possibly owning this \
                 save; roles: {}",
                fence.role_summary()
            )
        };
        Err(SaveReadError::UnresolvedTargetOperation {
            operation_id: fence.operation_id,
            outcome: fence.outcome,
            detail,
        })
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
        // A restarted process must not stack a second write on a save whose
        // earlier operation never reached a terminal outcome.
        let scope = match &command {
            PlanCommand::RestoreFromBackup { .. } => WriteScope::AllRelated,
            PlanCommand::WriteMain { .. } => WriteScope::MainOnly,
        };
        self.require_no_unresolved_operation(save_path, scope)?;
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
        let restore_source = match &command {
            PlanCommand::RestoreFromBackup { backup_id } => Some(
                crate::backup::authenticate_restore_source(
                    &self.backup_root,
                    backup_id,
                    save_path,
                )?
                .identity,
            ),
            PlanCommand::WriteMain { .. } => None,
        };
        let (account_id, save_slot) = restore_source
            .as_ref()
            .map(|source| {
                (
                    source.account_id.to_string(),
                    source.save_slot_index.to_string(),
                )
            })
            .unwrap_or_default();
        Ok(SavePlan {
            plan_id: entropy_id(),
            backup_id: match &command {
                PlanCommand::RestoreFromBackup { backup_id } => backup_id.clone(),
                PlanCommand::WriteMain { .. } => String::new(),
            },
            kind,
            save_path: save_path.to_path_buf(),
            source_sha256: main.sha256.clone(),
            account_id,
            save_slot,
            product: None,
            restore_source,
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
        // A restarted process must not stack a second write on a save whose
        // earlier operation never reached a terminal outcome.
        self.require_no_unresolved_operation(save_path, WriteScope::MainOnly)?;
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
            restore_source: None,
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
            committed: None,
        }
    }

    /// Commit a plan: quiescence, backup, durable replace, readback, receipt.
    pub fn commit(&self, plan: &SavePlan) -> Result<OperationReceipt, SaveReadError> {
        let _commit_guard = COMMIT_SERIALIZER.lock().map_err(|_| SaveReadError::Io {
            path: self.state_root.display().to_string(),
            message: "the save commit serializer is poisoned".to_string(),
        })?;
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
        let scope = match &plan.command {
            PlanCommand::RestoreFromBackup { .. } => WriteScope::AllRelated,
            PlanCommand::WriteMain { .. } => WriteScope::MainOnly,
        };
        // Plans can outlive the process that prepared them. Recheck the
        // authoritative ledger at the final side-effect boundary while the
        // process-wide write lane is held, before checkpoint, intent or target
        // bytes are written.
        self.require_no_unresolved_operation(&plan.save_path, scope)?;
        if plan.kind == PlanKind::Restore {
            return self.commit_restore(plan);
        }
        // The generation must not have moved since preparation.
        let stage = std::time::Instant::now();
        self.require_generation_unchanged(&plan.save_path, &plan.baseline)?;
        timing("quiescent-baseline", stage);
        let stage = std::time::Instant::now();
        let backup_id = self.write_backup(&plan.plan_id, plan.kind.label(), &plan.baseline)?;
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
            committed: None,
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
                // The bytes on disk are now the owning generation: the business
                // commit has already happened. Persisting the terminal record is
                // a separate concern, so a receipt failure must never be turned
                // into a replayable not-committed result.
                let receipt = OperationReceipt {
                    operation_id: plan.plan_id.clone(),
                    kind: plan.kind.label().to_string(),
                    outcome: "committed".to_string(),
                    installed_sha256: Some(installed),
                    backup_id: Some(backup_id),
                    message: None,
                    committed: Some(true),
                };
                match self.write_receipt(&receipt) {
                    Ok(()) => Ok(receipt),
                    Err(write_error) => {
                        Err(self.commit_record_failure(plan, &receipt, write_error))
                    }
                }
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
                        committed: None,
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
                // The journal must not claim a success the commit never
                // reached. The shipped product separates a clean rollback from
                // one that could not be proven, so a later reader can tell them
                // apart; a failure to record it never masks the real error.
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
                    committed: Some(false),
                };
                self.write_receipt(&receipt)?;
                if replaced {
                    Err(SaveReadError::CommitUncertain {
                        message: receipt.message.clone().unwrap_or_default(),
                    })
                } else {
                    Err(error)
                }
            }
        }
    }

    /// Build the error for a target commit whose terminal record did not land.
    ///
    /// The bytes are already committed, so this is never a retryable
    /// not-committed result: the plan is consumed, the same operation id must
    /// not be written again, and the caller is told the commit landed while the
    /// record did not.
    fn commit_record_failure(
        &self,
        plan: &SavePlan,
        receipt: &OperationReceipt,
        write_error: SaveReadError,
    ) -> SaveReadError {
        // The target bytes are already committed, so this is never a retryable
        // not-committed result: the plan is consumed and the caller is told the
        // write landed while its record did not.
        //
        // Every receipt stage leaves the old, valid intent untouched, so the
        // realized outcome is still recoverable while that record is this
        // operation's own pending intent: rewrite the terminal record over the
        // stale intent and a later reader sees the authoritative committed
        // state. The failed stage already consumed its one arming and is never
        // re-consulted here, so this rewrite can only land the authoritative
        // outcome; it can never let a retry write the target a second time. A
        // genuine I/O error simply fails again, which still leaves the old
        // pending intent as the durable record.
        let recoverable = self
            .receipt(&plan.plan_id)
            .ok()
            .flatten()
            .map(|existing| existing.outcome == "pending")
            .unwrap_or(true);
        let recovered = recoverable && self.write_receipt(receipt).is_ok();
        self.consume_committed_plan(plan);
        let warning = if recovered {
            write_error.to_string()
        } else {
            format!(
                "{write_error}; the pending intent is still the only durable record for this operation"
            )
        };
        SaveReadError::CommitCompletedWithWarning {
            operation_id: plan.plan_id.clone(),
            warning,
        }
    }

    /// Remove a committed operation's stored plan so it cannot be replayed.
    fn consume_committed_plan(&self, plan: &SavePlan) {
        let _ = fs::remove_file(self.plan_dir().join(format!("{}.json", plan.plan_id)));
    }

    /// Commit one authenticated three-role restore transaction.
    fn commit_restore(&self, plan: &SavePlan) -> Result<OperationReceipt, SaveReadError> {
        let source_identity =
            plan.restore_source
                .as_ref()
                .ok_or_else(|| SaveReadError::TamperedRecord {
                    kind: "restore plan",
                    message: "the plan has no authenticated restore source".to_string(),
                })?;
        self.require_generation_unchanged(&plan.save_path, &plan.baseline)?;
        // Re-read and authenticate the complete source immediately before any
        // checkpoint or target mutation. The returned bytes are the only bytes
        // used by this commit, closing a prepare/commit source-swap race.
        let source = crate::backup::reauthenticate_restore_source(
            &self.backup_root,
            source_identity,
            &plan.save_path,
        )?;
        let checkpoint = self.write_restore_checkpoint(plan, &source.identity)?;
        if self.faults.should_fire(FaultPoint::AfterCheckpoint) {
            let error = SaveReadError::InjectedFault {
                stage: FaultPoint::AfterCheckpoint.label().to_string(),
            };
            let _ = self.record_restore_failure(
                &checkpoint,
                plan,
                &source.identity,
                &error.to_string(),
                RestoreJournalStatus {
                    state: "rolled_back",
                    installed_sha256: None,
                    committed: &[],
                    pending: &[],
                    rollback_errors: &[],
                },
            );
            return Err(error);
        }

        let main_source = source
            .files
            .iter()
            .find(|file| file.identity.role == SaveRole::Main)
            .ok_or_else(|| SaveReadError::TamperedRecord {
                kind: "restore source",
                message: "the authenticated source has no main role".to_string(),
            })?;
        self.write_receipt(&OperationReceipt {
            operation_id: plan.plan_id.clone(),
            kind: plan.kind.label().to_string(),
            outcome: "pending".to_string(),
            installed_sha256: Some(main_source.identity.sha256.clone()),
            backup_id: Some(source.identity.backup_id.clone()),
            message: None,
            committed: None,
        })?;
        if self.faults.should_fire(FaultPoint::AfterReceipt) {
            let error = SaveReadError::InjectedFault {
                stage: FaultPoint::AfterReceipt.label().to_string(),
            };
            return self.finish_restore_failure(plan, &source.identity, &checkpoint, &[], error);
        }

        let staged = match self.stage_restore_roles(plan, &source) {
            Ok(staged) => staged,
            Err(error) => {
                return self.finish_restore_failure(plan, &source.identity, &checkpoint, &[], error)
            }
        };
        if self.faults.should_fire(FaultPoint::AfterStage) {
            cleanup_staged_restore(&staged);
            let error = SaveReadError::InjectedFault {
                stage: FaultPoint::AfterStage.label().to_string(),
            };
            return self.finish_restore_failure(plan, &source.identity, &checkpoint, &[], error);
        }

        let mut committed = Vec::with_capacity(staged.len());
        for staged_role in &staged {
            if let Err(error) =
                self.require_restore_owned_generation(plan, &source.identity, &committed)
            {
                cleanup_staged_restore(&staged);
                return self.finish_restore_failure(
                    plan,
                    &source.identity,
                    &checkpoint,
                    &committed,
                    error,
                );
            }
            let pending = [staged_role.role];
            // Arm the durable role marker before the rename so a crash between
            // the dispatch and the completion record is discoverable on restart.
            if let Err(error) = self.record_restore_progress(
                &checkpoint,
                plan,
                &source.identity,
                &committed,
                &pending,
            ) {
                cleanup_staged_restore(&staged);
                return self.finish_restore_failure(
                    plan,
                    &source.identity,
                    &checkpoint,
                    &committed,
                    error,
                );
            }
            if self.faults.should_fire(FaultPoint::RestoreRoleReplace) {
                // The marker is durable and the replacement is about to run, so
                // a crash-cut here must be refused by the child harness rather
                // than allowed an ordinary in-process rollback.
                cleanup_staged_restore(&staged);
                return Err(SaveReadError::InjectedFault {
                    stage: format!(
                        "{}:{}",
                        FaultPoint::RestoreRoleReplace.label(),
                        staged_role.role.label()
                    ),
                });
            }
            // A deterministic crash cut, armed only by a test harness. It runs
            // after the durable intent for this role and before the rename, so
            // the journal names the role while its bytes are still the old
            // generation. There is no shipped behavior here: the default hook
            // returns immediately.
            if self.crash_hook == CrashHook::Exit(RestoreCut::BeforeReplace(staged_role.role)) {
                cleanup_staged_restore(&staged);
                crash_now(&RestoreCut::BeforeReplace(staged_role.role));
            }
            if let Err(error) = replace_durable(&staged_role.staged, &staged_role.target) {
                cleanup_staged_restore(&staged);
                return self.finish_restore_failure(
                    plan,
                    &source.identity,
                    &checkpoint,
                    &committed,
                    error,
                );
            }
            // The rename landed but the completion record has not been written
            // yet. A cut here leaves one role at A and the rest at B, which is
            // exactly the mix a restart must classify per role.
            if self.crash_hook == CrashHook::Exit(RestoreCut::AfterReplace(staged_role.role)) {
                crash_now(&RestoreCut::AfterReplace(staged_role.role));
            }
            committed.push(staged_role.role);
            if let Err(error) =
                self.record_restore_progress(&checkpoint, plan, &source.identity, &committed, &[])
            {
                cleanup_staged_restore(&staged);
                return self.finish_restore_failure(
                    plan,
                    &source.identity,
                    &checkpoint,
                    &committed,
                    error,
                );
            }
            if let Err(error) =
                self.require_restore_owned_generation(plan, &source.identity, &committed)
            {
                cleanup_staged_restore(&staged);
                return self.finish_restore_failure(
                    plan,
                    &source.identity,
                    &checkpoint,
                    &committed,
                    error,
                );
            }
        }
        cleanup_staged_restore(&staged);

        if self.faults.should_fire(FaultPoint::AfterReplace) {
            let error = SaveReadError::InjectedFault {
                stage: FaultPoint::AfterReplace.label().to_string(),
            };
            return self.finish_restore_failure(
                plan,
                &source.identity,
                &checkpoint,
                &committed,
                error,
            );
        }
        self.require_restore_owned_generation(plan, &source.identity, &committed)?;
        if self.faults.should_fire(FaultPoint::AfterReadback) {
            let error = SaveReadError::InjectedFault {
                stage: FaultPoint::AfterReadback.label().to_string(),
            };
            return self.finish_restore_failure(
                plan,
                &source.identity,
                &checkpoint,
                &committed,
                error,
            );
        }

        let message = self.complete_restore_checkpoint(
            &checkpoint,
            plan,
            &source.identity,
            &main_source.identity.sha256,
        );
        let receipt = OperationReceipt {
            operation_id: plan.plan_id.clone(),
            kind: plan.kind.label().to_string(),
            outcome: "committed".to_string(),
            installed_sha256: Some(main_source.identity.sha256.clone()),
            backup_id: Some(source.identity.backup_id.clone()),
            message,
            committed: Some(true),
        };
        match self.write_receipt(&receipt) {
            Ok(()) => Ok(receipt),
            Err(write_error) => Err(self.commit_record_failure(plan, &receipt, write_error)),
        }
    }

    fn stage_restore_roles(
        &self,
        plan: &SavePlan,
        source: &crate::backup::AuthenticatedRestoreSource,
    ) -> Result<Vec<StagedRestoreRole>, SaveReadError> {
        let targets = related_save_paths(&plan.save_path);
        let mut staged = Vec::with_capacity(3);
        for role in [SaveRole::System, SaveRole::GameBackup, SaveRole::Main] {
            let source_file = source
                .files
                .iter()
                .find(|file| file.identity.role == role)
                .ok_or_else(|| SaveReadError::TamperedRecord {
                    kind: "restore source",
                    message: format!("the authenticated source has no {} role", role.label()),
                })?;
            let target = targets
                .iter()
                .find(|(target_role, _)| *target_role == role)
                .map(|(_, path)| path.clone())
                .ok_or_else(|| SaveReadError::SaveChanged {
                    path: plan.save_path.display().to_string(),
                })?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| SaveReadError::Io {
                    path: parent.display().to_string(),
                    message: error.to_string(),
                })?;
            }
            let name = file_name(&target)?;
            let staged_path = target.with_file_name(format!(
                "{name}.scroll-generator-restore-{}.tmp",
                plan.plan_id
            ));
            if staged_path.exists() {
                let _ = fs::remove_file(&staged_path);
            }
            if let Err(error) = write_durable(&staged_path, &source_file.bytes) {
                cleanup_staged_restore(&staged);
                return Err(error);
            }
            let staged_bytes = fs::read(&staged_path).map_err(|error| SaveReadError::Io {
                path: staged_path.display().to_string(),
                message: error.to_string(),
            })?;
            let staged_sha256 = sha256_hex(&staged_bytes);
            if staged_sha256 != source_file.identity.sha256 {
                let _ = fs::remove_file(&staged_path);
                cleanup_staged_restore(&staged);
                return Err(SaveReadError::IntegrityMismatch {
                    path: staged_path.display().to_string(),
                    expected: source_file.identity.sha256.clone(),
                    actual: staged_sha256,
                });
            }
            staged.push(StagedRestoreRole {
                role,
                target,
                staged: staged_path,
            });
        }
        Ok(staged)
    }

    fn finish_restore_failure(
        &self,
        plan: &SavePlan,
        source: &crate::backup::RestoreSourceIdentity,
        checkpoint: &RestoreRollbackCheckpoint,
        committed: &[SaveRole],
        error: SaveReadError,
    ) -> Result<OperationReceipt, SaveReadError> {
        let rollback = if committed.is_empty() {
            Ok(())
        } else {
            self.rollback_restore_roles(plan, source, checkpoint, committed)
        };
        let (journal_state, outcome, rollback_errors) = match &rollback {
            Ok(()) => ("rolled_back", "not_committed", Vec::new()),
            Err(rollback_error) => (
                "recovery_required",
                "uncertain",
                vec![rollback_error.to_string()],
            ),
        };
        let _ = self.record_restore_failure(
            checkpoint,
            plan,
            source,
            &error.to_string(),
            RestoreJournalStatus {
                state: journal_state,
                installed_sha256: None,
                committed,
                pending: &[],
                rollback_errors: &rollback_errors,
            },
        );
        let message = match rollback {
            Ok(()) => format!("{error}; pre-restore checkpoint restored"),
            Err(rollback_error) => format!("{error}; rollback also failed: {rollback_error}"),
        };
        let receipt = OperationReceipt {
            operation_id: plan.plan_id.clone(),
            kind: plan.kind.label().to_string(),
            outcome: outcome.to_string(),
            installed_sha256: None,
            backup_id: Some(source.backup_id.clone()),
            message: Some(message.clone()),
            committed: Some(false),
        };
        self.write_receipt(&receipt)?;
        if outcome == "uncertain" {
            Err(SaveReadError::CommitUncertain { message })
        } else {
            Err(error)
        }
    }

    fn require_restore_owned_generation(
        &self,
        plan: &SavePlan,
        source: &crate::backup::RestoreSourceIdentity,
        committed: &[SaveRole],
    ) -> Result<(), SaveReadError> {
        let current = capture_related_fingerprints(&plan.save_path)?;
        for observed in current {
            if committed.contains(&observed.role) {
                let expected = source
                    .files
                    .iter()
                    .find(|file| file.role == observed.role)
                    .ok_or_else(|| SaveReadError::TamperedRecord {
                        kind: "restore source",
                        message: format!("missing role {}", observed.role.label()),
                    })?;
                if !observed.exists
                    || observed.length != expected.size
                    || observed.sha256 != expected.sha256
                {
                    return Err(SaveReadError::SaveChanged {
                        path: observed.path.display().to_string(),
                    });
                }
            } else {
                let expected = plan
                    .baseline
                    .iter()
                    .find(|entry| entry.role == observed.role)
                    .ok_or_else(|| SaveReadError::TamperedRecord {
                        kind: "restore plan",
                        message: format!("baseline missing role {}", observed.role.label()),
                    })?;
                if observed.exists != expected.exists
                    || observed.length != expected.length
                    || observed.sha256 != expected.sha256
                {
                    return Err(SaveReadError::SaveChanged {
                        path: observed.path.display().to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    fn rollback_restore_roles(
        &self,
        plan: &SavePlan,
        source: &crate::backup::RestoreSourceIdentity,
        checkpoint: &RestoreRollbackCheckpoint,
        committed: &[SaveRole],
    ) -> Result<(), SaveReadError> {
        let mut remaining = committed.to_vec();
        self.require_restore_owned_generation(plan, source, &remaining)?;
        for role in committed.iter().rev() {
            let saved = checkpoint
                .roles
                .iter()
                .find(|entry| entry.role == *role)
                .ok_or_else(|| SaveReadError::TamperedRecord {
                    kind: "restore checkpoint",
                    message: format!("checkpoint missing role {}", role.label()),
                })?;
            if saved.existed {
                let checkpoint_file = saved.checkpoint_file.as_ref().ok_or_else(|| {
                    SaveReadError::TamperedRecord {
                        kind: "restore checkpoint",
                        message: format!("checkpoint has no bytes for role {}", role.label()),
                    }
                })?;
                let bytes = fs::read(checkpoint_file).map_err(|error| SaveReadError::Io {
                    path: checkpoint_file.display().to_string(),
                    message: error.to_string(),
                })?;
                let digest = sha256_hex(&bytes);
                if digest != saved.sha256 {
                    return Err(SaveReadError::IntegrityMismatch {
                        path: checkpoint_file.display().to_string(),
                        expected: saved.sha256.clone(),
                        actual: digest,
                    });
                }
                replace_durable_bytes(checkpoint_file, &saved.target, &bytes)?;
            } else if saved.target.exists() {
                fs::remove_file(&saved.target).map_err(|error| SaveReadError::Io {
                    path: saved.target.display().to_string(),
                    message: error.to_string(),
                })?;
            }
            remaining.retain(|remaining_role| remaining_role != role);
            self.require_restore_owned_generation(plan, source, &remaining)?;
        }
        Ok(())
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
        match plan.kind {
            PlanKind::Restore => {
                if plan.product.is_some() {
                    return Err(SaveReadError::TamperedRecord {
                        kind: "restore plan",
                        message: "a restore plan cannot carry a product transform".to_string(),
                    });
                }
                let source =
                    plan.restore_source
                        .as_ref()
                        .ok_or_else(|| SaveReadError::TamperedRecord {
                            kind: "restore plan",
                            message: "the restore source identity is missing".to_string(),
                        })?;
                let command_backup_id = match &plan.command {
                    PlanCommand::RestoreFromBackup { backup_id } => backup_id,
                    PlanCommand::WriteMain { .. } => {
                        return Err(SaveReadError::TamperedRecord {
                            kind: "restore plan",
                            message: "a restore plan has a write-main command".to_string(),
                        })
                    }
                };
                if plan.backup_id != source.backup_id
                    || plan.backup_id != *command_backup_id
                    || source.files.len() != 3
                {
                    return Err(SaveReadError::TamperedRecord {
                        kind: "restore plan",
                        message: "the selected source identities disagree".to_string(),
                    });
                }
                for role in [SaveRole::Main, SaveRole::GameBackup, SaveRole::System] {
                    if source.files.iter().filter(|file| file.role == role).count() != 1 {
                        return Err(SaveReadError::TamperedRecord {
                            kind: "restore plan",
                            message: format!(
                                "the source identity does not contain exactly one {} role",
                                role.label()
                            ),
                        });
                    }
                }
            }
            _ => {
                if plan.restore_source.is_some() || !plan.backup_id.is_empty() {
                    return Err(SaveReadError::TamperedRecord {
                        kind: "plan",
                        message: "a non-restore plan carries restore-only identity".to_string(),
                    });
                }
            }
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
        if plan.product.is_some() || plan.kind == PlanKind::Restore {
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

    /// Record the pre-restore generation as the shipped automatic checkpoint.
    ///
    /// Mirrors `savegame.py`'s restore path: a new bundle holding the bytes this
    /// commit is about to replace, plus the journal that names the selected
    /// backup as its source. The bundle's `action` is the shipped marker the UI
    /// reads to present a restore as undoable.
    fn write_restore_checkpoint(
        &self,
        plan: &SavePlan,
        source: &crate::backup::RestoreSourceIdentity,
    ) -> Result<RestoreRollbackCheckpoint, SaveReadError> {
        let checkpoint_id = RollbackCheckpointId(self.write_backup(
            &plan.plan_id,
            "pre-restore-checkpoint",
            &plan.baseline,
        )?);
        let checkpoint_directory = self.backup_dir(&checkpoint_id.0);
        let roles = plan
            .baseline
            .iter()
            .map(|entry| RollbackRoleIdentity {
                role: entry.role,
                target: entry.path.clone(),
                existed: entry.exists,
                sha256: entry.sha256.clone(),
                checkpoint_file: entry.exists.then(|| {
                    checkpoint_directory.join(crate::backup::role_backup_file(entry.role))
                }),
            })
            .collect();
        let checkpoint = RestoreRollbackCheckpoint {
            id: checkpoint_id,
            roles,
        };
        self.write_restore_journal_value(
            &checkpoint.id,
            &self.restore_journal(
                &checkpoint,
                plan,
                source,
                RestoreJournalStatus {
                    state: "prepared",
                    installed_sha256: None,
                    committed: &[],
                    pending: &[],
                    rollback_errors: &[],
                },
            ),
        )?;
        Ok(checkpoint)
    }

    /// Mark a restore checkpoint committed.
    ///
    /// The save bytes are already restored by the time this runs, so a
    /// diagnostics failure is reported as a warning on a committed receipt
    /// instead of failing an operation that really did land.
    fn complete_restore_checkpoint(
        &self,
        checkpoint: &RestoreRollbackCheckpoint,
        plan: &SavePlan,
        source: &crate::backup::RestoreSourceIdentity,
        installed_sha256: &str,
    ) -> Option<String> {
        let committed = [SaveRole::Main, SaveRole::GameBackup, SaveRole::System];
        let journal = self.restore_journal(
            checkpoint,
            plan,
            source,
            RestoreJournalStatus {
                state: "committed",
                installed_sha256: Some(installed_sha256),
                committed: &committed,
                pending: &[],
                rollback_errors: &[],
            },
        );
        match self.write_restore_journal_value(&checkpoint.id, &journal) {
            Ok(()) => None,
            Err(error) => Some(format!(
                "the save was restored, but its restore journal could not be persisted: {error}"
            )),
        }
    }

    /// Record why a restore did not commit, without masking that reason.
    fn record_restore_failure(
        &self,
        checkpoint: &RestoreRollbackCheckpoint,
        plan: &SavePlan,
        source: &crate::backup::RestoreSourceIdentity,
        error: &str,
        status: RestoreJournalStatus<'_>,
    ) -> Result<(), SaveReadError> {
        let mut journal = self.restore_journal(checkpoint, plan, source, status);
        journal["error"] = serde_json::Value::String(error.to_string());
        self.write_restore_journal_value(&checkpoint.id, &journal)
    }

    /// Persist which role's replacement is about to be dispatched.
    ///
    /// This is written before the rename so a crash between the dispatch and the
    /// completion record leaves a durable marker. A restart can then re-read the
    /// target and distinguish source A, checkpoint B and an external C; an
    /// unrecorded role would otherwise look untouched. Only the failing role is
    /// named here, so the reader never assumes a multi-file atomic replace.
    fn record_restore_progress(
        &self,
        checkpoint: &RestoreRollbackCheckpoint,
        plan: &SavePlan,
        source: &crate::backup::RestoreSourceIdentity,
        committed: &[SaveRole],
        pending: &[SaveRole],
    ) -> Result<(), SaveReadError> {
        let journal = self.restore_journal(
            checkpoint,
            plan,
            source,
            RestoreJournalStatus {
                state: "roles_in_progress",
                installed_sha256: None,
                committed,
                pending,
                rollback_errors: &[],
            },
        );
        self.write_restore_journal_value(&checkpoint.id, &journal)
    }

    /// The journal the shipped product writes beside a restore checkpoint.
    fn restore_journal(
        &self,
        checkpoint: &RestoreRollbackCheckpoint,
        plan: &SavePlan,
        source: &crate::backup::RestoreSourceIdentity,
        status: RestoreJournalStatus<'_>,
    ) -> serde_json::Value {
        let role_results: Vec<serde_json::Value> = checkpoint
            .roles
            .iter()
            .map(|entry| {
                let source_file = source.files.iter().find(|file| file.role == entry.role);
                let replaced = status.committed.contains(&entry.role);
                let replace_pending = status.pending.contains(&entry.role);
                let final_state = if status.state == "committed" {
                    "source_installed"
                } else if replaced {
                    match status.state {
                        "rolled_back" => "checkpoint_restored",
                        "recovery_required" => "unknown",
                        _ => "replacement_applied",
                    }
                } else if replace_pending && status.state == "roles_in_progress" {
                    // The replacement was dispatched and the process died before
                    // recording completion. The bookkeeping was written before
                    // the rename, so this role is not "untouched": a reader that
                    // compares the target to A and B can classify it exactly.
                    "replacement_started"
                } else if status.state == "recovery_required" {
                    "untouched_or_external"
                } else {
                    "untouched"
                };
                serde_json::json!({
                    "role": entry.role.label(),
                    "target_path": entry.target,
                    "target_existed_before": entry.existed,
                    "target_before_sha256": entry.sha256,
                    "source_sha256": source_file.map(|file| file.sha256.clone()),
                    "replacement_started": replaced || replace_pending,
                    "replacement_completed": replaced,
                    "final_state": final_state,
                })
            })
            .collect();
        let mut journal = serde_json::json!({
            "schema": "nioh3-save-restore-journal/v1",
            "operation_id": plan.plan_id.clone(),
            "state": status.state,
            "steam_account_id": crate::paths::account_id_from_save_path(&plan.save_path).ok(),
            "save_slot_index": crate::paths::save_slot_index_from_path(&plan.save_path).ok(),
            "source_backup_directory": source.backup_id,
            "source_manifest_sha256": source.manifest_sha256,
            "rollback_checkpoint_directory": checkpoint.id.0,
            "targets": source.files.iter().map(|file| file.role.label()).collect::<Vec<_>>(),
            "role_results": role_results,
        });
        if let Some(digest) = status.installed_sha256 {
            journal["installed_sha256"] = serde_json::Value::String(digest.to_string());
            journal["committed_at_utc"] = serde_json::Value::String(timestamp_label());
        }
        if !status.rollback_errors.is_empty() {
            journal["rollback_errors"] = serde_json::json!(status.rollback_errors);
        }
        journal
    }

    fn write_restore_journal_value(
        &self,
        checkpoint_id: &RollbackCheckpointId,
        journal: &serde_json::Value,
    ) -> Result<(), SaveReadError> {
        let path = self
            .backup_dir(&checkpoint_id.0)
            .join("restore-journal.json");
        let text = serde_json::to_string_pretty(journal).map_err(|error| SaveReadError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        let staged = path.with_extension("json.scroll-generator-journal");
        let crashable_progress = journal.get("state").and_then(Value::as_str)
            == Some("roles_in_progress")
            && journal
                .get("role_results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|entry| {
                    entry
                        .get("replacement_completed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                });
        let cut = |candidate: RestoreCut| {
            crashable_progress && self.crash_hook == CrashHook::Exit(candidate)
        };
        let result = (|| -> Result<(), SaveReadError> {
            if cut(RestoreCut::JournalCreate) {
                crash_now(&RestoreCut::JournalCreate);
            }
            let mut handle = fs::File::create(&staged).map_err(|error| SaveReadError::Io {
                path: staged.display().to_string(),
                message: error.to_string(),
            })?;
            if cut(RestoreCut::JournalWrite) {
                crash_now(&RestoreCut::JournalWrite);
            }
            use std::io::Write;
            handle
                .write_all(text.as_bytes())
                .map_err(|error| SaveReadError::Io {
                    path: staged.display().to_string(),
                    message: error.to_string(),
                })?;
            if cut(RestoreCut::JournalFlush) {
                crash_now(&RestoreCut::JournalFlush);
            }
            handle.flush().map_err(|error| SaveReadError::Io {
                path: staged.display().to_string(),
                message: error.to_string(),
            })?;
            handle.sync_all().map_err(|error| SaveReadError::Io {
                path: staged.display().to_string(),
                message: error.to_string(),
            })?;
            if cut(RestoreCut::JournalReplace) {
                crash_now(&RestoreCut::JournalReplace);
            }
            replace_durable(&staged, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&staged);
        }
        result
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
        // The authoritative record for one operation must never be truncated in
        // place: a reader that loses the previous valid intent would have no way
        // to tell an interrupted receipt from an untouched target. Stage the new
        // bytes beside the record, flush them, then rename over the old one so a
        // crash leaves either the old record or the new one, never a partial file.
        // Each stage honors its own injected fault so the failure taxonomy can be
        // exercised at create/write/flush/replace independently.
        //
        // Those four stages belong to the *terminal* record: the
        // intent-to-terminal conversion this guard owns. A `pending` intent
        // write never consumes one of them, so an armed stage always fails the
        // authoritative record after the target bytes landed and leaves the
        // durable intent in place for a committed-with-warning recovery.
        let terminal = receipt.outcome != "pending";
        let staged = path.with_extension("json.scroll-generator-receipt");
        let stage_result = (|| -> Result<(), SaveReadError> {
            if terminal && self.faults.should_fire(FaultPoint::ReceiptCreate) {
                return Err(SaveReadError::InjectedFault {
                    stage: FaultPoint::ReceiptCreate.label().to_string(),
                });
            }
            let mut handle = fs::File::create(&staged).map_err(|error| SaveReadError::Io {
                path: staged.display().to_string(),
                message: error.to_string(),
            })?;
            if terminal && self.faults.should_fire(FaultPoint::ReceiptWrite) {
                return Err(SaveReadError::InjectedFault {
                    stage: FaultPoint::ReceiptWrite.label().to_string(),
                });
            }
            use std::io::Write;
            handle
                .write_all(text.as_bytes())
                .map_err(|error| SaveReadError::Io {
                    path: staged.display().to_string(),
                    message: error.to_string(),
                })?;
            if terminal && self.faults.should_fire(FaultPoint::ReceiptFlush) {
                return Err(SaveReadError::InjectedFault {
                    stage: FaultPoint::ReceiptFlush.label().to_string(),
                });
            }
            handle.flush().map_err(|error| SaveReadError::Io {
                path: staged.display().to_string(),
                message: error.to_string(),
            })?;
            handle.sync_all().map_err(|error| SaveReadError::Io {
                path: staged.display().to_string(),
                message: error.to_string(),
            })?;
            if terminal && self.faults.should_fire(FaultPoint::ReceiptReplace) {
                return Err(SaveReadError::InjectedFault {
                    stage: FaultPoint::ReceiptReplace.label().to_string(),
                });
            }
            replace_durable(&staged, &path)
        })();
        if stage_result.is_err() {
            let _ = fs::remove_file(&staged);
        }
        stage_result
    }
}

fn cleanup_staged_restore(staged: &[StagedRestoreRole]) {
    for entry in staged {
        if entry.staged.exists() {
            let _ = fs::remove_file(&entry.staged);
        }
    }
}

/// The process-level cut a deterministic crash harness asks for.
///
/// The production default is [`CrashHook::None`], so an ordinary host performs
/// no extra work: the hook is consulted only inside the restore loop, after the
/// durable per-role journal marker is on disk and before the replacement or the
/// in-process recovery runs. A hooked host terminates the process instead of
/// returning through `finish_restore_failure`, which is what makes the restart
/// classification meaningful rather than a normal rollback.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CrashHook {
    /// Ship behavior: never cut the process.
    #[default]
    None,
    /// Exit the process at the named restore cut, with no recovery running.
    Exit(RestoreCut),
}

/// One named restore cut a crash harness can stop the process at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreCut {
    /// After the role's durable intent is written, before its replacement.
    BeforeReplace(SaveRole),
    /// After the role's replacement but before its completion is recorded.
    AfterReplace(SaveRole),
    /// While staging the first journal update after one role was replaced.
    JournalCreate,
    JournalWrite,
    JournalFlush,
    JournalReplace,
}

impl RestoreCut {
    /// Stable label, including the role, used by the harness and its tests.
    pub fn label(self) -> String {
        match self {
            Self::BeforeReplace(role) => format!("before-replace:{}", role.label()),
            Self::AfterReplace(role) => format!("after-replace:{}", role.label()),
            Self::JournalCreate => "journal-create".to_string(),
            Self::JournalWrite => "journal-write".to_string(),
            Self::JournalFlush => "journal-flush".to_string(),
            Self::JournalReplace => "journal-replace".to_string(),
        }
    }
}

impl SaveRole {
    /// Parse a role back from its journal label.
    pub fn from_label(label: &str) -> Option<Self> {
        [Self::Main, Self::GameBackup, Self::System]
            .into_iter()
            .find(|role| role.label() == label)
    }
}

/// Terminate the process at a named restore cut.
///
/// Only reached through an explicitly armed [`CrashHook`], so no shipped code
/// path calls this. The exit code is fixed so the harness can assert it, and the
/// message goes to stderr so a mistaken production arming is visible.
fn crash_now(cut: &RestoreCut) -> ! {
    eprintln!("deterministic crash cut at {}", cut.label());
    std::process::exit(9);
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

    /// One receipt for the atomic-replacement cases, independent of a save.
    fn receipt_record(
        operation_id: &str,
        outcome: &str,
        committed: Option<bool>,
    ) -> OperationReceipt {
        OperationReceipt {
            operation_id: operation_id.to_string(),
            kind: "edit".to_string(),
            outcome: outcome.to_string(),
            installed_sha256: Some("a".repeat(64)),
            backup_id: Some("backup-20260920".to_string()),
            message: None,
            committed,
        }
    }

    #[test]
    fn only_pending_and_uncertain_outcomes_fence_a_target() {
        // The fence must never block a target whose operation reached a terminal
        // word, and must never let an unresolved one through.
        for outcome in ["pending", "uncertain"] {
            assert!(is_unresolved_outcome(outcome), "{outcome} must fence");
        }
        for outcome in ["committed", "not_committed", "discarded", "unknown"] {
            assert!(!is_unresolved_outcome(outcome), "{outcome} is terminal");
        }
    }

    #[test]
    fn a_role_is_classified_by_the_bytes_it_holds() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let c = "c".repeat(64);
        assert_eq!(classify_role_state(None, Some(&a), Some(&b)), "missing");
        assert_eq!(
            classify_role_state(Some(&a), Some(&a), Some(&b)),
            "A_source"
        );
        assert_eq!(
            classify_role_state(Some(&b), Some(&a), Some(&b)),
            "B_checkpoint"
        );
        assert_eq!(
            classify_role_state(Some(&c), Some(&a), Some(&b)),
            "external_C"
        );
        // Only the main role of a write records an installed generation, so a
        // role with no source digest resolves to its checkpoint.
        assert_eq!(
            classify_role_state(Some(&b), None, Some(&b)),
            "B_checkpoint"
        );
        // An operation with no recorded identity may not be rounded to a state.
        assert_eq!(classify_role_state(Some(&c), None, None), "unknown");
        // Case differences are not a different generation.
        assert_eq!(
            classify_role_state(Some(&a.to_uppercase()), Some(&a), Some(&b)),
            "A_source"
        );
    }

    /// The four receipt stages that resolve an intent into a terminal record.
    const RECEIPT_STAGES: [FaultPoint; 4] = [
        FaultPoint::ReceiptCreate,
        FaultPoint::ReceiptWrite,
        FaultPoint::ReceiptFlush,
        FaultPoint::ReceiptReplace,
    ];

    fn receipt_siblings(state_root: &Path, operation_id: &str) -> Vec<String> {
        let directory = state_root.join("v2-operations");
        let mut names: Vec<String> = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .filter(|name| name != &format!("{operation_id}.json"))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_terminal_receipt_stage_never_truncates_the_durable_intent() {
        // RW01: a failure at create/write/flush/replace must leave the previous
        // valid intent byte-identical, so a later reader can still parse the
        // operation's own intent instead of finding a truncated file.
        let root = TempRoot::new("receipt-stages");
        let operation_id = "0123456789abcdef0123456789abcdef";
        let path = root
            .0
            .join("v2-operations")
            .join(format!("{operation_id}.json"));
        for point in RECEIPT_STAGES {
            let writer = SaveTransactionHost::new(&root.0);
            let intent = receipt_record(operation_id, "pending", None);
            writer.write_receipt(&intent).unwrap();
            let before = fs::read_to_string(&path).unwrap();
            assert!(serde_json::from_str::<OperationReceipt>(&before).is_ok());

            let faults = TransactionFaults::default();
            faults.arm(point);
            let host = SaveTransactionHost::with_faults(&root.0, faults);
            let terminal = receipt_record(operation_id, "committed", Some(true));
            let error = host.write_receipt(&terminal).unwrap_err();
            assert!(
                matches!(&error, SaveReadError::InjectedFault { stage } if stage == point.label()),
                "{point:?}: {error}",
            );
            // The old intent is untouched, not truncated and not renamed away.
            assert_eq!(fs::read_to_string(&path).unwrap(), before, "{point:?}");
            let observed = host.receipt(operation_id).unwrap().unwrap();
            assert_eq!(observed.outcome, "pending", "{point:?}");
            assert_eq!(
                observed.installed_sha256, intent.installed_sha256,
                "{point:?}"
            );
            // No staged sibling may survive a failed stage.
            assert!(
                receipt_siblings(&root.0, operation_id).is_empty(),
                "{point:?}: {:?}",
                receipt_siblings(&root.0, operation_id),
            );
            // The arming was consumed by the failure, so one more write is the
            // terminal record rather than a second injected fault.
            host.write_receipt(&terminal).unwrap();
            assert_eq!(
                host.receipt(operation_id).unwrap().unwrap().outcome,
                "committed"
            );
            fs::remove_file(&path).unwrap();
        }
    }

    #[test]
    fn a_pending_intent_write_never_consumes_a_terminal_stage_arming() {
        // The commit writes its durable intent before it touches the target, so
        // a stage faulted here would abort before any bytes moved and could
        // never express the post-commit case RW01 needs.
        let root = TempRoot::new("receipt-intent");
        let operation_id = "fedcba9876543210fedcba9876543210";
        let faults = TransactionFaults::default();
        faults.arm(FaultPoint::ReceiptReplace);
        let host = SaveTransactionHost::with_faults(&root.0, faults);
        host.write_receipt(&receipt_record(operation_id, "pending", None))
            .unwrap();
        let error = host
            .write_receipt(&receipt_record(operation_id, "committed", Some(true)))
            .unwrap_err();
        assert!(
            matches!(&error, SaveReadError::InjectedFault { stage } if stage == "receipt-replace"),
            "{error}",
        );
        assert_eq!(
            host.receipt(operation_id).unwrap().unwrap().outcome,
            "pending"
        );
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
