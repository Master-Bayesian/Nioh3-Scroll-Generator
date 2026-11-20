//! Save-owned, immutable character contexts for the outfit optimizer.
//!
//! Immutable attack templates ship with the application. A current-role read
//! binds one template to the player's freshly observed inputs and owned save.
//! Private capture templates are accepted for older installations, but a
//! production read never substitutes their recorded character for the player.
//! A refresh retires the previous binding, including when the read fails.
use crate::error::HostError;
use crate::loadout::find_bundle;
use nioh3_loadout::{context::LoadoutContext, engine::Engine};
use nioh3_runtime::combat_context::SnapshotOutcome;
use nioh3_save::character::{equipment_record, equipment_slot_is_empty, EQUIPMENT_SLOT_COUNT};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Copy)]
pub(crate) enum SearchAuthority {
    BundleDefault,
    VerifiedNativeEpoch,
}

struct BoundContext {
    authority: SearchAuthority,
    id: String,
    save_id: String,
    source_sha256: String,
    engine: Arc<Engine>,
}

#[derive(Default)]
pub struct Contexts {
    current: HashMap<(String, String), BoundContext>,
}

struct BindingOwner<'a> {
    save_id: &'a str,
    source_sha256: &'a str,
    bundle_id: &'a str,
    bundle_path: &'a Path,
    school: &'a str,
    plain: &'a [u8],
}

struct PendingBinding {
    context: LoadoutContext,
    metadata: Value,
    source: &'static str,
    authority: SearchAuthority,
    recording: Option<crate::loadout_observation::RecordingOutcome>,
    expected_bundle_digest: Option<String>,
}

fn text<'a>(params: &'a Value, field: &str) -> Result<&'a str, HostError> {
    params[field]
        .as_str()
        .ok_or_else(HostError::invalid_request)
}

/// The immutable scenario is application data; the observation journal remains
/// private state. Buff variants deliberately use their own scene id, even when
/// they share the numeric evaluator through `assets`.
fn template_directory(state_root: &Path, data_root: &Path, bundle_id: &str) -> Option<PathBuf> {
    [
        data_root.join("loadout").join(bundle_id).join("context"),
        state_root.join("loadout-contexts").join(bundle_id),
    ]
    .into_iter()
    .find(|directory| directory.join("context.json").is_file())
}

impl Contexts {
    pub fn read(
        &mut self,
        state_root: &Path,
        data_root: &Path,
        plain: &[u8],
        params: &Value,
    ) -> Result<Value, HostError> {
        #[cfg(debug_assertions)]
        if crate::offline_e2e::enabled() {
            if std::env::var_os("NIOH3_E2E_SOURCE_EPOCH").is_some() {
                return self.read_replayed_epoch(
                    data_root,
                    plain,
                    params,
                    crate::offline_e2e::epoch_receipt,
                );
            }
            return self.read_with_snapshot_source(
                state_root,
                data_root,
                plain,
                params,
                crate::offline_e2e::origin(),
                crate::loadout_live::SnapshotUse::CurrentProcess,
                crate::offline_e2e::snapshot,
            );
        }
        self.read_with_snapshot_source(
            state_root,
            data_root,
            plain,
            params,
            crate::loadout_observation::ObservationOrigin::VerifiedProcessRead,
            crate::loadout_live::SnapshotUse::CurrentProcess,
            crate::loadout_live::read,
        )
    }

