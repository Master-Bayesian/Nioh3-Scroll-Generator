//! Replay or sample the exact first-use inputs in a fresh private directory.
use std::{env, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let [data, save, out, options @ ..] = args.as_slice() else {
        return Err("Usage: first_use_export DATA_ROOT SAVE_COPY NEW_OUTPUT [--fixture MEMORY] [--scene ID] [--reference-target selected] [--action-facts record]".into());
    };
    let mut fixture = None;
    let mut scene = None;
    let mut reference_target = false;
    let mut action_facts = false;
    let (pairs, remainder) = options.as_chunks::<2>();
    for pair in pairs {
        match pair[0].to_str() {
            Some("--fixture") if fixture.is_none() => fixture = Some(Path::new(&pair[1])),
            Some("--reference-target") if !reference_target && pair[1] == "selected" => {
                reference_target = true;
            }
            Some("--action-facts") if !action_facts && pair[1] == "record" => action_facts = true,
            Some("--scene") if scene.is_none() => {
                scene = Some(pair[1].to_str().ok_or("invalid scene ID")?)
            }
            _ => return Err("Unknown or repeated first-use option".into()),
        }
    }
    if !remainder.is_empty() {
        return Err("Missing option value".into());
    }
    nioh3_protected::first_use_acceptance::export_with_controls(
        Path::new(data),
        Path::new(save),
        Path::new(out),
        fixture,
        scene.unwrap_or("kusarigama-strong"),
        reference_target,
        action_facts,
    )?;
    Ok(())
}
