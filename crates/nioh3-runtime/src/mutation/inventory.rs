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
};

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
    if game_version != "PC v2.01" {
        return Err(invalid("PC v2.01 is required"));
    }
    let base = memory.module_base();
    let manager_address = base + layout.manager_pointer_rva;
    let manager = read_u64(memory, manager_address)?;
    let data = read_u64(memory, manager)?;
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
    if read_u64(memory, manager_address)? != manager || read_u64(memory, manager)? != data {
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
    if game_version != "PC v2.01" {
        return Err(invalid(
            "This inventory layout is validated only for PC v2.01",
        ));
    }
    let base = memory.module_base();
    let signature = memory.read(base + layout.insertion_rva, INSERTION_SIGNATURE.len())?;
    if signature != INSERTION_SIGNATURE {
        return Err(invalid("Inventory insertion signature mismatch"));
    }
    let manager_address = base + layout.manager_pointer_rva;
    let manager = read_u64(memory, manager_address)?;
    if manager == 0 {
        return Err(invalid("Item manager is not loaded"));
    }
    let data = read_u64(memory, manager)?;
    if data == 0 {
        return Err(invalid("Inventory data is not loaded"));
    }
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
    if read_u64(memory, manager_address)? != manager || read_u64(memory, manager)? != data {
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
