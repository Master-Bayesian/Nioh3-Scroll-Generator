//! Loadout search over real items: for one attack, which outfit from the player's items deals the most damage.
//!
//! Candidate inputs are reconstructed from explicit tables, source bindings and
//! a captured frame, then evaluated by translated damage code. Numeric agreement
//! on those inputs does not independently prove their producers or the replay
//! environment. Uncovered paths and unresolved contracts are reported explicitly.

pub mod agility;
mod bundle_inputs;
pub mod context;
pub mod damage;
pub mod engine;
pub mod formula;
pub mod gpu;
pub mod action_rows;
pub mod hit_buffer;
pub mod native;
pub mod pipeline;
pub mod search;

pub mod sequence;
pub mod sustained;

mod stat_words;

pub mod oracle_contract;

pub mod search_control;
