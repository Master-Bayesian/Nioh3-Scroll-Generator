//! `search.catalog`: the context-bound scroll option catalog.
//!
//! Ports the shipped application layer that renders one catalog payload:
//! `catalog.py` (`native_effect_name`, `searchable_scroll_effect_definitions`,
//! the curated `BETA_EFFECTS` overlay), `catalog_application.py`
//! (`recommended_level_metadata`, `terrain_choices`, `auxiliary_catalog`),
//! `auxiliary_catalog.py` (`AuxiliaryNameCatalog`) and `presentation_strings.py`.
//!
//! Every option comes from the shipped product tables on each request; no
//! captured response is replayed. Two things stay literal because the reference
//! itself only has them as source constants: the curated `BETA_EFFECTS` overlay
//! and the rarity-4 final Grace ids.
//!
//! Ordering notes, each verified against the shipped catalogs and covered by
//! `tests/migration/test_application_worker_parity.py`:
//!
//! - `serde_json` keeps objects in key order, so file-order iteration is
//!   reproduced where it is observable. The one file-order loop in the catalog
//!   is the terrain display lookup, and each display key (`0x0024`, `0x0039`,
//!   `0x0058`, `0x039F`) matches exactly one named terrain row in all three
//!   locales, so the lookup is a total map. Rows are still visited in ascending
//!   numeric key order, which is the shipped file order.
//! - Each name catalog lists one entry per language, so the reference's "first
//!   locale sharing a language" scan resolves to the same locale whatever order
//!   the locales are visited in.
//! - No two special-rule families share a case-folded name, so the reference's
//!   stable sort by `name.casefold()` is fully determined by the sort key.
//!   `to_lowercase` stands in for `casefold`, which is the same mapping for the
//!   shipped names (checked in the parity gate).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nioh3_data::PreviewResources;
use nioh3_domain::auxiliary::{
    describe_special_rule, legal_special_rule_keys, SpecialRuleEntry, SpecialRuleTables,
};
use nioh3_domain::effect::{
    EffectResourceBytes, EffectTableIndex, NativeWeightContext, SCROLL_RECORD_TYPES,
};
use nioh3_domain::enemy::{EnemyError, RosterTables};
use nioh3_domain::roster::enemy_role_by_lookup_key;
use serde_json::{json, Value};

use crate::engine::EngineError;
use crate::recommended_level::RecommendedLevelCurve;

/// `catalog_application.auxiliary_catalog` is only ever asked for NG3.
pub const CATALOG_PLAYTHROUGH: u8 = 3;
/// `auxiliary_catalog.AUXILIARY_NAME_SCHEMA`.
const AUXILIARY_NAME_SCHEMA: &str = "nioh3-scroll-auxiliary-names/v1";
/// `auxiliary_catalog.DEFAULT_SPECIAL_RULE_ITEM_NAMES` schema.
const SPECIAL_RULE_ITEM_NAME_SCHEMA: &str = "nioh3-special-rule-item-names/v1";
/// `catalog._load_multilingual_effect_names` schema.
const EFFECT_LOCALIZATION_SCHEMA: &str = "nioh3-effect-localization-catalog/v1";
/// The locales the versioned request contract can ask for.
const SUPPORTED_LOCALES: [&str; 3] = ["en-US", "ja-JP", "zh-CN"];
/// `auxiliary_catalog.load_auxiliary_name_catalog`'s shipped fallback locale.
const AUXILIARY_NAME_FALLBACK: &str = "ja-JP";
/// `catalog._player_ready_effect_name`'s "this is not a player-ready name" set.
const UNRESOLVED_MARKERS: [&str; 5] = ["{}", "~BUFF~", "~DEBUFF~", "^09", "\u{fffd}"];
/// `catalog._preferred_effect_locale`'s Windows host-locale outcome: the
/// reference's host name never carries a shipped language token, so its lookup
/// falls through to the Chinese names.
const HOST_LOCALE_FALLBACK: &str = "zh-CN";
/// `catalog`'s fallback for a native effect with no usable name.
const UNKNOWN_EFFECT_NAME: &str = "未知词条";
/// The `name_text_id` whose rule displays its own key as the qualifier.
const SELF_QUALIFIER_TEXT_ID: &str = "0x034ba650";
/// `catalog.R4_FINAL_GRACE_IDS`: the measured rarity-4 slot-5 output codes.
///
/// A rarity-4 map lists a stage-one output code per bucket; only the ids that
/// are verified final Grace ids may be presented as selectable Graces.
pub(crate) const R4_FINAL_GRACE_IDS: [u32; 21] = [
    0x6553, 0xCE68, 0xBABD, 0xEEEA, 0x16E2, 0x4192, 0x47EC, 0x4FE4, 0xEB61, 0x23E5, 0x2AE6, 0x8CCC,
    0xB24F, 0x5012, 0x7BEA, 0x590C, 0x4FA3, 0xB1E9, 0xE8EB, 0x7ECE, 0x71F6,
];

