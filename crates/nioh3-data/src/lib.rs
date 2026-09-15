//! Bounded native-resource adapter for the offline enemy generation slice.
//!
//! The adapter reads only the current product resources below a supplied
//! `nioh3_scroll_editor/data` directory. Every consumed binary blob is verified
//! against its manifest size and SHA-256 before typed rows are exposed. The
//! enemy-state capture is JSON with no sidecar digest, so it is gated by its
//! capture identity hash plus the observed-lookup and schema checks instead.
//! Declared paths must stay inside the product data root, and no research,
//! audit, or generated copy is used as a fallback.

use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Component, Path, PathBuf},
};

use nioh3_domain::enemy::{
    ContextTables, Eligibility, EnemyStateTables, RosterTables, ENEMY_TEXT_SHA256,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

mod effect_resource;

pub use effect_resource::{
    load_effect_resource, GRACE_MAP_FORMAT, GRACE_MAP_GAME_VERSION, GRACE_MAP_PATHS,
    GRACE_MAP_RECORD_TYPE,
};

/// Resource directory, relative to the supplied `nioh3_scroll_editor/data` root.
pub const AUXILIARY_RESOURCE_DIR: &str = "auxiliary_generation/pc_v2_00_02/resource_v3";
/// Resource directory, relative to the supplied `nioh3_scroll_editor/data` root.
pub const R4_RESOURCE_DIR: &str = "r4_finalizer/pc_v2_00_02/resource_v1";
/// Enemy-state capture, relative to the supplied `nioh3_scroll_editor/data` root.
pub const ENEMY_STATE_TABLES_PATH: &str = "enemy_states/pc_v2_01/native_tables.json";

const AUXILIARY_SCHEMA: &str = "nioh3-auxiliary-generation-resource/v3";
const R4_SCHEMA: &str = "nioh3-r4-finalizer-resource/v1";

/// Native table binaries carry a four-byte tag and a little-endian row count.
const TABLE_HEADER_BYTES: usize = 8;
const TERRAIN_STRIDE: usize = 52;
const ENEMY_STRIDE: usize = 28;
const CONTEXT_STRIDE: usize = 48;
const OPTIONAL_MULTIPLIER_STRIDE: usize = 32;
const ENEMY_PARAMETER_GATE_ENTRY_BYTES: usize = 8;
const ENEMY_STATE_ROW_STRIDE: usize = 24;
const TERRAIN_BYTE_OFFSET: usize = 0x12;
const CONFIG_ROW_BYTES: usize = 0x20;

/// Typed inputs for offline enemy generation, all verified before use.
#[derive(Debug)]
pub struct EnemyResources {
    pub roster: RosterTables,
    pub context: ContextTables,
    pub states: EnemyStateTables,
}

/// Terrain-keyed native position rows, matching the domain record.
type PositionsByTerrain = BTreeMap<u8, Vec<[u8; ENEMY_STATE_ROW_STRIDE]>>;

/// Load every enemy-generation resource below `data_root`.
///
/// `data_root` is the product `nioh3_scroll_editor/data` directory. Missing,
/// undeclared, or digest-mismatched files are reported as errors instead of
/// being silently substituted.
pub fn load_enemy_resources(data_root: &Path) -> Result<EnemyResources, Box<dyn Error>> {
    let root = canonical_dir(data_root, "product data directory")?;
    let roster = load_roster_resource(&declared_resource_root(&root, AUXILIARY_RESOURCE_DIR)?)?;
    let context = load_context_resource(&declared_resource_root(&root, R4_RESOURCE_DIR)?)?;
    let states_path = declared_path(&root, ENEMY_STATE_TABLES_PATH, "enemy-state capture")?;
    let states = load_enemy_states_file(&states_path)?;
    Ok(EnemyResources {
        roster,
        context,
        states,
    })
}

/// Parse the native enemy-state capture with `possessed_generation.py` semantics.
///
/// Capture identity, the observed config lookup, complete terrain scans, row
/// keys, and config width are enforced. Eligibility entries whose native
/// semantics are only partially captured stay [`Eligibility::Unknown`], and
/// lookups that the capture does not list are never fabricated.
pub fn parse_enemy_state_tables(bytes: &[u8]) -> Result<EnemyStateTables, Box<dyn Error>> {
    let document: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("enemy-state capture is not valid JSON: {error}"))?;

    let text_sha256 =
        field_str(&document, "text_sha256", "enemy-state capture")?.to_ascii_lowercase();
    if !text_sha256.eq_ignore_ascii_case(ENEMY_TEXT_SHA256) {
        return Err("enemy-state capture: unsupported executable/table identity".into());
    }
    if document
        .get("config_4543_lookup_observed")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("enemy-state capture: config 4543 lookup not observed".into());
    }

    let positions = parse_positions_by_terrain(&document)?;
    let eligibility = parse_eligibility_by_lookup(&document)?;
    let config = parse_config_row(&document)?;

    Ok(EnemyStateTables {
        text_sha256,
        positions_by_terrain: positions,
        eligibility,
        enemy_index_complete: document
            .get("enemy_index_complete")
            .and_then(Value::as_bool)
            == Some(true),
        config_4543: config,
    })
}

