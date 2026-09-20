//! Fail-closed identity for the *selected* generation resource bundle.
//!
//! `nioh3-worker`'s `runtime_resource_digest` hashes every file below the data
//! root, so two roles that hash the same root can publish the same identity
//! while reading different versioned payloads. A whole-root hash is not a
//! selected-bundle identity: it cannot distinguish "same label, different
//! executing resource" from "same resource".
//!
//! This module defines the contract the generation service needs instead. One
//! resolver binds an exact executable file version to the version-selected
//! resource directory and to a digest over exactly the files the versioned
//! preview/effect materialization reads for that version:
//!
//! * An unregistered executable version is rejected before the data root is
//!   opened, so no fallback content can ever be read for an unknown identity.
//! * The aliased PC versions (v2.00.02 / v2.01) share one identity; PC v2.02
//!   owns a different one, and both may coexist below a single data root.
//! * Changing an *unselected* sibling bundle (the other version's directory, an
//!   unused auxiliary resource generation, or an unrelated extra file) leaves
//!   the identity unchanged; changing a *selected* input changes it.
//!
//! The selection is not guessed and is not a second copy of the loader's list.
//! It is driven by the same crate-internal descriptors the versioned loaders
//! read (`crate::resource_descriptor`): the nine R4 tables plus the bonus-curve
//! rows/index and the playthrough blob, the five auxiliary tables and whichever
//! of their declared key indexes the descriptor marks as loader input, the
//! enemy-parameter gate, the two Grace maps, and the enemy-state capture. A
//! loader and this resolver cannot therefore disagree about a table or a key
//! blob. Every declared blob is verified against the same manifest size and
//! SHA-256 the loaders enforce, and each `manifest.json` that declares those
//! blobs is itself an input, read once and hashed from that same buffer. A
//! manifest that declares a table name outside the descriptor set still fails
//! closed instead of silently under-covering the identity.
//!
//! Within-root canonical aliases (a declared path that resolves by symlink or
//! hardlink to a different path inside the resource root) are accepted and fold
//! by their canonical target path, so two spellings of one payload produce one
//! selection entry.
//!
//! Two digests are published from the same ordered selection:
//!
//! * `versioned_digest` covers only the files below the version-selected
//!   resource directory.
//! * `bundle_digest` covers every selected input, including the version-invariant
//!   companion resources the materialization also reads.
//!
//! Both fold `len(u32 LE) || data-root-relative POSIX path || file SHA-256` in
//! the same canonical order the worker uses, so the two schemes stay comparable.

use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{
    auxiliary_table, canonical_dir, declared_path, declared_resource_root, field, field_str,
    r4_table, read_declared_blob, read_manifest_snapshot, resource_descriptor,
    AUXILIARY_RESOURCE_DIR, AUXILIARY_SCHEMA, ENEMY_STATE_TABLES_PATH, R4_SCHEMA,
};
use crate::GRACE_MAP_PATHS;

/// One file the selected bundle reads, by data-root-relative POSIX path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleFile {
    pub relative_path: String,
    pub size: u64,
    /// Upper-case hex SHA-256 of the file bytes, matching the manifest spelling.
    pub sha256: String,
}

/// The identity of the generation resource bundle selected for one executable
/// version. This is the value a role should hand across the handshake instead of
/// a whole-root digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedGenerationBundle {
    /// Exact executable file version this identity was resolved for.
    pub file_version: (u16, u16, u16, u16),
    /// Version-selected offline resource directory below the data root.
    pub versioned_resource_dir: &'static str,
    /// Identity over every selected input, including version-invariant ones.
    pub bundle_digest: String,
    /// Identity over only the files below `versioned_resource_dir`.
    pub versioned_digest: String,
    /// The ordered, de-duplicated selection the two digests fold.
    pub files: Vec<BundleFile>,
}

/// Resolve the selected generation resource bundle for one executable version.
///
/// The executable version is validated first: an unregistered version returns an
/// error before the data root is read, so an unknown identity can never be
/// resolved against fallback content. Every declared input is verified against
/// its manifest size and SHA-256; a missing, escaping, or altered file is an
/// error rather than a substitution.
pub fn resolve_selected_generation_bundle(
    data_root: &Path,
    file_version: (u16, u16, u16, u16),
) -> Result<SelectedGenerationBundle, Box<dyn Error>> {
    // Reject an unknown version before anything below the data root is opened.
    let versioned_resource_dir = super::r4_resource_dir_for_file_version(file_version)?;

    let root = canonical_dir(data_root, "product data directory")?;
    let auxiliary_root = declared_resource_root(&root, AUXILIARY_RESOURCE_DIR)?;
    let versioned_root = declared_resource_root(&root, versioned_resource_dir)?;

    let mut selected = Vec::new();
    collect_versioned_files(&versioned_root, &mut selected)?;
    collect_auxiliary_files(&auxiliary_root, &mut selected)?;
    for (_, relative) in GRACE_MAP_PATHS {
        collect_plain_file(&root, relative, "grace map", false, &mut selected)?;
    }
    collect_plain_file(
        &root,
        ENEMY_STATE_TABLES_PATH,
        "enemy-state capture",
        false,
        &mut selected,
    )?;

    let ordered = order_and_deduplicate(&root, selected)?;
    let versioned_digest = fold_digest(ordered.iter().filter(|file| file.versioned));
    let bundle_digest = fold_digest(ordered.iter());
    let files = ordered
        .into_iter()
        .map(|file| BundleFile {
            relative_path: file.relative,
            size: file.size,
            sha256: sha256_hex_upper_from_digest(&file.digest),
        })
        .collect();

    Ok(SelectedGenerationBundle {
        file_version,
        versioned_resource_dir,
        bundle_digest,
        versioned_digest,
        files,
    })
}

/// One resolved file before ordering and digest folding.
struct SelectedFile {
    path: PathBuf,
    versioned: bool,
    digest: [u8; 32],
    size: u64,
}

