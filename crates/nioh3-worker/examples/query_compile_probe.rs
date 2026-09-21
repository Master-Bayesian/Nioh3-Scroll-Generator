//! Development probe: compile queries in Rust and compare with Python.
//!
//! `deliverables/m23b-search/dump_query_expectations.py` records, for
//! representative queries, the native arguments the shipped Python worker
//! builds from the product tables and the DLL page for bounded windows. This
//! probe compiles the same queries with `query_compile::QueryCompiler` and
//! compares compiled spec, pivot values, matches and stage counts.
//!
//! Usage:
//!   cargo run --example query_compile_probe -- <expectations.json> <data_root>
//!       [--allow-cpu] [--accelerator <path>]

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use nioh3_worker::native_search::{
    Accelerator, AuxiliaryPivotSpec, ExecutionPolicy, PivotMatch, PivotWindow, R4PrimaryPivotSpec,
};
use nioh3_worker::query::SearchQuery;
use nioh3_worker::query_compile::{QueryCompiler, Route};
use nioh3_worker::search_backend::NativePivotQuery;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let expectations_path = args
        .next()
        .unwrap_or_else(|| "deliverables/m23b-search/evidence/query_expectations.json".into());
    let data_root = args
        .next()
        .unwrap_or_else(|| "nioh3_scroll_editor/data".into());
    let mut allow_cpu = false;
    let mut accelerator_path: Option<PathBuf> = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--allow-cpu" => allow_cpu = true,
            "--accelerator" => accelerator_path = args.next().map(PathBuf::from),
            other => {
                eprintln!("unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }

    let text = match std::fs::read_to_string(&expectations_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cannot read {expectations_path}: {error}");
            return ExitCode::from(2);
        }
    };
    let payload: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid expectations json: {error}");
            return ExitCode::from(2);
        }
    };

    let application_root = Path::new("../..");
    let resolved_override = accelerator_path.or_else(|| {
        env::var("NIOH3_SEED_ACCELERATOR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
    });
    let Some(accelerator) = Accelerator::load(application_root, resolved_override.as_deref())
    else {
        eprintln!("accelerator unavailable");
        return ExitCode::from(3);
    };
    let policy = if allow_cpu {
        ExecutionPolicy::AllowBulkCpu
    } else {
        ExecutionPolicy::StrictGpu
    };
    let _pin = match accelerator.pin_policy(policy) {
        Ok(pin) => pin,
        Err(error) => {
            eprintln!("cannot pin policy: {error:?}");
            return ExitCode::from(3);
        }
    };

    let compiler = match QueryCompiler::load(Path::new(&data_root)) {
        Ok(compiler) => compiler,
        Err(error) => {
            eprintln!("cannot load tables: {error}");
            return ExitCode::from(3);
        }
    };
    // The shipped dispatch gates the rarity-3 named-primary pivot on the real
    // `d3d11_effect_acceleration_available` probe; the probe passes the same
    // answer the worker's factory would.
    let preimage_available =
        nioh3_worker::capabilities::probe(application_root, None, None).directcompute_probe;

    let expected_values = payload["values_sha256"].as_str().unwrap_or_default();
    let mut failures: Vec<Value> = Vec::new();
    let mut results: Vec<Value> = Vec::new();
    let mut values_match = false;

    for entry in payload["queries"].as_array().cloned().unwrap_or_default() {
        let name = entry["name"].as_str().unwrap_or("?").to_string();
        let query = match SearchQuery::from_payload(&entry["query"]) {
            Ok(query) => query,
            Err(error) => {
                failures.push(json!({"name": name, "parse_error": error.message}));
                continue;
            }
        };
        let compiled = match compiler.compile(&query, &accelerator, preimage_available) {
            Ok(compiled) => compiled,
            Err(error) => {
                failures.push(json!({"name": name, "compile_error": error.to_string()}));
                continue;
            }
        };
        let expectation = &entry["expectation"];
        let route = expectation["route"].as_str().unwrap_or("");
        let route_name = match compiled.route {
            Route::Auxiliary => "auxiliary",
            Route::R4Primary => "r4_primary",
            Route::CompletePreimage => "complete_preimage",
            Route::OneWildcardPreimage => "one_wildcard_preimage",
            Route::FullFamily => "full_family",
            Route::R3PrimaryPivot => "r3_primary_pivot",
            Route::PartialEffectFilter => "partial_effect_filter",
        };
        if route != route_name {
            failures.push(json!({
                "name": name, "route_expected": route, "route_actual": route_name,
            }));
            continue;
        }

        let compiled_values = compiled.native.values().to_vec();
        let values_digest = digest_u16(&compiled_values);
        values_match = values_digest == expected_values;
        if !values_match {
            failures.push(json!({
                "name": name, "values_len": compiled_values.len(),
                "values_digest": values_digest,
            }));
        }

        let spec_match = match &compiled.native {
            NativePivotQuery::Auxiliary { spec, .. } => {
                let actual = auxiliary_spec_json(spec);
                let equal = actual == expectation["spec"];
                if !equal {
                    failures.push(json!({
                        "name": name,
                        "spec_mismatch": first_difference(&actual, &expectation["spec"]),
                    }));
                }
                equal
            }
            NativePivotQuery::R4Primary { spec, .. } => {
                let equal = r4_spec_matches(spec, &expectation["spec"]);
                if !equal {
                    failures.push(json!({"name": name, "spec_mismatch": "r4_primary"}));
                }
                equal
            }
            NativePivotQuery::Natural { .. } => false,
            // The effect-preimage route is verified by its own recorded vectors
            // (tests/migration/test_search_worker_parity.py and the worker's
            // effect_path tests), not by this pivot-values probe.
            NativePivotQuery::EffectPreimage { .. } => false,
            // The rarity-3 named-primary pivot is verified by the migration
            // parity gate against the shipped worker's own cursor, not by this
            // pivot-values probe.
            NativePivotQuery::PrimaryPivot { .. } => false,
        };

        let mut window_results = Vec::new();
        for window_json in expectation["windows"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let start = window_json["start"].as_u64().unwrap_or(0);
            let stop = window_json["stop"].as_u64().unwrap_or(0);
            let window = PivotWindow {
                start_index: start,
                stop_index: stop,
                low16_stride: 0x9E37,
                draw_index: 1,
            };
            let expected_matches = pairs(&window_json["matches"]);
            let (actual_matches, stage_ok) = match &compiled.native {
                NativePivotQuery::Auxiliary { spec, .. } => {
                    match accelerator.collect_auxiliary_pivot_page(
                        &compiled_values,
                        spec,
                        window,
                        1_000_000,
                    ) {
                        Ok(page) => {
                            let stage_ok = window_json["stage_counts"]
                                .as_array()
                                .map(|items| {
                                    let expected: Vec<u64> = items
                                        .iter()
                                        .map(|item| item.as_u64().unwrap_or(0))
                                        .collect();
                                    expected == page.stage_counts
                                })
                                .unwrap_or(true);
                            (
                                page.matches
                                    .iter()
                                    .map(|item| [u64::from(item.seed), item.trial])
                                    .collect::<Vec<_>>(),
                                stage_ok,
                            )
                        }
                        Err(error) => {
                            failures.push(json!({
                                "name": name, "start": start, "error": format!("{error:?}"),
                            }));
                            continue;
                        }
                    }
                }
                NativePivotQuery::R4Primary { spec, .. } => {
                    match accelerator.collect_r4_primary_pivot_page(&compiled_values, spec, window)
                    {
                        Ok(matches) => (
                            matches
                                .iter()
                                .map(|item| [u64::from(item.seed), item.trial])
                                .collect::<Vec<_>>(),
                            true,
                        ),
                        Err(error) => {
                            failures.push(json!({
                                "name": name, "start": start, "error": format!("{error:?}"),
                            }));
                            continue;
                        }
                    }
                }
                NativePivotQuery::Natural { .. } => (Vec::new(), true),
                NativePivotQuery::EffectPreimage { .. } => (Vec::new(), true),
                NativePivotQuery::PrimaryPivot { .. } => (Vec::new(), true),
            };
            let matches_ok = actual_matches == expected_matches;
            if !matches_ok || !stage_ok {
                failures.push(json!({
                    "name": name, "start": start, "matches_ok": matches_ok,
                    "stages_ok": stage_ok,
                    "expected_count": expected_matches.len(),
                    "actual_count": actual_matches.len(),
                    "first_expected": expected_matches.first(),
                    "first_actual": actual_matches.first(),
                }));
            }
            window_results.push(json!({
                "start": start, "stop": stop,
                "expected": expected_matches.len(),
                "actual": actual_matches.len(),
                "matches_ok": matches_ok,
                "stages_ok": stage_ok,
            }));
        }

        results.push(json!({
            "name": name,
            "route": route_name,
            "values_len": compiled_values.len(),
            "spec_ok": spec_match,
            "windows": window_results,
        }));
    }

    let report = json!({
        "values_len_expected": payload["values_len"],
        "values_sha256_expected": expected_values,
        "values_match": values_match,
        "queries": results,
        "failures": failures,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
    if failures.is_empty() && values_match {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn pairs(value: &Value) -> Vec<[u64; 2]> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|pair| [pair[0].as_u64().unwrap_or(0), pair[1].as_u64().unwrap_or(0)])
                .collect()
        })
        .unwrap_or_default()
}

