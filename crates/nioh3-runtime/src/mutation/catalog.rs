//! The concrete [`crate::mutation::live_add::CatalogPolicy`].
//!
//! `models.ScrollCandidate.install_blocker` has two branches that need the
//! effect catalog rather than the record shape, and they are the two this module
//! owns:
//!
//! * an `effect_sequence_only` candidate may only be installed when the shipped
//!   gate accepts its playthrough and rarity *and* the offline effect sequence
//!   can be turned into a complete installation record;
//! * a rarity-4 `native_stage_one` candidate always carries an unresolved
//!   terminal slot, because the observed resolution list is incomplete by
//!   definition and an unknown token is not evidence that finalization is
//!   unnecessary.
//!
//! `install_blocker` returns its refusal texts verbatim; the parity gate
//! compares them with the shipped implementation over a matrix of candidates.

use crate::error::RuntimeError;
use crate::mutation::inventory::RECORD_SIZE;
use crate::mutation::live_add::{CandidateStage, CatalogPolicy, InstallationCandidate};

/// `emaki_exchange.EFFECT_START`.
pub const EFFECT_START: usize = 0x34;
/// `emaki_exchange.EFFECT_STRIDE`.
pub const EFFECT_STRIDE: usize = 0x18;
/// The seven effect slots a scroll record carries.
pub const EFFECT_COUNT: usize = 7;

/// The shipped refusal for an effect-only candidate that cannot be bound.
pub const EFFECT_SEQUENCE_REFUSAL: &str =
    "当前候选只包含离线词条序列，而且该周目/稀有度尚未通过完整记录原生一致性门禁，\
暂不允许写入。";
/// The shipped refusal for a rarity-4 native intermediate record.
pub const UNRESOLVED_SLOT_REFUSAL: &str =
    "当前候选仍是原生中间态，包含尚未完成最终解析的结果码，拒绝写入。";

/// One effect slot as the record carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecordEffectSlot {
    pub prefix: u32,
    pub effect_id: u32,
    pub value: u32,
    pub metadata: u32,
    /// `record[offset + 0x0D]`.
    pub flag_0: u8,
    /// `record[offset + 0x0E]`.
    pub flag_1: u8,
}

impl RecordEffectSlot {
    /// The shipped "this slot still has content" test.
    pub fn is_present(&self) -> bool {
        self.effect_id != 0
    }

    /// `live_add_native_transport`'s eligibility for slot 5.
    pub fn completion_loop_eligible(&self) -> bool {
        self.is_present() && self.flag_0 & 0x40 == 0 && self.flag_1 & 0x04 == 0
    }

    /// The promoted/completed bit the native completion loop sets.
    pub fn is_completed(&self) -> bool {
        self.flag_1 & 0x04 != 0
    }
}

/// Parse the seven effect slots out of a canonical record.
pub fn record_effect_slots(
    record: &[u8],
) -> Result<[RecordEffectSlot; EFFECT_COUNT], RuntimeError> {
    if record.len() != RECORD_SIZE {
        return Err(RuntimeError::InventoryInvalid {
            detail: "Expected one canonical scroll installation record".to_string(),
        });
    }
    let mut slots = [RecordEffectSlot::default(); EFFECT_COUNT];
    for (index, slot) in slots.iter_mut().enumerate() {
        let start = EFFECT_START + index * EFFECT_STRIDE;
        let word = |offset: usize| {
            u32::from_le_bytes([
                record[start + offset],
                record[start + offset + 1],
                record[start + offset + 2],
                record[start + offset + 3],
            ])
        };
        *slot = RecordEffectSlot {
            prefix: word(0),
            effect_id: word(4),
            value: word(8),
            metadata: word(0x0C),
            flag_0: record[start + 0x0D],
            flag_1: record[start + 0x0E],
        };
    }
    Ok(slots)
}

