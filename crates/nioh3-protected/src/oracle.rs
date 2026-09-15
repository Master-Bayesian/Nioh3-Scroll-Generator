//! The batch-oracle seam the protected runtime loops drive.
//!
//! `nioh3_runtime::mutation::NativeBatchOracle` is the concrete product oracle:
//! it attaches to one verified game process, verifies every native signature
//! and then owns one remote region for the batch, seed-range and completion
//! calls. That type cannot be constructed without a real game, which is exactly
//! why the host loops must not name it directly.
//!
//! This module keeps the loops honest and testable at the same time: the loops
//! see [`BatchOracle`], the product binds [`NativeOracle`], and an off-by-default
//! build binds [`scripted::ScriptedOracle`], which answers with deterministic
//! rows built by the same pure record emitter the real path uses. Nothing here
//! weakens the product path: the scripted oracle exists only under
//! `cfg(test)`/`test-fake`, produces records through
//! `nioh3_runtime::mutation::oracle::source_record`, and never touches a
//! process.

use nioh3_runtime::mutation::oracle::NativeBatchOracle;
use nioh3_runtime::RuntimeError;

/// Batch-call timeout the shipped host uses for an ordinary generation.
pub const ORACLE_TIMEOUT_MS: u32 = 60_000;
/// Batch-call timeout the shipped host uses for a live map capture.
pub const ORACLE_MAP_TIMEOUT_MS: u32 = 120_000;
/// `NativeBatchOracle(max_batch_size=128)` on the protected runtime path.
pub const ORACLE_BATCH_SIZE: usize = 128;

/// The four native calls the protected loops make on one oracle.
pub trait BatchOracle {
    fn max_batch_size(&self) -> usize;

    /// `NativeBatchOracle.generate`: one explicit source record per output.
    fn generate(
        &mut self,
        source_records: &[Vec<u8>],
        timeout_ms: u32,
    ) -> Result<Vec<Vec<u8>>, RuntimeError>;

    /// `NativeBatchOracle.generate_seed_range`.
    fn generate_seed_range(
        &mut self,
        template: &[u8],
        start_seed: u32,
        seed_step: u32,
        count: u32,
        playthrough: Option<u32>,
        timeout_ms: u32,
    ) -> Result<Vec<Vec<u8>>, RuntimeError>;

    /// `NativeBatchOracle.finalize_stage_records_batch`.
    fn finalize_stage_records_batch(
        &mut self,
        source_records: &[Vec<u8>],
        reveal: bool,
        timeout_ms: u32,
    ) -> Result<Vec<Vec<u8>>, RuntimeError>;

    /// `NativeBatchOracle.remote_call_pending`: a retired owner that may still
    /// owe a native call must stay retained rather than be discarded.
    fn remote_call_pending(&self) -> bool;
}

/// The product oracle.
pub struct NativeOracle(pub NativeBatchOracle);

impl NativeOracle {
    pub fn new(
        pid: u32,
        module_base: u64,
        profile: nioh3_runtime::NativeRuntimeProfile,
    ) -> Result<Self, RuntimeError> {
        Ok(Self(NativeBatchOracle::new(
            pid,
            module_base,
            profile,
            ORACLE_BATCH_SIZE,
            true,
        )?))
    }

    /// `NativeBatchOracle.open`, including every signature check.
    pub fn open(
        &mut self,
        session: Box<dyn nioh3_runtime::mutation::win_session::RemoteSession + Send>,
    ) -> Result<(), RuntimeError> {
        self.0.open(session)
    }

    pub fn close(&mut self) {
        self.0.close();
    }
}

impl BatchOracle for NativeOracle {
    fn max_batch_size(&self) -> usize {
        ORACLE_BATCH_SIZE
    }

    fn generate(
        &mut self,
        source_records: &[Vec<u8>],
        timeout_ms: u32,
    ) -> Result<Vec<Vec<u8>>, RuntimeError> {
        self.0.generate(source_records, timeout_ms)
    }

    fn generate_seed_range(
        &mut self,
        template: &[u8],
        start_seed: u32,
        seed_step: u32,
        count: u32,
        playthrough: Option<u32>,
        timeout_ms: u32,
    ) -> Result<Vec<Vec<u8>>, RuntimeError> {
        self.0.generate_seed_range(
            template,
            start_seed,
            seed_step,
            count,
            playthrough,
            0,
            timeout_ms,
        )
    }

