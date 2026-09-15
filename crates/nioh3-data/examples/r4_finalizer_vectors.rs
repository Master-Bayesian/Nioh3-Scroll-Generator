//! Development parity emitter for the M2.2 NG3 rarity-4 finalizer slice.
//!
//! Loads the shipped resource through the real product adapter
//! (`nioh3_data::load_effect_resource`), materializes the native stage-one
//! insertion record and the finalized preview through the domain record/rarity
//! pair API, and prints deterministic tab-separated rows.
//!
//! `tests/migration/test_r4_finalizer_parity.py` reproduces every row through
//! the retained Python reference and independently compares the tracked native
//! corpus bytes (`test_fixtures/r4_native_corpus`). This example is not a
//! product CLI.
//!
//! Usage: `cargo run --example r4_finalizer_vectors -- [data_root] [corpus_root]`

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use nioh3_data::load_effect_resource;
use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::r4_finalizer::{FinalizerAttemptTrace, R4FinalizerEngine, R4FinalizerError};
use nioh3_domain::record::{
    materialize_ng3_rarity4_final_record, materialize_ng3_rarity4_stage_one_record,
    ScrollRecordBytes,
};

/// Level used by the fixed seed sweep; the reference default is 180.
const LEVEL: u16 = 180;
/// Fixed lineage fields shared by the non-fixture sweeps.
const SWEEP_RECOMMENDED_LEVEL: u16 = 183;
const SWEEP_GENERATION_SERIAL: u32 = 3_283_000;
const SWEEP_TRANSFER_COUNT: u32 = 0;
/// Broader sweep size; both sides derive the same multiplicative sequence.
const SWEEP_SEEDS: u32 = 48;
const SWEEP_MULTIPLIER: u32 = 2_654_435_761;
/// Levels swept by the level block.
const LEVEL_SWEEP: [u16; 9] = [1, 30, 90, 150, 180, 300, 500, 700, 65_535];
/// Level-sweep stride indices used to derive extra seeds.
///
/// Index 1 is deliberate: seed `2_654_435_761` is level-sensitive in the
/// completion path (a slot's resolved value crosses the `value != 1` prior-row
/// eligibility test), so the level block is not vacuous for that branch.
const LEVEL_SWEEP_STRIDES: [u32; 3] = [1, 5, 97];
/// Seeds used by the `reveal` block, chosen to span both auxiliary-mode
/// branches (`0x3C`/`0x3E` for reveal, `0x3B`/`0x3D` otherwise) and both
/// changed and unchanged completion outcomes.
const REVEAL_SEEDS: [u32; 6] = [
    1,
    2_965,
    2_654_435_762,
    2_802_362_287,
    3_189_639_204,
    774_553_835,
];
/// Native corpora, relative to the corpus root, in emit order.
const CORPUS_DIRS: [&str; 2] = ["base", "distributed"];

/// Fixed edge and known seeds, in the order both sides emit them.
fn seed_sweep() -> Vec<u32> {
    let mut seeds = vec![
        0,
        1,
        2,
        2_965,
        240_348_265,
        6_096_970,
        74_063_692,
        82_212_268,
        183_696_634,
        241_719_428,
        0x7FFF_FFFF,
        0x8000_0000,
        0xFFFF_FFFE,
        0xFFFF_FFFF,
    ];
    for index in 0..SWEEP_SEEDS {
        seeds.push(index.wrapping_mul(SWEEP_MULTIPLIER));
    }
    dedupe(seeds)
}

/// Fixed edge seeds plus two stride seeds for the level sweep.
fn level_seed_sweep() -> Vec<u32> {
    let mut seeds = vec![0, 1, 0x7FFF_FFFF, 0xFFFF_FFFF];
    for index in LEVEL_SWEEP_STRIDES {
        seeds.push(index.wrapping_mul(SWEEP_MULTIPLIER));
    }
    dedupe(seeds)
}

/// Drop later duplicates while keeping the emit order, so both sides agree.
fn dedupe(seeds: Vec<u32>) -> Vec<u32> {
    let mut seen = std::collections::BTreeSet::new();
    seeds
        .into_iter()
        .filter(|seed| seen.insert(*seed))
        .collect()
}

/// Local error alias: domain errors carry structured data instead of `Display`.
type EmitResult<T> = Result<T, String>;

/// One native corpus pair: the tracked stage bytes and the sibling final bytes.
struct NativePair {
    relative: String,
    stage: Vec<u8>,
    final_bytes: Vec<u8>,
}

