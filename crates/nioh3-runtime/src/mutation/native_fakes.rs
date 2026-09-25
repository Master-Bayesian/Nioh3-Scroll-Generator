//! Injected transports for the native executor and the batch oracle.
//!
//! Compiled only for `cfg(test)` or with the off-by-default `test-fake`
//! feature, so the shipped library keeps exactly one process implementation.
//! The fixture is byte-addressable, so the executor's real gates (dispatch
//! signature, inventory capture, capacity, serial, slot) run over the same
//! shapes a game process produces, without a game process.

use crate::error::RuntimeError;
use crate::mutation::descriptor::verify_assembly_preview;
use crate::mutation::evidence::{
    preview_owner_fingerprint, preview_rejection_receipt, verify_dispatch, PREVIEW_PHASE_AFTER,
    PREVIEW_PHASE_BEFORE,
};
use crate::mutation::inventory::{hex_decode, RECORD_SIZE};
use crate::mutation::live_add::LIVE_ADD_DISPLAY_VERSION;
use crate::mutation::live_fakes::{
    ByteMemory, InventoryFixture, FIXTURE_BASE, FIXTURE_PID, FIXTURE_PROFILE_ID,
};
use crate::mutation::native_abi::{hex, LiveAddLayout, DISPATCH_SOURCE_OFFSET, PC_V201_LIVE_ADD};
use crate::mutation::native_executor::{DispatchMode, LiveAddTransport, ReceiptStore};
use crate::mutation::win_session::{
    DebugEvent, DebugSession, OwnerThreadSnapshot, RemoteSession, RuntimeOwnerSession,
    RuntimeOwnerSnapshot, ThreadContext, EXCEPTION_BREAKPOINT, EXCEPTION_SINGLE_STEP,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Every injected native failure the executor gates use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeFaults {
    /// The insertion happened, the receipt was never acknowledged.
    pub reply_lost: bool,
    /// The isolated preview's builder output differs from the reviewed record.
    pub preview_mismatch: bool,
    /// The preview leaves an unsettled receipt that still owns the target.
    pub preview_incomplete: bool,
    /// The periodic idle window was missed: nothing was dispatched.
    pub idle_miss: bool,
    /// The submission never reached the target: no receipt exists.
    pub reject_before_dispatch: bool,
    /// The destination write is skipped so recovery sees a proven absence.
    pub skip_destination: bool,
    /// R1: the durable record is written settled while the process that owns
    /// the allocation and the debug session is still alive (its durable entry
    /// was deregistered before the owner finished). The receipt is not proof.
    pub settled_receipt_with_live_owner: bool,
}

/// A transport over one fixture process image.
pub struct FakeLiveAddTransport {
    pub fixture: InventoryFixture,
    pub layout: LiveAddLayout,
    /// Exact executable identity this transport claims, when it claims one.
    pub executable_sha256: Option<String>,
    /// Every target read this transport served, so a test can prove that a
    /// refused binding never touched the target.
    pub reads: u64,
    pub store: ReceiptStore,
    pub faults: NativeFaults,
    pub submissions: Vec<String>,
    pub module_name: String,
    pub current_creation_time: String,
    /// The admitted operation whose allocation and debug session this transport
    /// still owns. A durable receipt is a claim either way, so ownership lives
    /// here and outlives a receipt that was rewritten as settled.
    pub live_owner: Option<String>,
}

impl FakeLiveAddTransport {
    pub fn new(directory: &Path, fixture: InventoryFixture) -> Result<Self, RuntimeError> {
        Self::with_faults(directory, fixture, NativeFaults::default())
    }

    pub fn with_faults(
        directory: &Path,
        fixture: InventoryFixture,
        faults: NativeFaults,
    ) -> Result<Self, RuntimeError> {
        Self::with_layout_faults(directory, fixture, PC_V201_LIVE_ADD, None, faults)
    }

    /// The same injected transport over any accepted layout, claiming an exact
    /// executable identity when the caller supplies one.
    pub fn with_layout_faults(
        directory: &Path,
        fixture: InventoryFixture,
        layout: LiveAddLayout,
        executable_sha256: Option<&str>,
        faults: NativeFaults,
    ) -> Result<Self, RuntimeError> {
        let mut fixture = fixture;
        // The dispatch shim's own contract site: the seven signature bytes.
        fixture.memory.write(
            fixture.base + layout.dispatch_rva,
            &layout.dispatch_signature,
        );
        // The mission scheduler owner the plan pins, plus its accepted idle
        // phase, so the executor's own re-checks resolve.
        let scheduler = fixture.base + 0x90_0000;
        fixture.memory.write(
            fixture.base + layout.scheduler_pointer_rva,
            &scheduler.to_le_bytes(),
        );
        fixture
            .memory
            .write(scheduler + layout.scheduler_pending_offset, &[0u8; 4]);
        fixture
            .memory
            .write(scheduler + layout.scheduler_ready_offset, &[1u8]);
        let builder = vec![0xB8u8; layout.builder_size as usize];
        // The insertion site starts with the approved signature the inventory
        // capture gates on; the rest is the executor's own stub body.
        let mut insertion = vec![0xC3u8; layout.insertion_size as usize];
        insertion[..crate::mutation::inventory::INSERTION_SIGNATURE.len()]
            .copy_from_slice(&crate::mutation::inventory::INSERTION_SIGNATURE);
        fixture
            .memory
            .write(fixture.base + layout.builder_rva, &builder);
        fixture
            .memory
            .write(fixture.base + layout.insertion_rva, &insertion);
        // A layout with a reviewed ambient-identity chain gets its exact code
        // and no online session, so the builder identity reads as offline.
        if let Some(identity) = crate::mutation::native_abi::builder_identity_for(&layout) {
            for (rva, code) in identity.code {
                fixture.memory.write(
                    fixture.base + rva,
                    &crate::mutation::inventory::hex_decode(code)?,
                );
            }
            fixture.memory.write(
                fixture.base + identity.session_pointer_rva,
                &0u64.to_le_bytes(),
            );
        }
        let current = fixture.creation_time.to_string();
        Ok(Self {
            fixture,
            layout,
            executable_sha256: executable_sha256.map(str::to_string),
            reads: 0,
            store: ReceiptStore::new(directory)?,
            faults,
            submissions: Vec::new(),
            module_name: "Nioh3.exe".to_string(),
            current_creation_time: current,
            live_owner: None,
        })
    }

    /// The account/slot shape the save side validates, plus a container.
    pub fn save_path(&self, root: &Path) -> PathBuf {
        root.join("76561198000000000")
            .join("SAVEDATA00")
            .join("SAVEDATA.BIN")
    }

    fn receipt(&self, operation_id: &str) -> Result<Value, RuntimeError> {
        self.store.read(operation_id)
    }

    /// The status a native dispatch would leave behind.
    fn write_receipt(&self, receipt: &Value) -> Result<(), RuntimeError> {
        self.store.save(receipt)
    }

    /// `_run`'s observable effect: the accepted insertion.
    fn apply_insertion(&mut self, params: &Value, source: &[u8]) -> Result<Vec<u8>, RuntimeError> {
        let slot = params.get("slot").and_then(Value::as_u64).unwrap_or(0) as usize;
        let serial = params.get("serial").and_then(Value::as_u64).unwrap_or(0);
        let (before, _) = self.fixture.capture()?;
        let mut destination = source.to_vec();
        let flags = u32::from_le_bytes([
            destination[0x18],
            destination[0x19],
            destination[0x1A],
            destination[0x1B],
        ]) | 0x0400_0080;
        destination[0x18..0x1C].copy_from_slice(&flags.to_le_bytes());
        destination[0x1C..0x20].copy_from_slice(&before.acquisition_order_counter.to_le_bytes());
        destination[0x28..0x30].copy_from_slice(&serial.to_le_bytes());
        let data = self.fixture.data()?;
        let container = data + self.fixture.layout.container_offset;
        self.fixture
            .memory
            .write(container + (slot * RECORD_SIZE) as u64, &destination);
        self.fixture
            .memory
            .write(data, &(before.acquisition_order_counter + 1).to_le_bytes());
        self.fixture
            .memory
            .write(data + 8, &(serial + 1).to_le_bytes());
        self.fixture.refresh_index()?;
        Ok(destination)
    }
}

impl LiveAddTransport for FakeLiveAddTransport {
    fn pid(&self) -> u32 {
        FIXTURE_PID
    }

    fn profile_id(&self) -> &'static str {
        self.layout.profile_id
    }

    fn module_base(&mut self) -> Result<u64, RuntimeError> {
        Ok(self.fixture.base)
    }

    fn creation_time(&mut self) -> Result<String, RuntimeError> {
        Ok(self.current_creation_time.clone())
    }

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        self.reads += 1;
        self.fixture.memory.read(address, size)
    }

    fn executable_sha256(&mut self) -> Result<Option<String>, RuntimeError> {
        Ok(self.executable_sha256.clone())
    }

    fn ping(&mut self) -> Result<Value, RuntimeError> {
        let unresolved_owner = self.store.unresolved_owner()?;
        Ok(json!({
            "pid": FIXTURE_PID,
            "profile_id": self.layout.profile_id,
            "busy": self.live_owner.is_some() || unresolved_owner.is_some(),
            "unresolved_operation_id": unresolved_owner,
        }))
    }

    fn dispatch(&mut self, mode: DispatchMode, params: &Value) -> Result<Value, RuntimeError> {
        let operation_id = params
            .get("operation_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.submissions.push(operation_id.clone());
        if self.faults.reject_before_dispatch {
            // The submission never reached the target: no receipt exists, so
            // ownership is provably absent.
            return Err(RuntimeError::NativeDispatch {
                detail: "submission rejected before dispatch".to_string(),
            });
        }
        if let Some(unresolved) = self.store.unresolved_owner()? {
            return Err(RuntimeError::NativeDispatch {
                detail: format!(
                    "Previous native operation {unresolved} is unresolved; recover it, never replay"
                ),
            });
        }
        if let Some(owner) = &self.live_owner {
            // The real binding refuses a dispatch while it still owns an
            // allocation or a debug session, even when the durable record of
            // that owner was already rewritten as settled.
            return Err(RuntimeError::NativeDispatch {
                detail: format!(
                    "Previous native operation {owner} is unresolved; recover it, never replay"
                ),
            });
        }
        if self.store.exists(&operation_id) {
            return Err(RuntimeError::NativeDispatch {
                detail: "Operation already submitted or executor is occupied".to_string(),
            });
        }
        let creation = self.current_creation_time.clone();
        let mut receipt = json!({
            "operation_id": operation_id,
            "pid": FIXTURE_PID,
            "process_creation_time": creation,
            "phase": "preparing",
            "active": true,
            "released": false,
            "redirect_count": 0,
            "breakpoint_count": -1,
            "business_outcome": "pending",
            "remote_execution": "not_started",
            "allocation_state": "not_allocated",
            "debugger_state": "not_attached",
            "thread_cleanup": {},
            "executor": "windows-native",
            "mode": mode.receipt_mode(),
            "serial": params.get("serial").cloned().unwrap_or(Value::Null),
            "slot": params.get("slot").cloned().unwrap_or(Value::Null),
            "candidate_id": params.get("candidate_id").cloned().unwrap_or(Value::Null),
            "parent_operation_id": params
                .get("parent_operation_id")
                .cloned()
                .unwrap_or(Value::Null),
        });
        self.write_receipt(&receipt)?;
        if mode == DispatchMode::Preview
            && (self.faults.preview_mismatch || self.faults.preview_incomplete)
        {
            let assembly = hex_decode(
                params
                    .get("expected_record_hex")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )?;
            let mut source = assembly;
            source[0x28..0x30].copy_from_slice(&[0xFF; 8]);
            let creation = self.current_creation_time.clone();
            if self.faults.preview_mismatch {
                // The isolated builder output differs from the reviewed record:
                // the run settles as a formal rejection with every proof present.
                source[0x20] ^= 0xFF;
                // The fixture's own native index through the shipped traversal, so
                // the fixture's index evidence is the real index too.
                let (inventory, index) = self.fixture.capture()?;
                let native_index = index.to_json();
                let before = preview_owner_fingerprint(
                    &self.fixture.container()?,
                    self.fixture.serial_counter()?,
                    inventory.acquisition_order_counter,
                    &self.fixture.layout,
                    FIXTURE_PROFILE_ID,
                    FIXTURE_PID,
                    &creation,
                    self.fixture.manager()?,
                    self.fixture.data()?,
                    FIXTURE_BASE,
                    &native_index,
                    PREVIEW_PHASE_BEFORE,
                );
                let after = preview_owner_fingerprint(
                    &self.fixture.container()?,
                    self.fixture.serial_counter()?,
                    inventory.acquisition_order_counter,
                    &self.fixture.layout,
                    FIXTURE_PROFILE_ID,
                    FIXTURE_PID,
                    &creation,
                    self.fixture.manager()?,
                    self.fixture.data()?,
                    FIXTURE_BASE,
                    &native_index,
                    PREVIEW_PHASE_AFTER,
                );
                let rejected = preview_rejection_receipt(
                    &operation_id,
                    params.get("parent_operation_id").and_then(Value::as_str),
                    FIXTURE_PID,
                    &creation,
                    params
                        .get("descriptor_hex")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    params
                        .get("expected_record_hex")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    params
                        .get("builder_code_hex")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    &source,
                    before,
                    after,
                )?;
                self.write_receipt(&rejected)?;
                return Ok(rejected);
            }
            // An unsettled preview: the target owner is not proven released, so
            // the child must stay blocked instead of being settled.
            if let Some(object) = receipt.as_object_mut() {
                object.insert("phase".to_string(), json!("uncertain"));
                object.insert("released".to_string(), json!(false));
                object.insert("active".to_string(), json!(false));
                object.insert("business_outcome".to_string(), json!("unknown"));
                object.insert("remote_execution".to_string(), json!("unknown"));
                object.insert("allocation_state".to_string(), json!("retained"));
                object.insert("debugger_state".to_string(), json!("unknown"));
                object.insert("breakpoint_count".to_string(), json!(-1));
                object.insert("source_hex".to_string(), json!(hex(&source)));
            }
            self.live_owner = Some(operation_id.clone());
            self.write_receipt(&receipt)?;
            return Err(RuntimeError::NativeDispatch {
                detail: "native preview result is uncertain; query the receipt".to_string(),
            });
        }
        if self.faults.idle_miss {
            if let Some(object) = receipt.as_object_mut() {
                object.insert("phase".to_string(), json!("rejected"));
                object.insert("active".to_string(), json!(false));
                object.insert("released".to_string(), json!(true));
                object.insert("breakpoint_count".to_string(), json!(0));
                object.insert("business_outcome".to_string(), json!("rejected"));
                object.insert("remote_execution".to_string(), json!("not_started"));
                object.insert("allocation_state".to_string(), json!("freed"));
                object.insert("debugger_state".to_string(), json!("detached"));
                object.insert("thread_cleanup".to_string(), json!({}));
                object.insert(
                    "error".to_string(),
                    json!("No accepted idle dispatch before timeout"),
                );
            }
            self.write_receipt(&receipt)?;
            return Ok(receipt);
        }
        let assembly = hex_decode(
            params
                .get("expected_record_hex")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )?;
        let source = if mode == DispatchMode::Insert {
            let mut source = assembly.clone();
            let serial = params.get("serial").and_then(Value::as_u64).unwrap_or(0);
            source[0x28..0x30].copy_from_slice(&serial.to_le_bytes());
            source
        } else {
            let mut source = assembly.clone();
            source[0x28..0x30].copy_from_slice(&[0xFF; 8]);
            verify_assembly_preview(&assembly, &source)?;
            source
        };
        let mut frame = frame_for(self.fixture.base + self.layout.insertion_rva, 0);
        if let Some(object) = frame.as_object_mut() {
            object.insert("phase".to_string(), json!("completed"));
            object.insert("redirect_count".to_string(), json!(1));
            object.insert("released".to_string(), json!(true));
            object.insert("active".to_string(), json!(false));
            object.insert("breakpoint_count".to_string(), json!(0));
            object.insert("breakpoints".to_string(), json!([]));
            object.insert(
                "business_outcome".to_string(),
                json!(if mode == DispatchMode::Insert {
                    "committed"
                } else {
                    "completed"
                }),
            );
            object.insert("remote_execution".to_string(), json!("quiescent"));
            object.insert("allocation_state".to_string(), json!("freed"));
            object.insert("debugger_state".to_string(), json!("detached"));
            object.insert("thread_cleanup".to_string(), json!({}));
            object.insert("mode".to_string(), json!(mode.receipt_mode()));
            object.insert("status".to_string(), json!(3));
            object.insert("source_hex".to_string(), json!(hex(&source)));
            object.insert("process_creation_time".to_string(), json!(creation));
        }
        if mode == DispatchMode::Insert && !self.faults.skip_destination {
            let destination = self.apply_insertion(params, &source)?;
            if let Some(object) = frame.as_object_mut() {
                object.insert("destination_hex".to_string(), json!(hex(&destination)));
                object.insert(
                    "remainder_hex".to_string(),
                    json!(hex(&vec![0u8; RECORD_SIZE])),
                );
                object.insert(
                    "slot".to_string(),
                    params.get("slot").cloned().unwrap_or(Value::Null),
                );
            }
        }
        verify_dispatch(&frame)?;
        // The receipt's own identity survives the dispatch; the frame only adds
        // the observations it recorded.
        if let (Some(target), Some(observed)) = (receipt.as_object_mut(), frame.as_object()) {
            for (key, value) in observed {
                target.insert(key.clone(), value.clone());
            }
        }
        if self.faults.reply_lost {
            // The insertion is on disk; the acknowledgement never arrived, so
            // ownership stays unresolved until a verified recovery.
            if let Some(object) = receipt.as_object_mut() {
                object.insert("phase".to_string(), json!("uncertain"));
                object.insert("released".to_string(), json!(false));
                object.insert("active".to_string(), json!(false));
                object.insert("breakpoint_count".to_string(), json!(-1));
                object.insert("remote_execution".to_string(), json!("unknown"));
                object.insert("allocation_state".to_string(), json!("retained"));
                object.insert("debugger_state".to_string(), json!("unknown"));
                object.insert(
                    "thread_cleanup".to_string(),
                    json!({"unknown": {"cleanup_state": "unknown"}}),
                );
            }
            self.live_owner = Some(operation_id);
            self.write_receipt(&receipt)?;
            return Err(RuntimeError::NativeDispatch {
                detail: "native reply lost; query the receipt".to_string(),
            });
        }
        if self.faults.settled_receipt_with_live_owner {
            // R1 window: the receipt reads settled (the reaper deregistered its
            // durable entry) while this transport still owns the allocation and
            // the debug session. A receipt cannot release that owner.
            self.live_owner = Some(operation_id);
            self.write_receipt(&receipt)?;
            return Err(RuntimeError::NativeDispatch {
                detail: "native reply lost; query the receipt".to_string(),
            });
        }
        self.write_receipt(&receipt)?;
        Ok(receipt)
    }

    fn status(&mut self, operation_id: &str) -> Result<Value, RuntimeError> {
        if self.store.exists(operation_id) {
            let receipt = self.receipt(operation_id)?;
            return Ok(self.store.authoritative_state(&receipt)?.unwrap_or(receipt));
        }
        Err(RuntimeError::NativeDispatch {
            detail: "Unknown native operation".to_string(),
        })
    }

    /// Verification only, exactly like the real transport: the destination
    /// record plus the advanced serial prove the outcome, or its absence does.
    fn release(&mut self, operation_id: &str) -> Result<Value, RuntimeError> {
        let mut value = self.store.read(operation_id)?;
        if let Some(terminal) = self.store.authoritative_state(&value)? {
            return Ok(terminal);
        }
        let slot = value.get("slot").and_then(Value::as_u64).unwrap_or(0) as usize;
        let planned = value.get("serial").and_then(Value::as_u64).unwrap_or(0);
        let data = self.fixture.data()?;
        let container = self.fixture.container()?;
        let serial = self.fixture.serial_counter()?;
        let record = container[slot * RECORD_SIZE..(slot + 1) * RECORD_SIZE].to_vec();
        let occupied = record[0] != 0 || record[1] != 0;
        if occupied && serial > planned {
            if let Some(object) = value.as_object_mut() {
                object.insert("phase".to_string(), json!("completed"));
                object.insert("active".to_string(), json!(false));
                object.insert("business_outcome".to_string(), json!("committed"));
                object.insert(
                    "recovered_by".to_string(),
                    json!("destination_record_and_serial"),
                );
                object.insert("destination_hex".to_string(), json!(hex(&record)));
            }
            let _ = data;
            self.store.save(&value)?;
            return Ok(value);
        }
        if !occupied && serial == planned {
            if let Some(object) = value.as_object_mut() {
                object.insert("phase".to_string(), json!("rejected"));
                object.insert("active".to_string(), json!(false));
                object.insert("business_outcome".to_string(), json!("rejected"));
                object.insert("recovered_by".to_string(), json!("proven_absence"));
            }
            self.store.save(&value)?;
            return Ok(value);
        }
        Err(RuntimeError::NativeDispatch {
            detail: "Previous executor ownership is unresolved; never replay this operation"
                .to_string(),
        })
    }

    fn operation_known(&mut self, operation_id: &str) -> bool {
        self.store.exists(operation_id)
    }

    fn owner_retained(&mut self) -> bool {
        self.live_owner.is_some()
            || self
                .store
                .unresolved_owner()
                .map(|owner| owner.is_some())
                .unwrap_or(true)
    }

    fn safe_to_shutdown(&mut self) -> bool {
        !self.owner_retained()
    }

    /// Every durable preview receipt this parent owns, read-only.
    fn preview_receipts(&mut self, parent_operation_id: &str) -> Result<Vec<Value>, RuntimeError> {
        Ok(self
            .store
            .all()?
            .into_iter()
            .filter(|receipt| {
                receipt.get("mode").and_then(Value::as_str) == Some("preview")
                    && receipt.get("parent_operation_id").and_then(Value::as_str)
                        == Some(parent_operation_id)
            })
            .collect())
    }
}

