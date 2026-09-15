//! Development-only protected save/runtime worker.
//!
//! Speaks the shipped `protected-request`/`protected-response` frame protocol.
//! The binary refuses to start without an explicit `--dev-protected-only`
//! acknowledgement, which only the development launch path in
//! `apps/tauri/src-tauri/src/worker.rs` passes when `NIOH3_RUST_PROTECTED_WORKER`
//! names this binary. The packaged host keeps the shipped worker and never
//! passes the flag, so this binary cannot become the production worker by
//! accident.

use std::env;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use std::process::ExitCode;

use nioh3_protected::{
    serve, Contract, Role, RoleApplication, RuntimeApplication, SaveApplication,
};
use nioh3_worker::engine::Engine;

const USAGE: &str = "\
usage: nioh3-protected-worker --role <save|runtime> \
--state-root <DIR> --data-root <DIR> --contract-dir <DIR> [--accelerator <DLL>]

Serves the protected save or runtime surface of the Nioh 3 backend over the
shipped framed-JSON protocol. This binary is not selectable as the production
worker by an environment variable; the development selection (the broker sees
NIOH3_RUST_PROTECTED_WORKER set) additionally requires the explicit
--dev-protected-only acknowledgement. --state-root may also be supplied as
NIOH3_STATE_ROOT, which is the shipped broker's own variable.";

#[derive(Debug)]
struct Options {
    role: Role,
    state_root: PathBuf,
    data_root: PathBuf,
    contract_dir: PathBuf,
    accelerator: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match parse_options(&args, development_selected()) {
        Ok(Some(options)) => run(&options),
        Ok(None) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Whether this launch was selected through the development environment.
///
/// The broker only honours `NIOH3_RUST_PROTECTED_WORKER` in a development
/// launch, so seeing it here means an operator explicitly asked for this binary
/// and must also acknowledge the development acknowledgement flag. A packaged
/// broker invokes the shipped `worker/nioh3-protected-worker.exe` directly with
/// just `--role`, and that shape is accepted without the flag.
fn development_selected() -> bool {
    env::var("NIOH3_RUST_PROTECTED_WORKER")
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
}

/// `Ok(None)` means the caller asked for help.
fn parse_options(args: &[String], development_selected: bool) -> Result<Option<Options>, String> {
    let mut dev_only = false;
    let mut role = None;
    let mut state_root = None;
    let mut data_root = None;
    let mut contract_dir = None;
    let mut accelerator = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dev-protected-only" => dev_only = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(None);
            }
            "--role" => {
                index += 1;
                role = Some(match args.get(index).map(String::as_str) {
                    Some("save") => Role::Save,
                    Some("runtime") => Role::Runtime,
                    other => {
                        return Err(format!(
                            "--role must be save or runtime, got {}",
                            other.unwrap_or("<missing>")
                        ))
                    }
                });
            }
            "--state-root" => {
                index += 1;
                state_root = Some(PathBuf::from(
                    args.get(index).ok_or("--state-root needs a value")?,
                ));
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

    if development_selected && !dev_only {
        return Err(
            "refusing to start: this development worker was selected through \
             NIOH3_RUST_PROTECTED_WORKER and requires the explicit \
             --dev-protected-only acknowledgement"
                .to_string(),
        );
    }
    let state_root = state_root.or_else(|| {
        env::var_os("NIOH3_STATE_ROOT")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    });
    Ok(Some(Options {
        role: role.ok_or("--role is required")?,
        state_root: state_root.ok_or("--state-root (or NIOH3_STATE_ROOT) is required")?,
        data_root: data_root.ok_or("--data-root is required")?,
        contract_dir: contract_dir.ok_or("--contract-dir is required")?,
        accelerator: accelerator.or_else(|| {
            env::var_os("NIOH3_SEED_ACCELERATOR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        }),
    }))
}

fn run(options: &Options) -> ExitCode {
    let contract = match Contract::load(&options.contract_dir) {
        Ok(contract) => contract,
        Err(error) => {
            eprintln!("protected worker startup failed: {error}");
            return ExitCode::from(1);
        }
    };
    // The protected host publishes the same generation identity as the shipped
    // worker, so it captures the context through the same loader the read-only
    // worker uses.
    let engine = match Engine::load(
        &options.data_root,
        &options.contract_dir,
        options.accelerator.clone(),
    ) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!(
                "protected worker startup failed: {}: {}",
                error.code, error.message
            );
            return ExitCode::from(1);
        }
    };
    let context = engine.context().clone();
    let application: Box<dyn RoleApplication> = match options.role {
        Role::Save => {
            match SaveApplication::new(options.state_root.clone(), &options.data_root, context) {
                Ok(application) => Box::new(application),
                Err(error) => {
                    eprintln!("protected worker startup failed: {error}");
                    return ExitCode::from(1);
                }
            }
        }
        Role::Runtime => {
            match RuntimeApplication::new(options.state_root.clone(), &options.data_root, context) {
                Ok(application) => Box::new(application),
                Err(error) => {
                    eprintln!("protected worker startup failed: {error}");
                    return ExitCode::from(1);
                }
            }
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut source = BufReader::new(stdin.lock());
    let mut sink = BufWriter::new(stdout.lock());
    match serve(application, &contract, &mut source, &mut sink) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("protected frame fault: {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn development_acknowledgement_is_mandatory() {
        let error = parse_options(
            &args(&[
                "--role",
                "save",
                "--state-root",
                "s",
                "--data-root",
                "d",
                "--contract-dir",
                "c",
            ]),
            true,
        )
        .expect_err("must refuse without the acknowledgement");
        assert!(error.contains("--dev-protected-only"));
    }

    #[test]
    fn required_paths_and_role_are_validated() {
        assert!(parse_options(&args(&["--dev-protected-only"]), false).is_err());
        let options = parse_options(
            &args(&[
                "--dev-protected-only",
                "--role",
                "runtime",
                "--state-root",
                "s",
                "--data-root",
                "d",
                "--contract-dir",
                "c",
            ]),
            true,
        )
        .expect("valid options")
        .expect("not help");
        assert_eq!(options.role, Role::Runtime);
        assert_eq!(options.state_root, PathBuf::from("s"));
        assert_eq!(options.accelerator, None);
    }

    #[test]
    fn the_packaged_invocation_shape_is_accepted_without_the_dev_flag() {
        // The packaged broker invokes the shipped worker with just `--role`.
        let options = parse_options(
            &args(&[
                "--role",
                "save",
                "--state-root",
                "s",
                "--data-root",
                "d",
                "--contract-dir",
                "c",
            ]),
            false,
        )
        .expect("packaged shape is accepted")
        .expect("not help");
        assert_eq!(options.role, Role::Save);
    }

    #[test]
    fn unknown_arguments_are_rejected() {
        assert!(parse_options(&args(&["--dev-protected-only", "--wat"]), false).is_err());
    }
}
