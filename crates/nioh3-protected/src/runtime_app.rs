//! Port of `nioh3_scroll_editor/runtime_application.RuntimeApplication`.
//!
//! The runtime role keeps live-game ownership outside the killable read-only
//! search worker. The parts of that ownership a broker crash must not be able
//! to discard are modelled in full:
//!
//! - `runtime.status` never touches the game. It publishes the override
//!   session, the retired native calls and the live-addition ownership, exactly
//!   as `RuntimeApplication.status` does, and it prunes the retired list the
//!   same way.
//! - `runtime.start_override` arms the shipped auxiliary descriptor hook and/or
//!   the PC v2.01 challenge-capacity hook through the concrete Windows sessions.
//!   Ownership is retained before the first byte is written and kept whenever a
//!   rollback cannot be confirmed, so an ambiguous target is never reported as
//!   clean.
//! - `runtime.stop_override` and the process `finally` block restore through the
//!   same session objects the start used, so EOF is not permission to abandon a
//!   hook.
//! - `runtime.live_add_*` and `runtime.live_batch_*` are the reviewed insertion
//!   surface. They own a real backup and a real native transport.
//!
//! The three scan methods (`generate`, `search`, `capture_grace`) need a running
//! game. They run the ported shipped loops over the batch oracle: the measured
//! Grace and primary maps, the joint constraint solver, the accelerated Grace
//! enumeration and the plain seed scan all live in [`crate::scan`],
//! [`crate::maps`] and [`crate::grace_capture`]. With no game they fail at the
//! same point the shipped host does - the verified game identity - rather than
//! fabricating an answer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use nioh3_worker::engine::EngineContext;

#[cfg(windows)]
use crate::app::FinalizeStep;
use crate::app::{JobContext, Role, RoleApplication};
use crate::error::HostError;

/// The one ownership reason a runtime host without a Windows binding can give.
///
/// `shutdown` and the finalization decision both use it so the control plane's
/// error text, the degraded `finalization.reason` and the shutdown reply cannot
/// drift apart.
pub const NON_WINDOWS_OWNERSHIP_REASON: &str = "the runtime adapter requires Windows";

/// The `CountEdit` object the protected contract publishes for `runtime.count_*`.
fn count_status_json(status: &nioh3_runtime::mutation::CountStatus) -> Value {
    serde_json::json!({
        "operation_id": status.operation_id,
        "plan_digest": status.plan_digest,
        "state": status.state.as_str(),
        "seed": status.seed,
        "rarity": status.rarity,
        "old_count": status.old_count,
        "new_count": status.new_count,
        "error": status.error,
    })
}

/// The protected runtime role.
pub struct RuntimeApplication {
    compatibility: crate::compatibility::CompatibilitySession,
    state_root: PathBuf,
    data_root: PathBuf,
    context: Option<EngineContext>,
    context_loader: Option<DeferredContext>,
    /// Candidates the most recent native generation published, keyed by id.
    candidates: HashMap<String, Value>,
    /// The override/live-add ownership state machine.
    #[cfg(windows)]
    host: nioh3_runtime::mutation::WindowsMutationHost,
    /// `enemy_role_by_lookup_key` over the shipped roster, loaded once.
    #[cfg(windows)]
    roster_roles: Option<std::collections::BTreeMap<u32, u8>>,
    /// The reviewed live-addition application, created on first use.
    #[cfg(windows)]
    live_add: Option<nioh3_runtime::mutation::LiveAddApplication>,
    /// The game process lifetime `(pid, creation time)` `live_add` is bound to.
    #[cfg(windows)]
    live_add_process: Option<(u32, u64)>,
    #[cfg(windows)]
    live_add_context: Option<String>,
    #[cfg(windows)]
    equipment_add: Option<nioh3_runtime::mutation::equipment_add::EquipmentAddition>,
    #[cfg(windows)]
    equipment_add_process: Option<(u32, u64)>,
    /// The product tables the offline preview composition reads, loaded once.
    #[cfg(windows)]
    preview: Option<Box<nioh3_data::PreviewResources>>,
    /// Offline generation resource version for the preview tables. `None`
    /// selects the shipped legacy resource; the release default is the version
    /// this build ships with.
    #[cfg(windows)]
    resource_version: Option<(u16, u16, u16, u16)>,
    /// Composed auxiliary halves, keyed by seed: one scan can revisit a seed
    /// across batches, and the shipped generator is deterministic per seed.
    #[cfg(windows)]
    auxiliary_cache: HashMap<u32, nioh3_domain::preview::AuxiliaryPreview>,
    /// Oracle owners whose allocation or remote thread is not yet proven
    /// released. These receipts outlive the job that produced them.
    #[cfg(windows)]
    retired_oracles: Vec<RetiredOracleOwner>,
}

struct DeferredContext {
    contract_dir: PathBuf,
    accelerator: Option<PathBuf>,
    selection: Option<nioh3_worker::engine::ContextSelection>,
}

#[cfg(windows)]
enum RetiredOracleOwner {
    Native(nioh3_runtime::mutation::oracle::OracleRetirement),
    #[cfg(feature = "test-fake")]
    Scripted {
        release_file: Option<PathBuf>,
    },
}

#[cfg(windows)]
impl RetiredOracleOwner {
    fn refresh(&self) -> bool {
        match self {
            Self::Native(owner) => owner.refresh(),
            #[cfg(feature = "test-fake")]
            Self::Scripted { release_file } => {
                release_file.as_ref().is_some_and(|path| path.is_file())
            }
        }
    }

    fn error(&self) -> Option<String> {
        match self {
            Self::Native(owner) => owner.snapshot().error,
            #[cfg(feature = "test-fake")]
            Self::Scripted { .. } => Some("scripted remote call still owns cleanup".to_string()),
        }
    }
}

/// The directory that holds the `game_versions/*.json` runtime profiles.
///
/// Workers are launched with `--data-root <runtime>/data`, while the profile
/// documents live in `data/game_versions`; a root that already names the
/// profile directory is used unchanged. Every runtime identity resolves its
/// profile through this one rule, so no path joins `data/pc_v2_02.json`.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn profile_dir_for(data_root: &Path) -> PathBuf {
    let nested = data_root.join("game_versions");
    if nested.is_dir() {
        nested
    } else {
        data_root.to_path_buf()
    }
}

/// Where every native live-add executor keeps its receipts and admission lock.
fn native_executor_directory(state_root: &Path) -> PathBuf {
    state_root.join("live-add").join("native-executor")
}

