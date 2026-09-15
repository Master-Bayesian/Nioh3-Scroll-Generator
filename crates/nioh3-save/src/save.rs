//! Validated view over one decrypted user save.

use sha2::{Digest, Sha256};

use crate::crypto;
use crate::error::SaveReadError;
use crate::layout::{
    slot_offset, SCROLL_RECORD_BYTES, SCROLL_REGION_END, USER_SAVE_BYTES, USER_SAVE_MAGIC,
};

/// A decrypted user-save blob that passed the shipped size and magic checks.
///
/// Construction is the only way to obtain a reference to the bytes, so every
/// later index is provably inside the buffer.
#[derive(Debug, Clone)]
pub struct DecryptedSave {
    bytes: Vec<u8>,
}

impl DecryptedSave {
    /// Decrypt one encrypted container and validate the result.
    ///
    /// This is the shipped read path: the container is decoded with the audited
    /// AES-128 primitive and the shipped format rules, then handed to
    /// [`Self::new`] so the same size and magic checks apply to encrypted input.
    pub fn from_container(bytes: &[u8]) -> Result<Self, SaveReadError> {
        Self::new(crypto::decrypt_container(bytes)?)
    }

    /// Validate a decrypted user save that was produced by the shipped
    /// encrypt/decrypt component.
    pub fn new(bytes: Vec<u8>) -> Result<Self, SaveReadError> {
        let actual = bytes.len();
        if actual != USER_SAVE_BYTES {
            return Err(SaveReadError::SaveLength {
                expected: USER_SAVE_BYTES,
                actual,
            });
        }
        if !bytes.starts_with(USER_SAVE_MAGIC) {
            return Err(SaveReadError::SaveMagic);
        }
        if bytes.len() < SCROLL_REGION_END {
            return Err(SaveReadError::InventoryRegionTruncated {
                needed: SCROLL_REGION_END,
                actual: bytes.len(),
            });
        }
        Ok(Self { bytes })
    }

    /// Whole validated blob.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// SHA-256 of the blob, lowercase hex, matching the shipped integrity check.
    pub fn sha256(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(&self.bytes);
        format!("{:x}", digest.finalize())
    }

    /// One raw `0xE8` record by slot index.
    ///
    /// The returned window is always exactly `SCROLL_RECORD_BYTES` long: the
    /// slot index is range-checked and construction already proved the region
    /// is present.
    #[allow(clippy::indexing_slicing)]
    pub fn raw_record(&self, slot_index: usize) -> Result<&[u8], SaveReadError> {
        let offset =
            slot_offset(slot_index).ok_or(SaveReadError::SlotIndex { index: slot_index })?;
        Ok(&self.bytes[offset..offset + SCROLL_RECORD_BYTES])
    }
}

/// SHA-256 of a byte slice in the same lowercase-hex form as [`DecryptedSave::sha256`].
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}
