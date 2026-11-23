//! Immutable, source-bound event plans consumed by the production Engine.
//! Catalog definitions are deliberately not a landing/order oracle. A plan
//! supplies those scenario decisions and binds them to exact dense templates.
use crate::pipeline::{Frame, HitPacket, PacketKind};
use crate::damage_dealt::TriggeredDamage;
use crate::sequence::inputs::{
    assemble_event_words, EventInput, NextHitPhase, PacketPolicy, Settlement, SingleUseBinding,
    SourceIdentity, Word, WordPatch,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Active,
    RemovedPendingRebuild,
    Inactive,
}

impl From<Phase> for NextHitPhase {
    fn from(value: Phase) -> Self {
        match value {
            Phase::Active => Self::Active,
            Phase::RemovedPendingRebuild => Self::RemovedPendingRebuild,
            Phase::Inactive => Self::Inactive,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    owner: u64,
    incarnation: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Packet {
    PreparedActor,
    OwnerWeaponProjectile {
        object: u64,
        incarnation: u64,
        owner: u64,
        descriptor_kind: u8,
        source_tag: u32,
    },
    UnimbuedOwnerProjectile {
        object: u64,
        incarnation: u64,
        owner: u64,
        descriptor_kind: u8,
        source_tag: u32,
        power: u32,
        main_flags: u32,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settle {
    source_present: bool,
    hp_owner_present: bool,
    packet_flags_296_298: [u8; 3],
    actor_event_241: bool,
    physical_motion: i16,
    elemental_motion: i16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceIdentity {
    resource: String,
    action: u16,
    parameter_sha256: String,
}

impl ResourceIdentity {
    fn validate(&self) -> Result<(), &'static str> {
        if self.resource.is_empty()
            || self.parameter_sha256.len() != 64
            || !self
                .parameter_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("invalid resource/action/parameter identity");
        }
        // The row's action is not necessarily its spawning action. Keep this
        // typed identity separate from the caller's action parameter.
        let _ = self.action;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Template {
    id: String,
    landed: Option<bool>,
    guard: Option<bool>,
    packet: Packet,
    #[serde(default)]
    rebuild_before: bool,
    #[serde(default = "yes")]
    scored: bool,
    settlement: Option<Settle>,
    /// Original settlement eligibility for the projected one-hit grant.
    #[serde(default)]
    peak_consumes_follow_up: Option<bool>,
    #[serde(default)]
    peak_allows_coating: Option<bool>,
    #[serde(default)]
    peak_actor_arts: Option<crate::arts_proficiency::PreparedActorRoute>,
    // Resource identity remains a triple; action numbers alone are not keys.
    identity: Option<ResourceIdentity>,
}
fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    scalar_addresses: [u64; 2],
    active_bits: [u32; 2],
    rebuilt_bits: [u32; 2],
    active_membership: Vec<Word>,
    removed_membership: Vec<Word>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Boundary {
    Recovery { amount_bits: u32 },
    Rebuild,
    Stop { reason: StopReason },
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StopReason {
    Released,
    Interrupted,
    Exhausted,
    GuardFalse,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Cost {
    /// Explicitly resolved cost multiplier, not an equipment-invariant inference.
    ResolvedMultiplier { bits: u32 },
    /// Native ordinary Ninja Reaper branch, rebuilt from this candidate STAT/D.
    OrdinaryNinjaReaper,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContactBoundary {
    // Position in this action's ordered contact occurrences, not a template ID.
    before_contact: usize,
    boundaries: Vec<Boundary>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    resource: String,
    action: u16,
    parameter_hex: String,
    parameter_sha256: String,
    contacts: Vec<usize>,
    cost: Cost,
    #[serde(default)]
    before: Vec<Boundary>,
    #[serde(default)]
    contact_boundaries: Vec<ContactBoundary>,
    #[serde(skip)]
    base_cost: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Mode {
    FixedContacts,
    HeldReaper,
    HeldWaterSword {
        #[serde(alias = "current_effect", deserialize_with = "crate::arts_proficiency::read_start")]
        arts: crate::arts_proficiency::Start,
        #[serde(default)]
        objective: crate::water_sword::Objective,
    },
    FiniteSequence {
        actions: Vec<Action>,
        #[serde(default)]
        initial_ki_bits: Option<u32>,
        #[serde(default)]
        ki_address: Option<u64>,
        #[serde(default)]
        debit_address: Option<u64>,
    },
    CyclicSequence {
        entry: Vec<Action>,
        cycle: Vec<Action>,
        max_cycles: Option<u32>,
        #[serde(default)]
        initial_ki_bits: Option<u32>,
        #[serde(default)]
        ki_address: Option<u64>,
        #[serde(default)]
        debit_address: Option<u64>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema: String,
    source: Source,
    initial_epoch: u64,
    initial_phase: Phase,
    main_address: u64,
    #[serde(default)]
    actor_skill_key: Option<u16>,
    #[serde(default)]
    direct_owner_element: bool,
    #[serde(default)]
    actor_damage_kind_from_row: bool,
    #[serde(default)]
    initial_projectile: Option<crate::action_rows::InitialProjectile>,
    #[serde(default)]
    candidate_source_address: Option<u64>,
    #[serde(default)]
    candidate_category_field: Option<u64>,
    #[serde(default)]
    candidate_inventory_record: Option<u64>,
    #[serde(default)]
    triggered_damage: Option<TriggeredDamage>,
    #[serde(default)]
    triggered_damage_owner: Option<u64>,
    #[serde(default)]
    peak_held_supplies: bool,
    #[serde(default)]
    peak_saturated_target: bool,
    #[serde(default)]
    peak_target_ailments: bool,
    #[serde(default)]
    peak_coating: Option<crate::peak_coating::HeldCoating>,
    template_layout_sha256: String,
    hit_values_sha256: String,
    runtime_extensions_closed: bool,
    mode: Mode,
    templates: Vec<Template>,
    single_use: Option<Status>,
    // Human/machine evidence stays in the owned document. It never overrides
    // admission fields or supplies a hidden default for runtime facts.
    provenance: serde_json::Value,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ExecutionEvent {
    pub template: usize,
    pub ordinal: u64,
    pub epoch: u64,
    pub phase: NextHitPhase,
    pub scored: bool,
    pub ki: Option<(u64, u32)>,
    pub debit: Option<(u64, u32)>,
    pub peak_arts_words: Option<[u32; 3]>,
    pub coating_power: Option<u32>,
}

pub(super) struct EventProgram {
    document: Document,
    source: SourceIdentity,
    status: Option<SingleUseBinding>,
    initial_phase: NextHitPhase,
    patches: Vec<Vec<WordPatch>>,
    packets: Vec<PacketPolicy>,
    actor_damage_rows: Option<Vec<ActorDamageRow>>,
    fixed: Vec<ExecutionEvent>,
    pub gpu_compatible: bool,
    held_templates_reusable: bool,
    water: Option<(crate::water_sword::Role, [f32; 10])>,
    peak_arts_packets: Option<(bool, Vec<ArtsPacket>)>,
    peak_supplies: Option<crate::pipeline::source_rebuild::PeakSupplies>,
}

#[derive(Clone, Copy)]
enum ArtsPacket {
    Basic,
    Martial,
    Projectile,
    PreparedActor(crate::arts_proficiency::PreparedActorRoute),
}

struct ActorDamageRow {
    kind: PacketKind,
    allows_owner_element: bool,
}

fn hex_bytes(value: &str) -> Result<Vec<u8>, &'static str> {
    if !value.len().is_multiple_of(2) {
        return Err("odd parameter hex length");
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let a = (pair[0] as char)
                .to_digit(16)
                .ok_or("invalid parameter hex")?;
            let b = (pair[1] as char)
                .to_digit(16)
                .ok_or("invalid parameter hex")?;
            Ok((a * 16 + b) as u8)
        })
        .collect()
}

struct SequenceState {
    ki: f32,
    epoch: u64,
    phase: NextHitPhase,
}

struct ResourceBindings<'a> {
    stat: &'a [u8],
    d: &'a [u8],
    maximum: f32,
    ki_address: Option<u64>,
    debit_address: Option<u64>,
}

fn validate_resource_binding(
    initial: Option<u32>,
    ki: Option<u64>,
    debit: Option<u64>,
) -> Result<(), &'static str> {
    if initial.is_some_and(|b| !f32::from_bits(b).is_finite())
        || ki.is_some_and(|a| a == 0 || a.checked_add(3).is_none())
        || debit.is_some_and(|a| a == 0 || a.checked_add(3).is_none())
    {
        return Err("invalid sequence resource-state binding");
    }
    if let (Some(ki), Some(debit)) = (ki, debit) {
        if ki <= debit + 3 && debit <= ki + 3 {
            return Err("resource and pending debit fields overlap");
        }
    }
    Ok(())
}

fn compile_action(action: &mut Action, templates: usize) -> Result<(), &'static str> {
    let parameter = hex_bytes(&action.parameter_hex)?;
    if action.resource.is_empty()
        || parameter.len() != 336
        || action.parameter_sha256 != format!("{:x}", Sha256::digest(&parameter))
        || u16::from_le_bytes([parameter[0xC], parameter[0xD]]) != action.action
        || action.contacts.iter().any(|&i| i >= templates)
        || action
            .contact_boundaries
            .iter()
            .any(|b| b.before_contact >= action.contacts.len())
    {
        return Err("sequence action parameter identity or contact occurrence is invalid");
    }
    if matches!(action.cost, Cost::ResolvedMultiplier { bits } if !f32::from_bits(bits).is_finite())
    {
        return Err("nonfinite resolved action cost");
    }
    for boundary in action
        .before
        .iter()
        .chain(action.contact_boundaries.iter().flat_map(|c| &c.boundaries))
    {
        if matches!(boundary, Boundary::Recovery { amount_bits } if !f32::from_bits(*amount_bits).is_finite())
        {
            return Err("nonfinite recovery boundary");
        }
    }
    if matches!(action.cost, Cost::OrdinaryNinjaReaper) {
        let expected = match action.action {
            25192 => "4dcd2ead36a957c410bf0f1e9032f384700208ca3ed019d40b005fa9ace57044",
            25193 => "cc770be9c5339979c5ccfe041236f049e45e590875613c70073ce2e20dce4e49",
            25194 => "b3d9e9dc157cbfb0f23b134488dca5c99da575be09618b69d10786b920da10f1",
            _ => "",
        };
        if action.resource != "0x6000001ea" || expected != action.parameter_sha256 {
            return Err("ordinary Ninja Reaper debit producer belongs to another action branch");
        }
    }
    action.base_cost = Some(i16::from_le_bytes([parameter[0x16], parameter[0x17]]) as f32);
    Ok(())
}

fn apply_boundaries(
    boundaries: &[Boundary],
    state: &mut SequenceState,
) -> Result<bool, &'static str> {
    for boundary in boundaries {
        match boundary {
            Boundary::Recovery { amount_bits } => {
                // Resolved net delta, including native clamp semantics. No
                // additional zero/maximum clamp is inferred here.
                state.ki += f32::from_bits(*amount_bits);
                if !state.ki.is_finite() {
                    return Err("nonfinite resource after recovery");
                }
            }
            Boundary::Rebuild => {
                state.epoch = state
                    .epoch
                    .checked_add(1)
                    .ok_or("event rebuild epoch overflow")?;
                state.phase = state.phase.after_rebuild();
            }
            Boundary::Stop { reason } => {
                let _ = reason;
                return Ok(false);
            }
        }
    }
    Ok(true)
}

impl EventProgram {
    pub(super) fn append_held_effect(
        &self,
        stat: &[u8],
        d: &[u8],
        writes: &mut Vec<Word>,
    ) -> Result<(), &'static str> {
        if let Some((role, _)) = self.water {
            let [mult, level, scalar] = role.effect_words(stat);
            let main = self.document.main_address;
            writes.extend([
                (main + 0x174, mult),
                (main + 0x178, level),
                (main + 0x17c, scalar),
            ]);
            if let Some(arts) = role.effect {
                if f32::from_le_bytes(d[0x898..0x89c].try_into().unwrap()) > 0.0 {
                    return Err("active effect needs the additional owner resource branch");
                }
                if self.document.triggered_damage != Some(TriggeredDamage::TheoreticalPeak) {
                    writes.push((0x1c6_28b4_a544, arts.owner_additive_bits));
                }
            }
        }
        Ok(())
    }
    pub(super) fn bind_held_action(
        &mut self,
        context: Option<&crate::context::EncodedContext>,
    ) -> Result<(), String> {
        use crate::water_sword as water;
        use crate::arts_proficiency::{KiPhase, Start};
        let Mode::HeldWaterSword { arts, objective } = self.document.mode else {
            return Ok(());
        };
        let ctx = context.ok_or("held action requires the current character")?;
        let role = water::Role::decode(
            ctx.read_known(water::ROLE_FACTS)
                .map_err(|e| e.to_string())?,
        )?;
        if arts == Start::Untriggered && role.effect.is_some() {
            return Err("untriggered scenario contains an active object".into());
        }
        if arts == Start::FullGaugeActivation && !role.effect.is_some_and(|effect| effect.ki_phase == KiPhase::Ready && effect.distinct_keys == 0) {
            return Err("fresh Arts Proficiency requires a new zero-stack object".into());
        }
        if arts == Start::TheoreticalPeak && role.effect.is_some_and(|effect| effect != crate::arts_proficiency::ActiveArts::saturated()) {
            return Err("peak Arts requires its saturated charging phase".into());
        }
        objective.validate(role)?;
        let mut base = [0.0; 10];
        for (phase, pair) in self
            .document
            .templates
            .as_chunks::<2>()
            .0
            .iter()
            .enumerate()
        {
            let hash = water::PARAMETERS[phase];
            let action = water::FIRST + phase as u16;
            let state =
                crate::action_rows::ActionController::resolve(water::RESOURCE, action, hash)?;
            base[phase] =
                i16::from_le_bytes(state.parameter[0x16..0x18].try_into().unwrap()) as f32;
            let skill = u16::from_le_bytes(state.parameter[0xdc..0xde].try_into().unwrap());
            for (row, template) in pair.iter().enumerate() {
                if !template.identity.as_ref().is_some_and(|id| {
                    id.resource == water::RESOURCE
                        && id.action == action
                        && id.parameter_sha256 == hash
                }) || !matches!(template.packet, Packet::PreparedActor)
                    || template.landed != Some(true)
                    || template.guard != Some(true)
                    || !template.scored
                {
                    return Err(
                        "held template differs from its action identity or landing objective"
                            .into(),
                    );
                }
                let patches = &mut self.patches[2 * phase + row];
                let state_addr = water::phase_address(phase);
                let row_addr = water::row_address(phase, row);
                for (at, value) in [
                    (water::CONTROLLER_LINK, state_addr as u32),
                    (water::CONTROLLER_LINK + 4, (state_addr >> 32) as u32),
                    (self.document.main_address + 0x136, u32::from(skill)),
                    (self.document.main_address + 0x190, row_addr as u32),
                    (self.document.main_address + 0x194, (row_addr >> 32) as u32),
                ] {
                    patches.push(WordPatch::full(at, value));
                }
            }
        }
        self.water = Some((role, base));
        Ok(())
    }
    pub(super) fn bind_direct_owner_element(
        &mut self,
        context: Option<&crate::context::EncodedContext>,
    ) -> Result<(), String> {
        if !self.document.direct_owner_element {
            return Ok(());
        }
        if !matches!(
            self.document.mode,
            Mode::FixedContacts | Mode::HeldWaterSword { .. }
        ) || self
            .packets
            .iter()
            .any(|p| !matches!(p, PacketPolicy::PreparedActor))
        {
            return Err("owner elemental override requires fixed direct actor contacts".into());
        }
        let ctx = context.ok_or("direct actor status element needs a current role")?;
        let main = self.document.main_address;
        let actor = u64::from_le_bytes(ctx.read_known(main + 0x198).map_err(|e| e.to_string())?);
        let owner = u64::from_le_bytes(
            ctx.read_known(actor.checked_add(0x3a0).ok_or("actor overflow")?)
                .map_err(|e| e.to_string())?,
        );
        let field = |offset| {
            owner
                .checked_add(offset)
                .ok_or_else(|| "owner overflow".to_string())
        };
        let word = |offset| -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                ctx.read_known(field(offset)?).map_err(|e| e.to_string())?,
            ))
        };
        if word(0x188)? & 2 != 0 {
            return Err("alternate direct owner branch needs its actor predicate".into());
        }
        let element = word(0x2e9c)?;
        let active = element != u32::MAX;
        let power = active.then(|| word(0x2ea4)).transpose()?;
        for (index, patches) in self.patches.iter_mut().enumerate() {
            let accepts_element = self
                .actor_damage_rows
                .as_ref()
                .is_none_or(|rows| rows[index].allows_owner_element);
            let flags = patches
                .iter_mut()
                .find(|p| p.address == main + 0x13c && p.mask == u32::MAX)
                .ok_or("missing per-contact actor flags")?;
            flags.value =
                (flags.value & !(1 << 28)) | (u32::from(active && accepts_element) << 28);
            if let Some(power) = power.filter(|_| accepts_element) {
                patches.extend([
                    WordPatch::full(main + 0x14, element),
                    WordPatch::full(main + 0x18, power),
                ]);
            }
        }
        // The dense ABI still holds the unbound public templates. Prepared CUDA
        // consumes these same bound words as CPU and original-MAIN auditing.
        self.gpu_compatible = false;
        Ok(())
    }
    pub(super) fn damage_owner(&self) -> Option<u64> {
        self.document.triggered_damage.map(|_| {
            self.document
                .triggered_damage_owner
                .unwrap_or(self.document.source.owner)
        })
    }
    pub(super) fn append_triggered_damage(
        &self,
        mode: usize,
        stat: &[u8],
        damage: &[u8],
        frame: &Frame,
        writes: &mut Vec<Word>,
    ) -> Result<(), &'static str> {
        let Some(trigger) = self.document.triggered_damage else {
            return Ok(());
        };
        let owner = self.damage_owner().ok_or("missing damage owner")?;
        if self.document.peak_saturated_target || self.document.peak_target_ailments {
            // MAIN reads these gear/skill words only when its target lookup
            // finds an ailment. Supply real materialized words even for an
            // unchanged zero that the usual sparse frame writer would omit.
            let (base, _) = frame.damage_baseline();
            for offset in [0x308, 0x30c, 0x310, 0x314, 0x318, 0x7cc] {
                writes.push((base + offset as u64, crate::pipeline::u32_at(damage, offset)));
            }
        }
        let factor = if self.document.peak_held_supplies {
            let supplies = frame.source_rebuild.as_ref()
                .and_then(|program| program.peak_supplies)
                .ok_or("missing projected held supplies")?;
            trigger.factor_with_supplies(mode, damage, supplies.damage_offer(stat)?)?
        } else {
            trigger.first_hit_factor(mode, damage)?
        };
        writes.push((owner.checked_add(0x2e3c).ok_or("invalid damage owner")?, factor.to_bits()));
        if matches!(trigger, TriggeredDamage::StyleShiftThenAbsorption | TriggeredDamage::TheoreticalPeak) {
            let program = frame.source_rebuild.as_ref().ok_or("missing configured skill sources")?;
            // The common configured Flexibility skill is the sole producer of 8556.
            // The action reads its gate before rebuilding the destination outfit.
            let enabled = program.configured_flexibility.ok_or("missing configured Flexibility skill")?;
            let follow_up = if enabled { 1.06f32 } else { 1.0 };
            writes.push((owner + 0x2e34, follow_up.to_bits()));
            writes.push((owner + 0x2e38, follow_up.to_bits()));
            if trigger == TriggeredDamage::TheoreticalPeak && mode == 0 {
                let learned = program.arts_proficiency_learned.ok_or("missing Arts Proficiency skill")?;
                let additive = if learned && damage[0x72a] != 0 { 0.1f32 } else { 0.0 };
                writes.push((owner + 0x2df4, additive.to_bits()));
                if self.water.is_none() && learned
                    && f32::from_bits(crate::pipeline::u32_at(damage, 0x898)) > 0.0
                {
                    return Err("active effect needs the additional owner resource branch");
                }
            }
        }
        Ok(())
    }
    pub(super) fn candidate_source_layout(&self) -> Option<(u64, u64, Option<u64>)> {
        self.document
            .candidate_source_address
            .zip(self.document.candidate_category_field)
            .map(|(source, category)| (source, category, self.document.candidate_inventory_record))
    }
    pub(super) fn initial_projectile(&self) -> Option<&crate::action_rows::InitialProjectile> {
        self.document.initial_projectile.as_ref()
    }
    pub fn compile(
        bytes: &[u8],
        frame: &Frame,
        addresses: &[u64],
        rows: &[Vec<u32>],
        declared_hits: &[u8],
        context: Option<&crate::context::EncodedContext>,
    ) -> Result<Self, String> {
        let mut doc: Document = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if (doc.peak_saturated_target || doc.peak_target_ailments) && !doc.peak_held_supplies
            || doc.peak_saturated_target && doc.peak_target_ailments
        {
            return Err("projected target requires one bound peak supply policy".into());
        }
        let peak_supplies = frame.source_rebuild.as_ref().and_then(|p| p.peak_supplies);
        if let Some(coating) = doc.peak_coating {
            if !doc.peak_held_supplies
                || !peak_supplies.is_some_and(|stock| stock.can_coat(coating))
                || doc.templates.iter().any(|template| template.peak_allows_coating.is_none())
                || !doc.templates.iter().any(|template| template.peak_allows_coating == Some(true))
            {
                return Err("held coating requires its supply and verified packet routes".into());
            }
        }
        if doc.peak_held_supplies && doc.triggered_damage != Some(TriggeredDamage::TheoreticalPeak) {
            return Err("held supplies require the theoretical peak objective".into());
        }
        if matches!(doc.mode, Mode::HeldWaterSword { .. })
            && (doc.templates.len() != 20
                || doc.single_use.is_some()
                || !matches!(doc.initial_phase, Phase::Inactive))
        {
            return Err("held Water Sword requires ten pairs of direct windows".into());
        }
        if doc.triggered_damage.is_some()
            && (!frame.has_source_rebuild()
                || doc.candidate_source_address.is_none()
                || !(matches!(doc.mode, Mode::FixedContacts)
                    && (doc.templates.len() == 1
                        || doc.triggered_damage == Some(TriggeredDamage::TheoreticalPeak)
                            && doc.templates.iter().all(|t| t.peak_consumes_follow_up.is_some()))
                    || doc.triggered_damage == Some(TriggeredDamage::SamuraiDeflect)
                    && matches!(doc.mode, Mode::HeldWaterSword {
                        arts: crate::arts_proficiency::Start::FullGaugeActivation,
                        objective: crate::water_sword::Objective::OpeningThreeActions,
                    })
                    || doc.triggered_damage == Some(TriggeredDamage::TheoreticalPeak)
                    && matches!(doc.mode, Mode::HeldWaterSword {
                        arts: crate::arts_proficiency::Start::TheoreticalPeak, ..
                    }))
                || doc.single_use.is_some()
                || !matches!(doc.initial_phase, Phase::Inactive))
        {
            return Err(
                "damage trigger requires a source-bound fixed hit or the verified fresh Arts opening".into(),
            );
        }
        if let Some(spawn) = &doc.initial_projectile {
            if !matches!(doc.templates.as_slice(), [t] if matches!(t.packet,
                Packet::OwnerWeaponProjectile {object, descriptor_kind, source_tag, ..} if spawn.matches(object,descriptor_kind,source_tag)))
            {
                return Err("initial projectile differs from its event packet".into());
            }
        } else if doc.actor_skill_key.is_some()
            && doc
                .templates
                .iter()
                .any(|t| !matches!(t.packet, Packet::PreparedActor))
        {
            return Err("actor skill key requires direct actor contacts".into());
        }
        if doc.schema != "nioh3-production-event-plan/v1" || !doc.runtime_extensions_closed {
            return Err("unsupported event plan or unresolved runtime extensions".into());
        }
        if doc.source.owner != frame.source_owner_address()
            || doc.source.owner == 0
            || doc.source.incarnation == 0
            || doc.main_address != frame.main_address()
        {
            return Err("event plan source incarnation/owner or MAIN binding differs".into());
        }
        let layout: Vec<_> = addresses.iter().flat_map(|a| a.to_le_bytes()).collect();
        if doc.template_layout_sha256 != format!("{:x}", Sha256::digest(&layout))
            || doc.hit_values_sha256 != format!("{:x}", Sha256::digest(declared_hits))
        {
            return Err("event plan does not bind the owned hit bytes/layout".into());
        }
        if doc.templates.len() != rows.len() || rows.iter().any(|r| r.len() != addresses.len()) {
            return Err("event plan template count/layout differs".into());
        }
        if matches!(doc.mode, Mode::HeldReaper) != frame.held_reaper {
            return Err("event plan held mode differs from the frame".into());
        }
        let source = SourceIdentity {
            owner: doc.source.owner,
            incarnation: doc.source.incarnation,
        };
        let status = doc.single_use.as_ref().map(|s| SingleUseBinding {
            source,
            scalar_addresses: s.scalar_addresses,
            active_bits: s.active_bits,
            rebuilt_bits: s.rebuilt_bits,
            active_membership: s.active_membership.clone(),
            removed_membership: s.removed_membership.clone(),
        });
        if let Some(s) = &status {
            s.words(doc.initial_phase.into())?;
        }
        if !matches!(doc.initial_phase, Phase::Inactive) && status.is_none() {
            return Err("active or removed single-use state requires both bound snapshots".into());
        }
        // The context can carry a newer, consumed snapshot than the bundle's
        // retained entry row. Its complete membership and scalar words choose
        // the current state; a frozen Active plan must never restore the buff.
        let requested_phase: NextHitPhase = doc.initial_phase.into();
        let initial_phase = if let Some(binding) = &status {
            let first = rows
                .first()
                .ok_or("single-use source has no initial contact snapshot")?;
            let mut current = std::collections::BTreeMap::new();
            for (&a, &value) in addresses.iter().zip(first) {
                for (i, b) in value.to_le_bytes().iter().enumerate() {
                    current.insert(a + i as u64, *b);
                }
            }
            let mut matching = Vec::new();
            for phase in [
                NextHitPhase::Active,
                NextHitPhase::RemovedPendingRebuild,
                NextHitPhase::Inactive,
            ] {
                let words = binding.words(phase)?;
                if words.iter().all(|&(a, v)| {
                    v.to_le_bytes()
                        .iter()
                        .enumerate()
                        .all(|(i, b)| current.get(&(a + i as u64)) == Some(b))
                }) {
                    matching.push(phase);
                }
            }
            if matching.len() == 1 {
                matching[0]
            } else if matching.contains(&requested_phase) {
                requested_phase
            } else {
                return Err(
                    "current source membership/scalars do not identify a bound initial phase"
                        .into(),
                );
            }
        } else {
            requested_phase
        };
        let mut ids = std::collections::HashSet::new();
        let mut packets = Vec::new();
        for (index, t) in doc.templates.iter().enumerate() {
            if t.id.is_empty()
                || !ids.insert(t.id.as_str())
                || t.landed.is_none()
                || t.guard.is_none()
                || t.landed == Some(true) && t.guard == Some(false)
            {
                return Err("event identity or explicit landing/guard decision is invalid".into());
            }
            let expected = frame
                .hit_packets
                .get(index)
                .copied()
                .unwrap_or(HitPacket::Prepared);
            let packet = match (&t.packet, expected) {
                (Packet::PreparedActor, HitPacket::Prepared) => PacketPolicy::PreparedActor,
                (
                    Packet::OwnerWeaponProjectile {
                        object,
                        incarnation,
                        owner,
                        source_tag,
                        ..
                    },
                    HitPacket::Prepared,
                ) if doc.initial_projectile.is_some()
                    && *owner == source.owner
                    && *incarnation != 0
                    && frame.uses_weapon_element()
                    && frame.has_source_rebuild() =>
                {
                    PacketPolicy::OwnerWeaponProjectile {
                        projectile: *object,
                        projectile_incarnation: *incarnation,
                        source_tag: *source_tag,
                    }
                }
                (
                    Packet::UnimbuedOwnerProjectile {
                        object,
                        incarnation,
                        owner,
                        descriptor_kind,
                        source_tag,
                        power,
                        main_flags,
                    },
                    HitPacket::UnimbuedOwnerProjectile {
                        power: p,
                        main_flags: f,
                    },
                ) if *object != 0
                    && *incarnation != 0
                    && *owner == source.owner
                    && *descriptor_kind == 0
                    && *power == p
                    && *main_flags == f =>
                {
                    PacketPolicy::UnimbuedOwnerProjectile {
                        main: frame.main_address(),
                        descriptor_kind: *descriptor_kind,
                        projectile: *object,
                        projectile_incarnation: *incarnation,
                        source_tag: *source_tag,
                        power: *power,
                        main_flags: *main_flags,
                    }
                }
                _ => return Err("event packet policy differs from its frame/source binding".into()),
            };
            if let PacketPolicy::UnimbuedOwnerProjectile { projectile, .. }
            | PacketPolicy::OwnerWeaponProjectile { projectile, .. } = packet
            {
                let word = |offset| {
                    addresses
                        .iter()
                        .rposition(|&a| a == frame.main_address() + offset)
                        .map(|i| rows[index][i])
                };
                let (Some(lo), Some(hi)) = (word(0x198), word(0x19c)) else {
                    return Err("projectile source words are not bound by the template".into());
                };
                if u64::from(hi) << 32 | u64::from(lo) != projectile {
                    return Err("projectile object differs from the event packet source".into());
                }
            }
            if let Some(identity) = &t.identity {
                identity.validate()?;
            }
            packets.push(packet);
        }
        match &mut doc.mode {
            Mode::FiniteSequence {
                actions,
                initial_ki_bits,
                ki_address,
                debit_address,
            } => {
                validate_resource_binding(*initial_ki_bits, *ki_address, *debit_address)?;
                for action in actions {
                    compile_action(action, rows.len())?;
                }
            }
            Mode::CyclicSequence {
                entry,
                cycle,
                initial_ki_bits,
                ki_address,
                debit_address,
                ..
            } => {
                validate_resource_binding(*initial_ki_bits, *ki_address, *debit_address)?;
                for action in entry.iter_mut().chain(cycle.iter_mut()) {
                    compile_action(action, rows.len())?;
                }
            }
            Mode::FixedContacts | Mode::HeldReaper | Mode::HeldWaterSword { .. } => {}
        }
        if doc.candidate_source_address.is_some() != doc.candidate_category_field.is_some()
            || (doc.candidate_inventory_record.is_some() && doc.candidate_source_address.is_none())
        {
            return Err("incomplete candidate source layout".into());
        }
        let coating_flags_at = doc.main_address + 0x13c;
        // A dense hit table can omit flags common to all contacts. Bind the
        // original known word from the same image in that case.
        let retained_coating_flags = if doc.peak_coating.is_some()
            && !addresses.contains(&coating_flags_at)
        {
            let bytes = context.ok_or("coating route needs its captured packet")?
                .read_known::<4>(coating_flags_at).map_err(|e| e.to_string())?;
            Some(WordPatch::full(coating_flags_at, u32::from_le_bytes(bytes)))
        } else {
            None
        };
        let patches = rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let actor_category = frame.has_source_rebuild()
                    && doc.candidate_source_address.is_some()
                    && matches!(
                        doc.templates[i].packet,
                        Packet::PreparedActor | Packet::OwnerWeaponProjectile { .. }
                    );
                addresses
                    .iter()
                    .copied()
                    .zip(row.iter().copied())
                    .filter(|(address, _)| !actor_category || *address != doc.main_address + 0x144)
                    .map(|(address, value)| WordPatch::full(address, value))
                    .chain(retained_coating_flags.filter(|_| doc.templates[i].peak_allows_coating == Some(true)))
                    .collect()
            })
            .collect();
        let peak = doc.triggered_damage == Some(TriggeredDamage::TheoreticalPeak);
        let peak_arts_packets = match &frame.source_rebuild {
            Some(program) if peak && matches!(doc.mode, Mode::FixedContacts)
                && program.binding.form == 0 =>
            {
                let learned = program.arts_proficiency_learned
                    .ok_or("missing Arts Proficiency skill")?;
                let mut classes = Vec::with_capacity(doc.templates.len());
                for template in &doc.templates {
                    if let Some(route) = template.peak_actor_arts {
                        if !matches!(template.packet, Packet::PreparedActor) {
                            return Err("actor Arts route needs a prepared actor contact".into());
                        }
                        classes.push(ArtsPacket::PreparedActor(route));
                        continue;
                    }
                    if !matches!(template.packet, Packet::PreparedActor) {
                        classes.push(ArtsPacket::Projectile);
                        continue;
                    }
                    let id = template.identity.as_ref().ok_or("peak attack needs its resource action")?;
                    let action = crate::action_rows::ActionController::resolve(
                        &id.resource, id.action, &id.parameter_sha256,
                    )?;
                    let skill = i32::from_le_bytes(action.parameter[0xdc..0xe0].try_into().unwrap());
                    classes.push(if skill >= 0 { ArtsPacket::Martial } else { ArtsPacket::Basic });
                }
                Some((learned, classes))
            }
            _ => None,
        };
        let actor_damage_rows = if doc.actor_damage_kind_from_row {
            if frame.packet_kind != PacketKind::Weapon
                || !matches!(doc.mode, Mode::FixedContacts)
                || packets
                    .iter()
                    .any(|p| !matches!(p, PacketPolicy::PreparedActor))
                || addresses.contains(&(doc.main_address + 0xC))
            {
                return Err(
                    "row damage kind requires fixed actor rows with candidate weapon kind".into(),
                );
            }
            let ctx = context.ok_or("row damage kind needs its installed controller rows")?;
            let index = |offset| {
                addresses
                    .iter()
                    .position(|&address| address == doc.main_address + offset)
                    .ok_or("missing per-contact damage row pointer")
            };
            let (low, high) = (index(0x190)?, index(0x194)?);
            let routes = rows
                .iter()
                .map(|row| {
                    let address = (u64::from(row[high]) << 32) | u64::from(row[low]);
                    let raw = ctx.read_known::<0x95>(address).map_err(|e| e.to_string())?;
                    Ok::<_, String>(ActorDamageRow {
                        kind: PacketKind::DamageRow(raw[0x94] as i8),
                        // PC202 35A220 skips the temporary owner element when
                        // either damage-row flag 38 or 39 is present.
                        allows_owner_element: raw[4] & 0xC0 == 0,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Some(routes)
        } else {
            None
        };
        let can_use_dense_packets = !peak && actor_damage_rows.is_none();
        let initial_phase = if peak { NextHitPhase::Active } else { initial_phase };
        let mut result = Self {
            peak_arts_packets,
            peak_supplies,
            document: doc,
            source,
            status,
            initial_phase,
            patches,
            packets,
            actor_damage_rows,
            fixed: Vec::new(),
            gpu_compatible: false,
            held_templates_reusable: false,
            water: None,
        };
        let mut epoch = result.document.initial_epoch;
        let mut phase = result.initial_phase;
        if matches!(result.document.mode, Mode::FixedContacts | Mode::HeldReaper) {
            for index in 0..rows.len() {
                result.fixed.push(result.advance(
                    index,
                    index as u64,
                    &mut epoch,
                    &mut phase,
                    None,
                    None,
                )?);
            }
        }
        if matches!(result.document.mode, Mode::HeldReaper) {
            if result.fixed.len() != 3 {
                return Err("held Reaper requires its entry and two alternating templates".into());
            }
            // A template is reusable only after the first complete cycle has
            // reached the same phase at BOTH loop contacts and at the cycle
            // boundary. Comparing just the first three input rows misses a
            // status consumed by the second loop contact.
            let cycle_entry_phase = phase;
            let mut proof_epoch = 0;
            let next_first = result.advance(1, 3, &mut proof_epoch, &mut phase, None, None)?;
            let next_second = result.advance(2, 4, &mut proof_epoch, &mut phase, None, None)?;
            result.held_templates_reusable = result
                .document
                .templates
                .iter()
                .all(|t| t.landed == Some(true) && t.guard == Some(true) && t.scored)
                && next_first.phase == result.fixed[1].phase
                && next_second.phase == result.fixed[2].phase
                && phase == cycle_entry_phase;
        }
        // GPU templates already contain the same state snapshots in the old
        // native hit ABI. Refuse only an actual semantic/layout mismatch.
        result.gpu_compatible = can_use_dense_packets
            && matches!(result.document.mode, Mode::FixedContacts | Mode::HeldReaper);
        for event in result.fixed.iter().filter(|_| can_use_dense_packets) {
            let mut built = Vec::new();
            let admitted = result.assemble(*event, &[], &mut built)?;
            let mut expected: Vec<_> = addresses
                .iter()
                .copied()
                .zip(rows[event.template].iter().copied())
                .collect();
            frame.append_hit_packet(event.template, &mut expected);
            let canonical = |words: Vec<Word>| {
                let mut values = std::collections::BTreeMap::new();
                for (a, v) in words {
                    for (i, b) in v.to_le_bytes().iter().enumerate() {
                        values.insert(a + i as u64, *b);
                    }
                }
                values
            };
            result.gpu_compatible &=
                admitted && event.scored && canonical(built) == canonical(expected);
        }
        if matches!(result.document.mode, Mode::HeldReaper) {
            result.gpu_compatible &= result.held_templates_reusable;
        }
        // The provenance is retained in the immutable plan; it is never a way
        // to turn an unsupported source or guard into a numeric event.
        let _ = &result.document.provenance;
        Ok(result)
    }

    fn advance(
        &self,
        template: usize,
        ordinal: u64,
        epoch: &mut u64,
        phase: &mut NextHitPhase,
        ki: Option<Word>,
        debit: Option<Word>,
    ) -> Result<ExecutionEvent, &'static str> {
        let t = &self.document.templates[template];
        let peak = self.document.triggered_damage == Some(TriggeredDamage::TheoreticalPeak);
        // The ideal full-hit objective settles each scored window before the next.
        // Captured status plans retain their observed rebuild boundaries.
        if peak {
            *phase = phase.after_rebuild();
        }
        if t.rebuild_before {
            *epoch = epoch.checked_add(1).ok_or("event rebuild epoch overflow")?;
            *phase = phase.after_rebuild();
        }
        let event = ExecutionEvent {
            template,
            ordinal,
            epoch: *epoch,
            phase: *phase,
            scored: t.scored,
            ki,
            debit,
            peak_arts_words: None,
            coating_power: None,
        };
        if *phase == NextHitPhase::Active && t.landed == Some(true) {
            if peak {
                if t.peak_consumes_follow_up.unwrap_or(true) {
                    *phase = NextHitPhase::RemovedPendingRebuild;
                }
            } else {
                let s = t.settlement.as_ref().ok_or("active status requires explicit settlement facts")?;
                *phase = phase.after_settlement(Settlement {
                    landed: true,
                    source_present: s.source_present,
                    hp_owner_present: s.hp_owner_present,
                    packet_flags_296_298: s.packet_flags_296_298,
                    actor_event_241: s.actor_event_241,
                    physical_motion: s.physical_motion,
                    elemental_motion: s.elemental_motion,
                });
            }
        }
        Ok(event)
    }

    pub fn all_fixed_events_scored(&self) -> bool {
        matches!(self.document.mode, Mode::HeldWaterSword { .. })
            || matches!(self.document.mode, Mode::FixedContacts)
                && self.fixed.iter().all(|event| event.scored)
    }

    pub fn is_single_fixed_contact(&self) -> bool {
        matches!(self.document.mode, Mode::FixedContacts)
            && matches!(self.fixed.as_slice(), [event] if event.scored)
    }

    pub fn requires_occurrence_execution(&self) -> bool {
        matches!(
            self.document.mode,
            Mode::FiniteSequence { .. } | Mode::CyclicSequence { .. } | Mode::HeldWaterSword { .. }
        ) || self.document.triggered_damage == Some(TriggeredDamage::TheoreticalPeak)
            || matches!(self.document.mode, Mode::HeldReaper) && !self.held_templates_reusable
    }

    pub fn reuses_held_templates(&self) -> bool {
        matches!(self.document.mode, Mode::HeldReaper) && self.held_templates_reusable
    }

    #[cfg(test)]
    pub(super) fn audit_force_held_template_reuse(&mut self) {
        self.held_templates_reusable = true;
    }

    pub fn diagnostics(&self) -> serde_json::Value {
        serde_json::json!({"schema":self.document.schema,"source_owner":self.source.owner,
            "source_owner_namespace":"normalized_pipeline_owner",
            "source_incarnation":self.source.incarnation,"initial_epoch":self.document.initial_epoch,
            "source_incarnation_scope":"explicit event-context incarnation; not a physical process pointer lifetime",
            "templates":self.document.templates.len(),"single_use_snapshots":self.status.is_some(),
            "initial_phase":format!("{:?}",self.initial_phase),
            "initial_phase_binding":"current context template membership and scalar snapshots",
            "gpu_dense_input_equivalent":self.gpu_compatible,
            "held_template_reuse_proven":self.held_templates_reusable,
            "rebuild_scope":"explicit single-use membership and aggregate snapshots; ordinary source production remains separately bound",
            "mode":match self.document.mode {
                Mode::FixedContacts=>"fixed_contacts",Mode::HeldReaper=>"held_reaper",Mode::FiniteSequence{..}=>"finite_sequence",
                Mode::CyclicSequence{..}=>"cyclic_sequence",Mode::HeldWaterSword{..}=>"held_water_sword"}})
    }

    pub fn events(&self, stat: &[u8], d: &[u8]) -> Result<Vec<ExecutionEvent>, &'static str> {
        let mut events = self.build_events(stat, d)?;
        if let Some(coating) = self.document.peak_coating {
            let caster = crate::peak_prepared::jutsu_power(stat)
                .ok_or("prepared coating requires the candidate's caster stat")?;
            let power = self.peak_supplies.and_then(|stock| stock.coating_power(coating, caster))
                .ok_or("selected coating has no prepared provider")?;
            for event in &mut events {
                event.coating_power = Some(power);
            }
        }
        Ok(events)
    }

    fn build_events(&self, stat: &[u8], d: &[u8]) -> Result<Vec<ExecutionEvent>, &'static str> {
        if let Mode::HeldWaterSword { objective, .. } = self.document.mode {
            let (role, base) = self.water.ok_or("held action has no bound character")?;
            let mut events = Vec::new();
            let mut phase = self.initial_phase;
            let mut epoch = self.document.initial_epoch;
            crate::water_sword::walk_to(objective, stat, d, role, &base, |action, ki| {
                events
                    .try_reserve(2)
                    .map_err(|_| "held sequence exceeds available memory")?;
                for template in [action * 2, action * 2 + 1] {
                    events.push(self.advance(
                        template,
                        events.len() as u64,
                        &mut epoch,
                        &mut phase,
                        Some((crate::water_sword::CURRENT_KI, ki.to_bits())),
                        None,
                    )?);
                }
                Ok(())
            })?;
            return Ok(events);
        }
        if matches!(self.document.mode, Mode::HeldReaper) {
            let count = crate::sustained::reaper_steps(stat, d)?;
            let remaining = u64::from(count.saturating_sub(1));
            let rebuilds = u64::from(count > 0 && self.document.templates[0].rebuild_before)
                + remaining.div_ceil(2) * u64::from(self.document.templates[1].rebuild_before)
                + (remaining / 2) * u64::from(self.document.templates[2].rebuild_before);
            self.document
                .initial_epoch
                .checked_add(rebuilds)
                .ok_or("event rebuild epoch overflow")?;
            if self.held_templates_reusable {
                return Ok(self.fixed.clone());
            }
            let mut events = Vec::new();
            events
                .try_reserve(count as usize)
                .map_err(|_| "held event allocation unavailable")?;
            let mut epoch = self.document.initial_epoch;
            let mut phase = self.initial_phase;
            for ordinal in 0..count {
                let template = if ordinal == 0 {
                    0
                } else if ordinal % 2 == 1 {
                    1
                } else {
                    2
                };
                events.push(self.advance(
                    template,
                    u64::from(ordinal),
                    &mut epoch,
                    &mut phase,
                    None,
                    None,
                )?);
            }
            return Ok(events);
        }
        let (entry, cycle, max_cycles, initial_ki_bits, ki_address, debit_address) =
            match &self.document.mode {
                Mode::FiniteSequence {
                    actions,
                    initial_ki_bits,
                    ki_address,
                    debit_address,
                } => (
                    actions.as_slice(),
                    &[][..],
                    Some(0),
                    *initial_ki_bits,
                    *ki_address,
                    *debit_address,
                ),
                Mode::CyclicSequence {
                    entry,
                    cycle,
                    max_cycles,
                    initial_ki_bits,
                    ki_address,
                    debit_address,
                } => (
                    entry.as_slice(),
                    cycle.as_slice(),
                    *max_cycles,
                    *initial_ki_bits,
                    *ki_address,
                    *debit_address,
                ),
                Mode::FixedContacts | Mode::HeldReaper | Mode::HeldWaterSword { .. } => {
                    let mut events = self.fixed.clone();
                    if let Some((learned, classes)) = &self.peak_arts_packets {
                        let arts = crate::arts_proficiency::ActiveArts::saturated();
                        let basic = if *learned { arts.packet_words(stat, false) } else { [0; 3] };
                        let martial = if *learned { arts.packet_words(stat, true) } else { [0; 3] };
                        for event in &mut events {
                            event.peak_arts_words = Some(match classes[event.template] {
                                ArtsPacket::Basic => basic,
                                ArtsPacket::Martial => martial,
                                ArtsPacket::Projectile => [0; 3],
                                ArtsPacket::PreparedActor(route) => route.words(martial),
                            });
                        }
                    }
                    return Ok(events);
                }
            };
        let maximum = u32::from_le_bytes(
            stat.get(0x1c..0x20)
                .ok_or("STAT maximum Ki unavailable")?
                .try_into()
                .map_err(|_| "STAT maximum Ki extent differs")?,
        ) as f32;
        let ki = initial_ki_bits.map_or(maximum, f32::from_bits);
        if !ki.is_finite() || ki > maximum {
            return Err("initial Ki is outside the actor maximum");
        }
        let mut state = SequenceState {
            ki,
            epoch: self.document.initial_epoch,
            phase: self.initial_phase,
        };
        let bindings = ResourceBindings {
            stat,
            d,
            maximum,
            ki_address,
            debit_address,
        };
        let mut output = Vec::new();
        if !self.run_actions(entry, &bindings, &mut state, &mut output)? {
            return Ok(output);
        }
        let mut completed = 0u32;
        while !cycle.is_empty()
            && max_cycles.is_none_or(|limit| completed < limit)
            && (max_cycles.is_some() || state.ki > 0.0)
        {
            let before = state.ki;
            if !self.run_actions(cycle, &bindings, &mut state, &mut output)? {
                return Ok(output);
            }
            completed = completed
                .checked_add(1)
                .ok_or("sequence cycle count exceeds the result domain")?;
            if max_cycles.is_none() && state.ki >= before {
                return Err("unbounded sequence has no proven finite Ki depletion");
            }
        }
        Ok(output)
    }

