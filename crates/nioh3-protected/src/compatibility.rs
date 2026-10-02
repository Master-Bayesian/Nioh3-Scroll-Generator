//! Explicit, process-bound consent for executable variants, with verified backups.
use crate::HostError;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_BACKUP_BYTES: u64 = 128 * 1024 * 1024;
const REQUIRED_CHECKS: [&str; 7] = [
    "process_identity",
    "code_and_layout",
    "ownership_and_bounds",
    "verified_backup",
    "single_writer",
    "recovery_receipts",
    "readback",
];
static BACKUP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ExecutableIdentity {
    pub pid: u32,
    pub creation_filetime: u64,
    pub path: String,
    pub version: String,
    pub sha256: String,
}
impl ExecutableIdentity {
    pub fn reference(&self) -> bool {
        self.version == "2.0.2.0"
            && self
                .sha256
                .eq_ignore_ascii_case(nioh3_runtime::INVENTORY_EXECUTABLE_SHA256)
    }
    pub fn supported(&self) -> bool {
        matches!(self.version.as_str(), "2.0.0.2" | "2.0.1.0" | "2.0.2.0")
    }
}

#[derive(Serialize)]
struct Difference {
    code: &'static str,
    expected: String,
    actual: String,
}
fn differences(identity: &ExecutableIdentity) -> Vec<Difference> {
    let mut result = Vec::new();
    if identity.version != "2.0.2.0" {
        result.push(Difference {
            code: "game_version",
            expected: "2.0.2.0".into(),
            actual: identity.version.clone(),
        });
    }
    if !identity
        .sha256
        .eq_ignore_ascii_case(nioh3_runtime::INVENTORY_EXECUTABLE_SHA256)
    {
        result.push(Difference {
            code: "executable_sha256",
            expected: nioh3_runtime::INVENTORY_EXECUTABLE_SHA256.into(),
            actual: identity.sha256.clone(),
        });
    }
    if matches!(identity.version.as_str(), "2.0.0.2" | "2.0.1.0") {
        result.push(Difference {
            code: "character_layout_evidence",
            expected: "verified_reference".into(),
            actual: "experimental_version_selected".into(),
        });
    }
    result
}
fn feature_flags(identity: &ExecutableIdentity) -> [(&'static str, bool); 9] {
    [
        ("offline_search", identity.supported()),
        ("save_edit", identity.supported()),
        (
            "live_scroll_add",
            matches!(identity.version.as_str(), "2.0.1.0" | "2.0.2.0"),
        ),
        ("live_character", identity.supported()),
        ("live_equipment_add", identity.version == "2.0.2.0"),
        (
            "live_count_edit",
            matches!(identity.version.as_str(), "2.0.1.0" | "2.0.2.0"),
        ),
        ("native_generation", identity.supported()),
        ("temporary_override", identity.supported()),
        (
            "challenge_capacity_override",
            matches!(identity.version.as_str(), "2.0.1.0" | "2.0.2.0"),
        ),
    ]
}

fn operation_scoped(feature: &str) -> bool {
    matches!(feature, "live_scroll_add" | "live_equipment_add")
}

/// Check version-specific runtime capability, without granting write authority.
/// Live additions must still pass their own backup, code/layout, identity,
/// ownership, plan, receipt and readback checks at the operation boundary.
pub fn validate_feature(identity: &ExecutableIdentity, feature: &str) -> Result<(), HostError> {
    if !feature_flags(identity).into_iter().any(|(name, enabled)| {
        enabled && name == feature && !matches!(name, "offline_search" | "save_edit")
    }) {
        return Err(HostError::rejected(format!(
            "COMPATIBILITY_FEATURE_UNSUPPORTED: {feature} has no verified binding for actual FILEVERSION {}. Select a game version with a verified binding for this operation, reconnect, and prepare again; no operation was authorized.",
            identity.version
        )));
    }
    Ok(())
}

fn allowed_features(identity: &ExecutableIdentity) -> Vec<String> {
    feature_flags(identity)
        .into_iter()
        .filter(|(name, enabled)| {
            *enabled && !matches!(*name, "offline_search" | "save_edit") && !operation_scoped(name)
        })
        .map(|(name, _)| name.to_string())
        .collect()
}

fn operation_scoped_features(identity: &ExecutableIdentity) -> Vec<String> {
    feature_flags(identity)
        .into_iter()
        .filter(|(name, enabled)| *enabled && operation_scoped(name))
        .map(|(name, _)| name.to_string())
        .collect()
}

#[derive(Serialize)]
struct BackupFile {
    source: PathBuf,
    copy: PathBuf,
    bytes: u64,
    sha256: String,
}
struct VerifiedBackup {
    files: Vec<BackupFile>,
    manifest: PathBuf,
    manifest_bytes: u64,
    manifest_sha256: String,
}
impl VerifiedBackup {
    fn report(&self) -> Value {
        json!({"attempted":true,"verified":true,
            "paths":self.files.iter().map(|file| &file.copy).collect::<Vec<_>>(),
            "manifest":self.manifest,"files":self.files,"error":Value::Null})
    }
    fn verify_sources(&self) -> Result<(), String> {
        for file in &self.files {
            for _ in 0..2 {
                verify_file(&file.source, file.bytes, &file.sha256).map_err(|error| {
                    format!("Source changed after backup; prepare and review a fresh plan: {error}")
                })?;
            }
        }
        Ok(())
    }
    fn verify(&self) -> Result<(), String> {
        verify_file(&self.manifest, self.manifest_bytes, &self.manifest_sha256)?;
        for file in &self.files {
            verify_file(&file.copy, file.bytes, &file.sha256)?;
        }
        Ok(())
    }
}
struct PendingPlan {
    plan_id: String,
    bypassed_checks: Vec<String>,
    allowed_features: Vec<String>,
    audit_root: PathBuf,
    audit_path: Option<PathBuf>,
}
impl PendingPlan {
    fn report(&self) -> Value {
        json!({"plan_id":self.plan_id,"bypassed_checks":self.bypassed_checks,
            "allowed_features":self.allowed_features,"required_checks":REQUIRED_CHECKS,
            "audit_path":self.audit_path})
    }
}
#[derive(Default)]
pub struct CompatibilitySession {
    identity: Option<ExecutableIdentity>,
    backup: Option<VerifiedBackup>,
    backup_report: Option<Value>,
    plan: Option<PendingPlan>,
    accepted: bool,
    audit_error: Option<String>,
}
impl CompatibilitySession {
    fn bind(&mut self, identity: &ExecutableIdentity) {
        if self.identity.as_ref() != Some(identity) {
            self.identity = Some(identity.clone());
            self.backup = None;
            self.backup_report = None;
            self.cancel();
        }
    }
    /// Cancellation does not depend on finding a running game process.
    pub fn cancel(&mut self) {
        self.accepted = false;
        self.plan = None;
        self.audit_error = None;
    }
    fn report_current(&self, identity: &ExecutableIdentity) -> Value {
        let mut hard_blocks = Vec::new();
        if !identity.supported() {
            hard_blocks.push(json!({"code":"unsupported_version","detail":"No trusted resource, character layout or native ABI binding exists for this version; a local structure match does not authorize mutation"}));
        }
        if !identity.reference() {
            match &self.backup_report {
                None => hard_blocks.push(json!({"code":"backup_required","detail":"Prepare a fresh verified save backup for global compatibility consent; live scroll/equipment additions verify their own operation backups"})),
                Some(report) if self.backup.is_none() => hard_blocks.push(json!({"code":"backup_unverified","detail":report["error"]})),
                Some(_) => {}
            }
        }
        if let Some(error) = &self.audit_error {
            hard_blocks.push(json!({"code":"consent_audit_failed","detail":error}));
        }
        let features: serde_json::Map<String, Value> = feature_flags(identity)
            .into_iter()
            .map(|(name, enabled)| (name.to_string(), json!(enabled)))
            .collect();
        json!({"present":true,"process_id":identity.pid,"process_creation_time":identity.creation_filetime,
            "executable":identity.path,"game_version":identity.version,"sha256":identity.sha256,
            "reference_match":identity.reference(),"warning":!identity.reference(),"accepted":self.accepted,
            "backup":self.backup_report,"features":features,"differences":differences(identity),
            "operation_scoped_features":operation_scoped_features(identity),
            "hard_blocks":hard_blocks,"plan":self.plan.as_ref().map(PendingPlan::report)})
    }
    pub fn report(&mut self, identity: &ExecutableIdentity) -> Value {
        self.bind(identity);
        if self.backup.is_some() {
            // Inspect records a changed or missing copy as a hard block too.
            self.recheck_backup(false);
        }
        self.report_current(identity)
    }
    fn recheck_backup(&mut self, current_sources: bool) -> bool {
        let Some(backup) = &self.backup else {
            return false;
        };
        let verified = backup.verify().and_then(|()| {
            if current_sources {
                backup.verify_sources()
            } else {
                Ok(())
            }
        });
        match verified {
            Ok(()) => true,
            Err(error) => {
                if let Some(report) = &mut self.backup_report {
                    report["verified"] = json!(false);
                    report["error"] = json!(error);
                }
                self.backup = None;
                self.cancel();
                false
            }
        }
    }
    pub fn prepare_failed(&mut self, identity: &ExecutableIdentity, error: &str) -> Value {
        self.bind(identity);
        self.cancel();
        self.backup = None;
        self.backup_report =
            Some(json!({"attempted":true,"verified":false,"paths":[],"error":error}));
        self.report_current(identity)
    }
    pub fn prepare(
        &mut self,
        identity: &ExecutableIdentity,
        root: &Path,
        sources: &[PathBuf],
    ) -> Value {
        self.bind(identity);
        // Every explicit preparation replaces consent and reads current source bytes.
        self.cancel();
        self.backup = None;
        match backup(root, sources) {
            Ok(backup) => {
                let report = backup.report();
                if identity.supported() && !identity.reference() {
                    // The fresh manifest path includes a timestamp and sequence nonce.
                    let binding = json!({"identity":identity,"backup":report,"manifest_sha256":backup.manifest_sha256});
                    let plan_id = format!("{:x}", Sha256::digest(binding.to_string().as_bytes()));
                    self.plan = Some(PendingPlan {
                        plan_id,
                        bypassed_checks: differences(identity)
                            .iter()
                            .map(|difference| difference.code.to_string())
                            .collect(),
                        allowed_features: allowed_features(identity),
                        audit_root: root.join("compatibility-consents"),
                        audit_path: None,
                    });
                }
                self.backup_report = Some(report);
                self.backup = Some(backup);
            }
            Err(error) => return self.prepare_failed(identity, &error),
        }
        self.report_current(identity)
    }
    pub fn accept(
        &mut self,
        identity: &ExecutableIdentity,
        reviewed_plan_id: &str,
        confirmed: bool,
        backup_confirmed: bool,
    ) -> Result<Value, HostError> {
        self.bind(identity);
        self.accepted = false;
        if !identity.supported() {
            return Err(HostError::rejected(
                "Unsupported game version; no trusted resource/layout/ABI binding",
            ));
        }
        if !confirmed || !backup_confirmed {
            return Err(HostError::rejected("COMPATIBILITY_CONFIRMATION_REQUIRED: review the exact plan and confirm both the risks and the verified backup"));
        }
        if self
            .plan
            .as_ref()
            .is_none_or(|plan| plan.plan_id != reviewed_plan_id)
        {
            return Err(HostError::rejected(
                "COMPATIBILITY_PLAN_MISMATCH: prepare and review the current compatibility plan",
            ));
        }
        if !self.recheck_backup(true) {
            let detail = self
                .backup_report
                .as_ref()
                .and_then(|report| report["error"].as_str())
                .unwrap_or("No verified backup is available");
            return Err(HostError::rejected(format!(
                "COMPATIBILITY_BACKUP_REQUIRED: {detail}; prepare and review a fresh plan"
            )));
        }
        let plan = self
            .plan
            .as_ref()
            .ok_or_else(|| HostError::rejected("COMPATIBILITY_PLAN_MISMATCH"))?;
        let backup = self
            .backup
            .as_ref()
            .ok_or_else(|| HostError::rejected("COMPATIBILITY_BACKUP_REQUIRED"))?;
        match persist_consent(plan, identity, backup) {
            Ok(path) => {
                if let Some(plan) = &mut self.plan {
                    plan.audit_path = Some(path);
                }
                self.audit_error = None;
                self.accepted = true;
                Ok(self.report_current(identity))
            }
            Err(error) => {
                self.audit_error = Some(error.clone());
                Err(HostError::rejected(format!(
                    "COMPATIBILITY_AUDIT_FAILED: {error}"
                )))
            }
        }
    }
    pub fn require_feature(
        &mut self,
        identity: &ExecutableIdentity,
        feature: &str,
    ) -> Result<(), HostError> {
        validate_feature(identity, feature)?;
        if operation_scoped(feature) {
            // This is capability admission only. These operations own their
            // verified backups and all native write checks independently of
            // global compatibility consent or its cancellation state.
            return Ok(());
        }
        self.require(identity)
    }
    pub fn require(&mut self, identity: &ExecutableIdentity) -> Result<(), HostError> {
        self.bind(identity);
        if !identity.supported() {
            return Err(HostError::rejected(
                "Unsupported game version; no trusted resource/layout/ABI binding",
            ));
        }
        if identity.reference() {
            return Ok(());
        }
        if !self.accepted || self.plan.is_none() {
            return Err(HostError::rejected("COMPATIBILITY_CONFIRMATION_REQUIRED: prepare a fresh plan for the current process, review it and confirm a verified backup"));
        }
        if !self.recheck_backup(false) {
            let detail = self
                .backup_report
                .as_ref()
                .and_then(|report| report["error"].as_str())
                .unwrap_or("No verified backup is available");
            return Err(HostError::rejected(format!(
                "COMPATIBILITY_BACKUP_REQUIRED: {detail}; prepare and review a fresh plan"
            )));
        }
        Ok(())
    }
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.is_empty() || bytes.len() as u64 > maximum {
        return Err(format!(
            "{}: file size is outside the bounded backup range",
            path.display()
        ));
    }
    Ok(bytes)
}
fn verify_file(path: &Path, expected_bytes: u64, expected_sha256: &str) -> Result<(), String> {
    let bytes = read_bounded(path, expected_bytes)?;
    if bytes.len() as u64 != expected_bytes
        || format!("{:x}", Sha256::digest(&bytes)) != expected_sha256
    {
        return Err(format!(
            "{}: backup bytes or SHA-256 no longer match the verified manifest",
            path.display()
        ));
    }
    Ok(())
}
fn timestamp() -> Result<u128, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .map_err(|error| error.to_string())
}
fn backup(root: &Path, sources: &[PathBuf]) -> Result<VerifiedBackup, String> {
    if sources.is_empty() {
        return Err("No save was found automatically; a verified backup is required and manual confirmation cannot replace it".into());
    }
    let parent = root.join("compatibility-backups");
    std::fs::create_dir_all(&parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    let directory = parent.join(format!(
        "{}-{}-{}",
        timestamp()?,
        std::process::id(),
        BACKUP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let mut files = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let bytes = read_bounded(source, MAX_BACKUP_BYTES)?;
        if read_bounded(source, MAX_BACKUP_BYTES)? != bytes {
            return Err("Save changed while preparing the backup; retry at an idle screen".into());
        }
        let target = directory.join(format!("{index:02}-SAVEDATA.BIN"));
        let mut copy = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|error| format!("{}: {error}", target.display()))?;
        copy.write_all(&bytes)
            .and_then(|()| copy.sync_all())
            .map_err(|error| format!("{}: {error}", target.display()))?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        verify_file(&target, bytes.len() as u64, &sha256)?;
        files.push(BackupFile {
            source: source.clone(),
            copy: target,
            bytes: bytes.len() as u64,
            sha256,
        });
    }
    let manifest = directory.join("manifest.json");
    let bytes =
        serde_json::to_vec_pretty(&json!({"schema":"nioh3-compatibility-backup/v1","files":files}))
            .map_err(|error| error.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| error.to_string())?;
    let verified = VerifiedBackup {
        files,
        manifest,
        manifest_bytes: bytes.len() as u64,
        manifest_sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    verified.verify()?;
    Ok(verified)
}
fn persist_consent(
    plan: &PendingPlan,
    identity: &ExecutableIdentity,
    backup: &VerifiedBackup,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(&plan.audit_root).map_err(|error| {
        format!(
            "{}: {error}; restore access and prepare again",
            plan.audit_root.display()
        )
    })?;
    let path = plan.audit_root.join(format!("{}.json", plan.plan_id));
    let record = json!({"schema":"nioh3-compatibility-consent/v1","plan_id":plan.plan_id,
        "identity":identity,"bypassed_checks":plan.bypassed_checks,"allowed_features":plan.allowed_features,
        "required_checks":REQUIRED_CHECKS,"backup_manifest":backup.manifest,
        "backup_manifest_sha256":backup.manifest_sha256,"backup_files":backup.files,
        "confirmed":true,"backup_confirmed":true,"accepted_unix_nanos":timestamp()?.to_string()});
    let bytes = serde_json::to_vec_pretty(&record).map_err(|error| error.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "{}: {error}; restore access and prepare again",
                path.display()
            )
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| error.to_string())?;
    Ok(path)
}

/// Keep discovery errors distinct from an empty automatic-save folder.
pub fn discover_sources(root: &Path) -> Result<Vec<PathBuf>, HostError> {
    let sources = nioh3_save::paths::discover_save_paths(root).map_err(|error| {
        let reason = HostError::from_save(error);
        HostError::rejected(format!(
            "Save discovery under {} failed: {}; restore access and prepare again",
            root.display(),
            reason.message
        ))
    })?;
    if sources.is_empty() {
        return Err(HostError::rejected(format!("No save was found under {}; expected <account>/SAVEDATAxx/SAVEDATA.BIN. Create a save in game or restore access to this folder, then prepare again", root.display())));
    }
    Ok(sources)
}
pub fn automatic_sources() -> Result<Vec<PathBuf>, HostError> {
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        .ok_or_else(|| HostError::rejected("LOCALAPPDATA is unavailable; restore the application environment and prepare again"))?;
    discover_sources(&root.join("KoeiTecmo/NIOH3/Savedata"))
}
#[cfg(windows)]
pub fn running_identity() -> Result<ExecutableIdentity, HostError> {
    use nioh3_runtime::RuntimeError;
    let pid = nioh3_runtime::single_process_id(nioh3_runtime::GAME_IMAGE_NAME)
        .map_err(HostError::from_runtime)?;
    let creation = nioh3_runtime::process_creation_filetime(pid)
        .map_err(HostError::from_runtime)?
        .ok_or_else(|| HostError::from_runtime(RuntimeError::ProcessGone { pid }))?;
    let process =
        nioh3_runtime::ReadOnlyProcess::open(pid, nioh3_runtime::GAME_MODULE_NAME, Some(creation))
            .map_err(HostError::from_runtime)?;
    let path = process.image_path().map_err(HostError::from_runtime)?;
    // An unknown readable FILEVERSION belongs in diagnostics, never a fallback binding.
    let version = nioh3_runtime::file_version(&path)
        .map_err(HostError::from_runtime)?
        .display();
    let sha256 = nioh3_runtime::file_sha256(&path).map_err(HostError::from_runtime)?;
    Ok(ExecutableIdentity {
        pid,
        creation_filetime: creation,
        path,
        version,
        sha256,
    })
}
