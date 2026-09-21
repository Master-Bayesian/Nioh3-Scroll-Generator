//! Injected fixtures for the inventory, live-add and batch gates.
//!
//! Compiled only for `cfg(test)` or with the off-by-default `test-fake` feature,
//! so the shipped library keeps exactly one process implementation. The fixtures
//! are byte-addressable so the ported gates run over the same shapes the real
//! capture produces, without a game process.

use crate::error::RuntimeError;
use crate::mutation::count::{is_canonical_uuid, new_operation_id};
use crate::mutation::descriptor::{assembly_descriptor, verify_assembly_preview};
use crate::mutation::evidence::{
    preview_owner_fingerprint, preview_rejection_receipt, verify_dispatch, PREVIEW_PHASE_AFTER,
    PREVIEW_PHASE_BEFORE, REGISTERS,
};
use crate::mutation::inventory::{
    capture_read_only, hex_decode, Inventory, InventoryLayout, InventoryProcess, NativeIndex,
    CAPACITY, INSERTION_SIGNATURE, RECORD_SIZE,
};
use crate::mutation::live_add::{
    CatalogPolicy, InstallationCandidate, LiveAddExecutor, SaveBackup, SaveCheckpoint,
    LIVE_ADD_DISPLAY_VERSION,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Image base every fixture reports.
pub const FIXTURE_BASE: u64 = 0x7FF0_0000_0000;
/// Process id every fixture reports.
pub const FIXTURE_PID: u32 = 4321;
/// Creation FILETIME every fixture reports.
pub const FIXTURE_CREATION: u64 = 134_338_049_984_156_850;
/// The profile id the fixture advertises.
pub const FIXTURE_PROFILE_ID: &str = "pc-v2.01-live-add-r1";

/// A byte-addressable process memory space.
#[derive(Debug, Clone, Default)]
pub struct ByteMemory {
    bytes: BTreeMap<u64, u8>,
}

impl ByteMemory {
    pub fn write(&mut self, address: u64, data: &[u8]) {
        for (offset, byte) in data.iter().enumerate() {
            self.bytes.insert(address + offset as u64, *byte);
        }
    }

    pub fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        let mut result = Vec::with_capacity(size);
        for offset in 0..size as u64 {
            match self.bytes.get(&(address + offset)) {
                Some(byte) => result.push(*byte),
                None => {
                    return Err(RuntimeError::MemoryRead {
                        address,
                        size,
                        code: 299,
                    })
                }
            }
        }
        Ok(result)
    }

    fn u64(&self, address: u64) -> Result<u64, RuntimeError> {
        let raw = self.read(address, 8)?;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&raw);
        Ok(u64::from_le_bytes(bytes))
    }
}

/// A read-only inventory fixture in the shipped layout shape.
#[derive(Debug, Clone)]
pub struct InventoryFixture {
    pub layout: InventoryLayout,
    /// Display version the accepted pair binds this layout to.
    pub display_version: &'static str,
    /// Live-add profile id the accepted pair binds this layout to.
    pub profile_id: &'static str,
    pub base: u64,
    pub memory: ByteMemory,
    pub creation_time: u64,
}

impl InventoryFixture {
    /// A fixture whose container is exactly `container`, with the serial index
    /// re-derived from it. Used by the cross-language gate so both sides read
    /// the same bytes.
    pub fn from_container(
        container: &[u8],
        serial_counter: u64,
        acquisition_order: u32,
    ) -> Result<Self, RuntimeError> {
        Self::from_container_for_layout(
            crate::mutation::inventory::PC_V201_INVENTORY_LAYOUT,
            container,
            serial_counter,
            acquisition_order,
        )
    }

    /// The same fixture over any accepted layout, so a candidate run can seed a
    /// synthetic v2.02 image at the candidate RVAs instead of the shipped ones.
    pub fn from_container_for_layout(
        layout: InventoryLayout,
        container: &[u8],
        serial_counter: u64,
        acquisition_order: u32,
    ) -> Result<Self, RuntimeError> {
        if container.len() != CAPACITY as usize * RECORD_SIZE {
            return Err(RuntimeError::InventoryInvalid {
                detail: "Expected a complete 400-record container".to_string(),
            });
        }
        // The accepted pair's own RVAs, so a synthetic container can be compared
        // against the shipped `capture_inventory` on the same bytes.
        let display_version = Self::binding_version(&layout);
        let profile_id = Self::binding_profile_id(&layout);
        let base = FIXTURE_BASE;
        let mut memory = ByteMemory::default();
        memory.write(base + layout.insertion_rva, &INSERTION_SIGNATURE);
        let manager = base + 0x1_0000;
        let data = base + 0x2_0000;
        memory.write(base + layout.manager_pointer_rva, &manager.to_le_bytes());
        memory.write(manager, &data.to_le_bytes());
        memory.write(data, &[0u8; 16]);
        memory.write(data, &acquisition_order.to_le_bytes());
        memory.write(data + 8, &serial_counter.to_le_bytes());
        let container_address = data + layout.container_offset;
        memory.write(container_address, container);
        memory.write(
            container_address + layout.capacity_offset,
            &(CAPACITY as u64).to_le_bytes(),
        );
        let mut fixture = Self {
            layout,
            display_version,
            profile_id,
            base,
            memory,
            creation_time: FIXTURE_CREATION,
        };
        fixture.refresh_index()?;
        Ok(fixture)
    }

