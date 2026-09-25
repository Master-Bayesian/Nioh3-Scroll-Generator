//! Read-only scroll inventory capture shared by the count and live-add slices.
//!
//! Port of `live_inventory.capture_inventory` and the record helpers
//! `live_add_evidence.inventory_entries`. The gate is the shipped one: the
//! insertion function signature at `insertion_rva` must still be the approved
//! bytes, the manager/data owner must be loaded, the container capacity must be
//! the accepted 400, every live read must repeat identically, and the owner must
//! still resolve after the capture. Only then is an occupied slot exposed.
//!
//! The module reads. It opens nothing, writes nothing and allocates nothing; the
//! caller owns the process view it hands in.

use crate::error::RuntimeError;
use crate::mutation::count::sha256_hex;
use crate::mutation::memory::TargetProcess;
use std::collections::BTreeMap;

/// `SCROLL_RECORD_SIZE`.
pub const RECORD_SIZE: usize = 0xE8;
/// Byte offset of the generation seed inside one record.
pub const SEED_OFFSET: usize = 0x20;
/// Byte offset of the instance serial inside one record.
pub const SERIAL_OFFSET: usize = 0x28;
/// The accepted container capacity.
pub const CAPACITY: u32 = 400;
/// Bytes `live_inventory.capture_inventory` reads before the container.
pub const COUNTER_WINDOW: usize = 16;
/// `live_inventory.capture_inventory` insertion signature.
pub const INSERTION_SIGNATURE: [u8; 16] = [
    0x40, 0x55, 0x53, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57, 0x48, 0x8D, 0xAC,
];

/// The `live_add_profile` fields an inventory capture resolves addresses with.
///
/// `inventory_global_mode` is the explicit ABI choice mirrored from
/// `live_add_profile.InventoryGlobalMode`: one executable may expose
/// `global -> manager -> data` while another may expose `global -> data`
/// directly. A missing or unknown value is refused by
/// [`resolve_inventory_pointers`] before any dereference, so a candidate
/// layout cannot become dispatchable by supplying only an RVA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryLayout {
    pub insertion_rva: u64,
    pub manager_pointer_rva: u64,
    pub container_offset: u64,
    pub capacity_offset: u64,
    pub serial_index_offset: u64,
    pub capacity: u32,
    pub record_size: usize,
    pub serial_counter_offset: u64,
    pub inventory_global_mode: Option<&'static str>,
}

/// `live_add_profile.PC_V201`, the only layout the product validates.
pub const PC_V201_INVENTORY_LAYOUT: InventoryLayout = InventoryLayout {
    insertion_rva: 0x54_D294,
    manager_pointer_rva: 0x474D4E0,
    container_offset: 0x224A60,
    capacity_offset: 0x16A80,
    serial_index_offset: 0x23B5E8,
    capacity: CAPACITY,
    record_size: RECORD_SIZE,
    serial_counter_offset: 8,
    inventory_global_mode: Some(INVENTORY_GLOBAL_MODE_MANAGER_OBJECT),
};

/// The PC v2.02 candidate's inventory leg. Not accepted: nothing selects it.
///
/// Every address is copied from
/// [`crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE`] so the two legs
/// cannot drift, and the ABI mode is the live-verified `manager_object` shape
/// (`global -> manager -> data`), not a new one. The layout stays
/// non-dispatchable because the read gates accept the canonical display version
/// only: [`capture_inventory`] and [`capture_index`] refuse `PC v2.02` before
/// any dereference, so a candidate cannot be captured, indexed or dispatched by
/// supplying these numbers.
pub const PC_V202_INVENTORY_LAYOUT_CANDIDATE: InventoryLayout = InventoryLayout {
    insertion_rva: crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE.insertion_rva,
    manager_pointer_rva: crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE
        .manager_pointer_rva,
    container_offset: crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE.container_offset,
    capacity_offset: crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE.capacity_offset,
    serial_index_offset: crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE
        .serial_index_offset,
    capacity: crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE.capacity,
    record_size: crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE.record_size,
    serial_counter_offset: crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE
        .serial_counter_offset,
    inventory_global_mode: Some(INVENTORY_GLOBAL_MODE_MANAGER_OBJECT),
};

/// The accepted `(inventory layout, display version)` pairs.
///
/// The pair is the binding: a display version alone never authorizes a layout,
/// and a layout alone never authorizes a version. `PC v2.01` is the shipped
/// product pair; `PC v2.02` is the opt-in research candidate, whose layout is
/// only reachable through an executor whose own gate already proved the pinned
/// executable identity.
const ACCEPTED_INVENTORY_PAIRS: [(&InventoryLayout, &str, &str); 2] = [
    (
        &PC_V201_INVENTORY_LAYOUT,
        crate::mutation::native_abi::PRODUCT_DISPLAY_VERSION,
        crate::mutation::native_abi::PC_V201_LIVE_ADD.profile_id,
    ),
    (
        &PC_V202_INVENTORY_LAYOUT_CANDIDATE,
        crate::mutation::native_abi::CANDIDATE_DISPLAY_VERSION,
        crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE.profile_id,
    ),
];

/// The display version one exact layout is accepted with, if any.
pub fn accepted_inventory_version(layout: &InventoryLayout) -> Option<&'static str> {
    ACCEPTED_INVENTORY_PAIRS
        .iter()
        .find(|(accepted, _, _)| *accepted == layout)
        .map(|(_, version, _)| *version)
}