/// `dispatch_evidence.verify_dispatch`'s accepted register frame.
pub fn frame_for(function_address: u64, _entry: u64) -> Value {
    let before_rsp = 0x1000_0008u64;
    let after_rsp = before_rsp - 0x48;
    let left = before_rsp - 16;
    let result = left - 0x38;
    let low = (result & 0xFF) as u8;
    let eflags = u64::from(left < 0x38)
        | u64::from(low.count_ones().is_multiple_of(2)) << 2
        | u64::from((left ^ 0x38 ^ result) & 16 != 0) << 4
        | u64::from(result == 0) << 6
        | ((result >> 63) & 1) << 7
        | u64::from((left ^ 0x38) & (left ^ result) & (1u64 << 63) != 0) << 11;
    let mut before = serde_json::Map::new();
    let mut after = serde_json::Map::new();
    for name in crate::mutation::evidence::REGISTERS {
        let value = 0x4141_0000_0000 + name.len() as u64;
        before.insert(name.to_string(), json!(value));
        after.insert(name.to_string(), json!(value));
    }
    before.insert("RSP".to_string(), json!(before_rsp));
    before.insert("RIP".to_string(), json!(function_address));
    before.insert("EFLAGS".to_string(), json!(0x202u64));
    after.insert("RSP".to_string(), json!(after_rsp));
    after.insert("RIP".to_string(), json!(function_address + 7));
    after.insert("EFLAGS".to_string(), json!(0x202u64 & !0x8D5u64 | eflags));
    json!({"before": before, "after": after})
}

