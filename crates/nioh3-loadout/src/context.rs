//! Immutable, provenance-labelled inputs for one loadout evaluation context.
//!
//! Validation here is structural and byte-identity validation. It does not
//! certify a source derivation, buff model, live character, or attack scenario.
//! The caller must provide all character inputs and an explicitly matching
//! frame. No missing field is filled from the bundled sample character.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, fs, path::Path, sync::Arc};

pub const CONTEXT_SCHEMA: &str = "nioh3-loadout-context/v1";
pub const INVENTORY_BYTES: usize = 0x2018;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub capture_id: String,
    pub source_sha256: String,
    pub game_version: [u16; 4],
    /// Include the action/target scenario identity and evidence scope here.
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    DerivedNamed,
    /// Ordered original557BA0 submissions compiled before equipment collection.
    /// Values are u32; duplicate entries and zero-valued presence are meaningful.
    NativeBeforeEquipment,
    /// Ordered original557BA0 submissions after native set rewards, before the
    /// final materializer. These are never converted to fitted additive deltas.
    NativeAfterSets,
    /// An observed, immutable profile; not a proof of each source mechanism.
    ObservedNonEquipment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupOperation {
    pub group: u16,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceLayer {
    pub name: String,
    pub kind: SourceKind,
    pub provenance: Provenance,
    /// Order, duplicate groups, signed corrections and zero-valued entries
    /// are preserved exactly; they are not reduced to a lossy map.
    pub operations: Vec<GroupOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NonGearSources {
    pub attack_gain: u32,
    pub defense: u32,
    pub layers: Vec<SourceLayer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreEntry {
    pub key: u16,
    pub base: u32,
    pub dup: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterInputs {
    pub mode: i32,
    pub active: u32,
    pub cap: u32,
    pub plus_cap: u32,
    pub style: i32,
    pub unarmed: u32,
    pub level: u32,
    pub core_mode: i32,
    pub core_maps: [Vec<CoreEntry>; 2],
}

/// Recorded scenarios retain the attack bundle's original domain and image.
/// Captured scenes require complete canonical image/entry bytes and the full
/// MAIN evaluator; they never fall back to the recorded formula paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Scenario {
    RecordedScenario,
    CapturedScene {
        image_sha256: String,
        image_known_sha256: String,
        scene_entry_sha256: String,
        /// Admitted normal direct-actor attack; currently normalized to slot 0.
        attack_weapon_slot: u32,
        main_flags_298: u32,
    },
}

#[derive(Debug, Clone)]
pub enum SceneBytes {
    RecordedScenario,
    CapturedScene {
        image: Arc<[u8]>,
        image_known: Arc<[u8]>,
        scene_entry: Arc<[u8]>,
    },
}
impl SceneBytes {
    fn file(&self, name: &str) -> Option<&[u8]> {
        match (self, name) {
            (Self::CapturedScene { image, .. }, "image.bin") => Some(image),
            (Self::CapturedScene { image_known, .. }, "image_known.bin") => Some(image_known),
            (Self::CapturedScene { scene_entry, .. }, "scene_entry.bin") => Some(scene_entry),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextManifest {
    pub schema: String,
    pub provenance: Provenance,
    pub scenario: Scenario,
    pub attrs: [i32; 7],
    pub character: CharacterInputs,
    pub non_gear: NonGearSources,
    pub inventory_sha256: String,
    pub frame_sha256: String,
    /// Raw capture diagnostics only. These are not an established replacement
    /// for the model's post-materializer corrections and are not evaluated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_scalars: Option<RawNativeScalars>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawNativeScalars {
    pub source_5888: u32,
    pub source_5890: u32,
    pub source_5894: u32,
}

#[derive(Debug)]
pub struct ContextError(String);
impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ContextError {}
fn invalid(message: impl Into<String>) -> ContextError {
    ContextError(message.into())
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn normalize_hash(hash: &mut str, label: &str) -> Result<(), ContextError> {
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid(format!("{label} must be a SHA-256 digest")));
    }
    hash.make_ascii_lowercase();
    Ok(())
}
fn validate_provenance(value: &mut Provenance) -> Result<(), ContextError> {
    if value.capture_id.trim().is_empty() || value.description.trim().is_empty() {
        return Err(invalid("source capture_id and description are required"));
    }
    normalize_hash(&mut value.source_sha256, "source_sha256")
}
fn hash_part(hash: &mut Sha256, name: &str, bytes: &[u8]) {
    hash.update((name.len() as u64).to_le_bytes());
    hash.update(name.as_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

const IMAGE_CHUNK_BYTES: usize = 65_536;
const IMAGE_RECORD_BYTES: usize = 8 + IMAGE_CHUNK_BYTES;
const KNOWN_RECORD_BYTES: usize = 8 + IMAGE_CHUNK_BYTES / 8;
fn validate_image_known(image: &[u8], known: &[u8]) -> Result<(), ContextError> {
    if image.is_empty()
        || !image.len().is_multiple_of(IMAGE_RECORD_BYTES)
        || !known.len().is_multiple_of(KNOWN_RECORD_BYTES)
        || image.len() / IMAGE_RECORD_BYTES != known.len() / KNOWN_RECORD_BYTES
    {
        return Err(invalid(
            "captured image and known mask must contain matching complete chunks",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for (chunk, mask) in image
        .as_chunks::<IMAGE_RECORD_BYTES>()
        .0
        .iter()
        .zip(known.as_chunks::<KNOWN_RECORD_BYTES>().0.iter())
    {
        let address = u64::from_le_bytes(chunk[..8].try_into().unwrap());
        if address & 0xFFFF != 0
            || address.checked_add(65_536).is_none()
            || !seen.insert(address)
            || chunk[..8] != mask[..8]
        {
            return Err(invalid(
                "captured image/known chunks have unaligned, duplicate or mismatched addresses",
            ));
        }
    }
    Ok(())
}
fn known_bytes<const N: usize>(scene: &SceneBytes, address: u64) -> Result<[u8; N], ContextError> {
    let image = scene
        .file("image.bin")
        .ok_or_else(|| invalid("captured image missing"))?;
    let known = scene
        .file("image_known.bin")
        .ok_or_else(|| invalid("captured known mask missing"))?;
    let mut bytes = [0u8; N];
    for (byte, value) in bytes.iter_mut().enumerate() {
        let at = address
            .checked_add(byte as u64)
            .ok_or_else(|| invalid("captured word address overflow"))?;
        let chunk_base = at & !0xFFFF;
        let index = image
            .as_chunks::<IMAGE_RECORD_BYTES>()
            .0
            .iter()
            .position(|chunk| u64::from_le_bytes(chunk[..8].try_into().unwrap()) == chunk_base)
            .ok_or_else(|| invalid(format!("captured image has no page for {at:#x}")))?;
        let offset = (at - chunk_base) as usize;
        let bitmap = &known[index * KNOWN_RECORD_BYTES + 8..(index + 1) * KNOWN_RECORD_BYTES];
        if bitmap[offset >> 3] & (1 << (offset & 7)) == 0 {
            return Err(invalid(format!("captured image has an unknown byte at {at:#x} (read {address:#x}, width {N})")));
        }
        *value = image[index * IMAGE_RECORD_BYTES + 8 + offset];
    }
    Ok(bytes)
}
fn known_word(scene: &SceneBytes, address: u64) -> Result<u32, ContextError> {
    known_bytes(scene, address).map(u32::from_le_bytes)
}

/// Frozen inputs. Accessors expose only shared references; changing a context
/// requires constructing and validating a new value with a new digest.
#[derive(Debug, Clone)]
pub struct LoadoutContext {
    manifest: ContextManifest,
    inventory: Vec<u8>,
    frame: Vec<u8>,
    captured_frame: Option<Vec<u8>>,
    runtime_hit_inputs: Option<Vec<u8>>,
    scene: SceneBytes,
    digest: String,
}
impl LoadoutContext {
    /// Load exactly the three fixed context files, without arbitrary paths in
    /// the manifest: context.json, inventory.bin and frame.bin.
    pub fn load(directory: &Path) -> Result<Self, ContextError> {
        let read = |name: &str| {
            fs::read(directory.join(name)).map_err(|e| invalid(format!("{name}: {e}")))
        };
        let manifest: ContextManifest = serde_json::from_slice(&read("context.json")?)
            .map_err(|e| invalid(format!("context.json: {e}")))?;
        let scene = match &manifest.scenario {
            Scenario::RecordedScenario => SceneBytes::RecordedScenario,
            Scenario::CapturedScene { .. } => SceneBytes::CapturedScene {
                image: read("image.bin")?.into(),
                image_known: read("image_known.bin")?.into(),
                scene_entry: read("scene_entry.bin")?.into(),
            },
        };
        Self::new_with_scene(manifest, read("inventory.bin")?, read("frame.bin")?, scene)
    }

    pub fn new(
        manifest: ContextManifest,
        inventory: Vec<u8>,
        frame: Vec<u8>,
    ) -> Result<Self, ContextError> {
        Self::new_with_scene(manifest, inventory, frame, SceneBytes::RecordedScenario)
    }

    pub fn new_with_scene(
        mut manifest: ContextManifest,
        inventory: Vec<u8>,
        frame: Vec<u8>,
        scene: SceneBytes,
    ) -> Result<Self, ContextError> {
        match (&mut manifest.scenario, &scene) {
            (Scenario::RecordedScenario, SceneBytes::RecordedScenario) => {}
            (
                Scenario::CapturedScene {
                    image_sha256,
                    image_known_sha256,
                    scene_entry_sha256,
                    ..
                },
                SceneBytes::CapturedScene {
                    image,
                    image_known,
                    scene_entry,
                },
            ) => {
                normalize_hash(image_sha256, "image_sha256")?;
                normalize_hash(image_known_sha256, "image_known_sha256")?;
                normalize_hash(scene_entry_sha256, "scene_entry_sha256")?;
                if image.is_empty() || sha256(image) != *image_sha256 {
                    return Err(invalid("captured image missing or digest differs"));
                }
                if sha256(image_known) != *image_known_sha256 {
                    return Err(invalid("image known-mask digest differs"));
                }
                validate_image_known(image, image_known)?;
                if sha256(scene_entry) != *scene_entry_sha256 {
                    return Err(invalid("scene entry digest differs"));
                }
                // NKENTRY1 v1: header 16, GP 128, XMM 256, flags 8,
                // MXCSR 4, reserved 4. Semantic canonical-address/MXCSR
                // validation is mandatory again in the full MAIN loader.
                if scene_entry.len() != 416
                    || &scene_entry[..8] != b"NKENTRY1"
                    || scene_entry[8..12] != 1u32.to_le_bytes()
                    || scene_entry[12..16] != 400u32.to_le_bytes()
                    || scene_entry[412..416] != [0; 4]
                {
                    return Err(invalid(
                        "scene entry is not a complete NKENTRY1 version 1 record",
                    ));
                }
            }
            _ => {
                return Err(invalid(
                    "scenario tag and provided scene bytes differ; no fallback is allowed",
                ))
            }
        }
        if manifest.schema != CONTEXT_SCHEMA {
            return Err(invalid("unsupported loadout context schema"));
        }
        validate_provenance(&mut manifest.provenance)?;
        normalize_hash(&mut manifest.inventory_sha256, "inventory_sha256")?;
        normalize_hash(&mut manifest.frame_sha256, "frame_sha256")?;
        if inventory.len() != INVENTORY_BYTES {
            return Err(invalid(
                "inventory must contain the complete 0x2018-byte character inventory",
            ));
        }
        if sha256(&inventory) != manifest.inventory_sha256 {
            return Err(invalid("inventory digest differs from context manifest"));
        }
        if sha256(&frame) != manifest.frame_sha256 {
            return Err(invalid("frame digest differs from context manifest"));
        }
        let parsed_frame = crate::pipeline::Frame::load(&frame).map_err(invalid)?;
        if manifest.attrs.iter().any(|a| !(1..=999).contains(a)) {
            return Err(invalid(
                "attributes are outside the supported 1..=999 table domain",
            ));
        }
        let c = &manifest.character;
        if !(0..=1).contains(&c.mode) || c.active > 1 || !(0..=1).contains(&c.core_mode) {
            return Err(invalid(
                "character mode, active bank and core mode must identify one of two banks",
            ));
        }
        let mut captured_frame = if let Scenario::CapturedScene {
            attack_weapon_slot,
            main_flags_298,
            ..
        } = &manifest.scenario
        {
            if *attack_weapon_slot != 0 {
                return Err(invalid(
                    "captured attack must be normalized to the supported weapon slot 0",
                ));
            }
            let held = u32::from(inventory[c.active as usize * 0xF70 + 0xF68] == 0);
            if held != *attack_weapon_slot {
                return Err(invalid(
                    "captured attack weapon slot differs from the native bank selector",
                ));
            }
            let known_flags = known_word(&scene, parsed_frame.main_address() + 0x298)?;
            if known_flags != *main_flags_298 {
                return Err(invalid(
                    "MAIN+298 metadata differs from the known captured image",
                ));
            }
            // Both evaluators apply equipment changes over the actual scene.
            // A removed effect needs an explicit write if the captured D word
            // still contains the original outfit's contribution.
            let (address, range) = parsed_frame.damage_baseline();
            let mut bound = frame.clone();
            for (index, word) in bound[range].as_chunks_mut::<4>().0.iter_mut().enumerate() {
                if let Ok(value) = known_word(&scene, address + 4 * index as u64) {
                    *word = value.to_le_bytes();
                }
            }
            let pointer = |address: u64| -> Result<u64, ContextError> {
                let high = address
                    .checked_add(4)
                    .ok_or_else(|| invalid("source pointer overflows"))?;
                Ok(u64::from(known_word(&scene, address)?)
                    | (u64::from(known_word(&scene, high)?) << 32))
            };
            // frame.owner is the typed producer's normalized0x340000000
            // address. Resource ownership follows the actual MAIN entity
            // chain, independently of those normalized producer pointers.
            let mut actor_agility = [0u8; 4];
            let actor = pointer(parsed_frame.main_address() + 0x198).and_then(|source| {
                if matches!(known_bytes::<1>(&scene,source), Ok([tag]) if tag & 15 == 4) {
                    pointer(source.checked_add(0x260).ok_or_else(|| invalid("projectile owner field overflows"))?)
                } else {
                    Ok(source)
                }
            });
            if let Ok(player) = actor {
                for (index, offset) in [0u64, 0x886, 0x88A].into_iter().enumerate() {
                    if let Some(address) = player.checked_add(offset) {
                        if let Ok([value]) = known_bytes::<1>(&scene, address) {
                            actor_agility[index] = value;
                            actor_agility[3] |= 1 << index;
                        }
                    }
                }
                if let Some(owner_field) = player.checked_add(0x3A0) {
                    if let Ok(owner) = pointer(owner_field) {
                        if owner > 0
                            && owner.checked_add(0x164).is_some()
                            && matches!(pointer(owner), Ok(value) if value == player)
                        {
                            if let (Ok(current), Ok(maximum)) = (
                                known_word(&scene, owner + 0x15C),
                                known_word(&scene, owner + 0x160),
                            ) {
                                bound = crate::pipeline::Frame::with_owner_resource(
                                    &bound,
                                    [current, maximum],
                                )
                                .map_err(invalid)?;
                            }
                        }
                    }
                }
            }
            bound = crate::pipeline::Frame::with_actor_agility(&bound, actor_agility)
                .map_err(invalid)?;
            Some(bound)
        } else {
            None
        };
        if c.core_maps.iter().any(|map| map.len() > 256) {
            return Err(invalid("core source exceeds its native capacity"));
        }
        let mut names = std::collections::HashSet::new();
        let mut native_before = Vec::new();
        let mut native_after = Vec::new();
        for layer in &mut manifest.non_gear.layers {
            if layer.name.trim().is_empty() || !names.insert(layer.name.clone()) {
                return Err(invalid("source layer names must be nonempty and distinct"));
            }
            validate_provenance(&mut layer.provenance)?;
            if layer.provenance.game_version != manifest.provenance.game_version {
                return Err(invalid("source layer version differs from context version"));
            }
            let native = match layer.kind {
                SourceKind::NativeBeforeEquipment => Some(&mut native_before),
                SourceKind::NativeAfterSets => Some(&mut native_after),
                SourceKind::DerivedNamed | SourceKind::ObservedNonEquipment => None,
            };
            if let Some(native) = native {
                for op in &layer.operations {
                    if op.group == 0 || !(0..=u32::MAX as i64).contains(&op.value) {
                        return Err(invalid(
                            "native source submissions require nonzero u16 groups and u32 values",
                        ));
                    }
                    native.push((op.group, op.value as u32));
                }
            }
        }
        let native_frame = crate::pipeline::Frame::with_native_sources(
            captured_frame.as_ref().unwrap_or(&frame),
            &native_before,
            &native_after,
        )
        .map_err(invalid)?;
        if captured_frame.is_some() || native_frame != frame {
            captured_frame = Some(native_frame);
        }
        let canonical = serde_json::to_vec(&manifest).map_err(|e| invalid(e.to_string()))?;
        let mut hash = Sha256::new();
        hash_part(&mut hash, "context.json", &canonical);
        hash_part(&mut hash, "inventory.bin", &inventory);
        hash_part(&mut hash, "frame.bin", &frame);
        for name in ["image.bin", "image_known.bin", "scene_entry.bin"] {
            if let Some(bytes) = scene.file(name) {
                hash_part(&mut hash, name, bytes);
            }
        }
        let digest = format!("{:x}", hash.finalize());
        Ok(Self {
            manifest,
            inventory,
            frame,
            captured_frame,
            runtime_hit_inputs: parsed_frame.runtime_hit_inputs.clone(),
            scene,
            digest,
        })
    }
    pub fn manifest(&self) -> &ContextManifest {
        &self.manifest
    }
    pub fn inventory(&self) -> &[u8] {
        &self.inventory
    }
    pub fn frame(&self) -> &[u8] {
        &self.frame
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Encode once, then let both evaluators parse these exact byte buffers.
    /// Slot ranges are attack-bundle research-pool metadata, not player state.
    /// Rebind explicit character inputs while retaining the exact scene capability and bytes.
    /// Used by preparation-only refresh stages; this neither evaluates damage nor admits new scenarios.
    pub fn rebind_character_inputs(
        &self,
        manifest: ContextManifest,
        inventory: Vec<u8>,
    ) -> Result<Self, ContextError> {
        Self::new_with_scene(
            manifest,
            inventory,
            self.frame().to_vec(),
            self.scene.clone(),
        )
    }

    pub fn encode(&self, ranges: &[(u32, u32, u32)]) -> EncodedContext {
        let c = &self.manifest.character;
        let mut character = Vec::new();
        for word in [
            c.mode as u32,
            c.active,
            c.cap,
            c.plus_cap,
            c.style as u32,
            c.unarmed,
            c.level,
            c.core_mode as u32,
            c.core_maps[0].len() as u32,
            c.core_maps[1].len() as u32,
        ] {
            character.extend_from_slice(&word.to_le_bytes());
        }
        character.extend_from_slice(&self.inventory);
        for entry in c.core_maps.iter().flatten() {
            character.extend_from_slice(&entry.key.to_le_bytes());
            character.extend_from_slice(&entry.dup.to_le_bytes());
            character.extend_from_slice(&entry.base.to_le_bytes());
        }
        let mut characters = Vec::new();
        characters.extend_from_slice(&5u32.to_le_bytes());
        characters.extend_from_slice(b"char0");
        characters.extend_from_slice(&(character.len() as u32).to_le_bytes());
        characters.extend_from_slice(&character);
        let operations: Vec<_> = self
            .manifest
            .non_gear
            .layers
            .iter()
            .filter(|l| {
                !matches!(
                    l.kind,
                    SourceKind::NativeBeforeEquipment | SourceKind::NativeAfterSets
                )
            })
            .flat_map(|l| &l.operations)
            .collect();
        let mut setup = format!(
            "{} {} {}\n",
            self.manifest.non_gear.attack_gain,
            self.manifest.non_gear.defense,
            operations.len()
        );
        for operation in operations {
            setup.push_str(&format!("{} {}\n", operation.group, operation.value));
        }
        setup.push_str(&format!("{}\n", ranges.len()));
        for (slot, first, count) in ranges {
            setup.push_str(&format!("{slot} {first} {count}\n"));
        }
        setup.push_str(
            &self
                .manifest
                .attrs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
        );
        setup.push('\n');
        let setup = setup.into_bytes();
        let scene_metadata = match &self.manifest.scenario {
            Scenario::RecordedScenario => None,
            Scenario::CapturedScene {
                attack_weapon_slot,
                main_flags_298,
                ..
            } => {
                let mut bytes = [0u8; 16];
                bytes[..8].copy_from_slice(b"NKSCENE1");
                bytes[8..12].copy_from_slice(&attack_weapon_slot.to_le_bytes());
                bytes[12..16].copy_from_slice(&main_flags_298.to_le_bytes());
                Some(bytes)
            }
        };
        let frame = self.captured_frame.as_ref().unwrap_or(&self.frame);
        let hit_inputs = self.runtime_hit_inputs.clone();
        let mut hash = Sha256::new();
        hash_part(&mut hash, "characters.bin", &characters);
        hash_part(&mut hash, "gpu_setup.txt", &setup);
        hash_part(&mut hash, "frame.bin", frame);
        if let Some(bytes) = &hit_inputs {
            hash_part(&mut hash, "hits.txt", bytes);
        }
        for name in ["image.bin", "image_known.bin", "scene_entry.bin"] {
            if let Some(bytes) = self.scene.file(name) {
                hash_part(&mut hash, name, bytes);
            }
        }
        if let Some(bytes) = &scene_metadata {
            hash_part(&mut hash, "scene_metadata.bin", bytes);
        }
        EncodedContext {
            characters,
            setup,
            frame: frame.clone(),
            hit_inputs,
            scene: self.scene.clone(),
            scene_metadata,
            context_digest: self.digest.clone(),
            input_digest: format!("{:x}", hash.finalize()),
        }
    }
}

/// The shared CPU/NK_ABI 1 inputs. Fields remain private so callers cannot
/// change the GPU input after the CPU has parsed a context.
#[derive(Debug, Clone)]
pub struct EncodedContext {
    characters: Vec<u8>,
    setup: Vec<u8>,
    frame: Vec<u8>,
    hit_inputs: Option<Vec<u8>>,
    scene: SceneBytes,
    scene_metadata: Option<[u8; 16]>,
    context_digest: String,
    input_digest: String,
}
impl EncodedContext {
    pub(crate) fn read_known<const N: usize>(&self, address: u64) -> Result<[u8; N], ContextError> {
        known_bytes(&self.scene, address)
    }
    pub fn file(&self, name: &str) -> Option<&[u8]> {
        match name {
            "characters.bin" => Some(&self.characters),
            "gpu_setup.txt" => Some(&self.setup),
            "frame.bin" => Some(&self.frame),
            "hits.txt" => self.hit_inputs.as_deref(),
            "scene_metadata.bin" => self.scene_metadata.as_ref().map(|bytes| bytes.as_slice()),
            _ => self.scene.file(name),
        }
    }
    pub fn scene_metadata(&self) -> Option<(u32, u32)> {
        self.scene_metadata.as_ref().map(|bytes| {
            (
                u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
                u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            )
        })
    }
    pub fn is_captured_scene(&self) -> bool {
        matches!(self.scene, SceneBytes::CapturedScene { .. })
    }
    pub fn context_digest(&self) -> &str {
        &self.context_digest
    }
    /// Digest of the three buffers, including the bundle's pool ranges.
    pub fn input_digest(&self) -> &str {
        &self.input_digest
    }
}