    /// `records` are `(slot, serial, seed)`; the counters are supplied directly
    /// so a test can express a counter that disagrees with the records.
    pub fn new(records: &[(usize, u64, u32)], serial_counter: u64, acquisition_order: u32) -> Self {
        Self::new_for_layout(
            crate::mutation::inventory::PC_V201_INVENTORY_LAYOUT,
            records,
            serial_counter,
            acquisition_order,
        )
    }

    /// The same synthetic fixture over any accepted layout.
    pub fn new_for_layout(
        layout: InventoryLayout,
        records: &[(usize, u64, u32)],
        serial_counter: u64,
        acquisition_order: u32,
    ) -> Self {
        let display_version = Self::binding_version(&layout);
        let profile_id = Self::binding_profile_id(&layout);
        let base = FIXTURE_BASE;
        let mut memory = ByteMemory::default();
        memory.write(base + layout.insertion_rva, &INSERTION_SIGNATURE);
        let manager = base + 0x1_0000;
        let data = base + 0x2_0000;
        memory.write(base + layout.manager_pointer_rva, &manager.to_le_bytes());
        memory.write(manager, &data.to_le_bytes());
        memory.write(data, &[0u8; 16]);
        memory.write(data, &acquisition_order.to_le_bytes());
        memory.write(data + 8, &serial_counter.to_le_bytes());
        let container = data + layout.container_offset;
        memory.write(container, &vec![0u8; CAPACITY as usize * RECORD_SIZE]);
        memory.write(
            container + layout.capacity_offset,
            &(CAPACITY as u64).to_le_bytes(),
        );
        for (slot, serial, seed) in records {
            let record = assembly_record(0x1E82, *seed, 4);
            let mut record = record;
            record[0x28..0x30].copy_from_slice(&serial.to_le_bytes());
            record[0x1C..0x20].copy_from_slice(&acquisition_order.to_le_bytes());
            memory.write(container + (*slot * RECORD_SIZE) as u64, &record);
        }
        Self::seed_index(&mut memory, data + layout.serial_index_offset, records);
        Self {
            layout,
            display_version,
            profile_id,
            base,
            memory,
            creation_time: FIXTURE_CREATION,
        }
    }

