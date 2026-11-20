//! Per-occurrence MAIN input assembly over a resolved candidate/source state.
//! This is a common execution boundary, not runtime contact or source discovery.

use std::collections::BTreeMap;

pub type Word = (u64, u32);
type Result<T> = std::result::Result<T, &'static str>;

fn valid_word((address, _): Word) -> Result<()> {
    if address == 0 || address.checked_add(3).is_none() {
        return Err("word address is null or overflows");
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceIdentity {
    pub owner: u64,
    pub incarnation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextHitPhase {
    Active,
    RemovedPendingRebuild,
    Inactive,
}

#[derive(Clone, Copy, Debug)]
pub struct Settlement {
    pub landed: bool,
    pub source_present: bool,
    pub hp_owner_present: bool,
    pub packet_flags_296_298: [u8; 3],
    pub actor_event_241: bool,
    pub physical_motion: i16,
    pub elemental_motion: i16,
}

impl NextHitPhase {
    /// Original327DEC/207CBAC removes membership; class54 F8 teardown is empty.
    pub fn after_settlement(self, event: Settlement) -> Self {
        if self == Self::Active
            && event.landed
            && event.source_present
            && event.hp_owner_present
            && event.packet_flags_296_298 == [0; 3]
            && !event.actor_event_241
            && (event.physical_motion > 0 || event.elemental_motion > 0)
        {
            Self::RemovedPendingRebuild
        } else {
            self
        }
    }
    pub fn after_rebuild(self) -> Self {
        if self == Self::RemovedPendingRebuild {
            Self::Inactive
        } else {
            self
        }
    }
}

#[derive(Clone, Debug)]
pub struct SingleUseBinding {
    pub source: SourceIdentity,
    pub scalar_addresses: [u64; 2],
    pub active_bits: [u32; 2],
    pub rebuilt_bits: [u32; 2],
    pub active_membership: Vec<Word>,
    pub removed_membership: Vec<Word>,
}

fn disjoint(words: &[Word]) -> Result<std::collections::BTreeSet<u64>> {
    let mut bytes = std::collections::BTreeSet::new();
    for &(address, value) in words {
        valid_word((address, value))?;
        for byte in address..=address + 3 {
            if !bytes.insert(byte) {
                return Err("overlapping status snapshot words");
            }
        }
    }
    Ok(bytes)
}

impl SingleUseBinding {
    fn validate(&self) -> Result<()> {
        if self.source.owner == 0 || self.source.incarnation == 0 {
            return Err("status source incarnation is missing");
        }
        let scalar: Vec<_> = self
            .scalar_addresses
            .into_iter()
            .zip(self.active_bits)
            .collect();
        let scalar = disjoint(&scalar)?;
        if self
            .active_bits
            .iter()
            .chain(&self.rebuilt_bits)
            .any(|&x| !f32::from_bits(x).is_finite())
        {
            return Err("nonfinite materialized status word");
        }
        let active = disjoint(&self.active_membership)?;
        let removed = disjoint(&self.removed_membership)?;
        if active != removed || !active.is_disjoint(&scalar) {
            return Err("membership layouts differ or overlap status scalars");
        }
        Ok(())
    }

    /// Both materialized snapshots must be independently bound. Never divide
    /// the active aggregate by 1.06 to reconstruct other status contributions.
    pub fn words(&self, phase: NextHitPhase) -> Result<Vec<Word>> {
        self.validate()?;
        let scalars = if phase == NextHitPhase::Inactive {
            self.rebuilt_bits
        } else {
            self.active_bits
        };
        let membership = if phase == NextHitPhase::Active {
            &self.active_membership
        } else {
            &self.removed_membership
        };
        let mut words: Vec<_> = self.scalar_addresses.into_iter().zip(scalars).collect();
        words.extend_from_slice(membership);
        Ok(words)
    }
}

#[derive(Clone, Debug)]
pub struct ActorInputs {
    pub source: SourceIdentity,
    pub epoch: u64,
    pub candidate_words: Vec<Word>,
    pub single_use: Option<SingleUseBinding>,
}

impl ActorInputs {
    pub fn rebuilt(&self, epoch: u64, candidate_words: Vec<Word>) -> Result<Self> {
        if self.epoch.checked_add(1) != Some(epoch) {
            return Err("rebuild must advance one epoch");
        }
        let result = Self {
            epoch,
            candidate_words,
            ..self.clone()
        };
        result.validate()?;
        Ok(result)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.source.owner == 0 || self.source.incarnation == 0 {
            return Err("source owner/incarnation is missing");
        }
        for &word in &self.candidate_words {
            valid_word(word)?;
        }
        if let Some(binding) = &self.single_use {
            binding.validate()?;
            if binding.source != self.source {
                return Err("status binding belongs to another source");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WordPatch {
    pub address: u64,
    pub value: u32,
    pub mask: u32,
}
impl WordPatch {
    pub fn full(address: u64, value: u32) -> Self {
        Self {
            address,
            value,
            mask: u32::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum PacketPolicy {
    PreparedActor,
    /// Native2B55F1..2B562C ->2B5752, only descriptor type0. The source
    /// tag/owner are explicit bindings; no actor Hell/imbue inheritance.
    UnimbuedOwnerProjectile {
        main: u64,
        descriptor_kind: u8,
        projectile: u64,
        projectile_incarnation: u64,
        source_tag: u32,
        power: u32,
        main_flags: u32,
    },
}

#[derive(Clone, Debug)]
pub struct EventInput<'a> {
    pub occurrence: &'a str,
    pub source: SourceIdentity,
    pub epoch: u64,
    pub patches: &'a [WordPatch],
    pub packet: PacketPolicy,
    pub landed: Option<bool>,
    pub guard: Option<bool>,
    pub runtime_extensions_closed: bool,
}

#[derive(Clone, Debug)]
pub struct BuiltEvent {
    pub occurrence: String,
    pub source: SourceIdentity,
    pub epoch: u64,
    pub words: Vec<Word>,
    pub next_hit_phase: NextHitPhase,
    pub projectile: Option<u64>,
    pub projectile_incarnation: Option<u64>,
    pub source_tag: Option<u32>,
}

/// Fresh candidate -> event patches -> current status -> packet policy.
/// The reader must reject unknown baseline bytes. Partial writes preserve
/// bytes supplied by earlier overlapping words in their original order.
pub fn assemble_event_inputs(
    actor: &ActorInputs,
    event: EventInput<'_>,
    phase: NextHitPhase,
    mut read_word: impl FnMut(u64) -> Result<u32>,
) -> Result<Option<BuiltEvent>> {
    let mut words = Vec::new();
    let admitted = assemble_event_words(
        actor.source,
        actor.epoch,
        &actor.candidate_words,
        actor.single_use.as_ref(),
        &event,
        phase,
        &mut words,
        &mut read_word,
    )?;
    if !admitted {
        return Ok(None);
    }
    let (projectile, projectile_incarnation, source_tag) = match event.packet {
        PacketPolicy::PreparedActor => (None, None, None),
        PacketPolicy::UnimbuedOwnerProjectile {
            projectile,
            projectile_incarnation,
            source_tag,
            ..
        } => (
            Some(projectile),
            Some(projectile_incarnation),
            Some(source_tag),
        ),
    };
    Ok(Some(BuiltEvent {
        occurrence: event.occurrence.into(),
        source: actor.source,
        epoch: actor.epoch,
        words,
        next_hit_phase: phase,
        projectile,
        projectile_incarnation,
        source_tag,
    }))
}

/// The production evaluator shares this assembly boundary without cloning the
/// candidate into a temporary ActorInputs for every occurrence. The destination
/// is always cleared, including rejected/missed contacts.
#[allow(clippy::too_many_arguments)]
pub fn assemble_event_words(
    source: SourceIdentity,
    epoch: u64,
    candidate_words: &[Word],
    single_use: Option<&SingleUseBinding>,
    event: &EventInput<'_>,
    phase: NextHitPhase,
    words: &mut Vec<Word>,
    mut read_word: impl FnMut(u64) -> Result<u32>,
) -> Result<bool> {
    words.clear();
    if source.owner == 0 || source.incarnation == 0 {
        return Err("source owner/incarnation is missing");
    }
    for &word in candidate_words {
        valid_word(word)?;
    }
    if let Some(binding) = single_use {
        if binding.source != source {
            return Err("status binding belongs to another source");
        }
        binding.validate()?;
    }
    if event.source != source || event.epoch != epoch {
        return Err("event source or actor epoch differs");
    }
    if event.occurrence.is_empty() || !event.runtime_extensions_closed {
        return Err("event occurrence/runtime extension binding is missing");
    }
    let (Some(landed), Some(guard)) = (event.landed, event.guard) else {
        return Err("unknown landed or guard decision");
    };
    if landed && !guard {
        return Err("false guard cannot yield a landed contact");
    }
    if !landed {
        return Ok(false);
    }
    let packet = match event.packet {
        PacketPolicy::PreparedActor => Vec::new(),
        PacketPolicy::UnimbuedOwnerProjectile {
            main,
            descriptor_kind,
            projectile,
            projectile_incarnation,
            source_tag,
            power,
            main_flags,
        } => {
            if descriptor_kind != 0 || projectile == 0 || projectile_incarnation == 0 {
                return Err("unbound projectile packet producer");
            }
            if main == 0 || main.checked_add(0x29B).is_none() {
                return Err("MAIN address is null or overflows");
            }
            let _ = source_tag;
            vec![
                (main + 0x14, u32::MAX),
                (main + 0x18, power),
                (main + 0x298, main_flags),
            ]
        }
    };
    words.extend_from_slice(candidate_words);
    // Dense production templates own complete words. Avoid constructing a
    // byte map for that common path; the masked path retains byte precedence.
    if event.patches.iter().all(|patch| patch.mask == u32::MAX) {
        for patch in event.patches {
            valid_word((patch.address, patch.value))?;
            words.push((patch.address, patch.value));
        }
        if let Some(status) = single_use {
            words.extend(status.words(phase)?);
        }
        words.extend(packet);
        return Ok(true);
    }
    let mut bytes = BTreeMap::<u64, u8>::new();
    let store_bytes = |bytes: &mut BTreeMap<u64, u8>, address: u64, value: u32| {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            bytes.insert(address + i as u64, byte);
        }
    };
    for &(address, value) in words.iter() {
        store_bytes(&mut bytes, address, value);
    }
    for patch in event.patches {
        valid_word((patch.address, patch.value))?;
        if patch.value & !patch.mask != 0 {
            return Err("masked patch exceeds its ownership mask");
        }
        let value = if patch.mask == u32::MAX {
            patch.value
        } else {
            let mut before = [0; 4];
            let missing = (0..4).any(|i| !bytes.contains_key(&(patch.address + i)));
            let baseline = if missing {
                read_word(patch.address)?.to_le_bytes()
            } else {
                [0; 4]
            };
            for i in 0..4 {
                before[i] = *bytes
                    .get(&(patch.address + i as u64))
                    .unwrap_or(&baseline[i]);
            }
            (u32::from_le_bytes(before) & !patch.mask) | patch.value
        };
        words.push((patch.address, value));
        store_bytes(&mut bytes, patch.address, value);
    }
    if let Some(status) = single_use {
        words.extend(status.words(phase)?);
    }
    words.extend(packet);
    Ok(true)
}

#[cfg(test)]
#[path = "event_inputs_tests.rs"]
mod tests;
