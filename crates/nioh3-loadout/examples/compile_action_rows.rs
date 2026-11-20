//! Compile the ordered, landed templates of an explicit action scenario.
//! Reads one JSON array of resource-qualified row references from stdin.
use nioh3_loadout::action_rows::{ActionRow, DamageRowProgram};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let references: Vec<ActionRow> = serde_json::from_reader(std::io::stdin().lock())?;
    let rows = references
        .iter()
        .map(ActionRow::resolve)
        .collect::<Result<Vec<_>, _>>()?;
    let program = DamageRowProgram::compile(&rows)?;
    serde_json::to_writer(
        std::io::stdout().lock(),
        &json!({
            "schema":"nioh3-pc202-damage-row-program/v1",
            "initial_bytes":program.initial.as_slice(),
            "word_offsets":program.offsets,
            "template_words":program.words
        }),
    )?;
    Ok(())
}