fn parse_positions_by_terrain(document: &Value) -> Result<PositionsByTerrain, Box<dyn Error>> {
    const LABEL: &str = "enemy-state capture positions_by_terrain";
    let entries = field(document, "positions_by_terrain", LABEL)?
        .as_object()
        .ok_or_else(|| format!("{LABEL}: must be an object"))?;

    let mut positions = BTreeMap::new();
    for (key, entry) in entries {
        if entry.get("complete_terrain_scan").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let terrain = parse_index_key(key, LABEL)?;
        let terrain = u8::try_from(terrain)
            .map_err(|_| format!("{LABEL}: terrain key {key:?} is outside a single byte"))?;
        let rows_hex = entry
            .get("rows_hex")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{LABEL}: terrain {key:?} has no rows_hex array"))?;
        let mut rows = Vec::with_capacity(rows_hex.len());
        for (index, row_hex) in rows_hex.iter().enumerate() {
            let text = row_hex
                .as_str()
                .ok_or_else(|| format!("{LABEL}: terrain {key:?} row {index} is not a string"))?;
            let raw = decode_hex(text, &format!("{LABEL}: terrain {key:?} row {index}"))?;
            let row: [u8; ENEMY_STATE_ROW_STRIDE] = raw.try_into().map_err(|_| {
                format!(
                    "{LABEL}: terrain {key:?} row {index} must be {ENEMY_STATE_ROW_STRIDE:#x} bytes"
                )
            })?;
            if row[TERRAIN_BYTE_OFFSET] != terrain {
                return Err(format!(
                    "{LABEL}: terrain {key:?} row {index} carries terrain byte {:#04x}",
                    row[TERRAIN_BYTE_OFFSET]
                )
                .into());
            }
            rows.push(row);
        }
        if positions.insert(terrain, rows).is_some() {
            return Err(
                format!("{LABEL}: duplicate terrain key -> {terrain:#04x} (from {key:?})").into(),
            );
        }
    }
    Ok(positions)
}

fn parse_eligibility_by_lookup(
    document: &Value,
) -> Result<BTreeMap<u32, Eligibility>, Box<dyn Error>> {
    const LABEL: &str = "enemy-state capture eligibility_by_lookup";
    let entries = field(document, "eligibility_by_lookup", LABEL)?
        .as_object()
        .ok_or_else(|| format!("{LABEL}: must be an object"))?;

    let mut eligibility = BTreeMap::new();
    for (key, entry) in entries {
        let lookup = parse_index_key(key, LABEL)?;
        let lookup = u32::try_from(lookup)
            .map_err(|_| format!("{LABEL}: lookup key {key:?} is outside uint32"))?;
        let entry = entry
            .as_object()
            .ok_or_else(|| format!("{LABEL}: lookup 0x{lookup:X} must be an object"))?;
        if eligibility
            .insert(lookup, eligibility_state(entry))
            .is_some()
        {
            return Err(
                format!("{LABEL}: duplicate lookup key -> 0x{lookup:X} (from {key:?})").into(),
            );
        }
    }
    Ok(eligibility)
}

/// Map one captured lookup entry, keeping partially captured semantics unknown.
fn eligibility_state(entry: &Map<String, Value>) -> Eligibility {
    match entry.get("enemy_row_present").and_then(Value::as_bool) {
        Some(false) => Eligibility::EnemyAbsent,
        Some(true) => match entry.get("subtype_row_present").and_then(Value::as_bool) {
            Some(false) => Eligibility::SubtypeAbsent,
            Some(true) => match entry.get("flags14").and_then(Value::as_u64) {
                Some(flags) => match u32::try_from(flags) {
                    Ok(flags) => Eligibility::SubtypeFlags(flags),
                    Err(_) => Eligibility::Unknown,
                },
                None => Eligibility::Unknown,
            },
            None => Eligibility::Unknown,
        },
        None => Eligibility::Unknown,
    }
}

