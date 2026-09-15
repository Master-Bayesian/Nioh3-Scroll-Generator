//! Injected transports for the native executor and the batch oracle.
//!
//! Compiled only for `cfg(test)` or with the off-by-default `test-fake`
//! feature, so the shipped library keeps exactly one process implementation.
//! The fixture is byte-addressable, so the executor's real gates (dispatch
//! signature, inventory capture, capacity, serial, slot) run over the same
//! shapes a game process produces, without a game process.

use crate::error::RuntimeError;
use crate::mutation::descriptor::verify_assembly_preview;
use crate::mutation::evidence::verify_dispatch;
use crate::mutation::inventory::{hex_decode, RECORD_SIZE};
use crate::mutation::live_add::LIVE_ADD_DISPLAY_VERSION;
use crate::mutation::live_fakes::{ByteMemory, InventoryFixture, FIXTURE_BASE, FIXTURE_PID};
use crate::mutation::native_abi::{hex, LiveAddLayout, PC_V201_LIVE_ADD};
use crate::mutation::native_executor::{settled, DispatchMode, LiveAddTransport, ReceiptStore};
use crate::mutation::win_session::{DebugEvent, DebugSession, RemoteSession, ThreadContext};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Every injected native failure the executor gates use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeFaults {
    /// The insertion happened, the receipt was never acknowledged.
    pub reply_lost: bool,
    /// The periodic idle window was missed: nothing was dispatched.
    pub idle_miss: bool,
    /// The submission never reached the target: no receipt exists.
    pub reject_before_dispatch: bool,
    /// The destination write is skipped so recovery sees a proven absence.
    pub skip_destination: bool,
}

/// A transport over one fixture process image.
pub struct FakeLiveAddTransport {
    pub fixture: InventoryFixture,
    pub layout: LiveAddLayout,
    pub store: ReceiptStore,
    pub faults: NativeFaults,
    pub submissions: Vec<String>,
    pub module_name: String,
    pub current_creation_time: String,
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
        let mut fixture = fixture;
        // The dispatch shim's own contract site: the seven signature bytes.
        let layout = PC_V201_LIVE_ADD;
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
        let current = fixture.creation_time.to_string();
        Ok(Self {
            fixture,
            layout,
            store: ReceiptStore::new(directory)?,
            faults,
            submissions: Vec::new(),
            module_name: "Nioh3.exe".to_string(),
            current_creation_time: current,
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
        self.fixture.memory.read(address, size)
    }

    fn ping(&mut self) -> Result<Value, RuntimeError> {
        Ok(json!({
            "pid": FIXTURE_PID,
            "profile_id": self.layout.profile_id,
            "busy": self.store.unresolved_owner()?.is_some(),
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
            "executor": "windows-native",
            "mode": mode.receipt_mode(),
            "serial": params.get("serial").cloned().unwrap_or(Value::Null),
            "slot": params.get("slot").cloned().unwrap_or(Value::Null),
        });
        self.write_receipt(&receipt)?;
        if self.faults.idle_miss {
            if let Some(object) = receipt.as_object_mut() {
                object.insert("phase".to_string(), json!("rejected"));
                object.insert("active".to_string(), json!(false));
                object.insert("released".to_string(), json!(true));
                object.insert("breakpoint_count".to_string(), json!(0));
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
            }
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
            return self.receipt(operation_id);
        }
        Err(RuntimeError::NativeDispatch {
            detail: "Unknown native operation".to_string(),
        })
    }

    /// Verification only, exactly like the real transport: the destination
    /// record plus the advanced serial prove the outcome, or its absence does.
    fn release(&mut self, operation_id: &str) -> Result<Value, RuntimeError> {
        let mut value = self.store.read(operation_id)?;
        if settled(&value) {
            return Ok(value);
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
                object.insert("released".to_string(), json!(true));
                object.insert("breakpoint_count".to_string(), json!(0));
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
                object.insert("released".to_string(), json!(true));
                object.insert("breakpoint_count".to_string(), json!(0));
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

    fn safe_to_shutdown(&mut self) -> bool {
        self.store
            .unresolved_owner()
            .map(|owner| owner.is_none())
            .unwrap_or(false)
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

/// A byte-addressable debug session: no syscall, same contract.
pub struct FakeDebugSession {
    pub pid: u32,
    pub memory: ByteMemory,
    pub base: u64,
    pub creation_time: u64,
    pub attached: bool,
    pub events: std::collections::VecDeque<DebugEvent>,
    pub contexts: std::collections::BTreeMap<u32, ThreadContext>,
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

    fn free(&mut self, _address: u64) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn flush_instruction_cache(&mut self, _address: u64, _size: usize) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn attach(&mut self) -> Result<(), RuntimeError> {
        self.attached = true;
        Ok(())
    }

    fn attached(&self) -> bool {
        self.attached
    }

    fn detach(&mut self) -> Result<(), RuntimeError> {
        self.attached = false;
        Ok(())
    }

    fn wait(&mut self, _milliseconds: u32) -> Result<Option<DebugEvent>, RuntimeError> {
        Ok(self.events.pop_front())
    }

    fn resume(&mut self, _event: &DebugEvent, _handled: bool) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn context(&mut self, tid: u32) -> Result<ThreadContext, RuntimeError> {
        self.contexts
            .get(&tid)
            .copied()
            .ok_or(RuntimeError::SessionNotOpen)
    }

    fn set_context(&mut self, tid: u32, context: &ThreadContext) -> Result<(), RuntimeError> {
        self.contexts.insert(tid, *context);
        Ok(())
    }

    fn adopt_thread(&mut self, _tid: u32, _handle: u64) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn arm_thread(
        &mut self,
        _tid: u32,
        _entry: u64,
        _acknowledgement: u64,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn restore_threads(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn all_threads_exited(&mut self) -> Result<bool, RuntimeError> {
        Ok(false)
    }

    fn thread_signalled(&mut self, _tid: u32) -> Result<bool, RuntimeError> {
        Ok(false)
    }

    fn debug_break(&mut self) -> Result<(), RuntimeError> {
        Ok(())
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
