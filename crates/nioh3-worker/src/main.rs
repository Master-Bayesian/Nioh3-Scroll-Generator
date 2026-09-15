//! Development-only read-only preview worker.
//!
//! Speaks the shipped `offline_search` frame protocol but serves only
//! `handshake`, `candidate.preview` and `shutdown`. The binary refuses to start
//! without an explicit `--dev-preview-only` acknowledgement, so the shipped
//! Tauri host, whose fixed spawn command cannot pass that flag, can never
//! select it as a production worker.

use std::env;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use std::process::ExitCode;

use nioh3_worker::engine::{Engine, Outcome};
use nioh3_worker::protocol::{parse_request, request_id};
use nioh3_worker::transport::{read_frame, write_frame};

const USAGE: &str = "\
usage: nioh3-readonly-worker --dev-preview-only --data-root <DIR> --contract-dir <DIR> \
[--accelerator <DLL>]

Serves the read-only preview subset (handshake, candidate.preview, shutdown) of the
Nioh 3 offline search worker. Every other method is rejected. This binary is not
selectable as the production worker and always requires --dev-preview-only.";

#[derive(Debug)]
struct Options {
    data_root: PathBuf,
    contract_dir: PathBuf,
    accelerator: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match parse_options(&args) {
        Ok(Some(options)) => run(&options),
        Ok(None) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// `Ok(None)` means the caller asked for help.
fn parse_options(args: &[String]) -> Result<Option<Options>, String> {
    let mut dev_preview_only = false;
    let mut data_root = None;
    let mut contract_dir = None;
    let mut accelerator = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dev-preview-only" => dev_preview_only = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(None);
            }
            "--data-root" => {
                index += 1;
                data_root = Some(PathBuf::from(
                    args.get(index).ok_or("--data-root needs a value")?,
                ));
            }
            "--contract-dir" => {
                index += 1;
                contract_dir = Some(PathBuf::from(
                    args.get(index).ok_or("--contract-dir needs a value")?,
                ));
            }
            "--accelerator" => {
                index += 1;
                accelerator = Some(PathBuf::from(
                    args.get(index).ok_or("--accelerator needs a value")?,
                ));
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }

    if !dev_preview_only {
        return Err(
            "refusing to start: this development worker requires the explicit \
             --dev-preview-only acknowledgement and is never the production worker"
                .to_string(),
        );
    }
    let data_root = data_root.ok_or("--data-root is required")?;
    let contract_dir = contract_dir.ok_or("--contract-dir is required")?;
    let accelerator = accelerator.or_else(|| {
        env::var_os("NIOH3_SEED_ACCELERATOR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    });
    Ok(Some(Options {
        data_root,
        contract_dir,
        accelerator,
    }))
}

fn run(options: &Options) -> ExitCode {
    let mut engine = match Engine::load(
        &options.data_root,
        &options.contract_dir,
        options.accelerator.clone(),
    ) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!(
                "read-only worker startup failed: {}: {}",
                error.code, error.message
            );
            return ExitCode::from(1);
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut source = BufReader::new(stdin.lock());
    let mut sink = BufWriter::new(stdout.lock());

    loop {
        let payload = match read_frame(&mut source) {
            Ok(Some(payload)) => payload,
            Ok(None) => return ExitCode::SUCCESS,
            Err(error) => {
                // Frame-level faults are fatal in the shipped worker too.
                eprintln!("invalid frame: {error}");
                return ExitCode::from(1);
            }
        };
        let id = request_id(&payload);
        let outcome = match parse_request(&payload) {
            Ok(request) => engine.dispatch(request),
            Err(error) => Outcome::Reply(engine.error_reply(&id, &error)),
        };
        let (frame, stop) = match outcome {
            Outcome::Reply(frame) => (frame, false),
            Outcome::Stop(frame) => (frame, true),
        };
        if let Err(error) = write_frame(&mut sink, &frame) {
            eprintln!("cannot write response frame: {error}");
            return ExitCode::from(1);
        }
        if stop {
            return ExitCode::SUCCESS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn development_acknowledgement_is_mandatory() {
        let error = parse_options(&args(&["--data-root", "d", "--contract-dir", "c"]))
            .expect_err("must refuse without the acknowledgement");
        assert!(error.contains("--dev-preview-only"));
    }

    #[test]
    fn required_paths_are_validated() {
        assert!(parse_options(&args(&["--dev-preview-only"])).is_err());
        let options = parse_options(&args(&[
            "--dev-preview-only",
            "--data-root",
            "data",
            "--contract-dir",
            "contracts",
        ]))
        .expect("valid options")
        .expect("not help");
        assert_eq!(options.data_root, PathBuf::from("data"));
        assert_eq!(options.contract_dir, PathBuf::from("contracts"));
        assert_eq!(options.accelerator, None);
    }

    #[test]
    fn unknown_arguments_are_rejected() {
        assert!(parse_options(&args(&["--dev-preview-only", "--wat"])).is_err());
    }
}