/// The one deferred mutation a scripted session applies at the acknowledgement.
///
/// It exists so a test can present the shim's own builder output, drift the
/// container between the stopped baseline and the acknowledgement, or fail the
/// acknowledgement's container read, all through the shipped dispatch loop.
#[derive(Debug, Clone, Default)]
pub struct FakeSessionScript {
    /// The acknowledgement thread whose first context read applies the script.
    pub ack_tid: u32,
    /// Bytes written over the shim's source area when the script applies.
    pub source: Option<Vec<u8>>,
    /// A container byte written when the script applies.
    pub container_byte: Option<(u64, u8)>,
    /// Fail the container-sized read that follows the acknowledgement.
    pub fail_container_read: bool,
    /// Set once the script has run.
    pub applied: bool,
}

/// A byte-addressable debug session: no syscall, same contract.
pub struct FakeDebugSession {
    pub pid: u32,
    pub memory: ByteMemory,
    pub base: u64,
    pub creation_time: u64,
    pub attached: bool,
    pub events: std::collections::VecDeque<DebugEvent>,
    pub contexts: std::collections::BTreeMap<u32, ThreadContext>,
    pub owner: FakeRuntimeOwner,
    pub freed: Vec<u64>,
    /// The deferred acknowledgement script, when a test provides one.
    pub script: Option<FakeSessionScript>,
}

