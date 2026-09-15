//! Standard-library-only Nioh 3 domain semantics.
//!
//! Offline enemy generation with exact numerical replay. Resource loading and
//! transport remain outside this crate; the production backend is not switched.

pub mod context;
pub mod effect;
pub mod enemy;
pub mod preview;
pub mod record;
pub mod rng;
pub mod roster;
pub mod sequence;
pub mod wraith;
