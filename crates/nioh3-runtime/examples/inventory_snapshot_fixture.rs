//! Runnable fixture driver for the read-only runtime inventory snapshot.
//!
//! This is the bounded end-to-end check for the decode, paging and consistency
//! path: it drives the real request validation, the real chain walk and the real
//! record decoder against a declared memory map instead of a live game, and
//! prints the exact response object. Nothing here opens a process.
//!
//! Usage:
//!
//! ```text
//! inventory_snapshot_fixture --fixture <fixture.json> [--start N] [--limit N] [--out <file>]
//! ```
//!
//! Exit codes: `0` for an observed page, `1` for a typed refusal (the refusal
//! object is printed), `2` for a usage or file error.

use nioh3_runtime::inventory::{snapshot, FixtureMemory, InventoryRequest, DEFAULT_LIMIT};
use serde_json::{json, Value};

enum Failure {
    Usage(String),
    Refused(Value),
    Io(String),
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(()) => std::process::ExitCode::from(0),
        Err(Failure::Refused(value)) => {
            println!("{value}");
            std::process::ExitCode::from(1)
        }
        Err(Failure::Usage(message)) | Err(Failure::Io(message)) => {
            println!("{message}");
            std::process::ExitCode::from(2)
        }
    }
}

fn usage() -> String {
    concat!(
        "usage: inventory_snapshot_fixture --fixture <fixture.json> ",
        "[--start N] [--limit N] [--out <file>]\n",
    )
    .to_string()
}

fn dispatch(args: &[String]) -> Result<(), Failure> {
    let mut fixture: Option<String> = None;
    let mut out: Option<String> = None;
    let mut start: Option<u64> = None;
    let mut limit: Option<u64> = None;
    let mut index = 0usize;
    while index < args.len() {
        let name = args[index].as_str();
        let value = args
            .get(index + 1)
            .ok_or_else(|| Failure::Usage(format!("{name} needs a value\n{}", usage())))?;
        match name {
            "--fixture" => fixture = Some(value.clone()),
            "--out" => out = Some(value.clone()),
            "--start" => {
                start = Some(
                    value
                        .parse::<u64>()
                        .map_err(|error| Failure::Usage(format!("--start: {error}")))?,
                )
            }
            "--limit" => {
                limit = Some(
                    value
                        .parse::<u64>()
                        .map_err(|error| Failure::Usage(format!("--limit: {error}")))?,
                )
            }
            other => {
                return Err(Failure::Usage(format!(
                    "unknown option {other}\n{}",
                    usage()
                )))
            }
        }
        index += 2;
    }
    let fixture = fixture.ok_or_else(|| Failure::Usage(usage()))?;

    let text = std::fs::read_to_string(&fixture)
        .map_err(|error| Failure::Io(format!("fixture {fixture}: {error}")))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| Failure::Io(format!("fixture {fixture} is not JSON: {error}")))?;
    let memory =
        FixtureMemory::from_json(&value).map_err(|error| Failure::Refused(refusal(&error)))?;

    // The same request validation the protected host applies to the wire
    // parameters, so the fixture cannot exercise a shape the host would refuse.
    let params = json!({
        "start": start.unwrap_or(0),
        "limit": limit.unwrap_or(u64::from(DEFAULT_LIMIT)),
    });
    let request =
        InventoryRequest::from_json(&params).map_err(|error| Failure::Refused(refusal(&error)))?;
    let observed =
        snapshot(&memory, &request).map_err(|error| Failure::Refused(refusal(&error)))?;

    let rendered = serde_json::to_string_pretty(&observed)
        .map_err(|error| Failure::Io(format!("cannot render the snapshot: {error}")))?;
    println!("{rendered}");
    if let Some(out) = out {
        std::fs::write(&out, format!("{rendered}\n"))
            .map_err(|error| Failure::Io(format!("cannot write {out}: {error}")))?;
    }
    Ok(())
}

/// The refusal shape, so a refused fixture run is still machine-readable.
fn refusal(error: &nioh3_runtime::RuntimeError) -> Value {
    json!({
        "refused": {
            "code": error.code(),
            "message": error.message(),
        }
    })
}
