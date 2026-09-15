//! Bounded read-only adapter for the shipped effect/record tables used by the
//! M2.1 effect-sequence slice.
//!
//! File IO, manifest integrity, path containment and the Grace-map context gate
//! live here. Field decoding, index construction and numerical semantics stay in
//! `nioh3-domain`; this module only hands over header-stripped rows plus the
//! metadata the domain needs.
//!
//! Mirrors the Python reference contracts: `r4_finalizer_resource.py` (table
//! manifest and row strides) and `grace_map.py` (`_EXPECTED_CONTEXTS`,
//! `_EXPECTED_VERSION`, dense `0..=0xFFFF` inclusive range partition).

use std::{error::Error, fs, path::Path};

use nioh3_domain::effect::{
    EffectResourceBytes, EffectTableBytes, GraceMap, GraceRange, GRACE_MAP_CAPTURE_STATE,
};
use serde_json::Value;

use super::{
    canonical_dir, declared_path, declared_resource_root, field, field_str, field_usize,
    fixed_table, read_declared_blob, R4_RESOURCE_DIR, R4_SCHEMA, TABLE_HEADER_BYTES,
};

/// Shipped Grace maps, relative to the product `nioh3_scroll_editor/data` root.
pub const GRACE_MAP_PATHS: [(u8, &str); 2] = [
    (4, "grace_output_map_e604_r4_current.json"),
    (5, "grace_output_map_e604_r5_current.json"),
];

/// Verified Grace-map format, game version, record type and progression label
/// (`nioh3_scroll_editor/grace_map.py`, `_EXPECTED_CONTEXTS`).
pub const GRACE_MAP_FORMAT: &str = "nioh3-grace-first-u16-map-v2";
pub const GRACE_MAP_GAME_VERSION: &str = "2.00.02";
pub const GRACE_MAP_RECORD_TYPE: u32 = 0xE604;

/// Expected manifest row strides for the nine shipped tables.
const EXPECTED_TABLE_STRIDES: [(&str, usize); 9] = [
    ("item", 0x1A0),
    ("effect_group", 0x70),
    ("category", 0x6C),
    ("category_count_multiplier", 0x20),
    ("level_curve", 10),
    ("effect", 0xD8),
    ("optional_multiplier", 0x20),
    ("rarity_roll", 248),
    ("special_context", 48),
];

/// Bonus-curve row size declared by the shipped manifest (`bonus_curve.row_size`).
const BONUS_CURVE_ROW_BYTES: usize = 0x58;

/// Load every M2.1 effect resource below `data_root`.
///
/// `data_root` is the product `nioh3_scroll_editor/data` directory. Missing,
/// undeclared, duplicated, escaping, schema-mismatched, or digest-mismatched
/// resources are reported as errors instead of being silently substituted.
pub fn load_effect_resource(data_root: &Path) -> Result<EffectResourceBytes, Box<dyn Error>> {
    let root = canonical_dir(data_root, "product data directory")?;
    let resource_root = declared_resource_root(&root, R4_RESOURCE_DIR)?;
    let manifest = super::read_manifest(&resource_root, R4_SCHEMA)?;

    let mut tables = Vec::with_capacity(EXPECTED_TABLE_STRIDES.len());
    for (name, stride) in EXPECTED_TABLE_STRIDES {
        let table = fixed_table(
            &resource_root,
            super::r4_table(&manifest, name)?,
            name,
            stride,
        )?;
        tables.push(
            EffectTableBytes::new(
                name,
                table.row_size,
                table.store[TABLE_HEADER_BYTES..].to_vec(),
            )
            .map_err(|error| -> Box<dyn Error> { format!("{name}: {error:?}").into() })?,
        );
    }
    let mut tables = tables.into_iter();
    let mut next = || -> Result<EffectTableBytes, Box<dyn Error>> {
        tables
            .next()
            .ok_or_else(|| "table set is incomplete".into())
    };

    let (bonus_curve_rows, bonus_curve_index, playthrough_progress) =
        load_shipped_blobs(&resource_root, &manifest)?;
    let grace_maps = vec![
        load_grace_map(&root, GRACE_MAP_PATHS[0].0, GRACE_MAP_PATHS[0].1)?,
        load_grace_map(&root, GRACE_MAP_PATHS[1].0, GRACE_MAP_PATHS[1].1)?,
    ];

    Ok(EffectResourceBytes {
        schema: R4_SCHEMA.to_string(),
        // Field order mirrors EXPECTED_TABLE_STRIDES; keep the two in step.
        item: next()?,
        effect_group: next()?,
        category: next()?,
        category_count_multiplier: next()?,
        level_curve: next()?,
        effect: next()?,
        optional_multiplier: next()?,
        rarity_roll: next()?,
        special_context: next()?,
        bonus_curve_rows,
        bonus_curve_index,
        playthrough_progress,
        grace_maps,
    })
}