fn parse_config_row(document: &Value) -> Result<Option<[u8; CONFIG_ROW_BYTES]>, Box<dyn Error>> {
    const LABEL: &str = "enemy-state capture config_4543_hex";
    let value = field(document, "config_4543_hex", LABEL)?;
    match value {
        Value::Null => Ok(None),
        Value::String(text) => {
            let raw = decode_hex(text, LABEL)?;
            let row: [u8; CONFIG_ROW_BYTES] = raw.try_into().map_err(|_| {
                format!("{LABEL}: expected {CONFIG_ROW_BYTES:#x} bytes").to_string()
            })?;
            Ok(Some(row))
        }
        _ => Err(format!("{LABEL}: must be a hex string or null").into()),
    }
}

fn load_roster_resource(resource_root: &Path) -> Result<RosterTables, Box<dyn Error>> {
    let root = canonical_dir(resource_root, AUXILIARY_RESOURCE_DIR)?;
    let manifest = read_manifest(&root, AUXILIARY_SCHEMA)?;
    let tables = manifest
        .get("tables")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{AUXILIARY_SCHEMA}: manifest has no tables object"))?;

    let terrain_meta = auxiliary_table(tables, "auxiliary_terrain")?;
    let terrain = fixed_table(&root, terrain_meta, "auxiliary_terrain", TERRAIN_STRIDE)?;
    let keys_meta = field(terrain_meta, "keys_file", "auxiliary_terrain")?;
    let (_, keys_bytes) = read_declared_blob(&root, keys_meta, "auxiliary_terrain keys")?;
    let terrain_keys = parse_u16_keys("auxiliary_terrain", terrain.row_count, &keys_bytes)?;

    let enemies = fixed_table(
        &root,
        auxiliary_table(tables, "auxiliary_enemy_candidate")?,
        "auxiliary_enemy_candidate",
        ENEMY_STRIDE,
    )?;
    let contexts = fixed_table(
        &root,
        auxiliary_table(tables, "special_context")?,
        "special_context",
        CONTEXT_STRIDE,
    )?;

    let gate_meta = manifest
        .get("enemy_parameter_gate")
        .ok_or("enemy_parameter_gate: manifest has no enemy parameter gate")?;
    let entry_count = field_usize(gate_meta, "entry_count", "enemy_parameter_gate")?;
    let (_, gate_bytes) = read_declared_blob(
        &root,
        field(gate_meta, "file", "enemy_parameter_gate")?,
        "enemy_parameter_gate",
    )?;
    let parameter_types = parse_parameter_gate("enemy_parameter_gate", entry_count, &gate_bytes)?;

    Ok(RosterTables {
        enemies: enemies.fixed_rows::<ENEMY_STRIDE>(),
        contexts: contexts.fixed_rows::<CONTEXT_STRIDE>(),
        terrains: terrain.fixed_rows::<TERRAIN_STRIDE>(),
        terrain_keys,
        parameter_types,
    })
}

fn load_context_resource(resource_root: &Path) -> Result<ContextTables, Box<dyn Error>> {
    let root = canonical_dir(resource_root, R4_RESOURCE_DIR)?;
    let manifest = read_manifest(&root, R4_SCHEMA)?;

    let contexts = fixed_table(
        &root,
        r4_table(&manifest, "special_context")?,
        "special_context",
        CONTEXT_STRIDE,
    )?;
    let optional_multipliers = fixed_table(
        &root,
        r4_table(&manifest, "optional_multiplier")?,
        "optional_multiplier",
        OPTIONAL_MULTIPLIER_STRIDE,
    )?;

    Ok(ContextTables {
        contexts: contexts.fixed_rows::<CONTEXT_STRIDE>(),
        optional_multipliers: optional_multipliers.fixed_rows::<OPTIONAL_MULTIPLIER_STRIDE>(),
    })
}

fn load_enemy_states_file(path: &Path) -> Result<EnemyStateTables, Box<dyn Error>> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "enemy-state capture: cannot read {}: {error}",
            path.display()
        )
    })?;
    parse_enemy_state_tables(&bytes)
}

/// One native fixed-stride table with its embedded row-count header.
struct FixedStrideTable {
    name: String,
    row_size: usize,
    row_count: usize,
    store: Vec<u8>,
}