/// One selected file after canonical ordering.
struct OrderedFile {
    relative: String,
    versioned: bool,
    digest: [u8; 32],
    size: u64,
}

fn collect_versioned_files(root: &Path, out: &mut Vec<SelectedFile>) -> Result<(), Box<dyn Error>> {
    // One read: the bytes folded into the identity are the bytes that declared
    // the records selected below.
    let (manifest_path, manifest_bytes, manifest) = read_manifest_snapshot(root, R4_SCHEMA)?;
    push_file(manifest_path, &manifest_bytes, true, out);

    let tables = manifest
        .get("tables")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{R4_SCHEMA}: manifest has no tables array"))?;
    let declared = tables
        .iter()
        .map(|table| field_str(table, "name", R4_SCHEMA).map(str::to_string));
    let expected: Vec<&str> = resource_descriptor::R4_TABLES
        .iter()
        .map(|table| table.name)
        .collect();
    require_exact_names(R4_SCHEMA, declared, &expected)?;

    for descriptor in resource_descriptor::R4_TABLES {
        let name = descriptor.name;
        let table = r4_table(&manifest, name)?;
        collect_declared_file(root, field(table, "file", name)?, name, true, out)?;
    }

    for record in resource_descriptor::R4_COMPANION_RECORDS {
        collect_declared_file(
            root,
            resource_descriptor::manifest_record(&manifest, record)?,
            record.label,
            true,
            out,
        )?;
    }
    Ok(())
}

fn collect_auxiliary_files(root: &Path, out: &mut Vec<SelectedFile>) -> Result<(), Box<dyn Error>> {
    let (manifest_path, manifest_bytes, manifest) = read_manifest_snapshot(root, AUXILIARY_SCHEMA)?;
    push_file(manifest_path, &manifest_bytes, false, out);

    let tables: &Map<String, Value> = manifest
        .get("tables")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{AUXILIARY_SCHEMA}: manifest has no tables object"))?;
    let expected: Vec<&str> = resource_descriptor::AUXILIARY_TABLES
        .iter()
        .map(|table| table.name)
        .collect();
    require_exact_names(AUXILIARY_SCHEMA, tables.keys().cloned().map(Ok), &expected)?;

    for descriptor in resource_descriptor::AUXILIARY_TABLES {
        let name = descriptor.name;
        let table = auxiliary_table(tables, name)?;
        collect_declared_file(root, field(table, "file", name)?, name, false, out)?;
        if descriptor.keys {
            collect_declared_file(
                root,
                field(table, "keys_file", name)?,
                &format!("{name} keys"),
                false,
                out,
            )?;
        }
    }

    let gate_section = resource_descriptor::AUXILIARY_GATE_SECTION;
    let gate = manifest
        .get(gate_section)
        .ok_or("enemy_parameter_gate: manifest has no enemy parameter gate")?;
    collect_declared_file(
        root,
        field(gate, resource_descriptor::AUXILIARY_GATE_KEY, gate_section)?,
        resource_descriptor::AUXILIARY_GATE_LABEL,
        false,
        out,
    )?;
    Ok(())
}

/// Fail closed when the manifest declares a consumed-name set the contract does
/// not account for, so a new table cannot slip past a stale selection silently.
fn require_exact_names(
    schema: &str,
    declared: impl Iterator<Item = Result<String, Box<dyn Error>>>,
    expected: &[&str],
) -> Result<(), Box<dyn Error>> {
    let mut declared_names = Vec::new();
    for name in declared {
        declared_names.push(name?);
    }
    for name in &declared_names {
        if !expected.contains(&name.as_str()) {
            return Err(format!(
                "{schema}: manifest declares table {name:?}, which the selected-bundle \
                 contract does not account for; update the contract before trusting the \
                 identity"
            )
            .into());
        }
    }
    for name in expected {
        if !declared_names.iter().any(|declared| declared == name) {
            return Err(
                format!("{schema}: manifest is missing the consumed table {name:?}").into(),
            );
        }
    }
    Ok(())
}

fn collect_declared_file(
    root: &Path,
    record: &Value,
    label: &str,
    versioned: bool,
    out: &mut Vec<SelectedFile>,
) -> Result<(), Box<dyn Error>> {
    let (path, bytes) = read_declared_blob(root, record, label)?;
    push_file(path, &bytes, versioned, out);
    Ok(())
}

fn collect_plain_file(
    root: &Path,
    relative: &str,
    label: &str,
    versioned: bool,
    out: &mut Vec<SelectedFile>,
) -> Result<(), Box<dyn Error>> {
    let path = declared_path(root, relative, label)?;
    collect_plain_file_path(path, versioned, out)
}

fn collect_plain_file_path(
    path: PathBuf,
    versioned: bool,
    out: &mut Vec<SelectedFile>,
) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(&path)
        .map_err(|error| format!("selected bundle: cannot read {}: {error}", path.display()))?;
    push_file(path, &bytes, versioned, out);
    Ok(())
}

fn push_file(path: PathBuf, bytes: &[u8], versioned: bool, out: &mut Vec<SelectedFile>) {
    out.push(SelectedFile {
        path,
        versioned,
        digest: Sha256::digest(bytes).into(),
        size: bytes.len() as u64,
    });
}