    /// The display version the accepted inventory pair binds one layout to. An
    /// unaccepted layout keeps the product label so a fixture cannot silently
    /// invent a version; every read gate still refuses the pair.
    fn binding_version(layout: &InventoryLayout) -> &'static str {
        crate::mutation::inventory::accepted_inventory_version(layout)
            .unwrap_or(LIVE_ADD_DISPLAY_VERSION)
    }

    /// The live-add profile id the accepted inventory pair binds one layout to.
    /// An unaccepted layout keeps the shipped fixture id so nothing invents a
    /// binding the gates have not accepted.
    fn binding_profile_id(layout: &InventoryLayout) -> &'static str {
        crate::mutation::inventory::accepted_inventory_profile_id(layout)
            .unwrap_or(FIXTURE_PROFILE_ID)
    }

    /// A single FNV bucket holding the whole list, which is the smallest shape
    /// `live_inventory.inspect` accepts.
    fn seed_index(memory: &mut ByteMemory, header: u64, records: &[(usize, u64, u32)]) {
        let nodes: Vec<u64> = (0..records.len() as u64)
            .map(|index| header + 0x1000 + index * 0x40)
            .collect();
        let sentinel = header + 0x100;
        memory.write(header, &[0u8; 0x40]);
        memory.write(header + 0x40, &[0u8; 16]);
        memory.write(sentinel, &[0u8; 0x20]);
        for node in &nodes {
            memory.write(*node, &[0u8; 0x20]);
        }
        memory.write(header + 8, &sentinel.to_le_bytes());
        memory.write(header + 16, &(records.len() as u64).to_le_bytes());
        memory.write(header + 24, &(header + 0x40).to_le_bytes());
        memory.write(header + 0x30, &0u64.to_le_bytes());
        memory.write(header + 0x38, &1u64.to_le_bytes());
        if nodes.is_empty() {
            memory.write(sentinel, &sentinel.to_le_bytes());
            memory.write(sentinel + 8, &sentinel.to_le_bytes());
            return;
        }
        memory.write(sentinel, &nodes[0].to_le_bytes());
        memory.write(sentinel + 8, &nodes[nodes.len() - 1].to_le_bytes());
        for (index, ((slot, serial, _seed), node)) in records.iter().zip(nodes.iter()).enumerate() {
            let next = nodes.get(index + 1).copied().unwrap_or(sentinel);
            let previous = if index == 0 {
                sentinel
            } else {
                nodes[index - 1]
            };
            memory.write(*node, &next.to_le_bytes());
            memory.write(*node + 8, &previous.to_le_bytes());
            memory.write(*node + 0x10, &serial.to_le_bytes());
            memory.write(*node + 0x18, &(*slot as u32).to_le_bytes());
        }
        // The bucket walks the backward (`previous`) chain, so `first` is the
        // chain's end in list order and `current` is its newest node.
        memory.write(header + 0x40, &nodes[0].to_le_bytes());
        memory.write(header + 0x48, &nodes[nodes.len() - 1].to_le_bytes());
    }

    pub fn container(&self) -> Result<Vec<u8>, RuntimeError> {
        let data = self.data()?;
        self.memory.read(
            data + self.layout.container_offset,
            CAPACITY as usize * RECORD_SIZE,
        )
    }

    pub fn data(&self) -> Result<u64, RuntimeError> {
        let manager = self
            .memory
            .u64(self.base + self.layout.manager_pointer_rva)?;
        self.memory.u64(manager)
    }

    pub fn manager(&self) -> Result<u64, RuntimeError> {
        self.memory.u64(self.base + self.layout.manager_pointer_rva)
    }

    pub fn capture(&self) -> Result<(Inventory, NativeIndex), RuntimeError> {
        let mut view = FixtureView { fixture: self };
        capture_read_only(&mut view, &self.layout, self.display_version)
    }

    pub fn serial_counter(&self) -> Result<u64, RuntimeError> {
        self.memory
            .u64(self.data()? + self.layout.serial_counter_offset)
    }

    /// Re-derive the index from the current container: the fixture's stand-in
    /// for the engine's own bookkeeping after one insertion.
    pub fn refresh_index(&mut self) -> Result<(), RuntimeError> {
        let container = self.container()?;
        let data = self.data()?;
        let mut records = Vec::new();
        for slot in 0..CAPACITY as usize {
            let start = slot * RECORD_SIZE;
            if container[start] == 0 && container[start + 1] == 0 {
                continue;
            }
            let serial =
                u64::from_le_bytes(container[start + 0x28..start + 0x30].try_into().map_err(
                    |_| RuntimeError::InventoryInvalid {
                        detail: "Invalid scroll record length".to_string(),
                    },
                )?);
            records.push((slot, serial, 0u32));
        }
        Self::seed_index(
            &mut self.memory,
            data + self.layout.serial_index_offset,
            &records,
        );
        Ok(())
    }

    pub fn first_empty_slot(&self) -> Result<usize, RuntimeError> {
        let container = self.container()?;
        for slot in 0..CAPACITY as usize {
            let start = slot * RECORD_SIZE;
            if container[start] == 0 && container[start + 1] == 0 {
                return Ok(slot);
            }
        }
        Err(RuntimeError::LiveAddRejected {
            detail: "Scroll inventory is full".to_string(),
        })
    }

    fn set_u32(&mut self, address: u64, value: u32) {
        self.memory.write(address, &value.to_le_bytes());
    }

    fn set_u64(&mut self, address: u64, value: u64) {
        self.memory.write(address, &value.to_le_bytes());
    }
}

/// A read-only view over one fixture, for direct `capture_inventory` calls.
pub struct FixtureView<'a> {
    fixture: &'a InventoryFixture,
}

pub fn fixture_view(fixture: &InventoryFixture) -> FixtureView<'_> {
    FixtureView { fixture }
}

impl InventoryProcess for FixtureView<'_> {
    fn pid(&self) -> u32 {
        FIXTURE_PID
    }

    fn module_base(&self) -> u64 {
        self.fixture.base
    }

    fn creation_time(&mut self) -> Result<String, RuntimeError> {
        Ok(self.fixture.creation_time.to_string())
    }

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        self.fixture.memory.read(address, size)
    }
}

/// A canonical scroll record: `record_type`, `seed`, `rarity`, live flags.
pub fn assembly_record(record_type: u16, seed: u32, rarity: u8) -> Vec<u8> {
    let mut record = vec![0u8; RECORD_SIZE];
    record[0..2].copy_from_slice(&record_type.to_le_bytes());
    record[2..4].copy_from_slice(&0x0011u16.to_le_bytes());
    record[0x10..0x12].copy_from_slice(&0x2222u16.to_le_bytes());
    record[0x14..0x18].copy_from_slice(&0x3333_4444u32.to_le_bytes());
    // The raw record already carries the seeded nonstackable flag the assembly
    // descriptor gate requires; `new_assembly_record` installs builder metadata.
    record[0x18..0x1C].copy_from_slice(&0x0280_0002u32.to_le_bytes());
    record[0x20..0x24].copy_from_slice(&seed.to_le_bytes());
    record[0x28..0x30].copy_from_slice(&u64::MAX.to_le_bytes());
    record[0x30] = rarity;
    record[0x33] = 2;
    record[0xDC..0xE0].copy_from_slice(&0x5555_6666u32.to_le_bytes());
    for (index, byte) in record.iter_mut().enumerate().skip(0x34).take(0xDC - 0x34) {
        *byte = (index as u8).wrapping_mul(7).wrapping_add(3);
    }
    record
}