impl FakeDebugSession {
    pub fn new(fixture: &InventoryFixture) -> Self {
        Self {
            pid: FIXTURE_PID,
            memory: fixture.memory.clone(),
            base: FIXTURE_BASE,
            creation_time: fixture.creation_time,
            attached: false,
            events: std::collections::VecDeque::new(),
            contexts: std::collections::BTreeMap::new(),
            owner: FakeRuntimeOwner::default(),
            freed: Vec::new(),
            script: None,
        }
    }

    pub fn from_parts(pid: u32, memory: ByteMemory, base: u64, creation_time: u64) -> Self {
        Self {
            pid,
            memory,
            base,
            creation_time,
            attached: false,
            events: std::collections::VecDeque::new(),
            contexts: std::collections::BTreeMap::new(),
            owner: FakeRuntimeOwner::default(),
            freed: Vec::new(),
            script: None,
        }
    }
}

impl DebugSession for FakeDebugSession {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn creation_time(&mut self) -> Result<String, RuntimeError> {
        Ok(self.creation_time.to_string())
    }

    fn module_base(&mut self) -> Result<u64, RuntimeError> {
        Ok(self.base)
    }

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        if let Some(script) = &self.script {
            if script.fail_container_read
                && script.applied
                && size == crate::mutation::inventory::CAPACITY as usize * RECORD_SIZE
            {
                return Err(RuntimeError::NativeDispatch {
                    detail: "the acknowledgement's container read failed".to_string(),
                });
            }
        }
        self.memory.read(address, size)
    }

    fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
        self.memory.write(address, data);
        Ok(())
    }

    fn allocate(&mut self, size: usize) -> Result<u64, RuntimeError> {
        let address = self.base + 0x1_0000_0000;
        self.memory.write(address, &vec![0u8; size]);
        Ok(address)
    }

    fn free(&mut self, address: u64) -> Result<(), RuntimeError> {
        self.freed.push(address);
        Ok(())
    }

    fn flush_instruction_cache(&mut self, _address: u64, _size: usize) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn attach(&mut self) -> Result<(), RuntimeError> {
        self.attached = true;
        self.owner.attach();
        Ok(())
    }

    fn attached(&self) -> bool {
        self.attached
    }

    fn detach(&mut self) -> Result<(), RuntimeError> {
        let result = self.owner.detach();
        self.attached = self.owner.attached;
        result
    }

    fn wait(&mut self, _milliseconds: u32) -> Result<Option<DebugEvent>, RuntimeError> {
        let Some(event) = self.events.pop_front() else {
            return Ok(None);
        };
        self.contexts.entry(event.tid).or_default();
        self.owner.begin_event(event)?;
        if event.exception_code == Some(EXCEPTION_SINGLE_STEP) {
            if let Some(context) = self.contexts.get_mut(&event.tid) {
                if context.rip == context.dr0 {
                    context.dr6 |= 1;
                }
                if context.rip == context.dr1 {
                    context.dr6 |= 2;
                }
            }
        }
        Ok(Some(event))
    }

    fn resume(&mut self, event: &DebugEvent, handled: bool) -> Result<(), RuntimeError> {
        if self.owner.current_event != Some(*event) {
            return Err(RuntimeError::NativeDispatch {
                detail: "fake continuation does not match the pending event".to_string(),
            });
        }
        self.owner.continue_event(handled)?;
        if event.is_exit_thread() {
            self.owner.retire_exit_thread(event.tid)?;
        }
        Ok(())
    }

    fn context(&mut self, tid: u32) -> Result<ThreadContext, RuntimeError> {
        self.owner.context(tid)?;
        self.contexts
            .get(&tid)
            .copied()
            .ok_or(RuntimeError::SessionNotOpen)
    }

    fn set_context(&mut self, tid: u32, context: &ThreadContext) -> Result<(), RuntimeError> {
        self.owner.set_context(tid, context)?;
        if let Some(mut script) = self.script.clone() {
            let pending_acknowledgement = self.owner.current_event.as_ref().is_some_and(|event| {
                event.tid == tid && event.exception_code == Some(EXCEPTION_SINGLE_STEP)
            });
            if !script.applied && tid == script.ack_tid && pending_acknowledgement {
                // Apply between the acknowledgement's context and its reads, so
                // the stopped baseline has already been captured.
                if let Some(source) = &script.source {
                    self.memory
                        .write(self.base + 0x1_0000_0000 + DISPATCH_SOURCE_OFFSET, source);
                }
                if let Some((address, value)) = script.container_byte {
                    self.memory.write(address, &[value]);
                }
                script.applied = true;
                self.script = Some(script);
            }
        }
        self.contexts.insert(tid, *context);
        Ok(())
    }

    fn adopt_thread(&mut self, tid: u32, handle: u64) -> Result<(), RuntimeError> {
        let context = self.contexts.get(&tid).copied().unwrap_or_default();
        self.owner.adopt_thread(tid, handle, context)?;
        Ok(())
    }

    fn arm_thread(
        &mut self,
        tid: u32,
        entry: u64,
        acknowledgement: u64,
    ) -> Result<(), RuntimeError> {
        let mut context = self.context(tid)?;
        if context.dr7 & 0xFF != 0 {
            return Err(RuntimeError::NativeDispatch {
                detail: "a fake thread already has active hardware breakpoints".to_string(),
            });
        }
        context.dr0 = entry;
        context.dr1 = acknowledgement;
        context.dr6 = 0;
        context.dr7 = (context.dr7 & !0xFFFF_00FF) | 5;
        self.set_context(tid, &context)
    }

    fn restore_threads(&mut self) -> Result<(), RuntimeError> {
        let result = self.owner.restore_threads();
        for thread in self.owner.threads.values() {
            if thread.restored {
                if let Some(context) = self.contexts.get_mut(&thread.tid) {
                    context.dr0 = thread.current_debug[0];
                    context.dr1 = thread.current_debug[1];
                    context.dr2 = thread.current_debug[2];
                    context.dr3 = thread.current_debug[3];
                    context.dr6 = thread.current_debug[4];
                    context.dr7 = thread.current_debug[5];
                }
            }
        }
        result
    }

    fn all_threads_exited(&mut self) -> Result<bool, RuntimeError> {
        Ok(!self.owner.retired.is_empty() && self.owner.threads.is_empty())
    }

    fn thread_signalled(&mut self, tid: u32) -> Result<bool, RuntimeError> {
        Ok(self.owner.retired.iter().any(|thread| thread.tid == tid))
    }

    fn debug_break(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }
}

