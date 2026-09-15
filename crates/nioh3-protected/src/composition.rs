//! The concrete `EffectComposition` the protected host injects.
//!
//! The runtime crate deliberately carries no RNG, no finalizer and no product
//! tables: `mutation::catalog::EffectComposition` is a seam, and its default
//! refuses. This module is the production wiring for that seam, and it lives in
//! the protected host because the host is the crate that legitimately depends on
//! the domain generators, the record materializers and the product tables.
//!
//! Two save-side facts cannot be derived from a typed candidate and are injected
//! instead: the playthrough template the installation record must inherit and
//! the save-wide generation serial it must carry. Until both are bound the
//! composition returns `Ok(None)`, which the policy reports as the shipped
//! effect-sequence refusal rather than inventing a record.
//!
//! The value handed back is the *installation* record. For rarity 4 that is the
//! stage-one record the game completes exactly once on reveal; the completed
//! record stays inside the domain pair and is never installed, so the preview
//! and the write can never collapse into one artifact.

use std::path::{Path, PathBuf};

use nioh3_domain::effect::{EffectResourceBytes, EffectTableIndex};
use nioh3_domain::install_materialize::materialize_ng3_certified_install_record;
use nioh3_domain::record::ScrollRecordBytes;
use nioh3_runtime::mutation::catalog::{can_materialize_for_install, EffectComposition};
use nioh3_runtime::mutation::live_add::InstallationCandidate;
use nioh3_runtime::RuntimeError;

/// The save-side facts one composition binds the produced record to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallBinding {
    /// The playthrough template the installation record must inherit.
    pub template: Vec<u8>,
    /// The save-wide unique generation serial the record must carry.
    pub generation_serial: u32,
    /// The recommended level the record must carry.
    pub recommended_level: u16,
    /// The transfer count the record must carry.
    pub transfer_count: u32,
}

/// The concrete [`EffectComposition`] over `nioh3-domain` and `nioh3-data`.
///
/// This type routes and does not compute: the generators, the record
/// materializers and the R4 finalizer stay in `nioh3-domain`, and the product
/// tables and measured Grace maps stay in `nioh3-data`. It fails closed — an
/// unbound or unloaded composition returns `Ok(None)`.
pub struct DomainEffectComposition {
    data_root: PathBuf,
    resources: Option<ComposedResources>,
    binding: Option<InstallBinding>,
    last_composition: Option<Vec<u8>>,
}

struct ComposedResources {
    index: EffectTableIndex,
    effect: EffectResourceBytes,
}

impl DomainEffectComposition {
    /// Build the composition over one product data root; tables load on first
    /// use so construction never touches the disk.
    pub fn load(data_root: &Path) -> Self {
        Self {
            data_root: data_root.to_path_buf(),
            resources: None,
            binding: None,
            last_composition: None,
        }
    }

    /// Bind the save-side template and serial the next composition uses.
    pub fn bind(&mut self, binding: InstallBinding) {
        self.binding = Some(binding);
    }

    /// The record the most recent successful composition produced.
    pub fn last_composition(&self) -> Option<&[u8]> {
        self.last_composition.as_deref()
    }

    fn resources(&mut self) -> Result<&ComposedResources, RuntimeError> {
        if self.resources.is_none() {
            let effect = nioh3_data::load_effect_resource(&self.data_root).map_err(|error| {
                RuntimeError::InventoryInvalid {
                    detail: format!("effect resource is unavailable: {error}"),
                }
            })?;
            let index = EffectTableIndex::from_resource(&effect).map_err(|error| {
                RuntimeError::InventoryInvalid {
                    detail: format!("effect table index rejected the resource: {error:?}"),
                }
            })?;
            self.resources = Some(ComposedResources { index, effect });
        }
        self.resources
            .as_ref()
            .ok_or_else(|| RuntimeError::InventoryInvalid {
                detail: "effect resource is unavailable".to_string(),
            })
    }
}

impl EffectComposition for DomainEffectComposition {
    fn compose(
        &mut self,
        candidate: &InstallationCandidate,
    ) -> Result<Option<Vec<u8>>, RuntimeError> {
        if !can_materialize_for_install(candidate.stage, candidate.playthrough, candidate.rarity) {
            return Ok(None);
        }
        let Some(binding) = self.binding.clone() else {
            // Fail closed: without a template and a serial there is nothing
            // this build can honestly bind the sequence to.
            return Ok(None);
        };
        let level = u16::try_from(candidate.level).map_err(|_| RuntimeError::InventoryInvalid {
            detail: "candidate level does not fit the record field".to_string(),
        })?;
        let resources = self.resources()?;
        let grace_map = if candidate.rarity == 5 {
            resources.effect.grace_maps.get(1)
        } else {
            resources.effect.grace_maps.first()
        }
        .ok_or_else(|| RuntimeError::InventoryInvalid {
            detail: "the shipped Grace maps are unavailable".to_string(),
        })?;
        let template = ScrollRecordBytes::from_slice(&binding.template).map_err(|_| {
            RuntimeError::InventoryInvalid {
                detail: "Expected one canonical scroll installation record".to_string(),
            }
        })?;
        let (install_record, _completed) = materialize_ng3_certified_install_record(
            &resources.index,
            grace_map,
            &template,
            candidate.rarity,
            candidate.seed,
            level,
            binding.recommended_level,
            binding.generation_serial,
            binding.transfer_count,
        )
        .map_err(|error| RuntimeError::InventoryInvalid {
            detail: error.to_string(),
        })?;
        let record = install_record.into_bytes().to_vec();
        self.last_composition = Some(record.clone());
        Ok(Some(record))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use nioh3_runtime::mutation::catalog::{DomainCatalogPolicy, EFFECT_SEQUENCE_REFUSAL};
    use nioh3_runtime::mutation::inventory::RECORD_SIZE;
    use nioh3_runtime::mutation::live_add::{CandidateEffect, CandidateStage};

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

    /// The producer is a lazy, injected binding. Until the save side supplies
    /// the template and serial it must refuse exactly like the fail-closed
    /// default instead of composing a record from nothing, and it must not read
    /// the product data root to decide that.
    #[test]
    fn an_unbound_composition_refuses_instead_of_inventing_a_record() {
        let mut policy = DomainCatalogPolicy::with_composition(DomainEffectComposition::load(
            Path::new("no-such-data-root"),
        ));
        for rarity in [3u8, 4, 5] {
            let refused = policy
                .blocker(&candidate(
                    CandidateStage::EffectSequenceOnly,
                    rarity,
                    Some(3),
                ))
                .unwrap_or(None);
            assert_eq!(refused.as_deref(), Some(EFFECT_SEQUENCE_REFUSAL));
        }
        assert_eq!(policy.last_composition(), None);
        // A candidate the certified gate already rejects never reaches the
        // composition at all.
        let wrong_playthrough = policy
            .blocker(&candidate(CandidateStage::EffectSequenceOnly, 4, Some(2)))
            .unwrap_or(None);
        assert_eq!(wrong_playthrough.as_deref(), Some(EFFECT_SEQUENCE_REFUSAL));
    }
}