fn digest_u16(values: &[u16]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn auxiliary_spec_json(spec: &AuxiliaryPivotSpec) -> Value {
    json!({
        "mode_threshold": spec.mode_threshold,
        "filtered_terrain_rows": spec.filtered_terrain_rows,
        "terrain_row_count": spec.terrain_row_count,
        "allowed_terrain_rows": hex(&spec.allowed_terrain_rows),
        "has_terrain_constraint": spec.has_terrain_constraint,
        "descriptor_thresholds": spec.descriptor_thresholds,
        "selector_threshold": spec.selector_threshold,
        "role_five_threshold": spec.role_five_threshold,
        "selector_value": spec.selector_value,
        "enemy_rows": hex(&spec.enemy_rows),
        "terrains": hex(&spec.terrains),
        "contexts": hex(&spec.contexts),
        "enemy_criterion_groups": spec.enemy_criterion_groups,
        "enemy_group_count": spec.enemy_group_count,
        "scratch_group_count": spec.scratch_group_count,
        "rule_rows": hex(&spec.rule_rows),
        "rule_criterion_groups": spec.rule_criterion_groups,
    })
}

fn r4_spec_matches(spec: &R4PrimaryPivotSpec, expected: &Value) -> bool {
    let context_ok = digest_bytes(&spec.context_by_first_u16)
        == expected["context_by_first_u16_sha256"]
            .as_str()
            .unwrap_or("");
    let normal_ok = digest_bytes(&spec.normal_lookups)
        == expected["normal_lookups_sha256"].as_str().unwrap_or("");
    let promoted_ok = digest_bytes(&spec.promoted_lookups)
        == expected["promoted_lookups_sha256"].as_str().unwrap_or("");
    let promotion_ok = digest_bytes(&spec.promotion_success_lookup)
        == expected["promotion_success_lookup_sha256"]
            .as_str()
            .unwrap_or("");
    let random7_ok = digest_bytes(&spec.random7_lookup)
        == expected["random7_lookup_sha256"].as_str().unwrap_or("");
    let allowed_expected: Vec<u32> = expected["allowed_effect_ids"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_u64().unwrap_or(0) as u32)
                .collect()
        })
        .unwrap_or_default();
    let allowed_ok = spec.allowed_effect_ids == allowed_expected;
    let count_ok = u64::from(spec.context_count) == expected["context_count"].as_u64().unwrap_or(0);
    context_ok && normal_ok && promoted_ok && promotion_ok && random7_ok && allowed_ok && count_ok
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

fn first_difference(actual: &Value, expected: &Value) -> String {
    let (Some(actual_object), Some(expected_object)) = (actual.as_object(), expected.as_object())
    else {
        return "shape".to_string();
    };
    for (key, expected_value) in expected_object {
        match actual_object.get(key) {
            Some(actual_value) if actual_value == expected_value => {}
            Some(actual_value) => {
                return format!(
                    "{key}: expected {} actual {}",
                    summarise(expected_value),
                    summarise(actual_value)
                )
            }
            None => return format!("{key}: missing"),
        }
    }
    "unknown".to_string()
}

fn summarise(value: &Value) -> String {
    let text = value.to_string();
    if text.len() > 60 {
        format!("{}...", &text[..60])
    } else {
        text
    }
}

#[allow(dead_code)]
fn unused(_: Arc<Accelerator>, _: PivotMatch) {}