impl RuntimeOwnerSession for FakeDebugSession {
    fn begin_cleanup_barrier(&mut self) -> Result<(), RuntimeError> {
        // A fallible step can fail while one event is still pending. The shipped
        // loop would have continued it; do the same here, with the same owned
        // single-step treatment, before asking the fake for its stop barrier.
        if let Some(event) = self.owner.current_event {
            let handled = if event.is_exception() {
                match event.exception_code {
                    Some(EXCEPTION_SINGLE_STEP) => {
                        let mut context =
                            self.contexts.get(&event.tid).copied().unwrap_or_default();
                        if context.dr6 & 3 != 0 {
                            context.dr6 &= !3;
                            context.eflags |= 0x10000;
                            self.set_context(event.tid, &context)?;
                        }
                        true
                    }
                    Some(EXCEPTION_BREAKPOINT) => true,
                    _ => false,
                }
            } else {
                true
            };
            self.resume(&event, handled)?;
        }
        let tid = self
            .owner
            .threads
            .keys()
            .next()
            .copied()
            .or_else(|| self.contexts.keys().next().copied())
            .unwrap_or(1);
        self.events.push_back(DebugEvent {
            code: 1,
            pid: self.pid,
            tid,
            exception_code: Some(EXCEPTION_BREAKPOINT),
            ..DebugEvent::default()
        });
        Ok(())
    }