/// Every injected live-add failure the gates use.
#[derive(Debug, Clone, Default)]
pub struct LiveAddFaults {
    /// The write happened, the reply did not: the operation stays uncertain.
    pub reply_lost: bool,
    /// The isolated preview's builder output differs from the reviewed record.
    /// The dispatch settles as a formal rejection with every proof present.
    pub preview_mismatch: bool,
    /// The preview leaves an unsettled receipt, so its child stays blocked.
    pub preview_incomplete: bool,
    /// The transport can prove the submission never reached the target.
    pub submission_absent: bool,
    /// The periodic idle window was missed; nothing was dispatched.
    pub idle_miss: bool,
    /// Another container byte changed after the insertion.
    pub readback_changed: bool,
    /// The game was restarted between prepare and verification.
    pub pid_reuse: bool,
}

/// An injected native executor over the fixture memory.
pub struct FakeLiveAddExecutor {
    pub fixture: InventoryFixture,
    pub faults: LiveAddFaults,
    pub receipts: BTreeMap<String, Value>,
    pub current_creation_time: String,
    pub submissions: Vec<String>,
}

impl FakeLiveAddExecutor {
    pub fn new(fixture: InventoryFixture) -> Self {
        let current = fixture.creation_time.to_string();
        Self {
            fixture,
            faults: LiveAddFaults::default(),
            receipts: BTreeMap::new(),
            current_creation_time: current,
            submissions: Vec::new(),
        }
    }

    pub fn with_faults(fixture: InventoryFixture, faults: LiveAddFaults) -> Self {
        let mut executor = Self::new(fixture);
        if faults.pid_reuse {
            executor.current_creation_time = "134338049984156851".to_string();
        }
        executor.faults = faults;
        executor
    }

    fn context(&self) -> Result<Value, RuntimeError> {
        let (inventory, _index) = self.fixture.capture()?;
        let manager = self.fixture.manager()?;
        let data = self.fixture.data()?;
        let serial = self.fixture.serial_counter()?;
        let slot = self.fixture.first_empty_slot()?;
        Ok(json!({
            "pid": FIXTURE_PID,
            "profile_id": self.fixture.profile_id,
            "manager": manager,
            "data": data,
            "process_creation_time": inventory.process_creation_time,
            "serial": serial,
            "slot": slot,
            "scheduler_owner": 0x7FF0_0000_9000u64,
            "function_address": self.fixture.base + self.fixture.layout.insertion_rva,
            "container_hex": hex(&self.fixture.container()?),
            "insertion_code_hex": hex(&INSERTION_SIGNATURE),
            "builder_code_hex": hex(&assembly_descriptor(
                &assembly_record(0x1E82, 1, 4),
                false,
            )?),
        }))
    }

    /// The register frame `dispatch_evidence.verify_dispatch` accepts.
    fn dispatch_frame(
        function_address: u64,
        mut registers: BTreeMap<&'static str, u64>,
    ) -> Result<Value, RuntimeError> {
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
        for register in REGISTERS {
            registers.insert(register, 0x4141_0000_0000 + register.len() as u64);
        }
        let mut before = serde_json::Map::new();
        let mut after = serde_json::Map::new();
        for (key, value) in &registers {
            before.insert((*key).to_string(), json!(value));
            after.insert((*key).to_string(), json!(value));
        }
        before.insert("RSP".to_string(), json!(before_rsp));
        before.insert("RIP".to_string(), json!(function_address));
        before.insert("EFLAGS".to_string(), json!(0x202u64));
        after.insert("RSP".to_string(), json!(after_rsp));
        after.insert("RIP".to_string(), json!(function_address + 7));
        after.insert("EFLAGS".to_string(), json!(0x202u64 & !0x8D5u64 | eflags));
        Ok(json!({"before": before, "after": after}))
    }

    /// The isolated builder output: generated content unchanged, serial still
    /// unallocated. Any allocated serial here is a refusal, not a preview.
    fn preview_source(assembly: &[u8]) -> Result<Vec<u8>, RuntimeError> {
        let mut source = assembly.to_vec();
        if source.len() != RECORD_SIZE {
            return Err(RuntimeError::LiveAddRejected {
                detail: "Partial native assembly preview".to_string(),
            });
        }
        source[0x28..0x30].copy_from_slice(&[0xFF; 8]);
        Ok(source)
    }

    /// The accepted insertion source: the assembly record with its allocated
    /// serial, exactly what the native builder hands to the insertion routine.
    fn insert_source(plan: &Value, assembly: &[u8]) -> Result<Vec<u8>, RuntimeError> {
        let mut source = assembly.to_vec();
        if source.len() != RECORD_SIZE {
            return Err(RuntimeError::LiveAddRejected {
                detail: "Partial native assembly preview".to_string(),
            });
        }
        let serial = plan.get("serial").and_then(Value::as_u64).ok_or_else(|| {
            RuntimeError::LiveAddRejected {
                detail: "Stored plan content changed".to_string(),
            }
        })?;
        source[0x28..0x30].copy_from_slice(&serial.to_le_bytes());
        Ok(source)
    }

