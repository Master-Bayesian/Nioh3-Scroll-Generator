//! Candidate identity and install policy.
//!
//! Ports `nioh3_scroll_editor/core_services.py` `candidate_identity` and
//! `OperationPolicy`, plus the `ScrollCandidate.install_blocker` rules the
//! policy reads. These are application semantics: they bind a generated
//! candidate to one generation context and decide whether it may be installed.

use sha2::{Digest, Sha256};

use crate::context::hex_lower;

/// Scroll record size (`emaki_exchange.SCROLL_RECORD_SIZE`).
pub const SCROLL_RECORD_SIZE: usize = 0xE8;

/// Stage of a generated record, mirroring `CandidateRecordStage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordStage {
    FinalRecord,
    NativeStageOne,
    EffectSequenceOnly,
}

impl RecordStage {
    /// Serialized value the contract uses.
    pub fn as_str(self) -> &'static str {
        match self {
            RecordStage::FinalRecord => "final_record",
            RecordStage::NativeStageOne => "native_stage_one",
            RecordStage::EffectSequenceOnly => "effect_sequence_only",
        }
    }
}

/// One effect slot as the contract carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateEffect {
    pub slot: u32,
    pub effect_id: u32,
    pub value: i32,
    pub metadata: u32,
    pub prefix: u32,
    pub tail_0: u32,
    pub tail_1: u32,
    pub roll_percent: Option<u32>,
}

impl CandidateEffect {
    /// `models.ScrollCandidate.from_effect_sequence` repacking.
    ///
    /// Kept next to the identity so the hashed fields and the wire fields can
    /// never be built from different derivations.
    pub fn from_generated(
        slot: u8,
        effect_id: u32,
        value: i32,
        roll_percent: u8,
        category_and_flags: u8,
        effect_flags: u8,
        prefix: u16,
    ) -> Self {
        Self {
            slot: u32::from(slot),
            effect_id,
            value,
            metadata: u32::from(roll_percent)
                | (u32::from(category_and_flags) << 8)
                | (u32::from(effect_flags) << 16),
            prefix: u32::from(prefix),
            tail_0: 0,
            tail_1: 0,
            roll_percent: Some(u32::from(roll_percent)),
        }
    }
}

/// A generated candidate, mirroring the fields identity and policy read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub seed: u32,
    pub playthrough: Option<u8>,
    pub rarity: u8,
    pub record_stage: RecordStage,
    pub record: Vec<u8>,
    pub installation_record: Option<Vec<u8>>,
    pub effects: Vec<CandidateEffect>,
    /// 1-based solver trial this candidate came from, or `None` when the
    /// candidate was not produced by a search job. Mirrors
    /// `ScrollCandidate.joint_search_trial`, which the wire `cursor` field
    /// reports; it is deliberately not part of `candidate_identity`.
    pub joint_search_trial: Option<u64>,
}

impl Candidate {
    /// Slots whose resolved value is still a native intermediate result.
    pub fn unresolved_effect_slots(&self) -> Vec<u32> {
        if self.record_stage != RecordStage::NativeStageOne {
            return Vec::new();
        }
        if self.rarity == 4 {
            return vec![5];
        }
        Vec::new()
    }

    /// Whether a preview can be bound to a live save at install time.
    pub fn can_materialize_for_install(&self) -> bool {
        self.record_stage == RecordStage::EffectSequenceOnly
            && self.playthrough == Some(3)
            && matches!(self.rarity, 3..=5)
    }

    /// Exact port of `ScrollCandidate.install_blocker`.
    pub fn install_blocker(&self) -> Option<String> {
        if matches!(self.playthrough, Some(4) | Some(5)) {
            return Some("四、五周目候选仅供研究预览，禁止通过生成候选安装写入存档。".to_string());
        }
        if let Some(installation_record) = &self.installation_record {
            if installation_record.len() != SCROLL_RECORD_SIZE {
                return Some("候选携带的待揭露记录长度无效，拒绝写入。".to_string());
            }
            let seed = u32::from_le_bytes(
                installation_record[0x20..0x24]
                    .try_into()
                    .expect("record length checked"),
            );
            if seed != self.seed {
                return Some("候选预览与待揭露记录的 Seed 不一致，拒绝写入。".to_string());
            }
            if installation_record[0x30] != self.rarity {
                return Some("候选预览与待揭露记录的稀有度不一致，拒绝写入。".to_string());
            }
            if !self.record.is_empty() && installation_record[..2] != self.record[..2] {
                return Some("候选预览与待揭露记录的绘卷类型不一致，拒绝写入。".to_string());
            }
        }
        if self.record_stage == RecordStage::EffectSequenceOnly {
            if self.can_materialize_for_install() {
                return None;
            }
            return Some(
                "当前候选只包含离线词条序列，而且该周目/稀有度尚未通过完整记录原生一致性门禁，暂不允许写入。"
                    .to_string(),
            );
        }
        if !self.unresolved_effect_slots().is_empty() {
            return Some(
                "当前候选仍是原生中间态，包含尚未完成最终解析的结果码，拒绝写入。".to_string(),
            );
        }
        if self.record_stage == RecordStage::NativeStageOne && self.rarity < 4 {
            return Some(
                "当前低稀有度原生候选尚未通过最终记录一致性验证，暂不允许写入。".to_string(),
            );
        }
        None
    }

