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
    state_root: PathBuf,
    data_root: PathBuf,
    context: EngineContext,
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

impl RuntimeApplication {
    /// Build the runtime role for one state root and data root.
    pub fn new(
        state_root: PathBuf,
        data_root: &Path,
        context: EngineContext,
    ) -> Result<Self, HostError> {
        Ok(Self {
            state_root,
            data_root: data_root.to_path_buf(),
            context,
            candidates: HashMap::new(),
            #[cfg(windows)]
            host: nioh3_runtime::mutation::WindowsMutationHost::new(),
            #[cfg(windows)]
            roster_roles: None,
            #[cfg(windows)]
            live_add: None,
            #[cfg(windows)]
            preview: None,
            #[cfg(windows)]
            resource_version: Some(nioh3_data::CURRENT_RESOURCE_VERSION),
            #[cfg(windows)]
            auxiliary_cache: HashMap::new(),
            #[cfg(windows)]
            retired_oracles: Vec::new(),
        })
    }

    /// A count editor bound to the running game.
    ///
    /// `prepare`/`execute` own one explicit write to the live process, so they
    /// need the real module base; with no game this fails with the shipped
    /// process-absence error instead of fabricating a plan.
    #[cfg(windows)]
    fn count_editor_for_game(&self) -> Result<nioh3_runtime::mutation::CountEditor, HostError> {
        let pid = nioh3_runtime::single_process_id(nioh3_runtime::GAME_IMAGE_NAME)
            .map_err(HostError::from_runtime)?;
        let base = nioh3_runtime::module_range(pid, nioh3_runtime::GAME_MODULE_NAME)
            .map_err(HostError::from_runtime)?
            .base;
        let memory = nioh3_runtime::mutation::WindowsCountMemory::new(
            pid,
            base,
            nioh3_runtime::mutation::PC_V201_COUNT_LAYOUT,
            nioh3_runtime::mutation::WindowsCountProcesses,
        );
        nioh3_runtime::mutation::CountEditor::new(&self.state_root, Box::new(memory))
            .map_err(HostError::from_runtime)
    }