    fn preview_receipt(&self, plan: &Value, source: &[u8]) -> Result<Value, RuntimeError> {
        let mut receipt = Self::dispatch_frame(
            plan.get("function_address")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            BTreeMap::new(),
        )?;
        let object = receipt
            .as_object_mut()
            .ok_or_else(|| RuntimeError::LiveAddVerification {
                detail: "Exactly one acknowledged redirect is required".to_string(),
            })?;
        object.insert("phase".to_string(), json!("completed"));
        object.insert("redirect_count".to_string(), json!(1));
        object.insert("released".to_string(), json!(true));
        object.insert("active".to_string(), json!(false));
        object.insert("breakpoints".to_string(), json!([]));
        object.insert(
            "pid".to_string(),
            plan.get("pid").cloned().unwrap_or(json!(0)),
        );
        object.insert("source_hex".to_string(), json!(hex(source)));
        verify_dispatch(&receipt)?;
        Ok(receipt)
    }

    /// The identity the parent pinned for this preview attempt, or a fresh one.
    fn preview_child_id(plan: &Value) -> Result<String, RuntimeError> {
        match plan
            .get("preview_operation_id")
            .and_then(Value::as_str)
            .filter(|value| is_canonical_uuid(value))
        {
            Some(pinned) => Ok(pinned.to_string()),
            None => new_operation_id(),
        }
    }

    /// One owner-named preview fingerprint over the fixture's stopped container.
    fn preview_fingerprint(&self, plan: &Value, phase: &str) -> Result<Value, RuntimeError> {
        // The fixture's own native index, through the shipped capture traversal:
        // the fingerprint's index evidence is the real index, not the container.
        let (inventory, index) = self.fixture.capture()?;
        Ok(preview_owner_fingerprint(
            &self.fixture.container()?,
            self.fixture.serial_counter()?,
            inventory.acquisition_order_counter,
            &self.fixture.layout,
            self.fixture.profile_id,
            FIXTURE_PID,
            plan.get("process_creation_time")
                .and_then(Value::as_str)
                .unwrap_or(self.current_creation_time.as_str()),
            self.fixture.manager()?,
            self.fixture.data()?,
            self.fixture.base,
            &index.to_json(),
            phase,
        ))
    }
}

impl LiveAddExecutor for FakeLiveAddExecutor {
    fn inspect(&mut self) -> Result<(Value, Inventory, NativeIndex), RuntimeError> {
        let context = self.context()?;
        let (inventory, index) = self.fixture.capture()?;
        Ok((context, inventory, index))
    }

    fn preview(&mut self, plan: &Value, assembly_record: &[u8]) -> Result<Value, RuntimeError> {
        let operation_id = Self::preview_child_id(plan)?;
        let source = Self::preview_source(assembly_record)?;
        if self.faults.preview_mismatch {
            // The isolated builder output differs from the reviewed record. The
            // run settles as a formal rejection with every proof present, and the
            // adapter returns the same comparison failure the real one does.
            let mut mismatched = source.clone();
            mismatched[0x20] ^= 0xFF;
            let receipt = preview_rejection_receipt(
                &operation_id,
                plan.get("parent_operation_id").and_then(Value::as_str),
                FIXTURE_PID,
                plan.get("process_creation_time")
                    .and_then(Value::as_str)
                    .unwrap_or(self.current_creation_time.as_str()),
                plan.get("descriptor_hex")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                plan.get("expected_record_hex")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                plan.get("builder_code_hex")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                &mismatched,
                self.preview_fingerprint(plan, PREVIEW_PHASE_BEFORE)?,
                self.preview_fingerprint(plan, PREVIEW_PHASE_AFTER)?,
            )?;
            self.receipts.insert(operation_id.clone(), receipt);
            return Err(RuntimeError::LiveAddVerification {
                detail: "Native assembly differs from the expected installation record"
                    .to_string(),
            });
        }
        if self.faults.preview_incomplete {
            // An unsettled receipt: ownership is not proven ended, so the child
            // stays blocked instead of being settled.
            let mut receipt = self.preview_receipt(plan, &source)?;
            if let Some(object) = receipt.as_object_mut() {
                object.insert("operation_id".to_string(), json!(operation_id.clone()));
                object.insert("mode".to_string(), json!("preview"));
                object.insert("phase".to_string(), json!("uncertain"));
                object.insert("released".to_string(), json!(false));
                object.insert("business_outcome".to_string(), json!("unknown"));
                object.insert("remote_execution".to_string(), json!("unknown"));
                object.insert("allocation_state".to_string(), json!("retained"));
                object.insert("debugger_state".to_string(), json!("unknown"));
                object.insert("thread_cleanup".to_string(), json!({}));
                object.insert("breakpoint_count".to_string(), json!(-1));
                object.insert("slot".to_string(), json!(0));
                object.insert("serial".to_string(), json!(0));
            }
            self.receipts.insert(operation_id.clone(), receipt);
            return Err(RuntimeError::LiveAddVerification {
                detail: "Native dispatch result is uncertain; allocation retained, do not retry"
                    .to_string(),
            });
        }
        verify_assembly_preview(assembly_record, &source)?;
        let mut receipt = self.preview_receipt(plan, &source)?;
        if let Some(object) = receipt.as_object_mut() {
            object.insert("operation_id".to_string(), json!(operation_id.clone()));
            object.insert("mode".to_string(), json!("preview"));
            object.insert(
                "parent_operation_id".to_string(),
                plan.get("parent_operation_id").cloned().unwrap_or(Value::Null),
            );
            object.insert("business_outcome".to_string(), json!("completed"));
            object.insert("breakpoint_count".to_string(), json!(0));
            object.insert("remote_execution".to_string(), json!("quiescent"));
            object.insert("allocation_state".to_string(), json!("freed"));
            object.insert("debugger_state".to_string(), json!("detached"));
            object.insert("thread_cleanup".to_string(), json!({}));
            object.insert(
                "process_creation_time".to_string(),
                plan.get("process_creation_time")
                    .cloned()
                    .unwrap_or(json!(self.current_creation_time.clone())),
            );
        }
        self.receipts.insert(operation_id.clone(), receipt.clone());
        Ok(receipt)
    }

