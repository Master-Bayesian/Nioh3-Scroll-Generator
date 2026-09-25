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
//!
//! M2.3b adds `jobs` (the single-owner search job), `query` (typed, digest-bound
//! search parameters) and `schema` (the shipped request schema evaluated in
//! process), plus the candidate materialization `collector` drives. The native
//! bounded collector itself is `native_search`/`search_backend`.

pub mod capabilities;
pub mod catalog;
pub mod collector;
pub mod context;
pub mod effect_batch;
pub mod effect_path;
pub mod engine;
pub mod feasibility;
pub mod grace_map;
pub mod jobs;
pub mod model;
pub mod native;
pub mod native_search;
pub mod payload;
pub mod preimage;
pub mod protocol;
pub mod query;
pub mod query_compile;
pub mod recommended_level;
pub mod schema;
pub mod search_backend;
pub mod terrain;
pub mod transport;

pub use capabilities::Capabilities;
pub use catalog::{Catalog, CatalogInputs};
pub use collector::{
    BatchRequest, CandidateSource, CollectorError, IntersectionReport, IntersectionStageCount,
    MaterializedCandidate, SearchBatch, SearchCollector,
};
pub use context::{
    capture_legacy_context, capture_resolved_context, capture_resolved_context_from_bundle,
    legacy_identity_payload, resolved_identity_payload, ContextError, GameFileVersion,
    LegacyGenerationContext, ResolvedGenerationContext,
};
pub use engine::{ContextSelection, Engine, EngineContext, EngineError};
pub use jobs::{JobStore, JobView, StartParams};
pub use model::{candidate_identity, Candidate, CandidateEffect, RecordStage};
pub use native_search::{NativeCapabilities, NativeSearchError, PivotMatch, PivotWindow};
pub use preimage::{
    PreimageAccelerator, PreimageAdapterInfo, PreimageBackend, PreimageError, PreimageIdentity,
    PreimagePolicy,
};
pub use protocol::{Request, RequestError};
pub use query::SearchQuery;
pub use schema::RequestSchema;
pub use search_backend::{CollectedPage, NativePivotQuery, PageRequest, SearchBackend};
pub use transport::{read_frame, write_frame, TransportError, MAX_FRAME_BYTES};