    // Evidence origin and admission policy are independent. Only read() selects
    // the production source; this private seam also exercises its policy on retained bytes.
    #[allow(clippy::too_many_arguments)]
    fn read_with_snapshot_source(
        &mut self,
        state_root: &Path,
        data_root: &Path,
        plain: &[u8],
        params: &Value,
        origin: crate::loadout_observation::ObservationOrigin,
        usage: crate::loadout_live::SnapshotUse,
        snapshot_source: impl FnOnce() -> Result<Option<SnapshotOutcome>, HostError>,
    ) -> Result<Value, HostError> {
        let save_id = text(params, "save_id")?;
        let source_sha256 = text(params, "source_sha256")?;
        let bundle_id = text(params, "bundle")?;
        let key = (save_id.to_string(), bundle_id.to_string());
        // Any failure after a specific refresh is identified retires its
        // previous binding, including a removed registered attack bundle.
        self.current.remove(&key);
        let bundle = find_bundle(data_root, params)?;
        let directory = state_root.join("loadout-contexts").join(bundle_id);
        // Resolve role availability before loading the large immutable images.
        // In particular, an unloaded game must not spend time hashing an attack
        // template or accidentally turn its retained character into a result.
        let snapshot = match snapshot_source()? {
            Some(SnapshotOutcome::Ready(snapshot)) => Some(snapshot),
            Some(SnapshotOutcome::NotLoaded(_)) => {
                return Ok(
                    json!({"state":"unavailable","reason":"LOADOUT_CONTEXT_CHARACTER_NOT_LOADED"}),
                );
            }
            Some(SnapshotOutcome::Changed(_)) => {
                return Ok(json!({"state":"unavailable","reason":"LOADOUT_CONTEXT_CHANGED"}));
            }
            None if usage == crate::loadout_live::SnapshotUse::CurrentProcess => {
                return Ok(
                    json!({"state":"unavailable","reason":"LOADOUT_CONTEXT_CHARACTER_NOT_LOADED"}),
                );
            }
            None => None,
        };
        if let Some(current) = &snapshot {
            if matches!(
                origin,
                crate::loadout_observation::ObservationOrigin::VerifiedProcessRead
            ) && usage == crate::loadout_live::SnapshotUse::CurrentProcess
                && matches!(bundle.buff_scene(), crate::loadout::BuffScene::Current)
            {
                if let Some(epoch) =
                    crate::loadout_epoch::BoundLiveEpoch::acquire(state_root, bundle_id, current)?
                {
                    return self.publish(
                        BindingOwner {
                            save_id,
                            source_sha256,
                            bundle_id,
                            bundle_path: &epoch.bundle_path,
                            school: &bundle.school,
                            plain,
                        },
                        PendingBinding {
                            context: epoch.context,
                            metadata: epoch.profile,
                            source: "live",
                            authority: SearchAuthority::VerifiedNativeEpoch,
                            recording: None,
                            expected_bundle_digest: None,
                        },
                        |engine| epoch.verification.verify(engine),
                    );
                }
            }
        }
        let Some(template_directory) = template_directory(state_root, data_root, bundle_id) else {
            return Ok(json!({"state":"unavailable","reason":"LOADOUT_CONTEXT_MISSING"}));
        };
        let context = LoadoutContext::load(&template_directory)
            .map_err(|e| HostError::rejected(format!("LOADOUT_CONTEXT_INVALID: {e}")))?;
        let mut metadata: Value = serde_json::from_slice(
            &std::fs::read(template_directory.join("profile.json"))
                .map_err(|e| HostError::rejected(format!("LOADOUT_CONTEXT_INVALID: {e}")))?,
        )
        .map_err(|e| HostError::rejected(format!("LOADOUT_CONTEXT_INVALID: {e}")))?;
        let bundle_path = bundle.directory(data_root);
        // Every scenario needs the current character, including theoretical
        // buff projections. A fixed buff is not permission to use a sample role.
        let template_profile = metadata.clone();
        let (context, source, recording, expected_bundle_digest) = match snapshot {
            Some(snapshot) => {
                snapshot.validate().map_err(|error| {
                    HostError::rejected(format!("LOADOUT_CONTEXT_INVALID: {error}"))
                })?;
                let input = &snapshot.inputs;
                let school = if input.mode == 1 { "ninja" } else { "samurai" };
                if bundle.school != school {
                    return Ok(
                        json!({"state":"unavailable","reason":"LOADOUT_CONTEXT_SCHOOL_MISMATCH"}),
                    );
                }
                let offset =
                    input.active_bank as usize * 0xF70 + input.held_weapon_slot as usize * 0xE8;
                let item =
                    u16::from_le_bytes([input.inventory[offset], input.inventory[offset + 1]]);
                if let Some(class) = bundle.selection_weapon_class.or(bundle.weapon_class) {
                    if crate::item_kinds::type_class(data_root, item) != Some(class) {
                        return Ok(
                            json!({"state":"unavailable","reason":"LOADOUT_CONTEXT_WEAPON_MISMATCH"}),
                        );
                    }
                }
                let reference = crate::loadout_observation::ObservationReference::new(
                    &context,
                    &template_profile,
                )?;
                let refreshed = crate::loadout_live::refresh_snapshot_for_scene(
                    &context,
                    &bundle_path,
                    &mut metadata,
                    &snapshot,
                    usage,
                    bundle.buff_scene(),
                );
                let recording = crate::loadout_observation::record(
                    crate::loadout_observation::RecordingRequest {
                        directory: &directory,
                        bundle_path: &bundle_path,
                        bundle_id,
                        save_id,
                        source_sha256,
                        template: &context,
                        template_profile: &template_profile,
                        snapshot: &snapshot,
                        origin,
                        snapshot_use: usage,
                    },
                    refreshed.as_ref(),
                    &metadata,
                );
                metadata["source_reference"] =
                    serde_json::to_value(reference).map_err(|error| {
                        HostError::rejected(format!("LOADOUT_CONTEXT_INVALID: {error}"))
                    })?;
                let refreshed = refreshed?;
                let source = match origin {
                    crate::loadout_observation::ObservationOrigin::VerifiedProcessRead => "live",
                    crate::loadout_observation::ObservationOrigin::RetainedCaptureReplay
                    | crate::loadout_observation::ObservationOrigin::ControlledRetainedInput => {
                        "captured"
                    }
                };
                (
                    refreshed.context,
                    source,
                    Some(recording),
                    refreshed.bundle_input_sha256,
                )
            }
            // Only the explicitly diagnostic test/replay seam may evaluate a
            // retained role. This cannot be reached by a normal app read.
            None => (context, "captured", None, None),
        };
        self.publish(
            BindingOwner {
                save_id,
                source_sha256,
                bundle_id,
                bundle_path: &bundle_path,
                school: &bundle.school,
                plain,
            },
            PendingBinding {
                context,
                metadata,
                source,
                recording,
                expected_bundle_digest,
                authority: SearchAuthority::BundleDefault,
            },
            |_| Ok(()),
        )
    }

