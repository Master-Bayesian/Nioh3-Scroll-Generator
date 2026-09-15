//! Shared typed inputs for offline enemy generation. Resource I/O is external.

use std::{collections::BTreeMap, fmt};

pub const ENEMY_TEXT_SHA256: &str =
    "f8799b5db54a9ca46f52bcd6c037b2ad9b413dc83a26f1d3f0e61251bfb48023";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnemyError {
    InvalidInput(String),
    MissingData(String),
    Unsupported(String),
}

impl fmt::Display for EnemyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(s) | Self::MissingData(s) | Self::Unsupported(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for EnemyError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionVariant {
    Solo,
    Expedition,
}

#[derive(Debug, Clone)]
pub struct RosterTables {
    pub enemies: Vec<[u8; 28]>,
    pub contexts: Vec<[u8; 48]>,
    pub terrains: Vec<[u8; 52]>,
    pub terrain_keys: Vec<u16>,
    pub parameter_types: BTreeMap<u32, u32>,
}

#[derive(Debug, Clone)]
pub struct ContextTables {
    pub contexts: Vec<[u8; 48]>,
    pub optional_multipliers: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, Copy)]
pub struct RosterInput {
    pub seed: u32,
    pub playthrough: u8,
    pub variant: MissionVariant,
    pub auxiliary_mode: u8,
    pub terrain_row_index: usize,
    pub selector: u8,
    pub flags: [bool; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub wave_index: usize,
    pub position: usize,
    pub native_spawn_key: u32,
    pub lookup_key: u32,
    pub role: u8,
    pub source_row_index: usize,
    pub selector_class: u8,
    pub scratch_rule_key: u16,
}

#[derive(Debug, Clone)]
pub struct RosterResult {
    pub seed: u32,
    pub playthrough: u8,
    pub variant: MissionVariant,
    pub auxiliary_mode: u8,
    pub terrain: u8,
    pub branch_class: u8,
    pub waves: Vec<Vec<Occurrence>>,
    pub state_after_roster: u32,
    pub parent_draws: u64,
}

impl RosterResult {
    pub fn occurrences(&self) -> impl Iterator<Item = &Occurrence> {
        self.waves.iter().flatten()
    }
}

/// Native lookup outcomes retain the difference between absent and unobserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eligibility {
    EnemyAbsent,
    SubtypeAbsent,
    SubtypeFlags(u32),
    Unknown,
}

#[derive(Debug, Clone)]
pub struct EnemyStateTables {
    pub text_sha256: String,
    pub positions_by_terrain: BTreeMap<u8, Vec<[u8; 24]>>,
    pub eligibility: BTreeMap<u32, Eligibility>,
    pub enemy_index_complete: bool,
    pub config_4543: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Possession {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SourceTrial {
    pub selector: u8,
    pub spawn: u32,
    pub ticket: i32,
    pub state: u32,
    pub draw: u64,
    pub accepted: bool,
}

#[derive(Debug, Clone)]
pub struct WraithResult {
    pub exact: bool,
    /// Same flattened order as RosterResult::occurrences().
    pub states: Vec<Possession>,
    pub source_entry_state: Option<u32>,
    pub source_entry_draw: Option<u64>,
    pub final_state: Option<u32>,
    pub final_draws: Option<u64>,
    pub trials: Vec<SourceTrial>,
    pub missing: Vec<String>,
}
