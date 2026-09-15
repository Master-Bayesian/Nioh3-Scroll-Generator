//! Certified installation-record materializers for NG3 rarity 3, 4 and 5.
//!
//! Ports the record-writing half of `nioh3_scroll_editor/effect_sequence.py`:
//! `materialize_ng3_rarity3_record`, `materialize_ng3_rarity5_record`,
//! `materialize_ng3_certified_record` and
//! `materialize_ng3_certified_install_record`, together with the two slot
//! serializers they call.
//!
//! Rarity 4 has two native stages. [`materialize_ng3_certified_install_record`]
//! returns the stage-one record the save must receive *and* separately the
//! completed preview the game's reveal path produces. They are distinct owned
//! buffers and are never collapsed: writing the completed record makes the game
//! complete it a second time.
//!
//! Every materializer preserves the template verbatim outside the lineage
//! fields, the rarity pair, the challenge count, the seven effect slots and the
//! transfer count; only [`materialize_ng3_rarity4_stage_one_record`] additionally
//! zeroes the `+0x0C` completion salt.

use crate::effect::{EffectTableIndex, GraceMap};
use crate::record::{
    RecordError, ScrollEffect, ScrollRecord, ScrollRecordBytes, EFFECT_AREA_BYTES,
    EFFECT_SLOT_BASE, EFFECT_SLOT_COUNT, EFFECT_SLOT_STRIDE, EMPTY_EFFECT_ID,
};
use crate::sequence::{
    generate_ng3_rarity3_effect_sequence, generate_ng3_rarity5_effect_sequence,
    materialize_ng3_rarity4_final_record, materialize_ng3_rarity4_stage_one_record, SequenceError,
    NG3_PLAYTHROUGH, NG3_RECORD_TYPE, RARITY_DIVINE, RARITY_FINALIZABLE, RARITY_GROWING,
};

/// Fail-closed problems for one installation-record materialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMaterializeError {
    /// A generator or the R4 finalizer refused the request.
    Sequence(SequenceError),
    /// The recovered record codec rejected a field write.
    Record(RecordError),
    /// The template is not the native NG3 `0xE604` record.
    TemplateRecordType { record_type: u16 },
    /// A generated sequence did not match the context its serializer requires.
    UnsupportedEffectContext { rarity: u8 },
    /// Certified materialization exists only for rarity 3, 4 and 5.
    UnsupportedRarity(u8),
}

impl From<SequenceError> for InstallMaterializeError {
    fn from(error: SequenceError) -> Self {
        Self::Sequence(error)
    }
}

impl From<RecordError> for InstallMaterializeError {
    fn from(error: RecordError) -> Self {
        Self::Record(error)
    }
}

impl std::fmt::Display for InstallMaterializeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequence(error) => write!(formatter, "effect sequence refused: {error:?}"),
            Self::Record(error) => write!(formatter, "record codec refused: {error:?}"),
            Self::TemplateRecordType { record_type } => write!(
                formatter,
                "template must be a native NG3 0xE604 record, not 0x{record_type:04X}"
            ),
            Self::UnsupportedEffectContext { rarity } => {
                write!(
                    formatter,
                    "unsupported effect-sequence context for rarity {rarity}"
                )
            }
            Self::UnsupportedRarity(rarity) => write!(
                formatter,
                "certified NG3 materialization supports rarity 3, 4, or 5, not {rarity}"
            ),
        }
    }
}

impl std::error::Error for InstallMaterializeError {}