    #[cfg(debug_assertions)]
    #[cfg(any(debug_assertions, test))]
    fn read_replayed_epoch(
        &mut self,
        data_root: &Path,
        plain: &[u8],
        params: &Value,
        receipt_source: impl FnOnce() -> Result<PathBuf, HostError>,
    ) -> Result<Value, HostError> {
        let save_id = text(params, "save_id")?;
        let source_sha256 = text(params, "source_sha256")?;
        let bundle_id = text(params, "bundle")?;
        self.current
            .remove(&(save_id.to_owned(), bundle_id.to_owned()));
        let bundle = find_bundle(data_root, params)?;
        if !matches!(bundle.buff_scene(), crate::loadout::BuffScene::Current) {
            return Err(HostError::rejected(
                "LOADOUT_SOURCE_EPOCH_CURRENT_SCENE_REQUIRED",
            ));
        }
        let bundle_path = bundle.directory(data_root);
        let prepared = crate::loadout_epoch::PreparedReplayEpoch::load(
            bundle_id,
            &bundle_path,
            &receipt_source()?,
        )?;
        self.publish(
            BindingOwner {
                save_id,
                source_sha256,
                bundle_id,
                bundle_path: &bundle_path,
                school: &bundle.school,
                plain,
            },
            PendingBinding {
                context: prepared.context,
                metadata: prepared.profile,
                source: "captured",
                authority: SearchAuthority::BundleDefault,
                recording: None,
                expected_bundle_digest: None,
            },
            |engine| prepared.receipt.verify_engine(engine),
        )
    }

