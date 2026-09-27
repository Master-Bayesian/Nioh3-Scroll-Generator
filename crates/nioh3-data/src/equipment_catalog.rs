//! Offline, read-only normalization of the v0.8.1 equipment catalog artifacts.
//!
//! The adapter turns already-collected research artifacts into rows that always
//! carry their source identity, declared version, id namespace and original
//! text. It never infers legality, a type, or a maximum where the source
//! declares null, never repairs a malformed key from a neighbouring column, and
//! never drops a row: unusable rows are quarantined and sentinel rows are
//! flagged so a report can name them.
//!
//! Two save-side storage conventions coexist and must not be confused:
//!
//! * a save item key is the on-disk byte pair written as text (`EB9E`), so the
//!   numeric id is the little-endian read of those bytes (`0x9EEB`) and the
//!   written bytes round-trip unchanged;
//! * a save effect key is the numeric id written as text (`E99A`), so the
//!   on-disk form is the little-endian encoding of that number (`9A E9`).
//!
//! CT dumps carry raw byte arrays whose declared width is preserved: a
//! four-byte token keeps its full 32-bit value and its high word, and only an
//! explicitly zero high word may be compared with a two-byte namespace.
//! Trainer catalogs are numeric namespaces and are never merged with the save
//! or CT namespaces.
//!
//! Provenance is not compatibility. Rows here record where an id came from and
//! what it is called in that source; nothing in this module declares an id
//! legal, obtainable, bounded, or applicable to the current game version, and
//! nothing here writes a file, spawns a process, reads a game save, or reaches
//! a live process.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    path::PathBuf,
};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Report/format tag for the rows this adapter produces.
pub const EQUIPMENT_CATALOG_FORMAT: &str = "nioh3-equipment-catalog-adapter-v1";
/// Product-facing local-name response format.
pub const LOCAL_NAME_CATALOG_FORMAT: &str = "nioh3-local-name-catalog-v1";
/// The largest user-selected catalog accepted by the protected import route.
pub const MAX_CATALOG_BYTES: usize = 2 * 1024 * 1024;
/// The largest row set accepted by the protected import route.
pub const MAX_CATALOG_ROWS: usize = 10_000;
/// The largest display name accepted by the protected import route.
pub const MAX_CATALOG_NAME_CHARS: usize = 512;
/// The response deliberately does not make a version or legality claim.
pub const LOCAL_NAME_CATALOG_CLAIM_BOUNDARY: &str =
    "source declaration and byte-exact name observations only; no current-version compatibility, legality, obtainability, type, maximum, or write behavior is established";

/// The decoration run a CT dump uses for its section and set banner rows.
const CT_BANNER_RUN: &str = "———";

/// One declared catalog input and the role it plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogInput {
    pub role: CatalogRole,
    pub path: PathBuf,
    pub declared_version: String,
    /// Optional expected digest; a mismatch rejects the whole load.
    pub expected_sha256: Option<String>,
}

impl CatalogInput {
    pub fn new(
        role: CatalogRole,
        path: impl Into<PathBuf>,
        declared_version: impl Into<String>,
    ) -> Self {
        Self {
            role,
            path: path.into(),
            declared_version: declared_version.into(),
            expected_sha256: None,
        }
    }

    pub fn with_expected_sha256(mut self, sha256: impl Into<String>) -> Self {
        self.expected_sha256 = Some(sha256.into());
        self
    }
}

/// One user-selected catalog held in memory.
///
/// The protected import path intentionally accepts bytes, not a filesystem
/// path. The caller can therefore bind the reported SHA-256 to exactly the
/// bytes selected by the user without granting the backend a path-read
/// capability or persisting the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogBytesInput {
    pub role: CatalogRole,
    pub source_label: String,
    pub declared_version: String,
    pub locale: String,
    pub bytes: Vec<u8>,
}

impl CatalogBytesInput {
    pub fn new(
        role: CatalogRole,
        source_label: impl Into<String>,
        declared_version: impl Into<String>,
        locale: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            role,
            source_label: source_label.into(),
            declared_version: declared_version.into(),
            locale: locale.into(),
            bytes,
        }
    }
}

/// What a declared input is, which fixes both its parser and its namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CatalogRole {
    TrainerEquipment,
    TrainerEffectVariants,
    TrainerHellSkills,
    SaveActiveItems,
    SaveActiveEffects,
    SaveInactiveItems,
    SaveInactiveEffects,
    CtEquipment,
    CtEffectRaw,
    CtEffectSorted,
}

impl CatalogRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrainerEquipment => "trainer_equipment",
            Self::TrainerEffectVariants => "trainer_effect_variants",
            Self::TrainerHellSkills => "trainer_hell_skills",
            Self::SaveActiveItems => "save_active_items",
            Self::SaveActiveEffects => "save_active_effects",
            Self::SaveInactiveItems => "save_inactive_items",
            Self::SaveInactiveEffects => "save_inactive_effects",
            Self::CtEquipment => "ct_equipment",
            Self::CtEffectRaw => "ct_effect_raw",
            Self::CtEffectSorted => "ct_effect_sorted",
        }
    }

    pub fn namespace(self) -> IdNamespace {
        match self {
            Self::SaveActiveItems | Self::SaveInactiveItems => IdNamespace::SaveItemU16LeBytes,
            Self::SaveActiveEffects | Self::SaveInactiveEffects => {
                IdNamespace::SaveEffectU16Numeric
            }
            Self::TrainerEquipment | Self::TrainerEffectVariants | Self::TrainerHellSkills => {
                IdNamespace::TrainerNumeric
            }
            Self::CtEquipment | Self::CtEffectRaw | Self::CtEffectSorted => IdNamespace::CtRawBytes,
        }
    }

    fn shape(self) -> InputShape {
        match self {
            Self::SaveActiveItems | Self::SaveInactiveItems => InputShape::ItemObject,
            Self::SaveActiveEffects | Self::SaveInactiveEffects => InputShape::EffectArray,
            Self::TrainerEquipment => InputShape::TrainerEquipmentArray,
            Self::TrainerEffectVariants => InputShape::TrainerEffectArray,
            Self::TrainerHellSkills => InputShape::TrainerHellSkillArray,
            Self::CtEquipment | Self::CtEffectRaw | Self::CtEffectSorted => InputShape::CtDump,
        }
    }
}