/// One serialized effect slot block: the generated slots, then empty sentinels.
///
/// Mirrors `serialize_ng3_rarity3_effect_slots`,
/// `serialize_ng3_rarity4_stage_one_effect_slots` and
/// `serialize_rarity5_grace_effect_slots`. Each writes the four words at
/// `+0x00..+0x10` and leaves `tail_0`/`tail_1` zero, then fills every remaining
/// slot's id with `EMPTY_EFFECT_ID`; the three reference serializers agree
/// byte for byte on that layout.
fn serialize_effect_slots(effects: &[ScrollEffect]) -> [u8; EFFECT_AREA_BYTES] {
    let mut output = [0u8; EFFECT_AREA_BYTES];
    for (index, effect) in effects.iter().enumerate() {
        if index >= EFFECT_SLOT_COUNT {
            break;
        }
        let offset = index * EFFECT_SLOT_STRIDE;
        let metadata = u32::from(effect.roll_percent)
            | (u32::from(effect.category_and_flags) << 8)
            | (u32::from(effect.effect_flags) << 16);
        output[offset..offset + 4].copy_from_slice(&u32::from(effect.prefix_word).to_le_bytes());
        output[offset + 4..offset + 8].copy_from_slice(&effect.effect_id.to_le_bytes());
        output[offset + 8..offset + 12]
            .copy_from_slice(&(effect.resolved_value as u32).to_le_bytes());
        output[offset + 12..offset + 16].copy_from_slice(&metadata.to_le_bytes());
    }
    for index in effects.len().min(EFFECT_SLOT_COUNT)..EFFECT_SLOT_COUNT {
        let offset = index * EFFECT_SLOT_STRIDE;
        output[offset + 4..offset + 8].copy_from_slice(&EMPTY_EFFECT_ID.to_le_bytes());
    }
    output
}

/// The guard `serialize_ng3_rarity3_effect_slots` /
/// `serialize_ng3_rarity4_stage_one_effect_slots` apply to their input.
fn ensure_effect_context(
    sequence: &ScrollRecord,
    rarity: u8,
    effect_count: usize,
) -> Result<(), InstallMaterializeError> {
    if sequence.playthrough != NG3_PLAYTHROUGH
        || sequence.record_type != NG3_RECORD_TYPE
        || sequence.rarity != rarity
        || sequence.effects.len() != effect_count
    {
        return Err(InstallMaterializeError::UnsupportedEffectContext { rarity });
    }
    Ok(())
}

/// `materialize_ng3_rarity3_record`: the observed NG3 rarity-3 canonical layout.
///
/// The template's `+0x0C` salt is preserved; only the rarity-4 stage-one path
/// resets it.
pub fn materialize_ng3_rarity3_record(
    index: &EffectTableIndex,
    template: &ScrollRecordBytes,
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
    transfer_count: u32,
) -> Result<(ScrollRecordBytes, ScrollRecord), InstallMaterializeError> {
    ensure_ng3_template(template)?;
    let sequence = generate_ng3_rarity3_effect_sequence(index, seed, level)?;
    ensure_effect_context(&sequence, RARITY_GROWING, 5)?;
    let mut record = template.clone();
    write_lineage(
        &mut record,
        seed,
        level,
        recommended_level,
        generation_serial,
    )?;
    record.write_u8(0x30, RARITY_GROWING)?;
    record.write_u8(0x31, RARITY_GROWING)?;
    write_challenge_count(&mut record, seed)?;
    record.write_bytes(EFFECT_SLOT_BASE, &serialize_effect_slots(&sequence.effects))?;
    record.write_u32(0xDC, transfer_count)?;
    Ok((record, sequence))
}

/// `materialize_ng3_rarity5_record`: the canonical NG3 rarity-5 Grace layout.
pub fn materialize_ng3_rarity5_record(
    index: &EffectTableIndex,
    grace_map: &GraceMap,
    template: &ScrollRecordBytes,
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
    transfer_count: u32,
) -> Result<(ScrollRecordBytes, ScrollRecord), InstallMaterializeError> {
    ensure_ng3_template(template)?;
    let sequence = generate_ng3_rarity5_effect_sequence(index, grace_map, seed, level)?;
    ensure_effect_context(&sequence, RARITY_DIVINE, 6)?;
    let mut record = template.clone();
    write_lineage(
        &mut record,
        seed,
        level,
        recommended_level,
        generation_serial,
    )?;
    record.write_u8(0x30, RARITY_DIVINE)?;
    record.write_u8(0x31, RARITY_DIVINE)?;
    write_challenge_count(&mut record, seed)?;
    record.write_bytes(EFFECT_SLOT_BASE, &serialize_effect_slots(&sequence.effects))?;
    record.write_u32(0xDC, transfer_count)?;
    Ok((record, sequence))
}

