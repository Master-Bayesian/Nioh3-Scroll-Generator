//! Instance-scoped remaining-count edits with durable receipts.
//!
//! Port of `runtime_count_edit.RuntimeCountEditor`: plan, execute, status and
//! recover over an exclusive receipt directory, plus the record helpers
//! `checked_count` and `stable_identity`.
//!
//! Two boundaries differ from the shipped class on purpose:
//!
//! - Backup creation belongs to the save side (`create_backup_directory`,
//!   `write_backup_manifest`), which is a separate workstream. `prepare` takes
//!   the backup the save side produced and verifies it instead of copying it.
//! - `prepare` accepts the save path, its digest and the record hex explicitly
//!   rather than reading a snapshot registry.
//!
//! A write is only attempted after every gate passes, the readback is compared
//! against the expected record, and a lost reply is never replayed: the receipt
//! states are `prepared`, `rejected`, `uncertain` and `verified`, and only
//! `recover` may resolve `uncertain`, by observation.

use crate::error::RuntimeError;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// `SCROLL_RECORD_SIZE`.
pub const RECORD_SIZE: usize = 0xE8;
/// Byte offset of the remaining count inside the record.
pub const COUNT_OFFSET: usize = 0x33;
/// Byte offset of the instance serial inside the record.
pub const SERIAL_OFFSET: usize = 0x28;
/// Highest accepted remaining count.
pub const MAX_COUNT: u8 = 7;

/// `checked_count`.
pub fn checked_count(value: i64) -> Result<u8, RuntimeError> {
    if !(0..=MAX_COUNT as i64).contains(&value) {
        return Err(RuntimeError::InvalidCount { value });
    }
    Ok(value as u8)
}

/// `stable_identity`: the defined fields that must not change.
///
/// The new-item marker is masked because viewing a scroll can clear it, the
/// live count byte is dropped because it can differ from the last checkpoint,
/// and the undefined gap `0x24..0x28` is excluded.
pub fn stable_identity(raw: &[u8]) -> Vec<u8> {
    let mut masked = raw.to_vec();
    masked[0x18] &= !2;
    masked[COUNT_OFFSET] = 0;
    let mut identity = Vec::with_capacity(0xE0);
    identity.extend_from_slice(&masked[..0x24]);
    identity.extend_from_slice(&masked[0x28..0xE4]);
    identity
}

/// One located instance: the shipped capture dictionary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetCapture {
    pub pid: u32,
    pub creation_time: String,
    pub manager: u64,
    pub data: u64,
    pub address: u64,
    pub record_hex: String,
    pub serial: u64,
}

impl TargetCapture {
    /// The fields `recover` compares before it may verify an outcome.
    pub fn same_instance(&self, other: &Self) -> bool {
        self.pid == other.pid
            && self.creation_time == other.creation_time
            && self.manager == other.manager
            && self.data == other.data
            && self.address == other.address
            && self.serial == other.serial
    }
}

/// The instance-scoped record access a count edit needs.
pub trait CountMemory {
    /// `capture(serial)`.
    fn capture(&mut self, serial: u64) -> Result<TargetCapture, RuntimeError>;

    /// `write(expected, desired)`: revalidate the instance, write one byte with
    /// the minimum write rights, then read the whole record back.
    fn write(&mut self, expected: &TargetCapture, desired: u8) -> Result<Vec<u8>, RuntimeError>;
}

/// The two process views one count edit uses.
///
/// The read view locates the record and performs the readback; the write view
/// is opened only inside the explicit write and carries the minimal write mask.
pub trait CountProcesses {
    fn open_read(
        &self,
        pid: u32,
    ) -> Result<Box<dyn crate::mutation::memory::TargetProcess>, RuntimeError>;

    fn open_write(
        &self,
        pid: u32,
    ) -> Result<Box<dyn crate::mutation::memory::TargetProcess>, RuntimeError>;
}

/// The `live_add_profile` fields a count edit resolves addresses with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CountLayout {
    pub manager_pointer_rva: u64,
    pub container_offset: u64,
    pub capacity_offset: u64,
    /// `live_add_profile.PC_V201.insertion_rva`, the inventory signature site.
    pub insertion_rva: u64,
    pub capacity: u32,
    pub record_size: usize,
    pub count_offset: usize,
}

