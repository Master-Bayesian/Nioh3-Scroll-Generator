//! Read-only runtime inventory snapshot (the v0.8.1 equipment browser read).
//!
//! This module is deliberately separate from [`crate::profile`]. The generation
//! and live-add profiles pin the generation chain and carry an approval gate;
//! this surface pins its own four candidate-container sites and exactly one
//! executable build. Nothing here resolves a generation profile and nothing here
//! changes one, so the generation site set, its approvals and its identity
//! digest are untouched by this module.
//!
//! Boundaries kept from the bounded observation this ports:
//!
//! - one read-only handle, obtained through
//!   [`crate::platform::ReadOnlyProcess`] (`PROCESS_QUERY_INFORMATION |
//!   PROCESS_VM_READ` only);
//! - a targeted region query before every read, through
//!   [`crate::platform::ReadOnlyProcess::read_bounded`];
//! - no remote call, no getter invocation, no debugger, no hook, no breakpoint,
//!   no write handle, no save access, no retry loop and no polling;
//! - an explicitly bounded page: a client may choose only `start` and `limit`,
//!   never the process, the module, the executable digest, the profile or the
//!   slot domain;
//! - field values are the raw bytes read at the declared offsets. `level_raw`
//!   and `plus_raw` are u16 reads until an upper-width proof exists, and all
//!   seven effect entries, including the `0xFFFF`/`0x0000` sentinels, are
//!   preserved rather than filtered.
//!
//! The chain stays a candidate hypothesis: the retained-image idiom is
//! verified, the exact caller edge was not traced, and no capacity, occupancy,
//! ownership, stable item identity or legality is claimed here.

use crate::error::RuntimeError;
use crate::platform::ReadOnlyProcess;
use crate::profile::ProfileSite;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// The one executable display version this surface pins.
pub const INVENTORY_GAME_VERSION: &str = "2.0.2.0";

/// The one executable build this surface pins, uppercase hex.
pub const INVENTORY_EXECUTABLE_SHA256: &str =
    "E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130";

/// Collector safety ceiling on the observed slot count.
///
/// This is a collector bound, not capacity and not an occupied-item count.
pub const SLOT_SAFETY_CEILING: u64 = 2500;

/// The getter's record stride (`imul rax, rax, 0xf0`).
pub const RECORD_SIZE: u64 = 0xF0;

/// The global slot the retained call sites load, as a module RVA.
pub const GLOBAL_SLOT_RVA: u64 = 0x4751530;

/// `container = deref(deref(global)) + 0x10`.
pub const CONTAINER_BIAS: u64 = 0x10;

/// The count displacement the standalone getter compares against.
pub const COUNT_DISPLACEMENT: u64 = 0x927C0;

/// Default page size.
pub const DEFAULT_LIMIT: u32 = 64;

/// Largest page a client may request.
pub const MAX_LIMIT: u32 = 64;

/// Largest `start` a client may name.
pub const MAX_START: u32 = (SLOT_SAFETY_CEILING - 1) as u32;

/// Declared record offsets. These are the offsets the bounded observation
/// decoded, not a proof of the game's own layout.
pub const ITEM_ID_OFFSET: usize = 0x00;
pub const QUANTITY_OFFSET: usize = 0x04;
pub const LEVEL_OFFSET: usize = 0x06;
pub const PLUS_OFFSET: usize = 0x0A;
pub const RARITY_OFFSET: usize = 0x30;
pub const EFFECT_TABLE_OFFSET: usize = 0x38;
pub const EFFECT_STRIDE: usize = 0x18;
pub const EFFECT_ENTRIES: usize = 7;

/// The keys a request object may carry, and nothing else.
const ALLOWED_PARAM_KEYS: [&str; 2] = ["start", "limit"];