/// The live-add profile id one exact layout is accepted with, if any.
pub fn accepted_inventory_profile_id(layout: &InventoryLayout) -> Option<&'static str> {
    ACCEPTED_INVENTORY_PAIRS
        .iter()
        .find(|(accepted, _, _)| *accepted == layout)
        .map(|(_, _, profile_id)| *profile_id)
}

/// `live_add_profile.InventoryGlobalMode`: how the version-owned inventory
/// global reaches the data object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryGlobalMode {
    /// `global -> manager -> data`, the shipped PC v2.01 shape.
    ManagerObject,
    /// `global -> data`; a candidate shape with no manager object.
    DirectData,
}

/// `live_add_profile.InventoryGlobalMode.MANAGER_OBJECT.value`.
pub const INVENTORY_GLOBAL_MODE_MANAGER_OBJECT: &str = "manager_object";
/// `live_add_profile.InventoryGlobalMode.DIRECT_DATA.value`.
pub const INVENTORY_GLOBAL_MODE_DIRECT_DATA: &str = "direct_data";

impl InventoryGlobalMode {
    /// The shipped lowercase value.
    pub const fn value(self) -> &'static str {
        match self {
            Self::ManagerObject => INVENTORY_GLOBAL_MODE_MANAGER_OBJECT,
            Self::DirectData => INVENTORY_GLOBAL_MODE_DIRECT_DATA,
        }
    }

    /// `live_add_profile.InventoryGlobalMode.parse`: a missing or unknown mode
    /// is refused, so neither can silently select the manager-object read.
    pub fn parse(value: Option<&str>) -> Result<Self, RuntimeError> {
        match value {
            Some(INVENTORY_GLOBAL_MODE_MANAGER_OBJECT) => Ok(Self::ManagerObject),
            Some(INVENTORY_GLOBAL_MODE_DIRECT_DATA) => Ok(Self::DirectData),
            Some(other) => Err(invalid(&format!(
                "Unsupported inventory global mode: '{other}'"
            ))),
            None => Err(invalid("Inventory global mode is required")),
        }
    }
}

/// `live_add_profile.InventoryPointers`.
///
/// `manager_address` is the manager object only for `manager_object` mode;
/// direct-data mode has no manager object and therefore stores `None`
/// deliberately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryPointers {
    pub global_address: u64,
    pub manager_address: Option<u64>,
    pub data_address: u64,
}

/// Port of `live_add_profile.resolve_inventory_pointers`.
///
/// Missing/invalid modes and a null global fail before any dereference that
/// could be mistaken for a valid owner, and the two null-owner diagnostics
/// keep the shipped wording. Whether a layout may be dispatched at all stays
/// with the `PC v2.01` version gates in [`capture_inventory`] and
/// [`capture_index`]; this helper only mirrors the parse-and-resolve half.
pub fn resolve_inventory_pointers<M: InventoryProcess + ?Sized>(
    memory: &mut M,
    layout: &InventoryLayout,
) -> Result<InventoryPointers, RuntimeError> {
    let mode = InventoryGlobalMode::parse(layout.inventory_global_mode)?;
    if layout.manager_pointer_rva == 0 {
        return Err(invalid("Inventory global RVA is unresolved"));
    }
    let global_address = memory.module_base() + layout.manager_pointer_rva;
    let global_value = read_u64(memory, global_address)?;
    if global_value == 0 {
        return Err(match mode {
            InventoryGlobalMode::ManagerObject => invalid("Item manager is not loaded"),
            InventoryGlobalMode::DirectData => invalid("Inventory data is not loaded"),
        });
    }
    if mode == InventoryGlobalMode::DirectData {
        return Ok(InventoryPointers {
            global_address,
            manager_address: None,
            data_address: global_value,
        });
    }
    let data_address = read_u64(memory, global_value)?;
    if data_address == 0 {
        return Err(invalid("Inventory data is not loaded"));
    }
    Ok(InventoryPointers {
        global_address,
        manager_address: Some(global_value),
        data_address,
    })
}

/// The read-only process view an inventory capture needs.
pub trait InventoryProcess {
    fn pid(&self) -> u32;

    /// Image base of the module the layout RVAs are relative to.
    fn module_base(&self) -> u64;

    /// `ProcessReader.creation_time()`.
    fn creation_time(&mut self) -> Result<String, RuntimeError>;

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError>;
}

/// One occupied slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryEntry {
    pub slot_index: usize,
    pub record_hex: String,
    pub serial: String,
    pub seed: u32,
}

/// `live_inventory.capture_inventory` result, minus the timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub pid: u32,
    pub process_creation_time: String,
    pub capacity: u32,
    pub entries: Vec<InventoryEntry>,
    pub duplicate_scroll_serials: Vec<String>,
    pub serial_counter: String,
    pub acquisition_order_counter: u32,
    pub container_sha256: String,
}

/// One native serial-index entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub serial: String,
    pub slot: u32,
}

/// `live_inventory.capture_index` result, minus the timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeIndex {
    pub pid: u32,
    pub process_creation_time: String,
    pub node_count: u64,
    pub bucket_count: u64,
    pub entries: Vec<IndexEntry>,
}

