//! Protected save/runtime role host for the Nioh 3 backend migration.
//!
//! This crate owns the protected-worker *process*: the shipped stdio framing,
//! the versioned `protected-request`/`protected-response` contract, the
//! role-prefixed dispatch and the single-owner protected job machine behind the
//! shipped `ProtectedJobs`. It is the Rust counterpart of
//! `nioh3_scroll_editor/protected_worker.py` plus the two application facades
//! (`save_application.SaveApplication`, `runtime_application.RuntimeApplication`)
//! built on the ported `nioh3-save` and `nioh3-runtime` crates.
//!
//! Boundaries kept from the shipped host:
//!
//! - EOF never discards in-flight write ownership. The read loop stops, the
//!   protected job is cancelled and joined, and only then is the runtime role
//!   allowed to restore; a broken pipe is not permission to abandon a hook.
//! - The killable read-only search worker stays a separate process. Nothing in
//!   this crate serves `offline_search` methods, and no search route is exposed
//!   through the protected contract.
//! - No renderer bytes and no reconstructed records cross the wire: the save
//!   side consumes the typed candidate transfer, and the request that installs
//!   carries the finalized record pair the search side produced.

pub mod app;
pub mod composition;
pub mod contract;
pub mod error;
pub mod grace_capture;
pub mod host;
pub mod jobs;
pub mod maps;
pub mod oracle;
pub mod runtime_app;
pub mod runtime_backup;
pub mod save_app;
pub mod scan;
#[cfg(feature = "test-fake")]
pub mod scan_bench_api;

pub use app::{JobContext, Role, RoleApplication};
pub use contract::Contract;
pub use error::HostError;
pub use host::serve;
pub use jobs::ProtectedJobs;
pub use runtime_app::RuntimeApplication;
pub use save_app::SaveApplication;