/// The four retained-runtime sites verified before any chain read.
///
/// Names and byte strings are the ones the bounded observation verified in the
/// live image. The two global references are included so a shifted call site
/// cannot pass unnoticed.
pub fn inventory_sites() -> [ProfileSite; 4] {
    [
        site("global_ref_0x23E61F0", 0x23E61F0, "4c 8b 0d 39 b3 36 02"),
        site("global_ref_0x3EEDA3", 0x3EEDA3, "48 8b 0d 86 27 36 04"),
        site(
            "container_leaf_0x553038",
            0x553038,
            "4c 8b 01 49 83 c0 10 8b c2 49 3b 80 c0 27 09 00",
        ),
        site(
            "standalone_getter_0xF464C",
            0xF464C,
            "8b c2 48 3b 81 c0 27 09 00 73 0b 48 69 c0 f0 00 00 00",
        ),
    ]
}

/// The only two parameters a client may send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryRequest {
    pub start: u32,
    pub limit: u32,
}

impl InventoryRequest {
    /// The default page: first 64 slots.
    pub const DEFAULT_PAGE: Self = Self {
        start: 0,
        limit: DEFAULT_LIMIT,
    };

    /// Validate one explicit page.
    pub fn new(start: u32, limit: u32) -> Result<Self, RuntimeError> {
        if start > MAX_START {
            return Err(invalid(format!(
                "start must be an integer from 0 to {MAX_START}, not {start}"
            )));
        }
        if limit == 0 || limit > MAX_LIMIT {
            return Err(invalid(format!(
                "limit must be an integer from 1 to {MAX_LIMIT}, not {limit}"
            )));
        }
        Ok(Self { start, limit })
    }

    /// Parse the protected `params` object.
    ///
    /// An unknown key is refused, so a client cannot override the process, the
    /// executable digest, the profile or the slot domain through the request.
    pub fn from_json(params: &Value) -> Result<Self, RuntimeError> {
        let object = params
            .as_object()
            .ok_or_else(|| invalid("params must be an object"))?;
        for key in object.keys() {
            if !ALLOWED_PARAM_KEYS.contains(&key.as_str()) {
                return Err(invalid(format!(
                    "unsupported parameter {key}; only start and limit are accepted"
                )));
            }
        }
        let start = optional_u32(object.get("start"), "start", 0)?;
        let limit = optional_u32(object.get("limit"), "limit", DEFAULT_LIMIT)?;
        Self::new(start, limit)
    }

    fn end(&self) -> u32 {
        self.start.saturating_add(self.limit)
    }
}

/// One declared process identity for a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryProcess {
    pub pid: u32,
    pub creation_filetime: u64,
}

/// The read-only byte source a snapshot runs against.
///
/// The production implementation wraps a validated [`ReadOnlyProcess`]; the
/// fixture implementation answers from a declared region map, so the same
/// decode, paging and consistency code is exercised without a game.
pub trait InventoryMemory {
    fn module_base(&self) -> u64;
    fn process(&self) -> InventoryProcess;
    /// Re-read the creation FILETIME through the same handle the reads use.
    fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError>;
    fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError>;
}

/// The production memory source: one validated read-only process handle.
pub struct ProcessInventoryMemory<'a> {
    process: &'a ReadOnlyProcess,
}

impl<'a> ProcessInventoryMemory<'a> {
    pub fn new(process: &'a ReadOnlyProcess) -> Self {
        Self { process }
    }
}

impl InventoryMemory for ProcessInventoryMemory<'_> {
    fn module_base(&self) -> u64 {
        self.process.module_range().base
    }

    fn process(&self) -> InventoryProcess {
        let identity = self.process.identity();
        InventoryProcess {
            pid: identity.pid,
            creation_filetime: identity.creation_filetime,
        }
    }

    fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
        self.process.creation_filetime()
    }

    fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        self.process.read_bounded(address, size)
    }
}

/// A declared read-only region map, for fixtures and the runnable example.
///
/// A read that leaves the region that contains its address is refused exactly
/// as the production reader refuses it, so a fixture cannot demonstrate a read
/// the product could not perform.
pub struct FixtureMemory {
    module_base: u64,
    process: InventoryProcess,
    regions: Vec<(u64, Vec<u8>)>,
}