    #[cfg(not(windows))]
    fn count_editor_for_game(&self) -> Result<(), HostError> {
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
        let mut editor = self.count_editor_for_game()?;
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
        if !self.safe_to_shutdown()? {
            return Err(HostError::rejected(
                "Stop temporary overrides before editing remaining count",
            ));
        }
        let mut editor = self.count_editor_for_game()?;
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
        /// `running_game_identity()`: exactly one verified supported game.
        fn identity(&self) -> Result<GameIdentity, HostError> {
            nioh3_runtime::identify_running_game(&self.data_root).map_err(HostError::from_runtime)
        }

        /// The game identity the native live-add path uses.
        ///
        /// Live addition resolves the profile for its own purpose and from the
        /// profile-document directory, so a document approved for live addition
        /// only is reachable while every other native path keeps the existing
        /// blanket resolution and its refusals. The executable, version, module
        /// and process-identity checks are identical to [`Self::identity`]; only
        /// the profile approval purpose differs, and the executor still proves
        /// the profile id and, where pinned, the exact executable digest before
        /// any read or dispatch.
        fn live_add_identity(&self) -> Result<GameIdentity, HostError> {
            // Workers are launched with `--data-root <runtime>/data` while the
            // profile documents live in `data/game_versions`; a root that
            // already names the profile directory is used unchanged.
            let nested = self.data_root.join("game_versions");
            let profile_dir = if nested.is_dir() {
                nested
            } else {
                self.data_root.clone()
            };
            nioh3_runtime::identify_running_game_for(
                &profile_dir,
                nioh3_runtime::profile::ProfilePurpose::LiveAdd,
            )
            .map_err(HostError::from_runtime)
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
            if !self.safe_to_shutdown()? {
                return Err(HostError::rejected(
                    "Stop the existing override and wait for pending native calls",
                ));
            }
            let GameIdentity {
                identity,
                profile: runtime_profile,
                ..
            } = self.identity()?;
            let sessions = self.build_sessions(profile, identity.pid, runtime_profile)?;
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

        /// The reviewed live-addition application, built on first use.
        ///
        /// The binding is selected by the exact running executable version:
        /// PC v2.01 keeps the shipped layout and PC v2.02 selects the same
        /// accepted binding the native acceptance observed. Every other version
        /// selects nothing and refuses here. The executor still proves the
        /// profile id and, for a pinned binding, the exact executable digest
        /// before any read or dispatch.
        ///
        /// Construction needs the real game identity because the native
        /// transport attaches to that process; with no game this is the shipped
        /// process-absence failure rather than a refusal to serve.
        fn live_add_application(&mut self) -> Result<&mut LiveAddApplication, HostError> {
            if self.live_add.is_none() {
                let identity = self.live_add_identity()?;
                let (layout, display_version) =
                    Self::live_add_binding_for_version(identity.file_version)?;
                let directory = self.state_root.join("live-add").join("native-executor");
                let transport = NativeDebugTransport::new(
                    identity.identity.pid,
                    *layout,
                    nioh3_runtime::GAME_MODULE_NAME,
                    &directory,
                )
                .map_err(HostError::from_runtime)?;
                let executor = NativeLiveAddExecutor::new(transport, *layout, display_version);
                let application = LiveAddApplication::new(
                    &self.state_root,
                    self.context.digest(),
                    Box::new(executor),
                    Box::new(crate::runtime_backup::SaveBackupAdapter::new(
                        &self.state_root,
                    )),
                    Box::new(DomainCatalogPolicy::new()),
                )
                .map_err(HostError::from_runtime)?;
                self.live_add = Some(application);
            }
            self.live_add
                .as_mut()
                .ok_or_else(|| HostError::rejected("live-add application unavailable"))
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
            let application = self.live_add_application()?;
            let snapshot = match operation {
                "live_add_status" => application.status(&operation_id),
                "live_add_recover" => application.recover(&operation_id),
                _ => application.cancel(&operation_id),
            }
            .map_err(HostError::from_runtime)?;
            Ok(Self::live_add_result(snapshot.to_json()))
        }

        /// `RuntimeApplication.live_batch_status`: the reduced UI envelope.
        fn live_batch_status(&mut self, batch_id: &str) -> Result<Value, HostError> {
            let application = self.live_add_application()?;
            let value =
                LiveAddBatch::status(application, batch_id).map_err(HostError::from_runtime)?;
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
            if digest != self.context.digest() {
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
            let identity = self.identity()?;
            let mut oracle = NativeOracle::new(
                identity.identity.pid,
                identity.module.base,
                identity.profile.clone(),
            )
            .map_err(HostError::from_runtime)?;
            let session =
                RemoteSession::open(identity.identity.pid).map_err(HostError::from_runtime)?;
            oracle
                .open(Box::new(session))
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
                self.context.digest(),
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
            let wire =
                nioh3_worker::payload::transfer_json(&candidate, self.context.digest(), level);
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
                let digest = self.context.digest().to_string();
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
            let digest = self.context.digest().to_string();
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
            crate::app::protected_context_payload(&self.context)
        }

        fn direct(&mut self, method: &str, _params: &Value) -> Result<Value, HostError> {
            if method == "runtime.status" {
                return self.status();
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
            match operation {
                "status" => self.status(),
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
                "live_add_prepare" => {
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
                    let application = self.live_add_application()?;
                    let prepared = application
                        .prepare(&candidate, &save_path, previous.as_deref())
                        .map_err(HostError::from_runtime)?;
                    Ok(Self::live_add_result(prepared.to_json()))
                }
                "live_add_execute" => {
                    if !self.safe_to_shutdown()? {
                        return Err(HostError::rejected(
                            "Resolve existing runtime ownership before insertion",
                        ));
                    }
                    let operation_id = param_str(&params, "operation_id")?;
                    let plan_digest = param_str(&params, "plan_digest")?;
                    let application = self.live_add_application()?;
                    let snapshot = application
                        .execute(&operation_id, &plan_digest)
                        .map_err(HostError::from_runtime)?;
                    Ok(Self::live_add_result(snapshot.to_json()))
                }
                "live_add_status" | "live_add_recover" | "live_add_cancel" => {
                    self.live_add_snapshot(operation, &params)
                }
                "live_batch_prepare" => {
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
                    let application = self.live_add_application()?;
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
                    if !self.safe_to_shutdown()? {
                        return Err(HostError::rejected(
                            "Resolve existing runtime ownership before insertion",
                        ));
                    }
                    let batch_id = param_str(&params, "batch_id")?;
                    let plan_digest = param_str(&params, "plan_digest")?;
                    let application = self.live_add_application()?;
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
                    let application = self.live_add_application()?;
                    LiveAddBatch::cancel(application, &batch_id)
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
    }

    impl RoleApplication for RuntimeApplication {
        fn role(&self) -> Role {
            Role::Runtime
        }

        fn context_payload(&self) -> Value {
            crate::app::protected_context_payload(&self.context)
        }

        fn direct(&mut self, method: &str, _params: &Value) -> Result<Value, HostError> {
            if method == "runtime.status" {
                return self.status();
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
                "status" | "stop_override" | "start_override" | "generate" | "search"
                | "capture_grace" | "live_add_prepare" | "live_add_execute" | "live_add_status"
                | "live_add_recover" | "live_add_cancel" | "live_batch_prepare"
                | "live_batch_execute" | "live_batch_cancel" | "live_batch_status"
                | "count_prepare" | "count_execute" | "count_status" | "count_recover" => {
                    Self::unsupported()
                }
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
        let binding = nioh3_runtime::mutation::native_executor::accepted_live_add_binding(
            layout, display,
        )
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