impl NativeIndex {
    /// The shipped JSON shape, as a stored plan carries it.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, RuntimeError> {
        let broken = || invalid("Stored plan content changed");
        let entries = value
            .get("entries")
            .and_then(|value| value.as_array())
            .ok_or_else(broken)?
            .iter()
            .map(|entry| {
                Ok(IndexEntry {
                    serial: entry
                        .get("serial")
                        .and_then(|value| value.as_str())
                        .ok_or_else(broken)?
                        .to_string(),
                    slot: entry
                        .get("slot")
                        .and_then(|value| value.as_u64())
                        .ok_or_else(broken)? as u32,
                })
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        Ok(Self {
            pid: value
                .get("pid")
                .and_then(|value| value.as_u64())
                .ok_or_else(broken)? as u32,
            process_creation_time: value
                .get("process_creation_time")
                .and_then(|value| value.as_str())
                .ok_or_else(broken)?
                .to_string(),
            node_count: value
                .get("node_count")
                .and_then(|value| value.as_u64())
                .ok_or_else(broken)?,
            bucket_count: value
                .get("bucket_count")
                .and_then(|value| value.as_u64())
                .ok_or_else(broken)?,
            entries,
        })
    }

    /// The shipped JSON shape the receipts carry.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": "nioh3-native-serial-index/v1",
            "pid": self.pid,
            "read_only": true,
            "process_creation_time": self.process_creation_time,
            "node_count": self.node_count,
            "bucket_count": self.bucket_count,
            "entries": self
                .entries
                .iter()
                .map(|entry| serde_json::json!({"serial": entry.serial, "slot": entry.slot}))
                .collect::<Vec<_>>(),
            "scope": "Native FNV bucket/list agreement; historical unused keys may remain. Not an atomic snapshot.",
        })
    }

    /// `live_add_evidence.index_entries`: serial -> slot.
    pub fn entries_by_serial(&self) -> Result<BTreeMap<String, u32>, RuntimeError> {
        index_entries(&self.to_json())
    }

    /// The slot the native index resolves for one serial, when it has one.
    pub fn slot_of(&self, serial: &str) -> Option<u32> {
        self.entries
            .iter()
            .find(|entry| entry.serial == serial)
            .map(|entry| entry.slot)
    }
}

/// `live_add_evidence.index_entries` over the shipped JSON shape.
pub fn index_entries(value: &serde_json::Value) -> Result<BTreeMap<String, u32>, RuntimeError> {
    let items = value
        .get("entries")
        .and_then(|value| value.as_array())
        .ok_or_else(|| invalid("Duplicate keys or incorrect index node count"))?;
    let node_count = value
        .get("node_count")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| invalid("Duplicate keys or incorrect index node count"))?;
    let mut entries: BTreeMap<String, u32> = BTreeMap::new();
    for item in items {
        let serial = item
            .get("serial")
            .and_then(|value| value.as_str())
            .ok_or_else(|| invalid("Duplicate keys or incorrect index node count"))?;
        let slot = item
            .get("slot")
            .and_then(|value| value.as_u64())
            .ok_or_else(|| invalid("Duplicate keys or incorrect index node count"))?;
        if entries.insert(serial.to_string(), slot as u32).is_some() {
            return Err(invalid("Duplicate keys or incorrect index node count"));
        }
    }
    if entries.len() as u64 != items.len() as u64 || entries.len() as u64 != node_count {
        return Err(invalid("Duplicate keys or incorrect index node count"));
    }
    Ok(entries)
}

/// FNV-1a over the little-endian serial, as `live_inventory.fnv` computes it.
pub fn fnv(serial: u64) -> u64 {
    let mut value: u64 = 0xCBF2_9CE4_8422_2325;
    for byte in serial.to_le_bytes() {
        value = (value ^ byte as u64).wrapping_mul(0x100_0000_01B3);
    }
    value
}

fn u64_at(raw: &[u8], offset: usize) -> Result<u64, RuntimeError> {
    let slice = raw
        .get(offset..offset + 8)
        .ok_or_else(|| invalid("Invalid bounded serial-index header"))?;
    Ok(u64::from_le_bytes(slice.try_into().map_err(|_| {
        invalid("Invalid bounded serial-index header")
    })?))
}

/// Port of `live_inventory.inspect`: bounded FNV bucket/list agreement.
pub fn inspect_index<M: InventoryProcess + ?Sized>(
    memory: &mut M,
    address: u64,
) -> Result<(u64, u64, Vec<IndexEntry>), RuntimeError> {
    let header = memory.read(address, 0x40)?;
    let head = u64_at(&header, 8)?;
    let size = u64_at(&header, 16)?;
    let buckets = u64_at(&header, 24)?;
    let mask = u64_at(&header, 0x30)?;
    let bucket_count = u64_at(&header, 0x38)?;
    if head == 0
        || buckets == 0
        || bucket_count == 0
        || bucket_count > 1 << 20
        || mask != bucket_count - 1
        || bucket_count & mask != 0
        || size > 10_000
    {
        return Err(invalid("Invalid bounded serial-index header"));
    }

    // The list is walked first, then every bucket is resolved through it, and
    // every raw read is repeated at the end.
    let mut seen: BTreeMap<u64, (String, u32)> = BTreeMap::new();
    let mut snapshots: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut node = read_u64(memory, head)?;
    while node != head {
        if node == 0 || seen.contains_key(&node) || seen.len() as u64 >= size {
            return Err(invalid("Invalid serial-index list topology"));
        }
        let raw = memory.read(node, 0x20)?;
        let next = u64_at(&raw, 0)?;
        let serial = u64_at(&raw, 0x10)?.to_string();
        let slot = u32::from_le_bytes(
            raw[0x18..0x1C]
                .try_into()
                .map_err(|_| invalid("Invalid serial-index list topology"))?,
        );
        if seen.values().any(|entry| entry.0 == serial) {
            return Err(invalid("Duplicate full serial index key"));
        }
        seen.insert(node, (serial, slot));
        snapshots.push((node, raw));
        node = next;
    }
    if seen.len() as u64 != size {
        return Err(invalid("Index list size mismatch"));
    }

    for (node_address, (serial, _slot)) in &seen {
        let serial_value: u64 = serial
            .parse()
            .map_err(|_| invalid("Duplicate full serial index key"))?;
        let start = buckets + (fnv(serial_value) & mask) * 16;
        let raw_bucket = memory.read(start, 16)?;
        let first = u64_at(&raw_bucket, 0)?;
        let mut current = u64_at(&raw_bucket, 8)?;
        let mut traversed: Vec<u64> = Vec::new();
        loop {
            if current == head {
                return Err(invalid("Empty bucket for an indexed serial"));
            }
            if traversed.contains(&current) || !seen.contains_key(&current) {
                return Err(invalid("Invalid hash bucket topology"));
            }
            traversed.push(current);
            let raw = memory.read(current, 0x20)?;
            if u64_at(&raw, 0x10)? == serial_value {
                if current != *node_address {
                    return Err(invalid("Bucket/list lookup disagreement"));
                }
                break;
            }
            if current == first {
                return Err(invalid("Serial missing from its FNV bucket"));
            }
            current = u64_at(&raw, 8)?;
        }
        snapshots.push((start, raw_bucket));
    }
    if header != memory.read(address, 0x40)? {
        return Err(invalid("Serial index changed during capture"));
    }
    for (snapshot_address, raw) in snapshots {
        if raw != memory.read(snapshot_address, raw.len())? {
            return Err(invalid("Serial index changed during capture"));
        }
    }
    let mut ordered: Vec<(u64, IndexEntry)> = seen
        .values()
        .map(|(serial, slot)| {
            (
                serial.parse::<u64>().unwrap_or(u64::MAX),
                IndexEntry {
                    serial: serial.clone(),
                    slot: *slot,
                },
            )
        })
        .collect();
    ordered.sort_by_key(|entry| entry.0);
    let entries: Vec<IndexEntry> = ordered.into_iter().map(|entry| entry.1).collect();
    Ok((size, bucket_count, entries))
}