impl FixtureMemory {
    /// One fixture: `module_base`, `pid`, `creation_filetime`, `regions`.
    pub fn from_json(value: &Value) -> Result<Self, RuntimeError> {
        let module_base = fixture_u64(value.get("module_base"), "module_base")?;
        let pid = fixture_u64(value.get("pid"), "pid")?;
        let creation_filetime = fixture_u64(value.get("creation_filetime"), "creation_filetime")?;
        let pid = u32::try_from(pid).map_err(|_| invalid("pid does not fit a process id"))?;
        let raw_regions = value
            .get("regions")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("a fixture needs a regions array"))?;
        let mut regions = Vec::with_capacity(raw_regions.len());
        for region in raw_regions {
            let address = fixture_u64(region.get("address"), "region address")?;
            let bytes = region
                .get("bytes")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("a fixture region needs hex bytes"))?;
            regions.push((address, hex_bytes(bytes)?));
        }
        Ok(Self {
            module_base,
            process: InventoryProcess {
                pid,
                creation_filetime,
            },
            regions,
        })
    }
}

impl InventoryMemory for FixtureMemory {
    fn module_base(&self) -> u64 {
        self.module_base
    }

    fn process(&self) -> InventoryProcess {
        self.process
    }

    fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
        Ok(self.process.creation_filetime)
    }

    fn read(&self, address: u64, size: usize) -> Result<Vec<u8>, RuntimeError> {
        let end = address
            .checked_add(size as u64)
            .ok_or(RuntimeError::RegionNotReadable { address, size })?;
        for (base, bytes) in &self.regions {
            let limit = base.saturating_add(bytes.len() as u64);
            if address >= *base && end <= limit {
                let offset = (address - base) as usize;
                return Ok(bytes[offset..offset + size].to_vec());
            }
        }
        Err(RuntimeError::RegionNotReadable { address, size })
    }
}

/// What one read establishes about the candidate container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContainerContract {
    global_slot: u64,
    container: u64,
    count: u64,
}

/// One slot read twice: the bytes and their decoded fields.
struct PendingRow {
    area: u64,
    raw: Vec<u8>,
    decoded: Value,
}

/// Read one bounded page of raw inventory slots.
///
/// The order is part of the contract: verify the four sites, read the container
/// contract, read the requested page, read the contract again, refuse if any
/// root moved, re-read every page buffer and refuse if any of them is not
/// byte-equal, then re-check the process identity. Nothing is published unless
/// every step holds.
pub fn snapshot(
    memory: &dyn InventoryMemory,
    request: &InventoryRequest,
) -> Result<Value, RuntimeError> {
    let base = memory.module_base();
    verify_sites(memory, base)?;

    let before = read_container(memory, base)?;
    let count = u32::try_from(before.count).unwrap_or(u32::MAX);
    let (rows, next_start) = read_page(memory, &before, count, request)?;

    let after = read_container(memory, base)?;
    if after.global_slot != before.global_slot {
        return Err(chain("the inventory global slot changed between reads"));
    }
    if after.container != before.container {
        return Err(chain("the inventory container root changed between reads"));
    }
    if after.count != before.count {
        return Err(chain(format!(
            "the observed slot count changed between reads ({} -> {})",
            before.count, after.count
        )));
    }
    for row in &rows {
        let again = memory.read(row.area, RECORD_SIZE as usize)?;
        if again != row.raw {
            return Err(chain(format!(
                "a page buffer did not re-read byte-equal at {:#x}",
                row.area
            )));
        }
    }

    let declared = memory.process();
    if memory.recheck_creation_filetime()? != declared.creation_filetime {
        return Err(chain(
            "the process creation FILETIME changed during the read",
        ));
    }

    Ok(json!({
        "status": "observed",
        "game_version": INVENTORY_GAME_VERSION,
        "observed_at": utc_now_rfc3339(),
        "process": {
            "pid": declared.pid,
            "creation_filetime": declared.creation_filetime.to_string(),
        },
        "start": request.start,
        "limit": request.limit,
        "observed_slot_count": count,
        "next_start": next_start,
        "rows": rows.into_iter().map(|row| row.decoded).collect::<Vec<Value>>(),
        "consistency": "reread_equal",
        "read_only": true,
    }))
}

