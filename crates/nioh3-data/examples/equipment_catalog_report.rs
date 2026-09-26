//! Read-only equipment catalog report for the v0.8.1 offline intake.
//!
//! Loads the declared external artifacts, normalizes every row with the product
//! adapter, and writes one JSON report carrying input identities, per-role
//! counters, the reproduced set intersections, byte-order examples, the
//! synthetic negative cases and every quarantined or sentinel row.
//!
//! Exit codes: 0 report written, 2 a declared input was rejected, 3 an optional
//! product-bundled catalog was requested but is unavailable.

use std::{collections::BTreeMap, fs, path::PathBuf};

use nioh3_data::equipment_catalog::{
    load_catalog_set, normalize_ct_token, normalize_save_effect_key, normalize_save_item_key,
    CatalogInput, CatalogRole, CatalogSet, IdNamespace, KeyOutcome, RowState,
    EQUIPMENT_CATALOG_FORMAT,
};
use serde_json::{json, Map, Value};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match Cli::parse(&args) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(64);
        }
    };

    if let Err((code, report)) = run(&cli) {
        println!("{}", serde_json::to_string_pretty(&report).expect("report"));
        std::process::exit(code);
    }
}

#[derive(Debug, Default)]
struct Cli {
    inputs: Vec<(CatalogRole, PathBuf)>,
    expectations: Vec<(CatalogRole, String)>,
    out: Option<PathBuf>,
    product_equipment: Option<PathBuf>,
    product_effects: Option<PathBuf>,
}

impl Cli {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut cli = Cli::default();
        let mut index = 0;
        while index < args.len() {
            let flag = args[index].as_str();
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("missing value for {flag}"))?;
            index += 2;
            match flag {
                "--trainer-equipment" => cli
                    .inputs
                    .push((CatalogRole::TrainerEquipment, value.into())),
                "--trainer-effects" => cli
                    .inputs
                    .push((CatalogRole::TrainerEffectVariants, value.into())),
                "--trainer-hell" => cli
                    .inputs
                    .push((CatalogRole::TrainerHellSkills, value.into())),
                "--save-items" => cli
                    .inputs
                    .push((CatalogRole::SaveActiveItems, value.into())),
                "--save-effects" => cli
                    .inputs
                    .push((CatalogRole::SaveActiveEffects, value.into())),
                "--save-items-inactive" => cli
                    .inputs
                    .push((CatalogRole::SaveInactiveItems, value.into())),
                "--save-effects-inactive" => cli
                    .inputs
                    .push((CatalogRole::SaveInactiveEffects, value.into())),
                "--ct-equipment" => cli.inputs.push((CatalogRole::CtEquipment, value.into())),
                "--ct-effect-raw" => cli.inputs.push((CatalogRole::CtEffectRaw, value.into())),
                "--ct-effect-sorted" => {
                    cli.inputs.push((CatalogRole::CtEffectSorted, value.into()))
                }
                "--expect" => {
                    let (role, sha256) = value
                        .split_once('=')
                        .ok_or_else(|| "--expect wants role=sha256".to_string())?;
                    cli.expectations
                        .push((role_from_str(role)?, sha256.to_ascii_uppercase()));
                }
                "--out" => cli.out = Some(value.into()),
                "--product-equipment" => cli.product_equipment = Some(value.into()),
                "--product-effects" => cli.product_effects = Some(value.into()),
                other => return Err(format!("unknown argument {other}")),
            }
        }
        Ok(cli)
    }
}

fn role_from_str(raw: &str) -> Result<CatalogRole, String> {
    const ROLES: [CatalogRole; 10] = [
        CatalogRole::TrainerEquipment,
        CatalogRole::TrainerEffectVariants,
        CatalogRole::TrainerHellSkills,
        CatalogRole::SaveActiveItems,
        CatalogRole::SaveActiveEffects,
        CatalogRole::SaveInactiveItems,
        CatalogRole::SaveInactiveEffects,
        CatalogRole::CtEquipment,
        CatalogRole::CtEffectRaw,
        CatalogRole::CtEffectSorted,
    ];
    ROLES
        .into_iter()
        .find(|role| role.as_str() == raw)
        .ok_or_else(|| format!("unknown role {raw}"))
}