/// What a normalized number means. Two rows are comparable only inside one
/// namespace, and a CT token additionally needs an explicitly zero high word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdNamespace {
    /// Save item key: the on-disk bytes as text, read little-endian.
    SaveItemU16LeBytes,
    /// Save effect key: the numeric id as text.
    SaveEffectU16Numeric,
    /// CT dump token: raw bytes with the declared width preserved.
    CtRawBytes,
    /// Trainer catalog number.
    TrainerNumeric,
}

impl IdNamespace {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SaveItemU16LeBytes => "save_item_u16_le_bytes",
            Self::SaveEffectU16Numeric => "save_effect_u16_numeric",
            Self::CtRawBytes => "ct_raw_bytes",
            Self::TrainerNumeric => "trainer_numeric",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputShape {
    ItemObject,
    EffectArray,
    TrainerEquipmentArray,
    TrainerEffectArray,
    TrainerHellSkillArray,
    CtDump,
}

/// Why one row could not be normalized. The row itself is always retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineReason {
    UnparsableKey { detail: String },
    UnsupportedWidth { width: usize },
    OutOfRange { value: i64 },
}

impl QuarantineReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnparsableKey { .. } => "unparsable_key",
            Self::UnsupportedWidth { .. } => "unsupported_width",
            Self::OutOfRange { .. } => "out_of_range",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::UnparsableKey { detail } => detail.clone(),
            Self::UnsupportedWidth { width } => format!("token width {width} bytes"),
            Self::OutOfRange { value } => {
                format!("declared value {value} is outside 0..=65535")
            }
        }
    }
}

/// The reserved-token meanings a CT dump uses for its own empty rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SentinelKind {
    ZeroToken,
    AllOnesToken,
}

impl SentinelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ZeroToken => "sentinel_zero",
            Self::AllOnesToken => "sentinel_all_ones",
        }
    }
}

/// Whether a row carries an id, is a reserved sentinel, or is quarantined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowState {
    Accepted,
    Sentinel { kind: SentinelKind },
    Quarantined { reason: QuarantineReason },
}

impl RowState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Sentinel { kind } => kind.as_str(),
            Self::Quarantined { reason } => reason.as_str(),
        }
    }
}

/// A decoded key. `id` keeps the full declared width; `high_word` is `None`
/// when the source has no high word (a two-byte token) and `Some(0)` when the
/// source declared four bytes whose upper half is explicitly zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedKey {
    pub namespace: IdNamespace,
    pub key_bytes: Vec<u8>,
    pub id: u32,
    pub high_word: Option<u16>,
}

impl NormalizedKey {
    /// The only value a cross-width comparison may use.
    ///
    /// A two-byte token is comparable; a four-byte token is comparable only
    /// when its high word is explicitly zero.
    pub fn comparable_u16(&self) -> Option<u16> {
        match self.high_word {
            None => Some(self.id as u16),
            Some(0) => Some(self.id as u16),
            Some(_) => None,
        }
    }
}

/// The outcome of normalizing one declared key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOutcome {
    Key(NormalizedKey),
    Sentinel {
        kind: SentinelKind,
        key_bytes: Vec<u8>,
    },
    Quarantined(QuarantineReason),
}

impl KeyOutcome {
    pub fn quarantined(reason: QuarantineReason) -> Self {
        Self::Quarantined(reason)
    }
}

/// One normalized catalog row. Every row keeps its input index, original key
/// text and every declared source column, including the ones declared null.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRow {
    pub input_index: usize,
    pub role: CatalogRole,
    pub namespace: IdNamespace,
    pub row_index: usize,
    pub raw_key: String,
    pub key_bytes: Vec<u8>,
    pub id: Option<u32>,
    pub high_word: Option<u16>,
    /// A second declared identity in the same row (trainer `base_id`). It is
    /// never a substitute for `id`.
    pub secondary_id: Option<u32>,
    pub name: Option<String>,
    /// Verbatim optional source columns keyed by their exact source name. A
    /// `None` value means the source declared null; nothing is defaulted.
    pub declared_columns: BTreeMap<String, Option<String>>,
    pub state: RowState,
}

impl CatalogRow {
    /// The comparable u16 of an accepted row, honouring the high-word rule.
    pub fn comparable_u16(&self) -> Option<u16> {
        if !self.is_accepted() {
            return None;
        }
        match self.high_word {
            Some(0) | None => self.id.map(|id| id as u16),
            Some(_) => None,
        }
    }

    pub fn is_accepted(&self) -> bool {
        matches!(self.state, RowState::Accepted)
    }
}

/// Identity of one consumed input, captured from the same bytes that produced
/// its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub role: CatalogRole,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub declared_version: String,
}