#[cfg(any(windows, test))]
fn require_same_runtime_identity(
    expected: &crate::compatibility::ExecutableIdentity,
    actual: &crate::compatibility::ExecutableIdentity,
) -> Result<(), HostError> {
    if expected.pid != actual.pid
        || expected.creation_filetime != actual.creation_filetime
        || !expected.path.eq_ignore_ascii_case(&actual.path)
        || expected.version != actual.version
        || !expected.sha256.eq_ignore_ascii_case(&actual.sha256)
    {
        return Err(HostError::coded("COMPATIBILITY_IDENTITY_CHANGED", format!(
            "Expected PID {} creation {} version {} image {} SHA {}; detected PID {} creation {} version {} image {} SHA {}. Reconnect to the current game and prepare/review a new plan; recover any uncertain receipt before retrying.",
            expected.pid, expected.creation_filetime, expected.version, expected.path, expected.sha256,
            actual.pid, actual.creation_filetime, actual.version, actual.path, actual.sha256,
        )));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod compatibility_identity_tests {
    use super::require_same_runtime_identity;
    use crate::compatibility::ExecutableIdentity;

    #[test]
    fn changed_identity_requires_reconnection_before_admission() {
        let expected = ExecutableIdentity {
            pid: 7,
            creation_filetime: 88,
            path: "D:/Game/Nioh3.exe".into(),
            version: "2.0.2.0".into(),
            sha256: "aabb".into(),
        };
        for field in ["pid", "birth", "path", "version", "hash"] {
            let mut actual = expected.clone();
            match field {
                "pid" => actual.pid += 1,
                "birth" => actual.creation_filetime += 1,
                "path" => actual.path = "D:/Other/Nioh3.exe".into(),
                "version" => actual.version = "2.0.1.0".into(),
                _ => actual.sha256 = "ccdd".into(),
            }
            let error = require_same_runtime_identity(&expected, &actual)
                .err()
                .unwrap();
            assert_eq!(error.code, Some("COMPATIBILITY_IDENTITY_CHANGED"));
            assert!(error.message.contains("Expected PID 7 creation 88"));
            assert!(error.message.contains("Reconnect"));
        }
        let mut actual = expected.clone();
        actual.path = actual.path.to_ascii_uppercase();
        actual.sha256 = actual.sha256.to_ascii_uppercase();
        assert!(require_same_runtime_identity(&expected, &actual).is_ok());
    }
}

impl RuntimeApplication {
    /// Build the runtime role for one state root and data root.
    pub fn new(
        state_root: PathBuf,
        data_root: &Path,
        context: EngineContext,
    ) -> Result<Self, HostError> {
        Self::with_context(state_root, data_root, Some(context), None)
    }

    /// A runtime host can inspect, recover and add equipment without loading
    /// unrelated offline generation tables or selecting an installed image.
    pub fn deferred(
        state_root: PathBuf,
        data_root: &Path,
        contract_dir: &Path,
        accelerator: Option<PathBuf>,
        selection: Option<nioh3_worker::engine::ContextSelection>,
    ) -> Result<Self, HostError> {
        Self::with_context(
            state_root,
            data_root,
            None,
            Some(DeferredContext {
                contract_dir: contract_dir.to_path_buf(),
                accelerator,
                selection,
            }),
        )
    }

    fn generation_context(&self) -> Result<&EngineContext, HostError> {
        self.context.as_ref().ok_or_else(|| HostError::rejected(
            "Generation resources are not loaded for this operation; prepare it again after selecting a supported game"))
    }

    /// Ordinary completion is a read-only PC 2.02 resource operation. It must
    /// work with the game closed and must not replace a live target's context.
    fn completion_prediction(&self, params: &Value) -> Result<Value, HostError> {
        if let Some(loader) = &self.context_loader {
            let selection =
                loader
                    .selection
                    .unwrap_or(nioh3_worker::engine::ContextSelection::Production(
                        nioh3_worker::GameFileVersion(2, 0, 2, 0),
                    ));
            let engine = nioh3_worker::engine::Engine::load(
                &self.data_root,
                &loader.contract_dir,
                loader.accelerator.clone(),
                selection,
            )
            .map_err(|error| {
                HostError::coded(
                    error.code,
                    format!(
                        "{}; restore the resource files for this operation and prepare again",
                        error.message
                    ),
                )
            })?;
            return crate::scroll_completion::prediction_json(
                &self.data_root,
                engine.context(),
                params,
            );
        }
        crate::scroll_completion::prediction_json(
            &self.data_root,
            self.generation_context()?,
            params,
        )
    }

    fn ensure_generation_context(&mut self) -> Result<(), HostError> {
        if self.context.is_some() {
            return Ok(());
        }
        let loader = self
            .context_loader
            .as_ref()
            .ok_or_else(HostError::invalid_request)?;
        let selection = match loader.selection {
            Some(value) => value,
            None => {
                #[cfg(windows)]
                {
                    let identity = crate::compatibility::running_identity()?;
                    let version = nioh3_runtime::file_version(&identity.path)
                        .map_err(HostError::from_runtime)?;
                    let (a, b, c, d) = version.tuple();
                    nioh3_worker::engine::ContextSelection::Production(
                        nioh3_worker::GameFileVersion(a, b, c, d),
                    )
                }
                #[cfg(not(windows))]
                {
                    return Err(HostError::from_runtime(
                        nioh3_runtime::RuntimeError::UnsupportedPlatform,
                    ));
                }
            }
        };
        let engine = nioh3_worker::engine::Engine::load(
            &self.data_root,
            &loader.contract_dir,
            loader.accelerator.clone(),
            selection,
        )
        .map_err(|error| {
            HostError::coded(
                error.code,
                format!(
                    "{}; restore the resource files for this operation and prepare again",
                    error.message
                ),
            )
        })?;
        let context = engine.context().clone();
        #[cfg(windows)]
        {
            self.resource_version = match &context {
                EngineContext::Production(c) => {
                    let v = c.game_file_version;
                    Some((v.0, v.1, v.2, v.3))
                }
                EngineContext::LegacyTest(_) => Some(nioh3_data::CURRENT_RESOURCE_VERSION),
            };
        }
        self.context = Some(context);
        Ok(())
    }

    fn with_context(
        state_root: PathBuf,
        data_root: &Path,
        context: Option<EngineContext>,
        context_loader: Option<DeferredContext>,
    ) -> Result<Self, HostError> {
        // A live add whose process died (in this or any earlier version) may
        // have left its admission lock behind; one nobody holds is cleared
        // here so it can never block the player's next addition.
        let _ = nioh3_runtime::mutation::native_executor::AdmissionLock::reset(
            &native_executor_directory(&state_root),
        );
        #[cfg(windows)]
        let selected_resource_version = match &context {
            Some(EngineContext::Production(c)) => {
                let v = c.game_file_version;
                Some((v.0, v.1, v.2, v.3))
            }
            Some(EngineContext::LegacyTest(_)) => Some(nioh3_data::CURRENT_RESOURCE_VERSION),
            None => None,
        };
        Ok(Self {
            compatibility: Default::default(),
            state_root,
            data_root: data_root.to_path_buf(),
            context,
            context_loader,
            candidates: HashMap::new(),
            #[cfg(windows)]
            host: nioh3_runtime::mutation::WindowsMutationHost::new(),
            #[cfg(windows)]
            roster_roles: None,
            #[cfg(windows)]
            live_add: None,
            #[cfg(windows)]
            live_add_process: None,
            #[cfg(windows)]
            live_add_context: None,
            #[cfg(windows)]
            equipment_add: None,
            #[cfg(windows)]
            equipment_add_process: None,
            #[cfg(windows)]
            preview: None,
            #[cfg(windows)]
            resource_version: selected_resource_version,
            #[cfg(windows)]
            auxiliary_cache: HashMap::new(),
            #[cfg(windows)]
            retired_oracles: Vec::new(),
        })
    }

    /// Clear a live-add admission lock no running executor holds.
    fn reset_live_add_lock(&self) -> Result<Value, HostError> {
        let state = nioh3_runtime::mutation::native_executor::AdmissionLock::reset(
            &native_executor_directory(&self.state_root),
        )
        .map_err(HostError::from_runtime)?;
        Ok(serde_json::json!({ "state": state.as_str() }))
    }

    /// A count editor bound to the running game.
    ///
    /// `prepare`/`execute` own one explicit write to the live process, so they
    /// need the real module base; with no game this fails with the shipped
    /// process-absence error instead of fabricating a plan.
    #[cfg(windows)]
    fn count_editor_for_game(
        &self,
        approved: &crate::compatibility::ExecutableIdentity,
    ) -> Result<nioh3_runtime::mutation::CountEditor, HostError> {
        // The count edit reads the same manager-owned inventory the live-add
        // binding reads, so it resolves the game under that purpose and picks
        // the layout of the exact running version; any other build is refused
        // instead of being read through the PC v2.01 addresses.
        let identity = nioh3_runtime::identify_running_game_for(
            &profile_dir_for(&self.data_root),
            nioh3_runtime::profile::ProfilePurpose::LiveAdd,
        )
        .map_err(HostError::from_runtime)?;
        Self::require_resolved_identity(approved, &identity)?;
        let version = identity.file_version.tuple();
        let layout =
            nioh3_runtime::mutation::count_layout_for_game_version(version).ok_or_else(|| {
                HostError::rejected("Count editing is not accepted for this game version")
            })?;
        let memory = nioh3_runtime::mutation::WindowsCountMemory::new(
            identity.identity.pid,
            identity.module.base,
            layout,
            nioh3_runtime::mutation::WindowsCountProcesses,
        )
        .with_expected_creation(approved.creation_filetime);
        nioh3_runtime::mutation::CountEditor::new(&self.state_root, Box::new(memory))
            .map_err(HostError::from_runtime)
    }

    #[cfg(not(windows))]
    fn count_editor_for_game(
        &self,
        _approved: &crate::compatibility::ExecutableIdentity,
    ) -> Result<(), HostError> {
        Err(HostError::from_runtime(
            nioh3_runtime::RuntimeError::UnsupportedPlatform,
        ))
    }

    /// A count editor for the plan-only methods (`status`, `recover`).
    ///
    /// Neither reads the target, so the memory binding is never opened; the
    /// shipped editor answers them without a running game too.
    #[cfg(windows)]
    fn count_editor_for_plans(&self) -> Result<nioh3_runtime::mutation::CountEditor, HostError> {
        let memory = nioh3_runtime::mutation::WindowsCountMemory::new(
            0,
            0,
            nioh3_runtime::mutation::PC_V201_COUNT_LAYOUT,
            nioh3_runtime::mutation::WindowsCountProcesses,
        );
        nioh3_runtime::mutation::CountEditor::new(&self.state_root, Box::new(memory))
            .map_err(HostError::from_runtime)
    }

    #[cfg(not(windows))]
    fn count_editor_for_plans(&self) -> Result<(), HostError> {
        Err(HostError::from_runtime(
            nioh3_runtime::RuntimeError::UnsupportedPlatform,
        ))
    }

    fn count_prepare(&mut self, source: &Value, new_count: i64) -> Result<Value, HostError> {
        let approved = self.admit_runtime_feature("live_count_edit")?;
        if !self.safe_to_shutdown()? {
            return Err(HostError::rejected(
                "Stop temporary overrides before editing remaining count",
            ));
        }
        let save_path = source
            .get("save_path")
            .and_then(Value::as_str)
            .ok_or_else(HostError::invalid_request)?;
        let source_sha256 = source
            .get("source_sha256")
            .and_then(Value::as_str)
            .ok_or_else(HostError::invalid_request)?;
        let record_hex = source
            .get("record_hex")
            .and_then(Value::as_str)
            .ok_or_else(HostError::invalid_request)?;
        let stem = Path::new(save_path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("save");
        let backup_path = self
            .state_root
            .join("count-backups")
            .join(format!("{stem}.bin"));
        let mut editor = self.count_editor_for_game(&approved)?;
        let status = editor
            .prepare(
                Path::new(save_path),
                source_sha256,
                record_hex,
                &backup_path,
                new_count,
            )
            .map_err(HostError::from_runtime)?;
        Ok(count_status_json(&status))
    }

    fn count_execute(&mut self, operation_id: &str, plan_digest: &str) -> Result<Value, HostError> {
        let approved = self.admit_runtime_feature("live_count_edit")?;
        if !self.safe_to_shutdown()? {
            return Err(HostError::rejected(
                "Stop temporary overrides before editing remaining count",
            ));
        }
        let mut editor = self.count_editor_for_game(&approved)?;
        Ok(count_status_json(
            &editor
                .execute(operation_id, plan_digest)
                .map_err(HostError::from_runtime)?,
        ))
    }

    fn count_status(&mut self, operation_id: &str) -> Result<Value, HostError> {
        let editor = self.count_editor_for_plans()?;
        Ok(count_status_json(
            &editor
                .status(operation_id)
                .map_err(HostError::from_runtime)?,
        ))
    }

    fn count_recover(&mut self, operation_id: &str) -> Result<Value, HostError> {
        let mut editor = self.count_editor_for_plans()?;
        Ok(count_status_json(
            &editor
                .recover(operation_id)
                .map_err(HostError::from_runtime)?,
        ))
    }
}

// ---------------------------------------------------------------------------
// Windows: the real ownership, live-addition and native-oracle surface.
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod imp {
    use std::collections::BTreeMap;

    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};

    use nioh3_runtime::mutation::live_add::LiveAddApplication;
    use nioh3_runtime::mutation::session::{OverrideSession, WindowsSessionMemory};
    use nioh3_runtime::mutation::trampoline::{EnemyGroup, OverrideProfile};
    use nioh3_runtime::mutation::{
        catalog::DomainCatalogPolicy, native_abi::live_add_binding_for_game_version,
        native_executor::NativeDebugTransport, native_executor::NativeLiveAddExecutor,
        ChallengeOverrideProfile, LiveAddBatch,
    };
    use nioh3_runtime::{GameIdentity, NativeRuntimeProfile};

    use super::{
        finalize_reason, FinalizeStep, HostError, JobContext, PathBuf, RetiredOracleOwner, Role,
        RoleApplication, RuntimeApplication,
    };
    use crate::oracle::{BatchOracle, NativeOracle};
    use crate::scan::{
        AuxiliaryCriteria, AuxiliarySource, ScanFilters, ScanMatch, ScanProgress, ScanRequest,
    };

    /// The game session the scan's batch oracle drives.
    type RemoteSession = nioh3_runtime::mutation::win_session::WindowsRemoteSession;

    /// The oracle one scan owns.
    enum OracleHandle {
        Native(Box<NativeOracle>),
        #[cfg(feature = "test-fake")]
        Scripted {
            oracle: Box<crate::oracle::scripted::ScriptedOracle>,
            release_file: Option<PathBuf>,
        },
    }

    impl OracleHandle {
        fn batch(&mut self) -> &mut dyn BatchOracle {
            match self {
                OracleHandle::Native(oracle) => oracle.as_mut(),
                #[cfg(feature = "test-fake")]
                OracleHandle::Scripted { oracle, .. } => oracle.as_mut(),
            }
        }

        fn retirement(&self) -> Option<RetiredOracleOwner> {
            match self {
                OracleHandle::Native(oracle) => {
                    let retirement = oracle.0.retirement();
                    (!retirement.snapshot().safe_to_shutdown)
                        .then_some(RetiredOracleOwner::Native(retirement))
                }
                #[cfg(feature = "test-fake")]
                OracleHandle::Scripted {
                    oracle,
                    release_file,
                } => oracle
                    .remote_call_pending()
                    .then(|| RetiredOracleOwner::Scripted {
                        release_file: release_file.clone(),
                    }),
            }
        }

        /// The shipped `with oracle:` exit: the native region is always
        /// released, and the retire decision is taken afterwards.
        fn close(&mut self) {
            match self {
                OracleHandle::Native(oracle) => oracle.close(),
                #[cfg(feature = "test-fake")]
                OracleHandle::Scripted { .. } => {}
            }
        }
    }

    /// The auxiliary generator the scan consults, over the product tables.
    struct HostAuxiliarySource<'a> {
        tables: nioh3_domain::preview::PreviewTables<'a>,
        cache: &'a mut std::collections::HashMap<u32, nioh3_domain::preview::AuxiliaryPreview>,
    }

    impl AuxiliarySource for HostAuxiliarySource<'_> {
        fn compose(
            &mut self,
            seed: u32,
            playthrough: u32,
        ) -> Result<nioh3_domain::preview::AuxiliaryPreview, HostError> {
            if let Some(cached) = self.cache.get(&seed) {
                return Ok(cached.clone());
            }
            let playthrough = u8::try_from(playthrough)
                .map_err(|_| HostError::rejected("playthrough must be between 1 and 5, or None"))?;
            let composed =
                nioh3_domain::preview::compose_auxiliary_preview(seed, playthrough, &self.tables)
                    .map_err(|error| {
                    HostError::rejected(format!(
                        "the ported offline preview cannot compose this candidate, so the \
                     search fails closed rather than dropping it: {error:?}"
                    ))
                })?;
            self.cache.insert(seed, composed.clone());
            Ok(composed)
        }
    }

    fn param_str(params: &Value, name: &str) -> Result<String, HostError> {
        params
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(HostError::invalid_request)
    }

    fn param_u64(params: &Value, name: &str) -> Result<u64, HostError> {
        params
            .get(name)
            .and_then(Value::as_u64)
            .ok_or_else(HostError::invalid_request)
    }

    fn key_set(value: &Value, name: &str) -> std::collections::BTreeSet<u32> {
        value
            .get(name)
            .and_then(Value::as_array)
            .map(|keys| {
                keys.iter()
                    .filter_map(Value::as_u64)
                    .map(|key| key as u32)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn key_groups(value: &Value, name: &str) -> Vec<std::collections::BTreeSet<u32>> {
        value
            .get(name)
            .and_then(Value::as_array)
            .map(|groups| {
                groups
                    .iter()
                    .filter_map(Value::as_array)
                    .map(|group| {
                        group
                            .iter()
                            .filter_map(Value::as_u64)
                            .map(|key| key as u32)
                            .collect()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn key_list(value: &Value, name: &str) -> Vec<u32> {
        key_set(value, name).into_iter().collect()
    }

    fn group_list(value: &Value, name: &str) -> Vec<Vec<u32>> {
        key_groups(value, name)
            .into_iter()
            .map(|group| group.into_iter().collect())
            .collect()
    }

    /// `models.AuxiliarySearchCriteria(**criteria['auxiliary'])`.
    fn auxiliary_criteria(criteria: &Value) -> AuxiliaryCriteria {
        let Some(auxiliary) = criteria.get("auxiliary") else {
            return AuxiliaryCriteria::default();
        };
        AuxiliaryCriteria {
            required_terrain_effect_keys: key_list(auxiliary, "required_terrain_effect_keys"),
            required_terrain_effect_key_groups: group_list(
                auxiliary,
                "required_terrain_effect_key_groups",
            ),
            required_special_rule_keys: key_list(auxiliary, "required_special_rule_keys"),
            required_special_rule_key_groups: group_list(
                auxiliary,
                "required_special_rule_key_groups",
            ),
            required_enemy_lookup_keys: key_list(auxiliary, "required_enemy_lookup_keys"),
            required_enemy_lookup_key_groups: group_list(
                auxiliary,
                "required_enemy_lookup_key_groups",
            ),
        }
    }

    fn decode_hex(text: &str) -> Result<Vec<u8>, HostError> {
        if !text.len().is_multiple_of(2) {
            return Err(HostError::invalid_request());
        }
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(text.len() / 2);
        let mut index = 0;
        while index < bytes.len() {
            let high = (bytes[index] as char)
                .to_digit(16)
                .ok_or_else(HostError::invalid_request)?;
            let low = (bytes[index + 1] as char)
                .to_digit(16)
                .ok_or_else(HostError::invalid_request)?;
            out.push((high * 16 + low) as u8);
            index += 2;
        }
        Ok(out)
    }

    /// The shipped fail-closed wording for a candidate the ported preview
    /// composition cannot represent.
    fn unsupported_preview(error: impl std::fmt::Debug) -> HostError {
        HostError::coded(
            "UNSUPPORTED_CONTEXT",
            format!(
                "the ported offline preview cannot compose this candidate, so the \
                 search fails closed rather than dropping it: {error:?}"
            ),
        )
    }

    impl RuntimeApplication {
        /// The game identity the native generation oracle uses (native search,
        /// known-seed generation, grace-map capture). Resolved for
        /// [`ProfilePurpose::NativeOracle`] so its scoped approval reaches only
        /// this path.
        ///
        /// [`ProfilePurpose::NativeOracle`]: nioh3_runtime::profile::ProfilePurpose::NativeOracle
        fn oracle_identity(&self) -> Result<GameIdentity, HostError> {
            nioh3_runtime::identify_running_game_for(
                &super::profile_dir_for(&self.data_root),
                nioh3_runtime::profile::ProfilePurpose::NativeOracle,
            )
            .map_err(HostError::from_runtime)
        }

        /// The game identity the temporary override hooks use.
        ///
        /// Resolved for [`ProfilePurpose::TemporaryOverride`], so a version
        /// approved only for these hooks is reachable here while every other
        /// native path keeps its own approval gate.
        ///
        /// [`ProfilePurpose::TemporaryOverride`]: nioh3_runtime::profile::ProfilePurpose::TemporaryOverride
        fn override_identity(&self) -> Result<GameIdentity, HostError> {
            nioh3_runtime::identify_running_game_for(
                &super::profile_dir_for(&self.data_root),
                nioh3_runtime::profile::ProfilePurpose::TemporaryOverride,
            )
            .map_err(HostError::from_runtime)
        }

        /// `runtime.inventory_snapshot`: one bounded read-only page of raw
        /// inventory slots.
        ///
        /// Read-only and non-mutating. It validates the request first, then
        /// detects the sole game and opens one `PROCESS_QUERY_INFORMATION |
        /// PROCESS_VM_READ` handle. Everything after that is bound to that
        /// handle: its image path is queried through it, the fixed-file version
        /// and the pinned SHA-256 are derived from that path, and only then are
        /// the four declared sites and the heap page read.
        ///
        /// It resolves no generation profile, so the generation and live-add
        /// site sets, their approvals and their identity digests are untouched.
        /// The client can choose only `start` and `limit`: the process, the
        /// module, the digest, the profile and the slot domain are fixed here.
        fn inventory_snapshot(&self, params: &Value) -> Result<Value, HostError> {
            use nioh3_runtime::inventory::{snapshot, InventoryRequest, ProcessInventoryMemory};

            // A malformed request must never open a handle.
            let request = InventoryRequest::from_json(params).map_err(HostError::from_runtime)?;
            let process = Self::open_supported_reader()?;
            let memory = ProcessInventoryMemory::new(&process);
            snapshot(&memory, &request).map_err(HostError::from_runtime)
        }

        /// Admit only the requested capability. Live addition keeps its own
        /// target/backup/receipt checks; other capabilities retain compatibility
        /// consent. Recovery, status, cancellation and stop never require it.
        pub(super) fn admit_runtime_feature(
            &mut self,
            feature: &str,
        ) -> Result<crate::compatibility::ExecutableIdentity, HostError> {
            let identity = crate::compatibility::running_identity()?;
            self.compatibility.require_feature(&identity, feature)?;
            Ok(identity)
        }

        pub(super) fn require_resolved_identity(
            approved: &crate::compatibility::ExecutableIdentity,
            resolved: &GameIdentity,
        ) -> Result<(), HostError> {
            let actual = crate::compatibility::ExecutableIdentity {
                pid: resolved.identity.pid,
                creation_filetime: resolved.identity.creation_filetime,
                path: resolved.executable.clone(),
                version: resolved.file_version.display(),
                sha256: nioh3_runtime::file_sha256(&resolved.executable)
                    .map_err(HostError::from_runtime)?,
            };
            super::require_same_runtime_identity(approved, &actual)
        }

        fn require_resource_version(
            &self,
            actual: nioh3_runtime::FileVersion,
        ) -> Result<(), HostError> {
            if self.resource_version != Some(actual.tuple()) {
                return Err(HostError::coded("COMPATIBILITY_RESOURCE_CONTEXT_CHANGED", format!(
                    "Studio resources target {:?}; the running game is {}. Select the running Nioh3.exe in Settings, reopen Studio, and prepare the operation again.",
                    self.resource_version, actual.display()
                )));
            }
            Ok(())
        }

        /// A read handle on the process instance the compatibility review saw.
        /// This also serves diagnostics, without approving a version or layout.
        fn open_identity_reader(
            identity: &crate::compatibility::ExecutableIdentity,
        ) -> Result<nioh3_runtime::ReadOnlyProcess, HostError> {
            let process = nioh3_runtime::ReadOnlyProcess::open(
                identity.pid,
                nioh3_runtime::GAME_MODULE_NAME,
                Some(identity.creation_filetime),
            )
            .map_err(HostError::from_runtime)?;
            if !process
                .image_path()
                .map_err(HostError::from_runtime)?
                .eq_ignore_ascii_case(&identity.path)
            {
                return Err(HostError::rejected(
                    "The running image changed since the compatibility review",
                ));
            }
            Ok(process)
        }

        /// Read-only structural evidence, never a native binding or write grant.
        fn compatibility_probe(identity: &crate::compatibility::ExecutableIdentity) -> Value {
            match Self::open_identity_reader(identity) {
                Ok(process) => {
                    let memory = nioh3_runtime::ProcessInventoryMemory::new(&process);
                    nioh3_runtime::compatibility_probe::probe_character_layouts(
                        &memory,
                        process.module_range(),
                    )
                }
                Err(error) => json!({
                    "status": "structure_only", "outcome": "unavailable",
                    "observed_pid": identity.pid,
                    "observed_creation_filetime": identity.creation_filetime.to_string(),
                    "candidates": [], "note": error.message,
                }),
            }
        }

        /// One read-only handle on a registered build. Whole-file equality is
        /// advisory; individual readers still enforce their own layout checks.
        fn open_supported_reader() -> Result<nioh3_runtime::ReadOnlyProcess, HostError> {
            let pid = nioh3_runtime::single_process_id(nioh3_runtime::GAME_IMAGE_NAME)
                .map_err(HostError::from_runtime)?;
            let creation = nioh3_runtime::process_creation_filetime(pid)
                .map_err(HostError::from_runtime)?
                .ok_or_else(|| {
                    HostError::from_runtime(nioh3_runtime::RuntimeError::ProcessGone { pid })
                })?;
            let process = nioh3_runtime::ReadOnlyProcess::open(
                pid,
                nioh3_runtime::GAME_MODULE_NAME,
                Some(creation),
            )
            .map_err(HostError::from_runtime)?;
            let executable = process.image_path().map_err(HostError::from_runtime)?;
            let status = nioh3_runtime::verify_game_executable(&executable);
            if !status.supported() {
                return Err(HostError::from_runtime(
                    nioh3_runtime::RuntimeError::GameExecutableUnsupported {
                        path: executable,
                        state: status.state.as_str(),
                    },
                ));
            }
            Ok(process)
        }

        fn open_supported_reader_for(
            identity: &crate::compatibility::ExecutableIdentity,
        ) -> Result<nioh3_runtime::ReadOnlyProcess, HostError> {
            if !identity.supported() {
                return Err(HostError::from_runtime(
                    nioh3_runtime::RuntimeError::UnsupportedGameVersion {
                        display: identity.version.clone(),
                    },
                ));
            }
            Self::open_identity_reader(identity)
        }

        /// `runtime.character_snapshot`: the loaded character's currencies and
        /// every owned equipment record, read twice and published only when
        /// both reads agree.
        fn character_snapshot(&mut self) -> Result<Value, HostError> {
            use nioh3_runtime::character::LiveItemContainer;
            use nioh3_runtime::inventory::ProcessInventoryMemory;
            use nioh3_save::character::ItemContainer;

            let executable = crate::compatibility::running_identity()?;
            let process = Self::open_supported_reader_for(&executable)?;
            let memory = ProcessInventoryMemory::new(&process);
            let version = nioh3_runtime::character::character_layout(&executable.version)
                .ok_or_else(HostError::invalid_request)?;
            let read = nioh3_runtime::character::read_character_with_layout(&memory, version)
                .map_err(HostError::from_runtime)?;
            let mut currencies = serde_json::Map::new();
            for (currency, value) in &read.currencies {
                currencies.insert(currency.label().to_string(), json!(value));
            }
            let mut equipment = Vec::new();
            for slot_index in 0..nioh3_runtime::character::EQUIPMENT_SLOTS {
                let Some(record) = read.record(slot_index) else {
                    continue;
                };
                if nioh3_save::character::equipment_slot_is_empty(record) {
                    continue;
                }
                let fields = nioh3_save::character::equipment_fields(record)
                    .map_err(HostError::from_save)?;
                let class = crate::item_kinds::type_class(&self.data_root, fields.item_id);
                let mut row = crate::save_app::equipment_json(slot_index, &fields, class);
                row["record_sha256"] = json!(format!("{:x}", Sha256::digest(record)));
                row["audit"] = crate::equipment_rules::audit_json(&self.data_root, record);
                row["hell"] = json!(record.get(0x1A).is_some_and(|flags| flags & 0x10 != 0));
                equipment.push(row);
            }
            let items = match &read.items {
                Ok(_) => {
                    let mut items = Vec::new();
                    for container in LiveItemContainer::ALL {
                        for slot_index in 0..container.slots() {
                            let Some(record) = read.item(container, slot_index) else {
                                continue;
                            };
                            if !nioh3_save::character::equipment_slot_is_empty(record) {
                                items.push(crate::save_app::item_json(
                                    match container {
                                        LiveItemContainer::Held => ItemContainer::Held,
                                        LiveItemContainer::Storage => ItemContainer::Storage,
                                    },
                                    slot_index,
                                    record,
                                ));
                            }
                        }
                    }
                    Value::Array(items)
                }
                Err(_) => Value::Null,
            };
            Ok(json!({
                "source": "runtime",
                "game_version": executable.version,
                "compatibility": self.compatibility.report(&executable),
                "process_id": read.pid,
                "currencies": currencies,
                "equipment_slots": nioh3_runtime::character::EQUIPMENT_SLOTS,
                "equipment": equipment,
                "items": items,
            }))
        }

        /// `runtime.menu_selection`: the item under the in-game inventory menu's
        /// cursor, read-only, so the editor can follow the player's selection.
        fn menu_selection(&self) -> Result<Value, HostError> {
            use nioh3_runtime::character::{read_menu_selection, MenuSelection};
            use nioh3_runtime::inventory::{InventoryMemory, ProcessInventoryMemory};

            let process = Self::open_supported_reader()?;
            let memory = ProcessInventoryMemory::new(&process);
            let selection = read_menu_selection(&memory).map_err(HostError::from_runtime)?;
            let process_id = memory.process().pid;
            Ok(match selection {
                MenuSelection::Closed => json!({ "process_id": process_id, "menu_open": false }),
                MenuSelection::Equipment {
                    slot_index,
                    item_id,
                } => json!({
                    "process_id": process_id,
                    "menu_open": true,
                    "container": "equipment",
                    "slot_index": slot_index,
                    "item_id": item_id,
                }),
                MenuSelection::Item {
                    container,
                    slot_index,
                    item_id,
                } => json!({
                    "process_id": process_id,
                    "menu_open": true,
                    "container": container.label(),
                    "slot_index": slot_index,
                    "item_id": item_id,
                }),
                MenuSelection::Other { item_id } => json!({
                    "process_id": process_id,
                    "menu_open": true,
                    "slot_index": null,
                    "item_id": item_id,
                }),
            })
        }

        /// `runtime.character_edit`: compare-and-swap writes of currencies and
        /// modded equipment fields into the running game, and removal of
        /// unworn equipment.
        ///
        /// Every target must still hold the value the caller reviewed; the
        /// write handle carries only VM write rights and each target is read
        /// back. The game saves the new values itself. A removal leaves the
        /// slot exactly as the game frees one and is refused while either
        /// equipment set wears the item.
        fn character_edit(&mut self, params: &Value) -> Result<Value, HostError> {
            let executable = self.admit_runtime_feature("live_character")?;
            use nioh3_runtime::character::{
                LiveCurrency, LiveEdit, LiveEditOutcome, LiveItemContainer,
            };
            use nioh3_runtime::inventory::ProcessInventoryMemory;

            if !self.safe_to_shutdown()? {
                return Err(HostError::rejected(
                    "Finish the live scroll addition before editing the character",
                ));
            }
            let expected_pid = params
                .get("process_id")
                .and_then(Value::as_u64)
                .and_then(|pid| u32::try_from(pid).ok())
                .ok_or_else(HostError::invalid_request)?;
            let process = Self::open_supported_reader_for(&executable)?;
            let memory = ProcessInventoryMemory::new(&process);
            let version = nioh3_runtime::character::character_layout(&executable.version)
                .ok_or_else(HostError::invalid_request)?;
            let read = nioh3_runtime::character::read_character_with_layout(&memory, version)
                .map_err(HostError::from_runtime)?;
            let mut edits = Vec::new();
            let mut currency_changes = Vec::new();
            let reviewed = params.get("expected_currencies").and_then(Value::as_object);
            if let Some(requested) = params.get("currencies").and_then(Value::as_object) {
                for (label, value) in requested {
                    let currency =
                        LiveCurrency::from_label(label).ok_or_else(HostError::invalid_request)?;
                    let replacement = value.as_u64().ok_or_else(HostError::invalid_request)?;
                    let expected = reviewed
                        .and_then(|values| values.get(label))
                        .and_then(Value::as_u64)
                        .ok_or_else(HostError::invalid_request)?;
                    if expected == replacement {
                        continue;
                    }
                    currency_changes.push(json!({
                        "currency": label,
                        "before": expected,
                        "after": replacement,
                    }));
                    edits.push(LiveEdit::Currency {
                        currency,
                        expected,
                        replacement,
                    });
                }
            }
            let mut equipment_changes = Vec::new();
            for requested in params
                .get("equipment")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let slot_index = requested
                    .get("slot_index")
                    .and_then(Value::as_u64)
                    .ok_or_else(HostError::invalid_request)?
                    as usize;
                let reviewed_sha = requested
                    .get("expected_record_sha256")
                    .and_then(Value::as_str)
                    .ok_or_else(HostError::invalid_request)?;
                let mut patch: nioh3_save::character::EquipmentPatch = serde_json::from_value(
                    requested
                        .get("patch")
                        .cloned()
                        .ok_or_else(HostError::invalid_request)?,
                )
                .map_err(|_| HostError::invalid_request())?;
                crate::equipment_rules::fill_effect_markers(&self.data_root, &mut patch);
                let original = read
                    .record(slot_index)
                    .ok_or_else(HostError::invalid_request)?
                    .to_vec();
                if !format!("{:x}", Sha256::digest(&original)).eq_ignore_ascii_case(reviewed_sha) {
                    return Err(HostError::rejected(
                        "The equipment changed in game since it was read; reload and try again",
                    ));
                }
                let replacement = nioh3_save::character::patch_equipment(&original, &patch)
                    .map_err(HostError::from_save)?;
                if replacement == original {
                    continue;
                }
                let before = nioh3_save::character::equipment_fields(&original)
                    .map_err(HostError::from_save)?;
                let after = nioh3_save::character::equipment_fields(&replacement)
                    .map_err(HostError::from_save)?;
                equipment_changes.push(json!({
                    "slot_index": slot_index,
                    "before": crate::save_app::equipment_json(slot_index, &before, None),
                    "after": crate::save_app::equipment_json(slot_index, &after, None),
                }));
                edits.push(LiveEdit::Equipment {
                    slot_index,
                    expected: original,
                    replacement,
                });
            }
            let mut removed = Vec::new();
            let removals = params
                .get("remove")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            for requested in removals {
                let slot_index = requested
                    .get("slot_index")
                    .and_then(Value::as_u64)
                    .ok_or_else(HostError::invalid_request)?
                    as usize;
                let reviewed_sha = requested
                    .get("expected_record_sha256")
                    .and_then(Value::as_str)
                    .ok_or_else(HostError::invalid_request)?;
                if edits.iter().any(|edit| {
                    matches!(edit, LiveEdit::Equipment { slot_index: other, .. } if *other == slot_index)
                }) {
                    return Err(HostError::invalid_request());
                }
                let original = read
                    .record(slot_index)
                    .ok_or_else(HostError::invalid_request)?
                    .to_vec();
                if !format!("{:x}", Sha256::digest(&original)).eq_ignore_ascii_case(reviewed_sha) {
                    return Err(HostError::rejected(
                        "The equipment changed in game since it was read; reload and try again",
                    ));
                }
                if nioh3_save::character::equipment_is_worn(&original) {
                    return Err(HostError::rejected(
                        "Unequip the item in game before removing it",
                    ));
                }
                let replacement = nioh3_save::character::free_equipment_slot(&original)
                    .map_err(HostError::from_save)?;
                let before = nioh3_save::character::equipment_fields(&original)
                    .map_err(HostError::from_save)?;
                removed.push(json!({
                    "slot_index": slot_index,
                    "before": crate::save_app::equipment_json(slot_index, &before, None),
                }));
                edits.push(LiveEdit::Equipment {
                    slot_index,
                    expected: original,
                    replacement,
                });
            }
            let mut item_changes = Vec::new();
            for requested in params
                .get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let container = requested
                    .get("container")
                    .and_then(Value::as_str)
                    .and_then(LiveItemContainer::from_label)
                    .ok_or_else(HostError::invalid_request)?;
                let slot_index = requested
                    .get("slot_index")
                    .and_then(Value::as_u64)
                    .ok_or_else(HostError::invalid_request)?
                    as usize;
                let reviewed_sha = requested
                    .get("expected_record_sha256")
                    .and_then(Value::as_str)
                    .ok_or_else(HostError::invalid_request)?;
                let quantity = requested
                    .get("quantity")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(HostError::invalid_request)?;
                let original = match &read.items {
                    Ok(_) => read
                        .item(container, slot_index)
                        .ok_or_else(HostError::invalid_request)?
                        .to_vec(),
                    Err(reason) => return Err(HostError::rejected(reason.clone())),
                };
                if !format!("{:x}", Sha256::digest(&original)).eq_ignore_ascii_case(reviewed_sha) {
                    return Err(HostError::rejected(
                        "The item changed in game since it was read; reload and try again",
                    ));
                }
                let replacement = nioh3_save::character::patch_item_quantity(&original, quantity)
                    .map_err(HostError::from_save)?;
                if replacement == original {
                    continue;
                }
                item_changes.push(json!({
                    "container": container.label(),
                    "slot_index": slot_index,
                    "item_id": u16::from_le_bytes([original[0], original[1]]),
                    "before": nioh3_save::character::item_quantity(&original),
                    "after": quantity,
                }));
                edits.push(LiveEdit::Item {
                    container,
                    slot_index,
                    expected: original,
                    replacement,
                });
            }
            if edits.is_empty() {
                return Err(HostError::rejected("Nothing to change"));
            }
            let mut writer =
                nioh3_runtime::mutation::memory::WindowsProcess::open_field_write(expected_pid)
                    .map_err(HostError::from_runtime)?;
            let outcome = nioh3_runtime::character::apply_live_edits_with_layout(
                &memory,
                &mut writer,
                expected_pid,
                &edits,
                version,
            );
            nioh3_runtime::mutation::memory::TargetProcess::close(&mut writer);
            let (state, error) = match outcome {
                LiveEditOutcome::Verified => ("verified", Value::Null),
                LiveEditOutcome::Rejected(message) => ("rejected", json!(message)),
                LiveEditOutcome::Uncertain(message) => ("uncertain", json!(message)),
            };
            Ok(json!({"character_edit": {
                "state": state,
                "process_id": expected_pid,
                "currencies": currency_changes,
                "equipment": equipment_changes,
                "removed": removed,
                "items": item_changes,
                "error": error,
            }}))
        }

        /// `_enemy_role_by_lookup_key` over the shipped roster.
        fn roster_roles(&mut self) -> Result<&BTreeMap<u32, u8>, HostError> {
            if self.roster_roles.is_none() {
                let resources = nioh3_data::load_enemy_resources(&self.data_root)
                    .map_err(|error| HostError::coded("RESOURCE_MISMATCH", error.to_string()))?;
                let roles = nioh3_domain::roster::enemy_role_by_lookup_key(&resources.roster)
                    .map_err(|error| HostError::rejected(error.to_string()))?;
                self.roster_roles = Some(roles);
            }
            self.roster_roles
                .as_ref()
                .ok_or_else(|| HostError::rejected("roster unavailable"))
        }

        /// `RuntimeApplication.status`: publish the live-addition ownership
        /// before taking the snapshot, then the override session and the
        /// retired native calls.
        pub(super) fn status(&mut self) -> Result<Value, HostError> {
            self.retired_oracles.retain(|owner| !owner.refresh());
            let retired_count = self.retired_oracles.len() as u64;
            let retired_error = self
                .retired_oracles
                .iter()
                .find_map(RetiredOracleOwner::error);
            let ownership = match self.live_add.as_mut() {
                Some(application) => {
                    Some(application.ownership().map_err(HostError::from_runtime)?)
                }
                None => None,
            };
            let status = match ownership.as_ref() {
                Some(ownership) => self.host.status_with_live_add(ownership),
                None => self.host.status(),
            };
            let mut value = status.to_json();
            if let Some(application) = self.equipment_add.as_mut() {
                if !application.safe_to_shutdown() {
                    value["safe_to_shutdown"] = Value::Bool(false);
                    value["pending_remote_calls"] =
                        json!(value["pending_remote_calls"].as_u64().unwrap_or(0) + 1);
                    value["error"] = json!(format!(
                        "Equipment native owner retained; recover operation {}",
                        application.pending_preview().unwrap_or("the last addition")
                    ));
                }
            }
            if retired_count > 0 {
                let base_pending = value
                    .get("pending_remote_calls")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                value["pending_remote_calls"] = json!(base_pending + retired_count);
                value["safe_to_shutdown"] = Value::Bool(false);
                if value.get("error").is_none_or(Value::is_null) {
                    value["error"] = json!(retired_error);
                }
            }
            Ok(value)
        }

        /// `RuntimeApplication.stop_override`. A failed restoration retains
        /// ownership, so the error propagates instead of being swallowed.
        pub(super) fn stop_override(&mut self) -> Result<Value, HostError> {
            let status = self.host.stop_override().map_err(HostError::from_runtime)?;
            Ok(status.to_json())
        }

        pub(super) fn safe_to_shutdown(&mut self) -> Result<bool, HostError> {
            Ok(self.status()?.get("safe_to_shutdown") == Some(&Value::Bool(true)))
        }

        /// `RuntimeApplication.start_override`.
        pub(super) fn start_override(&mut self, profile: &Value) -> Result<Value, HostError> {
            let approved = self.admit_runtime_feature("temporary_override")?;
            if profile
                .get("challenge_capacity")
                .is_some_and(|value| !value.is_null())
            {
                self.compatibility
                    .require_feature(&approved, "challenge_capacity_override")?;
            }
            if !self.safe_to_shutdown()? {
                return Err(HostError::rejected(
                    "Stop the existing override and wait for pending native calls",
                ));
            }
            let resolved = self.override_identity()?;
            Self::require_resolved_identity(&approved, &resolved)?;
            self.require_resource_version(resolved.file_version)?;
            let GameIdentity {
                identity,
                profile: runtime_profile,
                ..
            } = resolved;
            let sessions = self
                .build_sessions(profile, identity.pid, runtime_profile)?
                .into_iter()
                .map(|session| session.with_expected_creation(approved.creation_filetime))
                .collect();
            let status = self
                .host
                .start_override(sessions)
                .map_err(HostError::from_runtime)?;
            Ok(status.to_json())
        }

        /// The shipped session list for one `runtime.start_override` profile.
        ///
        /// The auxiliary session is created when the profile changes any
        /// descriptor field and the challenge session when a capacity is
        /// requested; a profile that changes nothing is refused before any
        /// session is built.
        fn build_sessions(
            &mut self,
            profile: &Value,
            pid: u32,
            runtime_profile: NativeRuntimeProfile,
        ) -> Result<Vec<OverrideSession<WindowsSessionMemory>>, HostError> {
            let seed = profile
                .get("seed")
                .and_then(Value::as_u64)
                .ok_or_else(HostError::invalid_request)? as u32;
            let enemy_keys: Vec<u32> = profile
                .get("enemy_keys")
                .and_then(Value::as_array)
                .map(|keys| {
                    keys.iter()
                        .filter_map(Value::as_u64)
                        .map(|key| key as u32)
                        .collect()
                })
                .unwrap_or_default();
            let special_rule_keys = match profile.get("special_rule_keys") {
                None | Some(Value::Null) => None,
                Some(value) => {
                    let keys = value.as_array().ok_or_else(HostError::invalid_request)?;
                    if keys.len() != 3 {
                        return Err(HostError::rejected(
                            "special_rule_keys must contain exactly three keys",
                        ));
                    }
                    let mut resolved = [0u16; 3];
                    for (index, key) in keys.iter().enumerate() {
                        let value = key.as_u64().ok_or_else(HostError::invalid_request)?;
                        resolved[index] = u16::try_from(value).map_err(|_| {
                            HostError::rejected("special_rule_keys must fit in uint16")
                        })?;
                    }
                    Some(resolved)
                }
            };
            let terrain_value = match profile.get("terrain_value") {
                None | Some(Value::Null) => None,
                Some(value) => Some(
                    u8::try_from(value.as_u64().ok_or_else(HostError::invalid_request)?)
                        .map_err(|_| HostError::rejected("terrain_value must fit in uint8"))?,
                ),
            };
            let challenge_capacity = match profile.get("challenge_capacity") {
                None | Some(Value::Null) => None,
                Some(value) => Some(
                    u8::try_from(value.as_u64().ok_or_else(HostError::invalid_request)?).map_err(
                        |_| {
                            HostError::rejected("Expected a uint32 seed and a capacity from 1 to 7")
                        },
                    )?,
                ),
            };

            let mut sessions = Vec::new();
            if !enemy_keys.is_empty() || special_rule_keys.is_some() || terrain_value.is_some() {
                let roles = self.roster_roles()?;
                let mut groups = Vec::with_capacity(enemy_keys.len());
                for key in &enemy_keys {
                    let role = roles.get(key).copied().ok_or_else(|| {
                        HostError::rejected(format!(
                            "unknown enemy lookup key {key} for a temporary override"
                        ))
                    })?;
                    groups.push(EnemyGroup {
                        lookup_key: *key,
                        role: u32::from(role),
                    });
                }
                sessions.push(
                    OverrideSession::auxiliary(
                        OverrideProfile {
                            seed,
                            enemy_groups: groups,
                            special_rule_keys,
                            terrain_value,
                        },
                        pid,
                        runtime_profile.clone(),
                        WindowsSessionMemory,
                    )
                    .map_err(HostError::from_runtime)?,
                );
            }
            if let Some(capacity) = challenge_capacity {
                sessions.push(
                    OverrideSession::challenge(
                        ChallengeOverrideProfile { seed, capacity },
                        pid,
                        runtime_profile,
                        WindowsSessionMemory,
                    )
                    .map_err(HostError::from_runtime)?,
                );
            }
            if sessions.is_empty() {
                return Err(HostError::rejected("Select at least one temporary field"));
            }
            Ok(sessions)
        }

        /// The live-add binding the host selects for one resolved executable
        /// version.
        ///
        /// Selection only. It returns the identifiers the executor already
        /// accepts, and the executor still proves the advertised profile id and,
        /// where the binding pins one, the exact executable digest before any
        /// read or dispatch. Split out so the selection is unit-testable with no
        /// running game and no native call.
        pub(crate) fn live_add_binding_for_version(
            file_version: nioh3_runtime::FileVersion,
        ) -> Result<
            (
                &'static nioh3_runtime::mutation::native_abi::LiveAddLayout,
                &'static str,
            ),
            HostError,
        > {
            live_add_binding_for_game_version(file_version.tuple()).ok_or_else(|| {
                HostError::rejected("Live addition is not accepted for this game version")
            })
        }

        /// Build the insertion adapter from the actual process and its existing
        /// operation-specific binding. The executor checks the target's native
        /// code and containers, without loading a generation research profile.
        /// Cached native owners and durable operations retain their original
        /// process lifetime; only an idle adapter may be rebuilt after restart.
        fn live_add_application(&mut self) -> Result<&mut LiveAddApplication, HostError> {
            // Receipt recovery needs no generation resources. A context loaded
            // later is used only by future preparations; never discard owners.
            let candidate_context = self
                .context
                .as_ref()
                .map(|context| context.digest().to_string());
            if self.live_add_context != candidate_context
                && self
                    .live_add
                    .as_mut()
                    .is_some_and(|app| app.safe_to_shutdown())
            {
                self.live_add = None;
            }
            // The cached executor is bound to one game process lifetime. When the
            // game has since exited or restarted and the executor owns no native
            // state, rebuild it for the running game instead of querying a dead
            // process forever. An executor that still owns native state keeps its
            // lifetime, and durable operations stay bound to the process they
            // were planned against.
            if self.live_add.is_some() {
                if let Ok(current) = crate::compatibility::running_identity() {
                    let running = (current.pid, current.creation_filetime);
                    if self.live_add_process != Some(running)
                        && self
                            .live_add
                            .as_mut()
                            .is_some_and(|application| application.safe_to_shutdown())
                    {
                        self.live_add = None;
                    }
                }
            }
            if self.live_add.is_none() {
                let identity = crate::compatibility::running_identity()?;
                crate::compatibility::validate_feature(&identity, "live_scroll_add")?;
                let process = Self::open_identity_reader(&identity)?;
                let version =
                    nioh3_runtime::file_version(&identity.path).map_err(HostError::from_runtime)?;
                let (layout, display_version) = Self::live_add_binding_for_version(version)?;
                // Prove the module belongs to this process before constructing the transport.
                let _module = process.module_range();
                let directory = super::native_executor_directory(&self.state_root);
                let transport = NativeDebugTransport::new(
                    identity.pid,
                    *layout,
                    nioh3_runtime::GAME_MODULE_NAME,
                    &directory,
                )
                .map_err(HostError::from_runtime)?
                .with_expected_creation(identity.creation_filetime);
                let executor = NativeLiveAddExecutor::new(transport, *layout, display_version)
                    .with_compatible_executable();
                let application = LiveAddApplication::new(
                    &self.state_root,
                    candidate_context.as_deref().unwrap_or(""),
                    Box::new(executor),
                    Box::new(crate::runtime_backup::SaveBackupAdapter::new(
                        &self.state_root,
                    )),
                    Box::new(DomainCatalogPolicy::new()),
                )
                .map_err(HostError::from_runtime)?;
                self.live_add = Some(application);
                self.live_add_context = candidate_context;
                self.live_add_process = Some((identity.pid, identity.creation_filetime));
            }
            self.live_add
                .as_mut()
                .ok_or_else(|| HostError::rejected("live-add application unavailable"))
        }

        fn live_add_application_for(
            &mut self,
            approved: &crate::compatibility::ExecutableIdentity,
        ) -> Result<&mut LiveAddApplication, HostError> {
            let actual = crate::compatibility::running_identity()?;
            super::require_same_runtime_identity(approved, &actual)?;
            self.live_add_application()?;
            if self.live_add_process != Some((approved.pid, approved.creation_filetime)) {
                return Err(HostError::coded("COMPATIBILITY_IDENTITY_CHANGED", format!(
                    "Expected PID {} creation {}; the live-add adapter is bound to {:?}. Reconnect and prepare a new plan; recover any uncertain receipt before retrying.",
                    approved.pid, approved.creation_filetime, self.live_add_process
                )));
            }
            self.live_add
                .as_mut()
                .ok_or_else(|| HostError::rejected("live-add application unavailable"))
        }

        fn equipment_application(
            &mut self,
            operation: Option<&str>,
            approved: Option<&crate::compatibility::ExecutableIdentity>,
        ) -> Result<&mut nioh3_runtime::mutation::equipment_add::EquipmentAddition, HostError>
        {
            use nioh3_runtime::mutation::equipment_add::EquipmentAddition;
            if let Some(expected) = approved {
                let actual = crate::compatibility::running_identity()?;
                super::require_same_runtime_identity(expected, &actual)?;
            }
            let pid = if let Some(id) = operation {
                EquipmentAddition::operation_pid(&self.state_root, id)
                    .map_err(HostError::from_runtime)?
            } else if let Some(expected) = approved {
                Self::open_supported_reader_for(expected)?.identity().pid
            } else {
                Self::open_supported_reader()?.identity().pid
            };
            if approved.is_some_and(|expected| expected.pid != pid) {
                return Err(HostError::coded("COMPATIBILITY_IDENTITY_CHANGED", format!(
                    "The reviewed game PID is {}; this equipment plan belongs to PID {pid}. Reconnect and prepare a new plan; recover any uncertain receipt before retrying.",
                    approved.map(|expected| expected.pid).unwrap_or_default()
                )));
            }
            let approved_process =
                approved.map(|identity| (identity.pid, identity.creation_filetime));
            if self
                .equipment_add
                .as_ref()
                .is_none_or(|app| app.pid() != pid)
                || (approved.is_some() && self.equipment_add_process != approved_process)
            {
                if self
                    .equipment_add
                    .as_mut()
                    .is_some_and(|app| !app.safe_to_shutdown())
                {
                    return Err(HostError::rejected(
                        "Equipment native owner is still retained",
                    ));
                }
                let application = EquipmentAddition::new(pid, &self.state_root)
                    .map_err(HostError::from_runtime)?
                    .with_compatible_executable();
                self.equipment_add = Some(match approved {
                    Some(identity) => {
                        application.with_expected_creation(identity.creation_filetime)
                    }
                    None => application,
                });
                self.equipment_add_process = approved_process;
            }
            self.equipment_add
                .as_mut()
                .ok_or_else(|| HostError::rejected("Equipment addition unavailable"))
        }

        fn equipment_add_result(&self, mut state: Value) -> Result<Value, HostError> {
            let raw = state
                .get("preview_record_hex")
                .and_then(Value::as_str)
                .map(|hex| {
                    hex.as_bytes()
                        .chunks_exact(2)
                        .map(|pair| {
                            std::str::from_utf8(pair)
                                .ok()
                                .and_then(|v| u8::from_str_radix(v, 16).ok())
                                .ok_or_else(HostError::invalid_request)
                        })
                        .collect::<Result<Vec<u8>, _>>()
                })
                .transpose()?;
            state["preview"] = if let Some(record) = raw {
                let fields = nioh3_save::character::equipment_fields(&record)
                    .map_err(HostError::from_save)?;
                crate::save_app::equipment_json(
                    state["slot_index"].as_u64().unwrap_or(0) as usize,
                    &fields,
                    None,
                )
            } else {
                Value::Null
            };
            if let Some(object) = state.as_object_mut() {
                object.remove("preview_record_hex");
            }
            Ok(json!({"equipment_add":state}))
        }

        /// `{'live_add': snapshot}`.
        fn live_add_result(result: Value) -> Value {
            json!({"live_add": result})
        }

        fn live_add_snapshot(
            &mut self,
            operation: &str,
            params: &Value,
        ) -> Result<Value, HostError> {
            let operation_id = param_str(params, "operation_id")?;
            let snapshot = if operation == "live_add_recover" {
                self.live_add_application()?.recover(&operation_id)
            } else {
                // These application methods only read or claim the durable journal.
                // Keep cached native ownership untouched and work after a worker restart.
                let operations = nioh3_runtime::mutation::operations::LiveAddOperations::new(
                    &self.state_root.join("live-add"),
                )
                .map_err(HostError::from_runtime)?;
                if operation == "live_add_status" {
                    operations.snapshot(&operation_id)
                } else {
                    operations.cancel(&operation_id)
                }
            }
            .map_err(HostError::from_runtime)?;
            Ok(Self::live_add_result(snapshot.to_json()))
        }

        /// `RuntimeApplication.live_batch_status`: the reduced UI envelope.
        fn live_batch_status(&mut self, batch_id: &str) -> Result<Value, HostError> {
            let operations = nioh3_runtime::mutation::operations::LiveAddOperations::new(
                &self.state_root.join("live-add"),
            )
            .map_err(HostError::from_runtime)?;
            let value = LiveAddBatch::status_from_operations(&operations, batch_id)
                .map_err(HostError::from_runtime)?;
            let children = value
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let verified = children
                .iter()
                .filter(|child| child["state"].as_str() == Some("verified"))
                .count();
            Ok(json!({
                "live_batch": {
                    "batch_id": batch_id,
                    "plan_digest": value.get("plan_digest").cloned().unwrap_or(Value::Null),
                    "state": value.get("state").cloned().unwrap_or(Value::Null),
                    "count": value.get("requested_count").cloned().unwrap_or(Value::Null),
                    "verified_count": verified,
                    "child_operation_ids": children
                        .iter()
                        .map(|child| child["operation_id"].clone())
                        .collect::<Vec<_>>(),
                }
            }))
        }

        /// The shared `runtime.search`/`generate`/`capture_grace` pre-flight.
        ///
        /// Everything the shipped host does before the native seed scan is
        /// ported: the template must belong to this core, the runtime host must
        /// be idle, and the game identity must resolve. With no game the last
        /// step is the shipped process-absence failure.
        fn preflight(&mut self, template: &Value, method: &str) -> Result<(), HostError> {
            let digest = template
                .get("context_digest")
                .and_then(Value::as_str)
                .ok_or_else(HostError::invalid_request)?;
            if digest != self.generation_context()?.digest() {
                return Err(HostError::rejected(
                    "Template context differs from the running core",
                ));
            }
            if !self.safe_to_shutdown()? {
                let subject = if method == "capture_grace" {
                    "Map capture"
                } else {
                    "Native generation"
                };
                return Err(HostError::rejected(format!(
                    "{subject} requires an idle runtime host"
                )));
            }
            Ok(())
        }

        /// The product preview tables, loaded once on first use.
        fn preview_resources(&mut self) -> Result<&nioh3_data::PreviewResources, HostError> {
            if self.preview.is_none() {
                let loaded = match self.resource_version {
                    Some(version) => nioh3_data::load_preview_resources_for_file_version(
                        &self.data_root,
                        version,
                    ),
                    None => nioh3_data::load_preview_resources(&self.data_root),
                }
                .map_err(|error| HostError::coded("RESOURCE_MISMATCH", error.to_string()))?;
                self.preview = Some(Box::new(loaded));
            }
            self.preview
                .as_deref()
                .ok_or_else(|| HostError::rejected("preview tables unavailable"))
        }

        /// Take the oracle for one scan.
        ///
        /// The product path verifies the game identity and opens the native
        /// oracle; the off-by-default `test-fake` build may instead answer from
        /// a scripted table so the loops are exercised with fixed rows.
        fn open_oracle(&mut self) -> Result<OracleHandle, HostError> {
            #[cfg(feature = "test-fake")]
            if let Some(path) =
                std::env::var_os("NIOH3_PROTECTED_ORACLE_SCRIPT").filter(|value| !value.is_empty())
            {
                let script_path = std::path::Path::new(&path);
                let scripted = crate::oracle::scripted::load_script(script_path)
                    .map_err(HostError::rejected)?;
                let release_file = std::fs::read_to_string(script_path)
                    .ok()
                    .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                    .and_then(|payload| {
                        payload
                            .get("retirement_release_file")
                            .and_then(Value::as_str)
                            .map(PathBuf::from)
                    });
                return Ok(OracleHandle::Scripted {
                    oracle: Box::new(scripted),
                    release_file,
                });
            }
            // The scripted branch above never allocates in another process.
            // Every real generate/search/grace capture enters through here.
            let consent_identity = self.admit_runtime_feature("native_generation")?;
            let identity = self.oracle_identity()?;
            Self::require_resolved_identity(&consent_identity, &identity)?;
            self.require_resource_version(identity.file_version)?;
            let mut oracle = NativeOracle::new(
                identity.identity.pid,
                identity.module.base,
                identity.profile.clone(),
            )
            .map_err(HostError::from_runtime)?;
            let session =
                RemoteSession::open(identity.identity.pid).map_err(HostError::from_runtime)?;
            oracle
                .open_bound(Box::new(session), identity.identity, identity.module)
                .map_err(HostError::from_runtime)?;
            Ok(OracleHandle::Native(Box::new(oracle)))
        }

        /// The scan filters one request implies.
        fn build_request(
            &self,
            operation: &str,
            params: &Value,
            template: &Value,
        ) -> Result<ScanRequest, HostError> {
            let template_hex = template
                .get("template_hex")
                .and_then(Value::as_str)
                .ok_or_else(HostError::invalid_request)?;
            let template_bytes = decode_hex(template_hex)?;
            let rarity = param_u64(params, "rarity")? as u8;
            let level = param_u64(params, "level")? as u16;
            let recommended_level = param_u64(params, "recommended_level")? as u16;
            let playthrough = match operation {
                "capture_grace" => Some(param_u64(params, "playthrough")? as u32),
                _ => Some(param_u64(params, "playthrough")? as u32),
            };
            let mut filters = ScanFilters::new(rarity, playthrough);
            let mut start_seed = params.get("seed").and_then(Value::as_u64).unwrap_or(0) as u32;
            let mut max_seeds = 1u64;
            if operation == "search" {
                max_seeds = param_u64(params, "max_seeds")?;
                if let Some(criteria) = params.get("criteria") {
                    filters.primary_effect_ids = key_set(criteria, "primary_effect_ids");
                    filters.required_secondary_ids = key_set(criteria, "required_secondary_ids");
                    filters.required_secondary_id_groups =
                        key_groups(criteria, "required_secondary_id_groups");
                    filters.grace_effect_id = criteria
                        .get("grace_effect_id")
                        .and_then(Value::as_u64)
                        .map(|value| value as u32);
                    if filters.grace_effect_id.is_some() {
                        // Rarity 4 hides Grace in stage-one slot 5; rarity 5 and
                        // the accelerated paths read the final slot 6.
                        filters.grace_effect_slot = if rarity == 4 { 5 } else { 6 };
                    }
                    filters.auxiliary = auxiliary_criteria(criteria);
                }
            }
            if operation == "capture_grace" {
                // The capture itself takes no filters; the caller's result is
                // the map, not a candidate.
                filters.grace_effect_id = None;
                max_seeds = 0;
                start_seed = 0;
            }
            Ok(ScanRequest {
                template: template_bytes,
                start_seed,
                seed_step: 1,
                max_seeds,
                level,
                recommended_level,
                transfer_count: 0,
                filters,
                acceleration: crate::scan::ScanAcceleration::default(),
            })
        }

        /// `worker_contracts.candidate_payload` for one accepted native record.
        fn compose_payload(&mut self, matched: &ScanMatch, level: u16) -> Result<Value, HostError> {
            let seed = matched.seed;
            let playthrough = matched.playthrough.unwrap_or(3) as u8;
            let resources = self.preview_resources()?;
            let tables = nioh3_domain::preview::PreviewTables {
                roster: &resources.roster,
                context: &resources.context,
                rules: &resources.rules,
                states: &resources.states,
            };
            let auxiliary =
                nioh3_domain::preview::compose_auxiliary_preview(seed, playthrough, &tables)
                    .map_err(unsupported_preview)?;
            // The shipped payload publishes the enemy-state half for the NG3
            // playthrough only, and never generates it otherwise.
            let enemy_states = if playthrough == 3 {
                Some([
                    nioh3_domain::preview::compose_enemy_state_preview(
                        seed,
                        playthrough,
                        nioh3_domain::enemy::MissionVariant::Solo,
                        &tables,
                    )
                    .map_err(unsupported_preview)?,
                    nioh3_domain::preview::compose_enemy_state_preview(
                        seed,
                        playthrough,
                        nioh3_domain::enemy::MissionVariant::Expedition,
                        &tables,
                    )
                    .map_err(unsupported_preview)?,
                ])
            } else {
                None
            };
            let composition = nioh3_worker::engine::ComposedPreview {
                auxiliary,
                enemy_states,
                initial_challenge_capacity:
                    nioh3_domain::sequence::generate_challenge_attempt_count(seed),
            };
            let candidate = nioh3_worker::model::Candidate {
                seed,
                playthrough: matched.playthrough.map(|value| value as u8),
                rarity: matched.rarity,
                record_stage: matched.record_stage,
                record: matched.record.clone(),
                installation_record: matched.installation_record.clone(),
                effects: crate::scan::record_effect_entries(&matched.record)?,
                joint_search_trial: matched.joint_search_trial,
            };
            let mut payload = nioh3_worker::payload::candidate_payload_json(
                &candidate,
                self.generation_context()?.digest(),
                &composition,
            );
            // `worker_contracts.candidate_payload(candidate, service, evidence=...)`
            // takes the label as an argument: the offline worker publishes its
            // certified-replay label, while the protected runtime path publishes
            // the native generation label. The shared builder emits the former,
            // so this path states its own.
            if let Some(object) = payload.as_object_mut() {
                object.insert(
                    "evidence".to_string(),
                    Value::String("native_finalized_generation".to_string()),
                );
            }
            // The shipped search also retains the broker-only transfer block so
            // `runtime.export` can hand the same candidate to the save side.
            let wire = nioh3_worker::payload::transfer_json(
                &candidate,
                self.generation_context()?.digest(),
                level,
            );
            if let Some(candidate_id) = wire.get("candidate_id").and_then(Value::as_str) {
                self.candidates
                    .insert(candidate_id.to_string(), wire.clone());
            }
            Ok(payload)
        }

        /// `RuntimeApplication.generate` / `.search`.
        fn run_search(
            &mut self,
            operation: &str,
            params: &Value,
            ctx: &JobContext,
        ) -> Result<Value, HostError> {
            let template = params
                .get("template")
                .cloned()
                .ok_or_else(HostError::invalid_request)?;
            self.preflight(&template, operation)?;
            let mut request = self.build_request(operation, params, &template)?;
            let needs_auxiliary = !request.filters.auxiliary.is_empty();
            let mut handle = self.open_oracle()?;
            let mut last: Option<ScanProgress> = None;
            let mut cancel = || ctx.cancelled();
            // The shipped host prepares measured maps only for a bounded search
            // (`max_seeds > 1` with criteria), and only then turns acceleration
            // on; `runtime.generate` never measures anything.
            let mut phases: Vec<Value> = Vec::new();
            if operation == "search" && request.max_seeds > 1 {
                let fingerprint = template
                    .get("save_fingerprint")
                    .and_then(Value::as_str)
                    .ok_or_else(HostError::invalid_request)?
                    .to_string();
                let digest = self.generation_context()?.digest().to_string();
                let maps = crate::maps::prepare_maps(
                    handle.batch(),
                    &self.state_root,
                    &request.template,
                    &fingerprint,
                    &digest,
                    request.filters.playthrough.unwrap_or(0) as u8,
                    request.filters.rarity,
                    request.level,
                    request.recommended_level,
                    request.transfer_count,
                    request.filters.grace_effect_id,
                    &request.filters.primary_effect_ids,
                    &mut cancel,
                    &mut |_phase, value| {
                        ctx.progress(value.clone());
                        phases.push(value);
                    },
                )?;
                let after_trial = params
                    .get("after_trial")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let uses_primary = maps.uses_primary();
                let empty = maps.is_empty();
                request.acceleration.maps = maps;
                if uses_primary {
                    request.acceleration.joint_start_after_trial = after_trial;
                } else if !empty {
                    // A Grace-only map resumes by seed, one below the requested
                    // start so the caller's own seed is still considered.
                    request.acceleration.grace_start_after_seed = if request.start_seed != 0 {
                        Some(request.start_seed - 1)
                    } else {
                        None
                    };
                }
            }
            let outcome = {
                let Self {
                    preview,
                    auxiliary_cache,
                    ..
                } = self;
                if preview.is_none() {
                    let loaded = match self.resource_version {
                        Some(version) => nioh3_data::load_preview_resources_for_file_version(
                            &self.data_root,
                            version,
                        ),
                        None => nioh3_data::load_preview_resources(&self.data_root),
                    }
                    .map_err(|error| HostError::coded("RESOURCE_MISMATCH", error.to_string()))?;
                    *preview = Some(Box::new(loaded));
                }
                let resources = preview
                    .as_deref()
                    .ok_or_else(|| HostError::rejected("preview tables unavailable"))?;
                let tables = nioh3_domain::preview::PreviewTables {
                    roster: &resources.roster,
                    context: &resources.context,
                    rules: &resources.rules,
                    states: &resources.states,
                };
                let mut source = HostAuxiliarySource {
                    tables,
                    cache: auxiliary_cache,
                };
                let auxiliary: Option<&mut dyn AuxiliarySource> = if needs_auxiliary {
                    Some(&mut source)
                } else {
                    None
                };
                let mut report = |progress: ScanProgress| {
                    last = Some(progress);
                    ctx.progress(progress.to_json());
                };
                crate::scan::scan_next_candidate(
                    handle.batch(),
                    &request,
                    auxiliary,
                    &mut cancel,
                    &mut report,
                )
            };
            handle.close();
            if let Some(owner) = handle.retirement() {
                self.retired_oracles.push(owner);
            }
            let matched = outcome?;
            match matched {
                Some(matched) => {
                    let level = request.level;
                    let payload = self.compose_payload(&matched, level)?;
                    Ok(json!({"candidate": payload}))
                }
                None => {
                    let resume_seed = last
                        // `min(0xFFFFFFFF, ...)` is exactly the u32 saturation
                        // the shipped resume cursor performs.
                        .map(|progress| progress.current_seed.saturating_add(1))
                        .unwrap_or_else(|| request.start_seed.saturating_add(1));
                    Ok(json!({
                        "candidate": Value::Null,
                        "resume_trial": last.and_then(|progress| progress.joint_trial),
                        "resume_seed": resume_seed,
                    }))
                }
            }
        }

        /// `RuntimeApplication.capture_grace`.
        ///
        /// The map is measured from the game's own generator and persisted under
        /// the state root so a later search can reuse it while the game is
        /// closed. Nothing about the save is written: only the cache.
        fn capture_grace(&mut self, params: &Value, ctx: &JobContext) -> Result<Value, HostError> {
            let template_value = params
                .get("template")
                .cloned()
                .ok_or_else(HostError::invalid_request)?;
            self.preflight(&template_value, "capture_grace")?;
            let template_hex = template_value
                .get("template_hex")
                .and_then(Value::as_str)
                .ok_or_else(HostError::invalid_request)?;
            let template = decode_hex(template_hex)?;
            let fingerprint = template_value
                .get("save_fingerprint")
                .and_then(Value::as_str)
                .ok_or_else(HostError::invalid_request)?
                .to_string();
            let playthrough = param_u64(params, "playthrough")? as u8;
            let rarity = param_u64(params, "rarity")? as u8;
            let level = param_u64(params, "level")? as u16;
            let recommended_level = param_u64(params, "recommended_level")? as u16;

            let mut handle = self.open_oracle()?;
            let mut cancel = || ctx.cancelled();
            let outcome = crate::grace_capture::build_live_grace_output_map(
                handle.batch(),
                &template,
                playthrough,
                rarity,
                level,
                recommended_level,
                0,
                &mut cancel,
                &mut |progress| ctx.progress(progress.to_json()),
            );
            handle.close();
            if let Some(owner) = handle.retirement() {
                self.retired_oracles.push(owner);
            }
            let mapping = outcome?;
            let digest = self.generation_context()?.digest().to_string();
            let path = crate::save_app::grace_map_cache_path(
                &self.state_root,
                &fingerprint,
                playthrough,
                rarity,
                &digest,
            );
            let payload =
                crate::save_app::grace_map_to_cache_payload(&mapping, &fingerprint, &digest)?;
            crate::grace_capture::save_grace_map_cache(&path, &payload)?;
            Ok(json!({
                "captured": true,
                "playthrough": playthrough,
                "rarity": rarity,
            }))
        }
    }

    impl RoleApplication for RuntimeApplication {
        fn role(&self) -> Role {
            Role::Runtime
        }

        fn context_payload(&self) -> Value {
            self.context
                .as_ref()
                .map(crate::app::protected_context_payload)
                .unwrap_or(Value::Null)
        }

        fn direct(&mut self, method: &str, params: &Value) -> Result<Value, HostError> {
            if method == "runtime.compatibility" {
                if params["action"] == "cancel" {
                    self.compatibility.cancel();
                    return Ok(json!({"compatibility":{"cancelled":true,"accepted":false}}));
                }
                let identity = match crate::compatibility::running_identity() {
                    Ok(value) => value,
                    Err(error)
                        if error.message.contains("not found")
                            || error.message.contains("no running process") =>
                    {
                        return Ok(json!({"compatibility":{"present":false}}))
                    }
                    Err(error) => return Err(error),
                };
                let mut report = match params["action"].as_str() {
                    Some("inspect") => self.compatibility.report(&identity),
                    Some("prepare") => match crate::compatibility::automatic_sources() {
                        Ok(sources) => {
                            self.compatibility
                                .prepare(&identity, &self.state_root, &sources)
                        }
                        Err(error) => self.compatibility.prepare_failed(&identity, &error.message),
                    },
                    Some("accept") => self.compatibility.accept(
                        &identity,
                        params["plan_id"]
                            .as_str()
                            .ok_or_else(HostError::invalid_request)?,
                        params["confirmed"] == true,
                        params["backup_confirmed"] == true,
                    )?,
                    _ => return Err(HostError::invalid_request()),
                };
                if !identity.supported()
                    && matches!(params["action"].as_str(), Some("inspect" | "prepare"))
                {
                    report["probe"] = Self::compatibility_probe(&identity);
                }
                return Ok(json!({"compatibility":report}));
            }
            if method == "runtime.status" {
                return self.status();
            }
            if method == "runtime.inventory_snapshot" {
                return self.inventory_snapshot(params);
            }
            if method == "runtime.character_snapshot" {
                return self.character_snapshot();
            }
            if method == "runtime.menu_selection" {
                return self.menu_selection();
            }
            if method == "runtime.reset_live_add_lock" {
                return self.reset_live_add_lock();
            }
            if method == "runtime.equipment_rules" {
                return crate::equipment_rules::equipment_rules_json(&self.data_root, params);
            }
            if method == "runtime.effect_values" {
                return crate::equipment_rules::effect_values_json(&self.data_root, params);
            }
            if method == "runtime.equipment_seeds" {
                return crate::equipment_seeds::equipment_seeds_json(&self.data_root, params);
            }
            if method == "runtime.scroll_completion_predict" {
                return self.completion_prediction(params);
            }
            Err(HostError::rejected(format!(
                "INVALID_REQUEST: {method} is not an inline protected method"
            )))
        }

        fn run(
            &mut self,
            operation: &str,
            params: Value,
            ctx: &JobContext,
        ) -> Result<Value, HostError> {
            if matches!(
                operation,
                "generate" | "search" | "capture_grace" | "start_override"
            ) || matches!(operation, "live_add_prepare" | "live_batch_prepare")
            {
                self.ensure_generation_context()?;
            }
            match operation {
                "status" => self.status(),
                "character_edit" => self.character_edit(&params),
                "stop_override" => self.stop_override(),
                "start_override" => {
                    let profile = params
                        .get("profile")
                        .cloned()
                        .ok_or_else(HostError::invalid_request)?;
                    self.start_override(&profile)
                }
                "export" => {
                    let candidate_id = param_str(&params, "candidate_id")?;
                    self.candidates.get(&candidate_id).cloned().ok_or_else(|| {
                        HostError::rejected("Native candidate expired; generate again")
                    })
                }
                "count_prepare" => {
                    let source = params
                        .get("source")
                        .cloned()
                        .ok_or_else(HostError::invalid_request)?;
                    let new_count = params
                        .get("new_count")
                        .and_then(Value::as_i64)
                        .ok_or_else(HostError::invalid_request)?;
                    self.count_prepare(&source, new_count)
                }
                "count_execute" => {
                    let operation_id = param_str(&params, "operation_id")?;
                    let plan_digest = param_str(&params, "plan_digest")?;
                    self.count_execute(&operation_id, &plan_digest)
                }
                "count_status" => {
                    let operation_id = param_str(&params, "operation_id")?;
                    self.count_status(&operation_id)
                }
                "count_recover" => {
                    let operation_id = param_str(&params, "operation_id")?;
                    self.count_recover(&operation_id)
                }
                "equipment_add_prepare" => {
                    let approved = self.admit_runtime_feature("live_equipment_add")?;
                    if !self.safe_to_shutdown()? {
                        return Err(HostError::rejected(
                            "Equipment addition requires an idle runtime host",
                        ));
                    }
                    let item_id = params
                        .get("item_id")
                        .and_then(Value::as_u64)
                        .ok_or_else(HostError::invalid_request)?
                        as u16;
                    let rules = crate::equipment_rules::rules(&self.data_root)
                        .ok_or_else(|| HostError::rejected("Equipment tables unavailable"))?;
                    if rules.item(item_id).is_none() {
                        return Err(HostError::rejected(
                            "Select equipment from the current game's item table",
                        ));
                    }
                    let id = param_str(&params, "operation_id")?;
                    let source = match params.get("save_path").and_then(Value::as_str) {
                        Some(path) => PathBuf::from(path),
                        None => {
                            let sources = crate::compatibility::automatic_sources()?;
                            match sources.as_slice() {
                                [source] => source.clone(),
                                _ => return Err(HostError::coded("EQUIPMENT_BACKUP_SOURCE_REQUIRED",
                                    "More than one save was found. Select the current character's SAVEDATA.BIN backup path, then prepare again; no native preview has run.")),
                            }
                        }
                    };
                    let checkpoint = crate::runtime_backup::prepare_equipment_checkpoint(
                        &self.state_root,
                        &source,
                        &id,
                    )
                    .map_err(HostError::from_runtime)?;
                    let mut params = params;
                    params["save_checkpoint"] = checkpoint;
                    let result = self
                        .equipment_application(None, Some(&approved))?
                        .prepare(&id, &params)
                        .map_err(HostError::from_runtime)?;
                    self.equipment_add_result(result)
                }
                "equipment_add_execute" => {
                    let approved = self.admit_runtime_feature("live_equipment_add")?;
                    if !self.safe_to_shutdown()? {
                        return Err(HostError::rejected(
                            "Equipment addition requires an idle runtime host",
                        ));
                    }
                    let id = param_str(&params, "operation_id")?;
                    let digest = param_str(&params, "plan_digest")?;
                    let result = self
                        .equipment_application(Some(&id), Some(&approved))?
                        .execute(&id, &digest)
                        .map_err(HostError::from_runtime)?;
                    self.equipment_add_result(result)
                }
                "equipment_add_status" | "equipment_add_recover" | "equipment_add_cancel" => {
                    let id = param_str(&params, "operation_id")?;
                    if let Some(state)=nioh3_runtime::mutation::equipment_add::EquipmentAddition::unregistered_status(&self.state_root,&id).map_err(HostError::from_runtime)? {
                        return self.equipment_add_result(state);
                    }
                    let application = self.equipment_application(Some(&id), None)?;
                    let result = match operation {
                        "equipment_add_recover" => application.recover(&id),
                        "equipment_add_cancel" => application.cancel(&id),
                        _ => application.status(&id),
                    }
                    .map_err(HostError::from_runtime)?;
                    self.equipment_add_result(result)
                }
                "live_add_prepare" => {
                    let approved = self.admit_runtime_feature("live_scroll_add")?;
                    if !self.safe_to_shutdown()? {
                        return Err(HostError::rejected(
                            "Live addition requires an idle runtime host",
                        ));
                    }
                    let candidate = params
                        .get("candidate")
                        .cloned()
                        .ok_or_else(HostError::invalid_request)?;
                    let save_path = PathBuf::from(param_str(&params, "save_path")?);
                    let previous = params
                        .get("previous_operation_id")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let application = self.live_add_application_for(&approved)?;
                    let prepared = application
                        .prepare(&candidate, &save_path, previous.as_deref())
                        .map_err(HostError::from_runtime)?;
                    Ok(Self::live_add_result(prepared.to_json()))
                }
                "live_add_execute" => {
                    let approved = self.admit_runtime_feature("live_scroll_add")?;
                    if !self.safe_to_shutdown()? {
                        return Err(HostError::rejected(
                            "Resolve existing runtime ownership before insertion",
                        ));
                    }
                    let operation_id = param_str(&params, "operation_id")?;
                    let plan_digest = param_str(&params, "plan_digest")?;
                    let application = self.live_add_application_for(&approved)?;
                    let snapshot = application
                        .execute(&operation_id, &plan_digest)
                        .map_err(HostError::from_runtime)?;
                    Ok(Self::live_add_result(snapshot.to_json()))
                }
                "live_add_status" | "live_add_recover" | "live_add_cancel" => {
                    self.live_add_snapshot(operation, &params)
                }
                "live_batch_prepare" => {
                    let approved = self.admit_runtime_feature("live_scroll_add")?;
                    if !self.safe_to_shutdown()? {
                        return Err(HostError::rejected(
                            "Live addition requires an idle runtime host",
                        ));
                    }
                    let candidates = params
                        .get("candidates")
                        .and_then(Value::as_array)
                        .cloned()
                        .ok_or_else(HostError::invalid_request)?;
                    let save_path = PathBuf::from(param_str(&params, "save_path")?);
                    let application = self.live_add_application_for(&approved)?;
                    let plan = LiveAddBatch::prepare(application, &candidates, &save_path)
                        .map_err(HostError::from_runtime)?;
                    let batch_id = plan
                        .get("batch_id")
                        .and_then(Value::as_str)
                        .ok_or_else(HostError::invalid_request)?
                        .to_string();
                    self.live_batch_status(&batch_id)
                }
                "live_batch_execute" => {
                    let approved = self.admit_runtime_feature("live_scroll_add")?;
                    if !self.safe_to_shutdown()? {
                        return Err(HostError::rejected(
                            "Resolve existing runtime ownership before insertion",
                        ));
                    }
                    let batch_id = param_str(&params, "batch_id")?;
                    let plan_digest = param_str(&params, "plan_digest")?;
                    let application = self.live_add_application_for(&approved)?;
                    LiveAddBatch::execute(
                        application,
                        &batch_id,
                        &plan_digest,
                        &mut || ctx.cancelled(),
                        &mut |value| ctx.progress(value),
                    )
                    .map_err(HostError::from_runtime)?;
                    self.live_batch_status(&batch_id)
                }
                "live_batch_cancel" => {
                    let batch_id = param_str(&params, "batch_id")?;
                    let operations = nioh3_runtime::mutation::operations::LiveAddOperations::new(
                        &self.state_root.join("live-add"),
                    )
                    .map_err(HostError::from_runtime)?;
                    LiveAddBatch::cancel_from_operations(&operations, &batch_id)
                        .map_err(HostError::from_runtime)?;
                    self.live_batch_status(&batch_id)
                }
                "live_batch_status" => {
                    let batch_id = param_str(&params, "batch_id")?;
                    self.live_batch_status(&batch_id)
                }
                "generate" | "search" | "capture_grace" => {
                    if operation == "capture_grace" {
                        return self.capture_grace(&params, ctx);
                    }
                    if operation == "generate" {
                        let seed = param_u64(&params, "seed")?;
                        let natural =
                            u32::try_from(seed).is_ok_and(crate::maps::is_natural_scroll_id);
                        if !natural {
                            return Err(HostError::rejected(
                                "INVALID_SCROLL_ID: this scroll ID cannot occur in the game",
                            ));
                        }
                    }
                    self.run_search(operation, &params, ctx)
                }
                other => Err(HostError::rejected(format!(
                    "OPERATION_REJECTED: runtime.{other} is not a method the protected \
                     runtime contract can express"
                ))),
            }
        }

        fn shutdown(&mut self) -> Result<Value, HostError> {
            // Port of `RuntimeApplication.shutdown`: stop the override, then
            // report ownership. A failed restoration is reported rather than
            // raised, so the host keeps its owner.
            match self.stop_override() {
                Ok(_) => self.status(),
                Err(error) => Ok(json!({
                    "safe_to_shutdown": false,
                    "error": error.message,
                })),
            }
        }

        fn finalize(&mut self) -> FinalizeStep {
            // One decision per attempt. The host owns the bounded retry
            // schedule and the degraded state it ends in; this only reports
            // whether ownership is proven released and, when it is not, why.
            match self.shutdown() {
                Ok(value) if value.get("safe_to_shutdown") == Some(&Value::Bool(true)) => {
                    FinalizeStep::Released
                }
                Ok(value) => FinalizeStep::Retained {
                    reason: finalize_reason(&value, "runtime ownership is unresolved"),
                },
                Err(error) => FinalizeStep::Retained {
                    reason: format!(
                        "runtime status could not be read during finalization: {}",
                        error.message
                    ),
                },
            }
        }
    }
}

/// The shipped ownership reason for a `safe_to_shutdown: false` snapshot.
///
/// Non-Windows answers with `the runtime adapter requires Windows`, which says
/// the state cannot be resolved here at all rather than that it is still being
/// worked on.
fn finalize_reason(value: &Value, fallback: &str) -> String {
    match value.get("error") {
        Some(Value::String(message)) if !message.trim().is_empty() => message.trim().to_string(),
        _ => fallback.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Non-Windows: the protected runtime role cannot own a live target.
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
mod imp {
    use serde_json::{json, Value};

    use super::{
        FinalizeStep, HostError, JobContext, Role, RoleApplication, RuntimeApplication,
        NON_WINDOWS_OWNERSHIP_REASON,
    };

    impl RuntimeApplication {
        pub(super) fn admit_runtime_feature(
            &mut self,
            _feature: &str,
        ) -> Result<crate::compatibility::ExecutableIdentity, HostError> {
            Self::unsupported()
        }

        fn unsupported<T>() -> Result<T, HostError> {
            Err(HostError::from_runtime(
                nioh3_runtime::RuntimeError::UnsupportedPlatform,
            ))
        }

        pub(super) fn status(&mut self) -> Result<Value, HostError> {
            Self::unsupported()
        }

        pub(super) fn stop_override(&mut self) -> Result<Value, HostError> {
            Self::unsupported()
        }

        pub(super) fn safe_to_shutdown(&mut self) -> Result<bool, HostError> {
            Self::unsupported()
        }

        pub(super) fn inventory_snapshot(&mut self, _params: &Value) -> Result<Value, HostError> {
            Self::unsupported()
        }

        pub(super) fn character_snapshot(&mut self) -> Result<Value, HostError> {
            Self::unsupported()
        }

        pub(super) fn menu_selection(&mut self) -> Result<Value, HostError> {
            Self::unsupported()
        }
    }

    impl RoleApplication for RuntimeApplication {
        fn role(&self) -> Role {
            Role::Runtime
        }

        fn context_payload(&self) -> Value {
            self.context
                .as_ref()
                .map(crate::app::protected_context_payload)
                .unwrap_or(Value::Null)
        }

        fn direct(&mut self, method: &str, params: &Value) -> Result<Value, HostError> {
            if method == "runtime.status" {
                return self.status();
            }
            if method == "runtime.inventory_snapshot" {
                return self.inventory_snapshot(params);
            }
            if method == "runtime.character_snapshot" {
                return self.character_snapshot();
            }
            if method == "runtime.menu_selection" {
                return self.menu_selection();
            }
            if method == "runtime.reset_live_add_lock" {
                return self.reset_live_add_lock();
            }
            if method == "runtime.equipment_rules" {
                return crate::equipment_rules::equipment_rules_json(&self.data_root, params);
            }
            if method == "runtime.effect_values" {
                return crate::equipment_rules::effect_values_json(&self.data_root, params);
            }
            if method == "runtime.equipment_seeds" {
                return crate::equipment_seeds::equipment_seeds_json(&self.data_root, params);
            }
            if method == "runtime.scroll_completion_predict" {
                return self.completion_prediction(params);
            }
            Err(HostError::rejected(format!(
                "INVALID_REQUEST: {method} is not an inline protected method"
            )))
        }

        fn run(
            &mut self,
            operation: &str,
            params: Value,
            _ctx: &JobContext,
        ) -> Result<Value, HostError> {
            match operation {
                "export" => {
                    let candidate_id = params
                        .get("candidate_id")
                        .and_then(Value::as_str)
                        .ok_or_else(HostError::invalid_request)?;
                    self.candidates.get(candidate_id).cloned().ok_or_else(|| {
                        HostError::rejected("Native candidate expired; generate again")
                    })
                }
                "status"
                | "stop_override"
                | "start_override"
                | "generate"
                | "search"
                | "capture_grace"
                | "live_add_prepare"
                | "live_add_execute"
                | "live_add_status"
                | "equipment_add_prepare"
                | "equipment_add_execute"
                | "equipment_add_status"
                | "equipment_add_recover"
                | "equipment_add_cancel"
                | "live_add_recover"
                | "live_add_cancel"
                | "live_batch_prepare"
                | "live_batch_execute"
                | "live_batch_cancel"
                | "live_batch_status"
                | "count_prepare"
                | "count_execute"
                | "count_status"
                | "count_recover"
                | "inventory_snapshot"
                | "character_edit" => Self::unsupported(),
                other => Err(HostError::rejected(format!(
                    "OPERATION_REJECTED: runtime.{other} is not a method the protected \
                     runtime contract can express"
                ))),
            }
        }

        fn shutdown(&mut self) -> Result<Value, HostError> {
            Ok(json!({
                "safe_to_shutdown": false,
                "error": NON_WINDOWS_OWNERSHIP_REASON,
            }))
        }

        fn finalize(&mut self) -> FinalizeStep {
            // There is no Windows binding here, so no in-process attempt can
            // ever resolve the owner. Report that instead of spending the whole
            // bounded schedule: the host still stays retained, but one attempt
            // is enough to reach the explicit degraded state.
            FinalizeStep::Terminal {
                reason: NON_WINDOWS_OWNERSHIP_REASON.to_string(),
            }
        }
    }
}

/// Product-selection coverage for the host's live-add binding.
///
/// This proves which identifiers the host builds its executor with. It is
/// selection evidence only: it does not run a native dispatch, and the native
/// path plus disk persistence are observed separately by the acceptance run.
#[cfg(all(test, windows))]
mod live_add_selection_tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use nioh3_runtime::mutation::native_abi::{LiveAddLayout, PC_V202_LIVE_ADD_CANDIDATE};
    use nioh3_runtime::FileVersion;

    use super::RuntimeApplication;

    fn select(version: FileVersion) -> (&'static LiveAddLayout, &'static str) {
        RuntimeApplication::live_add_binding_for_version(version)
            .expect("this version selects a binding")
    }

    #[test]
    fn the_host_selects_one_live_add_binding_per_exact_version() {
        let (layout, display) = select(FileVersion::new(2, 0, 1, 0));
        assert_eq!(layout.profile_id, "pc-v2.01-live-add-r1");
        assert_eq!(display, "PC v2.01");

        let (layout, display) = select(FileVersion::new(2, 0, 2, 0));
        assert_eq!(layout.profile_id, "pc-v2.02-live-add-candidate");
        assert_eq!(display, "PC v2.02");
        // The selected v2.02 layout is field-for-field the accepted constant the
        // native acceptance observed - not a renamed twin or a re-derived one.
        assert_eq!(*layout, PC_V202_LIVE_ADD_CANDIDATE);
        // ... and the accepted binding still attaches the pinned executable
        // digest, so the wrong build refuses before any read or dispatch.
        let binding =
            nioh3_runtime::mutation::native_executor::accepted_live_add_binding(layout, display)
                .expect("the accepted binding table names this pair");
        assert_eq!(
            binding.executable_sha256,
            Some("E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130")
        );

        for (major, minor, build, revision) in [(2, 0, 0, 2), (2, 0, 2, 1), (2, 0, 3, 0)] {
            assert!(
                RuntimeApplication::live_add_binding_for_version(FileVersion::new(
                    major, minor, build, revision
                ))
                .is_err(),
                "{major}.{minor}.{build}.{revision} must select nothing"
            );
        }
    }
}

/// The runtime identity must read its profile from `data/game_versions`.
///
/// v0.8.0 passed the worker's `--data-root` (`.../data`) straight to the
/// profile loader, which then opened `data/pc_v2_02.json` and failed with an OS
/// "file not found" before any approval decision. These tests pin the shared
/// resolver against the shipped tree and prove the resolved directory reaches
/// the real loader for each purpose.
#[cfg(test)]
mod profile_dir_tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use nioh3_runtime::profile::{profile_for_game_version_for, ProfilePurpose};
    use nioh3_runtime::FileVersion;
    use std::path::Path;

    use super::profile_dir_for;

    fn data_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("nioh3_scroll_editor")
            .join("data")
    }

    #[test]
    fn the_worker_data_root_resolves_to_the_profile_directory() {
        let data_root = data_root();
        let profile_dir = profile_dir_for(&data_root);
        assert_eq!(profile_dir, data_root.join("game_versions"));
        assert!(profile_dir.join("pc_v2_02.json").is_file());
        // A root that already names the profile directory is used unchanged.
        assert_eq!(profile_dir_for(&profile_dir), profile_dir);
    }

    #[test]
    fn the_resolved_directory_reaches_the_version_approval_gate() {
        let profile_dir = profile_dir_for(&data_root());
        let v202 = FileVersion::new(2, 0, 2, 0);
        // The temporary overrides are approved for PC v2.02 ...
        let resolved =
            profile_for_game_version_for(v202, &profile_dir, ProfilePurpose::TemporaryOverride)
                .expect("the override purpose resolves the shipped v2.02 profile");
        assert_eq!(resolved.display_version, "PC v2.02");
        // ... and so is the native generation oracle ...
        let oracle = profile_for_game_version_for(v202, &profile_dir, ProfilePurpose::NativeOracle)
            .expect("the oracle purpose resolves the shipped v2.02 profile");
        assert_eq!(oracle, resolved);
        // ... while the blanket native-write purpose still refuses it by name,
        // never with an IO error about a misplaced file.
        let refused =
            profile_for_game_version_for(v202, &profile_dir, ProfilePurpose::NativeWrites)
                .expect_err("PC v2.02 is not blanket-approved");
        assert_eq!(
            refused,
            nioh3_runtime::RuntimeError::ProfileNotApproved {
                profile: "PC v2.02".to_string(),
            }
        );
    }
}

#[cfg(all(test, windows))]
mod receipt_control_tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

    use super::*;
    use nioh3_runtime::mutation::native_executor::AdmissionLock;
    use nioh3_runtime::mutation::operations::LiveAddOperations;
    use serde_json::json;

    const OPERATION: &str = "20000000-0000-4000-8000-000000000001";
    const BATCH: &str = "20000000-0000-4000-8000-000000000002";

    struct Fixture {
        root: PathBuf,
        operations: LiveAddOperations,
    }

    impl Fixture {
        fn new() -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "nioh3-receipt-control-{}-{stamp}",
                std::process::id()
            ));
            let operations = LiveAddOperations::new(&root.join("live-add")).unwrap();
            Self { root, operations }
        }

        fn prepare(&self) -> nioh3_runtime::mutation::operations::OperationSnapshot {
            self.operations
                .prepare(
                    OPERATION,
                    &json!({
                        "operation_id": OPERATION,
                        "pid": u32::MAX,
                        "process_creation_time": "owned-journal-fixture",
                    }),
                )
                .unwrap()
        }

        fn batch(&self) -> PathBuf {
            let first = self.prepare();
            let plan = json!({
                "batch_id": BATCH,
                "candidates": [{"candidate_id":"owned-candidate"}],
                "first": first.to_json(),
                "save_path": "owned-journal-only-no-save",
            });
            let digest = nioh3_save::save::sha256_hex(
                nioh3_runtime::mutation::count::canonical_json(&plan).as_bytes(),
            );
            let directory = self.root.join("live-add/batches").join(BATCH);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("plan.json"),
                serde_json::to_vec(&json!({"plan":plan,"digest":digest})).unwrap(),
            )
            .unwrap();
            directory
        }

        fn invoke(&self, operation: &'static str, params: Value) -> Value {
            // Every call starts a fresh application, with no selected image,
            // no loaded resources, and only the previous worker's journal.
            let mut application = RuntimeApplication::deferred(
                self.root.clone(),
                &self.root.join("missing-data"),
                &self.root.join("missing-contracts"),
                None,
                None,
            )
            .unwrap();
            let jobs = crate::jobs::ProtectedJobs::new();
            jobs.start(operation, false, move |context| {
                let result = application.run(operation, params, context);
                assert!(
                    application.live_add.is_none(),
                    "No native adapter may be created"
                );
                assert!(
                    application.context.is_none(),
                    "No generation resources may load"
                );
                result
            })
            .unwrap();
            jobs.join();
            jobs.current()["job"].clone()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn a_restarted_worker_reads_and_cancels_a_prepared_scroll_without_game_or_resources() {
        let fixture = Fixture::new();
        let prepared = fixture.prepare();
        let params = json!({"operation_id":OPERATION});
        let status = fixture.invoke("live_add_status", params.clone());
        assert_eq!(status["state"], "completed", "{status}");
        assert_eq!(status["result"]["live_add"]["state"], "prepared");
        assert_eq!(
            status["result"]["live_add"]["plan_digest"],
            prepared.plan_digest
        );

        let cancelled = fixture.invoke("live_add_cancel", params.clone());
        assert_eq!(cancelled["state"], "completed", "{cancelled}");
        assert_eq!(cancelled["result"]["live_add"]["state"], "cancelled");
        let restarted = fixture.invoke("live_add_status", params);
        assert_eq!(restarted["result"]["live_add"]["state"], "cancelled");
    }

    #[test]
    fn a_restarted_worker_preserves_an_uncertain_scroll_claim() {
        let fixture = Fixture::new();
        let prepared = fixture.prepare();
        fixture
            .operations
            .claim(OPERATION, &prepared.plan_digest)
            .unwrap();
        let claim_path = fixture
            .operations
            .directory(OPERATION)
            .unwrap()
            .join("claim.json");
        let claim = std::fs::read(&claim_path).unwrap();
        let params = json!({"operation_id":OPERATION});
        let status = fixture.invoke("live_add_status", params.clone());
        assert_eq!(status["result"]["live_add"]["state"], "uncertain");
        let cancelled = fixture.invoke("live_add_cancel", params);
        assert_eq!(cancelled["state"], "failed", "{cancelled}");
        assert_eq!(std::fs::read(claim_path).unwrap(), claim);
        assert_eq!(
            fixture
                .operations
                .snapshot(OPERATION)
                .unwrap()
                .state
                .as_str(),
            "uncertain"
        );
    }

    #[test]
    fn a_restarted_worker_cancels_a_prepared_batch_without_releasing_a_native_lock() {
        let fixture = Fixture::new();
        fixture.batch();
        let native_directory = fixture.root.join("live-add/native-executor");
        let _held = AdmissionLock::acquire(&native_directory).unwrap();
        let params = json!({"batch_id":BATCH});
        let status = fixture.invoke("live_batch_status", params.clone());
        assert_eq!(status["state"], "completed", "{status}");
        assert_eq!(status["result"]["live_batch"]["state"], "prepared");
        let cancelled = fixture.invoke("live_batch_cancel", params.clone());
        assert_eq!(cancelled["state"], "completed", "{cancelled}");
        assert_eq!(cancelled["result"]["live_batch"]["state"], "cancelled");
        let restarted = fixture.invoke("live_batch_status", params);
        assert_eq!(restarted["result"]["live_batch"]["state"], "cancelled");
        assert_eq!(
            fixture
                .operations
                .snapshot(OPERATION)
                .unwrap()
                .state
                .as_str(),
            "cancelled"
        );
        assert!(AdmissionLock::acquire(&native_directory).is_err());
        assert!(native_directory.join("admission.lock").is_file());
    }

    #[test]
    fn a_restarted_worker_reports_an_uncertain_batch_without_cancelling_its_child() {
        let fixture = Fixture::new();
        let directory = fixture.batch();
        let (digest, _) = fixture.operations.plan(OPERATION).unwrap();
        fixture.operations.claim(OPERATION, &digest).unwrap();
        let envelope: Value =
            serde_json::from_slice(&std::fs::read(directory.join("plan.json")).unwrap()).unwrap();
        std::fs::write(
            directory.join("claim.json"),
            serde_json::to_vec(&json!({"digest":envelope["digest"]})).unwrap(),
        )
        .unwrap();
        std::fs::write(
            directory.join("child-000.json"),
            serde_json::to_vec(&json!({"operation_id":OPERATION})).unwrap(),
        )
        .unwrap();
        let claim = std::fs::read(directory.join("claim.json")).unwrap();
        let params = json!({"batch_id":BATCH});
        let status = fixture.invoke("live_batch_status", params.clone());
        assert_eq!(status["state"], "completed", "{status}");
        assert_eq!(status["result"]["live_batch"]["state"], "uncertain");
        let cancelled = fixture.invoke("live_batch_cancel", params);
        assert_eq!(cancelled["state"], "failed", "{cancelled}");
        assert_eq!(std::fs::read(directory.join("claim.json")).unwrap(), claim);
        assert_eq!(
            fixture
                .operations
                .snapshot(OPERATION)
                .unwrap()
                .state
                .as_str(),
            "uncertain"
        );
        assert!(!directory.join("receipt.json").exists());
    }
}
