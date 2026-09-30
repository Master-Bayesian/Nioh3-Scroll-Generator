//! Native equipment generation/confirmation/insertion on the game dispatch
//! thread, with durable at-most-once claims and the shared native admission lock.
//! Equipment has its own container/counter proof; no scroll-index proof is claimed.

use super::count::{exclusive_json, is_canonical_uuid, new_operation_id, read_json, sha256_hex};
use super::evidence::{verify_dispatch, verify_dispatch_evidence};
use super::inventory::hex_decode;
use super::native_abi::{
    hex, PC_V202_CANDIDATE_EXECUTABLE_SHA256, PC_V202_EQUIPMENT_ADD as LAYOUT,
};
use super::native_executor::{
    settled, DispatchMode, LiveAddTransport, NativeDebugTransport, SAVE_GENERATION_SERIAL_MAX,
};
use super::operations::LiveAddOperations;
use crate::RuntimeError;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn rejected(text: impl Into<String>) -> RuntimeError {
    RuntimeError::LiveAddRejected {
        detail: text.into(),
    }
}
fn number(value: &Value, key: &str) -> Result<u64, RuntimeError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| rejected(format!("Missing {key}")))
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, RuntimeError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| rejected(format!("Missing {key}")))
}

pub struct EquipmentAddition {
    transport: NativeDebugTransport,
    operations: LiveAddOperations,
    requests: PathBuf,
    pending_preview: Option<String>,
    #[cfg(feature = "test-helper")]
    helper_binding: bool,
}
impl EquipmentAddition {
    pub fn new(pid: u32, state_root: &Path) -> Result<Self, RuntimeError> {
        let root = state_root.join("equipment-add");
        let requests = root.join("requests");
        std::fs::create_dir_all(&requests).map_err(|e| rejected(e.to_string()))?;
        Ok(Self {
            transport: NativeDebugTransport::new(
                pid,
                LAYOUT,
                "Nioh3.exe",
                &state_root.join("live-add/native-executor"),
            )?,
            operations: LiveAddOperations::new(&root.join("operations"))?,
            requests,
            pending_preview: None,
            #[cfg(feature = "test-helper")]
            helper_binding: false,
        })
    }
    /// Test-only dispatch against the named disposable helper, never the game.
    #[cfg(feature = "test-helper")]
    pub fn for_owned_helper(pid: u32, state_root: &Path) -> Result<Self, RuntimeError> {
        let image = crate::platform::query_image_path(pid)?;
        if !Path::new(&image).file_name().is_some_and(|name| {
            name.to_string_lossy()
                .eq_ignore_ascii_case("runtime_mutation_helper.exe")
        }) {
            return Err(rejected(
                "The helper seam only accepts runtime_mutation_helper.exe",
            ));
        }
        let mut app = Self::new(pid, state_root)?;
        app.transport = NativeDebugTransport::new(
            pid,
            LAYOUT,
            "runtime_mutation_helper.exe",
            &state_root.join("live-add/native-executor"),
        )?;
        app.helper_binding = true;
        Ok(app)
    }
    pub fn pid(&self) -> u32 {
        self.transport.pid()
    }
    pub fn safe_to_shutdown(&mut self) -> bool {
        self.transport.safe_to_shutdown()
    }
    pub fn pending_preview(&self) -> Option<&str> {
        self.pending_preview.as_deref()
    }
    fn request_path(&self, id: &str) -> Result<PathBuf, RuntimeError> {
        if !is_canonical_uuid(id) {
            return Err(rejected("Use a canonical operation UUID"));
        }
        Ok(self.requests.join(format!("{id}.json")))
    }
    pub fn operation_pid(state_root: &Path, id: &str) -> Result<u32, RuntimeError> {
        if !is_canonical_uuid(id) {
            return Err(rejected("Use a canonical operation UUID"));
        }
        let note = read_json(
            &state_root
                .join("equipment-add/requests")
                .join(format!("{id}.json")),
        )?;
        number(&note, "pid")
            .and_then(|v| u32::try_from(v).map_err(|_| rejected("Invalid process identity")))
    }
    /// A request is durable before native work. Absence proves this id never
    /// reached the coordinator; it does not require finding a running game.
    pub fn unregistered_status(state_root: &Path, id: &str) -> Result<Option<Value>, RuntimeError> {
        if !is_canonical_uuid(id) {
            return Err(rejected("Use a canonical operation UUID"));
        }
        let path = state_root
            .join("equipment-add/requests")
            .join(format!("{id}.json"));
        if path.try_exists().map_err(|e| rejected(e.to_string()))? {
            return Ok(None);
        }
        Ok(Some(
            json!({"operation_id":id,"plan_digest":Value::Null,"state":"rejected_before_dispatch",
            "process_id":Value::Null,"preview_record_hex":Value::Null,"slot_index":Value::Null,"error":Value::Null}),
        ))
    }
    fn binding(&mut self) -> Result<(), RuntimeError> {
        #[cfg(feature = "test-helper")]
        if self.helper_binding {
            let image = crate::platform::query_image_path(self.pid())?;
            if Path::new(&image).file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .eq_ignore_ascii_case("runtime_mutation_helper.exe")
            }) {
                return Ok(());
            }
            return Err(rejected("The owned helper identity changed"));
        }
        if !self
            .transport
            .executable_sha256()?
            .is_some_and(|hash| hash.eq_ignore_ascii_case(PC_V202_CANDIDATE_EXECUTABLE_SHA256))
        {
            return Err(rejected(
                "Equipment addition requires the supported PC 2.0.2.0 executable",
            ));
        }
        Ok(())
    }
    fn read_u64(&mut self, address: u64) -> Result<u64, RuntimeError> {
        let raw = self.transport.read(address, 8)?;
        Ok(u64::from_le_bytes(
            raw.try_into()
                .map_err(|_| rejected("Short equipment owner read"))?,
        ))
    }
    fn capture_once(&mut self) -> Result<Value, RuntimeError> {
        let base = self.transport.module_base()?;
        let manager = self.read_u64(base + LAYOUT.manager_pointer_rva)?;
        if manager == 0 {
            return Err(rejected("No character is loaded"));
        }
        let data = self.read_u64(manager)?;
        let player = self.read_u64(base + crate::character::PLAYER_POINTER_RVA)?;
        if data == 0
            || player == 0
            || self.read_u64(player)? != base + crate::character::PLAYER_VTABLE_RVA
            || player + crate::character::EQUIPMENT_OFFSET != data + LAYOUT.container_offset
        {
            return Err(rejected(
                "Equipment player layout does not match this build",
            ));
        }
        if self.read_u64(data + 0x10 + LAYOUT.capacity_offset)? != 2500 {
            return Err(rejected("Equipment capacity changed"));
        }
        let serial = self.read_u64(data + 8)?;
        let acquisition_order = u32::from_le_bytes(
            self.transport
                .read(data, 4)?
                .try_into()
                .map_err(|_| rejected("Short acquisition counter"))?,
        );
        if acquisition_order >= 65535 {
            return Err(rejected(
                "Equipment acquisition key cannot round-trip into the save",
            ));
        }
        if serial >= SAVE_GENERATION_SERIAL_MAX {
            return Err(rejected("Equipment serial cannot round-trip into the save"));
        }
        let container = self.transport.read(data + 0x10, 2500 * 0xF0)?;
        let builder = self
            .transport
            .read(base + LAYOUT.builder_rva, LAYOUT.builder_size as usize)?;
        let insertion = self
            .transport
            .read(base + LAYOUT.insertion_rva, LAYOUT.insertion_size as usize)?;
        let mut native_chain = Vec::new();
        for (rva, size) in [
            (0x551314, 0x26),
            (0x5513C8, 0x234),
            (0x557F34, 0x200),
            (0x552DD0, 0x128),
            (0x54C380, 0x41),
        ] {
            native_chain
                .push(json!({"rva":rva,"code_hex":hex(&self.transport.read(base+rva,size)?)}));
        }
        for (rva, signature) in [
            (0x551314, "4883EC2833C0668901488941048841"),
            (0x557F34, "488BC44889582055"),
            (0x5515FC, "48895C240848896C2410488974241857"),
            (0x54D324, "40555356574154415541564157488DAC"),
        ] {
            #[cfg(feature = "test-helper")]
            if self.helper_binding {
                continue;
            }
            let bytes = hex_decode(signature)?;
            if self.transport.read(base + rva, bytes.len())? != bytes {
                return Err(rejected("Equipment generation code identity changed"));
            }
        }
        Ok(
            json!({"pid":self.pid(),"process_creation_time":self.transport.creation_time()?,"module_base":base,
            "manager":manager,"data":data,"serial":serial,"acquisition_order":acquisition_order,"scheduler_owner":self.read_u64(base+LAYOUT.scheduler_pointer_rva)?,
            "container_hex":hex(&container),"builder_code_hex":hex(&builder),"insertion_code_hex":hex(&insertion),
            "native_chain":native_chain,"function_address":base+LAYOUT.insertion_rva}),
        )
    }
    fn capture(&mut self) -> Result<Value, RuntimeError> {
        self.binding()?;
        let first = self.capture_once()?;
        if first != self.capture_once()? {
            return Err(rejected(
                "Equipment changed during the read; try when the game is idle",
            ));
        }
        Ok(first)
    }
    fn public(&self, id: &str) -> Result<Value, RuntimeError> {
        let (digest, plan) = self.operations.plan(id)?;
        let snapshot = self.operations.snapshot(id)?;
        Ok(
            json!({"operation_id":id,"plan_digest":digest,"state":snapshot.state.as_str(),"process_id":plan["pid"],
            "preview_record_hex":plan["preview_record_hex"],"slot_index":plan["slot"],"error":Value::Null}),
        )
    }
    pub fn prepare(&mut self, id: &str, input: &Value) -> Result<Value, RuntimeError> {
        let request_path = self.request_path(id)?;
        if request_path.exists() {
            return self.status(id);
        }
        if !self.operations.unresolved_ids()?.is_empty() {
            return Err(rejected(
                "Recover the previous equipment addition before preparing another",
            ));
        }
        let mut descriptor = vec![0u8; 0xCC];
        let item =
            u16::try_from(number(input, "item_id")?).map_err(|_| rejected("Invalid item ID"))?;
        let level =
            u32::try_from(number(input, "level")?).map_err(|_| rejected("Invalid level"))?;
        let plus =
            u32::try_from(number(input, "plus")?).map_err(|_| rejected("Invalid plus level"))?;
        let rarity =
            u8::try_from(number(input, "rarity")?).map_err(|_| rejected("Invalid rarity"))?;
        let seed = u16::try_from(number(input, "seed")?)
            .map_err(|_| rejected("Invalid equipment seed"))?;
        if level == 0 || level > 65535 || plus > 65535 || rarity > 5 {
            return Err(rejected("Invalid equipment generation inputs"));
        }
        let mut capture = self.capture()?;
        let bytes = hex_decode(text(&capture, "container_hex")?)?;
        let slot = bytes
            .chunks_exact(0xF0)
            .position(|record| record[0] == 0 && record[1] == 0)
            .ok_or_else(|| rejected("The equipment inventory is full"))?;
        let child = new_operation_id()?;
        exclusive_json(
            &request_path,
            &json!({"operation_id":id,"preview_operation_id":child,"pid":self.pid(),"input":input}),
        )?;
        descriptor[..2].copy_from_slice(&item.to_le_bytes());
        descriptor[4..8].copy_from_slice(&level.to_le_bytes());
        descriptor[8..12].copy_from_slice(&plus.to_le_bytes());
        descriptor[12] = rarity;
        descriptor[0x13] = 1;
        let mut params = capture.clone();
        params["operation_id"] = json!(child);
        params["parent_operation_id"] = json!(id);
        params["descriptor_hex"] = json!(hex(&descriptor));
        params["native_equipment_seed"] = json!(seed);
        self.pending_preview = Some(child.clone());
        let dispatched = self.transport.dispatch(DispatchMode::Preview, &params);
        let receipt = match dispatched {
            Ok(value) => value,
            Err(error) => {
                let mut state = self.status(id)?;
                state["error"] = json!(error.message());
                return Ok(state);
            }
        };
        if settled(&receipt) {
            self.pending_preview = None;
        }
        if verify_dispatch(&receipt).is_err()
            || receipt["equipment_canaries_intact"] != true
            || receipt["equipment_preview_before"] != receipt["equipment_preview_after"]
        {
            return self.status(id);
        }
        let raw = hex_decode(text(&receipt, "source_hex")?)?;
        if raw.len() != 0xF0
            || raw[..2] != item.to_le_bytes()
            || raw[0x30] != rarity
            || raw[0x28..0x30] != [0xFF; 8]
            || u16::from_le_bytes([raw[0x22], raw[0x23]]) != seed
            || u16::from_le_bytes([raw[6], raw[7]]) != level as u16
            || u16::from_le_bytes([raw[10], raw[11]]) != plus as u16
        {
            return Err(rejected(
                "The native equipment preview returned a different item",
            ));
        }
        if raw[0x34..0xDC]
            .chunks_exact(0x18)
            .all(|e| e[4..8] == [0xFF; 4])
        {
            return Err(rejected(
                "This seed produces no equipment effects; choose another seed",
            ));
        }
        // A prepared item is bound to the exact inventory the nonallocating
        // preview proved unchanged, not to a new post-preview inventory.
        if self.capture()? != capture {
            return Err(rejected(
                "Equipment changed during the preview; prepare again",
            ));
        }
        capture["operation_id"] = json!(id);
        capture["kind"] = json!("equipment_native_add");
        capture["preview_operation_id"] = json!(child);
        capture["preview_record_hex"] = receipt["source_hex"].clone();
        capture["descriptor_hex"] = receipt["descriptor_hex"].clone();
        capture["slot"] = json!(slot);
        self.operations.prepare(id, &capture)?;
        self.public(id)
    }
    pub fn execute(&mut self, id: &str, digest: &str) -> Result<Value, RuntimeError> {
        let snapshot = self.operations.snapshot(id)?;
        if !snapshot.can_dispatch {
            return self.status(id);
        }
        let (stored, plan) = self.operations.plan(id)?;
        if stored != digest {
            return Err(rejected("Equipment plan digest changed"));
        }
        let current = self.capture()?;
        for key in [
            "pid",
            "process_creation_time",
            "module_base",
            "manager",
            "data",
            "serial",
            "acquisition_order",
            "scheduler_owner",
            "container_hex",
            "builder_code_hex",
            "insertion_code_hex",
            "native_chain",
        ] {
            if current[key] != plan[key] {
                return Err(rejected(
                    "Equipment changed after preview; cancel and prepare again",
                ));
            }
        }
        let mut params = plan.clone();
        let mut descriptor = hex_decode(text(&plan, "descriptor_hex")?)?;
        descriptor[0x13] = 0;
        let mut expected = hex_decode(text(&plan, "preview_record_hex")?)?;
        expected[0x28..0x30].copy_from_slice(&number(&plan, "serial")?.to_le_bytes());
        params["descriptor_hex"] = json!(hex(&descriptor));
        params["expected_record_hex"] = json!(hex(&expected));
        self.operations.claim(id, digest)?; // durable before any inserting dispatch
        let outcome = self.transport.dispatch(DispatchMode::Insert, &params);
        match outcome {
            Ok(_) => self.recover(id),
            Err(error) => {
                let mut state = self.recover(id)?;
                state["error"] = json!(error.message());
                Ok(state)
            }
        }
    }
    pub fn cancel(&mut self, id: &str) -> Result<Value, RuntimeError> {
        if self.operations.plan(id).is_err() {
            return self.recover(id);
        }
        if self.operations.snapshot(id)?.state.as_str() != "prepared" {
            return self.status(id);
        }
        self.operations.cancel(id)?;
        self.public(id)
    }
    pub fn status(&mut self, id: &str) -> Result<Value, RuntimeError> {
        if self.operations.plan(id).is_ok() {
            return self.public(id);
        }
        let note = read_json(&self.request_path(id)?)?;
        let child = text(&note, "preview_operation_id")?;
        let receipt = self.transport.status(child).ok();
        // The transport durably records a child before native work. A parent
        // left without that record never dispatched; status never retries it.
        let terminal =
            receipt.as_ref().is_some_and(settled) || !self.transport.operation_known(child);
        Ok(
            json!({"operation_id":id,"plan_digest":Value::Null,"state":if terminal {"rejected_before_dispatch"}else{"uncertain"},
            "process_id":note["pid"],"preview_record_hex":Value::Null,"slot_index":Value::Null,
            "error":receipt.as_ref().and_then(|r|r.get("error")).cloned().unwrap_or(Value::Null)}),
        )
    }
    pub fn recover(&mut self, id: &str) -> Result<Value, RuntimeError> {
        let Ok((_, plan)) = self.operations.plan(id) else {
            let note = read_json(&self.request_path(id)?)?;
            let child = text(&note, "preview_operation_id")?;
            let _ = self.transport.release(child);
            if self.transport.status(child).is_ok_and(|r| settled(&r)) {
                self.pending_preview = None;
            }
            return self.status(id);
        };
        if self.operations.snapshot(id)?.can_dispatch {
            return self.public(id);
        }
        if self.operations.snapshot(id)?.state.as_str() != "uncertain" {
            return self.public(id);
        }
        let receipt = match self.transport.release(id) {
            Ok(r) => r,
            Err(_) => return self.public(id),
        };
        if !settled(&receipt) {
            return self.public(id);
        }
        if receipt["equipment_guard_rejected_before_insertion"] == true
            && verify_dispatch_evidence(&receipt).is_ok()
        {
            let current = match self.capture() {
                Ok(value) => value,
                Err(_) => return self.public(id),
            };
            if current["process_creation_time"] == plan["process_creation_time"]
                && current["container_hex"] == plan["container_hex"]
            {
                self.operations.complete_equipment(id,&json!({"operation_id":id,"state":"rejected_before_insertion",
                    "equipment_container_unchanged_verified":true,"dispatch_and_cleanup_verified":true,"native_receipt":receipt}))?;
            }
            return self.public(id);
        }
        if receipt["redirect_count"] == 0 {
            self.operations.complete_equipment(id,&json!({"operation_id":id,"state":"rejected_before_dispatch","redirect_count":0,"native_receipt":receipt}))?;
            return self.public(id);
        }
        if verify_dispatch(&receipt).is_err() || receipt["status"] != 3 {
            return self.public(id);
        }
        let observed = match self.capture() {
            Ok(value) => value,
            Err(_) => return self.public(id),
        };
        if observed["pid"] != plan["pid"]
            || observed["process_creation_time"] != plan["process_creation_time"]
            || number(&observed, "serial")? != number(&plan, "serial")? + 1
            || number(&observed, "acquisition_order")? != number(&plan, "acquisition_order")? + 1
        {
            return self.public(id);
        }
        let slot = number(&plan, "slot")? as usize;
        if receipt["slot"] != slot {
            return self.public(id);
        }
        let mut expected = hex_decode(text(&plan, "container_hex")?)?;
        let observed_bytes = hex_decode(text(&observed, "container_hex")?)?;
        let source = hex_decode(text(&receipt, "source_hex")?)?;
        let destination = &observed_bytes[slot * 0xF0..(slot + 1) * 0xF0];
        let free = &expected[slot * 0xF0..(slot + 1) * 0xF0];
        // Exact copy 0x552DD0 plus acquisition setter 0x54C380. Opaque ranges
        // stay byte-equal to the free slot; they are not ignored or guessed.
        let fields_match = destination[..0x18] == source[..0x18]
            && destination[0x20..0x24] == source[0x20..0x24]
            && destination[0x28..0xE4] == source[0x28..0xE4]
            && destination[0x24..0x28] == free[0x24..0x28]
            && destination[0xE4..] == free[0xE4..]
            && u32::from_le_bytes(
                destination[0x1C..0x20]
                    .try_into()
                    .map_err(|_| rejected("Short key"))?,
            ) as u64
                == number(&plan, "acquisition_order")?
            && (u32::from_le_bytes(
                destination[0x18..0x1C]
                    .try_into()
                    .map_err(|_| rejected("Short record"))?,
            ) ^ u32::from_le_bytes(
                source[0x18..0x1C]
                    .try_into()
                    .map_err(|_| rejected("Short record"))?,
            )) & !0x80
                == 0;
        expected[slot * 0xF0..(slot + 1) * 0xF0].copy_from_slice(destination);
        if !fields_match || expected != observed_bytes {
            return self.public(id);
        }
        self.operations.complete_equipment(
            id,
            &json!({"operation_id":id,"state":"verified",
            "equipment_container_and_counters_verified":true,"dispatch_and_cleanup_verified":true,
            "native_receipt":receipt,"container_sha256":sha256_hex(&observed_bytes)}),
        )?;
        self.public(id)
    }
}