    fn runtime_owner_snapshot(&self) -> RuntimeOwnerSnapshot {
        let mut threads = self
            .owner
            .retired
            .iter()
            .map(|thread| OwnerThreadSnapshot {
                tid: thread.tid,
                instance: thread.instance,
                handle: thread.handle,
                handle_provenance: "scripted_debug_event",
                run_state: "exited",
                cleanup_state: "exited",
                error: None,
            })
            .collect::<Vec<_>>();
        threads.extend(self.owner.threads.values().map(|thread| {
            OwnerThreadSnapshot {
                tid: thread.tid,
                instance: thread.instance,
                handle: thread.handle,
                handle_provenance: "scripted_debug_event",
                run_state: match thread.state {
                    FakeThreadState::Running => "running",
                    FakeThreadState::Stopped => "stopped",
                    FakeThreadState::Exited => "exited",
                },
                cleanup_state: if thread.restored {
                    "original_restored"
                } else if self.owner.faults.restore_tid == Some(thread.tid) {
                    "restore_failed"
                } else {
                    "armed"
                },
                error: (self.owner.faults.restore_tid == Some(thread.tid))
                    .then(|| "injected debug-register restore failure".to_string()),
            }
        }));
        RuntimeOwnerSnapshot {
            debugger_state: self.owner.debugger_state,
            threads,
        }
    }
}

