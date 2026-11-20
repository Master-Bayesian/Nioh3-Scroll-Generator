//! Private,immutable source observations. Recording failure does not invalidate a valid refresh.
//! These capsules are diagnostic game data; they never authorize writes or substitute for a process read.
use crate::{
    error::HostError,
    loadout_live::{self, RefreshEvidence, RefreshedContext},
};
use nioh3_loadout::{
    context::{sha256, LoadoutContext},
    engine::{Backpack, Candidate, Engine, Limits, Outcome, SLOTS},
};
use nioh3_runtime::combat_context::CombatSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const SCHEMA: &str = "nioh3-loadout-observation/v2";
const MAX_CAPSULE: u64 = 1024 * 1024;
const BUNDLE_FILES: [&str; 23] = [
    "bundle.json",
    "programs.txt",
    "arena.bin",
    "mapped.bin",
    "tables.bin",
    "frame.bin",
    "characters.bin",
    "hits.txt",
    "gpu_setup.txt",
    "live_groups.txt",
    "survival_groups.txt",
    "kernel.dll",
    "main_cpu.dll",
    "main_device.dll",
    "image.bin",
    "pool.bin",
    "image_known.bin",
    "event_plan.json",
    "main_cpu.so",
    "main_device.so",
    "source_program.json",
    "source_numeric.bin",
    "runtime-contract.json",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationOrigin {
    VerifiedProcessRead,
    RetainedCaptureReplay,
    ControlledRetainedInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationReference {
    pub id: String,
    pub context_sha256: String,
    pub profile_sha256: String,
    pub capture_id: String,
    pub captured_at: String,
}
impl ObservationReference {
    pub(crate) fn new(context: &LoadoutContext, profile: &Value) -> Result<Self, HostError> {
        let profile_sha256 = sha256(&serde_json::to_vec(profile).map_err(failed)?);
        let context_sha256 = context.digest().to_string();
        Ok(Self {
            id: sha256(format!("{context_sha256}:{profile_sha256}").as_bytes()),
            context_sha256,
            profile_sha256,
            capture_id: context.manifest().provenance.capture_id.clone(),
            captured_at: profile["captured_at"].as_str().unwrap_or("").to_owned(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordingOutcome {
    Recorded {
        observation_id: String,
    },
    SnapshotOnly {
        observation_id: String,
        message: String,
    },
    Unavailable {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObservationTerminal {
    Ready {
        context_sha256: String,
        source_check: Value,
        evidence: Box<RefreshEvidence>,
        profile: Value,
    },
    Rejected {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceObservation {
    pub schema: String,
    pub observation_id: String,
    pub recorded_unix_nanos: String,
    pub origin: ObservationOrigin,
    /// New capsules own their exact admission policy. Omission preserves the
    /// canonical identity and diagnostic replay of historical v2 observations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) snapshot_use: Option<loadout_live::SnapshotUse>,
    /// Selection is independent of evidence origin and process admission.
    /// Legacy capsules retain their old identity when this field is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) buff_scene: Option<crate::loadout::BuffScene>,
    pub reference: ObservationReference,
    pub bundle: String,
    pub bundle_sha256: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_input_sha256: Option<String>,
    pub save_id: String,
    pub source_sha256: String,
    pub game_version: [u16; 4],
    pub executable_sha256: String,
    pub scope: String,
    pub snapshot: CombatSnapshot,
    pub terminal: ObservationTerminal,
}

impl SourceObservation {
    fn content_id(&self) -> Result<String, HostError> {
        let mut content = self.clone();
        content.observation_id.clear();
        Ok(sha256(&serde_json::to_vec(&content).map_err(failed)?))
    }
}

fn failed(error: impl std::fmt::Display) -> HostError {
    HostError::rejected(format!("LOADOUT_OBSERVATION_INVALID: {error}"))
}
fn file_hash(path: &Path) -> Result<String, HostError> {
    use sha2::{Digest, Sha256};
    let mut input = File::open(path).map_err(failed)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = input.read(&mut buffer).map_err(failed)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
fn bundle_hashes(bundle: &Path) -> Result<BTreeMap<String, String>, HostError> {
    BUNDLE_FILES
        .into_iter()
        .filter(|name| bundle.join(name).is_file())
        .map(|name| Ok((name.to_owned(), file_hash(&bundle.join(name))?)))
        .collect()
}
fn bundle_id(hashes: &BTreeMap<String, String>) -> Result<String, HostError> {
    Ok(sha256(&serde_json::to_vec(hashes).map_err(failed)?))
}

/// A preparation directory has one owner and is never a published baseline,
/// bundle or exported context until all writes have succeeded.
struct PendingDirectory {
    path: PathBuf,
    published: bool,
}
impl PendingDirectory {
    fn new(parent: &Path) -> Result<Self, HostError> {
        fs::create_dir_all(parent).map_err(failed)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(failed)?
            .as_nanos();
        let path = parent.join(format!(".pending-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).map_err(failed)?;
        Ok(Self {
            path,
            published: false,
        })
    }
    fn publish(mut self, destination: &Path) -> Result<(), HostError> {
        if destination.exists() {
            return Err(failed("immutable publication destination already exists"));
        }
        fs::rename(&self.path, destination).map_err(failed)?;
        self.published = true;
        Ok(())
    }
}
impl Drop for PendingDirectory {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn ensure_bundle(
    directory: &Path,
    source: &Path,
    hashes: &BTreeMap<String, String>,
) -> Result<(), HostError> {
    let destination = directory.join("bundles").join(bundle_id(hashes)?);
    if destination.exists() {
        if bundle_hashes(&destination)? != *hashes {
            return Err(failed("immutable bundle snapshot differs"));
        }
        return Ok(());
    }
    let pending = PendingDirectory::new(&directory.join("bundles"))?;
    for name in hashes.keys() {
        fs::copy(source.join(name), pending.path.join(name)).map_err(failed)?;
    }
    if bundle_hashes(&pending.path)? != *hashes {
        return Err(failed("bundle changed while recording"));
    }
    pending.publish(&destination)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), HostError> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(failed)?;
    let result = file
        .write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(failed);
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}
fn write_context(directory: &Path, context: &LoadoutContext) -> Result<(), HostError> {
    write_new(
        &directory.join("context.json"),
        &serde_json::to_vec(context.manifest()).map_err(failed)?,
    )?;
    write_new(&directory.join("inventory.bin"), context.inventory())?;
    write_new(&directory.join("frame.bin"), context.frame())?;
    let encoded = context.encode(&[]);
    for name in ["image.bin", "image_known.bin", "scene_entry.bin"] {
        if let Some(bytes) = encoded.file(name) {
            write_new(&directory.join(name), bytes)?;
        }
    }
    Ok(())
}
fn ensure_reference(
    directory: &Path,
    context: &LoadoutContext,
    profile: &Value,
    reference: &ObservationReference,
) -> Result<(), HostError> {
    let destination = directory.join("baselines").join(&reference.id);
    if destination.exists() {
        let retained = LoadoutContext::load(&destination).map_err(failed)?;
        let retained_profile: Value =
            serde_json::from_slice(&fs::read(destination.join("profile.json")).map_err(failed)?)
                .map_err(failed)?;
        if ObservationReference::new(&retained, &retained_profile)?.id != reference.id {
            return Err(failed("immutable reference differs"));
        }
        return Ok(());
    }
    let pending = PendingDirectory::new(&directory.join("baselines"))?;
    write_context(&pending.path, context)?;
    write_new(
        &pending.path.join("profile.json"),
        &serde_json::to_vec(profile).map_err(failed)?,
    )?;
    pending.publish(&destination)
}

enum RecordingStage {
    Preparing,
    CapsuleSealed { observation_id: String },
}

pub(crate) struct RecordingRequest<'a> {
    pub directory: &'a Path,
    pub bundle_path: &'a Path,
    pub bundle_id: &'a str,
    pub save_id: &'a str,
    pub source_sha256: &'a str,
    pub template: &'a LoadoutContext,
    pub template_profile: &'a Value,
    pub snapshot: &'a CombatSnapshot,
    pub origin: ObservationOrigin,
    pub snapshot_use: loadout_live::SnapshotUse,
}

/// Final publication order is baseline -> complete immutable capsule -> index.
/// A failed index retains a replayable capsule; a failed journal never replaces the refresh outcome.
pub(crate) fn record(
    request: RecordingRequest<'_>,
    refreshed: Result<&RefreshedContext, &HostError>,
    metadata: &Value,
) -> RecordingOutcome {
    let mut stage = RecordingStage::Preparing;
    let result = (|| -> Result<(), HostError> {
        request.snapshot.validate().map_err(failed)?;
        let directory = request.directory.join("source-observations");
        fs::create_dir_all(&directory).map_err(failed)?;
        let reference = ObservationReference::new(request.template, request.template_profile)?;
        ensure_reference(
            &directory,
            request.template,
            request.template_profile,
            &reference,
        )?;
        let resources = bundle_hashes(request.bundle_path)?;
        ensure_bundle(&directory, request.bundle_path, &resources)?;
        if let Ok(value) = refreshed {
            let retained_bundle = directory.join("bundles").join(bundle_id(&resources)?);
            let pinned =
                Engine::open_with_context(&retained_bundle, &value.context).map_err(failed)?;
            if pinned.bundle_input_digest() != value.bundle_input_sha256 {
                return Err(failed(
                    "recorded attack data differs from source reconstruction",
                ));
            }
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(failed)?
            .as_nanos();
        let observation_id = format!("{stamp}-{}", &reference.id[..16]);
        let terminal = match refreshed {
            Ok(value) => ObservationTerminal::Ready {
                context_sha256: value.context.digest().to_string(),
                source_check: metadata["source_check"].clone(),
                evidence: Box::new(value.evidence.clone()),
                profile: metadata.clone(),
            },
            Err(error) => ObservationTerminal::Rejected {
                message: error.to_string(),
            },
        };
        let buff_scene = metadata["refresh_policy"]["buff_scene"]
            .as_str()
            .map(|_| {
                serde_json::from_value(metadata["refresh_policy"]["buff_scene"].clone())
                    .map_err(failed)
            })
            .transpose()?;
        let mut observation=SourceObservation { schema:SCHEMA.to_owned(),observation_id:observation_id.clone(),recorded_unix_nanos:stamp.to_string(),origin:request.origin,snapshot_use:Some(request.snapshot_use),buff_scene,reference,
            bundle:request.bundle_id.to_owned(),bundle_sha256:resources,bundle_input_sha256:refreshed.ok().and_then(|value|value.bundle_input_sha256.clone()),save_id:request.save_id.to_owned(),source_sha256:request.source_sha256.to_owned(),
            game_version:[2,0,2,0],executable_sha256:nioh3_runtime::INVENTORY_EXECUTABLE_SHA256.to_owned(),
            scope:"Exact logical snapshot plus retained raw source buffers; observed source residual,not a complete source-causality or all-thread-atomicity proof. No game/save write.".to_owned(),
            snapshot:request.snapshot.clone(),terminal };
        observation.observation_id = observation.content_id()?;
        let observation_id = observation.observation_id.clone();
        let bytes = serde_json::to_vec(&observation).map_err(failed)?;
        if bytes.len() as u64 > MAX_CAPSULE {
            return Err(failed("source capsule exceeds its1MiB budget"));
        }
        let pending = directory.join(format!(".{observation_id}.pending"));
        write_new(&pending, &bytes)?;
        fs::rename(&pending, directory.join(format!("{observation_id}.json"))).map_err(failed)?;
        stage = RecordingStage::CapsuleSealed {
            observation_id: observation_id.clone(),
        };
        let line = json!({"schema":SCHEMA,"observation_id":observation_id,"capsule_sha256":sha256(&bytes),"reference_id":observation.reference.id,
            "captured_at":metadata["captured_at"],"context_sha256":refreshed.ok().map(|r|r.context.digest()),"inventory_sha256":sha256(&observation.snapshot.inputs.inventory),
            "attrs":observation.snapshot.inputs.raw_attrs,"source_check":metadata["source_check"]});
        let mut index = OpenOptions::new()
            .create(true)
            .append(true)
            .open(request.directory.join("source-checks.jsonl"))
            .map_err(failed)?;
        writeln!(index, "{line}").map_err(failed)?;
        index.sync_all().map_err(failed)
    })();
    match (result, stage) {
        (Ok(()), RecordingStage::CapsuleSealed { observation_id }) => {
            RecordingOutcome::Recorded { observation_id }
        }
        (Err(error), RecordingStage::CapsuleSealed { observation_id }) => {
            RecordingOutcome::SnapshotOnly {
                observation_id,
                message: error.to_string(),
            }
        }
        (Err(error), RecordingStage::Preparing) => RecordingOutcome::Unavailable {
            message: error.to_string(),
        },
        (Ok(()), RecordingStage::Preparing) => RecordingOutcome::Unavailable {
            message: "Source capsule was not published".into(),
        },
    }
}

/// Re-execute the actual refresh from an immutable private capsule. This never reads a process.
/// Returns a reviewable receipt and the refreshed context for optional original-instruction comparison.
pub fn replay(path: &Path, bundle_path: &Path) -> Result<(Value, LoadoutContext), HostError> {
    let (receipt, context, _) = replay_owned(path, bundle_path)?;
    Ok((receipt, context))
}

fn replay_owned(
    path: &Path,
    bundle_path: &Path,
) -> Result<(Value, LoadoutContext, SourceObservation), HostError> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(failed)?
        .take(MAX_CAPSULE + 1)
        .read_to_end(&mut bytes)
        .map_err(failed)?;
    if bytes.len() as u64 > MAX_CAPSULE {
        return Err(failed("source capsule exceeds its1MiB budget"));
    }
    let observation: SourceObservation = serde_json::from_slice(&bytes).map_err(failed)?;
    if observation.schema != SCHEMA {
        return Err(failed("source capsule schema differs"));
    }
    if observation.content_id()? != observation.observation_id {
        return Err(failed("source capsule content identity differs"));
    }
    if path.file_stem().and_then(|name| name.to_str()) != Some(observation.observation_id.as_str())
    {
        return Err(failed("source capsule filename identity differs"));
    }
    if observation.game_version != [2, 0, 2, 0]
        || observation.executable_sha256 != nioh3_runtime::INVENTORY_EXECUTABLE_SHA256
    {
        return Err(failed("source capsule executable profile differs"));
    }
    if observation.reference.id.len() != 64
        || !observation
            .reference
            .id
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err(failed("invalid reference identity"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| failed("missing observation directory"))?;
    let retained_bundle;
    let bundle_path = if bundle_path == Path::new("@recorded") {
        retained_bundle = parent
            .join("bundles")
            .join(bundle_id(&observation.bundle_sha256)?);
        retained_bundle.as_path()
    } else {
        bundle_path
    };
    if bundle_hashes(bundle_path)? != observation.bundle_sha256 {
        return Err(failed("bundle resources differ from recorded observation"));
    }
    observation.snapshot.validate().map_err(failed)?;
    let baseline = parent.join("baselines").join(&observation.reference.id);
    let context = LoadoutContext::load(&baseline).map_err(failed)?;
    let mut metadata: Value =
        serde_json::from_slice(&fs::read(baseline.join("profile.json")).map_err(failed)?)
            .map_err(failed)?;
    if ObservationReference::new(&context, &metadata)? != observation.reference {
        return Err(failed("reference resources differ"));
    }
    let usage = observation
        .snapshot_use
        .unwrap_or(loadout_live::SnapshotUse::RetainedDiagnostic);
    let usage_binding = if observation.snapshot_use.is_some() {
        "recorded_policy"
    } else {
        "legacy_diagnostic_default"
    };
    let refreshed = loadout_live::refresh_snapshot_for_scene(
        &context,
        bundle_path,
        &mut metadata,
        &observation.snapshot,
        usage,
        observation
            .buff_scene
            .unwrap_or(crate::loadout::BuffScene::Current),
    );
    let expected = &observation.terminal;
    let refreshed = match (refreshed, expected) {
        (
            Ok(value),
            ObservationTerminal::Ready {
                context_sha256,
                source_check,
                evidence,
                profile: _,
            },
        ) => {
            if value.context.digest() != context_sha256
                || metadata["source_check"] != *source_check
                || value.evidence != **evidence
            {
                return Err(failed("replayed refresh differs from immutable capsule"));
            }
            if observation.bundle_input_sha256.is_some()
                && observation.bundle_input_sha256 != value.bundle_input_sha256
            {
                return Err(failed(
                    "replayed attack data differs from source reconstruction",
                ));
            }
            value
        }
        (Err(error), ObservationTerminal::Rejected { message })
            if error.to_string() == *message =>
        {
            return Ok((
                json!({"matched":true,"terminal":"rejected","message":message,"observation_id":observation.observation_id,
                    "execution_origin":"retained_capture_replay","recorded_origin":observation.origin,
                    "snapshot_use":usage,"snapshot_use_binding":usage_binding,"live_process_accessed":false}),
                context,
                observation,
            ))
        }
        (Err(error), _) => return Err(failed(format!("replayed refresh failed: {error}"))),
        (Ok(_), _) => return Err(failed("replayed refresh unexpectedly succeeded")),
    };
    let engine = Engine::open_with_context(bundle_path, &refreshed.context).map_err(failed)?;
    if engine.bundle_input_digest() != refreshed.bundle_input_sha256 {
        return Err(failed("attack bundle changed during observation replay"));
    }
    let candidate = Candidate {
        attrs: refreshed.context.manifest().attrs,
        items: [None; SLOTS],
    };
    let pack = Backpack {
        pool: Vec::new(),
        ranges: Vec::new(),
        origin: Vec::new(),
    };
    let score = match engine.evaluate(&candidate, &pack, &Limits::NONE, &mut engine.work()) {
        Outcome::Damage(bits) => {
            json!({"status":"answered","bits":bits,"damage":f32::from_bits(bits)})
        }
        Outcome::NoPath => json!({"status":"no_path"}),
        Outcome::OutsideLimits => json!({"status":"outside_limits"}),
        Outcome::Refused(reason) => json!({"status":"refused","reason":reason}),
    };
    Ok((
        json!({"matched":true,"terminal":"ready","observation_id":observation.observation_id,"origin":observation.origin,
        "execution_origin":"retained_capture_replay","recorded_origin":observation.origin,
        "snapshot_use":usage,"snapshot_use_binding":usage_binding,"live_process_accessed":false,
        "status_graph_evidence":if observation.snapshot.status_tree_raw.is_some() {"retained_bounded_nodes"} else {"legacy_logical_membership_only"},
        "secondary_channel_evidence":if observation.snapshot.inputs.stats.secondary_attack.is_some() {"observed_equal_residual"} else {"legacy_primary_only"},
        "reference":observation.reference,"context_sha256":refreshed.context.digest(),"source_check":metadata["source_check"],"score":score,
        "scope":"Actual retained-snapshot refresh and evaluator replay;not new live-game or independent source-causality acceptance."}),
        refreshed.context,
        observation,
    ))
}

/// Test authority remains file-backed and cannot authorize a game read/write.
#[cfg(debug_assertions)]
pub(crate) fn snapshot_for_offline_e2e(path: &Path) -> Result<CombatSnapshot, HostError> {
    let mut raw = Vec::new();
    File::open(path)
        .map_err(failed)?
        .take(MAX_CAPSULE + 1)
        .read_to_end(&mut raw)
        .map_err(failed)?;
    if raw.len() as u64 > MAX_CAPSULE {
        return Err(failed("source capsule exceeds its bounded size"));
    }
    let observation: SourceObservation = serde_json::from_slice(&raw).map_err(failed)?;
    if observation.schema != SCHEMA
        || observation.content_id()? != observation.observation_id
        || path.file_stem().and_then(|name| name.to_str())
            != Some(observation.observation_id.as_str())
        || observation.game_version != [2, 0, 2, 0]
        || observation.executable_sha256 != nioh3_runtime::INVENTORY_EXECUTABLE_SHA256
        || !matches!(observation.terminal, ObservationTerminal::Ready { .. })
    {
        return Err(failed(
            "offline acceptance capsule identity/profile/terminal differs",
        ));
    }
    observation.snapshot.validate().map_err(failed)?;
    Ok(observation.snapshot)
}

pub fn export_replayed_context(
    path: &Path,
    bundle_path: &Path,
    destination: &Path,
) -> Result<Value, HostError> {
    let (receipt, context, observation) = replay_owned(path, bundle_path)?;
    if destination.exists() {
        return Err(failed("context export destination already exists"));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| failed("missing export parent"))?;
    let pending = PendingDirectory::new(parent)?;
    write_context(&pending.path, &context)?;
    if let ObservationTerminal::Ready { mut profile, .. } = observation.terminal {
        profile["source_reference"] =
            serde_json::to_value(&observation.reference).map_err(failed)?;
        profile["replayed_observation_origin"] =
            serde_json::to_value(observation.origin).map_err(failed)?;
        write_new(
            &pending.path.join("profile.json"),
            &serde_json::to_vec(&profile).map_err(failed)?,
        )?;
    } else {
        return Err(failed(
            "a rejected refresh cannot be exported as a current context",
        ));
    }
    write_new(
        &pending.path.join("replay.json"),
        &serde_json::to_vec_pretty(&receipt).map_err(failed)?,
    )?;
    pending.publish(destination)?;
    Ok(receipt)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod portable_tests {
    use super::*;
    use nioh3_loadout::context::{ContextManifest, Scenario};

    fn work_directory(label: &str) -> PathBuf {
        let root = std::env::var_os("NIOH3_BUILD_ROOT")
            .map(PathBuf::from)
            .expect("build root");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = root.join("tmp").join(format!(
            "live-refresh-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn immutable_bundle_keeps_and_binds_prepared_device_and_runtime_contract() {
        let root = work_directory("device-contract");
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        for name in [
            "main_cpu.dll",
            "main_device.dll",
            "main_device.so",
            "runtime-contract.json",
        ] {
            fs::write(source.join(name), format!("original {name}")).unwrap();
        }
        let hashes = bundle_hashes(&source).unwrap();
        for name in ["main_device.dll", "main_device.so", "runtime-contract.json"] {
            assert!(
                hashes.contains_key(name),
                "missing immutable identity for {name}"
            );
        }
        let destination = root.join("recording");
        ensure_bundle(&destination, &source, &hashes).unwrap();
        let retained = destination
            .join("bundles")
            .join(bundle_id(&hashes).unwrap());
        assert_eq!(bundle_hashes(&retained).unwrap(), hashes);
        for name in ["main_device.dll", "main_device.so", "runtime-contract.json"] {
            let original = fs::read(retained.join(name)).unwrap();
            fs::write(retained.join(name), b"tampered").unwrap();
            assert!(
                ensure_bundle(&destination, &source, &hashes).is_err(),
                "tamper escaped for {name}"
            );
            fs::write(retained.join(name), original).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_publication_cleans_owned_pending_and_preserves_destination() {
        let root = work_directory("publication");
        let existing = root.join("published");
        fs::create_dir(&existing).unwrap();
        write_new(&existing.join("old"), b"old immutable bytes").unwrap();
        let pending = PendingDirectory::new(&root).unwrap();
        let pending_path = pending.path.clone();
        write_new(&pending.path.join("new"), b"new bytes").unwrap();
        assert!(pending.publish(&existing).is_err());
        assert!(
            !pending_path.exists(),
            "failed preparation leaked a pending directory"
        );
        assert_eq!(
            fs::read(existing.join("old")).unwrap(),
            b"old immutable bytes"
        );
        assert!(!existing.join("new").exists());
        assert!(write_new(&existing.join("old"), b"overwrite").is_err());
        assert_eq!(
            fs::read(existing.join("old")).unwrap(),
            b"old immutable bytes"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "Requires the packaged Nov15 capsule only; no game, save or full replay image"]
    fn legacy_fresh_read_capsule_keeps_its_exact_content_identity() {
        let evidence =
            PathBuf::from(std::env::var_os("NIOH3_REFRESH_EVIDENCE_ROOT").expect("evidence root"));
        let id = "eb44f06c141594617a44a767507a635fc443a7c217d9f61ae4ac7b60023dca77";
        let bytes = fs::read(
            evidence
                .join("live-refresh-20261115")
                .join(format!("{id}.json")),
        )
        .unwrap();
        let observation: SourceObservation = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(observation.content_id().unwrap(), id);
        observation.snapshot.validate().unwrap();
        assert!(matches!(
            observation.origin,
            ObservationOrigin::VerifiedProcessRead
        ));
        assert!(observation.snapshot_use.is_none());
        assert!(serde_json::to_value(&observation)
            .unwrap()
            .get("snapshot_use")
            .is_none());
        assert!(observation.snapshot.status_tree_raw.is_none());
        assert!(observation.snapshot.inputs.stats.secondary_attack.is_none());
        assert!(observation.snapshot.inputs.owner_flags_188.is_none());
        assert!(observation.bundle_input_sha256.is_none());
        let report = json!({"passed":true,"observation_id":id,"recomputed_content_id":observation.content_id().unwrap(),
            "recorded_origin":observation.origin,"snapshot_use_omitted":true,"historical_fields_fabricated":false,
            "live_process_accessed":false,"scope":"Canonical content identity and optional-field omission of the actual retained Nov15 v2 capsule; no new process read or full refresh replay."});
        if let Some(output) = std::env::var_os("NIOH3_LIVE_REFRESH_RESULTS") {
            let output = PathBuf::from(output);
            fs::create_dir_all(&output).unwrap();
            fs::write(
                output.join("legacy_capsule_identity.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
        }
        println!("{report}");
    }

    #[test]
    #[ignore = "Requires packaged logical capture/frame; executes rejected-state journal/replay without MAIN"]
    fn production_journal_replays_rejection_and_preserves_file_failure_boundaries() {
        let evidence =
            PathBuf::from(std::env::var_os("NIOH3_REFRESH_EVIDENCE_ROOT").expect("evidence root"));
        let root = work_directory("journal");
        let directory = root.join("context");
        fs::create_dir(&directory).unwrap();
        let bundle = root.join("bundle");
        fs::create_dir(&bundle).unwrap();
        write_new(&bundle.join("bundle.json"), b"{}\n").unwrap();
        let old_id = "eb44f06c141594617a44a767507a635fc443a7c217d9f61ae4ac7b60023dca77";
        let original: SourceObservation = serde_json::from_slice(
            &fs::read(
                evidence
                    .join("live-refresh-20261115")
                    .join(format!("{old_id}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let mut snapshot = original.snapshot;
        let frame = fs::read(evidence.join("held-context/frame.bin")).unwrap();
        let mut manifest: ContextManifest =
            serde_json::from_slice(&fs::read(evidence.join("held-context/context.json")).unwrap())
                .unwrap();
        manifest.scenario = Scenario::RecordedScenario;
        manifest.inventory_sha256 = sha256(&snapshot.inputs.inventory);
        manifest.frame_sha256 = sha256(&frame);
        manifest.provenance.capture_id = "portable-journal-rejection-control".into();
        manifest.provenance.description = "Synthetic recorded-scenario envelope over retained logical inputs; rejects form before evaluator access.".into();
        let context =
            LoadoutContext::new(manifest, snapshot.inputs.inventory.clone(), frame).unwrap();
        let metadata: Value =
            serde_json::from_slice(&fs::read(evidence.join("held-context/profile.json")).unwrap())
                .unwrap();
        snapshot.inputs.mode = 0;
        let error = loadout_live::refresh_snapshot(
            &context,
            &bundle,
            &mut metadata.clone(),
            &snapshot,
            loadout_live::SnapshotUse::RetainedDiagnostic,
        )
        .err()
        .expect("form refusal");
        assert!(error.to_string().contains("SCHOOL_MISMATCH"));
        let record_case = || {
            record(
                RecordingRequest {
                    directory: &directory,
                    bundle_path: &bundle,
                    bundle_id: "portable-journal-control",
                    save_id: "synthetic-owned-copy",
                    source_sha256: "diagnostic",
                    template: &context,
                    template_profile: &metadata,
                    snapshot: &snapshot,
                    origin: ObservationOrigin::ControlledRetainedInput,
                    snapshot_use: loadout_live::SnapshotUse::RetainedDiagnostic,
                },
                Err(&error),
                &metadata,
            )
        };
        let RecordingOutcome::Recorded { observation_id } = record_case() else {
            panic!("journal did not publish the actual rejected input")
        };
        let journal = directory.join("source-observations");
        let capsule = journal.join(format!("{observation_id}.json"));
        let first = fs::read(&capsule).unwrap();
        let (receipt, _) = replay(&capsule, Path::new("@recorded")).unwrap();
        assert_eq!(receipt["terminal"], "rejected");
        assert_eq!(receipt["execution_origin"], "retained_capture_replay");
        assert_eq!(receipt["recorded_origin"], "controlled_retained_input");
        assert_eq!(receipt["snapshot_use"], "retained_diagnostic");
        assert_eq!(receipt["snapshot_use_binding"], "recorded_policy");
        // A declared legacy-format control omits only the new admission field.
        // Its missing owner evidence must still replay the original diagnostic
        // school refusal, rather than acquiring current-process admission.
        let mut legacy_control: SourceObservation = serde_json::from_slice(&first).unwrap();
        legacy_control.snapshot_use = None;
        legacy_control.scope =
            "Controlled legacy-format omission over retained bytes; not a new game observation."
                .into();
        legacy_control.observation_id = legacy_control.content_id().unwrap();
        let legacy_path = journal.join(format!("{}.json", legacy_control.observation_id));
        write_new(&legacy_path, &serde_json::to_vec(&legacy_control).unwrap()).unwrap();
        let legacy_receipt = replay(&legacy_path, Path::new("@recorded")).unwrap().0;
        assert_eq!(legacy_receipt["terminal"], "rejected");
        assert_eq!(legacy_receipt["message"], error.to_string());
        assert_eq!(legacy_receipt["snapshot_use"], "retained_diagnostic");
        assert_eq!(
            legacy_receipt["snapshot_use_binding"],
            "legacy_diagnostic_default"
        );
        assert_eq!(
            legacy_receipt["recorded_origin"],
            "controlled_retained_input"
        );
        assert_eq!(legacy_receipt["live_process_accessed"], false);
        let index: Value = serde_json::from_str(
            fs::read_to_string(directory.join("source-checks.jsonl"))
                .unwrap()
                .trim(),
        )
        .unwrap();
        assert_eq!(index["capsule_sha256"], sha256(&first));
        let destination = root.join("must-not-export-rejected");
        assert!(export_replayed_context(&capsule, Path::new("@recorded"), &destination).is_err());
        assert!(
            !destination.exists(),
            "rejected refresh was published as a context"
        );
        assert!(!fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with(".pending-")));
        let mut tampered: Value = serde_json::from_slice(&first).unwrap();
        tampered["recorded_unix_nanos"] = json!("1");
        fs::write(&capsule, serde_json::to_vec(&tampered).unwrap()).unwrap();
        assert!(replay(&capsule, Path::new("@recorded"))
            .unwrap_err()
            .to_string()
            .contains("content identity differs"));
        fs::write(&capsule, &first).unwrap();
        let index_path = directory.join("source-checks.jsonl");
        fs::rename(&index_path, directory.join("retained-index.jsonl")).unwrap();
        fs::create_dir(&index_path).unwrap();
        let RecordingOutcome::SnapshotOnly {
            observation_id: unindexed,
            ..
        } = record_case()
        else {
            panic!("actual index I/O failure did not preserve a sealed capsule")
        };
        replay(
            &journal.join(format!("{unindexed}.json")),
            Path::new("@recorded"),
        )
        .unwrap();
        fs::remove_dir(index_path).unwrap();
        fs::rename(&journal, directory.join("retained-journal")).unwrap();
        fs::write(&journal, b"controlled directory obstacle").unwrap();
        assert!(matches!(
            record_case(),
            RecordingOutcome::Unavailable { .. }
        ));
        assert!(error.to_string().contains("SCHOOL_MISMATCH"));
        fs::remove_file(&journal).unwrap();
        fs::rename(directory.join("retained-journal"), &journal).unwrap();
        assert_eq!(fs::read(&capsule).unwrap(), first);
        let report = json!({"passed":true,"checks":["actual rejected-state refresh and exact replay",
            "legacy omission keeps diagnostic rejection and retained provenance",
            "sealed capsule before valid index","index I/O failure keeps replayable capsule",
            "journal I/O failure preserves primary refusal","capsule identity corruption refused",
            "rejected export refused without partial publication","immutable earlier capsule unchanged"],
            "full_ready_refresh_executed":false,"game_or_save_accessed":false,
            "scope":"Actual observation/refresh modules over retained source bytes and a synthetic rejection-only envelope; no full MAIN or protected-host transport acceptance."});
        if let Some(output) = std::env::var_os("NIOH3_LIVE_REFRESH_RESULTS") {
            let output = PathBuf::from(output);
            fs::create_dir_all(&output).unwrap();
            fs::write(
                output.join("journal_rejection_e2e.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
        }
        println!("{report}");
        fs::remove_dir_all(root).unwrap();
    }
}
