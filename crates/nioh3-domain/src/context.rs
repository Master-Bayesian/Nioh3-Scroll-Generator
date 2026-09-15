//! Auxiliary descriptor-context port of `nioh3_scroll_editor/auxiliary_generation.py`
//! for PC v2.00.02.
//!
//! Ported entry points and the descriptor field each one drives:
//!
//! | reference function | native RVA | descriptor byte |
//! | --- | --- | --- |
//! | `generate_auxiliary_mode` | 0x10291F0 | `+0x1E` |
//! | `generate_terrain` | 0x1028ED0 | `+0x1F` |
//! | `generate_auxiliary_descriptor_flags` | 0x1028520 | `+0x20..+0x23` |
//!
//! The three scoped seed derivations those generators install are ported with
//! them. Everything here is pointer-free: only caller-supplied rows are read,
//! and a missing, duplicated, or non-native row fails closed instead of being
//! guessed. Diagnostic event traces in the reference carry no returned value
//! and are deliberately not modeled.

use crate::enemy::{ContextTables, EnemyError, RosterTables};
use crate::rng::{cvtt_i32, f32_of, lottery_10000, LcgStream};

/// Lookup key of the optional-multiplier row read by RVA 0x10291F0.
pub const AUXILIARY_MODE_THRESHOLD_KEY: u32 = 0x1E7D;
/// Lookup keys of the three descriptor flag rows, in native evaluation order.
pub const AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS: [u32; 3] = [0x3903, 0x779F, 0x0275];
/// Lookup key of the optional-multiplier row behind the descriptor selector.
pub const AUXILIARY_DESCRIPTOR_SELECTOR_KEY: u32 = 0xDA38;
/// Low seed mask the auxiliary mode stream installs.
pub const AUXILIARY_MODE_SEED_MASK_LOW: u32 = 0x01E3_C78F;
/// High seed mask the mode and descriptor streams install.
pub const AUXILIARY_MODE_SEED_MASK_HIGH: u32 = 0x00E1_C387;

/// Byte offsets inside a 0x20-byte optional-multiplier row.
const OPTIONAL_ROW_BASE: usize = 0x10;
const OPTIONAL_ROW_KEY: usize = 0x14;
const OPTIONAL_ROW_SCALE: usize = 0x18;

/// Byte offsets inside a 0x30-byte context row.
const CONTEXT_MODE: usize = 0x28;
const CONTEXT_BRANCH: usize = 0x29;

/// Byte offsets and bits inside a 0x34-byte terrain row.
const TERRAIN_EXCLUDED_FLAG: usize = 0x2E;
const TERRAIN_EXCLUDED_BIT: u8 = 0x02;
const TERRAIN_VALUE: usize = 0x30;

/// Byte offset of the selector value inside a 0x1C-byte enemy-candidate row.
const ENEMY_SELECTOR_VALUE: usize = 0x19;

/// Resolved descriptor context for one displayed Seed.
///
/// `selector` is the value the roster generator must apply. The higher-level
/// public wrapper fails closed on a nonzero selector, exactly like the current
/// `generate_enemy_variant`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContext {
    pub auxiliary_mode: u8,
    pub mode_branch: u8,
    pub mode_draws: u64,
    pub mode_row_index: Option<usize>,
    pub terrain_row_index: usize,
    pub terrain_value: u8,
    pub used_filtered_pool: bool,
    pub selector: u8,
    pub flags: [bool; 3],
    pub descriptor_draws: u64,
}

/// Auxiliary mode phase result: descriptor byte `+0x1E`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModeOutcome {
    value: u8,
    branch_class: u8,
    draws: u64,
    row_index: Option<usize>,
}

/// Terrain phase result: descriptor byte `+0x1F`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerrainOutcome {
    value: u8,
    row_index: usize,
    used_filtered_pool: bool,
}

/// Descriptor flag and selector phase result: bytes `+0x20..+0x23`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DescriptorOutcome {
    selector: u8,
    flags: [bool; 3],
    draws: u64,
}