/// Port of `live_inventory.capture_index`.
pub fn capture_index<M: InventoryProcess + ?Sized>(
    memory: &mut M,
    layout: &InventoryLayout,
    game_version: &str,
) -> Result<NativeIndex, RuntimeError> {
    if accepted_inventory_version(layout) != Some(game_version) {
        return Err(invalid("PC v2.01 is required"));
    }
    let pointers = resolve_inventory_pointers(memory, layout)?;
    let data = pointers.data_address;
    if read_u64(
        memory,
        data + layout.container_offset + layout.capacity_offset,
    )? != layout.capacity as u64
    {
        return Err(invalid("Unexpected inventory owner"));
    }
    let creation_time = memory.creation_time()?;
    let (node_count, bucket_count, entries) =
        inspect_index(memory, data + layout.serial_index_offset)?;
    if resolve_inventory_pointers(memory, layout)? != pointers {
        return Err(invalid("Inventory owner changed"));
    }
    Ok(NativeIndex {
        pid: memory.pid(),
        process_creation_time: creation_time,
        node_count,
        bucket_count,
        entries,
    })
}

/// The shipped JSON shape `live_inventory.capture_inventory` returns.
pub fn inventory_json(inventory: &Inventory, game_version: &str) -> serde_json::Value {
    serde_json::json!({
        "schema": "nioh3-live-scroll-readonly/v1",
        "pid": inventory.pid,
        "process_creation_time": inventory.process_creation_time,
        "game_version": game_version,
        "read_only": true,
        "capacity": inventory.capacity,
        "entries": inventory
            .entries
            .iter()
            .map(|entry| serde_json::json!({
                "slot_index": entry.slot_index,
                "record_hex": entry.record_hex,
                "serial": entry.serial,
                "seed": entry.seed,
            }))
            .collect::<Vec<_>>(),
        "duplicate_scroll_serials": inventory.duplicate_scroll_serials,
        "serial_counter": inventory.serial_counter,
        "acquisition_order_counter": inventory.acquisition_order_counter,
        "container_sha256": inventory.container_sha256,
        "consistency": "Matching whole-container, counters and owner reads; not an atomic engine snapshot",
        "scope": "Inventory inspection only; no native invocation, serial reservation, mutation or proof of thread safety",
    })
}

/// The complete shipped read: container capture plus native serial index.
pub fn capture_read_only<M: InventoryProcess + ?Sized>(
    memory: &mut M,
    layout: &InventoryLayout,
    game_version: &str,
) -> Result<(Inventory, NativeIndex), RuntimeError> {
    let inventory = capture_inventory(memory, layout, game_version)?;
    let index = capture_index(memory, layout, game_version)?;
    Ok((inventory, index))
}

impl Inventory {
    /// The occupied records in slot order.
    pub fn entry(&self, slot_index: usize) -> Option<&InventoryEntry> {
        self.entries
            .iter()
            .find(|entry| entry.slot_index == slot_index)
    }