/// Every failure mode that rejects a load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    Oversize {
        role: CatalogRole,
        path: String,
        limit: usize,
        actual: usize,
        unit: &'static str,
    },
    Unavailable {
        role: CatalogRole,
        path: String,
        detail: String,
    },
    IdentityMismatch {
        role: CatalogRole,
        path: String,
        expected: String,
        actual: String,
    },
    DuplicateRole {
        role: CatalogRole,
    },
    Malformed {
        role: CatalogRole,
        path: String,
        detail: String,
    },
    EmptyAfterNormalization {
        role: CatalogRole,
        path: String,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversize {
                role,
                path,
                limit,
                actual,
                unit,
            } => write!(
                f,
                "catalog input too large for {}: {path} ({actual} {unit}, limit {limit})",
                role.as_str()
            ),
            Self::Unavailable { role, path, detail } => write!(
                f,
                "catalog input unavailable for {}: {path} ({detail})",
                role.as_str()
            ),
            Self::IdentityMismatch {
                role,
                path,
                expected,
                actual,
            } => write!(
                f,
                "catalog identity mismatch for {}: {path} expected {expected}, found {actual}",
                role.as_str()
            ),
            Self::DuplicateRole { role } => write!(
                f,
                "catalog role {} was declared more than once",
                role.as_str()
            ),
            Self::Malformed { role, path, detail } => write!(
                f,
                "catalog input malformed for {}: {path} ({detail})",
                role.as_str()
            ),
            Self::EmptyAfterNormalization { role, path } => write!(
                f,
                "catalog input for {} normalized to zero accepted rows: {path}",
                role.as_str()
            ),
        }
    }
}

impl Error for CatalogError {}

impl CatalogError {
    /// Stable machine-readable kind for a report.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Oversize { .. } => "oversize",
            Self::Unavailable { .. } => "unavailable",
            Self::IdentityMismatch { .. } => "identity_mismatch",
            Self::DuplicateRole { .. } => "duplicate_role",
            Self::Malformed { .. } => "malformed",
            Self::EmptyAfterNormalization { .. } => "empty_after_normalization",
        }
    }

    pub fn role(&self) -> CatalogRole {
        match self {
            Self::Oversize { role, .. }
            | Self::Unavailable { role, .. }
            | Self::IdentityMismatch { role, .. }
            | Self::DuplicateRole { role }
            | Self::Malformed { role, .. }
            | Self::EmptyAfterNormalization { role, .. } => *role,
        }
    }

    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Oversize { path, .. }
            | Self::Unavailable { path, .. }
            | Self::IdentityMismatch { path, .. }
            | Self::Malformed { path, .. }
            | Self::EmptyAfterNormalization { path, .. } => Some(path),
            Self::DuplicateRole { .. } => None,
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::Oversize {
                limit,
                actual,
                unit,
                ..
            } => format!("{actual} {unit} exceeds the limit of {limit}"),
            Self::Unavailable { detail, .. } | Self::Malformed { detail, .. } => detail.clone(),
            Self::IdentityMismatch {
                expected, actual, ..
            } => format!("expected {expected}, found {actual}"),
            Self::DuplicateRole { .. } => "role declared more than once".to_string(),
            Self::EmptyAfterNormalization { .. } => "normalized to zero accepted rows".to_string(),
        }
    }
}

/// A loaded catalog set plus the identity of every input that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSet {
    inputs: Vec<SourceIdentity>,
    rows: Vec<CatalogRow>,
    source_lines: BTreeMap<CatalogRole, usize>,
    banner_rows: BTreeMap<CatalogRole, usize>,
}

impl CatalogSet {
    pub fn inputs(&self) -> &[SourceIdentity] {
        &self.inputs
    }

    pub fn rows(&self) -> &[CatalogRow] {
        &self.rows
    }

    pub fn rows_for(&self, role: CatalogRole) -> impl Iterator<Item = &CatalogRow> {
        self.rows.iter().filter(move |row| row.role == role)
    }

    pub fn quarantined(&self) -> impl Iterator<Item = &CatalogRow> {
        self.rows
            .iter()
            .filter(|row| matches!(row.state, RowState::Quarantined { .. }))
    }

    pub fn sentinels(&self) -> impl Iterator<Item = &CatalogRow> {
        self.rows
            .iter()
            .filter(|row| matches!(row.state, RowState::Sentinel { .. }))
    }

    /// Distinct comparable ids of one role. Sentinels, quarantined rows and
    /// four-byte tokens with a non-zero high word are excluded, so no
    /// cross-namespace merge can happen through this function.
    pub fn ids(&self, role: CatalogRole) -> BTreeSet<u16> {
        let mut ids = BTreeSet::new();
        for row in self.rows_for(role) {
            if let Some(id) = row.comparable_u16() {
                ids.insert(id);
            }
        }
        ids
    }

    /// Source lines the parse consumed for one role, when the input was a dump.
    pub fn source_lines(&self, role: CatalogRole) -> usize {
        *self.source_lines.get(&role).unwrap_or(&0)
    }

    /// Banner rows the parse skipped for one role, when the input was a dump.
    pub fn banner_rows(&self, role: CatalogRole) -> usize {
        *self.banner_rows.get(&role).unwrap_or(&0)
    }
}

/// Source metadata carried by a product-facing local-name result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNameCatalogSource {
    pub role: String,
    pub namespace: String,
    pub source_label: String,
    pub declared_version: String,
    pub locale: String,
    pub bytes: u64,
    pub sha256: String,
}