/// Scoped seed installed at RVA 0x1029204..0x102921E.
pub fn derive_auxiliary_mode_seed(displayed_seed: u32) -> u32 {
    ((displayed_seed & AUXILIARY_MODE_SEED_MASK_LOW) << 3)
        | ((displayed_seed >> 4) & AUXILIARY_MODE_SEED_MASK_HIGH)
}

/// Swapped low 14-bit Seed halves installed at RVA 0x1028EEA..0x1028F08.
pub fn derive_terrain_seed(displayed_seed: u32) -> u32 {
    ((displayed_seed >> 14) & 0x3FFF) | ((displayed_seed & 0x3FFF) << 14)
}

/// Scoped seed installed at RVA 0x102853C..0x1028567.
pub fn derive_auxiliary_descriptor_seed(displayed_seed: u32) -> u32 {
    ((displayed_seed & AUXILIARY_MODE_SEED_MASK_HIGH) << 4)
        | ((displayed_seed >> 3) & AUXILIARY_MODE_SEED_MASK_LOW)
}

/// Integer threshold the native 0..9999 lottery compares against for `key`.
///
/// The row is resolved by the u32 at `+0x14` of the 0x20-byte
/// optional-multiplier table and must be unique. The product is binary32
/// rounded even when the scale is exactly `1.0`, which is deliberate: the
/// reference `rng::threshold_from_config` short-circuits that case, while the
/// optional-multiplier rows always round the base through binary32 first.
/// A product that cannot be represented as an `i32` is rejected outright.
pub fn optional_threshold(tables: &ContextTables, key: u32) -> Result<i32, EnemyError> {
    let row = find_optional_row(tables, key)?;
    let base = i32::from_le_bytes([
        row[OPTIONAL_ROW_BASE],
        row[OPTIONAL_ROW_BASE + 1],
        row[OPTIONAL_ROW_BASE + 2],
        row[OPTIONAL_ROW_BASE + 3],
    ]);
    let scale = f32::from_le_bytes([
        row[OPTIONAL_ROW_SCALE],
        row[OPTIONAL_ROW_SCALE + 1],
        row[OPTIONAL_ROW_SCALE + 2],
        row[OPTIONAL_ROW_SCALE + 3],
    ]);
    let product = f32_of(f64::from(f32_of(f64::from(base))) * f64::from(scale));
    if !product.is_finite() || !(-2147483648.0..2147483648.0).contains(&f64::from(product)) {
        return Err(EnemyError::InvalidInput(format!(
            "optional multiplier key 0x{key:04X} yields a non-integer threshold"
        )));
    }
    Ok(cvtt_i32(f64::from(product)))
}

/// Resolve the auxiliary mode, terrain row, and descriptor bytes for one Seed.
pub fn resolve_context(
    seed: u32,
    tables: &RosterTables,
    context: &ContextTables,
) -> Result<ResolvedContext, EnemyError> {
    let mode = resolve_mode(seed, context)?;
    let terrain = resolve_terrain(seed, mode.value, tables, context)?;
    let descriptor = resolve_descriptor(seed, mode.value, tables, context)?;
    Ok(ResolvedContext {
        auxiliary_mode: mode.value,
        mode_branch: mode.branch_class,
        mode_draws: mode.draws,
        mode_row_index: mode.row_index,
        terrain_row_index: terrain.row_index,
        terrain_value: terrain.value,
        used_filtered_pool: terrain.used_filtered_pool,
        selector: descriptor.selector,
        flags: descriptor.flags,
        descriptor_draws: descriptor.draws,
    })
}

