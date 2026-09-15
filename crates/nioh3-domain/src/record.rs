//! Typed scroll and effect records plus the recovered 0xE8 record byte codec.
//!
//! Field names and integer widths mirror `nioh3_scroll_editor/models.py` and the
//! reference effect-sequence result at the M2.1 baseline. The byte codec mirrors
//! `nioh3_scroll_editor/r4_finalizer_reference.py` and the slot serializer in
//! `nioh3_scroll_editor/effect_sequence.py`. `GenerationContext` digest binding,
//! candidate identity and every write path remain outside this crate.

use crate::r4_finalizer::FinalizerAttemptTrace;

// The materializers construct records from verified tables, so they live with
// the sequence paths; they are re-exported here because they are the entry
// points the record pair is produced from.
pub use crate::sequence::{
    materialize_ng3_rarity4_final_record, materialize_ng3_rarity4_stage_one_record,
};

/// Total bytes of one recovered scroll record.
pub const SCROLL_RECORD_BYTES: usize = 0xE8;
/// Byte offset of the first serialized effect slot.
pub const EFFECT_SLOT_BASE: usize = 0x34;
/// Serialized effect slots in one record.
pub const EFFECT_SLOT_COUNT: usize = 7;
/// Stride of one serialized effect slot.
pub const EFFECT_SLOT_STRIDE: usize = 0x18;
/// Bytes covered by the seven serialized effect slots.
pub const EFFECT_AREA_BYTES: usize = EFFECT_SLOT_COUNT * EFFECT_SLOT_STRIDE;
/// Sentinel effect id carried by every empty slot (`uint32::MAX`).
pub const EMPTY_EFFECT_ID: u32 = 0xFFFF_FFFF;

/// Fail-closed codec problems for the recovered 0xE8 record layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordError {
    /// The supplied buffer is not exactly one record long.
    Length { expected: usize, actual: usize },
    /// An effect-slot index outside `0..EFFECT_SLOT_COUNT`.
    SlotIndex { index: usize },
    /// A field access would run past the end of the record.
    FieldOutOfRange { offset: usize, width: usize },
    /// The serializer was handed a record outside its supported context.
    UnsupportedEffectContext,
    /// A binary32 intermediate was not finite, so the reference's `int()` fails.
    NonFiniteIntermediate { stage: &'static str },
    /// A binary32 intermediate was outside the truncation contract.
    OutOfRangeIntermediate { stage: &'static str },
}

/// One decoded 0x18-byte effect slot.
///
/// Field names and widths mirror `r4_finalizer_reference.EffectSlot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordSlot {
    pub prefix_word: u32,
    pub raw_id: u32,
    pub value: i32,
    pub roll_percent: u8,
    pub category_and_flags: u8,
    pub effect_flags: u8,
    pub byte_0f: u8,
    pub tail_0: u32,
    pub tail_1: u32,
}

impl RecordSlot {
    /// Decode one slot; the index is range-checked like the reference.
    pub fn parse(record: &ScrollRecordBytes, index: usize) -> Result<Self, RecordError> {
        let offset = ScrollRecordBytes::slot_offset(index)?;
        Ok(Self {
            prefix_word: record.read_u32(offset)?,
            raw_id: record.read_u32(offset + 0x04)?,
            value: record.read_i32(offset + 0x08)?,
            roll_percent: record.read_u8(offset + 0x0C)?,
            category_and_flags: record.read_u8(offset + 0x0D)?,
            effect_flags: record.read_u8(offset + 0x0E)?,
            byte_0f: record.read_u8(offset + 0x0F)?,
            tail_0: record.read_u32(offset + 0x10)?,
            tail_1: record.read_u32(offset + 0x14)?,
        })
    }

    /// Low 16 bits carry the serialized group key.
    pub fn prefix_id(&self) -> u16 {
        self.prefix_word as u16
    }

    /// Low six bits carry the category key.
    pub fn category(&self) -> u8 {
        self.category_and_flags & 0x3F
    }