/// Verify the four declared sites against the live image.
fn verify_sites(memory: &dyn InventoryMemory, base: u64) -> Result<usize, RuntimeError> {
    let sites = inventory_sites();
    for site in &sites {
        let address = base
            .checked_add(site.rva)
            .ok_or_else(|| chain(format!("site {} does not fit the module", site.name)))?;
        let value = memory.read(address, site.signature.len())?;
        if value != site.signature {
            return Err(RuntimeError::SignatureMismatch {
                site: site.name.to_string(),
                rva: site.rva,
            });
        }
    }
    Ok(sites.len())
}

/// Walk the global slot to the container and read its count.
fn read_container(
    memory: &dyn InventoryMemory,
    base: u64,
) -> Result<ContainerContract, RuntimeError> {
    let slot_address = base
        .checked_add(GLOBAL_SLOT_RVA)
        .ok_or_else(|| chain("the global slot address overflowed"))?;
    let global_slot = u64_at(&memory.read(slot_address, 8)?);
    if global_slot == 0 {
        return Err(chain("the inventory global slot is null"));
    }
    let root = u64_at(&memory.read(global_slot, 8)?);
    if root == 0 {
        return Err(chain("the inventory container root is null"));
    }
    let container = root
        .checked_add(CONTAINER_BIAS)
        .ok_or_else(|| chain("the container address overflowed"))?;
    let count_address = container
        .checked_add(COUNT_DISPLACEMENT)
        .ok_or_else(|| chain("the count address overflowed"))?;
    let count = u64_at(&memory.read(count_address, 8)?);
    if count > SLOT_SAFETY_CEILING {
        return Err(chain(format!(
            "the observed slot count {count} exceeds the {SLOT_SAFETY_CEILING} safety ceiling, \
             which is a collector bound and not capacity or an occupied count"
        )));
    }
    Ok(ContainerContract {
        global_slot,
        container,
        count,
    })
}

/// Read the requested page. `start` at or past the observed count is an empty
/// page, not a failure.
fn read_page(
    memory: &dyn InventoryMemory,
    contract: &ContainerContract,
    count: u32,
    request: &InventoryRequest,
) -> Result<(Vec<PendingRow>, Value), RuntimeError> {
    if request.start >= count {
        return Ok((Vec::new(), Value::Null));
    }
    let end = request.end().min(count);
    let mut rows = Vec::with_capacity((end - request.start) as usize);
    for slot in request.start..end {
        let area = contract
            .container
            .checked_add(slot as u64 * RECORD_SIZE)
            .ok_or_else(|| chain("a record address overflowed"))?;
        let raw = memory.read(area, RECORD_SIZE as usize)?;
        let decoded = decode_record(slot, &raw)?;
        rows.push(PendingRow { area, raw, decoded });
    }
    let next_start = if end < count { json!(end) } else { Value::Null };
    Ok((rows, next_start))
}

/// Decode the declared fields of one raw record. Every value is raw.
fn decode_record(slot: u32, raw: &[u8]) -> Result<Value, RuntimeError> {
    let needed = EFFECT_TABLE_OFFSET + EFFECT_ENTRIES * EFFECT_STRIDE;
    if raw.len() < needed {
        return Err(chain(format!(
            "the record for slot {slot} is {} bytes, shorter than the declared layout needs ({needed})",
            raw.len()
        )));
    }
    let mut effects = Vec::with_capacity(EFFECT_ENTRIES);
    for index in 0..EFFECT_ENTRIES {
        let base = EFFECT_TABLE_OFFSET + index * EFFECT_STRIDE;
        effects.push(json!({
            "slot": index,
            "id": u16_at(raw, base),
            "raw_value": u32_at(raw, base + 4),
        }));
    }
    Ok(json!({
        "slot": slot,
        "item_id": u16_at(raw, ITEM_ID_OFFSET),
        "level_raw": u16_at(raw, LEVEL_OFFSET),
        "plus_raw": u16_at(raw, PLUS_OFFSET),
        "quantity_raw": u16_at(raw, QUANTITY_OFFSET),
        "rarity_raw": raw[RARITY_OFFSET],
        "record_sha256": format!("{:x}", Sha256::digest(raw)),
        "effects": effects,
    }))
}