    fn finalize_stage_records_batch(
        &mut self,
        source_records: &[Vec<u8>],
        reveal: bool,
        timeout_ms: u32,
    ) -> Result<Vec<Vec<u8>>, RuntimeError> {
        self.0
            .finalize_stage_records_batch(source_records, reveal, timeout_ms)
    }

    fn remote_call_pending(&self) -> bool {
        self.0.remote_call_pending()
    }
}

/// A deterministic oracle for the gates.
///
/// Built only for tests and for the off-by-default `test-fake` build the
/// cross-language gate compiles, so the packaged host can never select it.
#[cfg(any(test, feature = "test-fake"))]
pub mod scripted {
    use std::collections::BTreeMap;

    use super::*;

    /// One scripted seed: the generator's stage-one output and the completion
    /// pass result for the same record.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScriptedRow {
        pub stage_one: Vec<u8>,
        pub completed: Vec<u8>,
    }

    /// Answers every batch call from a fixed table keyed by displayed seed.
    pub struct ScriptedOracle {
        rows: BTreeMap<u32, ScriptedRow>,
        template: Vec<u8>,
        rarity: u8,
        level: u16,
        recommended_level: u16,
        transfer_count: u32,
        max_batch_size: usize,
        pending: bool,
        /// Every call, in order, so a gate can assert the loop's exact batching.
        pub calls: Vec<OracleCall>,
    }

    /// One recorded oracle call.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OracleCall {
        Generate(Vec<u32>),
        GenerateSeedRange {
            start_seed: u32,
            seed_step: u32,
            count: u32,
            playthrough: Option<u32>,
        },
        Finalize(Vec<u32>),
    }

    impl ScriptedOracle {
        /// A script with no rows still answers: unscripted seeds are produced by
        /// the same pure emitter the product uses, so a scan can run over a
        /// range without a game.
        pub fn new(
            rows: Vec<(u32, Vec<u8>, Vec<u8>)>,
            template: Vec<u8>,
            rarity: u8,
            level: u16,
            recommended_level: u16,
            transfer_count: u32,
        ) -> Self {
            let rows = rows
                .into_iter()
                .map(|(seed, stage_one, completed)| {
                    (
                        seed,
                        ScriptedRow {
                            stage_one,
                            completed,
                        },
                    )
                })
                .collect();
            Self {
                rows,
                template,
                rarity,
                level,
                recommended_level,
                transfer_count,
                max_batch_size: ORACLE_BATCH_SIZE,
                pending: false,
                calls: Vec::new(),
            }
        }

        pub fn set_max_batch_size(&mut self, max_batch_size: usize) {
            self.max_batch_size = max_batch_size;
        }

        /// Make the oracle report an outstanding native call, so the host's
        /// retired-ownership rule can be exercised.
        pub fn set_remote_call_pending(&mut self, pending: bool) {
            self.pending = pending;
        }

        fn row(&self, seed: u32) -> ScriptedRow {
            if let Some(row) = self.rows.get(&seed) {
                return row.clone();
            }
            let record = nioh3_runtime::mutation::oracle::source_record(
                &self.template,
                seed,
                self.rarity,
                self.level,
                self.recommended_level,
                self.transfer_count,
            )
            .unwrap_or_default();
            ScriptedRow {
                completed: record.clone(),
                stage_one: record,
            }
        }
    }

    /// Load a scripted oracle from the JSON the gates hand the test-fake build.
    ///
    /// The file carries the template the loops must generate from plus one row
    /// per seed, so the scan's batching, ordering and completion calls are all
    /// observable without a game. It is read only under `test-fake`.
    pub fn load_script(path: &std::path::Path) -> Result<ScriptedOracle, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read the oracle script: {error}"))?;
        let payload: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("the oracle script is not valid JSON: {error}"))?;
        let hex = |name: &str| -> Result<Vec<u8>, String> {
            let value = payload
                .get(name)
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| format!("the oracle script needs a {name} string"))?;
            decode_hex(value)
        };
        let template = hex("template_hex")?;
        let number = |name: &str, default: u64| -> u64 {
            payload
                .get(name)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(default)
        };
        let rarity = u8::try_from(number("rarity", 4))
            .map_err(|_| "rarity does not fit in uint8".to_string())?;
        let level = u16::try_from(number("level", 180))
            .map_err(|_| "level does not fit in uint16".to_string())?;
        let recommended_level = u16::try_from(number("recommended_level", 183))
            .map_err(|_| "recommended_level does not fit in uint16".to_string())?;
        let transfer_count = u32::try_from(number("transfer_count", 0))
            .map_err(|_| "transfer_count does not fit in uint32".to_string())?;
        let mut rows = Vec::new();
        if let Some(entries) = payload.get("rows").and_then(serde_json::Value::as_array) {
            for entry in entries {
                let seed = entry
                    .get("seed")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| "every oracle row needs a seed".to_string())?;
                let stage_one = entry
                    .get("stage_one_hex")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "every oracle row needs stage_one_hex".to_string())?;
                let completed = entry
                    .get("completed_hex")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(stage_one);
                rows.push((
                    u32::try_from(seed).map_err(|_| "seed does not fit in uint32".to_string())?,
                    decode_hex(stage_one)?,
                    decode_hex(completed)?,
                ));
            }
        }
        let mut oracle = ScriptedOracle::new(
            rows,
            template,
            rarity,
            level,
            recommended_level,
            transfer_count,
        );
        // A gate may make the oracle report an outstanding native call, so the
        // host's retired-ownership rule can be exercised without a real thread.
        if payload
            .get("remote_call_pending")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            oracle.set_remote_call_pending(true);
        }
        Ok(oracle)
    }

    fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
        if !text.len().is_multiple_of(2) {
            return Err("a hex field has an odd length".to_string());
        }
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(text.len() / 2);
        let mut index = 0;
        while index < bytes.len() {
            let high = (bytes[index] as char)
                .to_digit(16)
                .ok_or_else(|| "a hex field has a non-hex digit".to_string())?;
            let low = (bytes[index + 1] as char)
                .to_digit(16)
                .ok_or_else(|| "a hex field has a non-hex digit".to_string())?;
            out.push((high * 16 + low) as u8);
            index += 2;
        }
        Ok(out)
    }

    fn seed_of(record: &[u8]) -> u32 {
        if record.len() < 0x24 {
            return 0;
        }
        u32::from_le_bytes([record[0x20], record[0x21], record[0x22], record[0x23]])
    }

    impl BatchOracle for ScriptedOracle {
        fn max_batch_size(&self) -> usize {
            self.max_batch_size
        }

        fn generate(
            &mut self,
            source_records: &[Vec<u8>],
            _timeout_ms: u32,
        ) -> Result<Vec<Vec<u8>>, RuntimeError> {
            let seeds: Vec<u32> = source_records
                .iter()
                .map(|record| seed_of(record))
                .collect();
            self.calls.push(OracleCall::Generate(seeds.clone()));
            Ok(seeds
                .into_iter()
                .map(|seed| self.row(seed).stage_one)
                .collect())
        }

        fn generate_seed_range(
            &mut self,
            _template: &[u8],
            start_seed: u32,
            seed_step: u32,
            count: u32,
            playthrough: Option<u32>,
            _timeout_ms: u32,
        ) -> Result<Vec<Vec<u8>>, RuntimeError> {
            self.calls.push(OracleCall::GenerateSeedRange {
                start_seed,
                seed_step,
                count,
                playthrough,
            });
            let mut records = Vec::with_capacity(count as usize);
            for index in 0..count {
                let seed = start_seed.wrapping_add(index.wrapping_mul(seed_step));
                records.push(self.row(seed).stage_one);
            }
            Ok(records)
        }

        fn finalize_stage_records_batch(
            &mut self,
            source_records: &[Vec<u8>],
            _reveal: bool,
            _timeout_ms: u32,
        ) -> Result<Vec<Vec<u8>>, RuntimeError> {
            let seeds: Vec<u32> = source_records
                .iter()
                .map(|record| seed_of(record))
                .collect();
            self.calls.push(OracleCall::Finalize(seeds.clone()));
            Ok(seeds
                .into_iter()
                .map(|seed| self.row(seed).completed)
                .collect())
        }

        fn remote_call_pending(&self) -> bool {
            self.pending
        }
    }
}