/// `generate_auxiliary_mode`: descriptor byte `+0x1E` from RVA 0x10291F0.
fn resolve_mode(seed: u32, context: &ContextTables) -> Result<ModeOutcome, EnemyError> {
    let threshold = optional_threshold(context, AUXILIARY_MODE_THRESHOLD_KEY)?;
    let mut stream = LcgStream::new(derive_auxiliary_mode_seed(seed));
    let first_roll = lottery_10000(stream.u16());
    let mut draws = 1u64;

    // EDI starts at 2. A first roll that misses the threshold consumes one more
    // draw and splits the remaining path evenly into classes 1 and 0.
    let mut branch_class = 2u8;
    if first_roll >= threshold {
        let second_roll = scoped_random_int(&mut stream, 2);
        draws += 1;
        branch_class = if second_roll == 0 { 1 } else { 0 };
    }

    let matching: Vec<usize> = context
        .contexts
        .iter()
        .enumerate()
        .filter(|(_, row)| row[CONTEXT_BRANCH] == branch_class)
        .map(|(index, _)| index)
        .collect();
    if matching.is_empty() {
        return Ok(ModeOutcome {
            value: 0,
            branch_class,
            draws,
            row_index: None,
        });
    }

    let count = u32::try_from(matching.len())
        .map_err(|_| EnemyError::InvalidInput("context table exceeds uint32 rows".to_string()))?;
    let selected = scoped_random_int(&mut stream, count) as usize;
    draws += 1;
    let row_index = matching[selected];
    Ok(ModeOutcome {
        value: context.contexts[row_index][CONTEXT_MODE],
        branch_class,
        draws,
        row_index: Some(row_index),
    })
}

/// `generate_terrain`: descriptor byte `+0x1F` from RVA 0x1028ED0.
fn resolve_terrain(
    seed: u32,
    auxiliary_mode: u8,
    tables: &RosterTables,
    context: &ContextTables,
) -> Result<TerrainOutcome, EnemyError> {
    let matches = context_rows(context, auxiliary_mode)?;
    if tables.terrains.is_empty() {
        return Err(EnemyError::MissingData(
            "native terrain table is empty".to_string(),
        ));
    }
    if tables.terrain_keys.len() != tables.terrains.len() {
        return Err(EnemyError::InvalidInput(
            "terrain key index does not match row count".to_string(),
        ));
    }

    let mut stream = LcgStream::new(derive_terrain_seed(seed));
    let used_filtered_pool = matches.is_some_and(|row| row[CONTEXT_BRANCH] != 2);
    let (row_index, value) = if used_filtered_pool {
        let eligible: Vec<usize> = tables
            .terrains
            .iter()
            .enumerate()
            .filter(|(_, row)| row[TERRAIN_EXCLUDED_FLAG] & TERRAIN_EXCLUDED_BIT == 0)
            .map(|(index, _)| index)
            .collect();
        if eligible.is_empty() {
            return Err(EnemyError::MissingData(
                "native filtered terrain pool is empty".to_string(),
            ));
        }
        let count = u32::try_from(eligible.len()).map_err(|_| {
            EnemyError::InvalidInput("terrain table exceeds uint32 rows".to_string())
        })?;
        let selected = eligible[scoped_random_int(&mut stream, count) as usize];
        (selected, tables.terrains[selected][TERRAIN_VALUE])
    } else {
        let count = u32::try_from(tables.terrains.len()).map_err(|_| {
            EnemyError::InvalidInput("terrain table exceeds uint32 rows".to_string())
        })?;
        let selected = scoped_random_int(&mut stream, count) as usize;
        (selected, tables.terrain_keys[selected] as u8)
    };

    Ok(TerrainOutcome {
        value,
        row_index,
        used_filtered_pool,
    })
}