    /// The slot carries the empty-effect sentinel.
    pub fn is_empty(&self) -> bool {
        self.raw_id == EMPTY_EFFECT_ID
    }

    /// RVA 0x10280E0..0x10280F4 completion-loop eligibility.
    pub fn completion_loop_eligible(&self) -> bool {
        self.prefix_id() != 0
            && self.category_and_flags & 0x40 == 0
            && self.effect_flags & 0x04 == 0
    }

    /// RVA 0x2279A15..0x2279A2A wrapper prior-row eligibility.
    pub fn wrapper_prior_effect_eligible(&self) -> bool {
        self.prefix_id() != 0
            && self.category_and_flags & 0x40 == 0
            && self.effect_flags & 0x02 == 0
            && self.value != 1
    }

    /// RVA 0x1028106..0x102810C acceptance flag.
    pub fn completion_candidate_is_accepted(&self) -> bool {
        self.effect_flags & 0x04 != 0
    }
}

/// A validated, owned 0xE8 scroll record.
///
/// Every read and write is bounds-checked, so unknown or unmodified bytes are
/// preserved verbatim when a record is copied and only explicit fields change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollRecordBytes([u8; SCROLL_RECORD_BYTES]);

impl ScrollRecordBytes {
    /// Wrap an exactly-sized record buffer.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, RecordError> {
        let owned: [u8; SCROLL_RECORD_BYTES] =
            bytes.try_into().map_err(|_| RecordError::Length {
                expected: SCROLL_RECORD_BYTES,
                actual: bytes.len(),
            })?;
        Ok(Self(owned))
    }

    /// An all-zero record, as the offline materializers start from.
    pub fn zeroed() -> Self {
        Self([0u8; SCROLL_RECORD_BYTES])
    }

    pub fn as_bytes(&self) -> &[u8; SCROLL_RECORD_BYTES] {
        &self.0
    }

    pub fn into_bytes(self) -> [u8; SCROLL_RECORD_BYTES] {
        self.0
    }

    /// Absolute offset of one effect slot, range-checked.
    pub fn slot_offset(index: usize) -> Result<usize, RecordError> {
        if index >= EFFECT_SLOT_COUNT {
            return Err(RecordError::SlotIndex { index });
        }
        Ok(EFFECT_SLOT_BASE + index * EFFECT_SLOT_STRIDE)
    }

    fn field_range(offset: usize, width: usize) -> Result<usize, RecordError> {
        let end = offset
            .checked_add(width)
            .ok_or(RecordError::FieldOutOfRange { offset, width })?;
        if end > SCROLL_RECORD_BYTES {
            return Err(RecordError::FieldOutOfRange { offset, width });
        }
        Ok(end)
    }

    pub fn read_u8(&self, offset: usize) -> Result<u8, RecordError> {
        Self::field_range(offset, 1)?;
        Ok(self.0[offset])
    }

    pub fn read_u16(&self, offset: usize) -> Result<u16, RecordError> {
        let end = Self::field_range(offset, 2)?;
        Ok(u16::from_le_bytes(
            self.0[offset..end].try_into().expect("two bytes"),
        ))
    }

    pub fn read_u32(&self, offset: usize) -> Result<u32, RecordError> {
        let end = Self::field_range(offset, 4)?;
        Ok(u32::from_le_bytes(
            self.0[offset..end].try_into().expect("four bytes"),
        ))
    }

    pub fn read_i32(&self, offset: usize) -> Result<i32, RecordError> {
        let end = Self::field_range(offset, 4)?;
        Ok(i32::from_le_bytes(
            self.0[offset..end].try_into().expect("four bytes"),
        ))
    }

    pub fn write_u8(&mut self, offset: usize, value: u8) -> Result<(), RecordError> {
        Self::field_range(offset, 1)?;
        self.0[offset] = value;
        Ok(())
    }

    pub fn write_u16(&mut self, offset: usize, value: u16) -> Result<(), RecordError> {
        let end = Self::field_range(offset, 2)?;
        self.0[offset..end].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_u32(&mut self, offset: usize, value: u32) -> Result<(), RecordError> {
        let end = Self::field_range(offset, 4)?;
        self.0[offset..end].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    pub fn write_i32(&mut self, offset: usize, value: i32) -> Result<(), RecordError> {
        let end = Self::field_range(offset, 4)?;
        self.0[offset..end].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    /// Read-modify-write one `u32` field, for the recovered bit-preserving
    /// clears.
    pub fn update_u32(
        &mut self,
        offset: usize,
        update: impl FnOnce(u32) -> u32,
    ) -> Result<(), RecordError> {
        let current = self.read_u32(offset)?;
        self.write_u32(offset, update(current))
    }

    /// Copy bytes into the record at `offset`, bounds-checked.
    pub fn write_bytes(&mut self, offset: usize, source: &[u8]) -> Result<(), RecordError> {
        let end = Self::field_range(offset, source.len())?;
        self.0[offset..end].copy_from_slice(source);
        Ok(())
    }

    /// Decode one effect slot.
    pub fn slot(&self, index: usize) -> Result<RecordSlot, RecordError> {
        RecordSlot::parse(self, index)
    }

    /// Serialized record type at `+0x00`.
    pub fn record_type(&self) -> u16 {
        u16::from_le_bytes([self.0[0x00], self.0[0x01]])
    }

    /// Serialized level at `+0x06`.
    pub fn level(&self) -> u16 {
        u16::from_le_bytes([self.0[0x06], self.0[0x07]])
    }

    /// R4 completion salt at `+0x0C`.
    pub fn completion_salt(&self) -> u16 {
        u16::from_le_bytes([self.0[0x0C], self.0[0x0D]])
    }

    /// Recommended level at `+0x10`.
    pub fn recommended_level(&self) -> u16 {
        u16::from_le_bytes([self.0[0x10], self.0[0x11]])
    }

    /// Displayed seed at `+0x20`.
    pub fn displayed_seed(&self) -> u32 {
        u32::from_le_bytes([self.0[0x20], self.0[0x21], self.0[0x22], self.0[0x23]])
    }

    /// Generation serial at `+0x28`.
    pub fn generation_serial(&self) -> u32 {
        u32::from_le_bytes([self.0[0x28], self.0[0x29], self.0[0x2A], self.0[0x2B]])
    }

    /// Signed rarity byte at `+0x30`, as the finalizer seed derivation reads it.
    pub fn signed_rarity(&self) -> i8 {
        self.0[0x30] as i8
    }

    /// Unsigned rarity byte at `+0x30`.
    pub fn rarity(&self) -> u8 {
        self.0[0x30]
    }

    /// Transfer count at `+0xDC`.
    pub fn transfer_count(&self) -> u32 {
        u32::from_le_bytes([self.0[0xDC], self.0[0xDD], self.0[0xDE], self.0[0xDF]])
    }
}

/// Stage of a generated record, mirroring `CandidateRecordStage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordStage {
    FinalRecord,
    NativeStageOne,
    EffectSequenceOnly,
}

/// One generated effect slot, mirroring `GeneratedEffect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollEffect {
    pub slot: u8,
    pub source_index: u8,
    pub effect_id: u32,
    pub roll_percent: u8,
    pub category_and_flags: u8,
    pub effect_flags: u8,
    pub candidate_count: u32,
    pub resolved_value: i32,
    pub prefix_word: u16,
}

