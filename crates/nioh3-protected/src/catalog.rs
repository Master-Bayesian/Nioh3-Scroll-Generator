//! Pure read-only local-name catalog import.
//!
//! The handler accepts the bytes selected by the renderer, never a path. It
//! supports the one source that is an actual user-facing product artifact at
//! this stage (`items_little_endian.json`, represented by
//! `save_active_items`). CT/XML/7z and research-intermediate trainer dumps are
//! intentionally rejected rather than presented as product import formats.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use nioh3_data::equipment_catalog::{
    load_catalog_set_from_bytes, CatalogBytesInput, CatalogError, CatalogRole, LocalNameCatalog,
    MAX_CATALOG_BYTES,
};
use serde_json::Value;

use crate::error::HostError;

const MAX_SOURCE_LABEL_CHARS: usize = 256;
const MAX_DECLARED_VERSION_CHARS: usize = 128;
const MAX_LOCALE_CHARS: usize = 16;
const SUPPORTED_LOCALES: [&str; 3] = ["zh-CN", "en-US", "ja-JP"];

fn required_text<'a>(params: &'a Value, key: &str) -> Result<&'a str, HostError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| HostError::rejected(format!("CATALOG_INVALID_REQUEST: `{key}` is required")))
}

fn validate_label(value: &str, key: &str, max_chars: usize) -> Result<(), HostError> {
    let count = value.chars().count();
    if count == 0 || count > max_chars || value.chars().any(char::is_control) {
        return Err(HostError::rejected(format!(
            "CATALOG_INVALID_REQUEST: `{key}` must be 1..={max_chars} printable characters"
        )));
    }
    Ok(())
}

fn decode_content(encoded: &str) -> Result<Vec<u8>, HostError> {
    // Decode and re-encode to reject whitespace, URL-safe alphabets, and
    // non-canonical trailing bits. The digest then covers exactly these bytes.
    let bytes = STANDARD.decode(encoded).map_err(|error| {
        HostError::rejected(format!("CATALOG_INVALID_CONTENT: base64: {error}"))
    })?;
    if STANDARD.encode(&bytes) != encoded {
        return Err(HostError::rejected(
            "CATALOG_INVALID_CONTENT: content_base64 is not canonical standard base64",
        ));
    }
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(HostError::rejected(format!(
            "CATALOG_INPUT_TOO_LARGE: decoded content is {} bytes (limit {})",
            bytes.len(),
            MAX_CATALOG_BYTES
        )));
    }
    Ok(bytes)
}

fn catalog_error(error: CatalogError) -> HostError {
    HostError::rejected(format!("CATALOG_IMPORT_REJECTED: {error}"))
}

/// Handle `catalog.import_names` without save, game, network, or path access.
pub fn import_names(params: &Value) -> Result<Value, HostError> {
    let role = required_text(params, "role")?;
    if role != CatalogRole::SaveActiveItems.as_str() {
        return Err(HostError::rejected(format!(
            "CATALOG_ROLE_UNSUPPORTED: `{role}` is not a supported user-file catalog; only `save_active_items` / items_little_endian.json is accepted"
        )));
    }
    let source_label = required_text(params, "source_label")?;
    validate_label(source_label, "source_label", MAX_SOURCE_LABEL_CHARS)?;
    let declared_version = required_text(params, "declared_version")?;
    validate_label(
        declared_version,
        "declared_version",
        MAX_DECLARED_VERSION_CHARS,
    )?;
    let locale = required_text(params, "locale")?;
    if locale.chars().count() > MAX_LOCALE_CHARS || !SUPPORTED_LOCALES.contains(&locale) {
        return Err(HostError::rejected(
            "CATALOG_INVALID_REQUEST: locale must be zh-CN, en-US, or ja-JP",
        ));
    }
    let encoded = required_text(params, "content_base64")?;
    let bytes = decode_content(encoded)?;
    let input = CatalogBytesInput::new(
        CatalogRole::SaveActiveItems,
        source_label,
        declared_version,
        locale,
        bytes,
    );
    let set = load_catalog_set_from_bytes(&input).map_err(catalog_error)?;
    let result = LocalNameCatalog::from_catalog_set(&set, locale).map_err(catalog_error)?;
    Ok(result.to_json())
}
