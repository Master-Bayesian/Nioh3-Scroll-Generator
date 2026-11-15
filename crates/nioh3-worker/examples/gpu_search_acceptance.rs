//! Repeatable cross-backend acceptance over current PC v2.02 product tables.
//! Run through tools/verify_gpu_search_e2e.py; no game or save writes occur.
use nioh3_worker::engine::Materializer;
use nioh3_worker::native_search::{Accelerator, ExecutionPolicy, PivotWindow};
use nioh3_worker::query_compile::QueryCompiler;
use nioh3_worker::{CandidateSource, NativePivotQuery, PageRequest, SearchBackend, SearchQuery};
use serde_json::{json, Value};
use std::{env, fs, path::PathBuf, sync::Arc, time::Instant};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn query(
    compiler: &QueryCompiler,
    source: &Materializer,
    accelerator: &Accelerator,
    payload: Value,
    preimage: bool,
) -> Result<nioh3_worker::query_compile::CompiledQuery, String> {
    let mut query = SearchQuery::from_payload(&payload).map_err(|e| format!("query: {e:?}"))?;
    source
        .resolve_query(&mut query)
        .map_err(|e| format!("resolve: {e:?}"))?;
    compiler
        .compile(&query, accelerator, preimage)
        .map_err(|e| e.to_string())
}
fn payload(rarity: u8) -> Value {
    json!({"playthrough":3,"rarity":rarity,"level":180,"primary_effect_ids":[],"required_secondary_ids":[],"auxiliary":{"required_special_rule_key_groups":[[18464],[113],[32497]],"required_enemy_lookup_key_groups":[[264763],[586715]]},"terrain_selection_ids":["exact:"]})
}
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("directcompute");
    let output = args.get(2).ok_or("expected backend and output JSON path")?;
    if !matches!(mode, "cuda" | "directcompute" | "cpu" | "auto-fallback") {
        return Err("unsupported acceptance backend".into());
    }
    env::set_var(
        "NIOH3_SEED_BACKEND",
        match mode {
            "cpu" => "cuda",
            "auto-fallback" => "auto",
            other => other,
        },
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let data = root.join("data");
    let version = Some(nioh3_data::CURRENT_RESOURCE_VERSION);
    let source = Materializer::with_resource_version(&data, "gpu-seed-acceptance", version);
    let compiler =
        QueryCompiler::load_for_resource_version(&data, version).map_err(|e| e.to_string())?;
    let accelerator =
        Arc::new(Accelerator::load(&root, None).ok_or("seed accelerator load failed")?);
    if matches!(mode, "cpu" | "auto-fallback") {
        accelerator.force_cuda_failure(true);
    }
    let _policy = accelerator
        .pin_policy(if mode == "cpu" {
            ExecutionPolicy::AllowBulkCpu
        } else {
            ExecutionPolicy::StrictGpu
        })
        .map_err(|e| format!("policy: {e:?}"))?;
    let capabilities = accelerator.capabilities();
    if mode == "cuda" && !capabilities.cuda_seed_acceleration {
        return Err("CUDA is unavailable for this acceptance run".into());
    }
    if mode != "cpu" && mode != "cuda" && !capabilities.directcompute_seed_acceleration {
        return Err("DirectCompute seed self-test failed".into());
    }
    let feedback = query(&compiler, &source, &accelerator, payload(4), true)?;
    let NativePivotQuery::Auxiliary { values, spec } = feedback.native else {
        return Err("feedback did not compile to auxiliary route".into());
    };
    let windows = [(0, 131072), (16777000, 16908072), (0xFFFF0000, 0x100000000)];
    let mut checks = Vec::new();
    for (start, stop) in windows {
        let window = PivotWindow {
            start_index: start,
            stop_index: stop,
            low16_stride: 0x9E37,
            draw_index: 1,
        };
        let started = Instant::now();
        let found = accelerator
            .collect_natural_pivot_page(&values, window)
            .map_err(|e| format!("natural: {e:?}"))?;
        checks.push(json!({"name":format!("natural-{start}"),"pairs":found.iter().map(|m|[u64::from(m.seed),m.trial]).collect::<Vec<_>>(),"backend":format!("{:?}",accelerator.last_backend()),"elapsed_ms":started.elapsed().as_secs_f64()*1000.0}));
        let started = Instant::now();
        let page = accelerator
            .collect_auxiliary_pivot_page(&values, &spec, window, 1000000)
            .map_err(|e| format!("auxiliary: {e:?}"))?;
        checks.push(json!({"name":format!("feedback-{start}"),"pairs":page.matches.iter().map(|m|[u64::from(m.seed),m.trial]).collect::<Vec<_>>(),"stages":page.stage_counts,"backend":format!("{:?}",page.backend),"elapsed_ms":started.elapsed().as_secs_f64()*1000.0}));
    }
    let seeds: Vec<u32> = (0..8192u32)
        .map(|i| ((i.wrapping_mul(0x13579B).wrapping_add(179474327)) & 0x0FFFFFFF) | 1)
        .collect();
    for key in [18464u16, 113, 32497] {
        let mut rules = spec.clone();
        rules.has_terrain_constraint = false;
        rules.allowed_terrain_rows.fill(1);
        rules.enemy_criterion_groups =
            rules.enemy_criterion_groups[rules.enemy_group_count as usize..].to_vec();
        rules.enemy_group_count = 0;
        rules.rule_criterion_groups = vec![vec![key]];
        let started = Instant::now();
        let selected = accelerator
            .auxiliary_criteria_selected(&rules, &seeds)
            .map_err(|e| format!("rule {key}: {e:?}"))?;
        checks.push(json!({"name":format!("rule-predicate-{key}"),"selected":selected,"backend":format!("{:?}",accelerator.last_backend()),"elapsed_ms":started.elapsed().as_secs_f64()*1000.0}));
    }
    let mut p = payload(4);
    p["primary_effect_ids"] = json!([30543]);
    p["auxiliary"] = json!({});
    p["terrain_selection_ids"] = json!([]);
    let primary = query(&compiler, &source, &accelerator, p, true)?;
    let NativePivotQuery::R4Primary {
        values: primary_values,
        spec: primary_spec,
    } = primary.native
    else {
        return Err("R4 did not compile to its primary pivot".into());
    };
    let window = PivotWindow {
        start_index: 0,
        stop_index: 1000000,
        low16_stride: 0x9E37,
        draw_index: 1,
    };
    let started = Instant::now();
    let found = accelerator
        .collect_r4_primary_pivot_page(&primary_values, &primary_spec, window)
        .map_err(|e| format!("R4: {e:?}"))?;
    checks.push(json!({"name":"r4-primary","pairs":found.iter().map(|m|[u64::from(m.seed),m.trial]).collect::<Vec<_>>(),"backend":format!("{:?}",accelerator.last_backend()),"elapsed_ms":started.elapsed().as_secs_f64()*1000.0}));
    let mut p = payload(3);
    p["primary_effect_ids"] = json!([30543]);
    p["auxiliary"] = json!({});
    p["terrain_selection_ids"] = json!([]);
    let primary = query(&compiler, &source, &accelerator, p, false)?;
    let filter = primary
        .page_filter
        .and_then(|p| p.primary)
        .ok_or("R3 primary predicate missing")?;
    let selected = accelerator
        .primary_effect_selected(&filter, &seeds)
        .map_err(|e| format!("R3: {e:?}"))?;
    checks.push(json!({"name":"r3-primary-predicate","selected":selected,"backend":format!("{:?}",accelerator.last_backend())}));
    let mut terrain = spec.clone();
    terrain.enemy_criterion_groups.clear();
    terrain.enemy_group_count = 0;
    terrain.scratch_group_count = 0;
    terrain.rule_criterion_groups.clear();
    terrain.rule_rows.clear();
    let backend = SearchBackend::new(
        Some(Arc::clone(&accelerator)),
        Err(nioh3_worker::PreimageError::Absent(
            "not used by this auxiliary acceptance".to_string(),
        )),
    );
    let pivot = NativePivotQuery::Auxiliary {
        values: values.clone(),
        spec: terrain,
    };
    let first = backend
        .collect_page(&pivot, &PageRequest::chunk(0, 1000000, 1000000, 3), &|| {
            false
        })
        .map_err(|e| format!("first page: {e:?}"))?;
    let second = backend
        .collect_page(
            &pivot,
            &PageRequest::chunk(first.next_cursor, 1000000, 1000000, 3),
            &|| false,
        )
        .map_err(|e| format!("resume page: {e:?}"))?;
    for (name, page) in [("page-first", first), ("page-resumed", second)] {
        checks.push(json!({"name":name,"pairs":page.matches.iter().map(|m|[u64::from(m.seed),m.trial]).collect::<Vec<_>>(),"stages":page.stage_counts,"cursor":page.next_cursor,"backend":format!("{:?}",page.backend)}));
    }
    let cancel_flag = std::sync::atomic::AtomicBool::new(false);
    let feedback_pivot = NativePivotQuery::Auxiliary {
        values: values.clone(),
        spec: spec.clone(),
    };
    let cancelled = backend
        .collect_page_with_progress(
            &feedback_pivot,
            &PageRequest::chunk(0, 262144, 131072, 1000000),
            &|| cancel_flag.load(std::sync::atomic::Ordering::Relaxed),
            &mut |_| cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed),
        )
        .map_err(|e| format!("cancelled page: {e:?}"))?;
    if !cancelled.cancelled || cancelled.next_cursor != 131072 {
        return Err("cancellation did not retain the decided exclusive prefix".into());
    }
    let resumed = backend
        .collect_page(
            &feedback_pivot,
            &PageRequest::chunk(cancelled.next_cursor, 131072, 131072, 1000000),
            &|| false,
        )
        .map_err(|e| format!("cancel resume: {e:?}"))?;
    let whole = backend
        .collect_page(
            &feedback_pivot,
            &PageRequest::chunk(0, 262144, 131072, 1000000),
            &|| false,
        )
        .map_err(|e| format!("uninterrupted page: {e:?}"))?;
    let joined: Vec<_> = cancelled
        .matches
        .iter()
        .chain(&resumed.matches)
        .copied()
        .collect();
    let joined_stages: Vec<_> = cancelled
        .stage_counts
        .iter()
        .zip(&resumed.stage_counts)
        .map(|(a, b)| a + b)
        .collect();
    if joined != whole.matches || joined_stages != whole.stage_counts {
        return Err("cancel/resume changed the candidate stream or prefix counts".into());
    }
    for (name, page) in [
        ("cancelled-prefix", cancelled),
        ("after-cancellation", resumed),
        ("uninterrupted", whole),
    ] {
        checks.push(json!({"name":name,"pairs":page.matches.iter().map(|m|[u64::from(m.seed),m.trial]).collect::<Vec<_>>(),"stages":page.stage_counts,"cursor":page.next_cursor,"cancelled":page.cancelled,"backend":format!("{:?}",page.backend)}));
    }
    let NativePivotQuery::Auxiliary {
        spec: terrain_spec, ..
    } = &pivot
    else {
        unreachable!("terrain fixture is auxiliary");
    };
    let overflow = accelerator.collect_auxiliary_pivot_page(
        &values,
        terrain_spec,
        PivotWindow {
            start_index: 0,
            stop_index: 131072,
            low16_stride: 0x9E37,
            draw_index: 1,
        },
        1,
    );
    let overflow_error = match overflow {
        Err(error) => format!("{error:?}"),
        Ok(_) => return Err("output overflow was silently truncated".into()),
    };
    let bulk_cpu_calls = accelerator.bulk_cpu_call_count();
    if mode != "cpu" && bulk_cpu_calls != 0 {
        return Err(format!("strict GPU leaked {bulk_cpu_calls} bulk CPU calls"));
    }
    let expected = match mode {
        "cpu" => "NativeCpu",
        "cuda" => "Cuda",
        _ => "DirectCompute",
    };
    if checks.iter().any(|c| c["backend"] != expected) {
        return Err(format!("an operation did not use {expected}"));
    }
    let report = json!({"schema":"nioh3-gpu-search-acceptance/v1","backend":mode,"vendor":env::var("NIOH3_D3D11_VENDOR").unwrap_or_default(),"resource_version":version,"bulk_cpu_calls":bulk_cpu_calls,"overflow_refused":true,"overflow_error":overflow_error,"checks":checks});
    fs::write(
        output,
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!(
        "GPU_ACCEPTANCE_OK backend={mode} checks={} bulk_cpu_calls={bulk_cpu_calls}",
        report["checks"].as_array().unwrap().len()
    );
    Ok(())
}
