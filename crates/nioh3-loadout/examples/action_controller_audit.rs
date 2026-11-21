//! Cross-check the runtime controller reader against independently exported PC202 action-file records.
use nioh3_loadout::action_rows::ActionController;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs,
    io::{BufRead, BufReader},
};

#[derive(Deserialize)]
struct Expected {
    resource: String,
    action: u16,
    parameter_sha256: String,
    first: u16,
    count: usize,
    ordered_row_sha256: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let input = args.next().ok_or("expected an action-file JSONL export")?;
    let output = args.next().ok_or("expected an output receipt")?;
    let mut groups = BTreeMap::new();
    let mut actions = 0;
    let mut rows = 0;
    for line in BufReader::new(fs::File::open(input)?).lines() {
        let expected: Expected = serde_json::from_str(&line?)?;
        let state = ActionController::resolve(
            &expected.resource,
            expected.action,
            &expected.parameter_sha256,
        )?;
        let mut hash = Sha256::new();
        for bytes in &state.rows {
            hash.update(bytes);
        }
        if state.first_index != expected.first
            || state.rows.len() != expected.count
            || format!("{:x}", hash.finalize()) != expected.ordered_row_sha256
            || format!("{:x}", Sha256::digest(state.parameter)) != expected.parameter_sha256
        {
            return Err(format!(
                "Controller differs for {} action {}",
                expected.resource, expected.action
            )
            .into());
        }
        *groups.entry(expected.resource).or_insert(0) += 1;
        actions += 1;
        rows += state.rows.len();
    }
    if actions == 0 {
        return Err("Empty action corpus".into());
    }
    let report = serde_json::json!({"passed":true,"actions":actions,"row_references":rows,"by_resource":groups});
    fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
