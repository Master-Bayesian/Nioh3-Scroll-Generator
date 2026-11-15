//! Seed-to-scroll generation against the game's own records.
//!
//! `test_fixtures/r4_native_corpus` holds stage-one and finalized NG3 rarity-4
//! records the game generated (v2.00.02, playthrough 3, level 180). They are the
//! oracle: each pair must be rebuilt byte for byte from its own lineage. The
//! sweep digest pins every other seed to today's output, so any change to
//! generation is a deliberate snapshot update.

use std::path::{Path, PathBuf};

use nioh3_data::load_effect_resource;
use nioh3_domain::effect::{EffectTableIndex, GraceMap};
use nioh3_domain::record::{materialize_ng3_rarity4_final_record, ScrollRecordBytes};
use sha2::{Digest, Sha256};

const RECORD_SIZE: usize = 0xE8;
const SWEEP_SEEDS: u32 = 4096;
const SWEEP_MULTIPLIER: u32 = 2_654_435_761;
const SWEEP_LEVELS: [u16; 4] = [1, 180, 500, 65_535];
const SWEEP_DIGEST: &str = "edbc0f6494005257d33b339df291d53a3dbc352a2702dd06af017949ea89d33b";

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct NativePair {
    name: String,
    stage: Vec<u8>,
    final_bytes: Vec<u8>,
}

fn native_pairs() -> Vec<NativePair> {
    let corpus = repository().join("test_fixtures/r4_native_corpus");
    let mut pairs = Vec::new();
    for directory in ["base", "distributed"] {
        let mut stages: Vec<PathBuf> = std::fs::read_dir(corpus.join(directory))
            .expect("corpus directory")
            .map(|entry| entry.expect("corpus entry").path())
            .filter(|path| path.to_string_lossy().ends_with("_stage.bin"))
            .collect();
        stages.sort();
        for stage in stages {
            let final_path =
                PathBuf::from(stage.to_string_lossy().replace("_stage.bin", "_final.bin"));
            pairs.push(NativePair {
                name: format!(
                    "{directory}/{}",
                    stage.file_name().unwrap().to_string_lossy()
                ),
                stage: std::fs::read(&stage).expect("stage bytes"),
                final_bytes: std::fs::read(&final_path).expect("final bytes"),
            });
        }
    }
    pairs
}

struct Lineage {
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
    transfer_count: u32,
}

fn build(
    index: &EffectTableIndex,
    grace: &GraceMap,
    template: &ScrollRecordBytes,
    lineage: &Lineage,
) -> (Vec<u8>, Vec<u8>) {
    let pair = materialize_ng3_rarity4_final_record(
        index,
        grace,
        template,
        lineage.seed,
        lineage.level,
        lineage.recommended_level,
        lineage.generation_serial,
        lineage.transfer_count,
    )
    .expect("the record materializes");
    (
        pair.install_record().as_bytes().to_vec(),
        pair.preview_record().as_bytes().to_vec(),
    )
}

#[test]
fn generation_reproduces_the_games_records_and_the_pinned_sweep() {
    let resource = load_effect_resource(&repository().join("data")).expect("shipped resource");
    let index = EffectTableIndex::from_resource(&resource).expect("effect index");
    let grace = &resource.grace_maps[0];
    let pairs = native_pairs();
    assert_eq!(pairs.len(), 10);

    for pair in &pairs {
        assert_eq!(pair.stage.len(), RECORD_SIZE, "{}", pair.name);
        let template = ScrollRecordBytes::from_slice(&pair.stage).expect("native record");
        let lineage = Lineage {
            seed: template.displayed_seed(),
            level: template.level(),
            recommended_level: template.recommended_level(),
            generation_serial: template.generation_serial(),
            transfer_count: template.transfer_count(),
        };
        let (stage, finalized) = build(&index, grace, &template, &lineage);
        assert!(
            stage == pair.stage,
            "{}: stage differs from the game",
            pair.name
        );
        assert!(
            finalized == pair.final_bytes,
            "{}: final differs from the game",
            pair.name
        );
    }

    let donor = ScrollRecordBytes::from_slice(&pairs[0].stage).expect("donor record");
    let mut digest = Sha256::new();
    for level in SWEEP_LEVELS {
        for step in 0..SWEEP_SEEDS {
            let lineage = Lineage {
                seed: step.wrapping_mul(SWEEP_MULTIPLIER),
                level,
                recommended_level: 183,
                generation_serial: 3_283_000,
                transfer_count: 0,
            };
            let (stage, finalized) = build(&index, grace, &donor, &lineage);
            digest.update(&stage);
            digest.update(&finalized);
        }
    }
    let actual = format!("{:x}", digest.finalize());
    assert_eq!(
        actual, SWEEP_DIGEST,
        "seed-to-scroll output changed; update SWEEP_DIGEST only if the change is intended"
    );
}