/// `generate_auxiliary_descriptor_flags`: bytes `+0x20..+0x23` from RVA 0x1028520.
///
/// The three output flags are exact pointer-free parity. The uncommon selector
/// path chooses one value from a native linked list whose order was not
/// captured, so this port fails closed unless the whole enemy-candidate table
/// carries exactly one nonzero selector value.
fn resolve_descriptor(
    seed: u32,
    auxiliary_mode: u8,
    tables: &RosterTables,
    context: &ContextTables,
) -> Result<DescriptorOutcome, EnemyError> {
    let matches = context_rows(context, auxiliary_mode)?;
    let mut stream = LcgStream::new(derive_auxiliary_descriptor_seed(seed));
    let mut flags = [false; 3];
    for (slot, key) in AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS.iter().enumerate() {
        let threshold = optional_threshold(context, *key)?;
        let roll = lottery_10000(stream.u16());
        flags[slot] = threshold > roll;
    }
    let mut draws = 3u64;

    let mut selector = 0u8;
    if matches.is_some_and(|row| row[CONTEXT_BRANCH] != 0) {
        let selector_threshold = optional_threshold(context, AUXILIARY_DESCRIPTOR_SELECTOR_KEY)?;
        let selector_roll = lottery_10000(stream.u16());
        draws += 1;
        if selector_threshold > selector_roll {
            selector = unique_selector_value(tables)?;
        }
    }

    Ok(DescriptorOutcome {
        selector,
        flags,
        draws,
    })
}

/// The unique nonzero selector value of the enemy-candidate table.
fn unique_selector_value(tables: &RosterTables) -> Result<u8, EnemyError> {
    let mut distinct: Vec<u8> = Vec::new();
    for row in &tables.enemies {
        let value = row[ENEMY_SELECTOR_VALUE];
        if value != 0 && !distinct.contains(&value) {
            distinct.push(value);
        }
    }
    if distinct.len() != 1 {
        return Err(EnemyError::Unsupported(format!(
            "auxiliary descriptor selector linked-list order is required for {} distinct \
             table values",
            distinct.len()
        )));
    }
    Ok(distinct[0])
}

/// The optional-multiplier row for `key`, which must resolve exactly once.
fn find_optional_row(tables: &ContextTables, key: u32) -> Result<&[u8; 32], EnemyError> {
    let mut found: Option<usize> = None;
    for (index, row) in tables.optional_multipliers.iter().enumerate() {
        if row_key(row) != key {
            continue;
        }
        if found.is_some() {
            return Err(EnemyError::InvalidInput(format!(
                "optional multiplier key 0x{key:04X} resolved to multiple rows"
            )));
        }
        found = Some(index);
    }
    match found {
        Some(index) => Ok(&tables.optional_multipliers[index]),
        None => Err(EnemyError::MissingData(format!(
            "optional multiplier key 0x{key:04X} is not captured"
        ))),
    }
}

/// The single context row whose mode byte is `auxiliary_mode`, if any.
fn context_rows(
    context: &ContextTables,
    auxiliary_mode: u8,
) -> Result<Option<&[u8; 48]>, EnemyError> {
    let mut matched: Option<&[u8; 48]> = None;
    for row in &context.contexts {
        if row[CONTEXT_MODE] != auxiliary_mode {
            continue;
        }
        if matched.is_some() {
            return Err(EnemyError::InvalidInput(format!(
                "auxiliary mode 0x{auxiliary_mode:02X} resolved to multiple context rows"
            )));
        }
        matched = Some(row);
    }
    Ok(matched)
}

/// Lookup key u32 of an optional-multiplier row.
fn row_key(row: &[u8; 32]) -> u32 {
    u32::from_le_bytes([
        row[OPTIONAL_ROW_KEY],
        row[OPTIONAL_ROW_KEY + 1],
        row[OPTIONAL_ROW_KEY + 2],
        row[OPTIONAL_ROW_KEY + 3],
    ])
}