/// `materialize_ng3_certified_record`: bind one certified result to a template.
///
/// Returns the record the shipped path reports as the candidate's `record` —
/// the completed record for rarity 4, the canonical record for rarity 3 and 5.
pub fn materialize_ng3_certified_record(
    index: &EffectTableIndex,
    grace_map: &GraceMap,
    template: &ScrollRecordBytes,
    rarity: u8,
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
    transfer_count: u32,
) -> Result<(ScrollRecordBytes, ScrollRecord), InstallMaterializeError> {
    match rarity {
        RARITY_GROWING => materialize_ng3_rarity3_record(
            index,
            template,
            seed,
            level,
            recommended_level,
            generation_serial,
            transfer_count,
        ),
        RARITY_FINALIZABLE => {
            ensure_ng3_template(template)?;
            let pair = materialize_ng3_rarity4_final_record(
                index,
                grace_map,
                template,
                seed,
                level,
                recommended_level,
                generation_serial,
                transfer_count,
            )?;
            Ok((
                pair.preview_record().clone(),
                pair.preview_sequence().clone(),
            ))
        }
        RARITY_DIVINE => materialize_ng3_rarity5_record(
            index,
            grace_map,
            template,
            seed,
            level,
            recommended_level,
            generation_serial,
            transfer_count,
        ),
        other => Err(InstallMaterializeError::UnsupportedRarity(other)),
    }
}

/// `materialize_ng3_certified_install_record`: the record a save must receive.
///
/// Rarity 4 acquisition has two native stages, so this returns the stage-one
/// record the save must receive plus the completed preview the reveal path
/// produces. Every other certified rarity is already stored in its installable
/// form, and its two values coincide by construction.
pub fn materialize_ng3_certified_install_record(
    index: &EffectTableIndex,
    grace_map: &GraceMap,
    template: &ScrollRecordBytes,
    rarity: u8,
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
    transfer_count: u32,
) -> Result<(ScrollRecordBytes, ScrollRecord), InstallMaterializeError> {
    if rarity != RARITY_FINALIZABLE {
        return materialize_ng3_certified_record(
            index,
            grace_map,
            template,
            rarity,
            seed,
            level,
            recommended_level,
            generation_serial,
            transfer_count,
        );
    }
    let (install_record, _stage_one) = materialize_ng3_rarity4_stage_one_record(
        index,
        grace_map,
        template,
        seed,
        level,
        recommended_level,
        generation_serial,
        transfer_count,
    )?;
    let pair = materialize_ng3_rarity4_final_record(
        index,
        grace_map,
        template,
        seed,
        level,
        recommended_level,
        generation_serial,
        transfer_count,
    )?;
    Ok((install_record, pair.preview_sequence().clone()))
}

/// The `template must be a native NG3 0xE604 record` guard.
fn ensure_ng3_template(template: &ScrollRecordBytes) -> Result<(), InstallMaterializeError> {
    let record_type = template.record_type();
    if record_type != NG3_RECORD_TYPE {
        return Err(InstallMaterializeError::TemplateRecordType { record_type });
    }
    Ok(())
}

/// The five lineage fields every materializer overwrites.
fn write_lineage(
    record: &mut ScrollRecordBytes,
    seed: u32,
    level: u16,
    recommended_level: u16,
    generation_serial: u32,
) -> Result<(), InstallMaterializeError> {
    record.write_u16(0x06, level)?;
    record.write_u16(0x08, level)?;
    record.write_u16(0x10, recommended_level)?;
    record.write_u16(0x12, recommended_level)?;
    record.write_u32(0x20, seed)?;
    record.write_u32(0x28, generation_serial)?;
    Ok(())
}

