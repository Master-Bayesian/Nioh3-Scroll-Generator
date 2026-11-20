//! Replay or sample the exact first-use inputs in a fresh private directory.
use std::{env, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let (data, save, out, fixture) = match args.as_slice() {
        [data, save, out] => (data, save, out, None),
        [data, save, out, flag, path] if flag == "--fixture" => {
            (data, save, out, Some(Path::new(path)))
        }
        _ => {
            return Err(
                "Usage: first_use_export DATA_ROOT SAVE_COPY NEW_OUTPUT [--fixture MEMORY]".into(),
            )
        }
    };
    nioh3_protected::first_use_acceptance::export(
        Path::new(data),
        Path::new(save),
        Path::new(out),
        fixture,
    )?;
    Ok(())
}