/// Read every `*_stage.bin`/`*_final.bin` pair in emit order.
fn native_pairs(corpus_root: &Path) -> EmitResult<Vec<NativePair>> {
    let mut pairs = Vec::new();
    for directory in CORPUS_DIRS {
        let root = corpus_root.join(directory);
        let mut stage_paths: Vec<PathBuf> = fs::read_dir(&root)
            .map_err(|error| format!("{}: {error}", root.display()))?
            .map(|entry| entry.map(|value| value.path()))
            .collect::<Result<_, _>>()
            .map_err(|error| format!("{}: {error}", root.display()))?;
        stage_paths.retain(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_stage.bin"))
        });
        stage_paths.sort();
        for stage_path in stage_paths {
            let stage_name = stage_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| format!("{}: non-UTF-8 file name", stage_path.display()))?;
            let final_path =
                stage_path.with_file_name(stage_name.replace("_stage.bin", "_final.bin"));
            let stage = fs::read(&stage_path)
                .map_err(|error| format!("{}: {error}", stage_path.display()))?;
            let final_bytes = fs::read(&final_path)
                .map_err(|error| format!("{}: {error}", final_path.display()))?;
            pairs.push(NativePair {
                relative: format!("{directory}/{stage_name}"),
                stage,
                final_bytes,
            });
        }
    }
    Ok(pairs)
}