/// `live_add_profile.PC_V201`, the only layout the product validates.
pub const PC_V201_COUNT_LAYOUT: CountLayout = CountLayout {
    manager_pointer_rva: 0x474D4E0,
    container_offset: 0x224A60,
    capacity_offset: 0x16A80,
    insertion_rva: 0x54_D294,
    capacity: 400,
    record_size: RECORD_SIZE,
    count_offset: COUNT_OFFSET,
};

/// Port of `runtime_count_edit.WindowsCountMemory` over the shipped address
/// math: module base + `manager_pointer_rva`, then `container_offset`, then the
/// fixed-capacity record array.
///
/// `capture_inventory`'s signature gate and entry filter live in the inventory
/// slice; this adapter keeps the same owner, capacity and serial checks so a
/// located record is still validated before it is exposed.
pub struct WindowsCountMemory<P: CountProcesses> {
    pid: u32,
    module_base: u64,
    layout: CountLayout,
    processes: P,
    reader: Option<Box<dyn crate::mutation::memory::TargetProcess>>,
}

impl<P: CountProcesses> WindowsCountMemory<P> {
    pub fn new(pid: u32, module_base: u64, layout: CountLayout, processes: P) -> Self {
        Self {
            pid,
            module_base,
            layout,
            processes,
            reader: None,
        }
    }

    fn reader(
        &mut self,
    ) -> Result<&mut Box<dyn crate::mutation::memory::TargetProcess>, RuntimeError> {
        if self.reader.is_none() {
            self.reader = Some(self.processes.open_read(self.pid)?);
        }
        self.reader.as_mut().ok_or(RuntimeError::SessionNotOpen)
    }

    /// The inventory layout this adapter validates against.
    fn inventory_layout(&self) -> crate::mutation::inventory::InventoryLayout {
        crate::mutation::inventory::InventoryLayout {
            insertion_rva: self.layout.insertion_rva,
            manager_pointer_rva: self.layout.manager_pointer_rva,
            container_offset: self.layout.container_offset,
            capacity_offset: self.layout.capacity_offset,
            serial_index_offset: crate::mutation::inventory::PC_V201_INVENTORY_LAYOUT
                .serial_index_offset,
            capacity: self.layout.capacity,
            record_size: self.layout.record_size,
            // `WindowsCountMemory.capture` reads its counters with the shipped
            // fixed layout: acquisition order first, serial at +8.
            serial_counter_offset: 8,
        }
    }

    /// The record for `serial` through the shipped inventory gate.
    fn capture_of(&mut self, serial: u64) -> Result<TargetCapture, RuntimeError> {
        let layout = self.layout;
        let inventory_layout = self.inventory_layout();
        let pid = self.pid;
        let module_base = self.module_base;
        let inventory = {
            let reader = self.reader()?;
            let mut view = crate::mutation::inventory::ReadView {
                pid,
                module_base,
                reader: &mut **reader,
            };
            crate::mutation::inventory::capture_inventory(&mut view, &inventory_layout, "PC v2.01")?
        };
        let matches: Vec<crate::mutation::inventory::InventoryEntry> = inventory
            .entries
            .iter()
            .filter(|entry| entry.serial == serial.to_string())
            .cloned()
            .collect();
        if matches.len() != 1 {
            return Err(RuntimeError::CountInstanceUnavailable { serial });
        }
        let entry = matches[0].clone();
        let manager_address = module_base + layout.manager_pointer_rva;
        let reader = self.reader()?;
        let manager = read_u64(reader, manager_address)?;
        let data = read_u64(reader, manager)?;
        let address =
            data + layout.container_offset + (entry.slot_index * layout.record_size) as u64;
        let record = reader.read(address, layout.record_size)?;
        if record
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            != entry.record_hex
        {
            return Err(RuntimeError::CountSourceChanged {
                detail: "Scroll changed during inspection".to_string(),
            });
        }
        Ok(TargetCapture {
            pid,
            creation_time: self.creation_time()?,
            manager,
            data,
            address,
            record_hex: entry.record_hex,
            serial,
        })
    }
}

impl<P: CountProcesses> CountMemory for WindowsCountMemory<P> {
    fn capture(&mut self, serial: u64) -> Result<TargetCapture, RuntimeError> {
        self.capture_of(serial)
    }