    fn publish(
        &mut self,
        owner: BindingOwner<'_>,
        pending: PendingBinding,
        verify_engine: impl FnOnce(&Engine) -> Result<(), HostError>,
    ) -> Result<Value, HostError> {
        let BindingOwner {
            save_id,
            source_sha256,
            bundle_id,
            bundle_path,
            school: expected_school,
            plain,
        } = owner;
        let PendingBinding {
            context,
            metadata,
            source,
            recording,
            expected_bundle_digest,
            authority,
        } = pending;
        let manifest = context.manifest();
        let school = if manifest.character.mode == 1 {
            "ninja"
        } else {
            "samurai"
        };
        if expected_school != school {
            return Ok(json!({"state":"unavailable","reason":"LOADOUT_CONTEXT_SCHOOL_MISMATCH"}));
        }
        // E8 bytes are the native item body. The save's worn-slot trailers
        // belong to the save and are not used to fabricate live-bank records.
        let inventory = context.inventory();
        let bank = manifest.character.active as usize * 0xF70;
        for slot in [0usize, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15] {
            let record = &inventory[bank + slot * 0xE8..bank + (slot + 1) * 0xE8];
            if equipment_slot_is_empty(record) {
                continue;
            }
            let mut found = false;
            if slot == 15 {
                // Native slot 15 is the equipped scroll. Its exact E8 body
                // belongs to the separate scroll bank, not the F0 gear array.
                for index in 0..nioh3_save::layout::SCROLL_SLOT_COUNT {
                    if let Some(offset) = nioh3_save::layout::slot_offset(index) {
                        if plain.get(offset..offset + 0xE8) == Some(record) {
                            found = true;
                            break;
                        }
                    }
                }
            } else {
                for index in 0..EQUIPMENT_SLOT_COUNT {
                    let saved = equipment_record(plain, index).map_err(HostError::from_save)?;
                    if saved.get(..0xE8) == Some(record) {
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                return Ok(
                    json!({"state":"unavailable","reason":"LOADOUT_CONTEXT_CHARACTER_MISMATCH"}),
                );
            }
        }
        let engine = Arc::new(
            Engine::open_with_context(bundle_path, &context)
                .map_err(|e| HostError::rejected(format!("LOADOUT_CONTEXT_INVALID: {e}")))?,
        );
        let bundle_input_digest = engine.bundle_input_digest();
        if expected_bundle_digest.is_some() && bundle_input_digest != expected_bundle_digest {
            return Err(HostError::rejected("LOADOUT_CONTEXT_BUNDLE_CHANGED"));
        }
        verify_engine(&engine)?;
        let identity = json!([
            bundle_id,
            context.digest(),
            save_id,
            source_sha256,
            bundle_input_digest
        ]);
        let id = format!("{:x}", Sha256::digest(identity.to_string().as_bytes()));
        let mut result = json!({
            "state":"ready", "context_id":id, "save_id":save_id, "source_sha256":source_sha256,
            "bundle":bundle_id, "captured_at":metadata["captured_at"],
            "attrs":manifest.attrs, "level":manifest.character.level, "school":school,
            "source":source, "skills":metadata["skills"], "buffs":metadata["buffs"], "notes":metadata["notes"],
        });
        if let Some(recording) = recording {
            result["source_check"] = metadata["source_check"].clone();
            result["source_reference"] = metadata["source_reference"].clone();
            result["source_recording"] = serde_json::to_value(recording).map_err(|error| {
                HostError::rejected(format!("LOADOUT_CONTEXT_INVALID: {error}"))
            })?;
        }
        self.current.insert(
            (save_id.to_string(), bundle_id.to_string()),
            BoundContext {
                authority,
                id,
                save_id: save_id.to_string(),
                source_sha256: source_sha256.to_string(),
                engine,
            },
        );
        Ok(result)
    }

    pub fn engine(&self, params: &Value) -> Result<Arc<Engine>, HostError> {
        Ok(self.search_binding(params)?.0)
    }

    pub(crate) fn search_binding(
        &self,
        params: &Value,
    ) -> Result<(Arc<Engine>, SearchAuthority), HostError> {
        let save_id = text(params, "save_id")?;
        let source_sha256 = text(params, "source_sha256")?;
        let context_id = text(params, "context_id")?;
        let found = self
            .current
            .iter()
            .find(|((_, bundle), value)| {
                value.id == context_id
                    && value.save_id == save_id
                    && value.source_sha256 == source_sha256
                    && params
                        .get("bundle")
                        .is_none_or(|requested| requested == bundle)
            })
            .map(|(_, value)| (Arc::clone(&value.engine), value.authority));
        found.ok_or_else(|| HostError::rejected("OPTIMIZER_CONTEXT_STALE"))
    }
}

#[cfg(test)]
#[path = "loadout_context_e2e.rs"]
mod observation_e2e;

#[cfg(test)]
#[path = "loadout_product_e2e.rs"]
mod product_e2e;

#[cfg(all(test, debug_assertions))]
#[path = "loadout_epoch_e2e.rs"]
mod epoch_e2e;

/// Compose a live-generation binding after checking the actual prepared frame
/// and independently replayed epoch. Used by the current capture finalizer.
#[doc(hidden)]
pub fn prepare_live_epoch_binding(
    id: &str,
    bundle: &Path,
    receipt: &Path,
    snapshot: &Path,
) -> Result<Vec<u8>, HostError> {
    let source: nioh3_runtime::combat_context::CombatSnapshot = serde_json::from_slice(
        &std::fs::read(snapshot).map_err(|e| HostError::rejected(e.to_string()))?,
    )
    .map_err(|e| HostError::rejected(e.to_string()))?;
    let prepared = crate::loadout_epoch::PreparedReplayEpoch::load(id, bundle, receipt)?;
    let binding = prepared.binding_bytes(&source)?;
    let engine = Engine::open_with_context(bundle, &prepared.context)
        .map_err(|e| HostError::rejected(e.to_string()))?;
    prepared.receipt.verify_engine(&engine)?;
    Ok(binding)
}