    /// The shipped JSON shape, as a stored plan carries it.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, RuntimeError> {
        let broken = || invalid("Stored plan content changed");
        let entries = value
            .get("entries")
            .and_then(|value| value.as_array())
            .ok_or_else(broken)?
            .iter()
            .map(|entry| {
                Ok(InventoryEntry {
                    slot_index: entry
                        .get("slot_index")
                        .and_then(|value| value.as_u64())
                        .ok_or_else(broken)? as usize,
                    record_hex: entry
                        .get("record_hex")
                        .and_then(|value| value.as_str())
                        .ok_or_else(broken)?
                        .to_string(),
                    serial: entry
                        .get("serial")
                        .and_then(|value| value.as_str())
                        .ok_or_else(broken)?
                        .to_string(),
                    seed: entry
                        .get("seed")
                        .and_then(|value| value.as_u64())
                        .ok_or_else(broken)? as u32,
                })
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        Ok(Self {
            pid: value
                .get("pid")
                .and_then(|value| value.as_u64())
                .ok_or_else(broken)? as u32,
            process_creation_time: value
                .get("process_creation_time")
                .and_then(|value| value.as_str())
                .ok_or_else(broken)?
                .to_string(),
            capacity: value
                .get("capacity")
                .and_then(|value| value.as_u64())
                .ok_or_else(broken)? as u32,
            entries,
            duplicate_scroll_serials: value
                .get("duplicate_scroll_serials")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            serial_counter: value
                .get("serial_counter")
                .and_then(|value| value.as_str())
                .ok_or_else(broken)?
                .to_string(),
            acquisition_order_counter: value
                .get("acquisition_order_counter")
                .and_then(|value| value.as_u64())
                .ok_or_else(broken)? as u32,
            container_sha256: value
                .get("container_sha256")
                .and_then(|value| value.as_str())
                .ok_or_else(broken)?
                .to_string(),
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

pub(crate) fn hex_decode(text: &str) -> Result<Vec<u8>, RuntimeError> {
    if !text.len().is_multiple_of(2) {
        return Err(RuntimeError::InventoryInvalid {
            detail: "record hex has an odd length".to_string(),
        });
    }
    let digits: Vec<u8> = text.bytes().collect();
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in digits.chunks(2) {
        let high =
            (pair[0] as char)
                .to_digit(16)
                .ok_or_else(|| RuntimeError::InventoryInvalid {
                    detail: "record hex is not hexadecimal".to_string(),
                })?;
        let low = (pair[1] as char)
            .to_digit(16)
            .ok_or_else(|| RuntimeError::InventoryInvalid {
                detail: "record hex is not hexadecimal".to_string(),
            })?;
        bytes.push((high * 16 + low) as u8);
    }
    Ok(bytes)
}

pub(crate) fn invalid(detail: &str) -> RuntimeError {
    RuntimeError::InventoryInvalid {
        detail: detail.to_string(),
    }
}

fn read_u64<M: InventoryProcess + ?Sized>(
    memory: &mut M,
    address: u64,
) -> Result<u64, RuntimeError> {
    let raw = memory.read(address, 8)?;
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&raw[..8]);
    Ok(u64::from_le_bytes(bytes))
}

/// Port of `live_inventory.capture_inventory`.
///
/// The timestamp and the descriptive scope strings are presentation; everything
/// the gates act on is returned here.
pub fn capture_inventory<M: InventoryProcess + ?Sized>(
    memory: &mut M,
    layout: &InventoryLayout,
    game_version: &str,
) -> Result<Inventory, RuntimeError> {
    if accepted_inventory_version(layout) != Some(game_version) {
        return Err(invalid(
            "This inventory layout is validated only for PC v2.01",
        ));
    }
    let base = memory.module_base();
    let signature = memory.read(base + layout.insertion_rva, INSERTION_SIGNATURE.len())?;
    if signature != INSERTION_SIGNATURE {
        return Err(invalid("Inventory insertion signature mismatch"));
    }
    let pointers = resolve_inventory_pointers(memory, layout)?;
    let data = pointers.data_address;
    let container = data + layout.container_offset;
    if read_u64(memory, container + layout.capacity_offset)? != layout.capacity as u64 {
        return Err(invalid("Unexpected scroll container capacity"));
    }
    let creation_time = memory.creation_time()?;
    let counters = memory.read(data, COUNTER_WINDOW)?;
    let size = layout.capacity as usize * layout.record_size;
    let raw = memory.read(container, size)?;
    if raw != memory.read(container, raw.len())? || counters != memory.read(data, COUNTER_WINDOW)? {
        return Err(invalid(
            "Inventory or counters changed during capture; retry at rest",
        ));
    }
    if resolve_inventory_pointers(memory, layout)? != pointers {
        return Err(invalid("Inventory owner changed during capture"));
    }

    let mut entries = Vec::new();
    for slot in 0..layout.capacity as usize {
        let start = slot * layout.record_size;
        let record = &raw[start..start + layout.record_size];
        if u16::from_le_bytes([record[0], record[1]]) == 0 {
            continue;
        }
        let serial = u64::from_le_bytes(
            record[SERIAL_OFFSET..SERIAL_OFFSET + 8]
                .try_into()
                .map_err(|_| invalid("Expected full saved record"))?,
        );
        entries.push(InventoryEntry {
            slot_index: slot,
            record_hex: hex(record),
            serial: serial.to_string(),
            seed: u32::from_le_bytes(
                record[SEED_OFFSET..SEED_OFFSET + 4]
                    .try_into()
                    .map_err(|_| invalid("Expected full saved record"))?,
            ),
        });
    }
    let mut duplicates = Vec::new();
    for entry in &entries {
        if entries
            .iter()
            .filter(|other| other.serial == entry.serial)
            .count()
            > 1
            && !duplicates.contains(&entry.serial)
        {
            duplicates.push(entry.serial.clone());
        }
    }
    duplicates.sort();

    let acquisition_order_counter = u32::from_le_bytes(
        counters[..4]
            .try_into()
            .map_err(|_| invalid("Expected counters at rest"))?,
    );
    let mut counter_bytes = [0u8; 8];
    let start = layout.serial_counter_offset as usize;
    counter_bytes.copy_from_slice(
        counters
            .get(start..start + 8)
            .ok_or_else(|| invalid("Expected counters at rest"))?,
    );
    Ok(Inventory {
        pid: memory.pid(),
        process_creation_time: creation_time,
        capacity: layout.capacity,
        entries,
        duplicate_scroll_serials: duplicates,
        serial_counter: u64::from_le_bytes(counter_bytes).to_string(),
        acquisition_order_counter,
        container_sha256: sha256_hex(&raw),
    })
}

/// Port of `live_add_evidence.inventory_slots`: occupied slot -> validated entry.
///
/// Unlike [`inventory_entries`] this keeps records that share an existing
/// `+0x28` serial: saves in the wild carry such duplicates and the game loads
/// them. Live addition keys old records by their physical slot and only
/// requires the *new* serial to be unused.
pub fn inventory_slots(
    inventory: &Inventory,
) -> Result<BTreeMap<usize, InventoryEntry>, RuntimeError> {
    if inventory.capacity != CAPACITY {
        return Err(invalid("Invalid inventory capacity"));
    }
    let mut slots: BTreeMap<usize, InventoryEntry> = BTreeMap::new();
    for entry in &inventory.entries {
        let raw = hex_decode(&entry.record_hex)?;
        if raw.len() != RECORD_SIZE {
            return Err(invalid("Invalid scroll record length"));
        }
        let serial = u64::from_le_bytes(
            raw[SERIAL_OFFSET..SERIAL_OFFSET + 8]
                .try_into()
                .map_err(|_| invalid("Invalid scroll record length"))?,
        )
        .to_string();
        let seed = u32::from_le_bytes(
            raw[SEED_OFFSET..SEED_OFFSET + 4]
                .try_into()
                .map_err(|_| invalid("Invalid scroll record length"))?,
        );
        if slots.contains_key(&entry.slot_index)
            || entry.slot_index >= CAPACITY as usize
            || serial != entry.serial
            || seed != entry.seed
            || u16::from_le_bytes([raw[0], raw[1]]) == 0
        {
            return Err(invalid("Invalid or duplicate occupied record"));
        }
        slots.insert(entry.slot_index, entry.clone());
    }
    Ok(slots)
}

/// Port of `live_add_evidence.index_resolves`: every occupied serial maps to
/// its slot, or to one slot of its duplicate group.
pub fn index_resolves(
    index: &BTreeMap<String, u32>,
    slots: &BTreeMap<usize, InventoryEntry>,
) -> bool {
    let mut groups: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for (slot, entry) in slots {
        groups
            .entry(entry.serial.as_str())
            .or_default()
            .push(*slot as u32);
    }
    groups
        .iter()
        .all(|(serial, group)| index.get(*serial).is_some_and(|slot| group.contains(slot)))
}

/// Port of `live_add_evidence.inventory_entries`: serial -> validated entry.
pub fn inventory_entries(
    inventory: &Inventory,
) -> Result<BTreeMap<String, InventoryEntry>, RuntimeError> {
    if inventory.capacity != CAPACITY || !inventory.duplicate_scroll_serials.is_empty() {
        return Err(invalid("Invalid inventory capacity or duplicate serials"));
    }
    let mut entries: BTreeMap<String, InventoryEntry> = BTreeMap::new();
    let mut slots = Vec::new();
    for entry in &inventory.entries {
        let raw = hex_decode(&entry.record_hex)?;
        if raw.len() != RECORD_SIZE {
            return Err(invalid("Invalid scroll record length"));
        }
        let serial = u64::from_le_bytes(
            raw[SERIAL_OFFSET..SERIAL_OFFSET + 8]
                .try_into()
                .map_err(|_| invalid("Invalid scroll record length"))?,
        )
        .to_string();
        let seed = u32::from_le_bytes(
            raw[SEED_OFFSET..SEED_OFFSET + 4]
                .try_into()
                .map_err(|_| invalid("Invalid scroll record length"))?,
        );
        if entries.contains_key(&entry.serial)
            || slots.contains(&entry.slot_index)
            || entry.slot_index >= CAPACITY as usize
            || serial != entry.serial
            || seed != entry.seed
            || u16::from_le_bytes([raw[0], raw[1]]) == 0
        {
            return Err(invalid("Invalid or duplicate occupied record"));
        }
        slots.push(entry.slot_index);
        entries.insert(entry.serial.clone(), entry.clone());
    }
    Ok(entries)
}

/// One read view plus its image base, the shape the layout RVAs need.
pub struct ReadView<'a> {
    pub pid: u32,
    pub module_base: u64,
    pub reader: &'a mut dyn TargetProcess,
}

impl InventoryProcess for ReadView<'_> {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn module_base(&self) -> u64 {
        self.module_base
    }

    fn creation_time(&mut self) -> Result<String, RuntimeError> {
        Ok(match self.reader.creation_filetime()? {
            Some(value) => value.to_string(),
            None => "unknown".to_string(),
        })
    }

    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        self.reader.read(address, size)
    }
}

