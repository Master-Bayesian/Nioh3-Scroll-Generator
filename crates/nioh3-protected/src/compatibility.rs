//! Explicit, process-bound consent for executable variants, with truthful backups.
use crate::HostError;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
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
#[derive(Default)]
pub struct CompatibilitySession {
    identity: Option<ExecutableIdentity>,
    backup: Option<Value>,
    accepted: bool,
}
impl CompatibilitySession {
    fn bind(&mut self, identity: &ExecutableIdentity) {
        if self.identity.as_ref() != Some(identity) {
            self.identity = Some(identity.clone());
            self.backup = None;
            self.accepted = false;
        }
    }
    pub fn report(&mut self, identity: &ExecutableIdentity) -> Value {
        self.bind(identity);
        json!({"present":true,"process_id":identity.pid,"process_creation_time":identity.creation_filetime,
            "executable":identity.path,"game_version":identity.version,"sha256":identity.sha256,
            "reference_match":identity.reference(),"warning":!identity.reference(),"accepted":self.accepted,
            "backup":self.backup,"features":{"offline_search":identity.supported(),"save_edit":identity.supported(),
                "live_scroll_add":identity.version=="2.0.1.0"||identity.version=="2.0.2.0",
                "live_character":identity.supported(),
                "live_equipment_add":identity.version=="2.0.2.0"} })
    }
    pub fn prepare(
        &mut self,
        identity: &ExecutableIdentity,
        root: &Path,
        sources: &[PathBuf],
    ) -> Value {
        self.bind(identity);
        if self
            .backup
            .as_ref()
            .is_none_or(|value| value["verified"] != true)
        {
            self.backup = Some(backup(root, sources));
        }
        self.report(identity)
    }
    pub fn accept(
        &mut self,
        identity: &ExecutableIdentity,
        confirmed: bool,
        backup_confirmed: bool,
    ) -> Result<Value, HostError> {
        self.bind(identity);
        if !identity.supported() {
            return Err(HostError::rejected(
                "Unsupported game version; no matching resources",
            ));
        }
        if !confirmed || !backup_confirmed || self.backup.is_none() {
            return Err(HostError::rejected("COMPATIBILITY_CONFIRMATION_REQUIRED: review the version warning and confirm a usable backup"));
        }
        self.accepted = true;
        Ok(self.report(identity))
    }
    pub fn require(&mut self, identity: &ExecutableIdentity) -> Result<(), HostError> {
        self.bind(identity);
        if !identity.reference() && !self.accepted {
            return Err(HostError::rejected("COMPATIBILITY_CONFIRMATION_REQUIRED: review the version warning and confirm a usable backup"));
        }
        Ok(())
    }
}

fn backup(root: &Path, sources: &[PathBuf]) -> Value {
    let result = (|| -> Result<Vec<String>, String> {
        if sources.is_empty() {
            return Err("No save was found automatically; make and confirm a manual backup".into());
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let parent = root.join("compatibility-backups");
        std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        let directory = parent.join(stamp.to_string());
        std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let mut paths = Vec::new();
        let mut entries = Vec::new();
        for (index, source) in sources.iter().take(16).enumerate() {
            let size = std::fs::metadata(source).map_err(|e| e.to_string())?.len();
            if size == 0 || size > 128 * 1024 * 1024 {
                return Err("Save size is outside the bounded backup range".into());
            }
            let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
            if std::fs::read(source).map_err(|e| e.to_string())? != bytes {
                return Err(
                    "Save changed while preparing the backup; retry at an idle screen".into(),
                );
            }
            let target = directory.join(format!("{index:02}-SAVEDATA.BIN"));
            std::fs::write(&target, &bytes).map_err(|e| e.to_string())?;
            if std::fs::read(&target).map_err(|e| e.to_string())? != bytes {
                return Err("Backup readback did not match".into());
            }
            paths.push(target.to_string_lossy().into_owned());
            entries.push(json!({"source":source,"copy":target,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))}));
        }
        std::fs::write(
            directory.join("manifest.json"),
            serde_json::to_vec_pretty(
                &json!({"schema":"nioh3-compatibility-backup/v1","files":entries}),
            )
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(paths)
    })();
    match result {
        Ok(paths) => json!({"attempted":true,"verified":true,"paths":paths,"error":Value::Null}),
        Err(error) => json!({"attempted":true,"verified":false,"paths":[],"error":error}),
    }
}

pub fn automatic_sources() -> Vec<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .and_then(|root| {
            nioh3_save::paths::discover_save_paths(&root.join("KoeiTecmo/NIOH3/Savedata")).ok()
        })
        .unwrap_or_default()
}
#[cfg(windows)]
pub fn running_identity() -> Result<ExecutableIdentity, HostError> {
    use nioh3_runtime::{GameCompatibility, RuntimeError};
    let pid = nioh3_runtime::single_process_id(nioh3_runtime::GAME_IMAGE_NAME)
        .map_err(HostError::from_runtime)?;
    let creation = nioh3_runtime::process_creation_filetime(pid)
        .map_err(HostError::from_runtime)?
        .ok_or_else(|| HostError::from_runtime(RuntimeError::ProcessGone { pid }))?;
    let process =
        nioh3_runtime::ReadOnlyProcess::open(pid, nioh3_runtime::GAME_MODULE_NAME, Some(creation))
            .map_err(HostError::from_runtime)?;
    let path = process.image_path().map_err(HostError::from_runtime)?;
    let status = nioh3_runtime::verify_game_executable(&path);
    let version = match (status.state, status.file_version) {
        (GameCompatibility::Supported, Some(version)) => version.display(),
        _ => {
            return Err(HostError::rejected(
                "Unsupported game version; no matching resources",
            ))
        }
    };
    let sha256 = nioh3_runtime::file_sha256(&path).map_err(HostError::from_runtime)?;
    Ok(ExecutableIdentity {
        pid,
        creation_filetime: creation,
        path,
        version,
        sha256,
    })
}
