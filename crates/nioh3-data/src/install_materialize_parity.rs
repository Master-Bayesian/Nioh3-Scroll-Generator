//! Shipped-resource check for the certified installation-record split.
//!
//! `save.materialize_live_many` must publish two records for a rarity-4
//! certified candidate: the stage-one record the save receives and the
//! completed record the reveal path produces. This test drives the real
//! product effect tables and Grace map, so the split is proven against the
//! tables the product ships rather than a synthetic index. It touches no save
//! and no game process.

use std::path::{Path, PathBuf};

use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::install_materialize::{
    materialize_ng3_certified_install_record, materialize_ng3_certified_record,
};
use nioh3_domain::record::ScrollRecordBytes;
use nioh3_domain::sequence::{materialize_ng3_rarity4_stage_one_record, NG3_RECORD_TYPE};

use crate::load_effect_resource;

fn data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data")
}

fn ng3_template() -> ScrollRecordBytes {
    let mut template = ScrollRecordBytes::zeroed();
    template
        .write_u16(0x00, NG3_RECORD_TYPE)
        .expect("template type");
    template
}

#[test]
fn the_installation_record_is_the_stage_one_record_and_not_the_completed_one() {
    let resource = load_effect_resource(&data_root()).expect("shipped effect resource loads");
    let index = EffectTableIndex::from_resource(&resource).expect("effect index builds");
    let stage_one_map = &resource.grace_maps[0];
    let template = ng3_template();

    let mut differences = 0;
    for seed in [1u32, 7, 0x1234_5678, 0x0FFF_FFFF] {
        let (stage_one, _) = materialize_ng3_rarity4_stage_one_record(
            &index,
            stage_one_map,
            &template,
            seed,
            180,
            183,
            11,
            0,
        )
        .expect("stage-one record");
        let (install, stage_sequence) = materialize_ng3_certified_install_record(
            &index,
            stage_one_map,
            &template,
            4,
            seed,
            180,
            183,
            11,
            0,
        )
        .expect("installation record");
        assert_eq!(
            install, stage_one,
            "the installation record must be the stage-one record for seed {seed}"
        );
        assert_eq!(stage_sequence.effects.len(), 5);

        let (completed, completed_sequence) = materialize_ng3_certified_record(
            &index,
            stage_one_map,
            &template,
            4,
            seed,
            180,
            183,
            11,
            0,
        )
        .expect("completed record");
        assert_eq!(completed_sequence.effects.len(), 5);
        assert_eq!(completed.level(), 180);
        assert_eq!(completed.recommended_level(), 183);
        assert_eq!(completed.transfer_count(), 0);
        if install != completed {
            differences += 1;
        }
    }
    assert!(
        differences > 0,
        "at least one seed must show the completed record differing from the \
         stage-one record, or the split would be untestable"
    );
}