/// One source row, including rows that cannot safely label a runtime id.
///
/// `id` preserves the normalized source value when one exists. `display_id`
/// is present only when the adapter's exact namespace rules allow a u16 display
/// comparison. A null `display_id`, a sentinel, a quarantined row, or a
/// conflicted id is never a display name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNameCatalogRow {
    pub input_index: usize,
    pub row_index: usize,
    pub source_role: String,
    pub namespace: String,
    pub raw_key: String,
    pub key_bytes_hex: String,
    pub id: Option<u32>,
    pub display_id: Option<u16>,
    pub high_word: Option<u16>,
    pub name: Option<String>,
    pub state: String,
    pub quarantine_reason: Option<String>,
    pub displayable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNameCatalogCounts {
    pub input_rows: usize,
    pub accepted_rows: usize,
    pub display_rows: usize,
    pub sentinel_rows: usize,
    pub quarantined_rows: usize,
    pub conflict_rows: usize,
    pub conflict_ids: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNameCatalogConflict {
    pub id: u16,
    pub namespace: String,
    pub names: Vec<String>,
    pub row_indices: Vec<usize>,
}

/// Typed read-only local-name import result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNameCatalog {
    pub schema: String,
    pub status: String,
    pub source: LocalNameCatalogSource,
    pub rows: Vec<LocalNameCatalogRow>,
    pub counts: LocalNameCatalogCounts,
    pub warnings: Vec<String>,
    pub conflicts: Vec<LocalNameCatalogConflict>,
    pub claim_boundary: String,
}

impl LocalNameCatalog {
    /// Build a display-safe result from one or more already-normalized rows.
    ///
    /// The protected import route supplies exactly one input. Keeping this
    /// constructor on the data adapter makes it impossible for a handler to
    /// reimplement byte order, high-word, sentinel, or quarantine semantics.
    pub fn from_catalog_set(
        set: &CatalogSet,
        locale: impl Into<String>,
    ) -> Result<Self, CatalogError> {
        let identity = set
            .inputs()
            .first()
            .ok_or_else(|| CatalogError::Malformed {
                role: CatalogRole::SaveActiveItems,
                path: String::new(),
                detail: "catalog contains no source identity".to_string(),
            })?;
        if set.inputs().len() != 1 {
            return Err(CatalogError::Malformed {
                role: identity.role,
                path: identity.path.clone(),
                detail: "local-name response requires exactly one source input".to_string(),
            });
        }

        let mut by_id: BTreeMap<u16, BTreeMap<String, Vec<usize>>> = BTreeMap::new();
        for row in set.rows() {
            if let (Some(id), Some(name)) = (row.comparable_u16(), row.name.as_deref()) {
                if !name.is_empty() {
                    by_id
                        .entry(id)
                        .or_default()
                        .entry(name.to_string())
                        .or_default()
                        .push(row.row_index);
                }
            }
        }

        let mut conflicts = Vec::new();
        let mut conflict_ids = BTreeSet::new();
        for (id, names) in &by_id {
            if names.len() > 1 {
                conflict_ids.insert(*id);
                let mut row_indices = names
                    .values()
                    .flat_map(|indices| indices.iter().copied())
                    .collect::<Vec<_>>();
                row_indices.sort_unstable();
                conflicts.push(LocalNameCatalogConflict {
                    id: *id,
                    namespace: identity.role.namespace().as_str().to_string(),
                    names: names.keys().cloned().collect(),
                    row_indices,
                });
            }
        }

        let mut rows = Vec::with_capacity(set.rows().len());
        let mut warnings = vec![LOCAL_NAME_CATALOG_CLAIM_BOUNDARY.to_string()];
        let mut accepted_rows = 0usize;
        let mut display_rows = 0usize;
        let mut sentinel_rows = 0usize;
        let mut quarantined_rows = 0usize;
        let mut conflict_rows = 0usize;
        for row in set.rows() {
            let display_id = row.comparable_u16();
            let name = row.name.clone();
            let nonempty_name = name.as_deref().is_some_and(|value| !value.is_empty());
            let displayable = row.is_accepted()
                && nonempty_name
                && display_id.is_some_and(|id| id != 0)
                && !display_id.is_some_and(|id| conflict_ids.contains(&id));
            if row.is_accepted() {
                accepted_rows += 1;
            }
            match &row.state {
                RowState::Sentinel { .. } => sentinel_rows += 1,
                RowState::Quarantined { .. } => quarantined_rows += 1,
                RowState::Accepted => {}
            }
            if display_id.is_some_and(|id| conflict_ids.contains(&id)) {
                conflict_rows += 1;
            }
            if displayable {
                display_rows += 1;
            }
            rows.push(LocalNameCatalogRow {
                input_index: row.input_index,
                row_index: row.row_index,
                source_role: row.role.as_str().to_string(),
                namespace: row.namespace.as_str().to_string(),
                raw_key: row.raw_key.clone(),
                key_bytes_hex: hex_bytes(&row.key_bytes),
                id: row.id,
                display_id,
                high_word: row.high_word,
                name,
                state: row.state.as_str().to_string(),
                quarantine_reason: match &row.state {
                    RowState::Quarantined { reason } => Some(reason.detail()),
                    _ => None,
                },
                displayable,
            });
        }
        if sentinel_rows > 0 {
            warnings.push("sentinel rows are retained for provenance but never label an empty or unknown runtime row".to_string());
        }
        if quarantined_rows > 0 {
            warnings
                .push("malformed keys are quarantined and never label a runtime row".to_string());
        }
        if conflict_rows > 0 {
            warnings.push("conflicting names for one display id are excluded from display rows; review conflicts explicitly".to_string());
        }
        if rows
            .iter()
            .any(|row| row.state == "accepted" && row.name.is_none())
        {
            warnings
                .push("accepted rows without a name are retained but not displayable".to_string());
        }
        if rows.iter().any(|row| row.display_id == Some(0)) {
            warnings.push(
                "display id 0 is retained as source data but cannot label an unknown runtime row"
                    .to_string(),
            );
        }

        Ok(Self {
            schema: LOCAL_NAME_CATALOG_FORMAT.to_string(),
            status: "ok".to_string(),
            source: LocalNameCatalogSource {
                role: identity.role.as_str().to_string(),
                namespace: identity.role.namespace().as_str().to_string(),
                source_label: identity.path.clone(),
                declared_version: identity.declared_version.clone(),
                locale: locale.into(),
                bytes: identity.bytes,
                sha256: identity.sha256.clone(),
            },
            counts: LocalNameCatalogCounts {
                input_rows: rows.len(),
                accepted_rows,
                display_rows,
                sentinel_rows,
                quarantined_rows,
                conflict_rows,
                conflict_ids: conflicts.len(),
            },
            rows,
            warnings,
            conflicts,
            claim_boundary: LOCAL_NAME_CATALOG_CLAIM_BOUNDARY.to_string(),
        })
    }

