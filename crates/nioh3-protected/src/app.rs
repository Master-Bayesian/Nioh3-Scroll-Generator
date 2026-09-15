//! The role abstraction the host drives, plus the run-time job context.

use serde_json::Value;

use crate::error::HostError;
pub use crate::jobs::JobContext;

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
    fn finalize(&mut self) {}
}