impl ScrollEffect {
    /// Low six bits of `category_and_flags`, as the reference exposes.
    pub fn category(&self) -> u8 {
        self.category_and_flags & 0x3F
    }
}

/// One generated sequence, mirroring `EffectSequenceResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollRecord {
    pub seed: u32,
    pub record_type: u16,
    pub rarity: u8,
    pub playthrough: u8,
    pub level: u16,
    pub effects: Vec<ScrollEffect>,
    pub promoted_source_indexes: Vec<u8>,
    pub random_draws: u32,
    pub final_rng_state: u32,
    pub terminal_is_special: bool,
}

impl ScrollRecord {
    pub fn primary(&self) -> Option<&ScrollEffect> {
        self.effects.first()
    }

    /// Serialize the seven-slot NG3 rarity-4 stage-one layout.
    ///
    /// Mirrors `serialize_ng3_rarity4_stage_one_effect_slots`, including its
    /// context guard and its use of positional order rather than the slot
    /// field.
    pub fn serialize_rarity4_stage_one_slots(
        &self,
    ) -> Result<[u8; EFFECT_AREA_BYTES], RecordError> {
        if self.playthrough != crate::sequence::NG3_PLAYTHROUGH
            || self.record_type != crate::sequence::NG3_RECORD_TYPE
            || self.rarity != crate::sequence::RARITY_FINALIZABLE
            || self.effects.len() != RARITY4_FINAL_EFFECT_COUNT
        {
            return Err(RecordError::UnsupportedEffectContext);
        }
        let mut output = [0u8; EFFECT_AREA_BYTES];
        for (index, effect) in self.effects.iter().enumerate() {
            let offset = index * EFFECT_SLOT_STRIDE;
            let metadata = u32::from(effect.roll_percent)
                | (u32::from(effect.category_and_flags) << 8)
                | (u32::from(effect.effect_flags) << 16);
            // `ScrollEffect::prefix_word` carries the recovered uint16 group
            // key, which the reference serializer zero-extends into the u32
            // word at `+0x00`.
            output[offset..offset + 4]
                .copy_from_slice(&u32::from(effect.prefix_word).to_le_bytes());
            output[offset + 4..offset + 8].copy_from_slice(&effect.effect_id.to_le_bytes());
            output[offset + 8..offset + 12]
                .copy_from_slice(&(effect.resolved_value as u32).to_le_bytes());
            output[offset + 12..offset + 16].copy_from_slice(&metadata.to_le_bytes());
        }
        for index in self.effects.len()..EFFECT_SLOT_COUNT {
            let offset = index * EFFECT_SLOT_STRIDE;
            output[offset + 4..offset + 8].copy_from_slice(&EMPTY_EFFECT_ID.to_le_bytes());
        }
        Ok(output)
    }