fn run(cli: &Cli) -> Result<(), (i32, Value)> {
    let mut inputs = Vec::with_capacity(cli.inputs.len());
    for (role, path) in &cli.inputs {
        let mut input = CatalogInput::new(*role, path.clone(), version_for(*role));
        if let Some((_, sha256)) = cli
            .expectations
            .iter()
            .find(|(expected_role, _)| expected_role == role)
        {
            input = input.with_expected_sha256(sha256.clone());
        }
        inputs.push(input);
    }

    let set = match load_catalog_set(&inputs) {
        Ok(set) => set,
        Err(error) => return Err((2, error_report(&error))),
    };

    let product_catalog = match product_input_state(cli) {
        Ok(state) => state,
        Err(report) => return Err((3, report)),
    };

    let report = build_report(&set, product_catalog);
    let rendered = serde_json::to_string_pretty(&report).expect("report serializes");
    match &cli.out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(error) = fs::write(path, format!("{rendered}\n")) {
                eprintln!("cannot write report {}: {error}", path.display());
                std::process::exit(1);
            }
        }
        None => println!("{rendered}"),
    }
    Ok(())
}

fn version_for(role: CatalogRole) -> &'static str {
    match role {
        CatalogRole::TrainerEquipment
        | CatalogRole::TrainerEffectVariants
        | CatalogRole::TrainerHellSkills => {
            "trainer sample 70.0.0 extracted catalogs; game version not established"
        }
        CatalogRole::SaveActiveItems | CatalogRole::SaveActiveEffects => {
            "save-editor b5d0789791fe31d06ad325d4012aa0333c60cd8f; no game version declared upstream"
        }
        CatalogRole::SaveInactiveItems | CatalogRole::SaveInactiveEffects => {
            "save-editor b5d0789791fe31d06ad325d4012aa0333c60cd8f; shipped but not loaded by the upstream application"
        }
        CatalogRole::CtEquipment => "ct-v2.00.02-2.1 (table declares only v2.00.02 is supported)",
        CatalogRole::CtEffectRaw => "effect dump v1.05CE raw",
        CatalogRole::CtEffectSorted => "effect dump v1.05CE sorted",
    }
}

fn error_report(error: &nioh3_data::equipment_catalog::CatalogError) -> Value {
    json!({
        "schema": EQUIPMENT_CATALOG_FORMAT,
        "status": "error",
        "error": {
            "kind": error.kind(),
            "role": error.role().as_str(),
            "path": error.path(),
            "detail": error.detail(),
            "message": error.to_string(),
        }
    })
}

/// The example's own product-side input state.
///
/// It never reports validation: there is no `available` or `validated` state,
/// only "nothing was supplied" and "a supplied path could not be read".
fn product_input_state(cli: &Cli) -> Result<Value, Value> {
    let supplied: Vec<(CatalogRole, &PathBuf)> = [
        (CatalogRole::SaveActiveItems, cli.product_equipment.as_ref()),
        (CatalogRole::SaveActiveEffects, cli.product_effects.as_ref()),
    ]
    .into_iter()
    .filter_map(|(role, path)| path.map(|path| (role, path)))
    .collect();

    if supplied.is_empty() {
        return Ok(json!({
            "status": "no_input_supplied",
            "checked": false,
            "reason": "no product-side catalog input was supplied to this report; ids stay unresolved",
        }));
    }

    let mut readable = Vec::new();
    for (role, path) in &supplied {
        match fs::read(path) {
            Ok(bytes) if !bytes.is_empty() => readable.push(json!({
                "role": role.as_str(),
                "path": path.display().to_string(),
                "bytes": bytes.len(),
            })),
            Ok(_) => return Err(product_input_failure(*role, path, "supplied file is empty")),
            Err(error) => return Err(product_input_failure(*role, path, &error.to_string())),
        }
    }
    Ok(json!({
        "status": "input_supplied_not_validated",
        "checked": false,
        "note": "bytes were readable; this report makes no validation claim about them",
        "inputs": readable,
    }))
}

fn product_input_failure(role: CatalogRole, path: &PathBuf, detail: &str) -> Value {
    json!({
        "schema": EQUIPMENT_CATALOG_FORMAT,
        "status": "input_unreadable",
        "role": role.as_str(),
        "path": path.display().to_string(),
        "detail": detail,
    })
}