    fn write(&mut self, expected: &TargetCapture, desired: u8) -> Result<Vec<u8>, RuntimeError> {
        // Every gate runs again, then the minimal write handle writes one byte,
        // and the readback comes back through the read view.
        let current = self.capture_of(expected.serial)?;
        if current.pid != expected.pid
            || current.creation_time != expected.creation_time
            || current.manager != expected.manager
            || current.data != expected.data
            || current.address != expected.address
            || current.record_hex != expected.record_hex
        {
            return Err(RuntimeError::CountInstanceChanged);
        }
        let mut writer = self.processes.open_write(self.pid)?;
        writer.write(
            expected.address + self.layout.count_offset as u64,
            &[desired],
        )?;
        writer.close();
        let record_size = self.layout.record_size;
        let reader = self.reader()?;
        reader.read(expected.address, record_size)
    }
}

impl<P: CountProcesses> WindowsCountMemory<P> {
    fn creation_time(&mut self) -> Result<String, RuntimeError> {
        let reader = self.reader()?;
        Ok(match reader.creation_filetime()? {
            Some(value) => value.to_string(),
            None => "unknown".to_string(),
        })
    }
}

fn read_u64(
    reader: &mut Box<dyn crate::mutation::memory::TargetProcess>,
    address: u64,
) -> Result<u64, RuntimeError> {
    let raw = reader.read(address, 8)?;
    Ok(u64::from_le_bytes(raw[..8].try_into().map_err(|_| {
        RuntimeError::MemoryRead {
            address,
            size: 8,
            code: 0,
        }
    })?))
}

/// The Windows opener: a read view and, per explicit write, the minimal write
/// handle.
#[cfg(windows)]
pub struct WindowsCountProcesses;

#[cfg(windows)]
impl CountProcesses for WindowsCountProcesses {
    fn open_read(
        &self,
        pid: u32,
    ) -> Result<Box<dyn crate::mutation::memory::TargetProcess>, RuntimeError> {
        Ok(Box::new(
            crate::mutation::memory::WindowsProcess::open_read(pid)?,
        ))
    }

    fn open_write(
        &self,
        pid: u32,
    ) -> Result<Box<dyn crate::mutation::memory::TargetProcess>, RuntimeError> {
        Ok(Box::new(
            crate::mutation::memory::WindowsProcess::open_count_write(pid)?,
        ))
    }
}

/// The concrete product adapter the protected host will use.
#[cfg(windows)]
pub type WindowsCountMemoryAdapter = WindowsCountMemory<WindowsCountProcesses>;

/// One reviewed count plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountPlan {
    pub operation_id: String,
    pub target: TargetCapture,
    pub new_count: u8,
    pub old_count: u8,
    pub seed: u32,
    pub rarity: u8,
    pub save_path: String,
    pub source_sha256: String,
    pub backup_path: String,
}

impl CountPlan {
    /// The exact object the digest covers.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "operation_id": self.operation_id,
            "target": {
                "pid": self.target.pid,
                "creation_time": self.target.creation_time,
                "manager": self.target.manager,
                "data": self.target.data,
                "address": self.target.address,
                "record_hex": self.target.record_hex,
                "serial": self.target.serial,
            },
            "new_count": self.new_count,
            "old_count": self.old_count,
            "seed": self.seed,
            "rarity": self.rarity,
            "save_path": self.save_path,
            "source_sha256": self.source_sha256,
            "backup_path": self.backup_path,
        })
    }

    pub fn digest(&self) -> String {
        sha256_hex(canonical_json(&self.to_json()).as_bytes())
    }
}

/// Receipt state of one count edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountState {
    Prepared,
    Rejected,
    Uncertain,
    Verified,
}

impl CountState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Rejected => "rejected",
            Self::Uncertain => "uncertain",
            Self::Verified => "verified",
        }
    }
}

/// The `count_edit` object the protected host returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountStatus {
    pub operation_id: String,
    pub plan_digest: String,
    pub state: CountState,
    pub seed: u32,
    pub rarity: u8,
    pub old_count: u8,
    pub new_count: u8,
    pub error: Option<String>,
}

/// Plan, execute, status and recover one count edit under `root/count-edits`.
pub struct CountEditor {
    operations: PathBuf,
    memory: Box<dyn CountMemory>,
}