/// `catalog.py` `BETA_EFFECTS`: the curated player-facing name overlay.
///
/// The names are used only where the reference uses them, as the fallback for a
/// native string that is missing or is an unresolved sentence template. In the
/// shipped tables exactly one reachable scroll effect needs it (`0xB82B`, whose
/// native zh-CN string is `^09~BUFF~{}^09~~`); the rest are carried verbatim so
/// the overlay stays faithful if the tables change.
#[rustfmt::skip]
const CURATED_EFFECT_NAMES: [(u16, &str); 40] = [
    (0x4647, "伤害反映（忍术威力）"),
    (0xA051, "对妖战术"),
    (0xA73D, "体力"),
    (0x190A, "阴阳术伤害"),
    (0x2B06, "咒之深奥"),
    (0xB613, "心之深奥"),
    (0xDFF0, "武之深奥"),
    (0x583B, "负面效果持续时间缩短"),
    (0xF9BE, "精髓并存（共通）"),
    (0xEA74, "精髓并存（忍者）"),
    (0x47BC, "合轴可继承稀有度"),
    (0x2EFC, "远距离伤害"),
    (0x9A3D, "强攻击精力消耗降低"),
    (0x6CE3, "不消耗使役符"),
    (0x6BEB, "近距离攻击精力伤害"),
    (0xEA53, "坚忍度"),
    (0x512D, "精髓并存武士"),
    (0x7499, "对人战术"),
    (0x3E7A, "精力恢复速度"),
    (0xB82B, "敌人精力耗尽时赋予受到伤害增加"),
    (0xD40A, "近距离攻击精力消耗降低"),
    (0x6E2B, "不消耗仙药"),
    (0xBC51, "精华槽增加量"),
    (0x3A8E, "武技精力伤害"),
    (0xCE1A, "武技伤害"),
    (0x3F41, "属性攻击伤害降低"),
    (0xAE5A, "技之深奥"),
    (0xA0A7, "近距离攻击打倒敌人时恢复体力"),
    (0xEF97, "速攻击伤害"),
    (0x28D1, "九十九化身持续时间延长"),
    (0x5CAC, "闪避动作精力消耗降低"),
    (0x1355, "灵力增加量"),
    (0xD411, "冲刺精力消耗降低"),
    (0x6AAF, "智之深奥"),
    (0xDAC2, "体之深奥"),
    (0x23E8, "刚之深奥"),
    (0xFBEE, "防御精力消耗降低"),
    (0x600F, "伤害反映（阴阳术术力）"),
    (0x8184, "装备品掉落率"),
    (0xDB20, "近距离攻击伤害"),
];

/// `presentation_strings.LABELS`, keyed by locale then label.
const LABELS: [(&str, [(&str, &str); 8]); 3] = [
    (
        "en-US",
        [
            ("none", "None"),
            ("no_terrain", "No terrain effects"),
            ("contains", "Contains {name}"),
            ("unknown_terrain", "Unknown terrain row {value}"),
            ("unknown_terrain_effect", "Unknown terrain effect {value}"),
            ("unknown_rule", "Unknown rule {value}"),
            ("unknown_enemy", "Unknown enemy {value}"),
            ("item", "Item {value}"),
        ],
    ),
    (
        "zh-CN",
        [
            ("none", "无"),
            ("no_terrain", "无地形效果"),
            ("contains", "包含{name}"),
            ("unknown_terrain", "未知地形行 {value}"),
            ("unknown_terrain_effect", "未知地形效果 {value}"),
            ("unknown_rule", "未知规则 {value}"),
            ("unknown_enemy", "未知敌人 {value}"),
            ("item", "道具 {value}"),
        ],
    ),
    (
        "ja-JP",
        [
            ("none", "なし"),
            ("no_terrain", "地形効果なし"),
            ("contains", "{name}を含む"),
            ("unknown_terrain", "不明な地形行 {value}"),
            ("unknown_terrain_effect", "不明な地形効果 {value}"),
            ("unknown_rule", "不明なルール {value}"),
            ("unknown_enemy", "不明な敵 {value}"),
            ("item", "アイテム {value}"),
        ],
    ),
];

/// `presentation_strings.label`. `name` and `value` fill the two placeholders
/// the shipped table uses; at most one of them is ever supplied per label.
fn render_label(locale: &str, key: &str, name: Option<&str>, value: Option<&str>) -> String {
    let table = LABELS
        .iter()
        .find(|(candidate, _)| *candidate == locale)
        .map(|(_, labels)| labels)
        .unwrap_or(&LABELS[0].1);
    let template = table
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, text)| *text)
        .expect("the shipped locale tables carry every label");
    template
        .replace("{name}", name.unwrap_or_default())
        .replace("{value}", value.unwrap_or_default())
}

/// `catalog._normalize_locale_tag`.
fn normalize_locale_tag(value: &str) -> String {
    let normalized = value.trim().replace('_', "-");
    if normalized.is_empty() {
        return "zh-CN".to_string();
    }
    let mut parts = normalized.split('-');
    let language = parts.next().unwrap_or_default().to_lowercase();
    match parts.next() {
        None => language,
        Some(region) => {
            let mut tag = format!("{language}-{}", region.to_uppercase());
            for rest in parts {
                tag.push('-');
                tag.push_str(rest);
            }
            tag
        }
    }
}

/// `catalog._preferred_effect_locale`.
///
/// Only the shipped platform (Windows) is supported, so the host half of the
/// reference resolves the same way it does for the Python worker:
///
/// - `NIOH3_SCROLL_LOCALE`, when set and non-empty, is the preferred locale,
///   normalized exactly like `_normalize_locale_tag` (underscores to hyphens,
///   lower-case language, upper-case region).
/// - Otherwise the reference reads `locale.getlocale()[0]`, which on Windows is
///   the C runtime's `setlocale(LC_CTYPE, "")` result. That call reports the OS
///   default UI locale as `Language_Region` with an English language word
///   (`English_United States`, `Japanese_Japan`) and ignores `LC_ALL`,
///   `LC_CTYPE` and `LANG`; a bare `en`/`ja`/`zh` token never appears. The
///   reference only accepts a shipped locale when the token before the hyphen
///   matches one of those three, so every such host locale takes the reference's
///   Chinese-name fallback. `HOST_LOCALE_FALLBACK` is that outcome, not a
///   preference of this port.
///
/// `tests/migration/test_application_worker_parity.py` drives both workers over
/// an environment matrix (unset, empty, explicit, non-matching, and a
/// `LC_ALL`-only case) and requires identical payloads, so a change in either
/// resolution is a gate failure rather than silent drift.
fn preferred_effect_locale() -> String {
    match std::env::var("NIOH3_SCROLL_LOCALE") {
        Ok(value) if !value.trim().is_empty() => normalize_locale_tag(&value),
        _ => HOST_LOCALE_FALLBACK.to_string(),
    }
}

