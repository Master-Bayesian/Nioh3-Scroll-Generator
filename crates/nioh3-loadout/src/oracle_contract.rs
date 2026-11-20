//! Replay provenance is separate from successful execution. Committing a page
//! does not turn its unobserved zero-filled neighbors into captured game bytes.
use std::collections::BTreeMap;

const CHUNK: u64 = 0x10000;

#[derive(Default)]
pub struct SnapshotKnowledge {
    chunks: BTreeMap<u64, Vec<u8>>,
}

impl SnapshotKnowledge {
    pub fn set_byte(&mut self, address: u64, known: bool) {
        let mask = self
            .chunks
            .entry(address & !(CHUNK - 1))
            .or_insert_with(|| vec![0; CHUNK as usize / 8]);
        let offset = (address & (CHUNK - 1)) as usize;
        let bit = 1 << (offset & 7);
        if known {
            mask[offset / 8] |= bit;
        } else {
            mask[offset / 8] &= !bit;
        }
    }

    pub fn declare_bytes(&mut self, start: u64, len: usize) {
        for offset in 0..len {
            self.set_byte(start + offset as u64, true);
        }
    }

    pub fn is_known(&self, address: u64) -> bool {
        let offset = (address & (CHUNK - 1)) as usize;
        self.chunks
            .get(&(address & !(CHUNK - 1)))
            .is_some_and(|m| m[offset / 8] & (1 << (offset & 7)) != 0)
    }

    pub fn require_range(&self, start: u64, len: usize) -> Result<(), u64> {
        for offset in 0..len {
            let address = start.checked_add(offset as u64).ok_or(u64::MAX)?;
            if !self.is_known(address) {
                return Err(address);
            }
        }
        Ok(())
    }

    pub fn encode(&self, chunks: &[u64]) -> Vec<u8> {
        let zero = vec![0; CHUNK as usize / 8];
        let mut result = Vec::with_capacity(chunks.len() * (8 + zero.len()));
        for &chunk in chunks {
            result.extend_from_slice(&chunk.to_le_bytes());
            result.extend_from_slice(self.chunks.get(&chunk).unwrap_or(&zero));
        }
        result
    }
}

/// Instruction identity and data/thread provenance are independent axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OracleEnvironment {
    pub tls_guard_captured: bool,
    pub tls_epoch_captured: bool,
    pub native_reads_checked: bool,
    pub source_admission_derived: bool,
}

impl OracleEnvironment {
    pub fn independently_grounded(self) -> bool {
        self.tls_guard_captured
            && self.tls_epoch_captured
            && self.native_reads_checked
            && self.source_admission_derived
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_allocated_neighbor_does_not_become_known() {
        let mut k = SnapshotKnowledge::default();
        k.declare_bytes(0x4BA66B0, 4);
        assert_eq!(k.require_range(0x4BA66A0, 4), Err(0x4BA66A0));
        assert!(k.require_range(0x4BA66B0, 4).is_ok());
        k.set_byte(0x4BA66B2, false);
        assert_eq!(k.require_range(0x4BA66B0, 4), Err(0x4BA66B2));
        let mask = k.encode(&[0x4BA0000]);
        assert_eq!(mask.len(), 8 + 8192);
        assert_eq!(mask[8 + 0x66B0 / 8] & 15, 11);
    }

    #[test]
    fn removing_instruction_cuts_is_insufficient_evidence() {
        for missing in 0..4 {
            let mut e = OracleEnvironment {
                tls_guard_captured: true,
                tls_epoch_captured: true,
                native_reads_checked: true,
                source_admission_derived: true,
            };
            assert!(e.independently_grounded());
            match missing {
                0 => e.tls_guard_captured = false,
                1 => e.tls_epoch_captured = false,
                2 => e.native_reads_checked = false,
                _ => e.source_admission_derived = false,
            }
            assert!(!e.independently_grounded());
        }
    }
}
