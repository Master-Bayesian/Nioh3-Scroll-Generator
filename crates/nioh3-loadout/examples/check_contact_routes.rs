//! Compare each paired PC202 contact row with original recipient-branch controls.
use nioh3_loadout::action_rows::{ActionController, CollisionComponent, CollisionRoute};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{env, fs};

#[derive(Deserialize)]
struct Row {
    resource: String,
    action: u16,
    row: usize,
    flags: u16,
    digest: String,
}
#[derive(Deserialize)]
struct Window {
    parameter_sha256: String,
    rows: Vec<Row>,
}
#[derive(Deserialize)]
struct Control {
    flags: u16,
    component: Option<i32>,
    continues: bool,
}
#[derive(Deserialize)]
struct Evidence {
    windows: Vec<Window>,
    native_controls: Vec<Control>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args_os()
        .nth(1)
        .ok_or("expected contact window report")?;
    let evidence: Evidence = serde_json::from_slice(&fs::read(path)?)?;
    let mut decisions = 0usize;
    for w in &evidence.windows {
        for r in &w.rows {
            let action = ActionController::resolve(&r.resource, r.action, &w.parameter_sha256)?;
            let raw = action.rows.get(r.row).ok_or("missing native row")?;
            if format!("{:x}", Sha256::digest(raw)) != r.digest {
                return Err("different row identity".into());
            }
            let mut cases = 0;
            for c in evidence
                .native_controls
                .iter()
                .filter(|c| c.flags == r.flags)
            {
                let state = c
                    .component
                    .map_or(CollisionComponent::Absent, CollisionComponent::ValueAt80);
                let expected = if c.continues {
                    CollisionRoute::Continue
                } else {
                    CollisionRoute::Rejected
                };
                if action.collision_routes(state).nth(r.row) != Some(expected) {
                    return Err("native collision route differs".into());
                }
                cases += 1;
            }
            if cases != 5
                || action
                    .collision_routes(CollisionComponent::Unobserved)
                    .nth(r.row)
                    != Some(CollisionRoute::NeedsComponent)
            {
                return Err("missing native recipient control or unobserved operand".into());
            }
            decisions += cases;
        }
    }
    if decisions == 0 {
        return Err("no collision controls exercised".into());
    }
    println!(
        "Verified {decisions} recipient decisions across {} paired contact windows",
        evidence.windows.len()
    );
    Ok(())
}