/// UTC now as `2026-09-21T21:18:25Z`.
pub fn utc_now_rfc3339() -> String {
    utc_rfc3339(SystemTime::now())
}

fn utc_rfc3339(now: SystemTime) -> String {
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        (rest % 3_600) / 60,
        rest % 60
    )
}

/// Days since the Unix epoch to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as u32;
    let month = if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    } as u32;
    (year + i64::from(month <= 2), month, day)
}

fn site(name: &'static str, rva: u64, signature: &str) -> ProfileSite {
    let bytes = {
        let digits: Vec<u8> = signature
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        let mut bytes = Vec::with_capacity(digits.len() / 2);
        for pair in digits.chunks(2) {
            let high = (pair[0] as char).to_digit(16).unwrap_or(0);
            let low = (pair[1] as char).to_digit(16).unwrap_or(0);
            bytes.push((high * 16 + low) as u8);
        }
        bytes
    };
    ProfileSite {
        name,
        rva,
        signature: bytes,
    }
}

fn hex_bytes(text: &str) -> Result<Vec<u8>, RuntimeError> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if !digits.len().is_multiple_of(2) {
        return Err(invalid("a fixture region is not an even-length hex string"));
    }
    let mut bytes = Vec::with_capacity(digits.len() / 2);
    for pair in digits.chunks(2) {
        let high = (pair[0] as char)
            .to_digit(16)
            .ok_or_else(|| invalid("a fixture region is not hexadecimal"))?;
        let low = (pair[1] as char)
            .to_digit(16)
            .ok_or_else(|| invalid("a fixture region is not hexadecimal"))?;
        bytes.push((high * 16 + low) as u8);
    }
    Ok(bytes)
}

fn fixture_u64(value: Option<&Value>, name: &str) -> Result<u64, RuntimeError> {
    let value = value.ok_or_else(|| invalid(format!("a fixture needs {name}")))?;
    if let Some(number) = value.as_u64() {
        return Ok(number);
    }
    let text = value
        .as_str()
        .ok_or_else(|| invalid(format!("{name} must be an integer or a hex string")))?;
    let trimmed = text.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16)
            .map_err(|_| invalid(format!("{name} is not hexadecimal")));
    }
    trimmed
        .parse::<u64>()
        .or_else(|_| u64::from_str_radix(trimmed, 16))
        .map_err(|_| invalid(format!("{name} is not an integer")))
}

fn optional_u32(value: Option<&Value>, name: &str, fallback: u32) -> Result<u32, RuntimeError> {
    match value {
        None | Some(Value::Null) => Ok(fallback),
        Some(value) => {
            let number = value
                .as_u64()
                .ok_or_else(|| invalid(format!("{name} must be a non-negative integer")))?;
            u32::try_from(number).map_err(|_| invalid(format!("{name} must be an integer")))
        }
    }
}

fn u16_at(raw: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([raw[offset], raw[offset + 1]])
}

fn u32_at(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        raw[offset],
        raw[offset + 1],
        raw[offset + 2],
        raw[offset + 3],
    ])
}

fn u64_at(raw: &[u8]) -> u64 {
    u64::from_le_bytes([
        raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
    ])
}

fn chain(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InventoryChain {
        detail: detail.into(),
    }
}

fn invalid(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInventoryRequest {
        detail: detail.into(),
    }
}
