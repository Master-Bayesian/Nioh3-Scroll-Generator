//! Read-only import of a retained compiler and hit generation.
//!
//! The executable app verifier recreates all three native maps and scores
//! the reference outfit before publishing this captured context. The replay
//! entry point never confers live source authority.
use crate::error::HostError;
use nioh3_loadout::{
    context::{sha256, CoreEntry, LoadoutContext, Scenario, SceneBytes},
    damage::DamageDetails,
    engine::{Backpack, Candidate, Engine, Limits, Outcome, SLOTS},
    pipeline::{self, Frame, Scratch},
};
use nioh3_runtime::combat_context::CombatSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::Path};

const SCHEMA: &str = "nioh3-retained-source-epoch-import/v1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EpochReceipt {
    schema: String,
    bundle: String,
    capture_sha256: String,
    generation: String,
    context_sha256: String,
    source_version: [u16; 4],
    native_stages: NativeStages,
    stats: NativeStats,
    damage: [u32; 3],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeStages {
    prefix: Vec<CoreEntry>,
    core: Vec<CoreEntry>,
    selected: Vec<CoreEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeStats {
    primary_attack: u32,
    secondary_attack: u32,
    defense: u32,
    health: u32,
}

fn rejected(error: impl std::fmt::Display) -> HostError {
    HostError::rejected(format!("LOADOUT_SOURCE_EPOCH_INVALID: {error}"))
}

pub(crate) struct PreparedReplayEpoch {
    pub context: LoadoutContext,
    pub profile: Value,
    pub receipt: EpochReceipt,
}

impl PreparedReplayEpoch {
    pub(crate) fn load(
        bundle_id: &str,
        bundle_path: &Path,
        receipt_path: &Path,
    ) -> Result<Self, HostError> {
        let file = fs::File::open(receipt_path).map_err(rejected)?;
        use std::io::Read;
        let mut raw = Vec::new();
        file.take(65_537).read_to_end(&mut raw).map_err(rejected)?;
        if raw.len() > 65_536 {
            return Err(rejected("source epoch receipt exceeds its bounded shape"));
        }
        let receipt: EpochReceipt = serde_json::from_slice(&raw).map_err(rejected)?;
        if receipt.schema != SCHEMA
            || receipt.bundle != bundle_id
            || receipt.source_version != [2, 0, 2, 0]
        {
            return Err(rejected(
                "source epoch belongs to another version or attack",
            ));
        }
        let path = bundle_path.join("context");
        let context = LoadoutContext::load(&path).map_err(rejected)?;
        if receipt.context_sha256 != context.digest()
            || receipt.capture_sha256 != context.manifest().provenance.source_sha256
        {
            return Err(rejected(
                "source epoch and immutable scene belong to different captures",
            ));
        }
        let profile =
            serde_json::from_slice(&fs::read(path.join("profile.json")).map_err(rejected)?)
                .map_err(rejected)?;
        Ok(Self {
            context,
            profile,
            receipt,
        })
    }
}

fn entries_equal(actual: &[pipeline::Entry], expected: &[CoreEntry]) -> bool {
    actual
        .iter()
        .map(|entry| (entry.key, entry.base, entry.dup))
        .eq(expected
            .iter()
            .map(|entry| (entry.key, entry.base, entry.dup)))
}

impl EpochReceipt {
    pub(crate) fn verify_engine(self, engine: &Engine) -> Result<(), HostError> {
        let encoded = engine
            .encoded_context()
            .ok_or_else(|| rejected("captured context missing"))?;
        let frame = Frame::load(
            encoded
                .file("frame.bin")
                .ok_or_else(|| rejected("frame missing"))?,
        )
        .map_err(rejected)?;
        if frame.source_generation() != Some(self.generation.as_str()) {
            return Err(rejected("compiler generation and frame differ"));
        }
        let mut characters = Frame::characters(
            encoded
                .file("characters.bin")
                .ok_or_else(|| rejected("character missing"))?,
        )
        .map_err(rejected)?
        .into_iter();
        let character = characters
            .next()
            .ok_or_else(|| rejected("character missing"))?;
        let mut scratch = Scratch::new();
        engine
            .tables()
            .pipeline(&frame, &character, &engine.start_attrs, &[], &mut scratch)
            .map_err(rejected)?;
        let source = scratch
            .source_rebuild_trace
            .as_ref()
            .ok_or_else(|| rejected("source replay missing"))?;
        let map = character.mode as usize;
        if !entries_equal(&source.prefix[map], &self.native_stages.prefix)
            || !entries_equal(&source.completed_core[map], &self.native_stages.core)
            || !entries_equal(&scratch.source.maps[map], &self.native_stages.selected)
        {
            return Err(rejected(
                "replayed source map differs from the observed generation",
            ));
        }
        let candidate = Candidate {
            attrs: engine.start_attrs,
            items: [None; SLOTS],
        };
        let pack = Backpack::from_slots(&[]).map_err(rejected)?;
        let mut work = engine.work();
        let actual = engine
            .prepared_stats(&candidate, &pack, &mut work)
            .map_err(rejected)?;
        if (
            actual.primary_attack,
            actual.secondary_attack,
            actual.defense,
            actual.health,
        ) != (
            self.stats.primary_attack,
            self.stats.secondary_attack,
            self.stats.defense,
            self.stats.health,
        ) {
            return Err(rejected(
                "replayed character stats differ from the observed generation",
            ));
        }
        let detailed = engine.damage_details(&candidate, &pack, &mut work);
        if !matches!(detailed, DamageDetails::Available { total, physical, elemental, contacts }
            if [total.to_bits(), physical.to_bits(), elemental.to_bits()] == self.damage && contacts.len() == 1)
            || !matches!(engine.evaluate(&candidate, &pack, &Limits::NONE, &mut work),
                Outcome::Damage(bits) if bits == self.damage[0])
        {
            return Err(rejected(
                "replayed attack does not match original damage evidence",
            ));
        }
        Ok(())
    }
}

const LIVE_BINDING_SCHEMA: &str = "nioh3-live-source-generation/v2";
const CURRENT_RESOURCE: u64 = 0x15C;
const MAXIMUM_RESOURCE: u64 = 0x160;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveGeneration {
    capture_sha256: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotBinding {
    pid: u32,
    creation_filetime: u64,
    module_base: u64,
    owner: u64,
    source: u64,
    core_source: u64,
    stat: u64,
    materializer: u64,
    selected_sha256: String,
    core_sha256: String,
    logical_inputs_sha256: String,
    status_tree_sha256: String,
}

impl SnapshotBinding {
    fn from_snapshot(snapshot: &CombatSnapshot) -> Result<Self, HostError> {
        snapshot.validate().map_err(rejected)?;
        let roots = &snapshot.roots;
        let mut producer_inputs = snapshot.inputs.clone();
        // Only the recoverable current resource is applied from the fresh
        // snapshot after verifying the original replay. The maximum, presence,
        // equipment and every other producer input still identify the epoch.
        if let Some([current, _]) = &mut producer_inputs.owner_resource_bits {
            *current = 0;
        }
        Ok(Self {
            pid: snapshot.process.pid,
            creation_filetime: snapshot.process.creation_filetime,
            module_base: snapshot.module_base,
            owner: roots.owner,
            source: roots.source,
            core_source: roots.core_source,
            stat: roots.stat,
            materializer: roots.materializer,
            selected_sha256: sha256(&snapshot.source_raw),
            core_sha256: sha256(&snapshot.core_source_raw),
            logical_inputs_sha256: sha256(&serde_json::to_vec(&producer_inputs).map_err(rejected)?),
            status_tree_sha256: sha256(
                &serde_json::to_vec(&snapshot.status_tree_raw).map_err(rejected)?,
            ),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LiveBinding {
    schema: String,
    capture_sha256: String,
    context_sha256: String,
    snapshot: SnapshotBinding,
}

pub(crate) struct BoundLiveEpoch {
    pub bundle_path: std::path::PathBuf,
    pub context: LoadoutContext,
    pub profile: Value,
    pub verification: EpochVerification,
}

pub(crate) enum EpochVerification {
    VerifyAtPublication(Box<EpochReceipt>),
    VerifiedBeforeResourceUpdate,
}

impl EpochVerification {
    pub(crate) fn verify(self, engine: &Engine) -> Result<(), HostError> {
        match self {
            Self::VerifyAtPublication(receipt) => receipt.verify_engine(engine),
            Self::VerifiedBeforeResourceUpdate => Ok(()),
        }
    }
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn read_limited(path: &Path, size: u64) -> Result<Vec<u8>, HostError> {
    use std::io::Read;
    let mut raw = Vec::new();
    fs::File::open(path)
        .map_err(rejected)?
        .take(size + 1)
        .read_to_end(&mut raw)
        .map_err(rejected)?;
    if raw.len() as u64 > size {
        return Err(rejected("source import record exceeds its byte bound"));
    }
    Ok(raw)
}

pub(crate) enum EpochLookup {
    Unstaged,
    Changed,
    Bound(Box<BoundLiveEpoch>),
}

impl BoundLiveEpoch {
    // Called with the current verified two-read game snapshot. A recording
    // alone cannot enter this path or acquire the app's live source label.
    pub(crate) fn acquire(
        state_root: &Path,
        bundle_id: &str,
        bundle_path: std::path::PathBuf,
        snapshot: &CombatSnapshot,
    ) -> Result<EpochLookup, HostError> {
        let root = state_root.join("loadout-live-epochs").join(bundle_id);
        let marker = root.join("active.json");
        match fs::metadata(&marker) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(EpochLookup::Unstaged)
            }
            Err(error) => return Err(rejected(error)),
            Ok(_) => {}
        }
        let active: ActiveGeneration =
            serde_json::from_slice(&read_limited(&marker, 4096)?).map_err(rejected)?;
        if !digest(&active.capture_sha256) {
            return Err(rejected("the active generation identity is invalid"));
        }
        let path = root.join(&active.capture_sha256);
        let binding: LiveBinding =
            serde_json::from_slice(&read_limited(&path.join("LIVE_BINDING.json"), 16_384)?)
                .map_err(rejected)?;
        if binding.schema != LIVE_BINDING_SCHEMA || binding.capture_sha256 != active.capture_sha256
        {
            return Err(rejected(
                "the active live generation differs from its binding",
            ));
        }
        if SnapshotBinding::from_snapshot(snapshot)? != binding.snapshot {
            return Ok(EpochLookup::Changed);
        }
        // The player generation owns the private context and original receipt.
        // The installed scene owns every executable and shared scoring input.
        let prepared = PreparedReplayEpoch::load(
            bundle_id,
            &path.join("bundle"),
            &path.join("EPOCH_INPUT.json"),
        )?;
        if binding.context_sha256 != prepared.context.digest()
            || binding.capture_sha256 != prepared.receipt.capture_sha256
        {
            return Err(rejected(
                "live binding differs from the verified source frame",
            ));
        }
        prepared.verify_snapshot_basis(snapshot)?;
        let observed_resource = prepared.verify_scene_actor(snapshot)?;
        let updated = snapshot
            .inputs
            .owner_resource_bits
            .map(|[current, _]| current)
            .filter(|current| *current != observed_resource);
        let PreparedReplayEpoch {
            mut context,
            profile,
            receipt,
        } = prepared;
        let verification = if let Some(current) = updated {
            let recorded_engine =
                Engine::open_with_context(&bundle_path, &context).map_err(rejected)?;
            if !recorded_engine.is_single_fixed_contact()
                || recorded_engine
                    .hit_inputs()
                    .0
                    .contains(&(snapshot.roots.owner + CURRENT_RESOURCE))
            {
                return Err(rejected(
                    "current resource rebind requires an unoverwritten direct-contact input",
                ));
            }
            receipt.verify_engine(&recorded_engine)?;
            context = update_current_resource(context, snapshot.roots.owner, current)?;
            EpochVerification::VerifiedBeforeResourceUpdate
        } else {
            EpochVerification::VerifyAtPublication(Box::new(receipt))
        };
        Ok(EpochLookup::Bound(Box::new(Self {
            bundle_path,
            context,
            profile,
            verification,
        })))
    }
}

impl PreparedReplayEpoch {
    fn verify_snapshot_basis(&self, snapshot: &CombatSnapshot) -> Result<(), HostError> {
        let character = &self.context.manifest().character;
        let input = &snapshot.inputs;
        if input.compiled_source_observation.is_none()
            || input.inventory != self.context.inventory()
            || input.raw_attrs != self.context.manifest().attrs
            || input.synchronized_attrs != input.raw_attrs
            || input.mode != character.mode
            || input.active_bank != character.active
            || input.held_weapon_slot != 0
            || input.level != character.level
            || input.style != character.style
        {
            return Err(rejected("current actor and epoch baseline do not match"));
        }
        let actual = &input.stats;
        let expected = &self.receipt.stats;
        if (
            actual.primary_attack,
            actual.secondary_attack,
            actual.defense,
            actual.health,
        ) != (
            expected.primary_attack,
            Some(expected.secondary_attack),
            expected.defense,
            expected.health,
        ) {
            return Err(rejected("current actor stats differ from the source epoch"));
        }
        let map = input.mode as usize;
        let agree = |left: &[nioh3_runtime::combat_context::AggregateEntry],
                     right: &[CoreEntry]| {
            left.iter()
                .map(|row| (row.key, row.base, row.dup))
                .eq(right.iter().map(|row| (row.key, row.base, row.dup)))
        };
        if !agree(&input.groups[map], &self.receipt.native_stages.selected)
            || !agree(&input.core_groups[map], &self.receipt.native_stages.core)
        {
            return Err(rejected(
                "current compiled maps differ from the observed epoch",
            ));
        }
        Ok(())
    }

    fn verify_scene_actor(&self, snapshot: &CombatSnapshot) -> Result<u32, HostError> {
        use crate::loadout_live::{scene_bytes, scene_pointer, scene_word};
        let encoded = self.context.encode(&[]);
        let image = encoded
            .file("image.bin")
            .ok_or_else(|| rejected("live epoch image missing"))?;
        let known = encoded
            .file("image_known.bin")
            .ok_or_else(|| rejected("live epoch known bytes missing"))?;
        let frame = Frame::load(self.context.frame()).map_err(rejected)?;
        let roots = &snapshot.roots;
        let read_q = |address| scene_pointer(image, known, address);
        let read_u = |address| scene_word(image, known, address);
        let actor = read_q(frame.main_address() + 0x198)?;
        if actor != roots.player
            || read_q(actor + 0x3A0)? != roots.owner
            || read_q(roots.owner + 0x2D90)? != roots.selected_stat_pointer
            || read_q(roots.owner + 0x2D80)? != roots.core_source
            || read_q(roots.stat + 0x7D8)? != roots.source
            || read_q(roots.stat + 0x7C0)? != roots.materializer
        {
            return Err(rejected(
                "the replay scene does not own the live source objects",
            ));
        }
        let selected = scene_bytes(image, known, roots.source, snapshot.source_raw.len())?;
        let core = scene_bytes(
            image,
            known,
            roots.core_source,
            snapshot.core_source_raw.len(),
        )?;
        if selected != snapshot.source_raw || core != snapshot.core_source_raw {
            return Err(rejected(
                "the replay scene's source bytes differ from the observed actor",
            ));
        }
        let input = &snapshot.inputs;
        let owner = roots.owner;
        if (
            [read_u(owner + 0x2E34)?, read_u(owner + 0x2E38)?],
            read_u(owner + 0x2E3C)?,
            read_u(owner + 0x2E40)?,
            read_q(owner + 0x58)?,
            read_q(owner + 0x50)?,
            Some(read_u(owner + 0x188)?),
            Some(read_u(owner + MAXIMUM_RESOURCE)?),
        ) != (
            input.status_single_use_bits,
            input.status_class0_bits,
            input.status_class1_bits,
            input.current_health,
            input.maximum_health,
            input.owner_flags_188,
            input.owner_resource_bits.map(|[_, maximum]| maximum),
        ) {
            return Err(rejected(
                "the replay scene's live status or resources differ",
            ));
        }
        let stored_members = read_q(owner + 0x3110)?;
        if stored_members != 0 || !input.status_keys.is_empty() {
            let tree = snapshot
                .status_tree_raw
                .as_ref()
                .ok_or_else(|| rejected("live status tree evidence missing"))?;
            crate::loadout_live::validate_status_objects(image, known, owner, tree)?;
        }
        read_u(owner + CURRENT_RESOURCE)
    }
    pub(crate) fn binding_bytes(&self, snapshot: &CombatSnapshot) -> Result<Vec<u8>, HostError> {
        self.verify_snapshot_basis(snapshot)?;
        self.verify_scene_actor(snapshot)?;
        serde_json::to_vec_pretty(&LiveBinding {
            schema: LIVE_BINDING_SCHEMA.into(),
            capture_sha256: self.receipt.capture_sha256.clone(),
            context_sha256: self.context.digest().into(),
            snapshot: SnapshotBinding::from_snapshot(snapshot)?,
        })
        .map_err(rejected)
    }
}

/// Every unpublished evaluation uses the recorded original damage as its
/// oracle. After that check, update this one typed word in a fresh scene.
fn update_current_resource(
    captured: LoadoutContext,
    owner: u64,
    current: u32,
) -> Result<LoadoutContext, HostError> {
    let encoded = captured.encode(&[]);
    let file = |name| {
        encoded
            .file(name)
            .ok_or_else(|| rejected(format!("live epoch {name} missing")))
    };
    let mut image = file("image.bin")?.to_vec();
    let mut known = file("image_known.bin")?.to_vec();
    let address = owner + CURRENT_RESOURCE;
    let recorded = [
        crate::loadout_live::scene_word(&image, &known, address)?,
        crate::loadout_live::scene_word(&image, &known, owner + MAXIMUM_RESOURCE)?,
    ];
    let frame = Frame::rebind_current_owner_resource(captured.frame(), recorded, current)
        .map_err(rejected)?;

    crate::loadout_live::replace_scene_word(
        &mut image,
        &mut known,
        address,
        &current.to_le_bytes(),
    )?;
    let mut manifest = captured.manifest().clone();
    let Scenario::CapturedScene {
        image_sha256,
        image_known_sha256,
        ..
    } = &mut manifest.scenario
    else {
        return Err(rejected("live epoch requires a captured scene"));
    };
    *image_sha256 = sha256(&image);
    *image_known_sha256 = sha256(&known);
    manifest.frame_sha256 = sha256(&frame);
    LoadoutContext::new_with_scene(
        manifest,
        captured.inventory().to_vec(),
        frame,
        SceneBytes::CapturedScene {
            image: image.into(),
            image_known: known.into(),
            scene_entry: file("scene_entry.bin")?.into(),
        },
    )
    .map_err(rejected)
}
