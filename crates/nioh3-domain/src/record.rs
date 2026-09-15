//! Typed scroll and effect records.
//!
//! Field names and integer widths mirror `nioh3_scroll_editor/models.py` and the
//! reference effect-sequence result at the M2.1 baseline. This module carries no
//! digests, identity hashing or serialization: the `GenerationContext` binding
//! remains a Python-side contract until the adapter/worker slice.

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
