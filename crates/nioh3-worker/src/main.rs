//! Read-only search worker with an explicit launch-mode acknowledgement.
//!
//! Speaks the shipped `offline_search` frame protocol. The binary refuses to
//! start without exactly one explicit launch-mode acknowledgement:
//! `--dev-preview-only` for a development launch (the shape
//! `apps/tauri/src-tauri/src/worker.rs` passes when `NIOH3_RUST_SEARCH_WORKER`
//! names this binary, and the shape every migration gate uses) or
//! `--packaged-worker` for a packaged launch, which the staged runtime's
//! manifest records. Nothing here makes the binary production by accident: the
//! default packaged host still spawns the shipped worker, and a launch with no
//! acknowledgement, both acknowledgements, or the development environment
//! variable without the development flag fails closed with a named error.

use std::env;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use std::process::ExitCode;

use nioh3_worker::engine::{Engine, Outcome};
use nioh3_worker::protocol::{parse_request, request_id};
use nioh3_worker::transport::{read_frame, write_frame};

const USAGE: &str = "\
usage: nioh3-readonly-worker (--dev-preview-only | --packaged-worker) \
--data-root <DIR> --contract-dir <DIR> [--accelerator <DLL>]

Serves the offline search surface of the Nioh 3 search worker (handshake,
search.catalog, recommended_level.resolve, cache.register, candidate.preview,
search.start, job.current, job.snapshot, job.cancel, candidate.export and
shutdown). Anything outside the shipped request contract is rejected.

Launch modes: --dev-preview-only marks a development launch; --packaged-worker
marks a launch from a staged packaged runtime. Exactly one is required.";

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
    let mut packaged_worker = false;
    let mut data_root = None;
    let mut contract_dir = None;
    let mut accelerator = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dev-preview-only" => dev_preview_only = true,
            "--packaged-worker" => packaged_worker = true,
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

    // Exactly one launch-mode acknowledgement, and the development environment
    // variable always forces the development acknowledgement so a stray
    // variable cannot silently select the packaged mode.
    let dev_selected = env::var_os("NIOH3_RUST_SEARCH_WORKER")
        .filter(|value| !value.is_empty())
        .is_some();
    match (dev_preview_only, packaged_worker, dev_selected) {
        (true, true, _) => {
            return Err("refusing to start: pass either --dev-preview-only or \
                 --packaged-worker, not both"
                .to_string())
        }
        (false, false, _) => {
            return Err("refusing to start: an explicit launch mode is required \
                 (--dev-preview-only for a development launch, --packaged-worker \
                 for a staged packaged runtime)"
                .to_string())
        }
        (false, true, true) => {
            return Err(
                "refusing to start: NIOH3_RUST_SEARCH_WORKER selects a development \
                 launch, which requires --dev-preview-only"
                    .to_string(),
            )
        }
        _ => {}
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
        let outcome = match parse_request(&payload, engine.schema()) {
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
        assert!(error.contains("explicit launch mode is required"));
    }

    /// The packaged mode is explicit, exclusive and unaffected by the
    /// development environment variable.
    #[test]
    fn packaged_launch_mode_is_explicit_and_exclusive() {
        let packaged = parse_options(&args(&[
            "--packaged-worker",
            "--data-root",
            "data",
            "--contract-dir",
            "contracts",
        ]))
        .expect("valid packaged options")
        .expect("not help");
        assert_eq!(packaged.data_root, PathBuf::from("data"));
        let both = parse_options(&args(&[
            "--dev-preview-only",
            "--packaged-worker",
            "--data-root",
            "data",
            "--contract-dir",
            "contracts",
        ]))
        .expect_err("both launch modes must be refused");
        assert!(both.contains("not both"));
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