/// Verified bonus-curve rows/index and the playthrough progress blob.
type ShippedBlobs = (Vec<u8>, Vec<u8>, Vec<u8>);

fn load_shipped_blobs(
    resource_root: &Path,
    manifest: &Value,
) -> Result<ShippedBlobs, Box<dyn Error>> {
    const LABEL: &str = "bonus_curve";
    let meta = field(manifest, "bonus_curve", LABEL)?;
    let row_size = field_usize(meta, "row_size", LABEL)?;
    if row_size != BONUS_CURVE_ROW_BYTES {
        return Err(format!(
            "{LABEL}: declared row size {row_size:#x} does not match the expected \
             {BONUS_CURVE_ROW_BYTES:#x}"
        )
        .into());
    }
    let entry_count = field_usize(meta, "entry_count", LABEL)?;
    let unique_row_count = field_usize(meta, "unique_row_count", LABEL)?;
    if unique_row_count > entry_count {
        return Err(format!("{LABEL}: unique rows exceed the entry count").into());
    }
    let (_, rows) = read_declared_blob(resource_root, field(meta, "rows_file", LABEL)?, LABEL)?;
    let expected_rows = row_size
        .checked_mul(entry_count)
        .ok_or_else(|| format!("{LABEL}: row store size overflows"))?;
    if rows.len() != expected_rows {
        return Err(format!(
            "{LABEL}: row store is {} bytes, expected {expected_rows}",
            rows.len()
        )
        .into());
    }
    let (_, index) = read_declared_blob(resource_root, field(meta, "index_file", LABEL)?, LABEL)?;
    let expected_index = entry_count
        .checked_mul(4)
        .ok_or_else(|| format!("{LABEL}: index size overflows"))?;
    if index.len() != expected_index {
        return Err(format!(
            "{LABEL}: index is {} bytes, expected {expected_index}",
            index.len()
        )
        .into());
    }

    const PLAYTHROUGH: &str = "playthrough";
    let playthrough = field(manifest, PLAYTHROUGH, PLAYTHROUGH)?;
    let selector_min = field_usize(playthrough, "selector_min", PLAYTHROUGH)?;
    let selector_max = field_usize(playthrough, "selector_max", PLAYTHROUGH)?;
    let values_per_selector = field_usize(playthrough, "values_per_selector", PLAYTHROUGH)?;
    if selector_min == 0 || selector_max < selector_min || values_per_selector == 0 {
        return Err(format!("{PLAYTHROUGH}: invalid selector geometry").into());
    }
    let expected = (selector_max - selector_min + 1)
        .checked_mul(values_per_selector)
        .and_then(|entries| entries.checked_mul(4))
        .ok_or_else(|| format!("{PLAYTHROUGH}: selector store size overflows"))?;
    let (_, progress) = read_declared_blob(
        resource_root,
        field(playthrough, "file", PLAYTHROUGH)?,
        PLAYTHROUGH,
    )?;
    if progress.len() != expected {
        return Err(format!(
            "{PLAYTHROUGH}: selector store is {} bytes, expected {expected}",
            progress.len()
        )
        .into());
    }
    Ok((rows, index, progress))
}

