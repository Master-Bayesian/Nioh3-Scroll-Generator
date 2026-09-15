//! Development-only read-only preview worker for the Nioh 3 backend migration.
//!
//! This crate owns the application layer the shipped Python worker keeps in
//! `nioh3_scroll_editor/core_services.py`, `candidate_transfer.py` and
//! `worker_transport.py`: the `GenerationContext` identity binding, candidate
//! identity, the bounded length-prefixed frame transport, request validation
//! and the handshake/preview responses.
//!
//! Pure generation stays in `nioh3-domain` and resource loading in
//! `nioh3-data`; nothing here is wired into the shipped Tauri host, and the
//! binary refuses to serve unless it is explicitly started for development.

pub mod context;
pub mod engine;
pub mod model;
pub mod native;
pub mod payload;
pub mod protocol;
pub mod transport;

pub use context::{capture_context, ContextError, GenerationContext};
pub use engine::{Engine, EngineError};
pub use model::{candidate_identity, Candidate, CandidateEffect, RecordStage};
pub use protocol::{Request, RequestError};
pub use transport::{read_frame, write_frame, TransportError, MAX_FRAME_BYTES};
