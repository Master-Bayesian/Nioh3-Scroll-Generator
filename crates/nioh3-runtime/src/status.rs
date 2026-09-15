//! The `runtime.status` ownership model.
//!
//! Port of `RuntimeApplication.status()` over its ownership state only. The
//! shipped host answers this method without opening the game process, and this
//! model keeps that property: it reads no process, no file and no environment.
//!
//! The wire response is the `RuntimeStatus` object in
//! `packages/contracts/protected-responses.ts`:
//! `override_state`, `hit_count`, `pending_remote_calls`, `safe_to_shutdown`
//! and `error`.

use serde_json::{json, Value};

/// Protected-contract method name this model answers.
pub const STATUS_METHOD: &str = "runtime.status";

pub const STATE_STOPPED: &str = "stopped";
pub const STATE_ARMED_NO_HIT: &str = "armed_no_hit";
pub const STATE_APPLIED_HIT: &str = "applied_hit";
pub const STATE_UNKNOWN: &str = "unknown";

/// One `runtime.status` answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStatus {
    pub override_state: &'static str,
    pub hit_count: u64,
    pub pending_remote_calls: u32,
    pub safe_to_shutdown: bool,
    pub error: Option<String>,
}

impl RuntimeStatus {
    /// The protected contract object, without any extra field.
    pub fn to_json(&self) -> Value {
        json!({
            "override_state": self.override_state,
            "hit_count": self.hit_count,
            "pending_remote_calls": self.pending_remote_calls,
            "safe_to_shutdown": self.safe_to_shutdown,
            "error": self.error,
        })
    }
}

/// State of the temporary mission-override session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OverrideSession {
    /// No override session is owned.
    #[default]
    Absent,
    /// A session is armed and reports a hit count.
    Active { hit_count: u64 },
    /// `hit_count()` failed; ownership is retained and the state is unknown.
    Faulted { message: String },
}

/// One retired oracle that may still owe a native call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetiredOracle {
    pub remote_call_pending: bool,
}

/// Ownership state behind `runtime.status` and `runtime.shutdown`.
///
/// Field-for-field port of the attributes `RuntimeApplication.status()` reads:
/// the live-add application's shutdown safety, the retired oracles and the
/// override session.
#[derive(Debug, Clone, Default)]
pub struct RuntimeOwnership {
    session: OverrideSession,
    live_add_unsafe: bool,
    retired_oracles: Vec<RetiredOracle>,
}

impl RuntimeOwnership {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a retired oracle. `remote_call_pending` means a native call may
    /// still be outstanding, which is exactly what the shipped host refuses to
    /// discard.
    pub fn retire_oracle(&mut self, remote_call_pending: bool) {
        self.retired_oracles.push(RetiredOracle {
            remote_call_pending,
        });
    }

    /// Mirrors `self.live_add is not None and not self.live_add.safe_to_shutdown()`.
    pub fn set_live_add_unsafe(&mut self, unsafe_ownership: bool) {
        self.live_add_unsafe = unsafe_ownership;
    }

    pub fn set_session(&mut self, session: OverrideSession) {
        self.session = session;
    }

    /// `RuntimeApplication.stop_override()` clears the session and answers the
    /// resulting status. A failed restoration retains ownership, so the caller
    /// sets [`OverrideSession::Faulted`] instead of clearing.
    pub fn stop_override(&mut self) -> RuntimeStatus {
        self.session = OverrideSession::Absent;
        self.status()
    }

    pub fn session(&self) -> &OverrideSession {
        &self.session
    }

    /// Retired oracles still retained after the most recent status snapshot.
    pub fn retained_retired_oracles(&self) -> usize {
        self.retired_oracles.len()
    }