    fn preview_children(
        &mut self,
        parent_operation_id: &str,
    ) -> Result<Vec<Value>, RuntimeError> {
        Ok(self
            .receipts
            .values()
            .filter(|receipt| {
                receipt.get("mode").and_then(Value::as_str) == Some("preview")
                    && receipt.get("parent_operation_id").and_then(Value::as_str)
                        == Some(parent_operation_id)
            })
            .cloned()
            .collect())
    }

    fn insert(&mut self, plan: &Value) -> Result<Value, RuntimeError> {
        let operation_id = plan
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::LiveAddRejected {
                detail: "Stored plan content changed".to_string(),
            })?
            .to_string();
        self.submissions.push(operation_id.clone());
        if self.faults.submission_absent {
            // A rejected submission has no native ownership, so nothing is
            // written and the application may record the proven absence.
            return Err(RuntimeError::LiveAddRejected {
                detail: "submission rejected before dispatch".to_string(),
            });
        }
        if self.faults.idle_miss {
            let mut receipt = Self::dispatch_frame(
                plan.get("function_address")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                BTreeMap::new(),
            )?;
            let object = receipt
                .as_object_mut()
                .ok_or_else(|| RuntimeError::LiveAddRejected {
                    detail: "Stored plan content changed".to_string(),
                })?;
            object.insert("redirect_count".to_string(), json!(0));
            object.insert("released".to_string(), json!(true));
            object.insert("active".to_string(), json!(false));
            object.insert("breakpoints".to_string(), json!([]));
            object.insert("phase".to_string(), json!("released"));
            object.insert("operation_id".to_string(), json!(operation_id.clone()));
            object.insert(
                "pid".to_string(),
                plan.get("pid").cloned().unwrap_or(json!(0)),
            );
            object.insert(
                "error".to_string(),
                json!("No accepted idle dispatch before timeout"),
            );
            self.receipts.insert(operation_id.clone(), receipt.clone());
            return Ok(receipt);
        }

        let assembly = hex_decode(
            plan.get("expected_record_hex")
                .and_then(Value::as_str)
                .ok_or_else(|| RuntimeError::LiveAddRejected {
                    detail: "Stored plan content changed".to_string(),
                })?,
        )?;
        let source = Self::insert_source(plan, &assembly)?;
        let mut destination = source.clone();
        let flags = u32::from_le_bytes([
            destination[0x18],
            destination[0x19],
            destination[0x1A],
            destination[0x1B],
        ]) | 0x0400_0080;
        destination[0x18..0x1C].copy_from_slice(&flags.to_le_bytes());
        let (before, _) = self.fixture.capture()?;
        destination[0x1C..0x20].copy_from_slice(&before.acquisition_order_counter.to_le_bytes());
        let slot = plan.get("slot").and_then(Value::as_u64).ok_or_else(|| {
            RuntimeError::LiveAddRejected {
                detail: "Stored plan content changed".to_string(),
            }
        })? as usize;
        let serial = plan.get("serial").and_then(Value::as_u64).ok_or_else(|| {
            RuntimeError::LiveAddRejected {
                detail: "Stored plan content changed".to_string(),
            }
        })?;
        let data = self.fixture.data()?;
        let container = data + self.fixture.layout.container_offset;
        self.fixture
            .memory
            .write(container + (slot * RECORD_SIZE) as u64, &destination);
        self.fixture
            .set_u32(data, before.acquisition_order_counter + 1);
        self.fixture.set_u64(data + 8, serial + 1);
        if self.faults.readback_changed {
            let untouched = container + (399 * RECORD_SIZE) as u64;
            self.fixture.memory.write(untouched, &[0x01, 0x02]);
        } else {
            self.fixture.refresh_index()?;
        }

