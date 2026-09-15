//! Fail-closed problems raised while reading an untrusted save blob.

use std::fmt;

/// A save-read failure. Every variant is raised before data is exposed, so a
/// caller that receives `Ok` can index the blob without further checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveReadError {
    /// The blob is not exactly [`crate::layout::USER_SAVE_BYTES`] long.
    SaveLength { expected: usize, actual: usize },
    /// The blob does not start with the decrypted-user-save magic.
    SaveMagic,
    /// The blob is too small to contain the whole fixed inventory region.
    InventoryRegionTruncated { needed: usize, actual: usize },
    /// A slot index outside `0..SCROLL_SLOT_COUNT`.
    SlotIndex { index: usize },
    /// The record selected for a template/unmapped read is empty.
    EmptyRecord { slot_index: usize },
    /// The requested playthrough has no authentic or synthesizable template.
    TemplateUnavailable { playthrough: u8 },
    /// A playthrough outside `1..=5`.
    Playthrough { playthrough: u8 },
    /// The declared record type is not a mapped scroll category.
    UnmappedRecord { slot_index: usize, record_type: u16 },
    /// The path does not end in `SAVEDATA??`.
    SlotDirectory { path: String },
    /// The directory above `SAVEDATA??` is not a numeric Steam account id.
    AccountDirectory { path: String },
    /// Save discovery was pointed at something that is not a readable directory.
    DiscoveryRoot { path: String },
    /// An encrypted container whose length matches neither shipped container.
    ContainerLength { actual: usize },
    /// Filesystem trouble while reading or writing a save or a journal record.
    Io { path: String, message: String },
    /// The save no longer matches the identity an operation was prepared for.
    SaveChanged { path: String },
    /// A digest check did not match, so the write is refused.
    IntegrityMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    /// A plan, backup or operation identifier this host does not own.
    UnknownIdentifier { kind: &'static str, value: String },
    /// A commit finished in a state that cannot be proven either way.
    CommitUncertain { message: String },
    /// `decrypt_container` was handed bytes that are already clear.
    ContainerAlreadyClear,
    /// `encrypt_container` was handed bytes that are already a container.
    ContainerAlreadyEncrypted,
    /// A user container decrypted to a buffer without the user-save magic.
    DecryptedMagic { actual: Vec<u8> },
    /// A record-level field access was rejected by the domain codec.
    Record(nioh3_domain::record::RecordError),
    /// A field window would run past the end of the buffer it was read from.
    FieldOutOfRange { offset: usize, width: usize },
    /// A write-side helper was handed a buffer that is not one record long.
    RecordLength { expected: usize, actual: usize },
    /// A write-side helper was handed a record whose type word is zero.
    RecordTypeZero,
    /// An insert-style write targeted a slot that still holds bytes.
    SlotNotZeroed { slot_index: usize },
    /// A free-slot-only write targeted an occupied slot.
    SlotOccupied { slot_index: usize },
    /// An allocation has no value left in its namespace.
    AllocationExhausted { kind: &'static str },
    /// An allocation produced (or was handed) a value outside its valid range.
    AllocationValue { field: &'static str },
    /// A product transform was handed inputs the shipped reference refuses.
    InvalidTransform { message: String },
    /// A stored plan or receipt failed validation as untrusted input.
    TamperedRecord { kind: &'static str, message: String },
    /// A transaction step was refused because the plan's target does not match.
    PlanTargetMismatch { expected: String, actual: String },
    /// The fault gate injected a failure at a named commit stage.
    InjectedFault { stage: String },
    /// A batch install found colliding record serials it must not rewrite.
    AppendOnlyRepairRequired,
    /// A batch install has no contiguous run long enough for its records.
    InsufficientContiguousSlots { first_slot: usize, needed: usize },
}

impl fmt::Display for SaveReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SaveLength { expected, actual } => write!(
                formatter,
                "expected a {expected:#x}-byte decrypted user save, got {actual:#x}"
            ),
            Self::SaveMagic => {
                write!(formatter, "decrypted user save must start with RNNUSR")
            }
            Self::InventoryRegionTruncated { needed, actual } => write!(
                formatter,
                "save is too small for the fixed inventory region: need {needed:#x} bytes, have {actual:#x}"
            ),
            Self::SlotIndex { index } => {
                write!(formatter, "slot index {index} is outside the scroll inventory")
            }
            Self::EmptyRecord { slot_index } => {
                write!(formatter, "slot {slot_index} holds no record")
            }
            Self::TemplateUnavailable { playthrough } => write!(
                formatter,
                "no authentic scroll template is available for playthrough {playthrough}"
            ),
            Self::Playthrough { playthrough } => {
                write!(formatter, "playthrough {playthrough} is outside 1..=5")
            }
            Self::UnmappedRecord {
                slot_index,
                record_type,
            } => write!(
                formatter,
                "slot {slot_index} has record type {record_type:#06X}, which is not a mapped scroll"
            ),
            Self::SlotDirectory { path } => {
                write!(formatter, "{path} is not a SAVEDATA?? directory entry")
            }
            Self::AccountDirectory { path } => {
                write!(formatter, "{path} has no numeric Steam account directory")
            }
            Self::DiscoveryRoot { path } => {
                write!(formatter, "{path} is not a readable save root directory")
            }
            Self::ContainerLength { actual } => write!(
                formatter,
                "encrypted save has {actual:#x} bytes, which is not a shipped container size"
            ),
            Self::Io { path, message } => write!(formatter, "{path}: {message}"),
            Self::SaveChanged { path } => write!(
                formatter,
                "{path} changed after the operation was prepared; refresh before committing"
            ),
            Self::IntegrityMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "{path} digest mismatch: expected {expected}, computed {actual}"
            ),
            Self::UnknownIdentifier { kind, value } => {
                write!(formatter, "unknown {kind} {value}")
            }
            Self::CommitUncertain { message } => write!(
                formatter,
                "the commit finished in an unprovable state: {message}; query the receipt before retrying"
            ),
            Self::ContainerAlreadyClear => {
                write!(formatter, "the buffer is already decrypted, not an encrypted container")
            }
            Self::ContainerAlreadyEncrypted => write!(
                formatter,
                "the buffer is already an encrypted container, not a decrypted save"
            ),
            Self::DecryptedMagic { actual } => write!(
                formatter,
                "a user container must decrypt to a buffer starting with RNNUSR; got {}",
                actual
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ),
            Self::Record(error) => write!(formatter, "{error:?}"),
            Self::FieldOutOfRange { offset, width } => {
                write!(formatter, "field window {offset:#x}+{width} lies outside the buffer")
            }
            Self::RecordLength { expected, actual } => write!(
                formatter,
                "expected a {expected:#x}-byte scroll record, got {actual:#x}"
            ),
            Self::RecordTypeZero => {
                write!(formatter, "a written scroll record must have a nonzero type word")
            }
            Self::SlotNotZeroed { slot_index } => write!(
                formatter,
                "slot {slot_index} is not fully zeroed, so it cannot receive an inserted record"
            ),
            Self::SlotOccupied { slot_index } => {
                write!(formatter, "slot {slot_index} is occupied")
            }
            Self::AllocationExhausted { kind } => {
                write!(formatter, "no unused {kind} remains in this save")
            }
            Self::AllocationValue { field } => {
                write!(formatter, "the {field} is outside its supported range")
            }
            Self::InvalidTransform { message } => {
                write!(formatter, "the product transform was refused: {message}")
            }
            Self::TamperedRecord { kind, message } => {
                write!(formatter, "the stored {kind} failed validation: {message}")
            }
            Self::PlanTargetMismatch { expected, actual } => write!(
                formatter,
                "the plan targets {expected}, but the host was asked to write {actual}"
            ),
            Self::InjectedFault { stage } => {
                write!(formatter, "an injected fault fired at commit stage {stage}")
            }
            Self::AppendOnlyRepairRequired => write!(
                formatter,
                "APPEND_ONLY_REPAIR_REQUIRED: existing scroll generation serials collide; \
                 adding scrolls must not silently rewrite owned records"
            ),
            Self::InsufficientContiguousSlots { first_slot, needed } => write!(
                formatter,
                "there is no contiguous run of {needed} free scroll slots starting at {first_slot}"
            ),
        }
    }
}

impl std::error::Error for SaveReadError {}

impl From<nioh3_domain::record::RecordError> for SaveReadError {
    fn from(error: nioh3_domain::record::RecordError) -> Self {
        Self::Record(error)
    }
}