/// The slots a record still has to resolve before it may be installed.
///
/// The shipped rule is deliberately blunt for rarity 4: the observed list of
/// resolved tokens is incomplete, so the terminal slot is always unresolved
/// until the game itself finalizes the record.
pub fn unresolved_effect_slots(
    record: &[u8],
    rarity: u8,
    native_stage_one: bool,
) -> Result<Vec<usize>, RuntimeError> {
    if !native_stage_one || rarity != 4 {
        return Ok(Vec::new());
    }
    let slots = record_effect_slots(record)?;
    let unresolved: Vec<usize> = slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.is_present() && !slot.is_completed())
        .map(|(index, _)| index + 1)
        .collect();
    if unresolved.is_empty() {
        // Fail closed: the shipped rule names slot 5 for every rarity-4 native
        // result, so an apparently complete record is still refused.
        Ok(vec![5])
    } else {
        Ok(unresolved)
    }
}

/// The typed composition source an `effect_sequence_only` candidate needs.
///
/// The runtime crate carries no RNG and no finalizer, so it cannot invent a
/// record from an effect sequence. The producer contract is one value: given the
/// typed transfer, return the complete installation record the game would have
/// produced, or `None` when this build has not certified that composition.
/// Production wiring injects the domain implementation; [`RefusingComposition`]
/// is the fail-closed default.
pub trait EffectComposition {
    fn compose(
        &mut self,
        candidate: &InstallationCandidate,
    ) -> Result<Option<Vec<u8>>, RuntimeError>;
}

/// The default: no composition is available, so the branch refuses.
#[derive(Debug, Clone, Copy, Default)]
pub struct RefusingComposition;

impl EffectComposition for RefusingComposition {
    fn compose(
        &mut self,
        _candidate: &InstallationCandidate,
    ) -> Result<Option<Vec<u8>>, RuntimeError> {
        Ok(None)
    }
}

/// `models.ScrollCandidate.can_materialize_for_install`.
pub fn can_materialize_for_install(
    stage: CandidateStage,
    playthrough: Option<u32>,
    rarity: u8,
) -> bool {
    stage == CandidateStage::EffectSequenceOnly && playthrough == Some(3) && matches!(rarity, 3..=5)
}

/// The concrete catalog policy.
pub struct DomainCatalogPolicy<C: EffectComposition = RefusingComposition> {
    composition: C,
    /// The record a successful composition produced, kept so the caller can
    /// publish the value the runtime actually installed.
    last_composition: Option<Vec<u8>>,
}

impl Default for DomainCatalogPolicy<RefusingComposition> {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainCatalogPolicy<RefusingComposition> {
    pub fn new() -> Self {
        Self {
            composition: RefusingComposition,
            last_composition: None,
        }
    }
}

impl<C: EffectComposition> DomainCatalogPolicy<C> {
    pub fn with_composition(composition: C) -> Self {
        Self {
            composition,
            last_composition: None,
        }
    }

    /// The record the most recent successful composition produced.
    pub fn last_composition(&self) -> Option<&[u8]> {
        self.last_composition.as_deref()
    }