/// Record byte `+0x33` from the Seed-derived challenge attempt count.
fn write_challenge_count(
    record: &mut ScrollRecordBytes,
    seed: u32,
) -> Result<(), InstallMaterializeError> {
    let count = crate::sequence::generate_challenge_attempt_count(seed);
    // The generator is bounded by MIN..=MAX, so the cast is exact.
    record.write_u8(0x33, count as u8)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::test_support::{dense_map, synthetic_index};
    use crate::sequence::{RARITY4_STAGE_ONE_SLOT, RARITY5_GRACE_SLOT};

    fn template() -> ScrollRecordBytes {
        let mut template = ScrollRecordBytes::zeroed();
        template
            .write_u16(0x00, NG3_RECORD_TYPE)
            .expect("template type");
        template
    }

    fn stage_one_map() -> GraceMap {
        dense_map(
            u32::from(NG3_RECORD_TYPE),
            RARITY_FINALIZABLE,
            RARITY4_STAGE_ONE_SLOT,
        )
    }

    fn grace_map() -> GraceMap {
        dense_map(
            u32::from(NG3_RECORD_TYPE),
            RARITY_DIVINE,
            RARITY5_GRACE_SLOT,
        )
    }

    /// The rarity-5 installation record binds the generated sequence to the
    /// template without touching any other byte, and reports a distinct
    /// completed sequence.
    #[test]
    fn a_rarity_five_record_binds_the_sequence_and_keeps_the_template() {
        let mut template = template();
        template.write_u8(0x40, 0x5A).expect("lineage byte");
        let (install, preview_sequence) = materialize_ng3_certified_install_record(
            &synthetic_index(),
            &grace_map(),
            &template,
            RARITY_DIVINE,
            0x1234_5678,
            180,
            183,
            7,
            0,
        )
        .expect("rarity-5 record");
        assert_eq!(install.rarity(), RARITY_DIVINE);
        assert_eq!(install.as_bytes()[0x40], 0x5A);
        assert_eq!(install.generation_serial(), 7);
        assert_eq!(preview_sequence.rarity, RARITY_DIVINE);
        assert_eq!(preview_sequence.effects.len(), 6);
        assert_eq!(
            &install.as_bytes()[EFFECT_SLOT_BASE..EFFECT_SLOT_BASE + 6 * EFFECT_SLOT_STRIDE],
            &serialize_effect_slots(&preview_sequence.effects)[..6 * EFFECT_SLOT_STRIDE]
        );
        // Only the serialized slots are empty past the sixth effect.
        assert_eq!(
            u32::from_le_bytes(
                install.as_bytes()[EFFECT_SLOT_BASE + 6 * EFFECT_SLOT_STRIDE + 4..][..4]
                    .try_into()
                    .expect("four bytes")
            ),
            EMPTY_EFFECT_ID
        );
    }

    /// A certified record inherits the template outside the lineage fields.
    #[test]
    fn a_certified_record_keeps_the_template_lineage() {
        let mut template = template();
        template.write_u8(0x40, 0x5A).expect("lineage byte");
        let (record, _sequence) = materialize_ng3_certified_record(
            &synthetic_index(),
            &grace_map(),
            &template,
            RARITY_GROWING,
            0x0000_00FF,
            12,
            99,
            3,
            4,
        )
        .expect("rarity-3 record");
        assert_eq!(record.as_bytes()[0x40], 0x5A);
        assert_eq!(record.level(), 12);
        assert_eq!(record.recommended_level(), 99);
        assert_eq!(record.displayed_seed(), 0x0000_00FF);
        assert_eq!(record.generation_serial(), 3);
        assert_eq!(record.transfer_count(), 4);
        assert_eq!(record.rarity(), RARITY_GROWING);
    }

    #[test]
    fn an_unsupported_rarity_is_refused() {
        let error = materialize_ng3_certified_record(
            &synthetic_index(),
            &grace_map(),
            &template(),
            2,
            1,
            1,
            1,
            1,
            1,
        )
        .expect_err("rarity 2 is not certified");
        assert_eq!(error, InstallMaterializeError::UnsupportedRarity(2));
    }

    /// `template must be a native NG3 0xE604 record` is enforced before any
    /// table is read, for the two-stage rarity as well.
    #[test]
    fn a_foreign_template_is_refused_before_any_table_use() {
        let mut foreign = ScrollRecordBytes::zeroed();
        foreign.write_u16(0x00, 0x516D).expect("foreign type");
        let error = materialize_ng3_certified_install_record(
            &synthetic_index(),
            &stage_one_map(),
            &foreign,
            RARITY_FINALIZABLE,
            1,
            1,
            1,
            1,
            1,
        )
        .expect_err("a foreign template is refused");
        // The two-stage path refuses through the stage-one materializer's own
        // guard, which reports the same record type the wrapper would.
        assert!(matches!(
            error,
            InstallMaterializeError::Sequence(SequenceError::TemplateRecordType {
                record_type: 0x516D
            }) | InstallMaterializeError::TemplateRecordType {
                record_type: 0x516D
            }
        ));
    }
}