/// Read one measured Grace map and gate it on its shipped context.
fn load_grace_map(
    data_root: &Path,
    rarity: u8,
    relative: &str,
) -> Result<GraceMap, Box<dyn Error>> {
    let label = format!("grace map rarity {rarity}");
    let path = declared_path(data_root, relative, &label)?;
    let bytes = fs::read(&path)
        .map_err(|error| format!("{label}: cannot read {}: {error}", path.display()))?;
    let document: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{label}: invalid JSON at {}: {error}", path.display()))?;

    let format = field_str(&document, "format", &label)?;
    if format != GRACE_MAP_FORMAT {
        return Err(format!("{label}: unsupported map format {format:?}").into());
    }
    let game_version = field_str(&document, "game_version", &label)?;
    if game_version != GRACE_MAP_GAME_VERSION {
        return Err(format!("{label}: unverified game version {game_version:?}").into());
    }
    let context = field(&document, "context", &label)?;
    // The shipped maps write `record_type` as a "0xE604" string, which the
    // Python `_integer` helper accepts alongside plain JSON numbers.
    let record_type = field_integer(context, "record_type", &label)?;
    let context_rarity = field_integer(context, "rarity", &label)?;
    let effect_slot = field_integer(context, "effect_slot", &label)?;
    let capture_state = field_str(context, "playthrough", &label)?;
    let expected_slot = if rarity == 4 { 5 } else { 6 };
    if record_type != u64::from(GRACE_MAP_RECORD_TYPE)
        || context_rarity != u64::from(rarity)
        || effect_slot != expected_slot as u64
        || capture_state != GRACE_MAP_CAPTURE_STATE
    {
        return Err(format!(
            "{label}: context {record_type:#x}/{context_rarity}/{capture_state}/{effect_slot} is \
             not the verified {:#x}/{rarity}/{GRACE_MAP_CAPTURE_STATE}/{expected_slot}",
            GRACE_MAP_RECORD_TYPE
        )
        .into());
    }

    let raw_ranges = field(&document, "ranges", &label)?
        .as_array()
        .ok_or_else(|| format!("{label}: ranges must be an array"))?;
    if raw_ranges.is_empty() {
        return Err(format!("{label}: ranges must not be empty").into());
    }
    let mut ranges = Vec::with_capacity(raw_ranges.len());
    for entry in raw_ranges {
        let start = field_usize(entry, "start", &label)?;
        let end = field_usize(entry, "end", &label)?;
        let effect_id = field_str(entry, "grace_id", &label)?;
        let effect_id = u32::from_str_radix(effect_id.trim_start_matches("0x"), 16)
            .map_err(|_| format!("{label}: invalid grace id {effect_id:?}"))?;
        ranges.push(GraceRange {
            start: u16::try_from(start)
                .map_err(|_| format!("{label}: range start {start} exceeds u16"))?,
            end: u16::try_from(end).map_err(|_| format!("{label}: range end {end} exceeds u16"))?,
            effect_id,
        });
    }

    let map = GraceMap {
        format: format.to_string(),
        game_version: game_version.to_string(),
        record_type: u32::try_from(record_type)
            .map_err(|_| format!("{label}: record type does not fit in u32"))?,
        rarity: u8::try_from(context_rarity)
            .map_err(|_| format!("{label}: rarity does not fit in u8"))?,
        // The shipped label is provenance, not a progression number: pass it
        // through verbatim so no capture playthrough is asserted.
        capture_state: capture_state.to_string(),
        effect_slot: u8::try_from(effect_slot)
            .map_err(|_| format!("{label}: effect slot does not fit in u8"))?,
        ranges,
    };
    // The domain owns the partition rule; fail closed on the same condition.
    map.validate()
        .map_err(|error| -> Box<dyn Error> { format!("{label}: {error:?}").into() })?;
    Ok(map)
}