        let mut receipt = Self::dispatch_frame(
            plan.get("function_address")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            BTreeMap::new(),
        )?;
        let object = receipt
            .as_object_mut()
            .ok_or_else(|| RuntimeError::LiveAddRejected {
                detail: "Stored plan content changed".to_string(),
            })?;
        object.insert("phase".to_string(), json!("completed"));
        object.insert("redirect_count".to_string(), json!(1));
        object.insert("released".to_string(), json!(true));
        object.insert("active".to_string(), json!(false));
        object.insert("breakpoints".to_string(), json!([]));
        object.insert("mode".to_string(), json!("single_native_insertion"));
        object.insert("operation_id".to_string(), json!(operation_id.clone()));
        object.insert(
            "pid".to_string(),
            plan.get("pid").cloned().unwrap_or(json!(0)),
        );
        object.insert("status".to_string(), json!(3));
        object.insert("slot".to_string(), json!(slot));
        object.insert("source_hex".to_string(), json!(hex(&source)));
        object.insert("destination_hex".to_string(), json!(hex(&destination)));
        object.insert(
            "remainder_hex".to_string(),
            json!(hex(&vec![0u8; RECORD_SIZE])),
        );
        object.insert(
            "process_creation_time".to_string(),
            plan.get("process_creation_time")
                .cloned()
                .unwrap_or(json!(self.fixture.creation_time.to_string())),
        );
        self.receipts.insert(operation_id.clone(), receipt.clone());
        if self.faults.reply_lost {
            return Err(RuntimeError::LiveAddRejected {
                detail: "native reply lost; query the receipt".to_string(),
            });
        }
        Ok(receipt)
    }

    fn readback(&mut self) -> Result<(Inventory, NativeIndex), RuntimeError> {
        self.fixture.capture()
    }

    fn recover(
        &mut self,
        operation_id: &str,
        _pid: u32,
        _process_creation_time: Option<&str>,
    ) -> Result<Value, RuntimeError> {
        self.receipts
            .get(operation_id)
            .cloned()
            .ok_or_else(|| RuntimeError::LiveAddRejected {
                detail: format!("no native receipt for {operation_id}"),
            })
    }

    fn require_process_instance(
        &mut self,
        _pid: u32,
        process_creation_time: Option<&str>,
    ) -> Result<(), RuntimeError> {
        if process_creation_time != Some(self.current_creation_time.as_str()) {
            return Err(RuntimeError::LiveAddRejected {
                detail: "PROCESS_INSTANCE_CHANGED: do not verify an old receipt against a new game"
                    .to_string(),
            });
        }
        Ok(())
    }

    fn submission_absent(&mut self, _operation_id: &str) -> bool {
        self.faults.submission_absent
    }

    fn safe_to_shutdown(&mut self) -> bool {
        true
    }
}

/// A save-side checkpoint that copies, verifies and "decrypts" in place.
pub struct FakeSaveBackup {
    pub root: PathBuf,
    pub counter: usize,
}

impl FakeSaveBackup {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            counter: 0,
        }
    }
}

impl SaveBackup for FakeSaveBackup {
    fn checkpoint(
        &mut self,
        source: &Path,
        raw: &[u8],
        operation_id: &str,
    ) -> Result<SaveCheckpoint, RuntimeError> {
        // The shipped order: validate the path shape, then create the backup.
        let account = source
            .parent()
            .and_then(|parent| parent.parent())
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .ok_or_else(|| RuntimeError::LiveAddRejected {
                detail: "无法从自动发现的存档中识别 Steam ID".to_string(),
            })?;
        account
            .parse::<u64>()
            .map_err(|_| RuntimeError::LiveAddRejected {
                detail: "无法从自动发现的存档中识别 Steam ID".to_string(),
            })?;
        let slot = source
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .ok_or_else(|| RuntimeError::LiveAddRejected {
                detail: "无法从存档路径识别游戏存档栏位".to_string(),
            })?;
        if !slot.starts_with("SAVEDATA") || slot[8..].parse::<u32>().is_err() {
            return Err(RuntimeError::LiveAddRejected {
                detail: "无法从存档路径识别游戏存档栏位".to_string(),
            });
        }
        self.counter += 1;
        let directory = self
            .root
            .join("backups")
            .join(format!("{operation_id}-{:03}", self.counter));
        std::fs::create_dir_all(&directory).map_err(|error| RuntimeError::Io {
            path: directory.display().to_string(),
            detail: error.to_string(),
        })?;
        let backup_path = directory.join("SAVEDATA.BIN");
        std::fs::write(&backup_path, raw).map_err(|error| RuntimeError::Io {
            path: backup_path.display().to_string(),
            detail: error.to_string(),
        })?;
        if crate::mutation::count::read_bytes(&backup_path)? != raw {
            return Err(RuntimeError::LiveAddRejected {
                detail: "Automatic save backup verification failed".to_string(),
            });
        }
        if crate::mutation::count::read_bytes(source)? != raw {
            return Err(RuntimeError::LiveAddRejected {
                detail: "Source save changed during backup".to_string(),
            });
        }
        let decrypted = directory.join("decrypted.bin");
        std::fs::write(&decrypted, raw).map_err(|error| RuntimeError::Io {
            path: decrypted.display().to_string(),
            detail: error.to_string(),
        })?;
        Ok(SaveCheckpoint {
            directory,
            backup_path,
            decrypted: raw.to_vec(),
        })
    }
}