    /// `OperationPolicy.evaluate(INSTALL_GENERATED)` reduced to its decision.
    pub fn installable(&self) -> bool {
        self.install_blocker().is_none()
    }
}

/// Exact port of `core_services.candidate_identity`.
///
/// The digest streams the context digest, seed, playthrough, rarity, record
/// stage, both record buffers and every `<7I>` effect tuple in candidate order.
pub fn candidate_identity(candidate: &Candidate, context_digest: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(context_digest.as_bytes());
    digest.update(candidate.seed.to_le_bytes());
    digest.update(i32::from(candidate.playthrough.unwrap_or(0)).to_le_bytes());
    digest.update(i32::from(candidate.rarity).to_le_bytes());
    digest.update(candidate.record_stage.as_str().as_bytes());
    digest.update(&candidate.record);
    digest.update(candidate.installation_record.as_deref().unwrap_or(&[]));
    for effect in &candidate.effects {
        for word in [
            effect.slot,
            effect.effect_id,
            effect.value as u32,
            effect.metadata,
            effect.prefix,
            effect.tail_0,
            effect.tail_1,
        ] {
            digest.update(word.to_le_bytes());
        }
    }
    hex_lower(&digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(
        slot: u32,
        effect_id: u32,
        value: i32,
        metadata: u32,
        prefix: u32,
        roll: u32,
    ) -> CandidateEffect {
        CandidateEffect {
            slot,
            effect_id,
            value,
            metadata,
            prefix,
            tail_0: 0,
            tail_1: 0,
            roll_percent: Some(roll),
        }
    }

    #[test]
    fn identity_matches_the_shipped_preview_oracle() {
        // Captured from the shipped Python worker at the current checkout
        // (`.codex_tmp/m23_worker_probe/oracle.json`), including the real
        // context digest. These rows pin the exact byte stream
        // `candidate_identity` must hash without needing the preview crate.
        const CONTEXT_DIGEST: &str =
            "3a4cf53e9e77729d3a30a13199cbafffe2bfcabcd96540fad3a7ab048be5d6ae";
        let cases = [
            (
                "8a585d15864583b611178a33fdebee3c5fce631c18e10f9dab851280484b1a40",
                6_096_970u32,
                3u8,
                vec![
                    effect(1, 64_494, 57, 17_756, 18_499, 92),
                    effect(2, 41_041, 9, 858, 19_584, 90),
                    effect(3, 45_971, 66, 2_394, 45_912, 90),
                    effect(4, 46_611, 150, 265_542, 60_613, 70),
                    effect(5, 1, 0, 8_653_824, 1, 0),
                ],
            ),
            (
                "f25c1c76a795ab87e296f4e2a5b34987d8ac8af8381fcc370556fd6772c666eb",
                1u32,
                4u8,
                vec![
                    effect(1, 46_611, 150, 281_944, 60_613, 88),
                    effect(2, 17_991, 14, 850, 46_757, 82),
                    effect(3, 54_289, 65, 7_256, 53_953, 88),
                    effect(4, 16_193, 11, 1_628, 31_441, 92),
                    effect(5, 27_311, 150, 265_568, 13_758, 96),
                ],
            ),
            (
                "872d2d066ffbc6f405daa6f59fbf212e83cce853ce650634a52839d48ba060e5",
                226_061_463u32,
                4u8,
                vec![
                    effect(1, 45_955, 56, 17_755, 23_657, 91),
                    effect(2, 20_781, 0, 265_559, 6_068, 87),
                    effect(3, 30_543, 131, 3_157, 48_901, 85),
                    effect(4, 48_209, 94, 6_488, 12_692, 88),
                    effect(5, 20_452, 0, 134_144, 20_973, 0),
                ],
            ),
        ];
        for (expected, seed, rarity, effects) in cases {
            let candidate = Candidate {
                seed,
                playthrough: Some(3),
                rarity,
                record_stage: RecordStage::EffectSequenceOnly,
                record: Vec::new(),
                installation_record: None,
                effects,
                joint_search_trial: None,
            };
            assert_eq!(
                candidate_identity(&candidate, CONTEXT_DIGEST),
                expected,
                "seed {seed} rarity {rarity}"
            );
        }
    }

    #[test]
    fn install_policy_matches_the_effect_sequence_preview() {
        let candidate = Candidate {
            seed: 1,
            playthrough: Some(3),
            rarity: 4,
            record_stage: RecordStage::EffectSequenceOnly,
            record: Vec::new(),
            installation_record: None,
            effects: Vec::new(),
            joint_search_trial: None,
        };
        assert!(candidate.installable());
        assert_eq!(candidate.install_blocker(), None);

        let ng4 = Candidate {
            playthrough: Some(4),
            ..candidate.clone()
        };
        assert!(!ng4.installable());
    }

    #[test]
    fn native_stage_one_rarity_four_keeps_the_unresolved_slot() {
        let candidate = Candidate {
            seed: 1,
            playthrough: Some(3),
            rarity: 4,
            record_stage: RecordStage::NativeStageOne,
            record: vec![0u8; SCROLL_RECORD_SIZE],
            installation_record: None,
            effects: Vec::new(),
            joint_search_trial: None,
        };
        assert_eq!(candidate.unresolved_effect_slots(), vec![5]);
        assert!(!candidate.installable());
    }
}