/// Render one attempt list so both languages compare field for field.
fn attempts_text(traces: &[FinalizerAttemptTrace]) -> String {
    traces
        .iter()
        .map(|trace| {
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{:08x}",
                trace.target_index,
                trace.assigned_category,
                trace.weight_slot,
                trace.pool_size,
                trace.total_weight,
                trace
                    .selected_effect_id
                    .map(|id| format!("{id:04X}"))
                    .unwrap_or_else(|| "-".to_string()),
                trace
                    .roll_percent
                    .map(|roll| roll.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                if trace.accepted { 1 } else { 0 },
                trace.final_rng_state,
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

/// Classify an error kind from its `Debug` output so the gate can pin it.
fn error_kind(error: &R4FinalizerError) -> String {
    let text = format!("{error:?}");
    for name in [
        "UnsupportedRecordSize",
        "UnsupportedRecordType",
        "UnsupportedRarity",
        "UnsupportedPlaythrough",
        "InvalidTargetIndex",
        "UnknownExistingEffect",
        "UnknownPriorEffect",
        "ConflictSetTooLarge",
        "AmbiguousAuxiliaryMode",
        "MissingSpecialContext",
    ] {
        if text.contains(name) {
            return name.to_string();
        }
    }
    format!("Other:{text}")
}

/// Lineage fields the materializer patches onto the donor template.
#[derive(Clone, Copy)]
struct Lineage {
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
    transfer_count: u32,
}

/// One built stage-one insertion plus finalized preview.
struct BuiltPair {
    stage: Vec<u8>,
    preview: Vec<u8>,
    accepted: Option<u8>,
    attempts: String,
}

/// Build the stage-one insertion record and the finalized preview pair.
fn build_pair(
    index: &EffectTableIndex,
    grace_map: &nioh3_domain::effect::GraceMap,
    template: &[u8],
    lineage: Lineage,
) -> EmitResult<BuiltPair> {
    let template = ScrollRecordBytes::from_slice(template).map_err(|error| format!("{error:?}"))?;
    let pair = materialize_ng3_rarity4_final_record(
        index,
        grace_map,
        &template,
        lineage.seed,
        lineage.level,
        lineage.recommended_level,
        lineage.generation_serial,
        lineage.transfer_count,
    )
    .map_err(|error| format!("{error:?}"))?;
    Ok(BuiltPair {
        stage: pair.install_record().as_bytes().to_vec(),
        preview: pair.preview_record().as_bytes().to_vec(),
        accepted: pair.accepted_index(),
        attempts: attempts_text(pair.attempts()),
    })
}

fn accepted_text(accepted: Option<u8>) -> String {
    accepted
        .map(|index| index.to_string())
        .unwrap_or_else(|| "-".to_string())
}

/// Materialize one stage-one record, then run the completion entry point for a
/// single `reveal` flag. The stage-one record itself does not depend on
/// `reveal`; only the completion and its attempt traces do.
fn build_reveal_row(
    index: &EffectTableIndex,
    engine: &R4FinalizerEngine<'_>,
    grace_map: &nioh3_domain::effect::GraceMap,
    donor: &[u8],
    seed: u32,
    reveal: bool,
) -> EmitResult<String> {
    let template = ScrollRecordBytes::from_slice(donor).map_err(|error| format!("{error:?}"))?;
    let (stage, _sequence) = materialize_ng3_rarity4_stage_one_record(
        index,
        grace_map,
        &template,
        seed,
        LEVEL,
        SWEEP_RECOMMENDED_LEVEL,
        SWEEP_GENERATION_SERIAL,
        SWEEP_TRANSFER_COUNT,
    )
    .map_err(|error| format!("{error:?}"))?;
    let completion = engine
        .finalize_completion(&stage, reveal)
        .map_err(|error| format!("{error:?}"))?;
    Ok(format!(
        "reveal\t{seed}\t{LEVEL}\t{}\t{}\t{}\t{}\t{}",
        u8::from(reveal),
        hex(stage.as_bytes()),
        hex(completion.record.as_bytes()),
        accepted_text(completion.accepted_index),
        attempts_text(&completion.attempts),
    ))
}

fn run() -> EmitResult<()> {
    let root = env::args()
        .nth(1)
        .unwrap_or_else(|| "../../nioh3_scroll_editor/data".to_string());
    let corpus_root = env::args()
        .nth(2)
        .unwrap_or_else(|| "../../test_fixtures/r4_native_corpus".to_string());
    let corpus_root = PathBuf::from(corpus_root);

    let resource = load_effect_resource(Path::new(&root)).map_err(|error| error.to_string())?;
    let index = EffectTableIndex::from_resource(&resource).map_err(|error| format!("{error:?}"))?;
    let engine = R4FinalizerEngine::new(&index).map_err(|error| format!("{error:?}"))?;
    let grace_map = &resource.grace_maps[0];

    let pairs = native_pairs(&corpus_root)?;
    let mut lines = Vec::new();

    // Native corpus block: byte-exact against the tracked stage/final files.
    for pair in &pairs {
        let template =
            ScrollRecordBytes::from_slice(&pair.stage).map_err(|error| format!("{error:?}"))?;
        let built = build_pair(
            &index,
            grace_map,
            &pair.stage,
            Lineage {
                seed: template.displayed_seed(),
                level: template.level(),
                recommended_level: template.recommended_level(),
                generation_serial: template.generation_serial(),
                transfer_count: template.transfer_count(),
            },
        )?;
        lines.push(format!(
            "pair\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            pair.relative,
            template.displayed_seed(),
            template.level(),
            template.recommended_level(),
            template.generation_serial(),
            template.transfer_count(),
            u8::from(built.stage == pair.stage),
            u8::from(built.preview == pair.final_bytes),
            hex(&built.stage),
            hex(&built.preview),
            accepted_text(built.accepted),
            built.attempts,
        ));
    }

    // Sweep block: donor template plus overridden lineage fields, level 180.
    let donor = pairs
        .first()
        .ok_or_else(|| "native corpus is empty".to_string())?
        .stage
        .clone();
    for seed in seed_sweep() {
        let built = build_pair(
            &index,
            grace_map,
            &donor,
            Lineage {
                seed,
                level: LEVEL,
                recommended_level: SWEEP_RECOMMENDED_LEVEL,
                generation_serial: SWEEP_GENERATION_SERIAL,
                transfer_count: SWEEP_TRANSFER_COUNT,
            },
        )?;
        lines.push(format!(
            "sweep\t{seed}\t{LEVEL}\t{}\t{}\t{}\t{}",
            hex(&built.stage),
            hex(&built.preview),
            accepted_text(built.accepted),
            built.attempts,
        ));
    }

    // Level block: outer loop over levels, then seeds, exactly as the gate
    // builds its reference rows.
    for level in LEVEL_SWEEP {
        for seed in level_seed_sweep() {
            let built = build_pair(
                &index,
                grace_map,
                &donor,
                Lineage {
                    seed,
                    level,
                    recommended_level: SWEEP_RECOMMENDED_LEVEL,
                    generation_serial: SWEEP_GENERATION_SERIAL,
                    transfer_count: SWEEP_TRANSFER_COUNT,
                },
            )?;
            lines.push(format!(
                "level\t{seed}\t{level}\t{}\t{}\t{}\t{}",
                hex(&built.stage),
                hex(&built.preview),
                accepted_text(built.accepted),
                built.attempts,
            ));
        }
    }

    // Reveal block: the completion entry point is also exposed with
    // `reveal = false`, which resolves a different candidate weight slot.
    for seed in REVEAL_SEEDS {
        for reveal in [true, false] {
            lines.push(build_reveal_row(
                &index, &engine, grace_map, &donor, seed, reveal,
            )?);
        }
    }

    // Rejection block: every unsupported input must fail closed.
    lines.push(reject_size("record_size_short", &donor, true));
    lines.push(reject_size("record_size_long", &donor, false));
    lines.push(reject_record_type(&engine, &donor));
    lines.push(reject_rarity(&engine, &donor));
    lines.push(reject_template_context(&index, grace_map, &donor));
    lines.push(reject_promotion_target(&engine, &donor));

    for line in lines {
        println!("{line}");
    }
    Ok(())
}

fn reject_size(case: &str, donor: &[u8], short: bool) -> String {
    let mut bytes = donor.to_vec();
    if short {
        bytes.truncate(donor.len() - 1);
    } else {
        bytes.push(0);
    }
    let kind = match ScrollRecordBytes::from_slice(&bytes) {
        Ok(_) => "ok".to_string(),
        Err(error) => format!("err:{}", classify_record_text(&format!("{error:?}"))),
    };
    format!("reject\t{case}\t{kind}")
}

/// Run one mutated record through the real completion entry point.
fn engine_rejection(engine: &R4FinalizerEngine<'_>, bytes: &[u8]) -> String {
    let template = match ScrollRecordBytes::from_slice(bytes) {
        Ok(template) => template,
        Err(error) => return format!("err:{}", classify_record_text(&format!("{error:?}"))),
    };
    match engine.finalize_completion(&template, true) {
        Ok(_) => "ok".to_string(),
        Err(error) => format!("err:{}", error_kind(&error)),
    }
}

/// A record type outside the R4 finalizer context must fail closed.
fn reject_record_type(engine: &R4FinalizerEngine<'_>, donor: &[u8]) -> String {
    let mut bytes = donor.to_vec();
    bytes[0x00] = 0x82;
    bytes[0x01] = 0x1E;
    format!("reject\trecord_type\t{}", engine_rejection(engine, &bytes))
}

/// A rarity outside the R4 finalizer context must fail closed.
fn reject_rarity(engine: &R4FinalizerEngine<'_>, donor: &[u8]) -> String {
    let mut bytes = donor.to_vec();
    bytes[0x30] = 5;
    format!("reject\trarity\t{}", engine_rejection(engine, &bytes))
}

/// Map a structural error text onto a stable kind name.
fn classify_record_text(text: &str) -> String {
    // `RecordError::Length` is the codec's rejection for a record that is not
    // exactly 0xE8 bytes; `TemplateRecordType` is the materializer's context
    // guard. Both are named here so the gate can pin them.
    if text.contains("Length") {
        return "RecordLength".to_string();
    }
    for name in [
        "UnsupportedRecordType",
        "UnsupportedRarity",
        "UnsupportedPlaythrough",
        "TemplateRecordType",
    ] {
        if text.contains(name) {
            return name.to_string();
        }
    }
    format!("Other:{text}")
}

/// A non-NG3 donor template must be rejected by the stage-one materializer.
fn reject_template_context(
    index: &EffectTableIndex,
    grace_map: &nioh3_domain::effect::GraceMap,
    donor: &[u8],
) -> String {
    let mut bytes = donor.to_vec();
    bytes[0x00] = 0x6D;
    bytes[0x01] = 0x51;
    let template = match ScrollRecordBytes::from_slice(&bytes) {
        Ok(template) => template,
        Err(error) => return format!("err:Other:{error:?}"),
    };
    match materialize_ng3_rarity4_stage_one_record(
        index,
        grace_map,
        &template,
        1,
        LEVEL,
        SWEEP_RECOMMENDED_LEVEL,
        SWEEP_GENERATION_SERIAL,
        SWEEP_TRANSFER_COUNT,
    ) {
        Ok(_) => "reject\ttemplate_context\tok".to_string(),
        Err(error) => format!(
            "reject\ttemplate_context\terr:{}",
            classify_record_text(&format!("{error:?}"))
        ),
    }
}

/// An out-of-range completion target must not be silently accepted.
fn reject_promotion_target(engine: &R4FinalizerEngine<'_>, donor: &[u8]) -> String {
    let template = match ScrollRecordBytes::from_slice(donor) {
        Ok(template) => template,
        Err(error) => return format!("err:Other:{error:?}"),
    };
    match engine.finalize_effect(&template, 7, true, &[], None) {
        Ok(_) => "reject\tpromotion_target\tok".to_string(),
        Err(error) => format!("reject\tpromotion_target\terr:{}", error_kind(&error)),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("R4_FINALIZER_VECTORS_ERROR: {error}");
        std::process::exit(1);
    }
}