/// Test-only state for one debugger-owned thread instance.
///
/// A numeric Win32 handle is deliberately not the identity: Windows may reuse
/// the value after the debug event that owned it has been continued. Tests use
/// `instance` to prove that a retired thread cannot be confused with a later
/// thread that happens to receive the same handle value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FakeOwnedThread {
    pub instance: u64,
    pub tid: u32,
    pub handle: u64,
    /// False after continuing EXIT_THREAD; the numeric value may be reused.
    pub handle_open: bool,
    pub state: FakeThreadState,
    pub original_debug: [u64; 6],
    pub current_debug: [u64; 6],
    pub restored: bool,
}

/// Whether the owner may legally read or write one thread's context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeThreadState {
    Running,
    Stopped,
    Exited,
}

/// Faults needed by the T3 runtime-owner regression matrix.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeOwnerFaults {
    /// Fail while restoring this thread, after earlier threads were restored.
    pub restore_tid: Option<u32>,
    /// Fail `DebugActiveProcessStop` and keep debugger ownership attached.
    pub detach: bool,
}

/// A strict offline model of debugger ownership.
///
/// This intentionally lives beside the older syscall-shaped fake instead of
/// changing the production trait. T3a uses it to express the state that the
/// current `DebugSession` API cannot yet carry: thread instance identity,
/// stopped/running/exited transitions, automatic event-handle closure,
/// explicit EXIT_THREAD retirement, partial restore, detach failure and the
/// handled/not-handled disposition of late exceptions.
#[derive(Debug)]
pub struct FakeRuntimeOwner {
    pub attached: bool,
    pub debugger_state: &'static str,
    pub threads: std::collections::BTreeMap<u32, FakeOwnedThread>,
    pub retired: Vec<FakeOwnedThread>,
    pub current_event: Option<DebugEvent>,
    pub resumes: Vec<(DebugEvent, bool)>,
    pub restore_attempts: Vec<(u32, u64)>,
    pub detach_attempts: u64,
    pub faults: RuntimeOwnerFaults,
    next_instance: u64,
}

impl Default for FakeRuntimeOwner {
    fn default() -> Self {
        Self {
            attached: false,
            debugger_state: "not_attached",
            threads: std::collections::BTreeMap::new(),
            retired: Vec::new(),
            current_event: None,
            resumes: Vec::new(),
            restore_attempts: Vec::new(),
            detach_attempts: 0,
            faults: RuntimeOwnerFaults::default(),
            next_instance: 0,
        }
    }
}

impl FakeRuntimeOwner {
    pub fn with_faults(faults: RuntimeOwnerFaults) -> Self {
        Self {
            faults,
            ..Self::default()
        }
    }

    pub fn attach(&mut self) {
        self.attached = true;
        self.debugger_state = "attached";
    }

    /// Adopt one handle from a create event and assign a non-reusable instance.
    pub fn adopt_thread(
        &mut self,
        tid: u32,
        handle: u64,
        context: ThreadContext,
    ) -> Result<u64, RuntimeError> {
        if self.threads.contains_key(&tid)
            || self.threads.values().any(|thread| thread.handle == handle)
        {
            return Err(owner_model_error("thread or handle is still owned"));
        }
        self.next_instance += 1;
        let instance = self.next_instance;
        let debug = debug_registers(&context);
        let state = if self.current_event.is_some() {
            FakeThreadState::Stopped
        } else {
            FakeThreadState::Running
        };
        self.threads.insert(
            tid,
            FakeOwnedThread {
                instance,
                tid,
                handle,
                handle_open: true,
                state,
                original_debug: debug,
                current_debug: debug,
                restored: false,
            },
        );
        Ok(instance)
    }

    /// Deliver one debug event. Its thread is stopped until `continue_event`.
    pub fn begin_event(&mut self, event: DebugEvent) -> Result<(), RuntimeError> {
        if self.current_event.is_some() {
            return Err(owner_model_error("a debug event is already pending"));
        }
        for thread in self.threads.values_mut() {
            if thread.state != FakeThreadState::Exited {
                thread.state = FakeThreadState::Stopped;
            }
        }
        self.current_event = Some(event);
        Ok(())
    }

    pub fn context(&self, tid: u32) -> Result<ThreadContext, RuntimeError> {
        let thread = self
            .threads
            .get(&tid)
            .ok_or_else(|| owner_model_error("thread instance is not owned"))?;
        if thread.state != FakeThreadState::Stopped {
            return Err(owner_model_error("thread context requires a stopped event"));
        }
        Ok(ThreadContext {
            dr0: thread.current_debug[0],
            dr1: thread.current_debug[1],
            dr2: thread.current_debug[2],
            dr3: thread.current_debug[3],
            dr6: thread.current_debug[4],
            dr7: thread.current_debug[5],
            ..ThreadContext::default()
        })
    }