    fn run_actions(
        &self,
        actions: &[Action],
        bindings: &ResourceBindings<'_>,
        state: &mut SequenceState,
        output: &mut Vec<ExecutionEvent>,
    ) -> Result<bool, &'static str> {
        for action in actions {
            if !apply_boundaries(&action.before, state)? {
                return Ok(false);
            }
            let base = action
                .base_cost
                .ok_or("action parameter was not compiled")?;
            if base > 0.0 && state.ki <= 0.0 {
                return Ok(false);
            }
            let debit = match action.cost {
                Cost::ResolvedMultiplier { bits } => base * f32::from_bits(bits),
                Cost::OrdinaryNinjaReaper => crate::sustained::ordinary_reaper_cost(
                    bindings.stat,
                    bindings.d,
                    base,
                    state.ki,
                    bindings.maximum,
                )?,
            };
            state.ki -= debit;
            if !debit.is_finite() || !state.ki.is_finite() {
                return Err("nonfinite action debit");
            }
            for (occurrence, &index) in action.contacts.iter().enumerate() {
                for boundary in &action.contact_boundaries {
                    if boundary.before_contact == occurrence
                        && !apply_boundaries(&boundary.boundaries, state)?
                    {
                        return Ok(false);
                    }
                }
                let ordinal = u64::try_from(output.len())
                    .map_err(|_| "event ordinal exceeds the result domain")?;
                output.push(self.advance(
                    index,
                    ordinal,
                    &mut state.epoch,
                    &mut state.phase,
                    bindings.ki_address.map(|a| (a, state.ki.to_bits())),
                    bindings.debit_address.map(|a| (a, debit.to_bits())),
                )?);
            }
        }
        Ok(true)
    }

    pub fn assemble(
        &self,
        event: ExecutionEvent,
        candidate: &[Word],
        output: &mut Vec<Word>,
    ) -> Result<bool, &'static str> {
        let t = &self.document.templates[event.template];
        let occurrence = format!("{}:{}", t.id, event.ordinal);
        let input = EventInput {
            occurrence: &occurrence,
            source: self.source,
            epoch: event.epoch,
            patches: &self.patches[event.template],
            packet: self.packets[event.template],
            landed: t.landed,
            guard: t.guard,
            runtime_extensions_closed: self.document.runtime_extensions_closed,
        };
        let admitted = assemble_event_words(
            self.source,
            event.epoch,
            candidate,
            self.status.as_ref(),
            &input,
            event.phase,
            output,
            |_| Err("event template requires unknown baseline bytes"),
        )?;
        if admitted {
            if let Some(rows) = &self.actor_damage_rows {
                // The ordinary constructor promotes row kind 1 when the selected
                // candidate weapon chooses kind 2. Each contact owns this choice.
                let address = self.document.main_address + 0xC;
                let &(_, weapon_kind) = candidate
                    .iter()
                    .rfind(|(at, _)| *at == address)
                    .ok_or("missing candidate weapon damage kind")?;
                output.push((address, rows[event.template].kind.resolve(weapon_kind)));
            }
            if self.document.triggered_damage == Some(TriggeredDamage::TheoreticalPeak)
                && event.phase == NextHitPhase::Inactive
            {
                // The first landed hit consumes the follow-up. The general
                // damage-dealt contribution survives the ordinary rebuild.
                let owner = self.damage_owner().ok_or("missing follow-up owner")?;
                output.extend([(owner + 0x2e34, 1f32.to_bits()), (owner + 0x2e38, 1f32.to_bits())]);
            }
            if let Some([mult, level, scalar]) = event.peak_arts_words {
                let main = self.document.main_address;
                output.extend([(main + 0x174, mult), (main + 0x178, level), (main + 0x17c, scalar)]);
                if let Some(route) = t.peak_actor_arts {
                    let flags = output.iter().rfind(|(address, _)| *address == main + 0x13c)
                        .map(|&(_, flags)| flags).ok_or("actor Arts route is missing prepared flags")?;
                    output.push((main + 0x13c, route.flags(flags, mult)));
                }
            }
            if let Some(coating) = self.document.peak_coating.filter(|_| t.peak_allows_coating == Some(true)) {
                let main = self.document.main_address;
                let (_, value) = output.iter().rfind(|(address, _)| *address == main + 0x13c)
                    .copied().ok_or("coating route is missing the prepared packet flags")?;
                let power = event.coating_power.ok_or("coating route is missing the selected power")?;
                output.extend(coating.packet_words(main, value, power));
            }
            output.extend(event.ki);
            output.extend(event.debit);
        }
        Ok(admitted)
    }
}