/// Re-express canonical paths as data-root-relative POSIX paths, drop duplicate
/// selections, and order them the way the worker orders its digests.
fn order_and_deduplicate(
    root: &Path,
    selected: Vec<SelectedFile>,
) -> Result<Vec<OrderedFile>, Box<dyn Error>> {
    let mut by_path: BTreeMap<String, OrderedFile> = BTreeMap::new();
    for file in selected {
        let relative = file
            .path
            .strip_prefix(root)
            .map_err(|_| {
                format!(
                    "selected bundle: {} escapes the product data directory",
                    file.path.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        by_path.entry(relative.clone()).or_insert(OrderedFile {
            relative,
            versioned: file.versioned,
            digest: file.digest,
            size: file.size,
        });
    }
    let mut ordered: Vec<OrderedFile> = by_path.into_values().collect();
    ordered.sort_by(|left, right| {
        left.relative
            .to_ascii_lowercase()
            .cmp(&right.relative.to_ascii_lowercase())
            .then_with(|| left.relative.cmp(&right.relative))
    });
    Ok(ordered)
}

/// Fold `len(u32 LE) || relative POSIX path || file SHA-256`, mirroring
/// `nioh3-worker`'s `runtime_resource_digest` entry encoding.
fn fold_digest<'a>(files: impl Iterator<Item = &'a OrderedFile>) -> String {
    let mut digest = Sha256::new();
    for file in files {
        let bytes = file.relative.as_bytes();
        digest.update((bytes.len() as u32).to_le_bytes());
        digest.update(bytes);
        digest.update(file.digest);
    }
    hex_lower(&digest.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from_digit(u32::from(byte >> 4), 16).expect("nibble"));
        output.push(char::from_digit(u32::from(byte & 0x0F), 16).expect("nibble"));
    }
    output
}

fn sha256_hex_upper_from_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02X}")).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use nioh3_domain::effect::GRACE_MAP_CAPTURE_STATE;
    use nioh3_domain::enemy::ENEMY_TEXT_SHA256;

    use super::*;
    use crate::{
        load_effect_resource_for_file_version, load_preview_resources_for_file_version,
        sha256_hex_upper, CONTEXT_STRIDE, ENEMY_STRIDE, OPTIONAL_MULTIPLIER_STRIDE,
        R4_RESOURCE_DIR, R4_RESOURCE_DIR_V202, RULE_CONFLICT_STRIDE, SPECIAL_RULE_STRIDE,
        TABLE_HEADER_BYTES, TERRAIN_STRIDE,
    };

    /// Row strides of the nine R4 tables, mirroring
    /// `resource_descriptor::R4_TABLES`; the versioned loader rejects a wrong
    /// value, which keeps this fixture honest.
    const VERSIONED_STRIDES: [(&str, usize); 9] = [
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

    /// Bonus-curve row size declared by the shipped manifest.
    const BONUS_CURVE_ROW_BYTES: usize = 0x58;

    /// Exact identity selection for PC v2.00.02 and its PC v2.01 alias, in
    /// canonical order. Every auxiliary key blob is listed, so a resolver that
    /// drops one while the loader still reads it fails this oracle.
    const LEGACY_SELECTION: [&str; 26] = [
        "auxiliary_generation/pc_v2_00_02/resource_v3/manifest.json",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_enemy_candidate.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_rule_conflict.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_rule_conflict_keys.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_terrain.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_terrain_keys.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/enemy_parameter_gate.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/scroll_special_rule.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/scroll_special_rule_keys.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/special_context.bin",
        "enemy_states/pc_v2_01/native_tables.json",
        "grace_output_map_e604_r4_current.json",
        "grace_output_map_e604_r5_current.json",
        "r4_finalizer/pc_v2_00_02/resource_v1/bonus_curve/index.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/bonus_curve/rows.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/globals/playthrough_progress.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/manifest.json",
        "r4_finalizer/pc_v2_00_02/resource_v1/tables/category.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/tables/category_count_multiplier.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/tables/effect.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/tables/effect_group.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/tables/item.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/tables/level_curve.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/tables/optional_multiplier.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/tables/rarity_roll.bin",
        "r4_finalizer/pc_v2_00_02/resource_v1/tables/special_context.bin",
    ];

    /// Exact identity selection for PC v2.02, in canonical order.
    const V202_SELECTION: [&str; 26] = [
        "auxiliary_generation/pc_v2_00_02/resource_v3/manifest.json",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_enemy_candidate.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_rule_conflict.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_rule_conflict_keys.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_terrain.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_terrain_keys.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/enemy_parameter_gate.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/scroll_special_rule.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/scroll_special_rule_keys.bin",
        "auxiliary_generation/pc_v2_00_02/resource_v3/tables/special_context.bin",
        "enemy_states/pc_v2_01/native_tables.json",
        "grace_output_map_e604_r4_current.json",
        "grace_output_map_e604_r5_current.json",
        "r4_finalizer/pc_v2_02/resource_v1/bonus_curve/index.bin",
        "r4_finalizer/pc_v2_02/resource_v1/bonus_curve/rows.bin",
        "r4_finalizer/pc_v2_02/resource_v1/globals/playthrough_progress.bin",
        "r4_finalizer/pc_v2_02/resource_v1/manifest.json",
        "r4_finalizer/pc_v2_02/resource_v1/tables/category.bin",
        "r4_finalizer/pc_v2_02/resource_v1/tables/category_count_multiplier.bin",
        "r4_finalizer/pc_v2_02/resource_v1/tables/effect.bin",
        "r4_finalizer/pc_v2_02/resource_v1/tables/effect_group.bin",
        "r4_finalizer/pc_v2_02/resource_v1/tables/item.bin",
        "r4_finalizer/pc_v2_02/resource_v1/tables/level_curve.bin",
        "r4_finalizer/pc_v2_02/resource_v1/tables/optional_multiplier.bin",
        "r4_finalizer/pc_v2_02/resource_v1/tables/rarity_roll.bin",
        "r4_finalizer/pc_v2_02/resource_v1/tables/special_context.bin",
    ];

    /// The shipped identity digests, as established by the review and the
    /// independent recomputation. Preserved so an encoding drift fails closed.
    const LEGACY_VERSIONED_DIGEST: &str =
        "915756776dc7c7a236bee49bef5f5ab19dd3a676bdcd405cf88c0fefed977532";
    const LEGACY_BUNDLE_DIGEST: &str =
        "5e81a56c268a18a6f799447fdd7c445fea949c24ae555e0cfa75cf9016d5cd92";
    const V202_VERSIONED_DIGEST: &str =
        "09fa65803a0c058880f4d38900290b152eab89febb03615ce2576f4b020b358b";
    const V202_BUNDLE_DIGEST: &str =
        "df15220de9e356755bd8b4e2ec33f4617cf0e7c140347898b8374fd75515acbf";

    fn resolve(root: &Path, version: (u16, u16, u16, u16)) -> SelectedGenerationBundle {
        resolve_selected_generation_bundle(root, version)
            .unwrap_or_else(|error| panic!("{version:?}: {error}"))
    }

    /// The identity-bearing fields, ignoring the requested version tuple.
    fn identity(bundle: &SelectedGenerationBundle) -> (&str, &str, &str, &[BundleFile]) {
        (
            bundle.versioned_resource_dir,
            &bundle.bundle_digest,
            &bundle.versioned_digest,
            &bundle.files,
        )
    }

    fn selected_paths(bundle: &SelectedGenerationBundle) -> Vec<&str> {
        bundle
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect()
    }

    fn scratch(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "nioh3-data-t5a-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn table_store(stride: usize, rows: usize) -> Vec<u8> {
        let mut store = vec![0u8; TABLE_HEADER_BYTES + stride * rows];
        store[0..4].copy_from_slice(&[0x00, 0x22, 0x04, 0x20]);
        store[4..TABLE_HEADER_BYTES].copy_from_slice(&(rows as u32).to_le_bytes());
        store
    }

    fn write_blob(root: &Path, relative: &str, bytes: &[u8]) -> Value {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(&path, bytes).expect("write blob");
        serde_json::json!({
            "filename": relative,
            "size": bytes.len(),
            "sha256": sha256_hex_upper(bytes),
        })
    }

    fn write_manifest(resource: &Path, manifest: &Value) {
        fs::write(
            resource.join("manifest.json"),
            serde_json::to_vec_pretty(manifest).unwrap(),
        )
        .unwrap();
    }

    fn write_versioned(root: &Path, dir: &str) {
        let resource = root.join(dir);
        fs::create_dir_all(resource.join("tables")).unwrap();
        fs::create_dir_all(resource.join("bonus_curve")).unwrap();
        fs::create_dir_all(resource.join("globals")).unwrap();

        let mut tables = Vec::new();
        for (name, stride) in VERSIONED_STRIDES {
            let file = write_blob(
                &resource,
                &format!("tables/{name}.bin"),
                &table_store(stride, 2),
            );
            tables.push(serde_json::json!({
                "name": name,
                "row_size": stride,
                "row_count": 2,
                "file": file,
            }));
        }
        let rows_file = write_blob(
            &resource,
            "bonus_curve/rows.bin",
            &[0u8; BONUS_CURVE_ROW_BYTES * 2],
        );
        let index_file = write_blob(&resource, "bonus_curve/index.bin", &[0u8; 2 * 4]);
        let playthrough_file = write_blob(
            &resource,
            "globals/playthrough_progress.bin",
            &[0u8; 5 * 4 * 4],
        );

        let manifest = serde_json::json!({
            "schema": R4_SCHEMA,
            "game_version": "PC v2.00.02",
            "tables": tables,
            "bonus_curve": {
                "row_size": BONUS_CURVE_ROW_BYTES,
                "entry_count": 2,
                "unique_row_count": 2,
                "rows_file": rows_file,
                "index_file": index_file,
            },
            "playthrough": {
                "selector_min": 1,
                "selector_max": 5,
                "values_per_selector": 4,
                "file": playthrough_file,
            },
            "files": [],
        });
        write_manifest(&resource, &manifest);
    }

    fn write_auxiliary(root: &Path) {
        let resource = root.join(AUXILIARY_RESOURCE_DIR);
        let terrain = write_blob(
            &resource,
            "tables/auxiliary_terrain.bin",
            &table_store(TERRAIN_STRIDE, 1),
        );
        let terrain_keys = write_blob(
            &resource,
            "tables/auxiliary_terrain_keys.bin",
            &[0xD4, 0x00],
        );
        let enemies = write_blob(
            &resource,
            "tables/auxiliary_enemy_candidate.bin",
            &table_store(ENEMY_STRIDE, 1),
        );
        let context = write_blob(
            &resource,
            "tables/special_context.bin",
            &table_store(CONTEXT_STRIDE, 1),
        );
        let rules = write_blob(
            &resource,
            "tables/scroll_special_rule.bin",
            &table_store(SPECIAL_RULE_STRIDE, 1),
        );
        let rule_keys = write_blob(
            &resource,
            "tables/scroll_special_rule_keys.bin",
            &[0x00, 0x00],
        );
        let conflicts = write_blob(
            &resource,
            "tables/auxiliary_rule_conflict.bin",
            &table_store(RULE_CONFLICT_STRIDE, 1),
        );
        let conflict_keys = write_blob(
            &resource,
            "tables/auxiliary_rule_conflict_keys.bin",
            &[0x00, 0x00],
        );
        let gate = write_blob(&resource, "tables/enemy_parameter_gate.bin", &[0u8; 8]);

        let manifest = serde_json::json!({
            "schema": AUXILIARY_SCHEMA,
            "game_version": "PC v2.00.02",
            "tables": {
                "auxiliary_terrain": {
                    "row_size": TERRAIN_STRIDE, "row_count": 1,
                    "file": terrain, "keys_file": terrain_keys,
                },
                "auxiliary_enemy_candidate": {
                    "row_size": ENEMY_STRIDE, "row_count": 1, "file": enemies,
                },
                "special_context": {
                    "row_size": CONTEXT_STRIDE, "row_count": 1, "file": context,
                },
                "scroll_special_rule": {
                    "row_size": SPECIAL_RULE_STRIDE, "row_count": 1,
                    "file": rules, "keys_file": rule_keys,
                },
                "auxiliary_rule_conflict": {
                    "row_size": RULE_CONFLICT_STRIDE, "row_count": 1,
                    "file": conflicts, "keys_file": conflict_keys,
                },
            },
            "enemy_parameter_gate": {"entry_count": 1, "file": gate},
        });
        write_manifest(&resource, &manifest);
    }

    fn write_shared(root: &Path) {
        for (rarity, relative) in crate::GRACE_MAP_PATHS {
            let map = serde_json::json!({
                "format": crate::GRACE_MAP_FORMAT,
                "game_version": crate::GRACE_MAP_GAME_VERSION,
                "context": {
                    "record_type": crate::GRACE_MAP_RECORD_TYPE,
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
        let capture = serde_json::json!({
            "schema_version": 1,
            "text_sha256": ENEMY_TEXT_SHA256,
            "enemy_index_complete": false,
            "config_4543_lookup_observed": true,
            "config_4543_hex": Value::Null,
            "positions_by_terrain": {},
            "eligibility_by_lookup": {},
        });
        let path = root.join(ENEMY_STATE_TABLES_PATH);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, serde_json::to_vec(&capture).unwrap()).unwrap();
    }

    /// A complete data root: both versioned bundles, the auxiliary bundle, an
    /// unused auxiliary generation, and the shared companion inputs.
    fn write_fixture(tag: &str) -> PathBuf {
        let root = scratch(tag);
        write_auxiliary(&root);
        let auxiliary_parent = root
            .join(AUXILIARY_RESOURCE_DIR)
            .parent()
            .unwrap()
            .to_path_buf();
        write_blob(
            &auxiliary_parent,
            "resource_v1/tables/unused.bin",
            b"unused auxiliary generation",
        );
        write_versioned(&root, R4_RESOURCE_DIR);
        write_versioned(&root, R4_RESOURCE_DIR_V202);
        write_shared(&root);
        root
    }

    /// Rewrite one versioned table with a same-shaped but different payload and
    /// keep its manifest record consistent, i.e. a legitimately changed bundle.
    fn tweak_versioned_table(root: &Path, dir: &str, name: &str, stride: usize) {
        let resource = root.join(dir);
        let manifest_path = resource.join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        let table = manifest["tables"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|table| table["name"] == serde_json::json!(name))
            .expect("declared table");
        let relative = table["file"]["filename"].as_str().unwrap().to_string();
        let mut store = table_store(stride, 2);
        store[TABLE_HEADER_BYTES] ^= 0xFF;
        fs::write(resource.join(&relative), &store).unwrap();
        table["file"] = serde_json::json!({
            "filename": relative,
            "size": store.len(),
            "sha256": sha256_hex_upper(&store),
        });
        write_manifest(&resource, &manifest);
    }

    /// Find the manifest record that declares `filename`, wherever it nests.
    fn find_declared_record<'a>(value: &'a mut Value, filename: &str) -> Option<&'a mut Value> {
        let matches = matches!(
            value,
            Value::Object(map)
                if map.get("filename").and_then(Value::as_str) == Some(filename)
        );
        if matches {
            return Some(value);
        }
        match value {
            Value::Object(map) => map
                .values_mut()
                .find_map(|child| find_declared_record(child, filename)),
            Value::Array(items) => items
                .iter_mut()
                .find_map(|child| find_declared_record(child, filename)),
            _ => None,
        }
    }

    fn read_manifest_value(resource: &Path) -> Value {
        serde_json::from_slice(&fs::read(resource.join("manifest.json")).unwrap()).unwrap()
    }

    /// Rewrite one manifest-declared blob and keep its record's size/SHA
    /// consistent, i.e. a legitimately changed declared input.
    fn rewrite_declared_blob(root: &Path, dir: &str, relative: &str, bytes: &[u8]) {
        let resource = root.join(dir);
        let mut manifest = read_manifest_value(&resource);
        let record = find_declared_record(&mut manifest, relative)
            .unwrap_or_else(|| panic!("no declared record for {relative}"));
        fs::write(resource.join(relative), bytes).unwrap();
        record["size"] = serde_json::json!(bytes.len());
        record["sha256"] = serde_json::json!(sha256_hex_upper(bytes));
        write_manifest(&resource, &manifest);
    }

    /// Point one declared record at a different declared path.
    fn repoint_declared_record(root: &Path, dir: &str, from: &str, to: &str) {
        let resource = root.join(dir);
        let mut manifest = read_manifest_value(&resource);
        let record = find_declared_record(&mut manifest, from)
            .unwrap_or_else(|| panic!("no declared record for {from}"));
        record["filename"] = serde_json::json!(to);
        write_manifest(&resource, &manifest);
    }

    /// Add a benign top-level key so the manifest bytes change without changing
    /// the declared selection.
    fn add_manifest_marker(root: &Path, dir: &str) {
        let resource = root.join(dir);
        let mut manifest = read_manifest_value(&resource);
        manifest["t5a_marker"] = serde_json::json!("changed");
        write_manifest(&resource, &manifest);
    }

    fn flip_last_byte(path: &Path) {
        let mut bytes = fs::read(path).unwrap();
        bytes.push(b' ');
        fs::write(path, &bytes).unwrap();
    }

    fn symlink_file(target: &Path, link: &Path) -> Result<(), String> {
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, link).map_err(|error| error.to_string())
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).map_err(|error| error.to_string())
        }
    }

    /// Create a directory link. Windows junctions need no symlink privilege, so
    /// this is the fallback that still exercises an outside-root target.
    fn link_directory(target: &Path, link: &Path) -> Result<(), String> {
        #[cfg(windows)]
        {
            // A junction needs no symlink privilege on NTFS. PowerShell is used
            // because it quotes the two paths without a nested-quoting layer.
            let script = format!(
                "New-Item -ItemType Junction -Path '{}' -Target '{}' -ErrorAction Stop | Out-Null",
                link.display(),
                target.display()
            );
            let output = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .output()
                .map_err(|error| error.to_string())?;
            if output.status.success() && link.exists() {
                Ok(())
            } else {
                Err(format!(
                    "junction status {:?}: {} {}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr).trim(),
                    String::from_utf8_lossy(&output.stdout).trim()
                ))
            }
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).map_err(|error| error.to_string())
        }
    }

    #[test]
    fn fixture_bundle_loads_through_the_versioned_entry_points() {
        // The selection is only meaningful if it describes a bundle the real
        // versioned loaders accept, so drive every version through them and
        // check the resolver against the exact selection oracle.
        let root = write_fixture("loadable");
        for (version, oracle) in [
            ((2, 0, 0, 2), LEGACY_SELECTION),
            ((2, 0, 1, 0), LEGACY_SELECTION),
            ((2, 0, 2, 0), V202_SELECTION),
        ] {
            load_preview_resources_for_file_version(&root, version)
                .unwrap_or_else(|error| panic!("{version:?} preview: {error}"));
            load_effect_resource_for_file_version(&root, version)
                .unwrap_or_else(|error| panic!("{version:?} effect: {error}"));
            assert_eq!(
                selected_paths(&resolve(&root, version)),
                oracle.to_vec(),
                "{version:?}: selection must match the exact 26-path oracle"
            );
        }
    }

    #[test]
    fn aliased_versions_share_one_bundle_identity() {
        let root = write_fixture("aliasing");
        let legacy = resolve(&root, (2, 0, 0, 2));
        let v201 = resolve(&root, (2, 0, 1, 0));

        assert_eq!(legacy.versioned_resource_dir, R4_RESOURCE_DIR);
        assert_eq!(
            identity(&legacy),
            identity(&v201),
            "v2.00.02 and v2.01 alias one payload"
        );
    }

    #[test]
    fn v202_bundle_is_separated_from_the_aliased_legacy_bundle() {
        let root = write_fixture("separation");
        let legacy = resolve(&root, (2, 0, 0, 2));
        let v202 = resolve(&root, (2, 0, 2, 0));

        // Old and new bundles coexist below this one data root...
        assert!(root.join(R4_RESOURCE_DIR).is_dir());
        assert!(root.join(R4_RESOURCE_DIR_V202).is_dir());
        // ...and each identity is bound to its own bundle.
        assert_eq!(v202.versioned_resource_dir, R4_RESOURCE_DIR_V202);
        assert_ne!(legacy.bundle_digest, v202.bundle_digest);
        assert_ne!(legacy.versioned_digest, v202.versioned_digest);
        assert_ne!(legacy.files, v202.files);
    }

    #[test]
    fn unknown_version_is_rejected_before_the_data_root_is_read() {
        let missing = scratch("unknown-version").join("does-not-exist");
        for unknown in [(2, 1, 0, 0), (1, 0, 0, 0), (2, 0, 3, 0), (0, 0, 0, 0)] {
            let error = resolve_selected_generation_bundle(&missing, unknown)
                .expect_err("an unregistered version must be refused");
            let message = error.to_string();
            assert!(
                message.contains("no offline generation resource"),
                "{unknown:?}: {message}"
            );
            // A missing root reports a different error, so the version check
            // provably ran first and no fallback content was opened.
            assert!(
                !message.contains("cannot resolve"),
                "{unknown:?}: {message}"
            );
        }
        // Control: the same missing root is reported for a registered version.
        let error = resolve_selected_generation_bundle(&missing, (2, 0, 0, 2))
            .expect_err("a missing data root must fail closed");
        assert!(error.to_string().contains("cannot resolve"), "{error}");
    }

    #[test]
    fn changing_an_unselected_sibling_bundle_leaves_the_identity_unchanged() {
        let root = write_fixture("siblings");
        let before = resolve(&root, (2, 0, 0, 2));

        // A legitimate change to the other version's payload.
        tweak_versioned_table(&root, R4_RESOURCE_DIR_V202, "item", 0x1A0);
        // A changed file in the unused auxiliary generation under the same parent.
        fs::write(
            root.join(AUXILIARY_RESOURCE_DIR)
                .parent()
                .unwrap()
                .join("resource_v1/tables/unused.bin"),
            b"unused auxiliary generation, changed",
        )
        .unwrap();
        // An unrelated extra file at the data root.
        fs::write(
            root.join("unrelated_extra.json"),
            b"{\"note\":\"never read\"}",
        )
        .unwrap();

        let after = resolve(&root, (2, 0, 0, 2));
        assert_eq!(before.bundle_digest, after.bundle_digest);
        assert_eq!(before.versioned_digest, after.versioned_digest);
        assert_eq!(before.files, after.files);

        // The sibling really did change, so this is not a no-op comparison.
        let sibling = resolve(&root, (2, 0, 2, 0));
        assert_ne!(before.versioned_digest, sibling.versioned_digest);
        assert_ne!(before.bundle_digest, sibling.bundle_digest);
    }

    #[test]
    fn every_selected_input_class_moves_the_identity() {
        struct MutationCase {
            name: &'static str,
            mutate: fn(&Path),
            /// Whether the mutation must also move the versioned-payload identity.
            affects_versioned: bool,
        }

        // One selected input per class the review asked for.
        let cases = [
            MutationCase {
                name: "r4 manifest",
                mutate: |root| add_manifest_marker(root, R4_RESOURCE_DIR_V202),
                affects_versioned: true,
            },
            MutationCase {
                name: "r4 blob",
                mutate: |root| {
                    tweak_versioned_table(
                        root,
                        R4_RESOURCE_DIR_V202,
                        "optional_multiplier",
                        OPTIONAL_MULTIPLIER_STRIDE,
                    );
                },
                affects_versioned: true,
            },
            MutationCase {
                name: "r4 companion blob",
                mutate: |root| {
                    const PLAYTHROUGH: &str = "globals/playthrough_progress.bin";
                    let path = root.join(R4_RESOURCE_DIR_V202).join(PLAYTHROUGH);
                    let mut bytes = fs::read(&path).expect("playthrough blob");
                    bytes[0] ^= 0xFF;
                    rewrite_declared_blob(root, R4_RESOURCE_DIR_V202, PLAYTHROUGH, &bytes);
                },
                affects_versioned: true,
            },
            MutationCase {
                name: "auxiliary manifest",
                mutate: |root| add_manifest_marker(root, AUXILIARY_RESOURCE_DIR),
                affects_versioned: false,
            },
            MutationCase {
                name: "auxiliary blob",
                mutate: |root| {
                    let mut store = table_store(TERRAIN_STRIDE, 1);
                    store[TABLE_HEADER_BYTES] ^= 0xFF;
                    rewrite_declared_blob(
                        root,
                        AUXILIARY_RESOURCE_DIR,
                        "tables/auxiliary_terrain.bin",
                        &store,
                    );
                },
                affects_versioned: false,
            },
            MutationCase {
                name: "auxiliary key index",
                mutate: |root| {
                    rewrite_declared_blob(
                        root,
                        AUXILIARY_RESOURCE_DIR,
                        "tables/auxiliary_rule_conflict_keys.bin",
                        &[0x01, 0x02],
                    );
                },
                affects_versioned: false,
            },
            MutationCase {
                name: "enemy-state capture",
                mutate: |root| flip_last_byte(&root.join(ENEMY_STATE_TABLES_PATH)),
                affects_versioned: false,
            },
        ];

        for case in cases {
            let name = case.name;
            let root = write_fixture(&format!("selected-{}", name.replace(' ', "-")));
            let version = (2, 0, 2, 0);
            let before = resolve(&root, version);
            (case.mutate)(&root);
            let after = resolve(&root, version);
            assert_ne!(before.bundle_digest, after.bundle_digest, "{name} bundle");
            if case.affects_versioned {
                assert_ne!(
                    before.versioned_digest, after.versioned_digest,
                    "{name} versioned"
                );
            } else {
                assert_eq!(
                    before.versioned_digest, after.versioned_digest,
                    "{name} versioned"
                );
            }
        }
    }

    #[test]
    fn a_selected_grace_map_moves_only_the_bundle_identity() {
        let root = write_fixture("selected-grace");
        let version = (2, 0, 2, 0);
        let before = resolve(&root, version);
        flip_last_byte(&root.join(crate::GRACE_MAP_PATHS[0].1));
        let after = resolve(&root, version);
        assert_ne!(before.bundle_digest, after.bundle_digest);
        assert_eq!(before.versioned_digest, after.versioned_digest);
    }

    #[test]
    fn a_manifest_table_outside_the_contract_fails_closed() {
        let root = write_fixture("guard-versioned");
        let manifest_path = root.join(R4_RESOURCE_DIR).join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["tables"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"name": "surprise_table", "row_size": 1, "row_count": 1}));
        write_manifest(&root.join(R4_RESOURCE_DIR), &manifest);
        let error = resolve_selected_generation_bundle(&root, (2, 0, 0, 2))
            .expect_err("an unaccounted table must fail closed");
        assert!(error.to_string().contains("surprise_table"), "{error}");

        let root = write_fixture("guard-auxiliary");
        let manifest_path = root.join(AUXILIARY_RESOURCE_DIR).join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["tables"]["surprise_table"] = serde_json::json!({"row_size": 1, "row_count": 1});
        write_manifest(&root.join(AUXILIARY_RESOURCE_DIR), &manifest);
        let error = resolve_selected_generation_bundle(&root, (2, 0, 0, 2))
            .expect_err("an unaccounted auxiliary table must fail closed");
        assert!(error.to_string().contains("surprise_table"), "{error}");
    }

    /// One resolution must read each manifest exactly once: the folded bytes are
    /// the parsed bytes, not a second read of the same path.
    #[test]
    fn each_manifest_is_read_from_one_snapshot_per_resolution() {
        let root = write_fixture("single-read");
        let _ = crate::manifest_read_probe::take();
        let _ = resolve(&root, (2, 0, 2, 0));
        assert_eq!(
            crate::manifest_read_probe::take(),
            2,
            "expected one snapshot read for the R4 manifest and one for the auxiliary manifest"
        );
    }

    #[test]
    fn traversal_and_absolute_declared_paths_are_rejected() {
        let cases = [
            "../escape.bin",
            "/escape.bin",
            "C:/escape.bin",
            "tables/../../escape.bin",
        ];
        for (index, relative) in cases.iter().enumerate() {
            let root = write_fixture(&format!("escape-{index}"));
            repoint_declared_record(
                &root,
                AUXILIARY_RESOURCE_DIR,
                "tables/auxiliary_terrain.bin",
                relative,
            );
            let error = resolve_selected_generation_bundle(&root, (2, 0, 0, 2))
                .expect_err("an escaping declared path must fail closed");
            assert!(
                error.to_string().contains("unsafe declared path"),
                "{relative}: {error}"
            );
        }
    }

    /// A declared path that resolves outside the resource root is rejected even
    /// though every component is a plain name.
    #[test]
    fn an_outside_root_symlink_target_is_rejected() {
        let root = write_fixture("symlink-outside");
        let outside = scratch("symlink-outside-target").join("outside.bin");
        fs::write(&outside, b"outside the data root").unwrap();
        let link = root
            .join(AUXILIARY_RESOURCE_DIR)
            .join("tables/escape_link.bin");
        if let Err(reason) = symlink_file(&outside, &link) {
            eprintln!("skipped: file symlinks unavailable on this host: {reason}");
            return;
        }
        repoint_declared_record(
            &root,
            AUXILIARY_RESOURCE_DIR,
            "tables/auxiliary_terrain.bin",
            "tables/escape_link.bin",
        );
        let error = resolve_selected_generation_bundle(&root, (2, 0, 0, 2))
            .expect_err("a target outside the resource root must fail closed");
        assert!(
            error.to_string().contains("escapes the resource root"),
            "{error}"
        );
    }

    /// Documented accepted behavior: a declared path that resolves by symlink to
    /// another path *inside* the resource root is accepted and folds by its
    /// canonical target, so one payload keeps one selection entry.
    #[test]
    fn a_within_root_canonical_alias_folds_to_its_target() {
        let root = write_fixture("symlink-inside");
        let target_relative = "tables/auxiliary_rule_conflict_keys.bin";
        let alias_relative = "tables/alias_rule_conflict_keys.bin";
        let resource = root.join(AUXILIARY_RESOURCE_DIR);
        if let Err(reason) = symlink_file(
            &resource.join(target_relative),
            &resource.join(alias_relative),
        ) {
            eprintln!("skipped: file symlinks unavailable on this host: {reason}");
            return;
        }
        let before = resolve(&root, (2, 0, 0, 2));
        repoint_declared_record(
            &root,
            AUXILIARY_RESOURCE_DIR,
            target_relative,
            alias_relative,
        );
        let after = resolve(&root, (2, 0, 0, 2));

        assert_eq!(
            selected_paths(&after),
            LEGACY_SELECTION.to_vec(),
            "the alias folds to its canonical target path, not to its own spelling"
        );
        assert_eq!(
            before.files, after.files,
            "a within-root alias of the same bytes is identity-neutral"
        );
    }

    /// A directory reparse point below the resource root that points outside it
    /// must be rejected. Windows can create a directory junction without the
    /// symlink privilege, so this usually runs where file symlinks cannot.
    #[test]
    fn an_outside_root_directory_link_target_is_rejected() {
        let root = write_fixture("dir-link-outside");
        let outside = scratch("dir-link-outside-target");
        fs::write(outside.join("outside.bin"), b"outside the data root").unwrap();
        let resource = root.join(AUXILIARY_RESOURCE_DIR);
        let link = resource.join("tables/escape_dir");
        if let Err(reason) = link_directory(&outside, &link) {
            eprintln!("skipped: directory links unavailable on this host: {reason}");
            return;
        }
        repoint_declared_record(
            &root,
            AUXILIARY_RESOURCE_DIR,
            "tables/auxiliary_terrain.bin",
            "tables/escape_dir/outside.bin",
        );
        let error = resolve_selected_generation_bundle(&root, (2, 0, 0, 2))
            .expect_err("a link target outside the resource root must fail closed");
        assert!(
            error.to_string().contains("escapes the resource root"),
            "{error}"
        );
    }

    /// Two records that name the same declared path fold into one selection
    /// entry, which is the same rule a within-root symlink alias relies on.
    #[test]
    fn one_payload_named_by_two_records_folds_to_one_selection_entry() {
        let root = write_fixture("duplicate-name");
        let resource = root.join(AUXILIARY_RESOURCE_DIR);
        let mut manifest = read_manifest_value(&resource);
        let twin = manifest["tables"]["auxiliary_enemy_candidate"]["file"].clone();
        manifest["tables"]["auxiliary_terrain"]["file"] = twin;
        write_manifest(&resource, &manifest);

        let bundle = resolve(&root, (2, 0, 0, 2));
        assert_eq!(
            bundle.files.len(),
            LEGACY_SELECTION.len() - 1,
            "the twin record must fold into the existing entry"
        );
        assert!(bundle.files.iter().any(|file| file
            .relative_path
            .ends_with("tables/auxiliary_enemy_candidate.bin")));
        assert!(!bundle
            .files
            .iter()
            .any(|file| file.relative_path.ends_with("tables/auxiliary_terrain.bin")));
    }

    #[test]
    fn selected_inputs_are_verified_against_their_manifest_declaration() {
        let root = write_fixture("altered-digest");
        let table = root
            .join(R4_RESOURCE_DIR)
            .join("tables/optional_multiplier.bin");
        let mut bytes = fs::read(&table).unwrap();
        bytes.push(0);
        fs::write(&table, &bytes).unwrap();
        let error = resolve_selected_generation_bundle(&root, (2, 0, 0, 2))
            .expect_err("an altered selected input must fail closed");
        let message = error.to_string();
        assert!(
            message.contains("size mismatch") || message.contains("SHA-256"),
            "{message}"
        );
    }

    #[test]
    fn product_bundle_identity_is_version_bound() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data");
        if !root.is_dir() {
            return;
        }
        let legacy = resolve(&root, (2, 0, 0, 2));
        let v201 = resolve(&root, (2, 0, 1, 0));
        let v202 = resolve(&root, (2, 0, 2, 0));

        assert_eq!(identity(&legacy), identity(&v201));
        assert_eq!(legacy.versioned_resource_dir, R4_RESOURCE_DIR);
        assert_eq!(v202.versioned_resource_dir, R4_RESOURCE_DIR_V202);
        assert_ne!(legacy.bundle_digest, v202.bundle_digest);

        // Exact selection oracle and preserved digest values for both bundles.
        assert_eq!(selected_paths(&legacy), LEGACY_SELECTION.to_vec());
        assert_eq!(selected_paths(&v202), V202_SELECTION.to_vec());
        assert_eq!(legacy.versioned_digest, LEGACY_VERSIONED_DIGEST);
        assert_eq!(legacy.bundle_digest, LEGACY_BUNDLE_DIGEST);
        assert_eq!(v202.versioned_digest, V202_VERSIONED_DIGEST);
        assert_eq!(v202.bundle_digest, V202_BUNDLE_DIGEST);
        // Stable across repeated resolution of the same product root.
        assert_eq!(
            v202.bundle_digest,
            resolve(&root, (2, 0, 2, 0)).bundle_digest
        );

        // Every versioned file of the v2.02 identity lives in its own directory.
        for file in &v202.files {
            if file.relative_path.starts_with("r4_finalizer/") {
                assert!(
                    file.relative_path.starts_with(R4_RESOURCE_DIR_V202),
                    "{}",
                    file.relative_path
                );
            }
        }
        // The selection lists selected inputs only; unrelated data-root files are
        // absent, so this is not a whole-root hash.
        assert!(v202
            .files
            .iter()
            .any(|file| file.relative_path == ENEMY_STATE_TABLES_PATH));
        assert!(v202
            .files
            .iter()
            .any(|file| file.relative_path.starts_with(AUXILIARY_RESOURCE_DIR)));
        assert!(!v202.files.iter().any(|file| file
            .relative_path
            .contains("effect_names_multilingual.json")));
        assert!(!v202.files.iter().any(|file| file
            .relative_path
            .contains("live_add_pc_v202_identity.json")));
    }
}