impl FixedStrideTable {
    fn load(
        name: &str,
        expected_stride: usize,
        row_size: usize,
        row_count: usize,
        store: Vec<u8>,
    ) -> Result<Self, Box<dyn Error>> {
        if row_size != expected_stride {
            return Err(format!(
                "{name}: declared stride {row_size:#x} does not match the expected \
                 {expected_stride:#x}"
            )
            .into());
        }
        if row_size == 0 {
            return Err(format!("{name}: declared stride must not be zero").into());
        }
        let expected = TABLE_HEADER_BYTES
            .checked_add(
                row_size
                    .checked_mul(row_count)
                    .ok_or_else(|| format!("{name}: declared row store size overflows"))?,
            )
            .ok_or_else(|| format!("{name}: declared row store size overflows"))?;
        if store.len() != expected {
            return Err(format!(
                "{name}: row-store size mismatch: expected {expected:#x}, got {:#x}",
                store.len()
            )
            .into());
        }
        let embedded = u32::from_le_bytes(store[4..TABLE_HEADER_BYTES].try_into().unwrap());
        if usize::try_from(embedded).ok() != Some(row_count) {
            return Err(
                format!("{name}: manifest count {row_count} != embedded count {embedded}").into(),
            );
        }
        Ok(Self {
            name: name.to_string(),
            row_size,
            row_count,
            store,
        })
    }

    fn row_bytes(&self, index: usize) -> &[u8] {
        debug_assert!(index < self.row_count);
        let start = TABLE_HEADER_BYTES + index * self.row_size;
        &self.store[start..start + self.row_size]
    }

    fn fixed_rows<const N: usize>(&self) -> Vec<[u8; N]> {
        debug_assert_eq!(self.row_size, N, "{}: row stride mismatch", self.name);
        (0..self.row_count)
            .map(|index| {
                let mut row = [0u8; N];
                row.copy_from_slice(self.row_bytes(index));
                row
            })
            .collect()
    }
}

fn fixed_table(
    root: &Path,
    meta: &Value,
    name: &str,
    expected_stride: usize,
) -> Result<FixedStrideTable, Box<dyn Error>> {
    let file = field(meta, "file", name)?;
    let (_, bytes) = read_declared_blob(root, file, name)?;
    FixedStrideTable::load(
        name,
        expected_stride,
        field_usize(meta, "row_size", name)?,
        field_usize(meta, "row_count", name)?,
        bytes,
    )
}

fn parse_u16_keys(name: &str, row_count: usize, data: &[u8]) -> Result<Vec<u16>, Box<dyn Error>> {
    let expected = row_count
        .checked_mul(2)
        .ok_or_else(|| format!("{name}: key resource size overflows"))?;
    if data.len() != expected {
        return Err(format!(
            "{name}: key resource size mismatch: expected {expected:#x}, got {:#x}",
            data.len()
        )
        .into());
    }
    Ok(data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect())
}

fn parse_parameter_gate(
    name: &str,
    entry_count: usize,
    data: &[u8],
) -> Result<BTreeMap<u32, u32>, Box<dyn Error>> {
    let expected = entry_count
        .checked_mul(ENEMY_PARAMETER_GATE_ENTRY_BYTES)
        .ok_or_else(|| format!("{name}: gate size overflows"))?;
    if data.len() != expected {
        return Err(format!(
            "{name}: size mismatch: expected {expected:#x}, got {:#x}",
            data.len()
        )
        .into());
    }
    let mut gate = BTreeMap::new();
    for chunk in data.chunks_exact(ENEMY_PARAMETER_GATE_ENTRY_BYTES) {
        let key = u32::from_le_bytes(chunk[0..4].try_into().unwrap());
        let parameter_type = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
        if gate.insert(key, parameter_type).is_some() {
            return Err(format!("{name}: duplicate enemy parameter gate key 0x{key:08X}").into());
        }
    }
    Ok(gate)
}

fn auxiliary_table<'a>(
    tables: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a Value, Box<dyn Error>> {
    tables
        .get(name)
        .ok_or_else(|| format!("{AUXILIARY_SCHEMA}: manifest has no table named {name:?}").into())
}

fn r4_table<'a>(manifest: &'a Value, name: &str) -> Result<&'a Value, Box<dyn Error>> {
    let mut matches = manifest
        .get("tables")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{R4_SCHEMA}: manifest has no tables array"))?
        .iter()
        .filter(|item| item.get("name").and_then(Value::as_str) == Some(name));
    let table = matches
        .next()
        .ok_or_else(|| format!("{R4_SCHEMA}: manifest has no table named {name:?}"))?;
    if matches.next().is_some() {
        return Err(format!("{R4_SCHEMA}: manifest declares table {name:?} more than once").into());
    }
    Ok(table)
}