impl CountEditor {
    /// `root` is the runtime state root; the editor owns `count-edits` below it.
    pub fn new(root: &Path, memory: Box<dyn CountMemory>) -> Result<Self, RuntimeError> {
        let operations = root.join("count-edits");
        fs::create_dir_all(&operations).map_err(|error| RuntimeError::Io {
            path: operations.display().to_string(),
            detail: error.to_string(),
        })?;
        Ok(Self { operations, memory })
    }

    pub fn directory(&self, operation_id: &str) -> Result<PathBuf, RuntimeError> {
        if !is_canonical_uuid(operation_id) {
            return Err(RuntimeError::ReceiptConflict {
                detail: "Expected canonical operation UUID".to_string(),
            });
        }
        Ok(self.operations.join(operation_id))
    }

    /// The directory that holds every operation of this editor.
    pub fn operations_directory(&self) -> PathBuf {
        self.operations.clone()
    }

    /// Swap the record access, which is how a test drives recovery against a
    /// record that changed underneath the receipt.
    pub fn with_memory(mut self, memory: Box<dyn CountMemory>) -> Self {
        self.memory = memory;
        self
    }

    /// Port of `plan`.
    pub fn plan(&self, operation_id: &str) -> Result<(String, CountPlan), RuntimeError> {
        let path = self.directory(operation_id)?.join("plan.json");
        let text = fs::read_to_string(&path).map_err(|error| RuntimeError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
        let envelope: serde_json::Value =
            serde_json::from_str(&text).map_err(|error| RuntimeError::ReceiptConflict {
                detail: format!("count plan is not valid JSON: {error}"),
            })?;
        let digest = envelope
            .get("digest")
            .and_then(|value| value.as_str())
            .ok_or_else(|| RuntimeError::ReceiptConflict {
                detail: "Count plan changed".to_string(),
            })?
            .to_string();
        let plan =
            plan_from_json(
                envelope
                    .get("plan")
                    .ok_or_else(|| RuntimeError::ReceiptConflict {
                        detail: "Count plan changed".to_string(),
                    })?,
            )?;
        if sha256_hex(canonical_json(&plan.to_json()).as_bytes()) != digest {
            return Err(RuntimeError::ReceiptConflict {
                detail: "Count plan changed".to_string(),
            });
        }
        Ok((digest, plan))
    }

    /// Port of `prepare`.
    ///
    /// The automatic backup is verified, not created: producing it is the save
    /// side's transaction.
    pub fn prepare(
        &mut self,
        save_path: &Path,
        source_sha256: &str,
        record_hex: &str,
        backup_path: &Path,
        new_count: i64,
    ) -> Result<CountStatus, RuntimeError> {
        let new_count = checked_count(new_count)?;
        let saved_record = hex_decode(record_hex)?;
        if saved_record.len() != RECORD_SIZE {
            return Err(RuntimeError::CountSourceChanged {
                detail: "Expected full saved record".to_string(),
            });
        }
        let serial = u64::from_le_bytes(
            saved_record[SERIAL_OFFSET..SERIAL_OFFSET + 8]
                .try_into()
                .map_err(|_| RuntimeError::CountSourceChanged {
                    detail: "Expected full saved record".to_string(),
                })?,
        );
        let state = self.memory.capture(serial)?;
        let current = hex_decode(&state.record_hex)?;
        if current.len() != RECORD_SIZE {
            return Err(RuntimeError::CountSourceChanged {
                detail: "Current record is not a full scroll record".to_string(),
            });
        }
        if stable_identity(&current) != stable_identity(&saved_record) {
            return Err(RuntimeError::CountSourceChanged {
                detail: "Saved defined record fields differ".to_string(),
            });
        }
        if current[0x0E] != 0 {
            return Err(RuntimeError::CountSourceChanged {
                detail: "Current scroll state is not supported for count editing".to_string(),
            });
        }
        let raw = read_bytes(save_path)?;
        if sha256_hex(&raw).to_lowercase() != source_sha256.to_lowercase() {
            return Err(RuntimeError::CountSourceChanged {
                detail: "Save changed; refresh inventory".to_string(),
            });
        }
        let backup = read_bytes(backup_path)?;
        if sha256_hex(&backup) != sha256_hex(&raw) {
            return Err(RuntimeError::BackupMismatch {
                path: backup_path.display().to_string(),
            });
        }

        let operation_id = new_operation_id()?;
        let plan = CountPlan {
            operation_id: operation_id.clone(),
            target: state,
            new_count,
            old_count: current[COUNT_OFFSET],
            seed: u32::from_le_bytes(current[0x20..0x24].try_into().map_err(|_| {
                RuntimeError::CountSourceChanged {
                    detail: "Expected full saved record".to_string(),
                }
            })?),
            rarity: current[0x30],
            save_path: save_path
                .canonicalize()
                .map_err(|error| RuntimeError::Io {
                    path: save_path.display().to_string(),
                    detail: error.to_string(),
                })?
                .display()
                .to_string(),
            source_sha256: sha256_hex(&raw),
            backup_path: backup_path.display().to_string(),
        };
        let directory = self.directory(&operation_id)?;
        fs::create_dir_all(&directory).map_err(|error| RuntimeError::Io {
            path: directory.display().to_string(),
            detail: error.to_string(),
        })?;
        exclusive_json(
            &directory.join("plan.json"),
            &serde_json::json!({"digest": plan.digest(), "plan": plan.to_json()}),
        )?;
        self.status(&operation_id)
    }

    /// Port of `status`.
    pub fn status(&self, operation_id: &str) -> Result<CountStatus, RuntimeError> {
        let (digest, plan) = self.plan(operation_id)?;
        let directory = self.directory(operation_id)?;
        let mut state = CountState::Prepared;
        let mut error = None;
        let claim = directory.join("claim.json");
        if claim.exists() {
            let value = read_json(&claim)?;
            if value.get("digest").and_then(|value| value.as_str()) != Some(digest.as_str()) {
                return Err(RuntimeError::ReceiptConflict {
                    detail: "Count claim differs".to_string(),
                });
            }
            state = CountState::Uncertain;
        }
        let receipt = directory.join("receipt.json");
        if receipt.exists() {
            let value = read_json(&receipt)?;
            let matches =
                value.get("digest").and_then(|value| value.as_str()) == Some(digest.as_str());
            if !matches || state != CountState::Uncertain {
                return Err(RuntimeError::ReceiptConflict {
                    detail: "Count receipt differs".to_string(),
                });
            }
            state = match value.get("state").and_then(|value| value.as_str()) {
                Some("verified") => CountState::Verified,
                Some("rejected") => CountState::Rejected,
                _ => CountState::Uncertain,
            };
            error = value
                .get("error")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        let recovery = directory.join("recovery.json");
        if recovery.exists() {
            let value = read_json(&recovery)?;
            let verified = value.get("state").and_then(|value| value.as_str()) == Some("verified");
            if value.get("digest").and_then(|value| value.as_str()) != Some(digest.as_str())
                || state != CountState::Uncertain
                || !verified
            {
                return Err(RuntimeError::ReceiptConflict {
                    detail: "Count recovery differs".to_string(),
                });
            }
            state = CountState::Verified;
            error = None;
        }
        Ok(CountStatus {
            operation_id: operation_id.to_string(),
            plan_digest: digest,
            state,
            seed: plan.seed,
            rarity: plan.rarity,
            old_count: plan.old_count,
            new_count: plan.new_count,
            error,
        })
    }

    /// Port of `execute`. Never replays a claimed or resolved operation.
    pub fn execute(
        &mut self,
        operation_id: &str,
        plan_digest: &str,
    ) -> Result<CountStatus, RuntimeError> {
        let (digest, plan) = self.plan(operation_id)?;
        if digest != plan_digest {
            return Err(RuntimeError::ReceiptConflict {
                detail: "Reviewed count plan digest differs".to_string(),
            });
        }
        let previous = self.status(operation_id)?;
        if previous.state != CountState::Prepared {
            return Ok(previous);
        }
        let directory = self.directory(operation_id)?;
        exclusive_json(
            &directory.join("claim.json"),
            &serde_json::json!({"digest": digest}),
        )?;

        let (state, error) = match self.attempt(&plan) {
            Ok(after) => {
                exclusive_json(
                    &directory.join("verified-record.json"),
                    &serde_json::json!({"record_hex": after}),
                )?;
                (CountState::Verified, None)
            }
            Err(failure) => (
                if failure.attempted {
                    CountState::Uncertain
                } else {
                    CountState::Rejected
                },
                Some(failure.error.message()),
            ),
        };
        exclusive_json(
            &directory.join("receipt.json"),
            &serde_json::json!({"digest": digest, "state": state.as_str(), "error": error}),
        )?;
        self.status(operation_id)
    }

    fn attempt(&mut self, plan: &CountPlan) -> Result<String, AttemptFailure> {
        for path in [&plan.save_path, &plan.backup_path] {
            let bytes = read_bytes(Path::new(path)).map_err(|error| AttemptFailure {
                attempted: false,
                error,
            })?;
            if sha256_hex(&bytes) != plan.source_sha256 {
                return Err(AttemptFailure {
                    attempted: false,
                    error: RuntimeError::CountSourceChanged {
                        detail: "Save or automatic backup changed; prepare again".to_string(),
                    },
                });
            }
        }
        let current = self
            .memory
            .capture(plan.target.serial)
            .map_err(|error| AttemptFailure {
                attempted: false,
                error,
            })?;
        if current != plan.target {
            return Err(AttemptFailure {
                attempted: false,
                error: RuntimeError::CountSourceChanged {
                    detail: "Scroll or game state changed; prepare again".to_string(),
                },
            });
        }
        let after = self
            .memory
            .write(&current, plan.new_count)
            .map_err(|error| AttemptFailure {
                attempted: true,
                error,
            })?;
        let mut expected = hex_decode(&current.record_hex).map_err(|error| AttemptFailure {
            attempted: true,
            error,
        })?;
        expected[COUNT_OFFSET] = plan.new_count;
        if after != expected {
            return Err(AttemptFailure {
                attempted: true,
                error: RuntimeError::CountSourceChanged {
                    detail: "Count write readback differs; inspect before retrying".to_string(),
                },
            });
        }
        Ok(after.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    /// Port of `recover`: observation only, never a write.
    pub fn recover(&mut self, operation_id: &str) -> Result<CountStatus, RuntimeError> {
        let status = self.status(operation_id)?;
        if status.state != CountState::Uncertain {
            return Ok(status);
        }
        let (digest, plan) = self.plan(operation_id)?;
        let backup = read_bytes(Path::new(&plan.backup_path))?;
        if sha256_hex(&backup) != plan.source_sha256 {
            return Err(RuntimeError::BackupMismatch {
                path: plan.backup_path.clone(),
            });
        }
        let current = self.memory.capture(plan.target.serial)?;
        if !current.same_instance(&plan.target) {
            return Err(RuntimeError::CountInstanceChanged);
        }
        let raw = hex_decode(&current.record_hex)?;
        if raw[COUNT_OFFSET] != plan.new_count
            || stable_identity(&raw) != stable_identity(&hex_decode(&plan.target.record_hex)?)
        {
            return self.status(operation_id);
        }
        let directory = self.directory(operation_id)?;
        exclusive_json(
            &directory.join("recovery.json"),
            &serde_json::json!({"digest": digest, "state": "verified", "record_hex": current.record_hex}),
        )?;
        self.status(operation_id)
    }
}

struct AttemptFailure {
    attempted: bool,
    error: RuntimeError,
}

fn plan_from_json(value: &serde_json::Value) -> Result<CountPlan, RuntimeError> {
    let conflict = || RuntimeError::ReceiptConflict {
        detail: "Count plan changed".to_string(),
    };
    let target = value.get("target").ok_or_else(conflict)?;
    let text = |value: &serde_json::Value, key: &str| -> Result<String, RuntimeError> {
        value
            .get(key)
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .ok_or_else(conflict)
    };
    let number = |value: &serde_json::Value, key: &str| -> Result<u64, RuntimeError> {
        value
            .get(key)
            .and_then(|value| value.as_u64())
            .ok_or_else(conflict)
    };
    Ok(CountPlan {
        operation_id: text(value, "operation_id")?,
        target: TargetCapture {
            pid: number(target, "pid")? as u32,
            creation_time: text(target, "creation_time")?,
            manager: number(target, "manager")?,
            data: number(target, "data")?,
            address: number(target, "address")?,
            record_hex: text(target, "record_hex")?,
            serial: number(target, "serial")?,
        },
        new_count: number(value, "new_count")? as u8,
        old_count: number(value, "old_count")? as u8,
        seed: number(value, "seed")? as u32,
        rarity: number(value, "rarity")? as u8,
        save_path: text(value, "save_path")?,
        source_sha256: text(value, "source_sha256")?,
        backup_path: text(value, "backup_path")?,
    })
}

/// `live_add_operations.canonical`: sorted keys, compact, ASCII escaped.
pub fn canonical_json(value: &serde_json::Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::Null => out.push_str("null"),
        serde_json::Value::Bool(true) => out.push_str("true"),
        serde_json::Value::Bool(false) => out.push_str("false"),
        serde_json::Value::Number(number) => out.push_str(&number.to_string()),
        serde_json::Value::String(text) => write_canonical_string(text, out),
        serde_json::Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        serde_json::Value::Object(object) => {
            out.push('{');
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical_string(key, out);
                out.push(':');
                if let Some(item) = object.get(key) {
                    write_canonical(item, out);
                }
            }
            out.push('}');
        }
    }
}

/// Python `json.dumps(..., ensure_ascii=True)` string escaping.
fn write_canonical_string(text: &str, out: &mut String) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7E => {
                let code = c as u32;
                if code > 0xFFFF {
                    let adjusted = code - 0x1_0000;
                    let high = 0xD800 + (adjusted >> 10);
                    let low = 0xDC00 + (adjusted & 0x3FF);
                    out.push_str(&format!("\\u{high:04x}\\u{low:04x}"));
                } else {
                    out.push_str(&format!("\\u{code:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub(crate) fn exclusive_json(path: &Path, value: &serde_json::Value) -> Result<(), RuntimeError> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| RuntimeError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
    file.write_all(canonical_json(value).as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| RuntimeError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })
}

pub(crate) fn read_json(path: &Path) -> Result<serde_json::Value, RuntimeError> {
    let text = fs::read_to_string(path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    serde_json::from_str(&text).map_err(|error| RuntimeError::ReceiptConflict {
        detail: format!("{} is not valid JSON: {error}", path.display()),
    })
}

pub(crate) fn read_bytes(path: &Path) -> Result<Vec<u8>, RuntimeError> {
    fs::read(path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut rendered = String::with_capacity(64);
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

fn hex_decode(text: &str) -> Result<Vec<u8>, RuntimeError> {
    if !text.len().is_multiple_of(2) {
        return Err(RuntimeError::CountSourceChanged {
            detail: "record hex has an odd length".to_string(),
        });
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    let digits: Vec<char> = text.chars().collect();
    for pair in digits.chunks(2) {
        let high = pair[0]
            .to_digit(16)
            .ok_or_else(|| RuntimeError::CountSourceChanged {
                detail: "record hex is not hexadecimal".to_string(),
            })?;
        let low = pair[1]
            .to_digit(16)
            .ok_or_else(|| RuntimeError::CountSourceChanged {
                detail: "record hex is not hexadecimal".to_string(),
            })?;
        bytes.push((high * 16 + low) as u8);
    }
    Ok(bytes)
}

pub(crate) fn is_canonical_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(index, character)| match index {
            8 | 13 | 18 | 23 => character == '-',
            _ => character.is_ascii_hexdigit() && !character.is_ascii_uppercase(),
        })
}

/// A UUID v4 in canonical lowercase form, from the operating system entropy
/// pool; the crate carries no uuid dependency.
pub(crate) fn new_operation_id() -> Result<String, RuntimeError> {
    let mut bytes = [0u8; 16];
    getrandom(&mut bytes)?;
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

/// `BCryptGenRandom` on Windows, `/dev/urandom` elsewhere.
#[cfg(windows)]
fn getrandom(buffer: &mut [u8]) -> Result<(), RuntimeError> {
    // `RtlGenRandom` (SystemFunction036) is available without any extra crate.
    #[link(name = "advapi32")]
    extern "system" {
        fn SystemFunction036(buffer: *mut u8, length: u32) -> u8;
    }
    let ok = unsafe { SystemFunction036(buffer.as_mut_ptr(), buffer.len() as u32) };
    if ok == 0 {
        return Err(RuntimeError::EntropyUnavailable);
    }
    Ok(())
}

#[cfg(not(windows))]
fn getrandom(buffer: &mut [u8]) -> Result<(), RuntimeError> {
    use std::io::Read;
    let mut source =
        fs::File::open("/dev/urandom").map_err(|_| RuntimeError::EntropyUnavailable)?;
    source
        .read_exact(buffer)
        .map_err(|_| RuntimeError::EntropyUnavailable)
}