/// A decrypted save carrying the fixture's records at the shipped offsets.
pub fn decrypted_save(fixture: &InventoryFixture) -> Result<Vec<u8>, RuntimeError> {
    let mut saved = vec![0u8; crate::mutation::live_add::SCROLL_GROUP_OFFSET + 400 * RECORD_SIZE];
    let container = fixture.container()?;
    for slot in 0..CAPACITY as usize {
        let start = slot * RECORD_SIZE;
        let record = &container[start..start + RECORD_SIZE];
        if record[0] == 0 && record[1] == 0 {
            continue;
        }
        let target = crate::mutation::live_add::SCROLL_GROUP_OFFSET + start;
        saved[target..target + RECORD_SIZE].copy_from_slice(record);
    }
    Ok(saved)
}

/// The catalog branches are the domain crate's; this one refuses to guess.
#[derive(Default)]
pub struct NoCatalogPolicy {
    pub blocker: Option<String>,
}

/// The two process views the count adapter opens, over one shared fixture.
pub struct FixtureProcesses {
    memory: std::rc::Rc<std::cell::RefCell<ByteMemory>>,
}

impl FixtureProcesses {
    pub fn new(fixture: &InventoryFixture) -> Self {
        Self {
            memory: std::rc::Rc::new(std::cell::RefCell::new(fixture.memory.clone())),
        }
    }

    pub fn memory(&self) -> std::rc::Rc<std::cell::RefCell<ByteMemory>> {
        std::rc::Rc::clone(&self.memory)
    }
}

impl crate::mutation::count::CountProcesses for FixtureProcesses {
    fn open_read(
        &self,
        _pid: u32,
    ) -> Result<Box<dyn crate::mutation::memory::TargetProcess>, RuntimeError> {
        Ok(Box::new(FixtureProcess {
            memory: std::rc::Rc::clone(&self.memory),
        }))
    }

    fn open_write(
        &self,
        _pid: u32,
    ) -> Result<Box<dyn crate::mutation::memory::TargetProcess>, RuntimeError> {
        Ok(Box::new(FixtureProcess {
            memory: std::rc::Rc::clone(&self.memory),
        }))
    }
}

/// One handle over the shared fixture memory.
pub struct FixtureProcess {
    memory: std::rc::Rc<std::cell::RefCell<ByteMemory>>,
}

impl crate::mutation::memory::TargetProcess for FixtureProcess {
    fn pid(&self) -> u32 {
        FIXTURE_PID
    }

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        self.memory.borrow().read(address, size)
    }

    fn write(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
        self.memory.borrow_mut().write(address, data);
        Ok(())
    }

    fn write_code(&mut self, address: u64, data: &[u8]) -> Result<(), RuntimeError> {
        self.write(address, data)
    }

    fn allocate_executable_near(&mut self, address: u64, size: usize) -> Result<u64, RuntimeError> {
        Err(RuntimeError::AllocationUnavailable {
            address,
            size: size as u64,
        })
    }

    fn free_allocation(&mut self, address: u64) -> Result<(), RuntimeError> {
        Err(RuntimeError::AllocationRelease { address, code: 0 })
    }

    fn exited(&mut self) -> Result<bool, RuntimeError> {
        Ok(false)
    }

    fn creation_filetime(&mut self) -> Result<Option<u64>, RuntimeError> {
        Ok(Some(FIXTURE_CREATION))
    }

    fn close(&mut self) {}
}

impl CatalogPolicy for NoCatalogPolicy {
    fn catalog_blocker(
        &mut self,
        _candidate: &InstallationCandidate,
    ) -> Result<Option<String>, RuntimeError> {
        Ok(self.blocker.clone())
    }
}

/// A canonical candidate payload for the gates.
pub fn candidate_payload(
    context_digest: &str,
    seed: u32,
    rarity: u8,
    record: &[u8],
    installation: Option<&[u8]>,
    stage: crate::mutation::live_add::CandidateStage,
) -> Result<Value, RuntimeError> {
    let candidate = InstallationCandidate {
        candidate_id: String::new(),
        context_digest: context_digest.to_string(),
        level: 1,
        seed,
        playthrough: Some(2),
        rarity,
        stage,
        record: record.to_vec(),
        installation_record: installation.map(<[u8]>::to_vec),
        effects: Vec::new(),
    };
    let identity = candidate.identity()?;
    Ok(json!({
        "candidate_id": identity,
        "context_digest": context_digest,
        "level": 1,
        "seed": seed,
        "playthrough": 2,
        "rarity": rarity,
        "record_stage": stage.value(),
        "record_hex": hex(record),
        "installation_record_hex": installation.map(hex),
        "effects": [],
    }))
}

fn hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}