    pub fn set_context(&mut self, tid: u32, context: &ThreadContext) -> Result<(), RuntimeError> {
        let thread = self
            .threads
            .get_mut(&tid)
            .ok_or_else(|| owner_model_error("thread instance is not owned"))?;
        if thread.state != FakeThreadState::Stopped {
            return Err(owner_model_error("thread context requires a stopped event"));
        }
        thread.current_debug = debug_registers(context);
        Ok(())
    }

    /// Continue the pending event. Continuing EXIT_THREAD automatically closes
    /// the event-supplied handle, but deliberately keeps a retired-required
    /// record until the owner acknowledges the exit with `retire_exit_thread`.
    pub fn continue_event(&mut self, handled: bool) -> Result<(), RuntimeError> {
        let event = self
            .current_event
            .take()
            .ok_or_else(|| owner_model_error("no debug event is pending"))?;
        self.resumes.push((event, handled));
        if let Some(thread) = self.threads.get_mut(&event.tid) {
            thread.state = if event.is_exit_thread() {
                thread.handle_open = false;
                FakeThreadState::Exited
            } else {
                FakeThreadState::Running
            };
        }
        for thread in self.threads.values_mut() {
            if thread.state != FakeThreadState::Exited {
                thread.state = FakeThreadState::Running;
            }
        }
        Ok(())
    }

    /// Retire the exact instance after EXIT_THREAD has been continued.
    pub fn retire_exit_thread(&mut self, tid: u32) -> Result<FakeOwnedThread, RuntimeError> {
        let thread = self
            .threads
            .get(&tid)
            .copied()
            .ok_or_else(|| owner_model_error("thread instance is not owned"))?;
        if thread.state != FakeThreadState::Exited {
            return Err(owner_model_error("EXIT_THREAD was not continued"));
        }
        let retired = self
            .threads
            .remove(&tid)
            .ok_or_else(|| owner_model_error("thread instance is not owned"))?;
        self.retired.push(retired);
        Ok(retired)
    }

    /// Restore in deterministic tid order. A fault leaves the failed and later
    /// instances owned, making partial cleanup visible to the caller.
    pub fn restore_threads(&mut self) -> Result<(), RuntimeError> {
        let tids = self.threads.keys().copied().collect::<Vec<_>>();
        for tid in tids {
            let thread = self
                .threads
                .get_mut(&tid)
                .ok_or_else(|| owner_model_error("thread instance is not owned"))?;
            if thread.state != FakeThreadState::Stopped {
                return Err(owner_model_error(
                    "restore requires a stopped event barrier",
                ));
            }
            self.restore_attempts.push((tid, thread.instance));
            if self.faults.restore_tid == Some(tid) {
                return Err(owner_model_error("injected debug-register restore failure"));
            }
            thread.current_debug = thread.original_debug;
            thread.restored = true;
        }
        Ok(())
    }

    pub fn detach(&mut self) -> Result<(), RuntimeError> {
        self.detach_attempts += 1;
        if self.faults.detach {
            self.debugger_state = "detach_failed";
            return Err(owner_model_error("injected debugger detach failure"));
        }
        self.attached = false;
        self.debugger_state = "detached";
        Ok(())
    }
}

fn debug_registers(context: &ThreadContext) -> [u64; 6] {
    [
        context.dr0,
        context.dr1,
        context.dr2,
        context.dr3,
        context.dr6,
        context.dr7,
    ]
}

fn owner_model_error(detail: &str) -> RuntimeError {
    RuntimeError::NativeDispatch {
        detail: detail.to_string(),
    }
}

/// A remote session over one fixture, for the oracle gates.
pub struct FakeRemoteSession {
    pub pid: u32,
    pub memory: ByteMemory,
    pub threads: Vec<u64>,
    pub next_thread: u64,
    pub exit_code: u32,
    pub wait_result: u32,
}

impl FakeRemoteSession {
    pub fn new() -> Self {
        Self {
            pid: FIXTURE_PID,
            memory: ByteMemory::default(),
            threads: Vec::new(),
            next_thread: 0x5000_0000,
            exit_code: 0,
            wait_result: crate::mutation::win_session::WAIT_OBJECT_0,
        }
    }
}

impl Default for FakeRemoteSession {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteSession for FakeRemoteSession {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        self.memory.read(address, size)
    }

    fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
        self.memory.write(address, data);
        Ok(())
    }

    fn allocate(&mut self, _size: usize) -> Result<u64, RuntimeError> {
        Ok(0x1000_0000)
    }

    fn free(&mut self, _address: u64) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn create_remote_thread(&mut self, _start: u64) -> Result<u64, RuntimeError> {
        let handle = self.next_thread;
        self.next_thread += 0x1000;
        self.threads.push(handle);
        Ok(handle)
    }

    fn wait_thread(&mut self, _thread: u64, _milliseconds: u32) -> Result<u32, RuntimeError> {
        Ok(self.wait_result)
    }

    fn thread_exit_code(&mut self, _thread: u64) -> Result<u32, RuntimeError> {
        Ok(self.exit_code)
    }

    fn close_thread(&mut self, _thread: u64) {}

    fn close(&mut self) {}
}

/// The display version every fixture advertises.
pub const FIXTURE_DISPLAY_VERSION: &str = LIVE_ADD_DISPLAY_VERSION;
