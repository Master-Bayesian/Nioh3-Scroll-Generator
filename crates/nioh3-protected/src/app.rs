//! The role abstraction the host drives, plus the run-time job context.

use serde_json::Value;

use nioh3_worker::engine::EngineContext;

use crate::error::HostError;
pub use crate::jobs::JobContext;

/// What one [`RoleApplication::finalize`] attempt proved about ownership.
///
/// This is the application's *decision*, never the process-lifetime policy: the
/// host owns the bounded retry schedule and the degraded state a bounded
/// schedule ends in (`crate::host`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalizeStep {
    /// The application no longer owns anything that needs this process alive.
    Released,
    /// Ownership is still unresolved. The host keeps the process retained and
    /// reports the reason while it is inside its active finalization window.
    Retained { reason: String },
    /// Retained, and no further in-process attempt can change that: the state
    /// cannot be resolved here (for example the platform cannot own a target),
    /// so the host may end its active window in the degraded terminal state
    /// instead of spending the whole bound.
    Terminal { reason: String },
}

/// One bounded finalization schedule.
///
/// The process must never be force-killed and must never exit while native
/// ownership is unresolved, so this budget only bounds the *active* retry
/// phase. After it, the host stays retained and observable with an explicit
/// degraded terminal state instead of spinning a bare sleep forever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinalizePlan {
    /// The maximum number of recorded finalization attempts before the host
    /// must publish its degraded terminal state.
    pub max_attempts: u32,
    /// The pause between attempts.
    pub retry_interval: std::time::Duration,
}

impl FinalizePlan {
    /// The shipped development schedule: 12 attempts at the historical 250 ms
    /// retry, so an unresolved owner becomes reportable within about three
    /// seconds while an ordinary release still returns on the first attempt.
    pub const DEFAULT: Self = Self {
        max_attempts: 12,
        retry_interval: std::time::Duration::from_millis(250),
    };
}

/// How a retained host keeps behaving after its bounded active phase ends.
///
/// Bounded does not mean "stop working and abandon the owner": the process may
/// never exit while ownership is unresolved, so once the active window closes
/// the host keeps polling at this slower cadence. The window exists to *report*
/// an unresolvable owner, not to stop watching it or to let the process exit.
pub const FINALIZE_DEGRADED_POLL: std::time::Duration = std::time::Duration::from_millis(1_000);

/// The two protected roles the shipped host serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Save,
    Runtime,
}

impl Role {
    /// `--role` argument value and the method prefix without the dot.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Save => "save",
            Self::Runtime => "runtime",
        }
    }

    /// `args.role + '.'`, the shipped role gate.
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Save => "save.",
            Self::Runtime => "runtime.",
        }
    }
}

/// One protected role application.
///
/// The host calls exactly the two entry points the shipped worker uses:
/// [`Self::direct`] for the methods it answers inline (`runtime.status`), and
/// [`Self::run`] for every role-prefixed method it defers to the job machine.
pub trait RoleApplication: Send {
    fn role(&self) -> Role;

    /// The handshake `context` object.
    fn context_payload(&self) -> Value;

    /// An inline method (`runtime.status`).
    fn direct(&mut self, method: &str, params: &Value) -> Result<Value, HostError>;

    /// A job method, dispatched by its suffix (`count_prepare`, `inventory`, ...).
    fn run(&mut self, operation: &str, params: Value, ctx: &JobContext)
        -> Result<Value, HostError>;

    /// The `shutdown` result for this role.
    fn shutdown(&mut self) -> Result<Value, HostError>;

    /// Run after the read loop ends, before the process exits.
    ///
    /// The runtime role uses this to drain ownership exactly like the shipped
    /// `finally` block; the save role has nothing to restore.
    /// Return [`FinalizeStep::Released`] only when the application no longer
    /// owns anything that requires this process to stay alive. Any other result
    /// keeps the host retained; logging an unresolved owner is not an exit
    /// proof, and neither is reaching the end of the host's bounded schedule.
    fn finalize(&mut self) -> FinalizeStep {
        FinalizeStep::Released
    }
}

/// The exact keys `ProtectedHandshake.context` allows.
///
/// The protected schema sets `additionalProperties: false` on that object, so
/// the host publishes exactly this shipped eight-key set and never the
/// read-only worker's wider resolved proof payload.
const PROTECTED_CONTEXT_KEYS: [&str; 8] = [
    "product_version",
    "game_profile",
    "resources_digest",
    "algorithm_version",
    "policy_version",
    "context_digest",
    "seed_accelerator_abi",
    "seed_accelerator_build_id",
];

/// Project an engine identity onto the protected handshake context.
///
/// The protected contract publishes the shipped legacy context shape, so the
/// resolved-versus-legacy distinction stays inside the host instead of leaking
/// the read-only worker's version-bound proof fields onto the protected wire.
/// `context_digest` is the authority digest the host itself compares an
/// incoming candidate or template `context_digest` against, so a client that
/// echoes the handshake is accepted by the same rule that keys the caches.
pub(crate) fn protected_context_payload(context: &EngineContext) -> Value {
    let mut payload = context.to_payload();
    let digest = context.digest().to_string();
    if let Some(object) = payload.as_object_mut() {
        object.retain(|key, _| PROTECTED_CONTEXT_KEYS.contains(&key.as_str()));
        object.insert("context_digest".to_string(), Value::String(digest));
    }
    payload
}