/// `int(text, 0)` for the integer spellings the shipped catalogs use.
fn parse_int_auto(text: &str) -> Option<i64> {
    let text = text.trim();
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let magnitude = if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()?
    } else if let Some(octal) = body.strip_prefix("0o").or_else(|| body.strip_prefix("0O")) {
        i64::from_str_radix(octal, 8).ok()?
    } else if let Some(binary) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
        i64::from_str_radix(binary, 2).ok()?
    } else {
        body.parse::<i64>().ok()?
    };
    Some(if negative { -magnitude } else { magnitude })
}

/// `_hex_key(value, width)`.
fn hex_key(value: u32, width: usize) -> String {
    format!("0X{value:0width$X}")
}

/// `" ".join(text.split())`, including the ASCII separators Python counts as
/// whitespace but the Unicode property does not.
fn collapse_whitespace(text: &str) -> String {
    text.split(|character: char| {
        character.is_whitespace() || matches!(character, '\u{1c}'..='\u{1f}')
    })
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

/// `AuxiliaryNameCatalog._render_rule_name`.
fn render_rule_name(name: &str, qualifier: Option<&str>) -> String {
    let first = qualifier.unwrap_or_default();
    let rendered = match python_format(name, first) {
        Some(text) => text,
        // The reference's fallback for a template its `str.format` call rejects.
        None => name
            .replace("{0}", first)
            .replace("{1}", "")
            .replace("{}", ""),
    };
    collapse_whitespace(&rendered)
}

/// `name.format(qualifier, "")` for the constructs the shipped names use (`{}`,
/// `{0}`, `{1}`, `{{`, `}}`), including Python's positional fields: the first
/// `{}` takes the qualifier and the second the empty second argument. Anything
/// else reports the error the reference catches, so the caller takes its
/// naive-replacement path.
fn python_format(name: &str, first: &str) -> Option<String> {
    let mut rendered = String::with_capacity(name.len());
    let mut characters = name.chars().peekable();
    let mut automatic_fields = 0usize;
    let mut used_automatic = false;
    let mut used_manual = false;
    while let Some(character) = characters.next() {
        match character {
            '{' => match characters.next() {
                Some('{') => rendered.push('{'),
                Some('}') => {
                    // Mixing automatic and manual numbering is a ValueError in
                    // Python, and only two arguments are supplied.
                    if used_manual || automatic_fields > 1 {
                        return None;
                    }
                    used_automatic = true;
                    if automatic_fields == 0 {
                        rendered.push_str(first);
                    }
                    automatic_fields += 1;
                }
                Some('0') => {
                    if characters.next() != Some('}') || used_automatic {
                        return None;
                    }
                    used_manual = true;
                    rendered.push_str(first);
                }
                Some('1') => {
                    if characters.next() != Some('}') || used_automatic {
                        return None;
                    }
                    used_manual = true;
                }
                _ => return None,
            },
            '}' => {
                if characters.next() != Some('}') {
                    return None;
                }
                rendered.push('}');
            }
            other => rendered.push(other),
        }
    }
    Some(rendered)
}

/// `catalog._load_multilingual_effect_names`.
///
/// A missing, unreadable, malformed, or schema-mismatched file leaves the
/// catalog empty exactly like the reference, so an effect stays unnamed instead
/// of the worker refusing to serve anything.
#[derive(Debug, Default)]
struct EffectNameCatalog {
    /// Effect id -> normalized locale -> name.
    names: BTreeMap<u32, BTreeMap<String, String>>,
}

impl EffectNameCatalog {
    fn load(data_root: &Path) -> Self {
        let Ok(text) = fs::read_to_string(data_root.join("effect_names_multilingual.json")) else {
            return Self::default();
        };
        let Ok(payload) = serde_json::from_str::<Value>(&text) else {
            return Self::default();
        };
        if payload.get("schema").and_then(Value::as_str) != Some(EFFECT_LOCALIZATION_SCHEMA) {
            return Self::default();
        }
        let Some(effects) = payload.get("effects").and_then(Value::as_object) else {
            return Self::default();
        };
        let mut names: BTreeMap<u32, BTreeMap<String, String>> = BTreeMap::new();
        for (raw_id, entry) in effects {
            let Some(effect_id) = parse_int_auto(raw_id).and_then(|id| u32::try_from(id).ok())
            else {
                continue;
            };
            let mut localized: BTreeMap<String, String> = BTreeMap::new();
            if let Some(locales) = entry.get("names").and_then(Value::as_object) {
                for (raw_locale, raw_name) in locales {
                    let name = raw_name.as_str().unwrap_or_default().trim();
                    if name.is_empty() {
                        continue;
                    }
                    localized.insert(normalize_locale_tag(raw_locale), name.to_string());
                }
            }
            if !localized.is_empty() {
                names.insert(effect_id, localized);
            }
        }
        Self { names }
    }

    /// `catalog.native_effect_name`: exact locale, then same language, then the
    /// reference's Chinese/English/any fallback chain.
    fn name(&self, effect_id: u32, locale: &str) -> Option<&str> {
        let names = self.names.get(&effect_id)?;
        let preferred = normalize_locale_tag(locale);
        if let Some(exact) = names.get(&preferred) {
            return Some(exact);
        }
        let language = preferred.split('-').next().unwrap_or_default();
        if let Some((_, name)) = names
            .iter()
            .find(|(candidate, _)| candidate.split('-').next() == Some(language))
        {
            return Some(name);
        }
        names
            .get("zh-CN")
            .or_else(|| names.get("en-US"))
            .or_else(|| names.values().next())
            .map(String::as_str)
    }
}

/// `catalog._player_ready_effect_name`.
fn player_ready_effect_name(
    catalog: &EffectNameCatalog,
    preferred_locale: &str,
    effect_id: u16,
) -> String {
    let curated = || {
        CURATED_EFFECT_NAMES
            .iter()
            .find(|(key, _)| *key == effect_id)
            .map(|(_, name)| (*name).to_string())
    };
    match catalog.name(u32::from(effect_id), preferred_locale) {
        None => curated().unwrap_or_else(|| UNKNOWN_EFFECT_NAME.to_string()),
        Some(name)
            if UNRESOLVED_MARKERS
                .iter()
                .any(|marker| name.contains(marker)) =>
        {
            curated().unwrap_or_else(|| name.to_string())
        }
        Some(name) => name.to_string(),
    }
}

/// `auxiliary_catalog._load_special_rule_item_names`.
fn load_special_rule_item_names(data_root: &Path) -> BTreeMap<u32, BTreeMap<String, String>> {
    let Ok(text) = fs::read_to_string(data_root.join("special_rule_item_names.json")) else {
        return BTreeMap::new();
    };
    let Ok(payload) = serde_json::from_str::<Value>(&text) else {
        return BTreeMap::new();
    };
    if payload.get("schema").and_then(Value::as_str) != Some(SPECIAL_RULE_ITEM_NAME_SCHEMA) {
        return BTreeMap::new();
    }
    let Some(items) = payload.get("items").and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    let mut names: BTreeMap<u32, BTreeMap<String, String>> = BTreeMap::new();
    for (raw_key, raw_names) in items {
        let Some(key) = parse_int_auto(raw_key).and_then(|key| u32::try_from(key).ok()) else {
            continue;
        };
        let Some(entries) = raw_names.as_object() else {
            continue;
        };
        let mut localized: BTreeMap<String, String> = BTreeMap::new();
        for (locale, raw_name) in entries {
            let name = raw_name.as_str().unwrap_or_default().trim();
            if name.is_empty() {
                continue;
            }
            localized.insert(locale.clone(), name.to_string());
        }
        if !localized.is_empty() {
            names.insert(key, localized);
        }
    }
    names
}

/// One `special_rules` entry of a shipped auxiliary-name catalog.
#[derive(Debug)]
struct RuleNameEntry {
    name: Option<String>,
    qualifier: Option<String>,
    display_name: Option<String>,
    name_text_id: Option<String>,
}

/// `auxiliary_catalog.AuxiliaryNameCatalog`: the names one locale ships.
#[derive(Debug)]
struct AuxiliaryNameCatalog {
    locale: String,
    /// Terrain display name per hash key, keyed by the shipped `0x%04X`
    /// spelling the reference compares against.
    terrain: BTreeMap<String, String>,
    special_rules: BTreeMap<String, RuleNameEntry>,
    enemies: BTreeMap<String, String>,
}

/// A non-empty trimmed string, mirroring `str(value or "").strip()` truthiness.
fn trimmed_string(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// A non-empty string, mirroring `str(value)` truthiness without trimming.
fn plain_string(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?;
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// `auxiliary_catalog.load_auxiliary_name_catalog` plus `_load_one`.
fn load_auxiliary_name_catalog(
    locale: &str,
    root: &Path,
) -> Result<AuxiliaryNameCatalog, EngineError> {
    let requested = root.join(format!("{locale}.json"));
    let path = if requested.is_file() {
        requested
    } else {
        root.join(format!("{AUXILIARY_NAME_FALLBACK}.json"))
    };
    let text = fs::read_to_string(&path).map_err(|error| {
        EngineError::new(
            "RESOURCE_MISMATCH",
            format!("no auxiliary-name catalog for {locale} or {AUXILIARY_NAME_FALLBACK}: {error}"),
        )
    })?;
    let payload: Value = serde_json::from_str(&text).map_err(|error| {
        EngineError::new(
            "RESOURCE_MISMATCH",
            format!("invalid auxiliary-name catalog {}: {error}", path.display()),
        )
    })?;
    if payload.get("schema").and_then(Value::as_str) != Some(AUXILIARY_NAME_SCHEMA) {
        return Err(EngineError::new(
            "INVALID_REQUEST",
            format!(
                "unsupported auxiliary-name catalog schema: {}",
                path.display()
            ),
        ));
    }

    let mut terrain: BTreeMap<String, String> = BTreeMap::new();
    let rows = payload
        .get("terrain")
        .and_then(Value::as_object)
        .ok_or_else(|| EngineError::new("RESOURCE_MISMATCH", "terrain must be an object"))?;
    // The reference walks the file order, which is ascending row index.
    let mut ordered: Vec<(&String, &Value)> = rows.iter().collect();
    ordered.sort_by_key(|(key, _)| row_order_key(key));
    for (_, entry) in ordered {
        let Some(name) = plain_string(entry.get("name")) else {
            continue;
        };
        let Some(hashes) = entry.get("hash_keys").and_then(Value::as_array) else {
            continue;
        };
        for hash in hashes.iter().filter_map(Value::as_str) {
            terrain
                .entry(hash.to_string())
                .or_insert_with(|| name.clone());
        }
    }

    let mut special_rules: BTreeMap<String, RuleNameEntry> = BTreeMap::new();
    let rules = payload
        .get("special_rules")
        .and_then(Value::as_object)
        .ok_or_else(|| EngineError::new("RESOURCE_MISMATCH", "special_rules must be an object"))?;
    for (key, entry) in rules {
        special_rules.insert(
            key.clone(),
            RuleNameEntry {
                name: trimmed_string(entry.get("name")),
                qualifier: trimmed_string(entry.get("qualifier")),
                display_name: trimmed_string(entry.get("display_name")),
                name_text_id: entry.get("name_text_id").map(|value| match value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                }),
            },
        );
    }

    let mut enemies: BTreeMap<String, String> = BTreeMap::new();
    let entries = payload
        .get("enemies")
        .and_then(Value::as_object)
        .ok_or_else(|| EngineError::new("RESOURCE_MISMATCH", "enemies must be an object"))?;
    for (key, entry) in entries {
        if let Some(name) = plain_string(entry.get("name")) {
            enemies.insert(key.clone(), name);
        }
    }

    Ok(AuxiliaryNameCatalog {
        locale: locale.to_string(),
        terrain,
        special_rules,
        enemies,
    })
}

/// Ascending numeric order for an integer-keyed object, with a deterministic
/// fallback for a key that is not an integer.
fn row_order_key(key: &str) -> (bool, i64, &str) {
    match parse_int_auto(key) {
        Some(value) => (false, value, ""),
        None => (true, 0, key),
    }
}

impl AuxiliaryNameCatalog {
    /// `AuxiliaryNameCatalog.terrain_effect_name`.
    fn terrain_effect_name(&self, key: u16) -> String {
        let wanted = format!("0x{key:04X}");
        match self.terrain.get(&wanted) {
            Some(name) => name.clone(),
            None => render_label(&self.locale, "unknown_terrain_effect", None, Some(&wanted)),
        }
    }

    /// `AuxiliaryNameCatalog.enemy_name`.
    fn enemy_name(&self, lookup_key: u32) -> String {
        match self.enemies.get(&hex_key(lookup_key, 8)) {
            Some(name) => name.clone(),
            None => render_label(
                &self.locale,
                "unknown_enemy",
                None,
                Some(&format!("0x{lookup_key:08X}")),
            ),
        }
    }

    /// `AuxiliaryNameCatalog.special_rule_name`.
    fn special_rule_name(
        &self,
        key: u16,
        tables: &SpecialRuleTables,
        effect_names: &EffectNameCatalog,
        item_names: &BTreeMap<u32, BTreeMap<String, String>>,
    ) -> String {
        if key == 0 {
            return render_label(&self.locale, "none", None, None);
        }
        if let Some(entry) = self.special_rules.get(&hex_key(u32::from(key), 4)) {
            let raw_name = entry.name.clone().unwrap_or_default();
            let mut qualifier = entry.qualifier.clone().unwrap_or_default();
            if qualifier.is_empty() {
                qualifier = self
                    .special_rule_qualifier(key, tables, effect_names, item_names)
                    .unwrap_or_default();
            }
            if qualifier.is_empty()
                && entry
                    .name_text_id
                    .clone()
                    .unwrap_or_default()
                    .to_lowercase()
                    == SELF_QUALIFIER_TEXT_ID
            {
                qualifier = format!("0x{key:04X}");
            }
            if !raw_name.is_empty() {
                let rendered = render_rule_name(
                    &raw_name,
                    if qualifier.is_empty() {
                        None
                    } else {
                        Some(qualifier.as_str())
                    },
                );
                if !rendered.is_empty() {
                    return rendered;
                }
            }
            if let Some(display_name) = entry.display_name.clone() {
                return display_name;
            }
        }
        render_label(
            &self.locale,
            "unknown_rule",
            None,
            Some(&format!("0x{key:04X}")),
        )
    }

    /// `AuxiliaryNameCatalog._special_rule_qualifier`.
    fn special_rule_qualifier(
        &self,
        key: u16,
        tables: &SpecialRuleTables,
        effect_names: &EffectNameCatalog,
        item_names: &BTreeMap<u32, BTreeMap<String, String>>,
    ) -> Option<String> {
        let detail = describe_special_rule(key, tables).ok()?;
        let qualifier_key = detail.qualifier_key?;
        match detail.qualifier_kind {
            Some("effect") => effect_names
                .name(qualifier_key, &self.locale)
                .map(str::to_string),
            Some("enemy") => Some(self.enemy_name(qualifier_key)),
            Some("item") => Some(self.item_qualifier_name(qualifier_key, item_names)),
            _ => None,
        }
    }

    /// `AuxiliaryNameCatalog._special_rule_qualifier`'s item branch.
    fn item_qualifier_name(
        &self,
        qualifier_key: u32,
        item_names: &BTreeMap<u32, BTreeMap<String, String>>,
    ) -> String {
        let names = item_names.get(&qualifier_key);
        if let Some(names) = names {
            if let Some(exact) = names.get(&self.locale) {
                return exact.clone();
            }
            let language = self.locale.split('-').next().unwrap_or_default();
            if let Some((_, name)) = names
                .iter()
                .find(|(locale, _)| locale.split('-').next() == Some(language))
            {
                return name.clone();
            }
        }
        if self.locale.starts_with("zh") {
            return format!("未识别阴阳术（原生编号 0x{qualifier_key:04X}，可生成）");
        }
        names
            .and_then(|names| names.get("en-US").cloned())
            .unwrap_or_else(|| {
                render_label(
                    &self.locale,
                    "item",
                    None,
                    Some(&format!("0x{qualifier_key:04X}")),
                )
            })
    }

    /// `AuxiliaryNameCatalog.special_rule_key_groups`, ordered by case-folded
    /// display name with numeric keys inside each family.
    fn special_rule_key_groups(
        &self,
        allowed: &BTreeSet<u16>,
        tables: &SpecialRuleTables,
        effect_names: &EffectNameCatalog,
        item_names: &BTreeMap<u32, BTreeMap<String, String>>,
    ) -> Result<Vec<(String, Vec<u16>)>, EngineError> {
        let mut grouped: BTreeMap<String, BTreeSet<u16>> = BTreeMap::new();
        for raw_key in self.special_rules.keys() {
            let key = parse_int_auto(raw_key)
                .and_then(|key| u16::try_from(key).ok())
                .ok_or_else(|| {
                    EngineError::new(
                        "RESOURCE_MISMATCH",
                        format!("special-rule catalog key {raw_key} is not a uint16"),
                    )
                })?;
            if !allowed.contains(&key) {
                continue;
            }
            let name = self
                .special_rule_name(key, tables, effect_names, item_names)
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            grouped.entry(name).or_default().insert(key);
        }
        let mut families: Vec<(String, Vec<u16>)> = grouped
            .into_iter()
            .map(|(name, keys)| (name, keys.into_iter().collect()))
            .collect();
        families.sort_by(|left, right| left.0.to_lowercase().cmp(&right.0.to_lowercase()));
        Ok(families)
    }
}

/// Everything one `search.catalog` payload is derived from.
pub struct CatalogInputs<'a> {
    pub context_digest: &'a str,
    pub rarity: u8,
    pub locale: &'a str,
    pub index: &'a EffectTableIndex,
    pub effect: &'a EffectResourceBytes,
    pub preview: &'a PreviewResources,
    pub recommended_level: &'a RecommendedLevelCurve,
}

/// The bundled name catalogs `search.catalog` renders from.
///
/// The multilingual effect catalog and the special-rule item names are read
/// once; the per-locale auxiliary name catalogs are read on first use and then
/// kept, matching the reference's `lru_cache`.
#[derive(Debug)]
pub struct Catalog {
    data_root: PathBuf,
    preferred_locale: String,
    effect_names: EffectNameCatalog,
    item_names: BTreeMap<u32, BTreeMap<String, String>>,
    locales: Mutex<BTreeMap<String, Arc<AuxiliaryNameCatalog>>>,
}

impl Catalog {
    /// Load the request-independent catalogs. The auxiliary names stay lazy.
    pub fn load(data_root: &Path) -> Result<Self, EngineError> {
        Ok(Self {
            data_root: data_root.to_path_buf(),
            preferred_locale: preferred_effect_locale(),
            effect_names: EffectNameCatalog::load(data_root),
            item_names: load_special_rule_item_names(data_root),
            locales: Mutex::new(BTreeMap::new()),
        })
    }

    /// The auxiliary-name catalog for one locale, cached per locale.
    fn auxiliary(&self, locale: &str) -> Result<Arc<AuxiliaryNameCatalog>, EngineError> {
        let mut cache = self.locales.lock().expect("auxiliary-name catalog cache");
        if let Some(catalog) = cache.get(locale) {
            return Ok(Arc::clone(catalog));
        }
        let root = self.data_root.join("auxiliary_names");
        let catalog = Arc::new(load_auxiliary_name_catalog(locale, &root)?);
        cache.insert(locale.to_string(), Arc::clone(&catalog));
        Ok(catalog)
    }

    /// The `search.catalog` result, mirroring `search_worker.main`.
    pub fn payload(&self, inputs: CatalogInputs<'_>) -> Result<Value, EngineError> {
        if !SUPPORTED_LOCALES.contains(&inputs.locale) {
            return Err(EngineError::new(
                "INVALID_REQUEST",
                format!("unsupported catalog locale: {}", inputs.locale),
            ));
        }
        let names = self.auxiliary(inputs.locale)?;
        let ordinary = self.ordinary_effects(inputs.index, inputs.rarity, inputs.locale)?;
        let grace = grace_effects(
            inputs.effect,
            inputs.rarity,
            inputs.locale,
            &self.effect_names,
        );
        let terrain = terrain_options(&inputs.preview.roster, &names, inputs.locale);
        let enemies = enemy_options(&inputs.preview.roster, &names)?;
        let allowed = legal_special_rule_keys(CATALOG_PLAYTHROUGH, &inputs.preview.rules)
            .map_err(rule_table_error)?;
        let rules = self.special_rule_options(&names, &inputs.preview.rules, &allowed)?;
        let families = names.special_rule_key_groups(
            &allowed,
            &inputs.preview.rules,
            &self.effect_names,
            &self.item_names,
        )?;
        Ok(json!({
            "context_digest": inputs.context_digest,
            "ordinary_effects": ordinary,
            "grace_effects": grace,
            "terrain_options": terrain,
            "enemy_options": enemies,
            "special_rule_options": rules,
            "special_rule_families": families
                .into_iter()
                .map(|(name, keys)| json!({"name": name, "keys": keys}))
                .collect::<Vec<Value>>(),
            "recommended_level": inputs.recommended_level.metadata(),
        }))
    }

    /// `catalog.searchable_scroll_effect_definitions` for one rarity.
    fn ordinary_effects(
        &self,
        index: &EffectTableIndex,
        rarity: u8,
        locale: &str,
    ) -> Result<Vec<Value>, EngineError> {
        if !(3..=5).contains(&rarity) {
            return Err(EngineError::new(
                "INVALID_REQUEST",
                format!("scroll rarity must be 3, 4, or 5, not {rarity}"),
            ));
        }
        let record_type = SCROLL_RECORD_TYPES[usize::from(CATALOG_PLAYTHROUGH - 1)];
        let capacities = index
            .category_capacities(record_type, rarity)
            .map_err(effect_table_error)?;
        let mut reachable: Vec<(String, u16, String)> = Vec::new();
        for (effect_id, effect) in index.effects_by_id.iter() {
            if effect.row_index == 0 {
                continue;
            }
            if !index
                .candidate_context_allowed(*effect_id, record_type, false)
                .map_err(effect_table_error)?
            {
                continue;
            }
            let group = index.groups_by_key.get(&effect.group_key).ok_or_else(|| {
                EngineError::new(
                    "RESOURCE_MISMATCH",
                    format!(
                        "effect 0x{effect_id:04X} references an undeclared group 0x{:04X}",
                        effect.group_key
                    ),
                )
            })?;
            let category_key = usize::from(group.category_key);
            if category_key >= capacities.len() || capacities[category_key] == 0 {
                continue;
            }
            let weight = index
                .native_effect_weight(
                    *effect_id,
                    NativeWeightContext {
                        record_type,
                        rarity,
                        playthrough: CATALOG_PLAYTHROUGH,
                        restricted_destination_slot: false,
                        extra_selector: 0,
                        rarity5_type_floor: 0,
                    },
                )
                .map_err(effect_table_error)?;
            if weight == 0 {
                continue;
            }
            let name =
                player_ready_effect_name(&self.effect_names, &self.preferred_locale, *effect_id);
            reachable.push((name.to_lowercase(), *effect_id, name));
        }
        // The reference sorts by the preferred-locale name it resolved above and
        // only then project the requested locale's name.
        reachable.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        Ok(reachable
            .into_iter()
            .map(|(_, effect_id, preferred_name)| {
                let name = if locale == "zh-CN" {
                    preferred_name
                } else {
                    self.effect_names
                        .name(u32::from(effect_id), locale)
                        .map(str::to_string)
                        .unwrap_or(preferred_name)
                };
                json!({"effect_id": effect_id, "name": name})
            })
            .collect())
    }

    /// `catalog_application.auxiliary_catalog`'s `special_rule_options`.
    fn special_rule_options(
        &self,
        names: &AuxiliaryNameCatalog,
        tables: &SpecialRuleTables,
        allowed: &BTreeSet<u16>,
    ) -> Result<Vec<Value>, EngineError> {
        let mut options = Vec::with_capacity(allowed.len());
        for key in allowed {
            let entry = describe_special_rule(*key, tables).map_err(rule_table_error)?;
            let name = names.special_rule_name(*key, tables, &self.effect_names, &self.item_names);
            options.push(json!({
                "key": key,
                "name": name,
                "variant": variant_json(&entry),
            }));
        }
        Ok(options)
    }
}

/// The `variant` object `catalog_application` renders from one decoded rule.
fn variant_json(entry: &SpecialRuleEntry) -> Value {
    json!({
        "key": entry.key,
        "row_index": entry.row_index,
        "raw_value": entry.raw_value.map(f64::from),
        "display_value": entry.display_value,
        "display_unit": entry.display_unit,
        "display_grade": entry.display_grade,
        "value_source_offset": entry.value_source_offset,
        "qualifier_kind": entry.qualifier_kind,
        "qualifier_key": entry.qualifier_key,
    })
}

/// `search_worker`'s `grace_effects`: the measured map's ids, filtered for
/// rarity 4 to the verified final Grace ids.
fn grace_effects(
    effect: &EffectResourceBytes,
    rarity: u8,
    locale: &str,
    names: &EffectNameCatalog,
) -> Vec<Value> {
    let map = match rarity {
        4 => effect.grace_maps.first(),
        5 => effect.grace_maps.get(1),
        _ => None,
    };
    let Some(map) = map else {
        return Vec::new();
    };
    let ids: BTreeSet<u32> = map
        .ranges
        .iter()
        .map(|range| range.effect_id)
        .filter(|effect_id| rarity != 4 || R4_FINAL_GRACE_IDS.contains(effect_id))
        .collect();
    ids.into_iter()
        .map(|effect_id| {
            let name = names
                .name(effect_id, locale)
                .map(str::to_string)
                .unwrap_or_else(|| format!("0x{effect_id:04X}"));
            json!({"effect_id": effect_id, "name": name})
        })
        .collect()
}

/// `catalog_application.terrain_choices` rendered for one locale.
///
/// The option ids and their order come from [`crate::terrain::terrain_choices`],
/// the same resolver the search uses, so a published option always selects the
/// rows it names.
fn terrain_options(
    roster: &RosterTables,
    names: &AuxiliaryNameCatalog,
    locale: &str,
) -> Vec<Value> {
    crate::terrain::terrain_choices(&roster.terrains)
        .into_iter()
        .map(|choice| {
            let display = choice
                .keys
                .iter()
                .map(|key| names.terrain_effect_name(*key))
                .collect::<Vec<_>>()
                .join(" + ");
            let name = if choice.aggregate {
                render_label(locale, "contains", Some(&display), None)
            } else if display.is_empty() {
                render_label(locale, "no_terrain", None, None)
            } else {
                display
            };
            json!({
                "option_id": choice.option_id,
                "name": name,
                "effect_keys": choice.keys,
                "aggregate": choice.aggregate,
            })
        })
        .collect()
}

/// `catalog_application.auxiliary_catalog`'s `enemy_options`.
fn enemy_options(
    roster: &RosterTables,
    names: &AuxiliaryNameCatalog,
) -> Result<Vec<Value>, EngineError> {
    let roles = enemy_role_by_lookup_key(roster).map_err(rule_table_error)?;
    Ok(roles
        .into_iter()
        .map(|(lookup_key, role)| {
            json!({
                "lookup_key": lookup_key,
                "role": role,
                "name": names.enemy_name(lookup_key),
            })
        })
        .collect())
}

/// A rule-table failure the reference reports as `INVALID_REQUEST`.
fn rule_table_error(error: EnemyError) -> EngineError {
    EngineError::new("INVALID_REQUEST", error.to_string())
}

/// An effect-table failure. The reference raises `ValueError`, which the worker
/// answers as `INVALID_REQUEST`.
fn effect_table_error(error: nioh3_domain::effect::EffectError) -> EngineError {
    EngineError::new("INVALID_REQUEST", format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nioh3_data::{load_effect_resource, load_preview_resources};

    fn data_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data")
    }

    #[test]
    fn locale_tags_normalize_like_the_reference() {
        assert_eq!(normalize_locale_tag(""), "zh-CN");
        assert_eq!(normalize_locale_tag("  "), "zh-CN");
        assert_eq!(normalize_locale_tag("zh_CN"), "zh-CN");
        assert_eq!(normalize_locale_tag("EN-us"), "en-US");
        assert_eq!(normalize_locale_tag("zh-Hans-CN"), "zh-HANS-CN");
    }

    #[test]
    fn rule_templates_render_like_the_reference() {
        assert_eq!(render_rule_name("神器掉落率上升{}", None), "神器掉落率上升");
        // Two automatic fields: the qualifier fills the first, the second stays
        // empty because that is the argument the reference passes.
        assert_eq!(
            render_rule_name("优先掉落率上升（{}）{}", Some("月读的恩宠")),
            "优先掉落率上升（月读的恩宠）"
        );
        assert_eq!(
            render_rule_name("伤害反映（{0}）", Some("忍术威力")),
            "伤害反映（忍术威力）"
        );
        assert_eq!(render_rule_name("{0}{1}", Some("x")), "x");
        assert_eq!(render_rule_name("foo {1} bar", None), "foo bar");
        assert_eq!(render_rule_name("  spaced   out  ", None), "spaced out");
        // Constructs `str.format` rejects take the naive replacement path:
        // an out-of-range index, and mixing automatic with manual numbering.
        assert_eq!(render_rule_name("{0}{2}", Some("x")), "x{2}");
        assert_eq!(render_rule_name("{} {0}", Some("x")), "x");
    }

    #[test]
    fn unresolved_native_names_take_the_curated_overlay() {
        let catalog = Catalog::load(&data_root()).expect("load the shipped catalogs");
        // 0xB82B is the one reachable effect whose native zh-CN string is a
        // sentence template with control markup.
        assert_eq!(
            player_ready_effect_name(&catalog.effect_names, &catalog.preferred_locale, 0xB82B),
            "敌人精力耗尽时赋予受到伤害增加"
        );
        assert_eq!(
            player_ready_effect_name(&catalog.effect_names, &catalog.preferred_locale, 0x1355),
            "灵力增加量"
        );
        // A native string that is missing entirely falls back to "unnamed".
        assert_eq!(
            player_ready_effect_name(&catalog.effect_names, &catalog.preferred_locale, 0x7FFF),
            UNKNOWN_EFFECT_NAME
        );
    }

    #[test]
    fn the_shipped_catalog_payload_has_the_measured_shape() {
        let root = data_root();
        let effect = load_effect_resource(&root).expect("load the shipped effect resource");
        let index = EffectTableIndex::from_resource(&effect).expect("index the shipped tables");
        let preview = load_preview_resources(&root).expect("load the shipped preview resources");
        let curve = crate::recommended_level::load(&root).expect("load the shipped curve");
        let catalog = Catalog::load(&root).expect("load the shipped catalogs");

        let mut ordinary = Vec::new();
        let mut grace = Vec::new();
        for rarity in [3u8, 4, 5] {
            let payload = catalog
                .payload(CatalogInputs {
                    context_digest: "test-context",
                    rarity,
                    locale: "zh-CN",
                    index: &index,
                    effect: &effect,
                    preview: &preview,
                    recommended_level: &curve,
                })
                .expect("compose the shipped catalog");
            ordinary.push(payload["ordinary_effects"].as_array().unwrap().len());
            grace.push(payload["grace_effects"].as_array().unwrap().len());
            assert_eq!(
                payload["terrain_options"].as_array().unwrap().len(),
                6,
                "terrain option count for rarity {rarity}"
            );
            assert_eq!(payload["enemy_options"].as_array().unwrap().len(), 487);
            assert_eq!(
                payload["special_rule_options"].as_array().unwrap().len(),
                277
            );
            assert_eq!(
                payload["special_rule_families"].as_array().unwrap().len(),
                103
            );
            assert_eq!(payload["context_digest"], json!("test-context"));
        }
        assert_eq!(ordinary, vec![49, 50, 50]);
        assert_eq!(grace, vec![0, 21, 11]);
    }

    #[test]
    fn the_measured_terrain_options_keep_their_reference_order() {
        let root = data_root();
        let preview = load_preview_resources(&root).expect("load the shipped preview resources");
        let catalog = Catalog::load(&root).expect("load the shipped catalogs");
        let names = catalog.auxiliary("zh-CN").expect("load the zh-CN catalog");
        let options = terrain_options(&preview.roster, &names, "zh-CN");
        let ids: Vec<&str> = options
            .iter()
            .map(|option| option["option_id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            vec![
                "exact:24",
                "exact:",
                "exact:24,39",
                "exact:58",
                "exact:24,39F",
                "contains:24",
            ]
        );
    }
}