    /// Decode the typed preview sequence of a completed rarity-4 record.
    ///
    /// Mirrors `_final_effect_sequence_from_record`: the first five slots are
    /// decoded positionally, `candidate_count` is not carried by the record and
    /// the promoted list holds zero-based slot indexes, exactly as the
    /// reference returns them.
    pub fn from_rarity4_final_record(
        record: &ScrollRecordBytes,
        final_rng_state: u32,
        terminal_is_special: bool,
    ) -> Result<Self, RecordError> {
        let mut effects = Vec::with_capacity(RARITY4_FINAL_EFFECT_COUNT);
        let mut promoted_source_indexes = Vec::new();
        for index in 0..RARITY4_FINAL_EFFECT_COUNT {
            let slot = record.slot(index)?;
            if slot.effect_flags & 0x04 != 0 {
                promoted_source_indexes.push(index as u8);
            }
            effects.push(ScrollEffect {
                slot: (index + 1) as u8,
                source_index: index as u8,
                effect_id: slot.raw_id,
                roll_percent: slot.roll_percent,
                category_and_flags: slot.category_and_flags,
                effect_flags: slot.effect_flags,
                candidate_count: 0,
                resolved_value: slot.value,
                // The recovered slot word is the uint16 group key zero-extended
                // by every serialized record this crate can produce; the
                // reference decode path reads the whole u32.
                prefix_word: slot.prefix_word as u16,
            });
        }
        Ok(Self {
            seed: record.displayed_seed(),
            record_type: record.record_type(),
            rarity: record.rarity(),
            playthrough: crate::sequence::NG3_PLAYTHROUGH,
            level: record.level(),
            effects,
            promoted_source_indexes,
            random_draws: 0,
            final_rng_state,
            terminal_is_special,
        })
    }

