//! Explicit numeric evidence; never promote an output residual into a producer.
use nioh3_loadout::{
    context::ContextManifest,
    engine::Engine,
    pipeline::source_numeric::{Inputs, Seed},
};
use nioh3_runtime::combat_context::CombatInputs;
use serde_json::Value;
#[derive(serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum Evidence {
    Unavailable { reason: String },
    Differs { derived: Seed, observed: [u32; 2] },
    DerivedMatched { derived: Seed, scope: &'static str },
}
pub(super) fn integrate(
    engine: &Engine,
    input: &CombatInputs,
    manifest: &mut ContextManifest,
    metadata: &mut Value,
) -> Result<(), serde_json::Error> {
    let Some(numeric) = &input.source_numeric_inputs else {
        // Legacy omission cannot retain an earlier derived-evidence label.
        if let Some(object) = metadata.as_object_mut() {
            object.remove("numeric_source_evidence");
        }
        return Ok(());
    };
    let observed = [manifest.non_gear.attack_gain, manifest.non_gear.defense];
    let evidence = match engine.source_numeric_seed_inputs(Inputs {
        guardian_records: &numeric.guardian_records,
        active_guardian: numeric.active_guardian,
        guardian_numbers_disabled: numeric.guardian_numbers_disabled,
        core_records: &numeric.core_records,
    }) {
        Err(reason) => Evidence::Unavailable {
            reason: reason.into(),
        },
        Ok(derived) => {
            if [derived.attack_gain, derived.defense] != observed {
                Evidence::Differs { derived, observed }
            } else {
                manifest.non_gear.attack_gain = derived.attack_gain;
                manifest.non_gear.defense = derived.defense;
                Evidence::DerivedMatched {derived,scope:"guardian/core numeric additions under explicit context item caps and supplied PC202 parameters; not ordinary source-map or runtime parameter-lifetime proof"}
            }
        }
    };
    // Serialization covers a closed enum of primitive numbers/strings.
    metadata["numeric_source_evidence"] = serde_json::to_value(evidence)?;
    Ok(())
}