    /// JSON wire projection used by the protected response contract.
    pub fn to_json(&self) -> Value {
        json!({
            "schema": self.schema,
            "status": self.status,
            "source": {
                "role": self.source.role,
                "namespace": self.source.namespace,
                "source_label": self.source.source_label,
                "declared_version": self.source.declared_version,
                "locale": self.source.locale,
                "bytes": self.source.bytes,
                "sha256": self.source.sha256,
            },
            "rows": self.rows.iter().map(|row| json!({
                "input_index": row.input_index,
                "row_index": row.row_index,
                "source_role": row.source_role,
                "namespace": row.namespace,
                "raw_key": row.raw_key,
                "key_bytes_hex": row.key_bytes_hex,
                "id": row.id,
                "display_id": row.display_id,
                "high_word": row.high_word,
                "name": row.name,
                "state": row.state,
                "quarantine_reason": row.quarantine_reason,
                "displayable": row.displayable,
            })).collect::<Vec<_>>(),
            "counts": {
                "input_rows": self.counts.input_rows,
                "accepted_rows": self.counts.accepted_rows,
                "display_rows": self.counts.display_rows,
                "sentinel_rows": self.counts.sentinel_rows,
                "quarantined_rows": self.counts.quarantined_rows,
                "conflict_rows": self.counts.conflict_rows,
                "conflict_ids": self.counts.conflict_ids,
            },
            "warnings": self.warnings,
            "conflicts": self.conflicts.iter().map(|conflict| json!({
                "id": conflict.id,
                "namespace": conflict.namespace,
                "names": conflict.names,
                "row_indices": conflict.row_indices,
            })).collect::<Vec<_>>(),
            "claim_boundary": self.claim_boundary,
        })
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

/// Normalize one save item key. The text is the on-disk byte order.
pub fn normalize_save_item_key(raw: &str) -> KeyOutcome {
    let bytes = match parse_hex_bytes(raw, 2) {
        Ok(bytes) => bytes,
        Err(reason) => return KeyOutcome::quarantined(reason),
    };
    KeyOutcome::Key(NormalizedKey {
        namespace: IdNamespace::SaveItemU16LeBytes,
        id: u32::from(u16::from_le_bytes([bytes[0], bytes[1]])),
        key_bytes: bytes,
        high_word: None,
    })
}

/// Normalize one save effect key. The text is the numeric id.
pub fn normalize_save_effect_key(raw: &str) -> KeyOutcome {
    if raw.len() != 4 || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return KeyOutcome::quarantined(QuarantineReason::UnparsableKey {
            detail: format!("effect key `{raw}` is not a four-digit hexadecimal id"),
        });
    }
    let id = u16::from_str_radix(raw, 16).expect("four hexadecimal digits");
    KeyOutcome::Key(NormalizedKey {
        namespace: IdNamespace::SaveEffectU16Numeric,
        key_bytes: id.to_le_bytes().to_vec(),
        id: u32::from(id),
        high_word: None,
    })
}

/// Normalize one CT dump token, preserving the declared width.
pub fn normalize_ct_token(raw: &str) -> KeyOutcome {
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    if tokens.is_empty() {
        return KeyOutcome::quarantined(QuarantineReason::UnparsableKey {
            detail: "empty token".to_string(),
        });
    }
    let mut bytes = Vec::with_capacity(tokens.len());
    for token in &tokens {
        if token.len() != 2 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
            return KeyOutcome::quarantined(QuarantineReason::UnparsableKey {
                detail: format!("token `{token}` is not one hexadecimal byte"),
            });
        }
        bytes.push(u8::from_str_radix(token, 16).expect("two hexadecimal digits"));
    }
    // The width is only a width problem once every byte is well formed; a
    // mangled separator is a key-format problem, not a wide token.
    if bytes.len() != 2 && bytes.len() != 4 {
        return KeyOutcome::quarantined(QuarantineReason::UnsupportedWidth { width: bytes.len() });
    }
    if bytes.iter().all(|byte| *byte == 0) {
        return KeyOutcome::Sentinel {
            kind: SentinelKind::ZeroToken,
            key_bytes: bytes,
        };
    }
    if bytes.iter().all(|byte| *byte == 0xFF) {
        return KeyOutcome::Sentinel {
            kind: SentinelKind::AllOnesToken,
            key_bytes: bytes,
        };
    }
    let mut id: u32 = 0;
    for (index, byte) in bytes.iter().enumerate() {
        id |= u32::from(*byte) << (8 * index);
    }
    let high_word = (bytes.len() == 4).then_some((id >> 16) as u16);
    KeyOutcome::Key(NormalizedKey {
        namespace: IdNamespace::CtRawBytes,
        key_bytes: bytes,
        id,
        high_word,
    })
}

/// Normalize one trainer catalog number.
pub fn normalize_trainer_id(value: i64) -> KeyOutcome {
    if !(0..=i64::from(u16::MAX)).contains(&value) {
        return KeyOutcome::quarantined(QuarantineReason::OutOfRange { value });
    }
    KeyOutcome::Key(NormalizedKey {
        namespace: IdNamespace::TrainerNumeric,
        key_bytes: (value as u16).to_le_bytes().to_vec(),
        id: value as u32,
        high_word: None,
    })
}

