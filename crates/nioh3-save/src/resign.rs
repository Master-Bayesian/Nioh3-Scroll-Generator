//! Re-sign a user save to another Steam account (存档改签).
//!
//! A decrypted user save names its account in two places: the header's 64-bit
//! Steam id at `+0x10`, and the owner id inside each scroll record the account
//! created (`+0x02`/`+0x04`/`+0x14`, see `inventory::account_id_from_record`).
//! Scrolls another player shared keep their own owner. Equipment and item
//! records carry no account id. The checksum is rewritten and the container
//! re-encrypted; nothing else changes.

use nioh3_domain::record::ScrollRecordBytes;

use crate::codec::patch_user_checksum;
use crate::crypto::{decrypt_container, encrypt_container};
use crate::error::SaveReadError;
use crate::inventory::{account_id_from_record, rebind_account_id};
use crate::layout::{slot_offset, SCROLL_RECORD_BYTES, SCROLL_SLOT_COUNT, USER_SAVE_BYTES};

/// Offset of the header's Steam account id in a decrypted user save.
pub const HEADER_ACCOUNT_OFFSET: usize = 0x10;

/// What a re-sign changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resigned {
    /// The re-signed, encrypted container.
    pub container: Vec<u8>,
    /// The account the source save was signed to.
    pub from_account: u64,
    /// Scroll records whose owner moved to the new account.
    pub rebound_scrolls: usize,
}

/// The account a decrypted user save is signed to.
pub fn header_account(decrypted: &[u8]) -> Result<u64, SaveReadError> {
    let bytes = decrypted
        .get(HEADER_ACCOUNT_OFFSET..HEADER_ACCOUNT_OFFSET + 8)
        .ok_or(SaveReadError::ContainerLength {
            actual: decrypted.len(),
        })?;
    let mut word = [0u8; 8];
    word.copy_from_slice(bytes);
    Ok(u64::from_le_bytes(word))
}

/// Re-sign an encrypted user save container to `to_account`.
pub fn resign_user_save(container: &[u8], to_account: u64) -> Result<Resigned, SaveReadError> {
    let mut clear = decrypt_container(container)?;
    if clear.len() != USER_SAVE_BYTES {
        return Err(SaveReadError::ContainerLength {
            actual: clear.len(),
        });
    }
    let from_account = header_account(&clear)?;
    clear[HEADER_ACCOUNT_OFFSET..HEADER_ACCOUNT_OFFSET + 8]
        .copy_from_slice(&to_account.to_le_bytes());
    let mut rebound_scrolls = 0;
    if from_account != to_account {
        for slot in 0..SCROLL_SLOT_COUNT {
            let Some(offset) = slot_offset(slot) else {
                continue;
            };
            let window = &clear[offset..offset + SCROLL_RECORD_BYTES];
            let record = ScrollRecordBytes::from_slice(window).map_err(|_| {
                SaveReadError::ContainerLength {
                    actual: clear.len(),
                }
            })?;
            if from_account == 0 || account_id_from_record(&record) != from_account {
                continue;
            }
            let rebound = rebind_account_id(&record, to_account);
            clear[offset..offset + SCROLL_RECORD_BYTES].copy_from_slice(rebound.as_bytes());
            rebound_scrolls += 1;
        }
    }
    patch_user_checksum(&mut clear)?;
    Ok(Resigned {
        container: encrypt_container(&clear)?,
        from_account,
        rebound_scrolls,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::layout::USER_SAVE_MAGIC;

    const OLD: u64 = 76_561_198_000_000_001;
    const NEW: u64 = 76_561_198_999_999_999;
    const FRIEND: u64 = 76_561_198_555_555_555;

    fn write_owner(clear: &mut [u8], slot: usize, account: u64) {
        let offset = slot_offset(slot).unwrap();
        clear[offset + 0x02..offset + 0x04]
            .copy_from_slice(&((account >> 48) as u16).to_le_bytes());
        clear[offset + 0x04..offset + 0x06]
            .copy_from_slice(&((account >> 32) as u16).to_le_bytes());
        clear[offset + 0x14..offset + 0x18]
            .copy_from_slice(&((account & 0xFFFF_FFFF) as u32).to_le_bytes());
    }
    fn owner(clear: &[u8], slot: usize) -> u64 {
        let offset = slot_offset(slot).unwrap();
        account_id_from_record(
            &ScrollRecordBytes::from_slice(&clear[offset..offset + SCROLL_RECORD_BYTES]).unwrap(),
        )
    }

    #[test]
    fn header_and_own_scrolls_move_shared_scrolls_stay() {
        let mut clear = vec![0u8; USER_SAVE_BYTES];
        clear[..USER_SAVE_MAGIC.len()].copy_from_slice(USER_SAVE_MAGIC);
        clear[HEADER_ACCOUNT_OFFSET..HEADER_ACCOUNT_OFFSET + 8].copy_from_slice(&OLD.to_le_bytes());
        write_owner(&mut clear, 0, OLD);
        write_owner(&mut clear, 1, FRIEND);
        write_owner(&mut clear, 7, OLD);
        let resigned = resign_user_save(&encrypt_container(&clear).unwrap(), NEW).unwrap();
        assert_eq!(resigned.from_account, OLD);
        assert_eq!(resigned.rebound_scrolls, 2);
        let after = decrypt_container(&resigned.container).unwrap();
        assert_eq!(header_account(&after).unwrap(), NEW);
        assert_eq!(owner(&after, 0), NEW);
        assert_eq!(owner(&after, 1), FRIEND);
        assert_eq!(owner(&after, 7), NEW);
        assert_eq!(owner(&after, 2), 0, "empty slots stay empty");
        // Outside the header, owner fields and checksum, the bytes are unchanged.
        let mut expected = clear.clone();
        expected[HEADER_ACCOUNT_OFFSET..HEADER_ACCOUNT_OFFSET + 8]
            .copy_from_slice(&NEW.to_le_bytes());
        write_owner(&mut expected, 0, NEW);
        write_owner(&mut expected, 7, NEW);
        patch_user_checksum(&mut expected).unwrap();
        assert_eq!(after, expected);
    }

    #[test]
    fn same_account_only_refreshes_the_checksum() {
        let mut clear = vec![0u8; USER_SAVE_BYTES];
        clear[..USER_SAVE_MAGIC.len()].copy_from_slice(USER_SAVE_MAGIC);
        clear[HEADER_ACCOUNT_OFFSET..HEADER_ACCOUNT_OFFSET + 8].copy_from_slice(&OLD.to_le_bytes());
        write_owner(&mut clear, 0, OLD);
        let resigned = resign_user_save(&encrypt_container(&clear).unwrap(), OLD).unwrap();
        assert_eq!(resigned.rebound_scrolls, 0);
        assert_eq!(
            owner(&decrypt_container(&resigned.container).unwrap(), 0),
            OLD
        );
    }
}
