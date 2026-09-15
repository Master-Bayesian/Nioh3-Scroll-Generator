//! Development probe: dump the auxiliary composition the worker consumes.
//!
//! Usage: `cargo run --example auxiliary_probe -- <data_root> <seed>...`

use std::collections::BTreeSet;
use std::env;
use std::path::Path;

use nioh3_data::load_preview_resources;
use nioh3_domain::preview::{compose_ng3_preview, AuxiliaryPreview, PreviewTables};

fn main() {
    let mut args = env::args().skip(1);
    let data_root = args
        .next()
        .unwrap_or_else(|| "../../nioh3_scroll_editor/data".to_string());
    let seeds: Vec<u32> = args.map(|value| value.parse().expect("seed")).collect();
    let resources = load_preview_resources(Path::new(&data_root)).expect("preview resources");
    let tables = PreviewTables {
        roster: &resources.roster,
        context: &resources.context,
        rules: &resources.rules,
        states: &resources.states,
    };

    for seed in seeds {
        let composition = compose_ng3_preview(seed, 3, &tables).expect("composition");
        let auxiliary: &AuxiliaryPreview = &composition.auxiliary;
        let scratch: BTreeSet<u16> = auxiliary
            .enemy_groups
            .iter()
            .flat_map(|group| group.entries.iter())
            .map(|entry| entry.scratch_rule_key)
            .filter(|key| *key != 0xFFFF)
            .collect();
        let keys: Vec<String> = auxiliary
            .special_rules
            .keys
            .iter()
            .map(|key| format!("0x{key:04X}"))
            .collect();
        println!(
            "seed={seed} scratch={scratch:?} target_budget={} draws={} keys={keys:?}",
            auxiliary.special_rules.target_budget, auxiliary.special_rules.random_draws
        );
    }
}