    /// `install_blocker`'s catalog branches, with the shipped text.
    pub fn blocker(
        &mut self,
        candidate: &InstallationCandidate,
    ) -> Result<Option<String>, RuntimeError> {
        if candidate.stage == CandidateStage::EffectSequenceOnly {
            if !can_materialize_for_install(
                candidate.stage,
                candidate.playthrough,
                candidate.rarity,
            ) {
                return Ok(Some(EFFECT_SEQUENCE_REFUSAL.to_string()));
            }
            return match self.composition.compose(candidate)? {
                Some(record) => {
                    self.last_composition = Some(record);
                    Ok(None)
                }
                None => Ok(Some(EFFECT_SEQUENCE_REFUSAL.to_string())),
            };
        }
        if candidate.stage == CandidateStage::NativeStageOne {
            let record = if candidate.installation_record.is_some() {
                candidate.install_record()
            } else {
                candidate.record.as_slice()
            };
            if record.len() != RECORD_SIZE {
                return Ok(None);
            }
            let unresolved = unresolved_effect_slots(record, candidate.rarity, true)?;
            return Ok(if unresolved.is_empty() {
                None
            } else {
                Some(UNRESOLVED_SLOT_REFUSAL.to_string())
            });
        }
        Ok(None)
    }
}

impl<C: EffectComposition> CatalogPolicy for DomainCatalogPolicy<C> {
    fn catalog_blocker(
        &mut self,
        candidate: &InstallationCandidate,
    ) -> Result<Option<String>, RuntimeError> {
        self.blocker(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutation::live_add::CandidateEffect;

    fn candidate(
        stage: CandidateStage,
        rarity: u8,
        playthrough: Option<u32>,
    ) -> InstallationCandidate {
        InstallationCandidate {
            candidate_id: "candidate".to_string(),
            context_digest: "0".repeat(64),
            level: 1,
            seed: 0x1234,
            playthrough,
            rarity,
            stage,
            record: vec![0u8; RECORD_SIZE],
            installation_record: None,
            effects: vec![CandidateEffect {
                slot: 1,
                effect_id: 0x100,
                value: 1,
                metadata: 0,
                prefix: 0,
                tail_0: 0,
                tail_1: 0,
            }],
        }
    }

    #[test]
    fn an_effect_only_candidate_is_refused_without_a_composition() {
        let mut policy = DomainCatalogPolicy::new();
        let refused = policy
            .blocker(&candidate(CandidateStage::EffectSequenceOnly, 4, Some(3)))
            .unwrap_or(None);
        assert_eq!(refused.as_deref(), Some(EFFECT_SEQUENCE_REFUSAL));
        let wrong_context = policy
            .blocker(&candidate(CandidateStage::EffectSequenceOnly, 4, Some(2)))
            .unwrap_or(None);
        assert_eq!(wrong_context.as_deref(), Some(EFFECT_SEQUENCE_REFUSAL));
    }

    struct Stub;

    impl EffectComposition for Stub {
        fn compose(
            &mut self,
            candidate: &InstallationCandidate,
        ) -> Result<Option<Vec<u8>>, RuntimeError> {
            Ok(Some(vec![candidate.rarity; RECORD_SIZE]))
        }
    }

    #[test]
    fn an_effect_only_candidate_is_accepted_when_the_composition_lands() {
        let mut policy = DomainCatalogPolicy::with_composition(Stub);
        let allowed = policy
            .blocker(&candidate(CandidateStage::EffectSequenceOnly, 5, Some(3)))
            .unwrap_or(None);
        assert_eq!(allowed, None);
        assert_eq!(
            policy.last_composition().map(<[u8]>::len),
            Some(RECORD_SIZE)
        );
    }

    #[test]
    fn a_final_record_candidate_needs_no_catalog_decision() {
        let mut policy = DomainCatalogPolicy::new();
        let allowed = policy
            .blocker(&candidate(CandidateStage::FinalRecord, 4, Some(2)))
            .unwrap_or(None);
        assert_eq!(allowed, None);
    }

    #[test]
    fn a_rarity_four_native_stage_one_is_always_unresolved() {
        let mut policy = DomainCatalogPolicy::new();
        let refused = policy
            .blocker(&candidate(CandidateStage::NativeStageOne, 4, Some(2)))
            .unwrap_or(None);
        assert_eq!(refused.as_deref(), Some(UNRESOLVED_SLOT_REFUSAL));
        assert_eq!(
            unresolved_effect_slots(&[0u8; RECORD_SIZE], 4, true).unwrap_or_default(),
            vec![5]
        );
        assert!(unresolved_effect_slots(&[0u8; RECORD_SIZE], 3, true)
            .unwrap_or_default()
            .is_empty());
    }

    #[test]
    fn the_record_predicates_match_the_shipped_slot_rules() {
        let mut record = vec![0u8; RECORD_SIZE];
        let start = EFFECT_START + 4 * EFFECT_STRIDE;
        record[start + 4] = 0x21;
        record[start + 5] = 0x01;
        let slots = record_effect_slots(&record).unwrap_or([RecordEffectSlot::default(); 7]);
        assert!(slots[4].is_present());
        assert!(slots[4].completion_loop_eligible());
        assert!(!slots[4].is_completed());
        assert!(!slots[0].is_present());
    }

    /// The fail-closed default stays the crate's default composition.
    #[test]
    fn the_default_policy_still_uses_the_refusing_composition() {
        let mut policy = DomainCatalogPolicy::default();
        let refused = policy
            .blocker(&candidate(CandidateStage::EffectSequenceOnly, 4, Some(3)))
            .unwrap_or(None);
        assert_eq!(refused.as_deref(), Some(EFFECT_SEQUENCE_REFUSAL));
        assert_eq!(policy.last_composition(), None);
    }
}
