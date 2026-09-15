//! Matched-workload benchmark for the protected runtime scan and map routes.
//!
//! The parity gate needs the *same* deterministic oracle on both sides of the
//! comparison, so it can separate host overhead from oracle cost. This example
//! is the Rust half: it reads one JSON spec on stdin, runs one route against the
//! scripted oracle, and prints the elapsed time, the recorded oracle call
//! sequence and the route's own result. The Python half in
//! `tests/migration/test_runtime_scan_performance_parity.py` runs the shipped
//! functions against an oracle with the same behaviour and compares both.
//!
//! The scripted oracle answers every call through the shipped
//! `nioh3_runtime::mutation::oracle::source_record` emitter, which is the same
//! pure record builder the Python stub calls, so the two sides do identical
//! oracle work and the difference that remains is host overhead.
//!
//! Built only with `--features test-fake`; the packaged host never includes it.

use std::io::Read;
use std::time::Instant;

use serde_json::{json, Value};

use nioh3_protected::oracle::scripted::OracleCall;
use nioh3_protected::scan::ScanRequest;
use nioh3_protected::scan_bench_api::{
    run_grace_capture, run_prepare_maps, run_search_route, OracleSpec,
};

fn main() {
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_err() {
        eprintln!("scan_bench: stdin is not readable");
        std::process::exit(2);
    }
    match run(&text) {
        Ok(report) => println!("{report}"),
        Err(error) => {
            println!("{}", json!({"error": error}));
            std::process::exit(1);
        }
    }
}

fn run(text: &str) -> Result<String, String> {
    let spec: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let route = spec
        .get("route")
        .and_then(Value::as_str)
        .ok_or("the spec needs a route")?
        .to_string();
    let oracle_spec = OracleSpec::from_json(&spec)?;
    let request = ScanRequest::from_bench_spec(&spec)?;
    // The caller owns the state root so a cold capture and a later reuse run
    // share one cache directory; a spec may name it explicitly, otherwise the
    // environment the gate exports is used.
    let state_root = match spec.get("state_root").and_then(Value::as_str) {
        Some(value) => std::path::PathBuf::from(value),
        None => std::env::var_os("NIOH3_STATE_ROOT")
            .filter(|value| !value.is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "nioh3-scan-bench-{}-{}",
                    std::process::id(),
                    route
                ))
            }),
    };
    if let Some(clean) = spec.get("clean_state").and_then(Value::as_bool) {
        if clean {
            let _ = std::fs::remove_dir_all(&state_root);
        }
    }
    std::fs::create_dir_all(&state_root).map_err(|error| error.to_string())?;

    let mut oracle = oracle_spec.build();
    let started = Instant::now();
    let result = match route.as_str() {
        "grace_capture" => run_grace_capture(&mut oracle, &request),
        "prepare_maps" => run_prepare_maps(&mut oracle, &state_root, &request),
        _ => run_search_route(&mut oracle, &state_root, &request, &route),
    };
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let calls = describe_calls(&oracle.calls);
    match result {
        Ok(value) => Ok(json!({
            "route": route,
            "elapsed_ms": elapsed_ms,
            "calls": calls,
            "result": value,
        })
        .to_string()),
        Err(error) => Ok(json!({
            "route": route,
            "elapsed_ms": elapsed_ms,
            "calls": calls,
            "error": error.message,
        })
        .to_string()),
    }
}

/// The recorded oracle calls, reduced to the facts both sides can compare.
fn describe_calls(calls: &[OracleCall]) -> Vec<Value> {
    calls
        .iter()
        .map(|call| match call {
            OracleCall::Generate(seeds) => json!({"call": "generate", "seeds": seeds}),
            OracleCall::GenerateSeedRange {
                start_seed,
                seed_step,
                count,
                playthrough,
            } => json!({
                "call": "seed_range",
                "start_seed": start_seed,
                "seed_step": seed_step,
                "count": count,
                "playthrough": playthrough,
            }),
            OracleCall::Finalize(seeds) => json!({"call": "finalize", "seeds": seeds}),
        })
        .collect()
}