fn parse_hex_bytes(raw: &str, expected: usize) -> Result<Vec<u8>, QuarantineReason> {
    let expected_chars = expected * 2;
    if raw.len() != expected_chars || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(QuarantineReason::UnparsableKey {
            detail: format!(
                "key `{raw}` is not {expected_chars} hexadecimal digits (declared width {expected} bytes)"
            ),
        });
    }
    let mut bytes = Vec::with_capacity(expected);
    for index in 0..expected {
        let slice = &raw[index * 2..index * 2 + 2];
        bytes.push(u8::from_str_radix(slice, 16).expect("two hexadecimal digits"));
    }
    Ok(bytes)
}

fn sha256_hex_upper(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

type ParsedInput = (Vec<RowFields>, Option<usize>, Option<usize>);

/// One row before it is bound to its input index and role.
struct RowFields {
    row_index: usize,
    raw_key: String,
    key_bytes: Vec<u8>,
    id: Option<u32>,
    high_word: Option<u16>,
    secondary_id: Option<u32>,
    name: Option<String>,
    declared_columns: BTreeMap<String, Option<String>>,
    state: RowState,
}

fn fields_from_outcome(
    row_index: usize,
    raw_key: &str,
    name: Option<String>,
    declared_columns: BTreeMap<String, Option<String>>,
    outcome: KeyOutcome,
    secondary_id: Option<u32>,
) -> RowFields {
    match outcome {
        KeyOutcome::Key(key) => RowFields {
            row_index,
            raw_key: raw_key.to_string(),
            key_bytes: key.key_bytes,
            id: Some(key.id),
            high_word: key.high_word,
            secondary_id,
            name,
            declared_columns,
            state: RowState::Accepted,
        },
        KeyOutcome::Sentinel { kind, key_bytes } => RowFields {
            row_index,
            raw_key: raw_key.to_string(),
            key_bytes,
            id: None,
            high_word: None,
            secondary_id,
            name,
            declared_columns,
            state: RowState::Sentinel { kind },
        },
        KeyOutcome::Quarantined(reason) => RowFields {
            row_index,
            raw_key: raw_key.to_string(),
            key_bytes: Vec::new(),
            id: None,
            high_word: None,
            secondary_id,
            name,
            declared_columns,
            state: RowState::Quarantined { reason },
        },
    }
}

/// Read every declared input and normalize it.
///
/// Any failure rejects the whole load: a catalog that is not fully identified
/// is reported as unavailable, malformed, or empty after normalization, never
/// as an implicitly "unrelated" set.
pub fn load_catalog_set(inputs: &[CatalogInput]) -> Result<CatalogSet, CatalogError> {
    let mut seen: BTreeSet<CatalogRole> = BTreeSet::new();
    for input in inputs {
        if !seen.insert(input.role) {
            return Err(CatalogError::DuplicateRole { role: input.role });
        }
    }

    let mut identities = Vec::with_capacity(inputs.len());
    let mut rows = Vec::new();
    let mut source_lines = BTreeMap::new();
    let mut banner_rows = BTreeMap::new();

    for (input_index, input) in inputs.iter().enumerate() {
        let display_path = input.path.display().to_string();
        let bytes = fs::read(&input.path).map_err(|error| CatalogError::Unavailable {
            role: input.role,
            path: display_path.clone(),
            detail: error.to_string(),
        })?;
        let sha256 = sha256_hex_upper(&bytes);
        if let Some(expected) = &input.expected_sha256 {
            if !expected.eq_ignore_ascii_case(&sha256) {
                return Err(CatalogError::IdentityMismatch {
                    role: input.role,
                    path: display_path,
                    expected: expected.to_uppercase(),
                    actual: sha256,
                });
            }
        }
        identities.push(SourceIdentity {
            role: input.role,
            path: display_path.clone(),
            bytes: bytes.len() as u64,
            sha256,
            declared_version: input.declared_version.clone(),
        });

        let (parsed, lines, banners) = parse_input(input.role, &display_path, &bytes)?;
        for row in parsed {
            rows.push(CatalogRow {
                input_index,
                role: input.role,
                namespace: input.role.namespace(),
                row_index: row.row_index,
                raw_key: row.raw_key,
                key_bytes: row.key_bytes,
                id: row.id,
                high_word: row.high_word,
                secondary_id: row.secondary_id,
                name: row.name,
                declared_columns: row.declared_columns,
                state: row.state,
            });
        }
        if let Some(lines) = lines {
            source_lines.insert(input.role, lines);
        }
        if let Some(banners) = banners {
            banner_rows.insert(input.role, banners);
        }
    }

    let set = CatalogSet {
        inputs: identities,
        rows,
        source_lines,
        banner_rows,
    };

    for input in inputs {
        let accepted = set
            .rows_for(input.role)
            .filter(|row| row.is_accepted())
            .count();
        if accepted == 0 {
            return Err(CatalogError::EmptyAfterNormalization {
                role: input.role,
                path: input.path.display().to_string(),
            });
        }
    }

    Ok(set)
}

/// Parse one user-selected source from an in-memory byte buffer.
///
/// This is deliberately a thin adapter over [`parse_input`]: it does not add
/// a second key parser or guess a namespace from a file extension. The source
/// label is provenance only and is never opened as a path.
pub fn load_catalog_set_from_bytes(input: &CatalogBytesInput) -> Result<CatalogSet, CatalogError> {
    if input.bytes.len() > MAX_CATALOG_BYTES {
        return Err(CatalogError::Oversize {
            role: input.role,
            path: input.source_label.clone(),
            limit: MAX_CATALOG_BYTES,
            actual: input.bytes.len(),
            unit: "bytes",
        });
    }
    if input.source_label.trim().is_empty() {
        return Err(CatalogError::Malformed {
            role: input.role,
            path: input.source_label.clone(),
            detail: "source label must not be empty".to_string(),
        });
    }

    let sha256 = sha256_hex_upper(&input.bytes);
    let (parsed, lines, banners) = parse_input(input.role, &input.source_label, &input.bytes)?;
    if parsed.len() > MAX_CATALOG_ROWS {
        return Err(CatalogError::Oversize {
            role: input.role,
            path: input.source_label.clone(),
            limit: MAX_CATALOG_ROWS,
            actual: parsed.len(),
            unit: "rows",
        });
    }
    let rows: Vec<CatalogRow> = parsed
        .into_iter()
        .map(|row| CatalogRow {
            input_index: 0,
            role: input.role,
            namespace: input.role.namespace(),
            row_index: row.row_index,
            raw_key: row.raw_key,
            key_bytes: row.key_bytes,
            id: row.id,
            high_word: row.high_word,
            secondary_id: row.secondary_id,
            name: row.name,
            declared_columns: row.declared_columns,
            state: row.state,
        })
        .collect();
    if rows.iter().all(|row| !row.is_accepted()) {
        return Err(CatalogError::EmptyAfterNormalization {
            role: input.role,
            path: input.source_label.clone(),
        });
    }

    Ok(CatalogSet {
        inputs: vec![SourceIdentity {
            role: input.role,
            path: input.source_label.clone(),
            bytes: input.bytes.len() as u64,
            sha256,
            declared_version: input.declared_version.clone(),
        }],
        rows,
        source_lines: lines.into_iter().map(|value| (input.role, value)).collect(),
        banner_rows: banners
            .into_iter()
            .map(|value| (input.role, value))
            .collect(),
    })
}

fn parse_input(role: CatalogRole, path: &str, bytes: &[u8]) -> Result<ParsedInput, CatalogError> {
    match role.shape() {
        InputShape::CtDump => parse_ct_dump(role, path, bytes),
        InputShape::ItemObject => parse_item_object(role, path, bytes),
        InputShape::EffectArray => parse_effect_array(role, path, bytes),
        InputShape::TrainerEquipmentArray => parse_trainer_equipment(role, path, bytes),
        InputShape::TrainerEffectArray => parse_trainer_effects(role, path, bytes),
        InputShape::TrainerHellSkillArray => parse_trainer_hell_skills(role, path, bytes),
    }
}

fn malformed(role: CatalogRole, path: &str, detail: impl Into<String>) -> CatalogError {
    CatalogError::Malformed {
        role,
        path: path.to_string(),
        detail: detail.into(),
    }
}

fn parse_json(role: CatalogRole, path: &str, bytes: &[u8]) -> Result<Value, CatalogError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| malformed(role, path, format!("not valid UTF-8: {error}")))?;
    serde_json::from_str(text)
        .map_err(|error| malformed(role, path, format!("invalid JSON: {error}")))
}