fn canonical_dir(path: &Path, label: &str) -> Result<PathBuf, Box<dyn Error>> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("{label}: cannot resolve {}: {error}", path.display()))?;
    if !canonical.is_dir() {
        return Err(format!("{label}: {} is not a directory", canonical.display()).into());
    }
    Ok(canonical)
}

fn declared_resource_root(data_root: &Path, relative: &str) -> Result<PathBuf, Box<dyn Error>> {
    let root = canonical_dir(&data_root.join(relative), relative)?;
    if !root.starts_with(data_root) {
        return Err(format!(
            "{relative}: canonical resource path escapes the product data directory"
        )
        .into());
    }
    Ok(root)
}

fn read_manifest(resource_root: &Path, expected_schema: &str) -> Result<Value, Box<dyn Error>> {
    let path = declared_path(resource_root, "manifest.json", expected_schema)?;
    let bytes = fs::read(&path)
        .map_err(|error| format!("{expected_schema}: cannot read {}: {error}", path.display()))?;
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "{expected_schema}: invalid JSON manifest at {}: {error}",
            path.display()
        )
    })?;
    let schema = manifest
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{expected_schema}: manifest has no schema"))?;
    if schema != expected_schema {
        return Err(format!("{expected_schema}: unsupported resource schema {schema:?}").into());
    }
    Ok(manifest)
}

/// Read and verify one manifest-declared blob against its size and SHA-256.
fn read_declared_blob(
    resource_root: &Path,
    record: &Value,
    label: &str,
) -> Result<(PathBuf, Vec<u8>), Box<dyn Error>> {
    let filename = field_str(record, "filename", label)?;
    let declared_size = field_usize(record, "size", label)?;
    let declared_sha = field_str(record, "sha256", label)?.to_ascii_uppercase();
    let path = declared_path(resource_root, filename, label)?;
    let data = fs::read(&path)
        .map_err(|error| format!("{label}: cannot read {}: {error}", path.display()))?;
    if data.len() != declared_size {
        return Err(format!(
            "{label}: {} size mismatch: expected {declared_size:#x}, got {:#x}",
            path.display(),
            data.len()
        )
        .into());
    }
    let digest = sha256_hex_upper(&data);
    if digest != declared_sha {
        return Err(format!(
            "{label}: {} SHA-256 mismatch: {digest} != {declared_sha}",
            path.display()
        )
        .into());
    }
    Ok((path, data))
}

/// Resolve a declared relative path, rejecting traversal, absolute, and escaping
/// targets before the file is opened.
fn declared_path(
    resource_root: &Path,
    relative: &str,
    label: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let mut normal = PathBuf::new();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => normal.push(part),
            _ => return Err(format!("{label}: unsafe declared path {relative:?}").into()),
        }
    }
    if normal.as_os_str().is_empty() {
        return Err(format!("{label}: empty declared path").into());
    }
    let target = resource_root.join(&normal);
    let canonical = target.canonicalize().map_err(|error| {
        format!(
            "{label}: missing declared file {}: {error}",
            target.display()
        )
    })?;
    if !canonical.starts_with(resource_root) {
        return Err(
            format!("{label}: declared path escapes the resource root: {relative:?}").into(),
        );
    }
    if !canonical.is_file() {
        return Err(format!("{label}: declared path is not a file: {relative:?}").into());
    }
    Ok(canonical)
}

fn field<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a Value, Box<dyn Error>> {
    value
        .get(key)
        .ok_or_else(|| format!("{label}: missing field {key:?}").into())
}

fn field_str<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a str, Box<dyn Error>> {
    field(value, key, label)?
        .as_str()
        .ok_or_else(|| format!("{label}: field {key:?} must be a string").into())
}

fn field_usize(value: &Value, key: &str, label: &str) -> Result<usize, Box<dyn Error>> {
    let number = field(value, key, label)?
        .as_u64()
        .ok_or_else(|| format!("{label}: field {key:?} must be a non-negative integer"))?;
    usize::try_from(number).map_err(|_| format!("{label}: field {key:?} is too large").into())
}