    /// Effects between primary and the terminal special slot.
    pub fn secondaries(&self) -> &[ScrollEffect] {
        if self.effects.len() < 2 {
            return &[];
        }
        if self.terminal_is_special {
            &self.effects[1..self.effects.len() - 1]
        } else {
            &self.effects[1..]
        }
    }

    /// Terminal slot: Grace for rarity 5, completion token for rarity 4.
    pub fn terminal(&self) -> Option<&ScrollEffect> {
        self.effects.last()
    }
}

/// Effects the completed rarity-4 preview carries.
pub const RARITY4_FINAL_EFFECT_COUNT: usize = 5;

/// Paired rarity-4 outputs: what may be installed, and what the game completes.
///
/// The two records are separate owned buffers, so neither can alias, overwrite
/// or collapse into the other. Only [`Self::install_record`] is an installation
/// artifact: the game's reveal path runs the native completion pass exactly
/// once, and handing it [`Self::preview_record`] would complete the record a
/// second time. This crate exposes no write path at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rarity4RecordPair {
    install_record: ScrollRecordBytes,
    preview_record: ScrollRecordBytes,
    preview_sequence: ScrollRecord,
    accepted_index: Option<u8>,
    attempts: Vec<FinalizerAttemptTrace>,
}

impl Rarity4RecordPair {
    pub(crate) fn new(
        install_record: ScrollRecordBytes,
        preview_record: ScrollRecordBytes,
        preview_sequence: ScrollRecord,
        accepted_index: Option<u8>,
        attempts: Vec<FinalizerAttemptTrace>,
    ) -> Self {
        Self {
            install_record,
            preview_record,
            preview_sequence,
            accepted_index,
            attempts,
        }
    }

    /// Stage-one record the save must receive.
    pub fn install_record(&self) -> &ScrollRecordBytes {
        &self.install_record
    }

    /// Completed record the native reveal path will produce. Preview only.
    pub fn preview_record(&self) -> &ScrollRecordBytes {
        &self.preview_record
    }

    /// Typed preview of the completed record.
    pub fn preview_sequence(&self) -> &ScrollRecord {
        &self.preview_sequence
    }

    /// Zero-based accepted slot, or `None` when completion changed nothing.
    pub fn accepted_index(&self) -> Option<u8> {
        self.accepted_index
    }

    /// Per-slot attempts in completion order.
    pub fn attempts(&self) -> &[FinalizerAttemptTrace] {
        &self.attempts
    }

    /// Whether the native completion pass changed the installed record.
    pub fn completion_changed_record(&self) -> bool {
        self.install_record != self.preview_record
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(slot: u8) -> ScrollEffect {
        ScrollEffect {
            slot,
            source_index: slot,
            effect_id: 0x1000 + u32::from(slot),
            roll_percent: 90,
            category_and_flags: 0x45,
            effect_flags: 0,
            candidate_count: 1,
            resolved_value: 1,
            prefix_word: 0x4C80,
        }
    }

    #[test]
    fn secondaries_follow_the_terminal_flag() {
        let mut record = ScrollRecord {
            seed: 1,
            record_type: 0xE604,
            rarity: 5,
            playthrough: 3,
            level: 180,
            effects: (1..=6).map(effect).collect(),
            promoted_source_indexes: vec![5],
            random_draws: 24,
            final_rng_state: 0x2FAC1E69,
            terminal_is_special: true,
        };
        assert_eq!(record.primary().unwrap().slot, 1);
        assert_eq!(record.secondaries().len(), 4);
        assert_eq!(record.terminal().unwrap().slot, 6);
        record.terminal_is_special = false;
        assert_eq!(record.secondaries().len(), 5);
        assert_eq!(record.primary().unwrap().category(), 0x05);
    }
}