/// `Lcg32.random_int`: floor of the binary32 product, clamped to `count - 1`.
///
/// This is the native clamp, not a modulo. `count` must be at least 1.
fn scoped_random_int(stream: &mut LcgStream, count: u32) -> u32 {
    debug_assert!(count >= 1, "random_int requires a nonempty range");
    let high16 = stream.u16();
    let fraction = f32_of(f64::from(high16) / 65536.0);
    let product = f32_of(f64::from(fraction) * f64::from(count as f32));
    let truncated = product as i64;
    truncated.clamp(0, i64::from(count) - 1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const MODE_VALUE: u8 = 0x77;

    fn optional_row(key: u32, base: i32, scale: f32) -> [u8; 32] {
        let mut row = [0u8; 32];
        row[OPTIONAL_ROW_BASE..OPTIONAL_ROW_BASE + 4].copy_from_slice(&base.to_le_bytes());
        row[OPTIONAL_ROW_KEY..OPTIONAL_ROW_KEY + 4].copy_from_slice(&key.to_le_bytes());
        row[OPTIONAL_ROW_SCALE..OPTIONAL_ROW_SCALE + 4].copy_from_slice(&scale.to_le_bytes());
        row
    }

    fn context_row(mode: u8, branch: u8) -> [u8; 48] {
        let mut row = [0u8; 48];
        row[CONTEXT_MODE] = mode;
        row[CONTEXT_BRANCH] = branch;
        row
    }

    fn terrain_row(flag: u8, value: u8) -> [u8; 52] {
        let mut row = [0u8; 52];
        row[TERRAIN_EXCLUDED_FLAG] = flag;
        row[TERRAIN_VALUE] = value;
        row
    }

    fn enemy_row(selector: u8) -> [u8; 28] {
        let mut row = [0u8; 28];
        row[ENEMY_SELECTOR_VALUE] = selector;
        row
    }

    /// All keys present with a zero threshold except the explicit overrides.
    fn thresholds(overrides: &[(u32, i32)]) -> Vec<[u8; 32]> {
        let mut keys = vec![
            AUXILIARY_MODE_THRESHOLD_KEY,
            AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS[0],
            AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS[1],
            AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS[2],
            AUXILIARY_DESCRIPTOR_SELECTOR_KEY,
        ];
        let mut rows = Vec::new();
        for key in keys.drain(..) {
            let base = overrides
                .iter()
                .find(|(override_key, _)| *override_key == key)
                .map_or(0, |(_, base)| *base);
            rows.push(optional_row(key, base, 1.0));
        }
        rows
    }

    fn context_tables(overrides: &[(u32, i32)]) -> ContextTables {
        ContextTables {
            contexts: vec![context_row(0x10, 0), context_row(0x20, 1)],
            optional_multipliers: thresholds(overrides),
        }
    }

    fn roster_tables(enemies: Vec<[u8; 28]>) -> RosterTables {
        RosterTables {
            enemies,
            contexts: vec![context_row(0x10, 0), context_row(0x20, 1)],
            terrains: vec![terrain_row(0x00, MODE_VALUE), terrain_row(0x02, 0x88)],
            terrain_keys: vec![0x11, 0x22],
            parameter_types: BTreeMap::new(),
        }
    }

    #[test]
    fn scoped_seed_derivations_match_the_native_vectors() {
        // Native vector seeds from the reference auxiliary-generation tests.
        assert_eq!(derive_auxiliary_mode_seed(203_900_415), 0x01DA_6C7F);
        assert_eq!(derive_auxiliary_descriptor_seed(203_900_415), 0x0394_D8FF);
        assert_eq!(
            derive_terrain_seed(0x0ABC_1234),
            ((0x0ABC_1234u32 >> 14) & 0x3FFF) | ((0x0ABC_1234u32 & 0x3FFF) << 14)
        );
    }

    #[test]
    fn optional_threshold_rounds_the_base_through_binary32_even_at_unit_scale() {
        // 2**24 + 1 is not representable in binary32, so the optional-multiplier
        // path loses it while rng::threshold_from_config returns it untouched.
        let context = ContextTables {
            contexts: Vec::new(),
            optional_multipliers: vec![optional_row(0x0001, 16_777_217, 1.0)],
        };
        assert_eq!(optional_threshold(&context, 0x0001).unwrap(), 16_777_216);
        assert_eq!(
            crate::rng::threshold_from_config(Some(&{
                let mut row = [0u8; 32];
                row[0x10..0x14].copy_from_slice(&16_777_217i32.to_le_bytes());
                row[0x18..0x1C].copy_from_slice(&1.0f32.to_le_bytes());
                row
            }))
            .unwrap(),
            16_777_217
        );
    }

    #[test]
    fn resolve_context_reads_mode_terrain_and_descriptor_rows() {
        let context = context_tables(&[]);
        let tables = roster_tables(Vec::new());
        let resolved = resolve_context(0, &tables, &context).unwrap();
        assert_eq!(
            resolved,
            ResolvedContext {
                auxiliary_mode: 0x20,
                // Seed 0 misses the zero threshold and splits to class 1.
                mode_branch: 1,
                mode_draws: 3,
                mode_row_index: Some(1),
                terrain_row_index: 0,
                terrain_value: MODE_VALUE,
                used_filtered_pool: true,
                selector: 0,
                flags: [false, false, false],
                descriptor_draws: 4,
            }
        );
    }

    #[test]
    fn descriptor_selector_requires_one_unique_table_value() {
        let context = context_tables(&[(AUXILIARY_DESCRIPTOR_SELECTOR_KEY, 10_000)]);
        let unique = roster_tables(vec![enemy_row(0), enemy_row(7), enemy_row(7)]);
        let resolved = resolve_context(0, &unique, &context).unwrap();
        assert_eq!(resolved.selector, 7);
        assert_eq!(resolved.flags, [false, false, false]);
        assert_eq!(resolved.descriptor_draws, 4);

        let ambiguous = roster_tables(vec![enemy_row(7), enemy_row(9)]);
        assert!(matches!(
            resolve_context(0, &ambiguous, &context),
            Err(EnemyError::Unsupported(_))
        ));
    }

    #[test]
    fn absent_duplicated_and_inconsistent_rows_fail_closed() {
        let mut context = context_tables(&[]);
        context
            .optional_multipliers
            .retain(|row| row_key(row) != AUXILIARY_DESCRIPTOR_SELECTOR_KEY);
        assert!(matches!(
            optional_threshold(&context, AUXILIARY_DESCRIPTOR_SELECTOR_KEY),
            Err(EnemyError::MissingData(_))
        ));

        let mut duplicated = context_tables(&[]);
        duplicated
            .optional_multipliers
            .push(optional_row(AUXILIARY_MODE_THRESHOLD_KEY, 3, 1.0));
        assert!(matches!(
            optional_threshold(&duplicated, AUXILIARY_MODE_THRESHOLD_KEY),
            Err(EnemyError::InvalidInput(_))
        ));

        let mut ambiguous_context = context_tables(&[]);
        ambiguous_context.contexts.push(context_row(0x20, 1));
        assert!(matches!(
            resolve_context(0, &roster_tables(Vec::new()), &ambiguous_context),
            Err(EnemyError::InvalidInput(_))
        ));

        let mut short_keys = roster_tables(Vec::new());
        short_keys.terrain_keys.pop();
        assert!(matches!(
            resolve_context(0, &short_keys, &context_tables(&[])),
            Err(EnemyError::InvalidInput(_))
        ));

        let mut empty_pool = roster_tables(Vec::new());
        empty_pool.terrains = vec![
            terrain_row(TERRAIN_EXCLUDED_BIT, MODE_VALUE),
            terrain_row(TERRAIN_EXCLUDED_BIT, MODE_VALUE),
        ];
        assert!(matches!(
            resolve_context(0, &empty_pool, &context_tables(&[])),
            Err(EnemyError::MissingData(_))
        ));

        let mut empty_table = roster_tables(Vec::new());
        empty_table.terrains = Vec::new();
        assert!(matches!(
            resolve_context(0, &empty_table, &context_tables(&[])),
            Err(EnemyError::MissingData(_))
        ));
    }
}