/// Parse the decimal or `0x`-prefixed key spellings used by the captures.
fn parse_index_key(text: &str, label: &str) -> Result<u64, Box<dyn Error>> {
    let trimmed = text.trim();
    let (radix, digits) = match trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(rest) => (16, rest),
        None => (10, trimmed),
    };
    if digits.is_empty() {
        return Err(format!("{label}: invalid table key {text:?}").into());
    }
    u64::from_str_radix(digits, radix)
        .map_err(|_| format!("{label}: invalid table key {text:?}").into())
}

fn decode_hex(text: &str, label: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if !digits.len().is_multiple_of(2) {
        return Err(format!("{label}: hex payload has an odd number of digits").into());
    }
    let mut out = Vec::with_capacity(digits.len() / 2);
    for pair in digits.chunks_exact(2) {
        let high = match hex_digit(pair[0]) {
            Some(value) => value,
            None => return Err(format!("{label}: invalid hex payload").into()),
        };
        let low = match hex_digit(pair[1]) {
            Some(value) => value,
            None => return Err(format!("{label}: invalid hex payload").into()),
        };
        out.push((high << 4) | low);
    }
    Ok(out)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn sha256_hex_upper(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU32, Ordering},
    };

    /// Product data root of this checkout, derived from the crate location.
    fn product_data_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data")
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(tag: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "nioh3-data-{}-{tag}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn fixed_table_store(row_size: usize, row_count: usize) -> Vec<u8> {
        let mut store = vec![0u8; TABLE_HEADER_BYTES + row_size * row_count];
        store[0..4].copy_from_slice(&[0x00, 0x22, 0x04, 0x20]);
        store[4..TABLE_HEADER_BYTES].copy_from_slice(&(row_count as u32).to_le_bytes());
        store
    }

    fn write_blob(root: &Path, relative: &str, data: &[u8]) -> Value {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(&path, data).expect("write blob");
        serde_json::json!({
            "filename": relative,
            "size": data.len(),
            "sha256": sha256_hex_upper(data),
        })
    }

    /// Minimal synthetic auxiliary resource with one row per consumed table.
    fn write_auxiliary_resource(root: &Path) -> Value {
        let terrain_file = write_blob(
            root,
            "tables/auxiliary_terrain.bin",
            &fixed_table_store(TERRAIN_STRIDE, 1),
        );
        let keys_file = write_blob(root, "tables/auxiliary_terrain_keys.bin", &[0xD4, 0x00]);
        let enemy_file = write_blob(
            root,
            "tables/auxiliary_enemy_candidate.bin",
            &fixed_table_store(ENEMY_STRIDE, 1),
        );
        let context_file = write_blob(
            root,
            "tables/special_context.bin",
            &fixed_table_store(CONTEXT_STRIDE, 1),
        );
        let gate_file = write_blob(root, "tables/enemy_parameter_gate.bin", &[0u8; 8]);
        serde_json::json!({
            "schema": AUXILIARY_SCHEMA,
            "game_version": "PC v2.00.02",
            "tables": {
                "auxiliary_terrain": {
                    "row_size": TERRAIN_STRIDE,
                    "row_count": 1,
                    "file": terrain_file,
                    "keys_file": keys_file,
                },
                "auxiliary_enemy_candidate": {
                    "row_size": ENEMY_STRIDE,
                    "row_count": 1,
                    "file": enemy_file,
                },
                "special_context": {
                    "row_size": CONTEXT_STRIDE,
                    "row_count": 1,
                    "file": context_file,
                },
            },
            "enemy_parameter_gate": {"entry_count": 1, "file": gate_file},
        })
    }

    fn write_manifest(root: &Path, manifest: &Value) {
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec_pretty(manifest).expect("encode manifest"),
        )
        .expect("write manifest");
    }

    /// Minimal synthetic R4 finalizer resource with one row per consumed table.
    fn write_r4_resource(root: &Path) -> Value {
        let context_file = write_blob(
            root,
            "tables/special_context.bin",
            &fixed_table_store(CONTEXT_STRIDE, 1),
        );
        let multiplier_file = write_blob(
            root,
            "tables/optional_multiplier.bin",
            &fixed_table_store(OPTIONAL_MULTIPLIER_STRIDE, 1),
        );
        serde_json::json!({
            "schema": R4_SCHEMA,
            "game_version": "PC v2.00.02",
            "tables": [
                {
                    "name": "special_context",
                    "row_size": CONTEXT_STRIDE,
                    "row_count": 1,
                    "file": context_file,
                },
                {
                    "name": "optional_multiplier",
                    "row_size": OPTIONAL_MULTIPLIER_STRIDE,
                    "row_count": 1,
                    "file": multiplier_file,
                },
            ],
        })
    }

    fn native_state_document() -> Value {
        serde_json::json!({
            "schema_version": 1,
            "text_sha256": ENEMY_TEXT_SHA256.to_ascii_uppercase(),
            "enemy_index_complete": true,
            "config_4543_lookup_observed": true,
            "config_4543_hex": Value::Null,
            "positions_by_terrain": {
                "0x8": {
                    "complete_terrain_scan": true,
                    "rows_hex": ["2aabf547f43fb946d0f25048c66ed6423f00080100000000"],
                },
                "0x9": {"complete_terrain_scan": false, "rows_hex": []},
            },
            "eligibility_by_lookup": {
                "0x1": {"enemy_row_present": false},
                "0x2": {"enemy_row_present": true, "subtype_row_present": false},
                "0x3": {"enemy_row_present": true, "subtype_row_present": true, "flags14": 0},
                "0x6": {"enemy_row_present": true, "subtype_row_present": true, "flags14": 3},
                "0x4": {"enemy_row_present": true, "subtype_row_present": true},
                "0x5": {"enemy_row_present": true},
            },
        })
    }

    #[test]
    fn current_product_resources_load() {
        let resources = load_enemy_resources(&product_data_root()).expect("load product data");

        assert_eq!(resources.roster.terrains.len(), 20);
        assert_eq!(resources.roster.terrain_keys.len(), 20);
        assert_eq!(resources.roster.enemies.len(), 487);
        assert_eq!(resources.roster.contexts.len(), 7);
        assert_eq!(resources.roster.parameter_types.len(), 1022);
        assert_eq!(resources.context.contexts.len(), 7);
        assert_eq!(resources.context.optional_multipliers.len(), 2951);

        let states = &resources.states;
        assert_eq!(states.text_sha256, ENEMY_TEXT_SHA256);
        assert!(states.enemy_index_complete);
        assert_eq!(
            states.config_4543.expect("captured config row").len(),
            CONFIG_ROW_BYTES
        );
        assert_eq!(states.positions_by_terrain.len(), 20);
        for (terrain, rows) in &states.positions_by_terrain {
            assert_eq!(rows.len(), 8);
            assert!(rows.iter().all(|row| row[TERRAIN_BYTE_OFFSET] == *terrain));
        }
        assert_eq!(states.eligibility.len(), 1022);
        assert!(states
            .eligibility
            .values()
            .any(|entry| *entry == Eligibility::SubtypeAbsent));
        assert!(states
            .eligibility
            .values()
            .any(|entry| matches!(entry, Eligibility::SubtypeFlags(_))));
    }

    #[test]
    fn native_state_eligibility_maps_without_fabricating_entries() {
        let bytes = serde_json::to_vec(&native_state_document()).expect("encode capture");
        let states = parse_enemy_state_tables(&bytes).expect("parse capture");

        assert_eq!(states.text_sha256, ENEMY_TEXT_SHA256);
        assert!(states.enemy_index_complete);
        assert!(states.config_4543.is_none());
        assert_eq!(states.positions_by_terrain.len(), 1);
        assert!(states.positions_by_terrain.contains_key(&0x8));
        assert!(!states.positions_by_terrain.contains_key(&0x9));

        assert_eq!(
            states.eligibility.get(&0x1),
            Some(&Eligibility::EnemyAbsent)
        );
        assert_eq!(
            states.eligibility.get(&0x2),
            Some(&Eligibility::SubtypeAbsent)
        );
        assert_eq!(
            states.eligibility.get(&0x3),
            Some(&Eligibility::SubtypeFlags(0))
        );
        assert_eq!(
            states.eligibility.get(&0x6),
            Some(&Eligibility::SubtypeFlags(3))
        );
        assert_eq!(states.eligibility.get(&0x4), Some(&Eligibility::Unknown));
        assert_eq!(states.eligibility.get(&0x5), Some(&Eligibility::Unknown));
        assert_eq!(states.eligibility.get(&0xDEAD), None);
    }

    #[test]
    fn native_state_malformed_captures_are_rejected() {
        type Mutation = fn(&mut Value);
        let cases: Vec<(&str, Mutation)> = vec![
            ("identity", |document: &mut Value| {
                document["text_sha256"] = serde_json::json!("00");
            }),
            ("config lookup", |document: &mut Value| {
                document["config_4543_lookup_observed"] = serde_json::json!(false);
            }),
            ("config width", |document: &mut Value| {
                document["config_4543_hex"] = serde_json::json!("0011");
            }),
            ("row length", |document: &mut Value| {
                document["positions_by_terrain"]["0x8"]["rows_hex"] = serde_json::json!(["00"]);
            }),
            ("row terrain byte", |document: &mut Value| {
                document["positions_by_terrain"]["0x8"]["rows_hex"] =
                    serde_json::json!["2aabf547f43fb946d0f25048c66ed6423f00090100000000"];
            }),
            ("row hex", |document: &mut Value| {
                document["positions_by_terrain"]["0x8"]["rows_hex"] =
                    serde_json::json!["zzabf547f43fb946d0f25048c66ed6423f00080100000000"];
            }),
            ("lookup key", |document: &mut Value| {
                document["eligibility_by_lookup"]["nope"] =
                    serde_json::json!({"enemy_row_present": true});
            }),
        ];
        for (name, mutate) in cases {
            let mut document = native_state_document();
            mutate(&mut document);
            let bytes = serde_json::to_vec(&document).expect("encode capture");
            assert!(
                parse_enemy_state_tables(&bytes).is_err(),
                "expected {name} to be rejected"
            );
        }
    }

    #[test]
    fn altered_table_digest_is_rejected() {
        let temp = TempDir::new("digest");
        let mut manifest = write_auxiliary_resource(&temp.path);
        manifest["tables"]["auxiliary_terrain"]["file"]["sha256"] =
            serde_json::json!("0".repeat(64));
        write_manifest(&temp.path, &manifest);

        let error = load_roster_resource(&temp.path).expect_err("digest mismatch must fail");
        assert!(
            error.to_string().contains("SHA-256"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn undeclared_and_escaping_paths_are_rejected() {
        for relative in ["../escape.bin", "/escape.bin", "C:/escape.bin"] {
            let temp = TempDir::new("path");
            let mut manifest = write_auxiliary_resource(&temp.path);
            manifest["tables"]["auxiliary_terrain"]["file"]["filename"] =
                serde_json::json!(relative);
            write_manifest(&temp.path, &manifest);

            let result = load_roster_resource(&temp.path);
            assert!(
                result.is_err(),
                "expected declared path {relative:?} to be rejected"
            );
        }
    }

    #[test]
    fn duplicate_declarations_and_normalized_keys_are_rejected() {
        // "8" and "0x8" are the same native terrain key.
        let mut document = native_state_document();
        document["positions_by_terrain"]["8"] = serde_json::json!({
            "complete_terrain_scan": true,
            "rows_hex": ["2aabf547f43fb946d0f25048c66ed6423f00080100000000"],
        });
        let bytes = serde_json::to_vec(&document).expect("encode capture");
        assert!(
            parse_enemy_state_tables(&bytes).is_err(),
            "duplicate terrain keys '8'/'0x8' must not overwrite"
        );

        // "1" and "0x1" are the same native lookup key.
        let mut document = native_state_document();
        document["eligibility_by_lookup"]["1"] = serde_json::json!({"enemy_row_present": false});
        let bytes = serde_json::to_vec(&document).expect("encode capture");
        assert!(
            parse_enemy_state_tables(&bytes).is_err(),
            "duplicate lookup keys '1'/'0x1' must not overwrite"
        );

        // A consumed named table declared twice is ambiguous.
        let temp = TempDir::new("duplicate-table");
        let mut manifest = write_r4_resource(&temp.path);
        let duplicate = manifest["tables"][0].clone();
        manifest["tables"]
            .as_array_mut()
            .expect("tables array")
            .push(duplicate);
        write_manifest(&temp.path, &manifest);
        let error = load_context_resource(&temp.path).expect_err("duplicate table entry must fail");
        assert!(
            error.to_string().contains("more than once"),
            "unexpected error: {error}"
        );

        // Control: the same resource without the duplicate is accepted.
        let temp = TempDir::new("single-table");
        let manifest = write_r4_resource(&temp.path);
        write_manifest(&temp.path, &manifest);
        let context = load_context_resource(&temp.path).expect("single table resource loads");
        assert_eq!(context.contexts.len(), 1);
        assert_eq!(context.optional_multipliers.len(), 1);
    }

    #[test]
    fn resources_outside_the_product_root_are_rejected() {
        let temp = TempDir::new("empty");
        assert!(load_enemy_resources(&temp.path).is_err());
    }
}