    /// Port of `RuntimeApplication.status()`.
    ///
    /// The shipped implementation counts pending calls, then prunes the
    /// retained list, so the count is unaffected by the order. The prune is
    /// still modelled because it is observable through
    /// [`Self::retained_retired_oracles`].
    pub fn status(&mut self) -> RuntimeStatus {
        let pending_retired = self
            .retired_oracles
            .iter()
            .filter(|oracle| oracle.remote_call_pending)
            .count() as u32;
        let pending_remote_calls = pending_retired + u32::from(self.live_add_unsafe);
        self.retired_oracles
            .retain(|oracle| oracle.remote_call_pending);

        let (override_state, hit_count, error) = match &self.session {
            OverrideSession::Absent => (STATE_STOPPED, 0, None),
            OverrideSession::Active { hit_count } => (
                if *hit_count > 0 {
                    STATE_APPLIED_HIT
                } else {
                    STATE_ARMED_NO_HIT
                },
                *hit_count,
                None,
            ),
            OverrideSession::Faulted { message } => (STATE_UNKNOWN, 0, Some(message.clone())),
        };

        RuntimeStatus {
            override_state,
            hit_count,
            pending_remote_calls,
            safe_to_shutdown: matches!(self.session, OverrideSession::Absent)
                && pending_remote_calls == 0,
            error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OverrideSession, RuntimeOwnership, STATE_APPLIED_HIT, STATE_ARMED_NO_HIT, STATE_STOPPED,
        STATE_UNKNOWN,
    };

    #[test]
    fn an_idle_host_is_safe_to_shutdown() {
        let mut ownership = RuntimeOwnership::new();
        let status = ownership.status();
        assert_eq!(status.override_state, STATE_STOPPED);
        assert_eq!(status.hit_count, 0);
        assert_eq!(status.pending_remote_calls, 0);
        assert!(status.safe_to_shutdown);
        assert_eq!(status.error, None);
    }

    #[test]
    fn an_armed_session_reports_no_hit_then_applied_hit() {
        let mut ownership = RuntimeOwnership::new();
        ownership.set_session(OverrideSession::Active { hit_count: 0 });
        let armed = ownership.status();
        assert_eq!(armed.override_state, STATE_ARMED_NO_HIT);
        // A live session owns the runtime even before it records a hit, exactly
        // as `RuntimeApplication.status()` reports it.
        assert!(!armed.safe_to_shutdown);

        ownership.set_session(OverrideSession::Active { hit_count: 3 });
        let applied = ownership.status();
        assert_eq!(applied.override_state, STATE_APPLIED_HIT);
        assert_eq!(applied.hit_count, 3);
        assert!(!applied.safe_to_shutdown);
    }

    #[test]
    fn a_failed_hit_count_keeps_ownership_and_reports_unknown() {
        let mut ownership = RuntimeOwnership::new();
        ownership.set_session(OverrideSession::Faulted {
            message: "access denied".to_string(),
        });
        let status = ownership.status();
        assert_eq!(status.override_state, STATE_UNKNOWN);
        assert_eq!(status.hit_count, 0);
        assert_eq!(status.error.as_deref(), Some("access denied"));
        assert!(!status.safe_to_shutdown);
        assert_eq!(ownership.stop_override().override_state, STATE_STOPPED);
    }

    #[test]
    fn pending_native_calls_block_shutdown_and_cleared_ones_are_pruned() {
        let mut ownership = RuntimeOwnership::new();
        ownership.retire_oracle(true);
        ownership.retire_oracle(false);
        let status = ownership.status();
        assert_eq!(status.pending_remote_calls, 1);
        assert!(!status.safe_to_shutdown);
        assert_eq!(ownership.retained_retired_oracles(), 1);

        ownership.set_live_add_unsafe(true);
        assert_eq!(ownership.status().pending_remote_calls, 2);
        ownership.set_live_add_unsafe(false);
        assert_eq!(ownership.status().pending_remote_calls, 1);
    }

    #[test]
    fn the_contract_object_has_exactly_the_published_keys() {
        let mut ownership = RuntimeOwnership::new();
        assert_eq!(
            ownership.status().to_json().to_string(),
            concat!(
                r#"{"error":null,"hit_count":0,"override_state":"stopped","#,
                r#""pending_remote_calls":0,"safe_to_shutdown":true}"#
            )
        );
    }
}
