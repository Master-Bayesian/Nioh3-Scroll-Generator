//! Reproduce the ordinary PC202 role and source reads against a retained, known-byte memory image.
use nioh3_runtime::combat_context::{
    progression::read_progression, read_combat_snapshot, SnapshotOutcome,
};
use nioh3_runtime::FixtureMemory;
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let [memory, category, output] = args.as_slice() else {
        return Err("Usage: read_progression_sources MEMORY CATEGORY OUTPUT".into());
    };
    let fixture = FixtureMemory::from_json(&serde_json::from_slice(&fs::read(memory)?)?)?;
    let SnapshotOutcome::Ready(snapshot) = read_combat_snapshot(&fixture)? else {
        return Err("role did not settle".into());
    };
    let sources = read_progression(&fixture, &snapshot, category.parse()?)?;
    let detail = serde_json::json!({"scope":"ordinary read from retained bytes; skill branch uses progress ranks","level":snapshot.inputs.level,
        "form":snapshot.inputs.mode,"digest":sources.digest(),"sources":sources});
    fs::write(PathBuf::from(output), serde_json::to_vec_pretty(&detail)?)?;
    Ok(())
}