/// Read-only Windows binding: one `PROCESS_VM_READ | QUERY_INFORMATION` view.
#[cfg(windows)]
pub struct WindowsInventory {
    pid: u32,
    module_base: u64,
    reader: crate::mutation::memory::WindowsProcess,
}

#[cfg(windows)]
impl WindowsInventory {
    /// Resolve the running module base, then open the read-only view.
    pub fn open(pid: u32, module_name: &str) -> Result<Self, RuntimeError> {
        let module_base = crate::platform::module_range(pid, module_name)?.base;
        Ok(Self {
            pid,
            module_base,
            reader: crate::mutation::memory::WindowsProcess::open_read(pid)?,
        })
    }

    /// The complete `live_inventory.capture_inventory` result.
    pub fn capture(
        &mut self,
        layout: &InventoryLayout,
        game_version: &str,
    ) -> Result<Inventory, RuntimeError> {
        let mut view = ReadView {
            pid: self.pid,
            module_base: self.module_base,
            reader: &mut self.reader,
        };
        capture_inventory(&mut view, layout, game_version)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        capture_index, capture_inventory, resolve_inventory_pointers, InventoryGlobalMode,
        InventoryLayout, InventoryProcess, INVENTORY_GLOBAL_MODE_DIRECT_DATA,
        INVENTORY_GLOBAL_MODE_MANAGER_OBJECT, PC_V201_INVENTORY_LAYOUT,
        PC_V202_INVENTORY_LAYOUT_CANDIDATE,
    };
    use crate::error::RuntimeError;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    /// A little-endian value map that records every read, so a test can prove
    /// which dereferences happened before an error.
    struct RecordingMemory {
        base: u64,
        values: BTreeMap<u64, u64>,
        reads: RefCell<Vec<(u64, usize)>>,
    }

