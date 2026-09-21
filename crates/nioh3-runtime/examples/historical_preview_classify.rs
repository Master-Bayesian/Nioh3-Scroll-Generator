//! Minimal dev probe for the one-time historical preview classification.
//!
//! Dry run by default: it verifies the authorization and the raw evidence and
//! prints the exact record a classification would persist, writing nothing. With
//! `--apply` it persists that record beside the receipt. The original receipt is
//! never written either way.
//!
//! ```text
//! historical_preview_classify --state-root <dir> --authorization <json>
//!     --inventory-before <json> --inventory-after <json>
//!     --runner-report <json> --runner-request <json> --runner-stdout <txt>
//!     [--apply]
//! ```

use nioh3_runtime::mutation::historical_preview::{
    classify_historical_preview, parse_authorization, read_classification,
    verify_historical_preview, HistoricalPreviewEvidencePaths,
};
use nioh3_runtime::mutation::native_executor::ReceiptStore;
use std::path::PathBuf;
use std::process::ExitCode;

struct Options {
    state_root: PathBuf,
    authorization: PathBuf,
    paths: HistoricalPreviewEvidencePaths,
    apply: bool,
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: historical_preview_classify --state-root <dir> --authorization <json> \
         --inventory-before <json> --inventory-after <json> --runner-report <json> \
         --runner-request <json> --runner-stdout <txt> [--apply]"
    );
    ExitCode::from(2)
}

fn parse_options() -> Result<Options, ExitCode> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut state_root = None;
    let mut authorization = None;
    let mut inventory_before = None;
    let mut inventory_after = None;
    let mut runner_report = None;
    let mut runner_request = None;
    let mut runner_stdout = None;
    let mut apply = false;
    let mut index = 0;
    while index < arguments.len() {
        let value = arguments.get(index + 1).map(PathBuf::from);
        match arguments[index].as_str() {
            "--state-root" => {
                state_root = value;
                index += 1;
            }
            "--authorization" => {
                authorization = value;
                index += 1;
            }
            "--inventory-before" => {
                inventory_before = value;
                index += 1;
            }
            "--inventory-after" => {
                inventory_after = value;
                index += 1;
            }
            "--runner-report" => {
                runner_report = value;
                index += 1;
            }
            "--runner-request" => {
                runner_request = value;
                index += 1;
            }
            "--runner-stdout" => {
                runner_stdout = value;
                index += 1;
            }
            "--apply" => apply = true,
            _ => return Err(usage()),
        }
        index += 1;
    }
    match (
        state_root,
        authorization,
        inventory_before,
        inventory_after,
        runner_report,
        runner_request,
        runner_stdout,
    ) {
        (
            Some(state_root),
            Some(authorization),
            Some(inventory_before),
            Some(inventory_after),
            Some(runner_report),
            Some(runner_request),
            Some(runner_stdout),
        ) => Ok(Options {
            state_root,
            authorization,
            paths: HistoricalPreviewEvidencePaths {
                inventory_before,
                inventory_after,
                runner_report,
                runner_request,
                runner_stdout,
            },
            apply,
        }),
        _ => Err(usage()),
    }
}

fn run() -> Result<(), String> {
    let options = parse_options().map_err(|_| "invalid arguments".to_string())?;
    let store_directory = options
        .state_root
        .join("live-add")
        .join("native-executor");
    let store = ReceiptStore::new(&store_directory).map_err(|error| error.message())?;
    let authorization_bytes =
        std::fs::read(&options.authorization).map_err(|error| error.to_string())?;
    let authorization_value: serde_json::Value =
        serde_json::from_slice(&authorization_bytes).map_err(|error| error.to_string())?;
    let authorization =
        parse_authorization(&authorization_value).map_err(|error| error.message())?;
    let operation_id = authorization.operation_id.clone();
    // Read-only authoritative status: the same reading admission uses, so the
    // operator sees whether this operation still owns the target before and
    // after the decision.
    match store.read(&operation_id) {
        Ok(receipt) => match store.authoritative_state(&receipt) {
            Ok(Some(state)) => println!(
                "authoritative\t{}",
                state.get("state").and_then(serde_json::Value::as_str).unwrap_or("native_settled")
            ),
            Ok(None) => println!("authoritative\topen"),
            Err(error) => println!("authoritative\terror\t{}", error.message()),
        },
        Err(error) => println!("authoritative\tabsent\t{}", error.message()),
    }
    match store.unresolved_owner() {
        Ok(Some(owner)) => println!("admission\tblocked\t{owner}"),
        Ok(None) => println!("admission\topen"),
        Err(error) => println!("admission\terror\t{}", error.message()),
    }
    if let Some(existing) =
        read_classification(&store, &operation_id).map_err(|error| error.message())?
    {
        println!(
            "existing\t{}",
            serde_json::to_string(&existing).map_err(|error| error.to_string())?
        );
    }
    let record = if options.apply {
        classify_historical_preview(&store, &authorization, &options.paths)
            .map_err(|error| error.message())?
    } else {
        verify_historical_preview(&store, &authorization, &options.paths)
            .map_err(|error| error.message())?
    };
    println!(
        "{}",
        serde_json::to_string(&record).map_err(|error| error.to_string())?
    );
    println!(
        "outcome\t{}",
        if options.apply { "classified" } else { "verified" }
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(detail) => {
            eprintln!("refused\t{detail}");
            ExitCode::FAILURE
        }
    }
}