fn json_text(value: Option<&Value>) -> Option<String> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(other) => Some(other.to_string()),
    }
}

fn columns(pairs: &[(&str, Option<String>)]) -> BTreeMap<String, Option<String>> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_string(), value.clone()))
        .collect()
}

fn parse_item_object(
    role: CatalogRole,
    path: &str,
    bytes: &[u8],
) -> Result<ParsedInput, CatalogError> {
    let value = parse_json(role, path, bytes)?;
    let map = value
        .as_object()
        .ok_or_else(|| malformed(role, path, "expected a JSON object keyed by item id"))?;
    if map.len() > MAX_CATALOG_ROWS {
        return Err(CatalogError::Oversize {
            role,
            path: path.to_string(),
            limit: MAX_CATALOG_ROWS,
            actual: map.len(),
            unit: "rows",
        });
    }
    let mut rows = Vec::with_capacity(map.len());
    for (row_index, (raw_key, entry)) in map.iter().enumerate() {
        let outcome = normalize_save_item_key(raw_key);
        let name = json_text(entry.get("name"));
        if let Some(name) = &name {
            if name.chars().count() > MAX_CATALOG_NAME_CHARS {
                return Err(CatalogError::Oversize {
                    role,
                    path: path.to_string(),
                    limit: MAX_CATALOG_NAME_CHARS,
                    actual: name.chars().count(),
                    unit: "name characters",
                });
            }
        }
        rows.push(fields_from_outcome(
            row_index,
            raw_key,
            name,
            columns(&[("type", json_text(entry.get("type")))]),
            outcome,
            None,
        ));
    }
    Ok((rows, None, None))
}

fn parse_effect_array(
    role: CatalogRole,
    path: &str,
    bytes: &[u8],
) -> Result<ParsedInput, CatalogError> {
    let value = parse_json(role, path, bytes)?;
    let array = value
        .as_array()
        .ok_or_else(|| malformed(role, path, "expected a JSON array of effect rows"))?;
    if array.len() > MAX_CATALOG_ROWS {
        return Err(CatalogError::Oversize {
            role,
            path: path.to_string(),
            limit: MAX_CATALOG_ROWS,
            actual: array.len(),
            unit: "rows",
        });
    }
    let mut rows = Vec::with_capacity(array.len());
    for (row_index, entry) in array.iter().enumerate() {
        let raw_key = json_text(entry.get("id")).unwrap_or_default();
        let outcome = normalize_save_effect_key(&raw_key);
        rows.push(fields_from_outcome(
            row_index,
            &raw_key,
            json_text(entry.get("Effect")),
            columns(&[
                ("type", json_text(entry.get("type"))),
                ("Effect Max", json_text(entry.get("Effect Max"))),
            ]),
            outcome,
            None,
        ));
    }
    Ok((rows, None, None))
}

