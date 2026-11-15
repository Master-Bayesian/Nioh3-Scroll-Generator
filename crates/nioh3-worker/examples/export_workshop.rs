//! Rebuild the workshop data files from the shipped tables.
//!
//! `cargo run --release --features workshop-export --example export_workshop -- [--check] [root]`
//!
//! Writes `apps/workshop/{catalog,editor-values,ui-locales}.json`; with
//! `--check` it only reports whether the committed files are current.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut check = false;
    let mut root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--check" => check = true,
            other => root = PathBuf::from(other),
        }
    }
    let files = match nioh3_worker::workshop::export(&root) {
        Ok(files) => files,
        Err(error) => {
            eprintln!("export failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let workshop = root.join("apps/workshop");
    let mut stale = Vec::new();
    for (name, text) in [
        ("catalog.json", &files.catalog),
        ("editor-values.json", &files.editor_values),
        ("ui-locales.json", &files.ui_locales),
    ] {
        let path = workshop.join(name);
        let current = std::fs::read(&path).unwrap_or_default();
        if current == text.as_bytes() {
            continue;
        }
        if check {
            stale.push(name);
        } else if let Err(error) = std::fs::write(&path, text) {
            eprintln!("cannot write {}: {error}", path.display());
            return ExitCode::FAILURE;
        } else {
            println!("updated {name}");
        }
    }
    if !stale.is_empty() {
        eprintln!(
            "stale workshop data: {}; run the export_workshop example",
            stale.join(", ")
        );
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