    impl RecordingMemory {
        fn with(values: &[(u64, u64)]) -> Self {
            Self {
                base: 0x1_0000_0000,
                values: values.iter().copied().collect(),
                reads: RefCell::new(Vec::new()),
            }
        }

        fn reads(&self) -> Vec<(u64, usize)> {
            self.reads.borrow().clone()
        }
    }

    impl InventoryProcess for RecordingMemory {
        fn pid(&self) -> u32 {
            4321
        }

        fn module_base(&self) -> u64 {
            self.base
        }

        fn creation_time(&mut self) -> Result<String, RuntimeError> {
            Ok("test".to_string())
        }

        fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
            self.reads.borrow_mut().push((address, size));
            let value = self.values.get(&address).copied().unwrap_or(0);
            Ok(value.to_le_bytes()[..size.min(8)].to_vec())
        }
    }

    fn layout(mode: Option<&'static str>) -> InventoryLayout {
        InventoryLayout {
            inventory_global_mode: mode,
            ..PC_V201_INVENTORY_LAYOUT
        }
    }

    #[test]
    fn the_shipped_v2_01_layout_carries_the_manager_object_mode() {
        assert_eq!(
            PC_V201_INVENTORY_LAYOUT.inventory_global_mode,
            Some(INVENTORY_GLOBAL_MODE_MANAGER_OBJECT)
        );
        assert_eq!(
            InventoryGlobalMode::parse(PC_V201_INVENTORY_LAYOUT.inventory_global_mode).ok(),
            Some(InventoryGlobalMode::ManagerObject)
        );
        assert_eq!(InventoryGlobalMode::ManagerObject.value(), "manager_object");
        assert_eq!(InventoryGlobalMode::DirectData.value(), "direct_data");
    }

    #[test]
    fn a_missing_or_unknown_global_mode_is_refused_before_any_read() {
        let mut memory = RecordingMemory::with(&[]);
        let missing = resolve_inventory_pointers(&mut memory, &layout(None)).err();
        assert_eq!(
            missing.as_ref().map(RuntimeError::code),
            Some("INVENTORY_INVALID")
        );
        assert_eq!(
            missing.map(|error| error.message()),
            Some("Inventory global mode is required".to_string())
        );
        assert!(memory.reads().is_empty());

        let unknown = resolve_inventory_pointers(&mut memory, &layout(Some("global_direct"))).err();
        assert_eq!(
            unknown.map(|error| error.message()),
            Some("Unsupported inventory global mode: 'global_direct'".to_string())
        );
        assert!(memory.reads().is_empty());

        let unresolved = resolve_inventory_pointers(
            &mut memory,
            &InventoryLayout {
                manager_pointer_rva: 0,
                ..PC_V201_INVENTORY_LAYOUT
            },
        )
        .err();
        assert_eq!(
            unresolved.map(|error| error.message()),
            Some("Inventory global RVA is unresolved".to_string())
        );
        assert!(memory.reads().is_empty());
    }

    #[test]
    fn manager_object_mode_dereferences_the_manager_before_the_data() -> Result<(), RuntimeError> {
        let base = 0x1_0000_0000;
        let slot = base + PC_V201_INVENTORY_LAYOUT.manager_pointer_rva;
        let manager = base + 0x1_0000;
        let data = base + 0x2_0000;
        let mut memory = RecordingMemory::with(&[(slot, manager), (manager, data)]);
        let pointers = resolve_inventory_pointers(&mut memory, &PC_V201_INVENTORY_LAYOUT)?;
        assert_eq!(pointers.global_address, slot);
        assert_eq!(pointers.manager_address, Some(manager));
        assert_eq!(pointers.data_address, data);
        assert_eq!(memory.reads(), vec![(slot, 8), (manager, 8)]);
        Ok(())
    }

    #[test]
    fn direct_data_mode_resolves_the_global_as_data_without_a_manager_read(
    ) -> Result<(), RuntimeError> {
        let base = 0x1_0000_0000;
        let slot = base + PC_V201_INVENTORY_LAYOUT.manager_pointer_rva;
        let data = base + 0x2_0000;
        let mut memory = RecordingMemory::with(&[(slot, data)]);
        let pointers = resolve_inventory_pointers(
            &mut memory,
            &layout(Some(INVENTORY_GLOBAL_MODE_DIRECT_DATA)),
        )?;
        assert_eq!(pointers.global_address, slot);
        assert_eq!(pointers.manager_address, None);
        assert_eq!(pointers.data_address, data);
        assert_eq!(memory.reads(), vec![(slot, 8)]);
        Ok(())
    }

    #[test]
    fn a_null_global_keeps_the_shipped_mode_specific_diagnostics() {
        let mut memory = RecordingMemory::with(&[]);
        let missing_manager =
            resolve_inventory_pointers(&mut memory, &PC_V201_INVENTORY_LAYOUT).err();
        assert_eq!(
            missing_manager.map(|error| error.message()),
            Some("Item manager is not loaded".to_string())
        );
        let missing_data = resolve_inventory_pointers(
            &mut memory,
            &layout(Some(INVENTORY_GLOBAL_MODE_DIRECT_DATA)),
        )
        .err();
        assert_eq!(
            missing_data.map(|error| error.message()),
            Some("Inventory data is not loaded".to_string())
        );
    }

    /// The executable non-enablement gate: a direct-data candidate layout
    /// cannot be reached through a non-v2.01 version, so PC v2.02 stays
    /// non-dispatchable in the Rust mirror and the shipped v2.01 layout keeps
    /// the manager-object ABI.
    #[test]
    fn a_direct_data_candidate_stays_non_dispatchable_for_v2_02() {
        let mut memory = RecordingMemory::with(&[]);
        let candidate = layout(Some(INVENTORY_GLOBAL_MODE_DIRECT_DATA));
        let inventory = capture_inventory(&mut memory, &candidate, "PC v2.02").err();
        assert_eq!(
            inventory.map(|error| error.message()),
            Some("This inventory layout is validated only for PC v2.01".to_string())
        );
        let index = capture_index(&mut memory, &candidate, "PC v2.02").err();
        assert_eq!(
            index.map(|error| error.message()),
            Some("PC v2.01 is required".to_string())
        );
        assert!(memory.reads().is_empty());
        assert_ne!(
            PC_V201_INVENTORY_LAYOUT.inventory_global_mode,
            Some(INVENTORY_GLOBAL_MODE_DIRECT_DATA)
        );
    }

    /// The candidate's own numbers stay coherent for the accepted ABI: the
    /// live-verified v2.02 global resolves `global -> manager -> data`.
    #[test]
    fn the_v2_02_candidate_inventory_layout_resolves_the_manager_object_chain(
    ) -> Result<(), RuntimeError> {
        let layout = PC_V202_INVENTORY_LAYOUT_CANDIDATE;
        assert_eq!(
            layout.inventory_global_mode,
            Some(INVENTORY_GLOBAL_MODE_MANAGER_OBJECT)
        );
        let base = 0x1_0000_0000;
        let slot = base + layout.manager_pointer_rva;
        let manager = base + 0x1_0000;
        let data = base + 0x2_0000;
        let mut memory = RecordingMemory::with(&[(slot, manager), (manager, data)]);
        let pointers = resolve_inventory_pointers(&mut memory, &layout)?;
        assert_eq!(pointers.global_address, slot);
        assert_eq!(pointers.manager_address, Some(manager));
        assert_eq!(pointers.data_address, data);
        assert_eq!(memory.reads(), vec![(slot, 8), (manager, 8)]);
        // The two legs are one definition, so the candidate's inventory view is
        // the live-add candidate's view and not a second copy of the numbers.
        let live_add = crate::mutation::native_abi::PC_V202_LIVE_ADD_CANDIDATE;
        assert_eq!(layout.insertion_rva, live_add.insertion_rva);
        assert_eq!(layout.manager_pointer_rva, live_add.manager_pointer_rva);
        assert_eq!(layout.container_offset, live_add.container_offset);
        assert_eq!(layout.capacity_offset, live_add.capacity_offset);
        assert_eq!(layout.serial_index_offset, live_add.serial_index_offset);
        assert_eq!(layout.serial_counter_offset, live_add.serial_counter_offset);
        assert_eq!(layout.record_size, live_add.record_size);
        assert_eq!(layout.capacity, live_add.capacity);
        Ok(())
    }

    /// The pair is the binding: a version string never authorizes a layout, and
    /// a layout never authorizes a version, so the inverted pairs are refused
    /// before any dereference while the accepted candidate pair opens the read.
    #[test]
    fn the_candidate_inventory_pair_is_accepted_only_together() {
        let mut memory = RecordingMemory::with(&[]);
        for (bound, version) in [
            (PC_V201_INVENTORY_LAYOUT, "PC v2.02"),
            (PC_V202_INVENTORY_LAYOUT_CANDIDATE, "PC v2.01"),
        ] {
            let inventory = capture_inventory(&mut memory, &bound, version).err();
            assert_eq!(
                inventory.map(|error| error.message()),
                Some("This inventory layout is validated only for PC v2.01".to_string())
            );
            let index = capture_index(&mut memory, &bound, version).err();
            assert_eq!(
                index.map(|error| error.message()),
                Some("PC v2.01 is required".to_string())
            );
            assert!(
                memory.reads().is_empty(),
                "{version} must be refused before any read"
            );
        }
        // The accepted candidate pair passes the version gate and stops at the
        // next gate on synthetic memory, so the read path provably opened.
        let signature =
            capture_inventory(&mut memory, &PC_V202_INVENTORY_LAYOUT_CANDIDATE, "PC v2.02").err();
        assert_eq!(
            signature.map(|error| error.message()),
            Some("Inventory insertion signature mismatch".to_string())
        );
        assert!(!memory.reads().is_empty());
        // The shipped layout keeps the accepted numbers and stays distinct.
        assert_eq!(PC_V201_INVENTORY_LAYOUT.insertion_rva, 0x54_D294);
        assert_eq!(PC_V201_INVENTORY_LAYOUT.manager_pointer_rva, 0x474D4E0);
        assert_ne!(PC_V201_INVENTORY_LAYOUT, PC_V202_INVENTORY_LAYOUT_CANDIDATE);
    }
}
