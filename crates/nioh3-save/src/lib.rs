//! Bounded read-only model of a decrypted Nioh 3 user save.
//!
//! This crate owns only the read side of the shipped save model: the fixed
//! inventory region, the account identity carried by a mapped scroll record, the
//! template records and the save-discovery path rules. It never writes a save,
//! never launches the game and never decrypts a file. The shipped encrypt/decrypt
//! component stays the source of the decrypted blob, exactly as in the Python
//! product (`nioh3_scroll_editor/savegame.SaveCrypto`), and every byte handed to
//! this crate is validated before it is indexed.
//!
//! Field offsets mirror `emaki_exchange.py` and
//! `nioh3_scroll_editor/savegame.py` at the v0.7.5 baseline. Record-level field
//! access is delegated to `nioh3_domain::record::ScrollRecordBytes` so the two
//! sides cannot drift.

pub mod backup;
pub mod codec;
pub mod crypto;
pub mod error;
pub mod inventory;
pub mod layout;
pub mod nioh_cipher;
pub mod paths;
pub mod save;
pub mod transaction;
pub mod transform;

pub use backup::{
    backups_root, list_backup_entries, list_backups_for, move_backup_to_recycle_bin,
    read_backup_manifest, role_backup_file, write_backup_manifest, BackupEntry, BackupFileEntry,
    BackupManifest, BACKUP_MANIFEST_SCHEMA, SAVE_SCHEMA_PROFILE,
};
pub use codec::{
    allocate_scroll_generation_serials, allocate_scroll_inventory_keys,
    clear_native_free_scroll_slot, compute_user_checksum, insert_scroll_record,
    patch_user_checksum, prepare_candidate_for_install, scroll_slot_is_empty,
    write_scroll_generation_serial, write_scroll_inventory_key, InsertReport,
    SCROLL_GENERATION_SERIAL_MAX, SCROLL_INVENTORY_KEY_MAX,
};
pub use crypto::{
    classify_container, decrypt_container, encrypt_container, is_encrypted, ContainerKind,
    CONTAINER_HEADER_BYTES, USER_CONTAINER_BYTES,
};
pub use error::SaveReadError;
pub use inventory::{SaveInventory, ScrollInventoryEntry, TemplateRecord};
pub use layout::{
    CATEGORY_TO_TYPE, SCROLL_GROUP_OFFSET, SCROLL_RECORD_BYTES, SCROLL_SLOT_COUNT, USER_SAVE_BYTES,
    USER_SAVE_MAGIC,
};
pub use paths::{account_id_from_save_path, discover_save_paths, save_slot_index_from_path};
pub use save::{sha256_hex, DecryptedSave};
pub use transaction::{
    capture_related_fingerprints, related_save_paths, FileFingerprint, OperationReceipt,
    PlanCommand, PlanKind, ProductPlanData, SavePlan, SaveRole, SaveTransactionHost,
};
pub use transform::{
    has_duplicate_generation_serials, patch_local_scroll_header, patch_local_scroll_record,
    playthrough_of, read_checksum, read_local_effect_slots, EffectPatch, HeaderPatch,
    InstallRequest, LocalEffectSlot, PlannedWrite, SaveTransformHost, SlotEdit,
};
