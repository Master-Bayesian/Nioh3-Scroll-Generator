//! Development probe: replay the captured auxiliary native windows in Rust.
//!
//! `deliverables/m23b-search/dump_native_spec.py` records the exact arguments
//! the shipped Python worker passes to `collect_auxiliary_pivot_matches` and
//! the DLL page each window produced. This probe rebuilds the same typed spec
//! and asks the Rust binding for the same windows, so the comparison is against
//! retained Python output rather than a reimplementation.
//!
//! Usage:
//!   cargo run --example native_search_probe -- <spec.json> [--allow-cpu]
//!       [--cancel-after-ms <ms>] [--cancel-window <trials>]

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use nioh3_worker::native_search::{Accelerator, AuxiliaryPivotSpec, ExecutionPolicy, PivotWindow};
use serde_json::{json, Value};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let spec_path = args
        .next()
        .unwrap_or_else(|| "deliverables/m23b-search/evidence/native_spec.json".to_string());
    let mut allow_cpu = false;
    let mut cancel_after_ms = 0u64;
    let mut cancel_window = 100_000_000u64;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--allow-cpu" => allow_cpu = true,
            "--cancel-after-ms" => {
                cancel_after_ms = args.next().and_then(|v| v.parse().ok()).unwrap_or(0)
            }
            "--cancel-window" => {
                cancel_window = args.next().and_then(|v| v.parse().ok()).unwrap_or(0)
            }
            other => {
                eprintln!("unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }

    let text = match std::fs::read_to_string(&spec_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cannot read {spec_path}: {error}");
            return ExitCode::from(2);
        }
    };
    let payload: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid spec json: {error}");
            return ExitCode::from(2);
        }
    };

    let values: Vec<u16> = payload["values"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_u64().unwrap_or(0) as u16)
                .collect()
        })
        .unwrap_or_default();
    let spec_json = &payload["spec"];
    let spec = match build_spec(spec_json) {
        Ok(spec) => spec,
        Err(message) => {
            eprintln!("invalid spec: {message}");
            return ExitCode::from(2);
        }
    };

    let application_root = Path::new("../..");
    let override_path = env::var("NIOH3_SEED_ACCELERATOR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let accelerator_path = override_path.clone().unwrap_or_else(|| {
        application_root
            .join("bin")
            .join("nioh3_seed_accelerator.dll")
    });
    let Some(accelerator) = Accelerator::load(application_root, override_path.as_deref()) else {
        eprintln!("accelerator unavailable at {}", accelerator_path.display());
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

    let output_capacity = spec_json["output_capacity"].as_u64().unwrap_or(1_000_000);
    let windows = payload["windows"].as_array().cloned().unwrap_or_default();
    let mut results = Vec::new();
    let mut failures: Vec<Value> = Vec::new();
    let mut backend = String::new();

    for window_json in &windows {
        let start = window_json["start"].as_u64().unwrap_or(0);
        let stop = window_json["stop"].as_u64().unwrap_or(0);
        let expected_matches: Vec<[u64; 2]> = window_json["matches"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|pair| [pair[0].as_u64().unwrap_or(0), pair[1].as_u64().unwrap_or(0)])
                    .collect()
            })
            .unwrap_or_default();
        let expected_stages: Vec<u64> = window_json["stage_counts"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| item.as_u64().unwrap_or(0))
                    .collect()
            })
            .unwrap_or_default();

        let window = PivotWindow {
            start_index: start,
            stop_index: stop,
            low16_stride: spec_json["low16_stride"].as_u64().unwrap_or(0x9E37) as u16,
            draw_index: spec_json["draw_index"].as_u64().unwrap_or(1) as u32,
        };
        let started = Instant::now();
        let page =
            match accelerator.collect_auxiliary_pivot_page(&values, &spec, window, output_capacity)
            {
                Ok(page) => page,
                Err(error) => {
                    failures.push(json!({
                        "start": start, "stop": stop, "error": format!("{error:?}"),
                    }));
                    continue;
                }
            };
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        backend = format!("{:?}", page.backend);

        let actual_matches: Vec<[u64; 2]> = page
            .matches
            .iter()
            .map(|item| [u64::from(item.seed), item.trial])
            .collect();
        let matches_equal = actual_matches == expected_matches;
        let stages_equal = page.stage_counts == expected_stages;
        if !matches_equal || !stages_equal {
            failures.push(json!({
                "start": start,
                "stop": stop,
                "matches_equal": matches_equal,
                "stages_equal": stages_equal,
                "expected_count": expected_matches.len(),
                "actual_count": actual_matches.len(),
                "expected_stages": expected_stages,
                "actual_stages": page.stage_counts,
                "first_expected": expected_matches.first(),
                "first_actual": actual_matches.first(),
            }));
        }
        results.push(json!({
            "start": start,
            "stop": stop,
            "matches": actual_matches.len(),
            "matches_equal": matches_equal,
            "stages_equal": stages_equal,
            "elapsed_ms": elapsed_ms,
        }));
    }

    let mut cancellation = Value::Null;
    if cancel_after_ms > 0 {
        let window = PivotWindow {
            start_index: 0,
            stop_index: cancel_window,
            low16_stride: spec_json["low16_stride"].as_u64().unwrap_or(0x9E37) as u16,
            draw_index: spec_json["draw_index"].as_u64().unwrap_or(1) as u32,
        };
        let deadline = Duration::from_millis(cancel_after_ms);
        let started = Instant::now();
        let cancelled_check = || started.elapsed() >= deadline;
        let mut matches = 0usize;
        let mut chunks = 0u32;
        let mut cursor = 0u64;
        // Mirror the shipped job loop: bounded chunks, cancel between them.
        loop {
            if cancelled_check() {
                break;
            }
            let stop = (cursor + 8_000_000).min(cancel_window);
            let chunk = PivotWindow {
                stop_index: stop,
                ..window
            };
            match accelerator.collect_auxiliary_pivot_page(&values, &spec, chunk, output_capacity) {
                Ok(page) => {
                    matches += page.matches.len();
                    chunks += 1;
                    cursor = stop;
                }
                Err(error) => {
                    failures.push(json!({"cancel_probe_error": format!("{error:?}")}));
                    break;
                }
            }
            if cursor >= cancel_window {
                break;
            }
        }
        cancellation = json!({
            "requested_after_ms": cancel_after_ms,
            "observed_ms": started.elapsed().as_secs_f64() * 1000.0,
            "chunks_completed": chunks,
            "scanned_trials": cursor,
            "matches": matches,
        });
    }

    let report = json!({
        "spec": spec_path,
        "values": values.len(),
        "family_size": values.len() as u64 * 0x1_0000,
        "policy": format!("{policy:?}"),
        "backend": backend,
        "windows": results,
        "failures": failures,
        "cancellation": cancellation,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn build_spec(spec: &Value) -> Result<AuxiliaryPivotSpec, String> {
    let groups_u16: Vec<Vec<u16>> = groups_u32(spec, "rule_criterion_groups")?
        .into_iter()
        .map(|group| group.into_iter().map(|key| key as u16).collect())
        .collect();
    Ok(AuxiliaryPivotSpec {
        draw_index: spec["draw_index"].as_u64().unwrap_or(1) as u32,
        playthrough: spec["playthrough"].as_u64().unwrap_or(3) as u8,
        mode_threshold: spec["mode_threshold"].as_i64().unwrap_or(0) as i32,
        filtered_terrain_rows: spec["filtered_terrain_rows"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| item.as_u64().unwrap_or(0) as u32)
                    .collect()
            })
            .unwrap_or_default(),
        terrain_row_count: spec["terrain_row_count"].as_u64().unwrap_or(0) as u32,
        allowed_terrain_rows: decode_hex(
            spec["allowed_terrain_rows"].as_str().unwrap_or_default(),
        )?,
        has_terrain_constraint: spec["has_terrain_constraint"].as_bool().unwrap_or(false),
        descriptor_thresholds: {
            let items = spec["descriptor_thresholds"]
                .as_array()
                .ok_or("missing descriptor_thresholds")?;
            if items.len() != 3 {
                return Err("descriptor_thresholds must have three entries".to_string());
            }
            [
                items[0].as_i64().unwrap_or(0) as i32,
                items[1].as_i64().unwrap_or(0) as i32,
                items[2].as_i64().unwrap_or(0) as i32,
            ]
        },
        selector_threshold: spec["selector_threshold"].as_i64().unwrap_or(0) as i32,
        role_five_threshold: spec["role_five_threshold"].as_i64().unwrap_or(0) as i32,
        selector_value: spec["selector_value"].as_u64().unwrap_or(0) as u8,
        enemy_rows: decode_hex(spec["enemy_rows"].as_str().unwrap_or_default())?,
        terrains: decode_hex(spec["terrains"].as_str().unwrap_or_default())?,
        contexts: decode_hex(spec["contexts"].as_str().unwrap_or_default())?,
        enemy_criterion_groups: groups_u32(spec, "enemy_criterion_groups")?,
        enemy_group_count: spec["enemy_group_count"].as_u64().unwrap_or(0) as u32,
        scratch_group_count: spec["scratch_group_count"].as_u64().unwrap_or(0) as u32,
        rule_rows: decode_hex(spec["rule_rows"].as_str().unwrap_or_default())?,
        rule_criterion_groups: groups_u16,
    })
}

fn groups_u32(spec: &Value, key: &str) -> Result<Vec<Vec<u32>>, String> {
    let items = spec[key]
        .as_array()
        .ok_or_else(|| format!("missing {key}"))?;
    items
        .iter()
        .map(|group| {
            let inner = group
                .as_array()
                .ok_or_else(|| format!("{key} group is not an array"))?;
            Ok(inner
                .iter()
                .map(|item| item.as_u64().map(|value| value as u32).unwrap_or(0))
                .collect::<Vec<u32>>())
        })
        .collect()
}

fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("hex payload has an odd length".to_string());
    }
    (0..text.len() / 2)
        .map(|index| {
            u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
                .map_err(|error| format!("invalid hex: {error}"))
        })
        .collect()
}