fn parse_trainer_equipment(
    role: CatalogRole,
    path: &str,
    bytes: &[u8],
) -> Result<ParsedInput, CatalogError> {
    let value = parse_json(role, path, bytes)?;
    let array = value.as_array().ok_or_else(|| {
        malformed(
            role,
            path,
            "expected a JSON array of trainer equipment rows",
        )
    })?;
    if array.len() > MAX_CATALOG_ROWS {
        return Err(CatalogError::Oversize {
            role,
            path: path.to_string(),
            limit: MAX_CATALOG_ROWS,
            actual: array.len(),
            unit: "rows",
        });
    }
    let mut rows = Vec::with_capacity(array.len());
    for (row_index, entry) in array.iter().enumerate() {
        let outcome = match entry.get("id").and_then(Value::as_i64) {
            Some(value) => normalize_trainer_id(value),
            None => KeyOutcome::quarantined(QuarantineReason::UnparsableKey {
                detail: "trainer equipment row has no numeric `id`".to_string(),
            }),
        };
        rows.push(fields_from_outcome(
            row_index,
            &json_text(entry.get("id_hex")).unwrap_or_default(),
            json_text(entry.get("name")),
            columns(&[
                ("category", json_text(entry.get("category"))),
                (
                    "default_damage_source",
                    json_text(entry.get("default_damage_source")),
                ),
                (
                    "default_weight_source",
                    json_text(entry.get("default_weight_source")),
                ),
            ]),
            outcome,
            None,
        ));
    }
    Ok((rows, None, None))
}

fn parse_trainer_effects(
    role: CatalogRole,
    path: &str,
    bytes: &[u8],
) -> Result<ParsedInput, CatalogError> {
    let value = parse_json(role, path, bytes)?;
    let array = value
        .as_array()
        .ok_or_else(|| malformed(role, path, "expected a JSON array of trainer effect rows"))?;
    if array.len() > MAX_CATALOG_ROWS {
        return Err(CatalogError::Oversize {
            role,
            path: path.to_string(),
            limit: MAX_CATALOG_ROWS,
            actual: array.len(),
            unit: "rows",
        });
    }
    let mut rows = Vec::with_capacity(array.len());
    for (row_index, entry) in array.iter().enumerate() {
        let outcome = match entry.get("variant_id").and_then(Value::as_i64) {
            Some(value) => normalize_trainer_id(value),
            None => KeyOutcome::quarantined(QuarantineReason::UnparsableKey {
                detail: "trainer effect row has no numeric `variant_id`".to_string(),
            }),
        };
        // A base id outside the u16 range is not a variant id and is never
        // truncated; it simply stays out of the base set.
        let secondary = entry
            .get("base_id")
            .and_then(Value::as_i64)
            .filter(|value| (0..=i64::from(u16::MAX)).contains(value))
            .map(|value| value as u32);
        rows.push(fields_from_outcome(
            row_index,
            &json_text(entry.get("variant_id_hex")).unwrap_or_default(),
            json_text(entry.get("name")),
            columns(&[
                ("base_id", json_text(entry.get("base_id"))),
                ("type_id", json_text(entry.get("type_id"))),
                ("rarity_id", json_text(entry.get("rarity_id"))),
                ("rarity_name", json_text(entry.get("rarity_name"))),
                (
                    "secondary_effect_name",
                    json_text(entry.get("secondary_effect_name")),
                ),
                (
                    "default_value_ui_source",
                    json_text(entry.get("default_value_ui_source")),
                ),
            ]),
            outcome,
            secondary,
        ));
    }
    Ok((rows, None, None))
}

fn parse_trainer_hell_skills(
    role: CatalogRole,
    path: &str,
    bytes: &[u8],
) -> Result<ParsedInput, CatalogError> {
    let value = parse_json(role, path, bytes)?;
    let array = value
        .as_array()
        .ok_or_else(|| malformed(role, path, "expected a JSON array of hell-skill rows"))?;
    if array.len() > MAX_CATALOG_ROWS {
        return Err(CatalogError::Oversize {
            role,
            path: path.to_string(),
            limit: MAX_CATALOG_ROWS,
            actual: array.len(),
            unit: "rows",
        });
    }
    let mut rows = Vec::with_capacity(array.len());
    for (row_index, entry) in array.iter().enumerate() {
        let outcome = match entry.get("id").and_then(Value::as_i64) {
            Some(value) => normalize_trainer_id(value),
            None => KeyOutcome::quarantined(QuarantineReason::UnparsableKey {
                detail: "hell-skill row has no numeric `id`".to_string(),
            }),
        };
        rows.push(fields_from_outcome(
            row_index,
            &json_text(entry.get("id_hex")).unwrap_or_default(),
            json_text(entry.get("name")),
            columns(&[
                ("source_file", json_text(entry.get("source_file"))),
                ("source_row", json_text(entry.get("source_row"))),
            ]),
            outcome,
            None,
        ));
    }
    Ok((rows, None, None))
}

fn parse_ct_dump(role: CatalogRole, path: &str, bytes: &[u8]) -> Result<ParsedInput, CatalogError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| malformed(role, path, format!("not valid UTF-8: {error}")))?;
    let mut rows = Vec::new();
    let mut banners = 0usize;
    let mut lines = 0usize;
    let mut row_index = 0usize;
    for line in text.lines() {
        lines += 1;
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let Some(separator) = line.find(':') else {
            banners += 1;
            continue;
        };
        let raw_key = line[..separator].trim().to_string();
        let name = line[separator + 1..].to_string();
        if name.contains(CT_BANNER_RUN) {
            banners += 1;
            continue;
        }
        let outcome = normalize_ct_token(&raw_key);
        rows.push(fields_from_outcome(
            row_index,
            &raw_key,
            Some(name),
            BTreeMap::new(),
            outcome,
            None,
        ));
        row_index += 1;
    }
    Ok((rows, Some(lines), Some(banners)))
}