/// Accept a JSON number or the `"0x..."`/decimal string form the shipped maps use
/// (mirrors the Python `_integer` helper).
fn field_integer(value: &Value, key: &str, label: &str) -> Result<u64, Box<dyn Error>> {
    let raw = field(value, key, label)?;
    if let Some(number) = raw.as_u64() {
        return Ok(number);
    }
    if let Some(text) = raw.as_str() {
        let text = text.trim();
        let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            Some(hex) => u64::from_str_radix(hex, 16),
            None => text.parse::<u64>(),
        };
        if let Ok(number) = parsed {
            return Ok(number);
        }
        return Err(format!("{label}: field {key:?} has an unparsable value {text:?}").into());
    }
    Err(format!("{label}: field {key:?} must be an integer or integer string").into())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, path::PathBuf};

    use super::*;
    use crate::sha256_hex_upper;

    /// The shipped product data root inside this checkout.
    fn shipped_data_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data")
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nioh3-data-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// Minimal valid resource tree: nine tables, bonus curve, playthrough and
    /// both Grace maps, all with correct digests.
    fn write_synthetic_root(root: &Path, mutate: impl FnOnce(&mut Value, &Path)) -> PathBuf {
        let resource = root.join(R4_RESOURCE_DIR);
        fs::create_dir_all(resource.join("tables")).unwrap();
        fs::create_dir_all(resource.join("bonus_curve")).unwrap();
        fs::create_dir_all(resource.join("globals")).unwrap();

        let mut records = Vec::new();
        let mut tables = Vec::new();
        for (name, stride) in EXPECTED_TABLE_STRIDES {
            let row_count = 2usize;
            let mut bytes = vec![0u8; TABLE_HEADER_BYTES + stride * row_count];
            bytes[..4].copy_from_slice(&[0x00, 0x22, 0x04, 0x20]);
            bytes[4..8].copy_from_slice(&(row_count as u32).to_le_bytes());
            let relative = format!("tables/{name}.bin");
            fs::write(resource.join(&relative), &bytes).unwrap();
            records.push(serde_json::json!({
                "filename": relative,
                "size": bytes.len(),
                "sha256": sha256_hex_upper(&bytes),
            }));
            tables.push(serde_json::json!({
                "name": name,
                "purpose": "synthetic",
                "row_size": stride,
                "row_count": row_count,
                "file": records.last().unwrap(),
            }));
        }

        let bonus_rows = vec![0u8; BONUS_CURVE_ROW_BYTES * 2];
        let bonus_index = vec![0u8; 2 * 4];
        let progress = vec![0u8; 5 * 4 * 4];
        fs::write(resource.join("bonus_curve/rows.bin"), &bonus_rows).unwrap();
        fs::write(resource.join("bonus_curve/index.bin"), &bonus_index).unwrap();
        fs::write(resource.join("globals/playthrough_progress.bin"), &progress).unwrap();

        let mut manifest = serde_json::json!({
            "schema": R4_SCHEMA,
            "tables": tables,
            "bonus_curve": {
                "row_size": BONUS_CURVE_ROW_BYTES,
                "entry_count": 2,
                "unique_row_count": 2,
                "invalid_row_index": u32::MAX,
                "rows_file": {"filename": "bonus_curve/rows.bin", "size": bonus_rows.len(),
                              "sha256": sha256_hex_upper(&bonus_rows)},
                "index_file": {"filename": "bonus_curve/index.bin", "size": bonus_index.len(),
                               "sha256": sha256_hex_upper(&bonus_index)},
            },
            "playthrough": {
                "selector_min": 1,
                "selector_max": 5,
                "values_per_selector": 4,
                "file": {"filename": "globals/playthrough_progress.bin", "size": progress.len(),
                         "sha256": sha256_hex_upper(&progress)},
            },
            "files": [],
        });

        for (rarity, relative) in GRACE_MAP_PATHS {
            let map = serde_json::json!({
                "format": GRACE_MAP_FORMAT,
                "game_version": GRACE_MAP_GAME_VERSION,
                "context": {
                    "record_type": GRACE_MAP_RECORD_TYPE,
                    "rarity": rarity,
                    "playthrough": GRACE_MAP_CAPTURE_STATE,
                    "effect_slot": if rarity == 4 { 5 } else { 6 },
                },
                "ranges": [
                    {"start": 0, "end": 0x7FFF, "grace_id": "0x00006553"},
                    {"start": 0x8000, "end": 0xFFFF, "grace_id": "0x0000CE68"},
                ],
            });
            fs::write(root.join(relative), map.to_string()).unwrap();
        }

        mutate(&mut manifest, root);
        fs::write(resource.join("manifest.json"), manifest.to_string()).unwrap();
        root.to_path_buf()
    }

    #[test]
    fn loads_shipped_effect_resource() {
        let resource = load_effect_resource(&shipped_data_root()).expect("shipped resource loads");
        assert_eq!(resource.schema, R4_SCHEMA);

        let expected: BTreeMap<&str, (usize, usize)> = BTreeMap::from([
            ("item", (0x1A0, 3362)),
            ("effect_group", (0x70, 1152)),
            ("category", (0x6C, 17)),
            ("category_count_multiplier", (0x20, 6)),
            ("level_curve", (10, 501)),
            ("effect", (0xD8, 3609)),
            ("optional_multiplier", (0x20, 2951)),
            ("rarity_roll", (248, 6)),
            ("special_context", (48, 7)),
        ]);
        for (name, (row_size, row_count)) in &expected {
            let table = match *name {
                "item" => &resource.item,
                "effect_group" => &resource.effect_group,
                "category" => &resource.category,
                "category_count_multiplier" => &resource.category_count_multiplier,
                "level_curve" => &resource.level_curve,
                "effect" => &resource.effect,
                "optional_multiplier" => &resource.optional_multiplier,
                "rarity_roll" => &resource.rarity_roll,
                _ => &resource.special_context,
            };
            assert_eq!(table.row_size, *row_size, "{name}: row size");
            assert_eq!(table.row_count(), *row_count, "{name}: row count");
            assert_eq!(
                table.rows.len(),
                row_size * row_count,
                "{name}: rows length"
            );
        }

        assert_eq!(resource.bonus_curve_rows.len(), 0x58 * 662);
        assert_eq!(resource.bonus_curve_index.len(), 662 * 4);
        assert_eq!(resource.playthrough_progress.len(), 5 * 4 * 4);

        assert_eq!(resource.grace_maps.len(), 2);
        assert_eq!(resource.grace_maps[0].rarity, 4);
        assert_eq!(resource.grace_maps[0].effect_slot, 5);
        assert_eq!(resource.grace_maps[0].ranges.len(), 21);
        assert_eq!(resource.grace_maps[1].rarity, 5);
        assert_eq!(resource.grace_maps[1].effect_slot, 6);
        assert_eq!(resource.grace_maps[1].ranges.len(), 11);
        for map in &resource.grace_maps {
            map.validate().expect("shipped map is a dense partition");
            // Provenance label round-trips verbatim; it is never a progression number.
            assert_eq!(map.capture_state, GRACE_MAP_CAPTURE_STATE);
            // Boundary lookups: first byte, both sides of the first break, last byte.
            let first = map.ranges[0];
            let second = map.ranges[1];
            assert_eq!(map.grace_id_for_first_u16(0), Some(first.effect_id));
            assert_eq!(map.grace_id_for_first_u16(first.end), Some(first.effect_id));
            assert_eq!(
                map.grace_id_for_first_u16(second.start),
                Some(second.effect_id)
            );
            assert_eq!(
                map.grace_id_for_first_u16(0xFFFF),
                Some(map.ranges.last().unwrap().effect_id)
            );
        }
    }

    #[test]
    fn loads_shipped_finalizer_inputs_for_the_completion_path() {
        let resource = load_effect_resource(&shipped_data_root()).expect("shipped resource loads");
        let u32_at = |row: &[u8], offset: usize| {
            u32::from_le_bytes(row[offset..offset + 4].try_into().unwrap())
        };
        let i32_at = |row: &[u8], offset: usize| {
            i32::from_le_bytes(row[offset..offset + 4].try_into().unwrap())
        };
        let f32_at = |row: &[u8], offset: usize| {
            f32::from_le_bytes(row[offset..offset + 4].try_into().unwrap())
        };

        // `special_context` drives the reveal branch of the weight-slot selector
        // (`r4_finalizer_engine._weight_slot`): the mode byte is matched at
        // +0x28 and the reveal flag is bit 0 of +0x2F. Both branches must be
        // represented or the shipped tables would only cover the ordinary slot.
        assert_eq!(resource.special_context.row_size, 48);
        assert_eq!(resource.special_context.row_count(), 7);
        let mut contexts = Vec::new();
        for index in 0..resource.special_context.row_count() {
            let row = resource.special_context.row(index).expect("row in range");
            contexts.push((row[0x28], row[0x29], row[0x2F] & 0x01));
        }
        assert_eq!(
            contexts,
            vec![
                (0x57, 0, 1),
                (0x6F, 0, 1),
                (0x4C, 1, 0),
                (0x7D, 1, 1),
                (0x48, 2, 0),
                (0x8E, 2, 0),
                (0x62, 2, 0),
            ]
        );
        assert!(contexts.iter().any(|(_, _, reveal)| *reveal == 1));
        assert!(contexts.iter().any(|(_, _, reveal)| *reveal == 0));

        // The auxiliary-mode threshold row (`generate_auxiliary_mode`, key
        // 0x1E7D) is the single 0x20-byte `optional_multiplier` row whose key
        // sits at +0x14; the finalizer needs its +0x10 base and +0x18 scale.
        assert_eq!(resource.optional_multiplier.row_size, 0x20);
        let mut matches = Vec::new();
        for index in 0..resource.optional_multiplier.row_count() {
            let row = resource
                .optional_multiplier
                .row(index)
                .expect("row in range");
            if u32_at(row, 0x14) == 0x1E7D {
                matches.push(index);
            }
        }
        assert_eq!(matches, vec![2259]);
        let threshold_row = resource
            .optional_multiplier
            .row(2259)
            .expect("row in range");
        assert_eq!(i32_at(threshold_row, 0x10), 2000);
        assert_eq!(f32_at(threshold_row, 0x18), 1.0);

        // Playthrough selector 3 (index 2) is the only progress vector the
        // finalizer is certified for, and it must not be all zeroes.
        assert_eq!(resource.playthrough_progress.len(), 5 * 4 * 4);
        let selector = |index: usize| -> [u32; 4] {
            let base = index * 16;
            let mut values = [0u32; 4];
            for (slot, value) in values.iter_mut().enumerate() {
                let offset = base + slot * 4;
                *value = u32::from_le_bytes(
                    resource.playthrough_progress[offset..offset + 4]
                        .try_into()
                        .unwrap(),
                );
            }
            values
        };
        assert_eq!(selector(2), [6510, 7710, 0, 7710]);
        assert_ne!(selector(2), [0, 0, 0, 0]);

        // The stage-one materializer consumes the rarity-4 Grace map.
        assert_eq!(resource.grace_maps[0].rarity, 4);
        assert_eq!(resource.grace_maps[0].effect_slot, 5);
        assert_eq!(resource.grace_maps[0].ranges.len(), 21);
    }

    #[test]
    fn rejects_resource_missing_a_finalizer_table() {
        let root = scratch_dir("missing-special-context");
        let root = write_synthetic_root(&root, |manifest, _root| {
            let tables = manifest["tables"]
                .as_array_mut()
                .expect("synthetic manifest declares tables");
            tables.retain(|table| {
                table.get("name").and_then(Value::as_str) != Some("special_context")
            });
        });
        let error = load_effect_resource(&root).expect_err("missing finalizer table is rejected");
        assert!(
            error.to_string().contains("special_context"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_altered_table_digest() {
        let root = scratch_dir("altered-digest");
        let root = write_synthetic_root(&root, |_manifest, root| {
            let path = root.join(R4_RESOURCE_DIR).join("tables/effect.bin");
            let mut bytes = fs::read(&path).unwrap();
            bytes.push(0);
            fs::write(&path, &bytes).unwrap();
        });
        let error = load_effect_resource(&root).expect_err("altered digest is rejected");
        assert!(
            error.to_string().contains("size mismatch") || error.to_string().contains("SHA-256"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_escaping_declared_path() {
        let root = scratch_dir("escaping-path");
        let root = write_synthetic_root(&root, |manifest, _root| {
            manifest["tables"][0]["file"]["filename"] = serde_json::json!("../outside.bin");
        });
        let error = load_effect_resource(&root).expect_err("escaping path is rejected");
        assert!(
            error.to_string().contains("unsafe declared path"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_duplicate_table_declaration() {
        let root = scratch_dir("duplicate-table");
        let root = write_synthetic_root(&root, |manifest, _root| {
            let duplicate = manifest["tables"][0].clone();
            manifest["tables"].as_array_mut().unwrap().push(duplicate);
        });
        let error = load_effect_resource(&root).expect_err("duplicate table is rejected");
        assert!(
            error.to_string().contains("more than once"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_grace_map_with_wrong_context_and_non_dense_ranges() {
        let wrong_context = scratch_dir("grace-context");
        let wrong_context = write_synthetic_root(&wrong_context, |_manifest, root| {
            let path = root.join(GRACE_MAP_PATHS[0].1);
            let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            map["context"]["effect_slot"] = serde_json::json!(6);
            fs::write(&path, map.to_string()).unwrap();
        });
        let error = load_effect_resource(&wrong_context).expect_err("wrong context is rejected");
        assert!(
            error.to_string().contains("is not the verified"),
            "unexpected error: {error}"
        );

        let unknown_label = scratch_dir("grace-label");
        let unknown_label = write_synthetic_root(&unknown_label, |_manifest, root| {
            let path = root.join(GRACE_MAP_PATHS[0].1);
            let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            map["context"]["playthrough"] = serde_json::json!("NG3");
            fs::write(&path, map.to_string()).unwrap();
        });
        let error = load_effect_resource(&unknown_label).expect_err("unknown label is rejected");
        assert!(
            error.to_string().contains("is not the verified"),
            "unexpected error: {error}"
        );

        let non_dense = scratch_dir("grace-dense");
        let non_dense = write_synthetic_root(&non_dense, |_manifest, root| {
            let path = root.join(GRACE_MAP_PATHS[1].1);
            let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            map["ranges"] = serde_json::json!([
                {"start": 0, "end": 0x7FFE, "grace_id": "0x00006553"},
                {"start": 0x8000, "end": 0xFFFF, "grace_id": "0x0000CE68"},
            ]);
            fs::write(&path, map.to_string()).unwrap();
        });
        let error = load_effect_resource(&non_dense).expect_err("non-dense ranges are rejected");
        assert!(
            error.to_string().contains("GraceMapNotDense"),
            "unexpected error: {error}"
        );
    }
}