fn build_report(set: &CatalogSet, product_catalog: Value) -> Value {
    let roles = [
        CatalogRole::TrainerEquipment,
        CatalogRole::TrainerEffectVariants,
        CatalogRole::TrainerHellSkills,
        CatalogRole::SaveActiveItems,
        CatalogRole::SaveActiveEffects,
        CatalogRole::SaveInactiveItems,
        CatalogRole::SaveInactiveEffects,
        CatalogRole::CtEquipment,
        CatalogRole::CtEffectRaw,
        CatalogRole::CtEffectSorted,
    ];

    let inputs: Vec<Value> = set
        .inputs()
        .iter()
        .map(|identity| {
            json!({
                "role": identity.role.as_str(),
                "namespace": identity.role.namespace().as_str(),
                "path": identity.path,
                "bytes": identity.bytes,
                "sha256": identity.sha256,
                "declared_version": identity.declared_version,
            })
        })
        .collect();

    let summaries: Vec<Value> = roles
        .iter()
        .filter(|role| set.rows_for(**role).next().is_some())
        .map(|role| summary_json(set, *role))
        .collect();

    let comparisons = [
        (
            IdSource::Primary(CatalogRole::TrainerEquipment),
            IdSource::Primary(CatalogRole::SaveActiveItems),
        ),
        (
            IdSource::Primary(CatalogRole::TrainerEquipment),
            IdSource::Primary(CatalogRole::CtEquipment),
        ),
        (
            IdSource::Primary(CatalogRole::SaveActiveItems),
            IdSource::Primary(CatalogRole::CtEquipment),
        ),
        (
            IdSource::Primary(CatalogRole::TrainerEffectVariants),
            IdSource::Primary(CatalogRole::SaveActiveEffects),
        ),
        (
            IdSource::Primary(CatalogRole::TrainerEffectVariants),
            IdSource::Primary(CatalogRole::CtEffectRaw),
        ),
        (
            IdSource::Primary(CatalogRole::SaveActiveEffects),
            IdSource::Primary(CatalogRole::CtEffectSorted),
        ),
        (
            IdSource::Primary(CatalogRole::CtEffectRaw),
            IdSource::Primary(CatalogRole::CtEffectSorted),
        ),
    ];
    let mut intersections: Vec<Value> = comparisons
        .iter()
        .filter(|(left, right)| role_is_loaded(set, *left) && role_is_loaded(set, *right))
        .map(|(left, right)| intersection_json(intersection(set, *left, *right)))
        .collect();
    let secondary = [
        (
            IdSource::Secondary(CatalogRole::TrainerEffectVariants),
            IdSource::Primary(CatalogRole::SaveActiveEffects),
        ),
        (
            IdSource::Secondary(CatalogRole::TrainerEffectVariants),
            IdSource::Primary(CatalogRole::CtEffectSorted),
        ),
    ];
    intersections.extend(
        secondary
            .iter()
            .filter(|(left, right)| role_is_loaded(set, *left) && role_is_loaded(set, *right))
            .map(|(left, right)| intersection_json(intersection(set, *left, *right))),
    );
    intersections.sort_by(|a, b| {
        let key = |value: &Value| {
            (
                value["left"].as_str().unwrap_or_default().to_string(),
                value["right"].as_str().unwrap_or_default().to_string(),
            )
        };
        key(a).cmp(&key(b))
    });

    let mut quarantine: Vec<Value> = set
        .quarantined()
        .map(|row| {
            let detail = match &row.state {
                RowState::Quarantined { reason } => reason.detail(),
                _ => String::new(),
            };
            json!({
                "role": row.role.as_str(),
                "row_index": row.row_index,
                "raw_key": row.raw_key,
                "name": row.name,
                "reason": row.state.as_str(),
                "detail": detail,
            })
        })
        .collect();
    quarantine.sort_by_key(|value| {
        (
            value["role"].as_str().unwrap_or_default().to_string(),
            value["row_index"].as_u64().unwrap_or(0),
        )
    });

    let mut sentinels: Vec<Value> = set
        .sentinels()
        .map(|row| {
            json!({
                "role": row.role.as_str(),
                "row_index": row.row_index,
                "raw_key": row.raw_key,
                "token": format_token(&row.key_bytes),
                "name": row.name,
                "state": row.state.as_str(),
            })
        })
        .collect();
    sentinels.sort_by_key(|value| {
        (
            value["role"].as_str().unwrap_or_default().to_string(),
            value["row_index"].as_u64().unwrap_or(0),
        )
    });

    let mut duplicate_name_examples: Vec<Value> = Vec::new();
    for role in [
        CatalogRole::SaveActiveItems,
        CatalogRole::TrainerEffectVariants,
    ] {
        let mut names: BTreeMap<String, Vec<&nioh3_data::equipment_catalog::CatalogRow>> =
            BTreeMap::new();
        for row in set.rows_for(role) {
            if let Some(name) = &row.name {
                names.entry(name.clone()).or_default().push(row);
            }
        }
        let mut groups: Vec<Value> = names
            .iter()
            .filter(|(_, rows)| rows.len() > 1)
            .take(3)
            .map(|(name, rows)| {
                json!({
                    "name": name,
                    "rows": rows
                        .iter()
                        .map(|row| json!({
                            "raw_key": row.raw_key,
                            "id": row.id.map(|id| format!("0x{id:04X}")),
                            "state": row.state.as_str(),
                        }))
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        groups.sort_by_key(|value| value["name"].as_str().unwrap_or_default().to_string());
        duplicate_name_examples.push(json!({"role": role.as_str(), "groups": groups}));
    }

    json!({
        "schema": EQUIPMENT_CATALOG_FORMAT,
        "status": "ok",
        "crate": "nioh3-data",
        "module": "equipment_catalog",
        "claim_boundary": "source namespaces and exact-set observations only; no legality, obtainability, type, maximum, version compatibility or write path is established",
        "inputs": inputs,
        "summaries": summaries,
        "intersections": intersections,
        "normalization_examples": normalization_examples(),
        "synthetic_cases": synthetic_cases(),
        "quarantine": quarantine,
        "sentinels": sentinels,
        "duplicate_name_examples": duplicate_name_examples,
        "product_catalog": product_catalog,
    })
}

fn role_is_loaded(set: &CatalogSet, source: IdSource) -> bool {
    let role = match source {
        IdSource::Primary(role) | IdSource::Secondary(role) => role,
    };
    set.rows_for(role).next().is_some()
}

fn summary_json(set: &CatalogSet, role: CatalogRole) -> Value {
    let summary = role_summary(set, role);
    json!({
        "role": summary.role.as_str(),
        "namespace": summary.namespace.as_str(),
        "input_index": summary.input_index,
        "path": summary.path,
        "bytes": summary.bytes,
        "sha256": summary.sha256,
        "declared_version": summary.declared_version,
        "source_lines": summary.source_lines,
        "banner_rows": summary.banner_rows,
        "raw_rows": summary.raw_rows,
        "accepted_rows": summary.accepted_rows,
        "sentinel_rows": summary.sentinel_rows,
        "quarantined_rows": summary.quarantined_rows,
        "unique_comparable_ids": summary.unique_comparable_ids,
        "duplicate_id_rows": summary.duplicate_id_rows,
        "unique_secondary_ids": summary.unique_secondary_ids,
        "tokens_with_nonzero_high_word": summary.tokens_with_nonzero_high_word,
        "width_counts": summary.width_counts.iter().map(|(width, count)| (width.to_string(), json!(count))).collect::<Map<String, Value>>(),
        "sentinel_values": summary.sentinel_values,
        "null_or_empty_name_rows": summary.null_or_empty_name_rows,
        "null_column_rows": summary.null_column_rows,
        "duplicate_name_groups": summary.duplicate_name_groups,
        "rows_lost_if_keyed_by_name": summary.rows_lost_if_keyed_by_name,
    })
}

fn intersection_json(intersection: SetIntersection) -> Value {
    json!({
        "left": intersection.left,
        "right": intersection.right,
        "intersection": intersection.intersection,
        "left_only": intersection.left_only,
        "right_only": intersection.right_only,
    })
}

fn key_case(case: &str, raw: &str, outcome: &KeyOutcome) -> Value {
    match outcome {
        KeyOutcome::Key(key) => {
            // Keep the declared width visible: a two-byte token prints four
            // digits and a four-byte token prints eight.
            let id_hex = if key.key_bytes.len() == 4 {
                format!("0x{:08X}", key.id)
            } else {
                format!("0x{:04X}", key.id)
            };
            json!({
                "case": case,
                "raw": raw,
                "namespace": key.namespace.as_str(),
                "key_bytes": format_token(&key.key_bytes),
                "id": key.id,
                "id_hex": id_hex,
            "high_word": key.high_word.map(|word| format!("0x{word:04X}")),
            "comparable_u16": key.comparable_u16().map(format_id),
            "status": "accepted",
            })
        }
        KeyOutcome::Sentinel { kind, key_bytes } => json!({
            "case": case,
            "raw": raw,
            "namespace": null,
            "key_bytes": format_token(key_bytes),
            "id": null,
            "high_word": null,
            "comparable_u16": null,
            "status": kind.as_str(),
        }),
        KeyOutcome::Quarantined(reason) => json!({
            "case": case,
            "raw": raw,
            "namespace": null,
            "key_bytes": null,
            "id": null,
            "high_word": null,
            "comparable_u16": null,
            "status": reason.as_str(),
            "detail": reason.detail(),
        }),
    }
}

fn normalization_examples() -> Value {
    json!([
        key_case(
            "save_item_stored_byte_order",
            "EB9E",
            &normalize_save_item_key("EB9E")
        ),
        key_case(
            "ct_equipment_two_byte_token",
            "EB 9E",
            &normalize_ct_token("EB 9E")
        ),
        key_case(
            "save_effect_numeric_hex",
            "E99A",
            &normalize_save_effect_key("E99A")
        ),
        key_case(
            "ct_effect_four_byte_zero_high_word",
            "9A E9 00 00",
            &normalize_ct_token("9A E9 00 00")
        ),
    ])
}

fn synthetic_cases() -> Value {
    let save_item_keys = [
        "NAN",
        "2026-09-3000:00:00",
        "2026-10-2100:00:00",
        "2026-10-0500:00:00",
        "2026-09-2500:00:00",
        "2026-05-2100:00:00",
        "2026-01-0500:00:00",
        "ABC",
        "0",
        "00000",
        "",
    ];
    let ct_keys = ["9A E9 00", "9A E9 00 00 00", "9AE9", "ZZ"];
    json!({
        "ct_four_byte_nonzero_high_word": key_case(
            "ct_four_byte_nonzero_high_word",
            "9A E9 01 00",
            &normalize_ct_token("9A E9 01 00"),
        ),
        "ct_zero_sentinel": key_case("ct_zero_sentinel", "00 00", &normalize_ct_token("00 00")),
        "ct_all_ones_sentinel": key_case(
            "ct_all_ones_sentinel",
            "FF FF FF FF",
            &normalize_ct_token("FF FF FF FF"),
        ),
        "save_item_quarantine": save_item_keys
            .iter()
            .map(|raw| key_case("save_item_quarantine", raw, &normalize_save_item_key(raw)))
            .collect::<Vec<_>>(),
        "ct_token_quarantine": ct_keys
            .iter()
            .map(|raw| key_case("ct_token_quarantine", raw, &normalize_ct_token(raw)))
            .collect::<Vec<_>>(),
    })
}

// ---------------------------------------------------------------------------
// Report-only helpers. They live here, not in the product module, because they
// exist to reproduce the Pro observations and shape one JSON artifact. The
// product module exposes only normalization, provenance, loading and row
// queries.
// ---------------------------------------------------------------------------

/// A set of ids to compare: a role's own id, or its declared secondary column
/// (trainer `base_id`). The two are never interchangeable.
#[derive(Clone, Copy, PartialEq, Eq)]
enum IdSource {
    Primary(CatalogRole),
    Secondary(CatalogRole),
}

impl IdSource {
    fn label(self) -> String {
        match self {
            Self::Primary(role) => role.as_str().to_string(),
            Self::Secondary(CatalogRole::TrainerEffectVariants) => {
                "trainer_effect_base".to_string()
            }
            Self::Secondary(role) => format!("{}_secondary", role.as_str()),
        }
    }
}

struct SetIntersection {
    left: String,
    right: String,
    intersection: usize,
    left_only: usize,
    right_only: usize,
}

fn id_set(set: &CatalogSet, source: IdSource) -> std::collections::BTreeSet<u16> {
    match source {
        IdSource::Primary(role) => set.ids(role),
        IdSource::Secondary(role) => set
            .rows_for(role)
            .filter(|row| row.is_accepted())
            .filter_map(|row| row.secondary_id)
            .filter(|id| *id <= u32::from(u16::MAX))
            .map(|id| id as u16)
            .collect(),
    }
}

fn intersection(set: &CatalogSet, left: IdSource, right: IdSource) -> SetIntersection {
    let left_ids = id_set(set, left);
    let right_ids = id_set(set, right);
    SetIntersection {
        left: left.label(),
        right: right.label(),
        intersection: left_ids.intersection(&right_ids).count(),
        left_only: left_ids.difference(&right_ids).count(),
        right_only: right_ids.difference(&left_ids).count(),
    }
}

fn format_id(id: u16) -> String {
    format!("0x{id:04X}")
}

fn format_token(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Counters for one role in this report.
struct RoleSummary {
    role: CatalogRole,
    namespace: IdNamespace,
    input_index: usize,
    path: String,
    bytes: u64,
    sha256: String,
    declared_version: String,
    source_lines: usize,
    banner_rows: usize,
    raw_rows: usize,
    accepted_rows: usize,
    sentinel_rows: usize,
    quarantined_rows: usize,
    unique_comparable_ids: usize,
    duplicate_id_rows: usize,
    unique_secondary_ids: Option<usize>,
    tokens_with_nonzero_high_word: usize,
    width_counts: BTreeMap<usize, usize>,
    sentinel_values: BTreeMap<String, usize>,
    null_or_empty_name_rows: usize,
    null_column_rows: BTreeMap<String, usize>,
    duplicate_name_groups: usize,
    rows_lost_if_keyed_by_name: usize,
}

fn role_summary(set: &CatalogSet, role: CatalogRole) -> RoleSummary {
    let input_index = set
        .inputs()
        .iter()
        .position(|identity| identity.role == role)
        .expect("a summary is only requested for a loaded role");
    let identity = &set.inputs()[input_index];

    let mut raw_rows = 0usize;
    let mut accepted_rows = 0usize;
    let mut sentinel_rows = 0usize;
    let mut quarantined_rows = 0usize;
    let mut ids: std::collections::BTreeSet<u16> = std::collections::BTreeSet::new();
    let mut secondary_ids: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    let mut nonzero_high = 0usize;
    let mut width_counts: BTreeMap<usize, usize> = BTreeMap::new();
    let mut sentinel_values: BTreeMap<String, usize> = BTreeMap::new();
    let mut null_or_empty_name = 0usize;
    let mut null_columns: BTreeMap<String, usize> = BTreeMap::new();
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    let mut has_secondary = false;

    for row in set.rows_for(role) {
        raw_rows += 1;
        if row.name.is_none() || row.name.as_deref() == Some("") {
            null_or_empty_name += 1;
        }
        if let Some(name) = &row.name {
            *names.entry(name.clone()).or_insert(0) += 1;
        }
        for (column, value) in &row.declared_columns {
            if value.is_none() {
                *null_columns.entry(column.clone()).or_insert(0) += 1;
            }
        }
        if !row.key_bytes.is_empty() {
            *width_counts.entry(row.key_bytes.len()).or_insert(0) += 1;
        }
        if let Some(secondary) = row.secondary_id {
            has_secondary = true;
            secondary_ids.insert(secondary);
        }
        match &row.state {
            RowState::Accepted => {
                accepted_rows += 1;
                if let Some(high) = row.high_word {
                    if high != 0 {
                        nonzero_high += 1;
                    }
                }
                if let Some(id) = row.comparable_u16() {
                    ids.insert(id);
                }
            }
            RowState::Sentinel { .. } => {
                sentinel_rows += 1;
                let token: String = row
                    .key_bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                *sentinel_values.entry(token).or_insert(0) += 1;
            }
            RowState::Quarantined { .. } => quarantined_rows += 1,
        }
    }

    let duplicate_name_groups = names.values().filter(|count| **count > 1).count();
    let rows_lost_if_keyed_by_name: usize = names
        .values()
        .filter(|count| **count > 1)
        .map(|count| count - 1)
        .sum();

    RoleSummary {
        role,
        namespace: role.namespace(),
        input_index,
        path: identity.path.clone(),
        bytes: identity.bytes,
        sha256: identity.sha256.clone(),
        declared_version: identity.declared_version.clone(),
        source_lines: set.source_lines(role),
        banner_rows: set.banner_rows(role),
        raw_rows,
        accepted_rows,
        sentinel_rows,
        quarantined_rows,
        unique_comparable_ids: ids.len(),
        duplicate_id_rows: accepted_rows.saturating_sub(ids.len()),
        unique_secondary_ids: has_secondary.then_some(secondary_ids.len()),
        tokens_with_nonzero_high_word: nonzero_high,
        width_counts,
        sentinel_values,
        null_or_empty_name_rows: null_or_empty_name,
        null_column_rows: null_columns,
        duplicate_name_groups,
        rows_lost_if_keyed_by_name,
    }
}
